// SPDX-License-Identifier: GPL-3.0-or-later
//! The animation scheduler on a real worker pool: no lost or duplicated results,
//! double buffering, cancellation, and agreement with synchronous evaluation.

mod common;

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use common::*;
use glam::Mat4;
use tessera_anim::{
    AnimError, AnimationScheduler, Animator, AttachFrame, AttachPointDesc, AttachTable,
    EvalScratch, Playback, SchedulerConfig, SubmitError, Submitted, Wrap, evaluate,
    skinning_matrices,
};
use tessera_assets::{CancelToken, PoolConfig, Priority, WorkerPool};
use tessera_havok::Transform;

const WAIT: Duration = Duration::from_secs(20);

fn pool(workers: usize) -> WorkerPool {
    WorkerPool::new(PoolConfig {
        workers,
        ..PoolConfig::default()
    })
}

/// Occupies every worker of `pool` until the returned sender is dropped or sent to.
fn block_workers(pool: &WorkerPool, workers: usize) -> Vec<mpsc::Sender<()>> {
    let mut release = Vec::new();
    let (started_tx, started_rx) = mpsc::channel();
    for _ in 0..workers {
        let (tx, rx) = mpsc::channel::<()>();
        let started = started_tx.clone();
        pool.spawn(Priority::Immediate, CancelToken::new(), move |_| {
            started.send(()).unwrap();
            let _ = rx.recv_timeout(Duration::from_secs(20));
        })
        .unwrap();
        release.push(tx);
    }
    for _ in 0..workers {
        started_rx.recv_timeout(WAIT).unwrap();
    }
    release
}

#[test]
fn results_are_neither_lost_nor_duplicated() {
    let p = pool(3);
    let s = skeleton(&tree_spec(24));
    let clip = bind(&spline_clip(61, 24, 60.0, cyclic(&s, 61, 0.5)), &s);
    let mut sched = AnimationScheduler::new(&p, SchedulerConfig::default());
    let mut instances = Vec::new();
    for i in 0..40 {
        let id = sched.add(s.clone(), None).unwrap();
        let mut a = Animator::new(s.clone());
        let mut pb = Playback::new(clip.clone(), Wrap::Loop);
        pb.time = i as f32 * 0.01;
        a.play(0, pb, 0.0, false).unwrap();
        instances.push((id, a));
    }
    let mut queued = 0u64;
    let mut busy = 0u64;
    let mut completed = 0u64;
    let mut last_frame = vec![0u64; instances.len()];
    for frame in 1..=60u64 {
        completed += sched.collect().completed as u64;
        for (k, (id, a)) in instances.iter_mut().enumerate() {
            // The front buffer only ever moves forward in frames.
            let pose = sched.pose(*id).unwrap();
            assert!(pose.frame >= last_frame[k]);
            last_frame[k] = pose.frame;
            a.advance(1.0 / 60.0);
            match sched.submit(*id, a, frame).unwrap() {
                Submitted::Queued => queued += 1,
                Submitted::Busy => {
                    busy += 1;
                    assert!(sched.is_busy(*id));
                }
            }
        }
        assert!(
            sched.stats().in_flight <= instances.len(),
            "at most one job per instance"
        );
    }
    let rest = sched.collect_all(WAIT);
    completed += rest.completed as u64;
    let st = sched.stats();
    assert_eq!(st.in_flight, 0);
    assert_eq!(st.submitted, queued);
    assert_eq!(st.busy, busy);
    assert_eq!(completed, queued, "every queued job came back exactly once");
    assert_eq!(st.completed, queued);
    assert_eq!(st.cancelled, 0);
    // Every instance ends with a pose from its last queued frame.
    for (id, _) in &instances {
        assert!(sched.has_pose(*id));
        assert!(!sched.is_busy(*id));
        assert!(sched.pose(*id).unwrap().frame >= 1);
    }
}

#[test]
fn worker_pose_equals_synchronous_evaluation() {
    let p = pool(2);
    let s = skeleton(&tree_spec(16));
    let clip = bind(&spline_clip(61, 16, 60.0, cyclic(&s, 61, 0.7)), &s);
    let attach = Arc::new(
        AttachTable::new(
            &[
                AttachPointDesc {
                    key: 18,
                    bone: 9,
                    frame: AttachFrame {
                        position: [0.0, 0.1, 0.0],
                        ..AttachFrame::IDENTITY
                    },
                },
                AttachPointDesc {
                    key: 1,
                    bone: -1,
                    frame: AttachFrame::IDENTITY,
                },
            ],
            16,
        )
        .unwrap(),
    );
    let mut sched = AnimationScheduler::new(&p, SchedulerConfig::default());
    let id = sched.add(s.clone(), Some(attach.clone())).unwrap();
    // Before any job: the reference pose, with attach transforms.
    assert!(!sched.has_pose(id));
    let pose0 = sched.pose(id).unwrap();
    assert_eq!(pose0.frame, 0);
    assert_eq!(pose0.attach.len(), 2);
    let mut a = Animator::new(s.clone());
    a.play(0, Playback::new(clip, Wrap::Loop), 0.0, false)
        .unwrap();
    a.advance(0.4);
    assert_eq!(sched.submit(id, &a, 7).unwrap(), Submitted::Queued);
    assert_eq!(sched.submit(id, &a, 7).unwrap(), Submitted::Busy);
    let c = sched.collect_all(WAIT);
    assert_eq!(c.completed, 1);
    let out = sched.pose(id).unwrap();
    assert_eq!(out.frame, 7);
    assert!(out.error.is_none());

    let mut snap = Vec::new();
    a.snapshot(&mut snap);
    let mut local = vec![Transform::IDENTITY; 16];
    evaluate(&s, &snap, &mut EvalScratch::new(), &mut local).unwrap();
    let mut model = vec![Mat4::IDENTITY; 16];
    let mut triples = vec![Transform::IDENTITY; 16];
    s.compose_with_triples(&local, &mut model, &mut triples)
        .unwrap();
    let mut skin = vec![Mat4::IDENTITY; 16];
    skinning_matrices(&s, &triples, &mut skin).unwrap();
    assert_eq!(out.local, local);
    assert_eq!(out.model, model);
    assert_eq!(out.skin, skin);
    let expect_attach = attach.model_transform(18, &model).unwrap();
    let got = out.attach.iter().find(|(k, _)| *k == 18).unwrap().1;
    assert_eq!(got, expect_attach);
    assert!(out.worker_time > Duration::ZERO);
}

