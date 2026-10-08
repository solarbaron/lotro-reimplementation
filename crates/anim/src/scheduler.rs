// SPDX-License-Identifier: GPL-3.0-or-later
//! Pose sampling on the asset workers (`docs/ARCHITECTURE.md` §20.3 step 6, §21;
//! `skeleton-anim.md` §10).
//!
//! Each frame the main thread first [`collects`](AnimationScheduler::collect) whatever
//! sampling jobs finished (never blocking), then [`submits`](AnimationScheduler::submit)
//! one job per animated instance. A job evaluates the instance's layer snapshot, composes
//! model transforms (bone sliders and body shape are not applied here: they deform the
//! meshes in bind space, [`crate::BindDeformation`], `appearance.md` §6.4), computes the skinning matrices and the holding-location
//! transforms, and hands its buffers back through a channel.
//!
//! Every instance owns two buffer sets (double buffering): the **front** set holds the
//! last completed pose, which the main thread reads (render, attachments) while the
//! **back** set travels with the job. When a job completes, its set becomes the front and
//! the old front becomes the next back set. An instance whose previous job has not come
//! back yet is not resubmitted ([`Submitted::Busy`]): it keeps its last pose for a frame
//! rather than queue duplicate work. Every submitted job reports back exactly once,
//! completed or cancelled (also when the pool refuses it, or a job panics on a worker:
//! the job keeps its buffers until it reports), so no buffers or results are lost.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use glam::Mat4;
use tessera_assets::{AssetError, CancelToken, JobContext, Priority, TaskHandle, WorkerPool};
use tessera_havok::Transform;

use crate::attach::AttachTable;
use crate::clip::SampleReport;
use crate::error::{AnimError, Result};
use crate::player::{Animator, EvalScratch, LayerSnapshot, evaluate};
use crate::skeleton::Skeleton;
use crate::skin::skinning_matrices;

/// Identifies an animated instance registered with an [`AnimationScheduler`]. Never
/// reused within one scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InstanceId(u64);

impl InstanceId {
    /// The raw value.
    #[must_use]
    pub fn get(self) -> u64 {
        self.0
    }
}

/// The result of one sampling job: everything the renderer and the attachment code need.
#[derive(Debug, Clone, Default)]
pub struct PoseOutput {
    /// The frame number passed to [`AnimationScheduler::submit`].
    pub frame: u64,
    /// Local transforms per bone.
    pub local: Vec<Transform>,
    /// Model-space transforms per bone (§5.3).
    pub model: Vec<Mat4>,
    /// Skinning matrices per bone (§4.4, §5.4), the palette indexed by bone index (§4.3).
    pub skin: Vec<Mat4>,
    /// Holding-location model transforms, (key, transform) in key order (§8.2).
    pub attach: Vec<(u32, Mat4)>,
    /// Tracks replaced by reference values while sampling.
    pub report: SampleReport,
    /// Time the worker spent on the job.
    pub worker_time: Duration,
    /// Why evaluation failed, if it did (the pose then holds the reference pose).
    pub error: Option<AnimError>,
}

#[derive(Debug, Default)]
struct Buffers {
    out: PoseOutput,
    layers: Vec<LayerSnapshot>,
    scratch: EvalScratch,
    /// Model triples of the pose (the input of the skinning matrices, §5.4).
    triples: Vec<Transform>,
}

impl Buffers {
    fn new(skeleton: &Skeleton) -> Box<Self> {
        let n = skeleton.bone_count();
        let mut b = Box::<Buffers>::default();
        b.out.local = skeleton.reference_pose();
        b.out.model = skeleton.reference_model().to_vec();
        b.out.skin = vec![Mat4::IDENTITY; n];
        b
    }
}

enum Msg {
    Done(InstanceId, Box<Buffers>),
    Dropped(InstanceId, Box<Buffers>),
}

/// What a job reads besides its buffers.
struct JobInputs {
    skeleton: Arc<Skeleton>,
    attach: Option<Arc<AttachTable>>,
}

