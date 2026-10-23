// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI's work on the asset workers (`ui-scaling.md` §8 S1 and S7, §6.5):
//!
//! - [`PoolSpawner`]: the UI asset worker pool as the paint scheduler's spawner
//!   (`Ui::set_paint_spawner`), so a paint job whose damage exceeds
//!   `tessera_ui::paint::WORKER_MIN_PX` (30,000) physical pixels runs off the frame thread
//!   and the UI's main-thread time stays within U1 (S1). The host sets it in the windowed
//!   and headless clients alike ([`super::UiHost::new`]).
//! - [`LadderPrefetch`]: after a change of the effective scale *Se* (and once the font
//!   ladders are registered at an *Se* other than 1), the printable ASCII range of every
//!   registered ladder face is derived at the new target heights on the workers
//!   (`tessera_ui::text::derive_ladder_step`, §6.5 last sentence) and handed to
//!   `Ui::accept_ladder_step` on the frame thread. The host asks at the change and holds the
//!   full repaint at the new content scale back until the settle (§4.5 step 4: *Se*
//!   unchanged for 150 ms; [`super::UiHost::settle_scale_repaint`] settles at once), polling
//!   the steps that came back just before it; so the settle's repaint finds the glyphs the
//!   workers derived in the meantime cached (S7: at most 5 ms per step on a worker, at most
//!   20 µs per glyph left on the frame thread). Elements that change for another reason
//!   during the preview are repainted at once and derive what they need on the frame thread,
//!   as before.
//!
//! Tessera choices (engineering): the content scale of the prefetch is *Se* (the scale of a
//! surface at fit factor 1; surfaces of other fit factors derive on demand as before) and the
//! text factor 1 (§7.5 applies it to a few texts only); `tessera-ui` does not say which fonts
//! are on screen, so every registered ladder face is derived, at most
//! [`MAX_PREFETCH_STEPS`] steps per scale and [`MAX_STEPS_IN_FLIGHT`] on the workers at once.
//! A new scale cancels the older scale's queued steps; results of an older scale are dropped
//! unseen.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use tessera_assets::{CancelToken, Priority, TaskHandle, WorkerPool};
use tessera_ui::Ui;
use tessera_ui::paint::PaintSpawner;
use tessera_ui::scaling::glyph::target_height;
use tessera_ui::text::{FontLadder, LadderStep, derive_ladder_step};

/// The UI asset worker pool as the paint scheduler's spawner (module docs). A job the pool
/// refuses (queue full, pool shut down) is dropped unrun, which the scheduler reports as a
/// failed worker job and repaints on the frame thread.
#[derive(Clone)]
pub struct PoolSpawner {
    pool: WorkerPool,
}

impl std::fmt::Debug for PoolSpawner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PoolSpawner")
    }
}

impl PoolSpawner {
    /// A spawner over `pool` (a clone shares the pool).
    #[must_use]
    pub fn new(pool: WorkerPool) -> Self {
        Self { pool }
    }
}

impl PaintSpawner for PoolSpawner {
    fn spawn(&self, work: Box<dyn FnOnce() + Send + 'static>) {
        // Visible: the surface's old picture shows until the job is back.
        if let Err(e) = self
            .pool
            .spawn(Priority::Visible, CancelToken::new(), move |_| work())
        {
            tracing::debug!("paint job refused by the UI workers: {e}");
        }
    }
}

/// Most ladder steps one change of *Se* derives ahead (Tessera bound: the plugin font enum
/// has 48 records).
pub const MAX_PREFETCH_STEPS: usize = 128;

/// Most ladder steps on the workers at once, of every generation (Tessera bound): a run of
/// *Se* changes (a resize drag below the design floor, a held scale key) never fills the UI
/// pool's queue, which the S1 paint jobs and the image decodes share. Steps over the bound
/// wait in [`LadderPrefetch`] and are spawned by a later [`LadderPrefetch::request`].
pub const MAX_STEPS_IN_FLIGHT: usize = MAX_PREFETCH_STEPS;

/// A step's one message back to the frame thread: the derived step when the job ran, `None`
/// when the job was dropped unrun (cancelled while queued, refused by a full queue, or the
/// pool shut down). Sent exactly once per spawn attempt, so the in-flight count stays exact.
struct Reply {
    tx: Sender<(u64, Option<LadderStep>)>,
    generation: u64,
    sent: bool,
}