#[test]
fn cancellation_returns_buffers_and_keeps_poses() {
    let workers = 2;
    let p = pool(workers);
    let s = skeleton(&tree_spec(8));
    let clip = bind(&spline_clip(31, 8, 30.0, cyclic(&s, 31, 0.4)), &s);
    let mut sched = AnimationScheduler::new(&p, SchedulerConfig::default());
    let mut a = Animator::new(s.clone());
    a.play(0, Playback::new(clip, Wrap::Loop), 0.0, false)
        .unwrap();
    let ids: Vec<_> = (0..10)
        .map(|_| sched.add(s.clone(), None).unwrap())
        .collect();

    // With every worker blocked, queued jobs are cancelled before they run.
    let release = block_workers(&p, workers);
    for id in &ids {
        assert_eq!(sched.submit(*id, &a, 1).unwrap(), Submitted::Queued);
    }
    assert_eq!(sched.stats().in_flight, 10);
    sched.cancel_all();
    // Removing an instance with a job out: its result is never delivered.
    assert!(sched.remove(ids[9]));
    assert!(!sched.remove(ids[9]));
    drop(release);
    let c = sched.collect_all(WAIT);
    assert_eq!(c.completed, 0);
    assert_eq!(c.cancelled + c.stale, 10);
    assert_eq!(c.stale, 1);
    assert_eq!(sched.stats().in_flight, 0);
    for id in &ids[..9] {
        assert!(
            !sched.has_pose(*id),
            "cancelled jobs leave the previous pose"
        );
        assert!(!sched.is_busy(*id), "buffers came back");
    }
    // The instances keep working after a cancel.
    for id in &ids[..9] {
        assert_eq!(sched.submit(*id, &a, 2).unwrap(), Submitted::Queued);
    }
    let c = sched.collect_all(WAIT);
    assert_eq!(c.completed, 9);
    for id in &ids[..9] {
        assert_eq!(sched.pose(*id).unwrap().frame, 2);
    }
    assert!(matches!(
        sched.submit(ids[9], &a, 3),
        Err(AnimError::UnknownInstance { .. })
    ));
}

#[test]
fn removal_while_running_discards_result() {
    let p = pool(1);
    let s = skeleton(&tree_spec(8));
    let clip = bind(&spline_clip(31, 8, 30.0, cyclic(&s, 31, 0.4)), &s);
    let mut sched = AnimationScheduler::new(&p, SchedulerConfig::default());
    let mut a = Animator::new(s.clone());
    a.play(0, Playback::new(clip, Wrap::Loop), 0.0, false)
        .unwrap();
    let id = sched.add(s.clone(), None).unwrap();
    sched.submit(id, &a, 1).unwrap();
    sched.remove(id);
    let c = sched.collect_all(WAIT);
    // Either the job was withdrawn (cancelled) or it ran before removal (stale): never
    // delivered to an instance, and never lost.
    assert_eq!(c.completed, 0);
    assert_eq!(c.cancelled + c.stale, 1);
    assert!(sched.pose(id).is_none());
}

#[test]
fn mismatched_animator_and_full_queue() {
    let p = WorkerPool::new(PoolConfig {
        workers: 1,
        max_queued: 1,
        ..PoolConfig::default()
    });
    let s = skeleton(&tree_spec(8));
    let other = skeleton(&tree_spec(9));
    let mut sched = AnimationScheduler::new(&p, SchedulerConfig::default());
    let id = sched.add(s.clone(), None).unwrap();
    let id2 = sched.add(s.clone(), None).unwrap();
    assert!(matches!(
        sched.submit(id, &Animator::new(other), 1),
        Err(AnimError::TrackCountMismatch { .. })
    ));
    let release = block_workers(&p, 1);
    let a = Animator::new(s.clone());
    assert_eq!(sched.submit(id, &a, 1).unwrap(), Submitted::Queued);
    // The queue (capacity 1) is full: the pool refuses; the buffers still come back.
    assert!(matches!(
        sched.try_submit(id2, &a, 1),
        Err(SubmitError::Pool(_))
    ));
    drop(release);
    let c = sched.collect_all(WAIT);
    assert_eq!(c.completed, 1);
    // A refusal is counted as refused, neither submitted nor cancelled.
    assert_eq!(c.cancelled, 0);
    assert_eq!(sched.stats().refused, 1);
    assert_eq!(sched.stats().submitted, 1);
    assert!(!sched.is_busy(id2));
    assert_eq!(sched.submit(id2, &a, 2).unwrap(), Submitted::Queued);
    assert_eq!(sched.collect_all(WAIT).completed, 1);
    let limited = AnimationScheduler::new(
        &p,
        SchedulerConfig {
            max_instances: 0,
            ..SchedulerConfig::default()
        },
    );
    let mut limited = limited;
    assert!(limited.add(s, None).is_err());
}