/// Owns a job's buffers; sends them back exactly once, as completed or as dropped. The
/// buffers stay in the guard while the job works, so an unwinding panic still returns them
/// (as dropped) through `Drop`.
struct JobGuard {
    id: InstanceId,
    buffers: Option<Box<Buffers>>,
    tx: Sender<Msg>,
    inputs: JobInputs,
}

impl JobGuard {
    fn run(self, ctx: &JobContext<'_>) {
        if ctx.is_cancelled() {
            return; // Drop reports it.
        }
        self.execute(sample_pose);
    }

    /// Runs `work` on the buffers, then reports them as completed.
    fn execute(mut self, work: impl FnOnce(&JobInputs, &mut Buffers)) {
        let Some(b) = self.buffers.as_deref_mut() else {
            return;
        };
        work(&self.inputs, b);
        if let Some(b) = self.buffers.take() {
            let _ = self.tx.send(Msg::Done(self.id, b));
        }
    }
}

/// The work of one sampling job.
fn sample_pose(inputs: &JobInputs, b: &mut Buffers) {
    let start = Instant::now();
    let skeleton = &inputs.skeleton;
    let n = skeleton.bone_count();
    let Buffers {
        out,
        layers,
        scratch,
        triples,
    } = b;
    out.local.resize(n, Transform::IDENTITY);
    out.model.resize(n, Mat4::IDENTITY);
    out.skin.resize(n, Mat4::IDENTITY);
    out.error = None;
    out.report = match evaluate(skeleton, layers, scratch, &mut out.local) {
        Ok(r) => r,
        Err(e) => {
            out.local.copy_from_slice(skeleton.reference_local());
            out.error = Some(e);
            SampleReport::default()
        }
    };
    triples.resize(n, Transform::IDENTITY);
    let composed = skeleton
        .compose_with_triples(&out.local, &mut out.model, triples)
        .and_then(|fallbacks| {
            out.report.model_fallbacks = fallbacks;
            skinning_matrices(skeleton, triples, &mut out.skin)
        });
    if let Err(e) = composed {
        out.error.get_or_insert(e);
    }
    match &inputs.attach {
        Some(t) => t.model_transforms(&out.model, &mut out.attach),
        None => out.attach.clear(),
    }
    // Clip references are not needed until the next submit refills the snapshot.
    layers.clear();
    out.worker_time = start.elapsed();
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        if let Some(mut b) = self.buffers.take() {
            b.layers.clear();
            let _ = self.tx.send(Msg::Dropped(self.id, b));
        }
    }
}

struct Slot {
    skeleton: Arc<Skeleton>,
    attach: Option<Arc<AttachTable>>,
    /// The pool refused the last job; its buffers come back as a dropped message that is
    /// not a cancellation.
    refused_pending: bool,
    front: Box<Buffers>,
    has_result: bool,
    spare: Option<Box<Buffers>>,
    in_flight: Option<TaskHandle>,
    token: CancelToken,
}

/// Configuration of an [`AnimationScheduler`].
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Priority of sampling jobs in the shared queue. Poses are needed next frame, so the
    /// default is [`Priority::Immediate`].
    pub priority: Priority,
    /// Most instances the scheduler accepts.
    pub max_instances: usize,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            priority: Priority::Immediate,
            max_instances: 65_536,
        }
    }
}

/// What [`AnimationScheduler::submit`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Submitted {
    /// A job was queued.
    Queued,
    /// The previous job has not been collected yet; nothing was queued and the instance
    /// keeps its last pose.
    Busy,
}

/// Outcome counts of one [`AnimationScheduler::collect`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Collected {
    /// Jobs whose pose became the instance's front buffer.
    pub completed: usize,
    /// Jobs that were cancelled or abandoned (their instance keeps its previous pose).
    pub cancelled: usize,
    /// Results for instances removed in the meantime (discarded).
    pub stale: usize,
    /// Worker time of the completed jobs.
    pub worker_time: Duration,
}

