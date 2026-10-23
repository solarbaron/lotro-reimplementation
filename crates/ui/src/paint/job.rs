// SPDX-License-Identifier: GPL-3.0-or-later
//! Paint jobs over horizontal bands (decision 0015 Decision 1.5, §6 "Spreading large
//! paints").
//!
//! A job takes an immutable paint snapshot and the canvas size at its start and paints the
//! merged damage of that moment in horizontal bands of [`BAND_ROWS`] rows, as many per frame
//! as the paint slice allows. It is never restarted or cancelled; it always finishes at its
//! own size. Because painting is a pure function of the snapshot, a band can later move to
//! a worker without changing this interface.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::display::DisplayList;
use super::geom::Rect;
use super::painter::{CanvasSet, ops_meeting, paint_rect_ops};

/// Rows of one band.
pub const BAND_ROWS: i32 = 16;

/// Decides when a frame's paint slice is used up. The scheduler always paints at least one
/// band per frame, so every job finishes after a bounded number of frames.
pub trait PaintBudget {
    /// Whether no more bands may be painted in this frame.
    fn exhausted(&mut self) -> bool;
}

/// A wall-clock slice (U4 (b): 4 ms on the main thread).
#[derive(Debug, Clone, Copy)]
pub struct TimeSlice {
    deadline: Instant,
}

impl TimeSlice {
    /// A slice of `slice` starting now.
    #[must_use]
    pub fn new(slice: Duration) -> Self {
        Self {
            deadline: Instant::now() + slice,
        }
    }
}

impl PaintBudget for TimeSlice {
    fn exhausted(&mut self) -> bool {
        Instant::now() >= self.deadline
    }
}

/// A fixed number of bands per frame (deterministic tests and probes): `BandBudget(n)`
/// allows `n` bands, counting the first. Because at least one band is always painted,
/// `BandBudget(0)` and `BandBudget(1)` both mean one band.
///
/// The painters ask [`PaintBudget::exhausted`] once after each band (before the next), so
/// each call counts one band painted.
#[derive(Debug, Clone, Copy)]
pub struct BandBudget(pub u32);

impl PaintBudget for BandBudget {
    fn exhausted(&mut self) -> bool {
        self.0 = self.0.saturating_sub(1);
        self.0 == 0
    }
}

/// No limit: every job runs to completion in the frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct Unlimited;

impl PaintBudget for Unlimited {
    fn exhausted(&mut self) -> bool {
        false
    }
}

/// One paint job of one surface.
#[derive(Debug, Clone)]
pub struct PaintJob {
    snapshot: Arc<DisplayList>,
    size: [u32; 2],
    rects: Vec<Rect>,
    next: usize,
    next_y: i32,
    /// The snapshot's operations that meet the job's damage box, for every band.
    candidates: Vec<u32>,
}

impl PaintJob {
    /// A job that paints `rects` (merged damage, clipped to the canvas) of a canvas of
    /// `size` from `snapshot`.
    #[must_use]
    pub fn new(snapshot: Arc<DisplayList>, size: [u32; 2], rects: Vec<Rect>) -> Self {
        let canvas = Rect::of_size(size[0], size[1]);
        let rects: Vec<Rect> = rects
            .into_iter()
            .map(|r| r.intersect(&canvas))
            .filter(|r| !r.is_empty())
            .collect();
        let next_y = rects.first().map_or(0, |r| r.y0);
        let damage = rects.iter().fold(Rect::EMPTY, |b, r| b.union(r));
        let candidates = ops_meeting(&snapshot, &damage);
        Self {
            snapshot,
            size,
            rects,
            next: 0,
            next_y,
            candidates,
        }
    }

    /// The snapshot.
    #[must_use]
    pub fn snapshot(&self) -> &Arc<DisplayList> {
        &self.snapshot
    }

    /// The canvas size the job paints at.
    #[must_use]
    pub fn size(&self) -> [u32; 2] {
        self.size
    }

    /// The rectangles the job paints.
    #[must_use]
    pub fn rects(&self) -> &[Rect] {
        &self.rects
    }

    /// The bounding box of the job's rectangles.
    #[must_use]
    pub fn damage_box(&self) -> Rect {
        self.rects.iter().fold(Rect::EMPTY, |b, r| b.union(r))
    }

    /// Whether no band is painted yet.
    #[must_use]
    pub fn is_fresh(&self) -> bool {
        self.next == 0 && self.rects.first().is_none_or(|r| self.next_y == r.y0)
    }

    /// Whether every band is painted.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.next >= self.rects.len()
    }

    /// The part not painted yet: the rest of the current rectangle and every later one.
    #[must_use]
    pub fn remaining(&self) -> Vec<Rect> {
        let mut out = Vec::new();
        if let Some(r) = self.rects.get(self.next) {
            out.push(Rect::new(r.x0, self.next_y, r.x1, r.y1));
            out.extend_from_slice(&self.rects[self.next + 1..]);
        }
        out
    }

    /// Paints the next band into `set` (which must have the job's size) and returns it, or
    /// `None` when the job is complete.
    pub fn paint_band(&mut self, set: &mut CanvasSet) -> Option<Rect> {
        let r = *self.rects.get(self.next)?;
        let y1 = self.next_y.saturating_add(BAND_ROWS).min(r.y1);
        let band = Rect::new(r.x0, self.next_y, r.x1, y1);
        paint_rect_ops(&self.snapshot, Some(&self.candidates), set, band);
        if y1 >= r.y1 {
            self.next += 1;
            self.next_y = self.rects.get(self.next).map_or(0, |n| n.y0);
        } else {
            self.next_y = y1;
        }
        Some(band)
    }

    /// Paints bands until the job completes or `budget` is used up (at least one band).
    /// Returns the bands painted.
    pub fn run(&mut self, set: &mut CanvasSet, budget: &mut dyn PaintBudget) -> Vec<Rect> {
        let mut out = Vec::new();
        while let Some(b) = self.paint_band(set) {
            out.push(b);
            if self.is_complete() || budget.exhausted() {
                break;
            }
        }
        out
    }
}