impl Reply {
    fn send(mut self, step: LadderStep) {
        self.sent = true;
        let _ = self.tx.send((self.generation, Some(step)));
    }
}

impl Drop for Reply {
    fn drop(&mut self) {
        if !self.sent {
            let _ = self.tx.send((self.generation, None));
        }
    }
}

/// The ladder steps derived ahead of a settle (module docs).
///
/// Each change of *Se* starts a **generation**: the previous generation's queued jobs are
/// cancelled (they leave the pool's queue unrun; a job already running finishes and its
/// result is dropped), and the new targets are spawned at [`Priority::Prefetch`], at most
/// [`MAX_STEPS_IN_FLIGHT`] on the workers at once. Targets that could not be spawned (the
/// bound, or a full queue) stay pending and are spawned by the next [`Self::request`] at the
/// same scale; the host asks once per frame.
pub struct LadderPrefetch {
    ladders: Vec<FontLadder>,
    /// The *Se* of the current generation, as bits (`None`: none asked yet).
    scale: Option<u64>,
    /// Bumped on every new scale; results of an older one are dropped.
    generation: u64,
    /// The current generation's targets not spawned yet (spawned from the back).
    pending: Vec<(usize, i32)>,
    /// The current generation's spawned jobs, cancelled when the next one starts.
    handles: Vec<TaskHandle>,
    tx: Sender<(u64, Option<LadderStep>)>,
    rx: Receiver<(u64, Option<LadderStep>)>,
    /// Messages still to come back, of every generation (one per spawn attempt).
    in_flight: usize,
    /// Steps accepted so far.
    accepted: u64,
    /// Steps asked for and not accepted so far (an older scale, or dropped unrun).
    stale: u64,
}

impl std::fmt::Debug for LadderPrefetch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LadderPrefetch")
            .field("ladders", &self.ladders.len())
            .field("scale", &self.scale.map(f64::from_bits))
            .field("generation", &self.generation)
            .field("pending", &self.pending.len())
            .field("in_flight", &self.in_flight)
            .field("accepted", &self.accepted)
            .field("stale", &self.stale)
            .finish()
    }
}

impl Default for LadderPrefetch {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            ladders: Vec::new(),
            scale: None,
            generation: 0,
            pending: Vec::new(),
            handles: Vec::new(),
            tx,
            rx,
            in_flight: 0,
            accepted: 0,
            stale: 0,
        }
    }
}

/// The (ladder index, target height) pairs of `ladders` at effective scale `se` that derive
/// glyphs: every face's line height at *c* = `se`, *x* = 1 (`ui-scaling.md` §6.4.1), without
/// the heights the ladder holds exactly; at most [`MAX_PREFETCH_STEPS`], in ladder order.
#[must_use]
pub fn prefetch_targets(ladders: &[FontLadder], se: f64) -> Vec<(usize, i32)> {
    let mut out = BTreeSet::new();
    'ladders: for (i, l) in ladders.iter().enumerate() {
        let heights = l.heights();
        for h in &heights {
            let t = target_height(*h, se, 1.0);
            if heights.contains(&t) {
                continue;
            }
            if out.len() >= MAX_PREFETCH_STEPS {
                break 'ladders;
            }
            out.insert((i, t));
        }
    }
    out.into_iter().collect()
}

impl LadderPrefetch {
    /// Takes the registered ladders (`Ui::set_font_ladders`); the next [`Self::request`]
    /// derives for them even at an unchanged scale. Cancels the steps of the old ladders.
    pub fn set_ladders(&mut self, ladders: Vec<FontLadder>) {
        self.cancel_generation();
        self.ladders = ladders;
        self.scale = None;
    }

    /// Cancels the current generation: its queued jobs leave the queue unrun (their
    /// [`Reply`] reports them), its pending targets are forgotten.
    fn cancel_generation(&mut self) {
        for h in self.handles.drain(..) {
            h.cancel();
        }
        self.pending.clear();
    }