/// Cumulative counters of an [`AnimationScheduler`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SchedulerStats {
    /// Registered instances.
    pub instances: usize,
    /// Jobs queued so far.
    pub submitted: u64,
    /// Submissions refused because the previous job was still out.
    pub busy: u64,
    /// Jobs the pool refused (queue full or shut down); not counted as submitted or
    /// cancelled.
    pub refused: u64,
    /// Jobs completed and collected.
    pub completed: u64,
    /// Jobs cancelled or abandoned.
    pub cancelled: u64,
    /// Results discarded because their instance was removed.
    pub stale: u64,
    /// Jobs submitted but not yet collected.
    pub in_flight: usize,
    /// Total worker time of collected jobs.
    pub worker_time: Duration,
}

/// Submits one pose-sampling job per animated instance per frame to a shared
/// [`WorkerPool`] and collects the results without blocking (double-buffered).
pub struct AnimationScheduler {
    pool: WorkerPool,
    config: SchedulerConfig,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    slots: HashMap<InstanceId, Slot>,
    next_id: u64,
    stats: SchedulerStats,
    /// The pool refused the last job: later refusals are logged at debug level only.
    pool_refusing: bool,
}

impl fmt::Debug for AnimationScheduler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnimationScheduler")
            .field("config", &self.config)
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}

impl AnimationScheduler {
    /// A scheduler submitting to `pool`.
    #[must_use]
    pub fn new(pool: &WorkerPool, config: SchedulerConfig) -> Self {
        let (tx, rx) = channel();
        Self {
            pool: pool.clone(),
            config,
            tx,
            rx,
            slots: HashMap::new(),
            next_id: 1,
            stats: SchedulerStats::default(),
            pool_refusing: false,
        }
    }

    /// Registers an animated instance of `skeleton` with optional holding locations; its
    /// pose is the reference pose until a job completes.
    ///
    /// # Errors
    ///
    /// [`AnimError::TooManyEntries`] when [`SchedulerConfig::max_instances`] instances
    /// exist.
    pub fn add(
        &mut self,
        skeleton: Arc<Skeleton>,
        attach: Option<Arc<AttachTable>>,
    ) -> Result<InstanceId> {
        if self.slots.len() >= self.config.max_instances {
            return Err(AnimError::TooManyEntries {
                table: "animation instances",
                count: self.slots.len() + 1,
                max: self.config.max_instances,
            });
        }
        let id = InstanceId(self.next_id);
        self.next_id += 1;
        let mut front = Buffers::new(&skeleton);
        if let Some(t) = &attach {
            t.model_transforms(&front.out.model, &mut front.out.attach);
        }
        let spare = Buffers::new(&skeleton);
        self.slots.insert(
            id,
            Slot {
                skeleton,
                attach,
                refused_pending: false,
                front,
                has_result: false,
                spare: Some(spare),
                in_flight: None,
                token: CancelToken::new(),
            },
        );
        Ok(id)
    }

    /// Removes an instance, cancelling its job if one is out. Returns false for an
    /// unknown id. A result that arrives later is discarded.
    pub fn remove(&mut self, id: InstanceId) -> bool {
        match self.slots.remove(&id) {
            Some(slot) => {
                slot.token.cancel();
                if let Some(h) = slot.in_flight {
                    h.cancel();
                }
                true
            }
            None => false,
        }
    }

    /// Number of registered instances.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether no instance is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Queues a sampling job for `id` from the animator's current state, tagged with
    /// `frame`. Does not block.
    ///
    /// # Errors
    ///
    /// [`AnimError::UnknownInstance`] for an unknown id,
    /// [`AnimError::TrackCountMismatch`] when the animator's skeleton has a different bone
    /// count than the instance's and [`AnimError::Mismatch`] when it has the same count but
    /// is a different rig (other parents or reference pose, [`Skeleton::same_rig`]). A full
    /// queue or a shut-down pool is reported as `Ok(Submitted::Busy)` (the instance keeps
    /// its pose) and counted in [`SchedulerStats::refused`]; the first refusal after a
    /// queued job is logged as a warning, later ones at debug level.
    /// [`AnimationScheduler::try_submit`] reports it as an error instead.
    pub fn submit(&mut self, id: InstanceId, animator: &Animator, frame: u64) -> Result<Submitted> {
        match self.try_submit(id, animator, frame) {
            Ok(s) => Ok(s),
            Err(SubmitError::Anim(e)) => Err(e),
            Err(SubmitError::Pool(e)) => {
                if std::mem::replace(&mut self.pool_refusing, true) {
                    tracing::debug!(error = %e, "animation job not queued");
                } else {
                    tracing::warn!(error = %e, "animation jobs not queued (pool refusing)");
                }
                Ok(Submitted::Busy)
            }
        }
    }

