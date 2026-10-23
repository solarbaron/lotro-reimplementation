// SPDX-License-Identifier: GPL-3.0-or-later
//! `ui-scaling.md` §8 S1 through the element tree: with a worker pool set
//! (`Ui::set_paint_spawner`), a frame whose damage of one surface exceeds 30,000 physical
//! pixels has that paint run off the frame thread; a small damage is painted on the frame
//! thread, and the composition is the same either way. Synthetic controls only.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tessera_ui::paint::{PaintSpawner, Unlimited, WORKER_MIN_PX};
use tessera_ui::{ControlKind, ElementKey, MemoryEnv, Ui, Vocab};

/// Runs each item on a new thread and joins it before returning (deterministic), recording
/// whether it ran off the caller's thread.
#[derive(Default)]
struct Pool {
    spawned: AtomicUsize,
    off_thread: Mutex<Vec<bool>>,
}

impl PaintSpawner for Pool {
    fn spawn(&self, work: Box<dyn FnOnce() + Send + 'static>) {
        self.spawned.fetch_add(1, Ordering::SeqCst);
        let caller = std::thread::current().id();
        let worker = std::thread::spawn(move || {
            work();
            std::thread::current().id()
        })
        .join()
        .expect("worker");
        self.off_thread.lock().unwrap().push(worker != caller);
    }
}

fn settle(ui: &mut Ui, t: &mut u64) {
    for _ in 0..200 {
        let _ = ui.paint(Duration::from_millis(*t), &mut Unlimited);
        *t += 16;
        if ui.paint_settled() {
            return;
        }
    }
    panic!("paint did not settle");
}

/// A 300 × 200 window (60,000 pixels at the identity) with a 40 × 40 child control.
fn scene() -> (Ui, ElementKey, ElementKey) {
    let mut ui = Ui::new(Box::new(MemoryEnv::default()), Vocab::default(), (800, 600));
    let w = ui.create_control(ControlKind::Window, 1).expect("window");
    ui.control_set_position(w, 50, 40);
    ui.control_set_size(w, 300, 200);
    ui.control_set_back_colour(w, Some([1.0, 0.2, 0.3, 0.4]));
    ui.control_set_visible(w, true);
    let c = ui.create_control(ControlKind::Control, 1).expect("control");
    ui.control_set_parent(c, Some(w)).expect("parent");
    ui.control_set_position(c, 10, 10);
    ui.control_set_size(c, 40, 40);
    ui.control_set_back_colour(c, Some([1.0, 0.9, 0.1, 0.1]));
    ui.control_set_visible(c, true);
    (ui, w, c)
}

#[test]
fn a_large_damage_is_painted_off_the_frame_thread_and_a_small_one_is_not() {
    assert_eq!(WORKER_MIN_PX, 30_000, "the S1 threshold");
    let (mut ui, w, c) = scene();
    let (mut reference, rw, rc) = scene();
    let pool = Arc::new(Pool::default());
    ui.set_paint_spawner(Some(pool.clone()));
    let mut t = 0;
    let mut rt = 0;
    settle(&mut ui, &mut t);
    settle(&mut reference, &mut rt);
    assert_eq!(pool.spawned.load(Ordering::SeqCst), 0, "first paints stay");
    // The window's whole back colour changes: 60,000 pixels of damage.
    ui.control_set_back_colour(w, Some([1.0, 0.6, 0.5, 0.1]));
    reference.control_set_back_colour(rw, Some([1.0, 0.6, 0.5, 0.1]));
    let f = ui.paint(Duration::from_millis(t), &mut Unlimited);
    t += 16;
    assert_eq!(pool.spawned.load(Ordering::SeqCst), 1, "handed to a worker");
    assert_eq!(*pool.off_thread.lock().unwrap(), vec![true]);
    assert_eq!(f.stats.worker_jobs_started, 1);
    assert!(f.stats.worker_pixels > 30_000);
    assert_eq!(f.stats.bands, 0, "no band on the frame thread");
    settle(&mut ui, &mut t);
    settle(&mut reference, &mut rt);
    for (x, y) in [(5, 5), (200, 150), (20, 20)] {
        assert_eq!(
            ui.surface_pixel(w, x, y),
            reference.surface_pixel(rw, x, y),
            "({x}, {y}) as painted on the frame thread"
        );
    }
    // The child's colour: 1,600 pixels, on the frame thread.
    ui.control_set_back_colour(c, Some([1.0, 0.1, 0.9, 0.1]));
    reference.control_set_back_colour(rc, Some([1.0, 0.1, 0.9, 0.1]));
    let f = ui.paint(Duration::from_millis(t), &mut Unlimited);
    t += 16;
    assert_eq!(
        pool.spawned.load(Ordering::SeqCst),
        1,
        "not handed to a worker"
    );
    assert_eq!(f.stats.worker_jobs_started, 0);
    assert!(f.stats.bands > 0);
    settle(&mut ui, &mut t);
    settle(&mut reference, &mut rt);
    assert_eq!(
        ui.surface_pixel(w, 20, 20),
        reference.surface_pixel(rw, 20, 20)
    );
    assert!(!ui.paint_on_worker(w));
}

/// Drops every item without running it (a pool shut down, or a cancelled item).
struct DropPool;

impl PaintSpawner for DropPool {
    fn spawn(&self, work: Box<dyn FnOnce() + Send + 'static>) {
        drop(work);
    }
}

/// A worker thread that panics before it runs its item (the item unwinds with it).
struct PanicPool;

impl PaintSpawner for PanicPool {
    fn spawn(&self, work: Box<dyn FnOnce() + Send + 'static>) {
        let r = std::thread::spawn(move || {
            let _held = work;
            panic!("worker thread failed (test)");
        })
        .join();
        assert!(r.is_err());
    }
}