    /// Asks the workers of `pool` for the steps of effective scale `se`. A new scale starts a
    /// new generation (the older one is cancelled); at the scale already asked for it only
    /// spawns the targets still pending. At *Se* = 1 every face is exact and nothing is
    /// asked. Returns the steps spawned by this call.
    pub fn request(&mut self, se: f64, pool: &WorkerPool) -> usize {
        if !se.is_finite() || se <= 0.0 || self.ladders.is_empty() {
            return 0;
        }
        if self.scale != Some(se.to_bits()) {
            self.cancel_generation();
            self.scale = Some(se.to_bits());
            self.generation = self.generation.wrapping_add(1);
            let mut targets = prefetch_targets(&self.ladders, se);
            targets.reverse();
            self.pending = targets;
        }
        self.spawn_pending(pool)
    }

    /// Spawns pending targets of the current generation while fewer than
    /// [`MAX_STEPS_IN_FLIGHT`] steps are out. A refused spawn keeps its target pending.
    fn spawn_pending(&mut self, pool: &WorkerPool) -> usize {
        let generation = self.generation;
        let mut spawned = 0;
        while let Some(&(i, target)) = self.pending.last() {
            if self.in_flight >= MAX_STEPS_IN_FLIGHT {
                break;
            }
            let Some(ladder) = self.ladders.get(i).cloned() else {
                self.pending.pop();
                continue;
            };
            let reply = Reply {
                tx: self.tx.clone(),
                generation,
                sent: false,
            };
            let job = move |ctx: &tessera_assets::JobContext<'_>| {
                if ctx.is_cancelled() {
                    // Dropping the reply reports the step as not run.
                    return;
                }
                reply.send(derive_ladder_step(&ladder, target));
            };
            // One message comes back for every attempt, also for a refused one (the pool
            // drops the closure, and with it the reply).
            self.in_flight += 1;
            match pool.spawn(Priority::Prefetch, CancelToken::new(), job) {
                Ok(h) => {
                    self.pending.pop();
                    self.handles.push(h);
                    spawned += 1;
                }
                Err(e) => {
                    tracing::debug!("ladder step refused by the UI workers: {e}");
                    break;
                }
            }
        }
        spawned
    }

    /// Hands the steps that came back for the current scale to `ui`
    /// (`Ui::accept_ladder_step`; their worker time reaches the frame's
    /// `PaintFrameStats::ladder_step_ns`) and drops older ones. Returns the steps accepted.
    pub fn poll(&mut self, ui: &mut Ui) -> usize {
        let mut n = 0;
        while let Ok((generation, step)) = self.rx.try_recv() {
            self.in_flight = self.in_flight.saturating_sub(1);
            match step {
                Some(step) if generation == self.generation => {
                    ui.accept_ladder_step(step);
                    self.accepted += 1;
                    n += 1;
                }
                _ => self.stale += 1,
            }
        }
        self.handles.retain(|h| !h.is_finished());
        n
    }

    /// Steps asked for that have not come back yet (every generation).
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.in_flight
    }

    /// Targets of the current scale not spawned yet.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// (steps accepted, steps asked for and dropped) so far: a step is dropped when it came
    /// back for an older scale or was never run (cancelled, refused).
    #[must_use]
    pub fn counts(&self) -> (u64, u64) {
        (self.accepted, self.stale)
    }

    /// The registered ladders.
    #[must_use]
    pub fn ladders(&self) -> &[FontLadder] {
        &self.ladders
    }
}

/// The spawner of `pool` as the `Arc` `Ui::set_paint_spawner` takes.
#[must_use]
pub fn paint_spawner(pool: &WorkerPool) -> Arc<dyn PaintSpawner> {
    Arc::new(PoolSpawner::new(pool.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_targets_without_ladders_or_at_scale_one() {
        assert!(prefetch_targets(&[], 1.5).is_empty());
        assert!(prefetch_targets(&[FontLadder::default()], 1.5).is_empty());
    }

    #[test]
    fn requests_need_ladders_and_a_new_scale() {
        let pool = WorkerPool::new(tessera_assets::PoolConfig {
            workers: 1,
            max_queued: 16,
            thread_name: "test".to_owned(),
        });
        let mut p = LadderPrefetch::default();
        assert_eq!(p.request(2.0, &pool), 0, "no ladders");
        p.set_ladders(vec![FontLadder::default()]);
        assert_eq!(p.request(f64::NAN, &pool), 0);
        assert_eq!(p.request(-1.0, &pool), 0);
        // An empty ladder has no heights: nothing to derive.
        assert_eq!(p.request(2.0, &pool), 0);
        assert_eq!(p.in_flight(), 0);
    }
}