    /// Like [`AnimationScheduler::submit`], but reports pool failures (queue full, pool
    /// shut down) as errors. The buffers of a refused job come back at the next collect.
    ///
    /// # Errors
    ///
    /// [`SubmitError::Anim`] for an unknown id or a skeleton mismatch,
    /// [`SubmitError::Pool`] when the pool refused the job.
    pub fn try_submit(
        &mut self,
        id: InstanceId,
        animator: &Animator,
        frame: u64,
    ) -> std::result::Result<Submitted, SubmitError> {
        let Some(slot) = self.slots.get_mut(&id) else {
            return Err(SubmitError::Anim(AnimError::UnknownInstance { id: id.0 }));
        };
        let bones = slot.skeleton.bone_count();
        if !Arc::ptr_eq(animator.skeleton(), &slot.skeleton) {
            if animator.skeleton().bone_count() != bones {
                return Err(SubmitError::Anim(AnimError::TrackCountMismatch {
                    tracks: animator.skeleton().bone_count(),
                    bones,
                }));
            }
            if !animator.skeleton().same_rig(&slot.skeleton) {
                return Err(SubmitError::Anim(AnimError::Mismatch {
                    what: "animator skeleton",
                }));
            }
        }
        let Some(mut b) = slot.spare.take() else {
            self.stats.busy += 1;
            return Ok(Submitted::Busy);
        };
        animator.snapshot(&mut b.layers);
        b.out.frame = frame;
        let guard = JobGuard {
            id,
            buffers: Some(b),
            tx: self.tx.clone(),
            inputs: JobInputs {
                skeleton: Arc::clone(&slot.skeleton),
                attach: slot.attach.clone(),
            },
        };
        match self
            .pool
            .spawn(self.config.priority, slot.token.clone(), move |ctx| {
                guard.run(ctx);
            }) {
            Ok(h) => {
                slot.in_flight = Some(h);
                self.stats.submitted += 1;
                self.stats.in_flight += 1;
                self.pool_refusing = false;
                Ok(Submitted::Queued)
            }
            Err(e) => {
                // The guard was dropped inside `spawn` and sent the buffers back; the
                // message is still to be collected, so it counts as in flight until then.
                slot.refused_pending = true;
                self.stats.refused += 1;
                self.stats.in_flight += 1;
                Err(SubmitError::Pool(e))
            }
        }
    }

    fn take(&mut self, msg: Msg, c: &mut Collected) {
        let (id, b, done) = match msg {
            Msg::Done(id, b) => (id, b, true),
            Msg::Dropped(id, b) => (id, b, false),
        };
        self.stats.in_flight = self.stats.in_flight.saturating_sub(1);
        let Some(slot) = self.slots.get_mut(&id) else {
            c.stale += 1;
            self.stats.stale += 1;
            return;
        };
        slot.in_flight = None;
        if !done && std::mem::take(&mut slot.refused_pending) {
            // Buffers of a job the pool refused: already counted in `refused`.
            slot.spare = Some(b);
            return;
        }
        if done {
            c.completed += 1;
            c.worker_time += b.out.worker_time;
            self.stats.completed += 1;
            self.stats.worker_time += b.out.worker_time;
            let old = std::mem::replace(&mut slot.front, b);
            slot.spare = Some(old);
            slot.has_result = true;
        } else {
            c.cancelled += 1;
            self.stats.cancelled += 1;
            slot.spare = Some(b);
        }
    }