/// Holds items until `release`, then runs them (and every later one) on joined threads.
#[derive(Default)]
struct DeferPool {
    running: std::sync::atomic::AtomicBool,
    held: Mutex<Vec<Box<dyn FnOnce() + Send + 'static>>>,
}

impl DeferPool {
    fn release(&self) {
        self.running.store(true, Ordering::SeqCst);
        let held = std::mem::take(&mut *self.held.lock().unwrap());
        for w in held {
            std::thread::spawn(w).join().expect("worker");
        }
    }
}

impl PaintSpawner for DeferPool {
    fn spawn(&self, work: Box<dyn FnOnce() + Send + 'static>) {
        if self.running.load(Ordering::SeqCst) {
            std::thread::spawn(work).join().expect("worker");
        } else {
            self.held.lock().unwrap().push(work);
        }
    }
}

fn same_pixels(ui: &Ui, w: ElementKey, reference: &Ui, rw: ElementKey) {
    for (x, y) in [(5, 5), (200, 150), (20, 20)] {
        assert_eq!(
            ui.surface_pixel(w, x, y),
            reference.surface_pixel(rw, x, y),
            "({x}, {y}) as painted on the frame thread"
        );
    }
}

/// Review finding: a job the pool drops without running, or whose worker thread panics,
/// reports back as a failure; the surface is repainted in full and does not wait for good.
#[test]
fn a_dropped_or_failed_worker_job_is_repainted() {
    let pools: [Arc<dyn PaintSpawner>; 2] = [Arc::new(DropPool), Arc::new(PanicPool)];
    for pool in pools {
        let (mut ui, w, _) = scene();
        let (mut reference, rw, _) = scene();
        ui.set_paint_spawner(Some(pool));
        let (mut t, mut rt) = (0, 0);
        settle(&mut ui, &mut t);
        settle(&mut reference, &mut rt);
        ui.control_set_back_colour(w, Some([1.0, 0.6, 0.5, 0.1]));
        reference.control_set_back_colour(rw, Some([1.0, 0.6, 0.5, 0.1]));
        let f = ui.paint(Duration::from_millis(t), &mut Unlimited);
        t += 16;
        assert_eq!(f.stats.worker_jobs_started, 1);
        assert_eq!(f.stats.worker_failures, 1, "reported back");
        assert!(!ui.paint_on_worker(w), "not waiting for the lost job");
        // Without a pool the repaint runs on the frame thread and settles.
        ui.set_paint_spawner(None);
        settle(&mut ui, &mut t);
        settle(&mut reference, &mut rt);
        same_pixels(&ui, w, &reference, rw);
    }
}

/// Damage added while a job is on a worker is painted by a later job.
#[test]
fn damage_added_while_a_job_is_on_a_worker_is_painted_after_it() {
    let (mut ui, w, _) = scene();
    let (mut reference, rw, _) = scene();
    let pool = Arc::new(DeferPool::default());
    ui.set_paint_spawner(Some(pool.clone()));
    let (mut t, mut rt) = (0, 0);
    settle(&mut ui, &mut t);
    settle(&mut reference, &mut rt);
    ui.control_set_back_colour(w, Some([1.0, 0.6, 0.5, 0.1]));
    let _ = ui.paint(Duration::from_millis(t), &mut Unlimited);
    t += 16;
    assert!(ui.paint_on_worker(w));
    ui.control_set_back_colour(w, Some([1.0, 0.1, 0.2, 0.9]));
    reference.control_set_back_colour(rw, Some([1.0, 0.1, 0.2, 0.9]));
    for _ in 0..3 {
        let _ = ui.paint(Duration::from_millis(t), &mut Unlimited);
        t += 16;
    }
    assert!(ui.paint_on_worker(w), "still away");
    assert!(!ui.paint_settled());
    pool.release();
    settle(&mut ui, &mut t);
    settle(&mut reference, &mut rt);
    same_pixels(&ui, w, &reference, rw);
}

/// A surface released and created again while its job is on a worker ignores the stale
/// result when it comes back.
#[test]
fn a_surface_released_while_its_job_is_on_a_worker_ignores_the_result() {
    let (mut ui, w, _) = scene();
    let (mut reference, rw, _) = scene();
    let pool = Arc::new(DeferPool::default());
    ui.set_paint_spawner(Some(pool.clone()));
    let (mut t, mut rt) = (0, 0);
    settle(&mut ui, &mut t);
    settle(&mut reference, &mut rt);
    ui.control_set_back_colour(w, Some([1.0, 0.6, 0.5, 0.1]));
    let _ = ui.paint(Duration::from_millis(t), &mut Unlimited);
    t += 16;
    assert!(ui.paint_on_worker(w));
    // Hidden: the surface is released; shown again: a new one with a first paint.
    ui.control_set_visible(w, false);
    let _ = ui.paint(Duration::from_millis(t), &mut Unlimited);
    t += 16;
    assert!(!ui.paint_on_worker(w));
    ui.control_set_back_colour(w, Some([1.0, 0.1, 0.2, 0.9]));
    reference.control_set_back_colour(rw, Some([1.0, 0.1, 0.2, 0.9]));
    ui.control_set_visible(w, true);
    let _ = ui.paint(Duration::from_millis(t), &mut Unlimited);
    t += 16;
    // The stale job comes back now.
    pool.release();
    let f = ui.paint(Duration::from_millis(t), &mut Unlimited);
    t += 16;
    assert_eq!(f.stats.worker_failures, 0);
    settle(&mut ui, &mut t);
    settle(&mut reference, &mut rt);
    same_pixels(&ui, w, &reference, rw);
}