    /// Takes every finished job's result without blocking: completed poses become their
    /// instance's front buffer. Call once per frame before [`AnimationScheduler::submit`].
    pub fn collect(&mut self) -> Collected {
        let mut c = Collected::default();
        while let Ok(msg) = self.rx.try_recv() {
            self.take(msg, &mut c);
        }
        c
    }

    /// Waits (blocking) until no job is out or `timeout` passed, collecting as results
    /// arrive. For tests, tools and shutdown; never call it in the frame loop.
    pub fn collect_all(&mut self, timeout: Duration) -> Collected {
        let deadline = Instant::now() + timeout;
        let mut c = self.collect();
        while self.stats.in_flight > 0 {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            match self.rx.recv_timeout(deadline - now) {
                Ok(msg) => self.take(msg, &mut c),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
        c
    }

    /// The instance's last completed pose (the reference pose, frame 0, before the first
    /// job completes).
    #[must_use]
    pub fn pose(&self, id: InstanceId) -> Option<&PoseOutput> {
        self.slots.get(&id).map(|s| &s.front.out)
    }

    /// Whether a job has completed for the instance since it was added.
    #[must_use]
    pub fn has_pose(&self, id: InstanceId) -> bool {
        self.slots.get(&id).is_some_and(|s| s.has_result)
    }

    /// Whether the instance has a job out.
    #[must_use]
    pub fn is_busy(&self, id: InstanceId) -> bool {
        self.slots.get(&id).is_some_and(|s| s.spare.is_none())
    }

    /// Cancels every job that is out (queued jobs never run; running ones finish early or
    /// complete). Instances stay registered and get a fresh token.
    pub fn cancel_all(&mut self) {
        for slot in self.slots.values_mut() {
            slot.token.cancel();
            if let Some(h) = &slot.in_flight {
                h.cancel();
            }
            slot.token = CancelToken::new();
        }
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> SchedulerStats {
        SchedulerStats {
            instances: self.slots.len(),
            ..self.stats
        }
    }
}

impl Drop for AnimationScheduler {
    fn drop(&mut self) {
        for slot in self.slots.values() {
            slot.token.cancel();
            if let Some(h) = &slot.in_flight {
                h.cancel();
            }
        }
    }
}

/// Why [`AnimationScheduler::try_submit`] did not queue a job.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SubmitError {
    /// The instance or its animator is not usable.
    #[error(transparent)]
    Anim(AnimError),
    /// The worker pool refused the job.
    #[error(transparent)]
    Pool(AssetError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard() -> (JobGuard, Receiver<Msg>) {
        let (tx, rx) = channel();
        let skeleton = Arc::new(Skeleton::from_parts(&[-1, 0], &[Transform::IDENTITY; 2]).unwrap());
        let g = JobGuard {
            id: InstanceId(7),
            buffers: Some(Buffers::new(&skeleton)),
            tx,
            inputs: JobInputs {
                skeleton,
                attach: None,
            },
        };
        (g, rx)
    }

    /// Review finding: a panic inside the work lost the buffers, so the instance stayed
    /// busy for good. Now the buffers come back as dropped.
    #[test]
    fn panicking_job_returns_its_buffers() {
        let (g, rx) = guard();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            g.execute(|_, _| panic!("deliberate test panic"));
        }));
        assert!(r.is_err());
        match rx.try_recv() {
            Ok(Msg::Dropped(id, b)) => {
                assert_eq!(id, InstanceId(7));
                assert_eq!(b.out.local.len(), 2);
            }
            Ok(Msg::Done(..)) => panic!("reported as completed"),
            Err(e) => panic!("nothing reported: {e}"),
        }
        assert!(rx.try_recv().is_err(), "reported once");

        let (g, rx) = guard();
        g.execute(sample_pose);
        assert!(matches!(rx.try_recv(), Ok(Msg::Done(InstanceId(7), _))));
        assert!(rx.try_recv().is_err(), "reported once");
    }
}
