// SPDX-License-Identifier: GPL-3.0-or-later
//! Paint jobs over frames, uploads, generations and first paints (decision 0015 Decision
//! 1.5, 1.8, §6 U3–U5; `render.md` §6.12).
//!
//! The [`Scheduler`] owns the CPU canvases of every surface and decides, once per frame
//! (frame-loop step 8), which jobs start, which bands are painted within the paint slice,
//! which rectangles are uploaded within the upload cap, and when textures are swapped. It
//! emits [`GpuOp`]s for the client, which applies them to `tessera-render`'s UI textures.
//!
//! **Rules implemented.**
//! - One job per surface at a time; it takes a snapshot and the canvas size at its start,
//!   paints the merged damage in bands, and is never restarted. Damage that arrives while it
//!   runs waits for the next job.
//! - **Content jobs** upload straight into the front texture in the frame they complete when
//!   the rectangles fit what is left of the upload cap and neither the canvas size nor the
//!   tile box changed; otherwise into a **back texture** (a GPU copy of the front, then the
//!   rectangles over as many frames as the cap needs), swapped when fully uploaded. Until the
//!   swap no new job starts on the surface.
//! - **Generations** (one per layout change of a group): each member's paint snapshot and
//!   merged damage are taken together when the generation starts, also for a member that is
//!   still busy with a content job (its generation job starts after it). Damage that arrives
//!   later stays in the member's list for a content job after the swap. A member's
//!   rectangles never reach a
//!   front texture before the generation swaps. A member whose size and tile box are
//!   unchanged **holds** them in its CPU canvas; the others use back textures. The
//!   generation swaps in the first frame in which every member is complete and the held
//!   rectangles fit what is left of the cap; if they never fit one frame, the holders move to
//!   back textures over the following frames. While it runs, the group is composed from the
//!   last swapped generation's geometry ([`Scheduler::shown`]). A layout change that arrives
//!   meanwhile waits for the next generation, which starts in the frame after the swap.
//!   While a surface belongs to a generation that has not swapped (running or waiting), no
//!   content job starts on it.
//! - **First paints** start at once (even while the group's generation waits) and are
//!   withheld until the job is complete, uploaded and the requested images have arrived, at
//!   most [`super::PaintLimits::first_paint_wait`]; after that, finished bands go straight
//!   into the texture as they complete. A first paint that belongs to a generation counts as
//!   complete for it under the same condition.
//! - **Scheduling of the shared slice and cap** (open review item, Tessera rule): first
//!   paints and generation jobs before content jobs, first-in first-out within each class,
//!   for both painting and uploading. A frame uploads first paints and generation work
//!   (back textures and the held rectangles of a swap) before any content job.
//! - **Tile boxes**: textures are sized to the tile-aligned box of the allocated tiles,
//!   decided once per job (at completion for content and generation jobs, at the first
//!   band with content for a first paint, whose box covers the rest of the canvas, or of the
//!   nested entry's coverage region for a canvas above). A box that grows, or whose
//!   allocated tiles shrink to half of it or less on an axis, goes through a back texture.
//!   A first paint's texture is shrunk once when the job completes (`render.md` §6.12).
//!   Freed tiles are cleared on the GPU, never uploaded as zeros.
//! - **Empty surfaces**: a snapshot of `None` (or one whose size cannot be stored, counted)
//!   leaves the surface with no canvas and no texture, so it has no entry (Decision 1.2);
//!   a generation member drops its texture at the swap.
//! - **Memory** (U5): painting allocates canvas tiles only within the canvas budget (the
//!   rest of the pixels is dropped, counted once per job). A back texture that does not fit
//!   the texture budget is not created; the surface keeps its front texture and waits
//!   (counted). A back texture that could never fit, even with every other texture
//!   released, is refused: the surface's textures are dropped (it draws nothing, counted)
//!   and a generation does not wait for it.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use super::PaintError;
use super::canvas::{Canvas, MAX_CANVAS_DIM, TILE};
use super::caps::{CapCounters, PaintLimits};
use super::display::DisplayList;
use super::geom::Rect;
use super::job::{PaintBudget, PaintJob};
use super::painter::{CanvasSet, Layer};

/// A surface, chosen by the caller (the element tree).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SurfaceId(pub u32);

/// A composition group: a root and everything derived from it (Decision 1.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupId(pub u32);

/// Which texture of a layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Slot {
    /// The texture that is shown.
    Front,
    /// The texture being filled for the next swap.
    Back,
}

/// One UI texture: a layer of a surface in a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TexKey {
    /// The surface.
    pub surface: SurfaceId,
    /// The canvas below or a canvas above.
    pub layer: Layer,
    /// Front or back.
    pub slot: Slot,
}

/// A GPU operation for the client to apply, in order, to `tessera-render`'s UI textures
/// (`render.md` §6.2, §6.3). Rectangles are in canvas coordinates; a texture covers its
/// `tile_box`, so texel (0, 0) is canvas pixel (`tile_box.x0`, `tile_box.y0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuOp {
    /// Create a transparent-black `Bgra8Unorm` texture covering `tile_box`.
    Create {
        /// The texture.
        tex: TexKey,
        /// Its box in canvas coordinates (tile-aligned, clipped to the canvas).
        tile_box: Rect,
    },
    /// Copy the overlap of two textures' boxes from `from` to `to` (same canvas
    /// coordinates).
    Copy {
        /// Source.
        from: TexKey,
        /// Destination.
        to: TexKey,
    },
    /// Upload `bytes` (tight rows of B, G, R, A) into `rect` of `tex`.
    Upload {
        /// The texture.
        tex: TexKey,
        /// The rectangle, inside the texture's box.
        rect: Rect,
        /// `rect.width() × rect.height() × 4` bytes.
        bytes: Vec<u8>,
    },
    /// Clear `rect` of `tex` to transparent black (a scissored clear draw).
    Clear {
        /// The texture.
        tex: TexKey,
        /// The rectangle, inside the texture's box.
        rect: Rect,
    },
    /// The back texture of `layer` becomes its front; the old front, if any, is destroyed.
    Promote {
        /// The surface.
        surface: SurfaceId,
        /// The layer.
        layer: Layer,
    },
    /// Destroy a texture.
    Destroy {
        /// The texture.
        tex: TexKey,
    },
}

/// The paint snapshot of a surface at a job's start.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// The display list (with the `Arc`ed images it names).
    pub list: Arc<DisplayList>,
    /// The canvas size (1..=the surface size cap on each axis; any other size is counted in
    /// [`CapCounters::invalid_snapshots`] and painted as an empty surface).
    pub size: [u32; 2],
}

/// Gives the scheduler a surface's current snapshot when a job starts.
pub trait SnapshotSource {
    /// The snapshot of `surface` now, or `None` when it has nothing to paint (an empty
    /// surface, Decision 1.1).
    fn snapshot(&mut self, surface: SurfaceId) -> Option<Snapshot>;
}

/// Per-frame figures (reported in `FrameStats`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaintFrameStats {
    /// Bands painted.
    pub bands: u32,
    /// Jobs started.
    pub jobs_started: u32,
    /// Jobs completed.
    pub jobs_completed: u32,
    /// Bytes uploaded (U3).
    pub upload_bytes: u64,
    /// Rectangles cleared on the GPU.
    pub clears: u32,
    /// Generations swapped.
    pub generations_swapped: u32,
    /// Bytes of CPU canvases after the frame (U5).
    pub canvas_bytes: u64,
    /// Bytes of UI textures after the frame (U5).
    pub texture_bytes: u64,
}

/// What a frame produced.
#[derive(Debug, Default)]
pub struct FrameOutput {
    /// GPU operations, in order.
    pub ops: Vec<GpuOp>,
    /// Groups whose generation swapped in this frame.
    pub swapped: Vec<GroupId>,
    /// First paints released (drawn for the first time) in this frame.
    pub first_drawn: Vec<SurfaceId>,
    /// Figures.
    pub stats: PaintFrameStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    FirstPaint,
    Content,
    Generation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// No job in flight.
    Idle,
    /// A job is painting.
    Painting,
    /// The job is complete; its rectangles are uploading into `queue_slot`.
    Uploading,
    /// The job is complete; its back textures do not fit the texture budget yet.
    AwaitingBack,
    /// A generation job is complete and holds its rectangles in the CPU canvas.
    Holding,
    /// A generation job is complete and uploaded into back textures.
    Ready,
}

#[derive(Debug, Default, Clone, Copy)]
struct LayerTex {
    front: Option<Rect>,
    back: Option<Rect>,
}

#[derive(Debug)]
struct SurfaceState {
    group: GroupId,
    canvases: Option<CanvasSet>,
    damage: super::damage::DamageList,
    textures: BTreeMap<Layer, LayerTex>,
    stage: Stage,
    kind: Kind,
    job: Option<PaintJob>,
    order: u64,
    /// Rectangles of the completed job (direct upload, hold, back queue).
    done_rects: Vec<Rect>,
    resized: bool,
    queue: VecDeque<(Layer, Rect)>,
    queue_slot: Slot,
    /// Coverage regions of the nested entries of the current job's snapshot.
    coverage: BTreeMap<u32, Rect>,
    generation: Option<u64>,
    /// The snapshot (`None`: nothing to paint) and merged damage taken when the generation
    /// started, for the member's generation job.
    gen_capture: Option<(Option<Snapshot>, Vec<Rect>)>,
    gen_first: bool,
    gen_job_started: bool,
    /// Time of the show for a surface that has never been drawn.
    first: Option<Duration>,
    first_started: bool,
    drawn: bool,
    images_pending: bool,
    /// Layers whose first-paint texture was refused for memory in the current job (counted
    /// once each).
    refused_layers: BTreeSet<Layer>,
    /// Whether the current job's canvas-memory refusal is counted.
    mem_counted: bool,
}

#[derive(Debug)]
struct Generation<G> {
    geometry: G,
    members: BTreeSet<SurfaceId>,
    migrating: bool,
}

#[derive(Debug)]
struct GroupState<G> {
    shown: Option<G>,
    running: Option<Generation<G>>,
    pending: Option<(G, BTreeSet<SurfaceId>)>,
}

/// The paint scheduler (see the module documentation). `G` is the caller's geometry of a
/// group (displayed boxes, coverage splits, positions): the scheduler keeps the last swapped
/// one for composition while a generation runs.
#[derive(Debug)]
pub struct Scheduler<G> {
    limits: PaintLimits,
    surfaces: BTreeMap<SurfaceId, SurfaceState>,
    groups: BTreeMap<GroupId, GroupState<G>>,
    /// Per-surface content scale `c` (`ui-scaling.md` §6.1), applied to the surface's
    /// [`super::CanvasSet`] when a job starts. Absent means `(1.0, 1.0)` (the crisp identity).
    surface_scale: BTreeMap<SurfaceId, [f64; 2]>,
    pending_ops: Vec<GpuOp>,
    next_order: u64,
    next_gen: u64,
    texture_bytes: u64,
    counters: CapCounters,
    warned_memory: bool,
    warned_snapshot: bool,
}

/// What [`Scheduler::go_back`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Back {
    /// Back textures created, rectangles queued.
    Started,
    /// They do not fit the texture budget now; the job waits.
    Wait,
    /// They could not fit even with every other texture released.
    Never,
}

fn bytes_of(r: &Rect) -> u64 {
    r.area() * 4
}

impl<G> Scheduler<G> {
    /// A scheduler with `limits`.
    #[must_use]
    pub fn new(limits: PaintLimits) -> Self {
        Self {
            limits,
            surfaces: BTreeMap::new(),
            surface_scale: BTreeMap::new(),
            groups: BTreeMap::new(),
            pending_ops: Vec::new(),
            next_order: 0,
            next_gen: 0,
            texture_bytes: 0,
            counters: CapCounters::default(),
            warned_memory: false,
            warned_snapshot: false,
        }
    }

    /// The limits.
    #[must_use]
    pub fn limits(&self) -> &PaintLimits {
        &self.limits
    }

    /// Fallback counters (memory waits and refusals).
    #[must_use]
    pub fn counters(&self) -> CapCounters {
        self.counters
    }

    /// Registers a new surface of `group`, shown at `now`: it needs a first paint.
    ///
    /// # Errors
    /// [`PaintError::DuplicateSurface`] when the id is in use.
    pub fn create_surface(
        &mut self,
        id: SurfaceId,
        group: GroupId,
        now: Duration,
    ) -> Result<(), PaintError> {
        if self.surfaces.contains_key(&id) {
            return Err(PaintError::DuplicateSurface(id.0));
        }
        self.groups.entry(group).or_insert_with(|| GroupState {
            shown: None,
            running: None,
            pending: None,
        });
        let mut damage = super::damage::DamageList::new();
        damage.add_full();
        self.surfaces.insert(
            id,
            SurfaceState {
                group,
                canvases: None,
                damage,
                textures: BTreeMap::new(),
                stage: Stage::Idle,
                kind: Kind::FirstPaint,
                job: None,
                order: 0,
                done_rects: Vec::new(),
                resized: false,
                queue: VecDeque::new(),
                queue_slot: Slot::Front,
                coverage: BTreeMap::new(),
                generation: None,
                gen_capture: None,
                gen_first: false,
                gen_job_started: false,
                first: Some(now),
                first_started: false,
                drawn: false,
                images_pending: false,
                refused_layers: BTreeSet::new(),
                mem_counted: false,
            },
        );
        Ok(())
    }

    /// Releases a surface (hidden, or its element destroyed): its canvases go now, its
    /// textures are destroyed by the next frame's operations.
    pub fn release_surface(&mut self, id: SurfaceId) {
        self.surface_scale.remove(&id);
        let Some(s) = self.surfaces.remove(&id) else {
            return;
        };
        for (layer, t) in &s.textures {
            for (slot, b) in [(Slot::Front, t.front), (Slot::Back, t.back)] {
                if let Some(b) = b {
                    self.texture_bytes = self.texture_bytes.saturating_sub(bytes_of(&b));
                    self.pending_ops.push(GpuOp::Destroy {
                        tex: TexKey {
                            surface: id,
                            layer: *layer,
                            slot,
                        },
                    });
                }
            }
        }
        if let Some(g) = self.groups.get_mut(&s.group) {
            if let Some(r) = &mut g.running {
                r.members.remove(&id);
            }
            if let Some((_, m)) = &mut g.pending {
                m.remove(&id);
            }
        }
    }

    /// Sets a surface's content scale `c` (`ui-scaling.md` §6.1), applied to its
    /// [`super::CanvasSet`] whenever a paint job starts. `(1.0, 1.0)` (the default) keeps the
    /// byte-exact crisp identity path. The UI sets it from the surface's UI-screen mapping.
    pub fn set_surface_scale(&mut self, id: SurfaceId, scale: [f64; 2]) {
        if scale == [1.0, 1.0] {
            self.surface_scale.remove(&id);
        } else {
            self.surface_scale.insert(id, scale);
        }
    }

    /// Removes a group and every surface in it.
    pub fn release_group(&mut self, group: GroupId) {
        let ids: Vec<SurfaceId> = self
            .surfaces
            .iter()
            .filter(|(_, s)| s.group == group)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.release_surface(id);
        }
        self.groups.remove(&group);
    }

    /// Adds damage in the surface's canvas coordinates (Decision 1.4).
    pub fn add_damage(&mut self, id: SurfaceId, r: Rect) {
        if let Some(s) = self.surfaces.get_mut(&id) {
            s.damage.add(r);
        }
    }

    /// Damages the whole canvas of a surface (a skin refresh, an image arriving everywhere).
    pub fn damage_all(&mut self, id: SurfaceId) {
        if let Some(s) = self.surfaces.get_mut(&id) {
            s.damage.add_full();
        }
    }

    /// Whether images requested by the surface's current states are still loading (part of
    /// the first-paint condition, Decision 1.8).
    pub fn set_images_pending(&mut self, id: SurfaceId, pending: bool) {
        if let Some(s) = self.surfaces.get_mut(&id) {
            s.images_pending = pending;
        }
    }

    /// Records a layout change of `group` (Decision 1.5): `geometry` is the group's new
    /// geometry and `members` the surfaces whose jobs belong to its generation (each whose
    /// canvas size changes, each parent whose nested entries change their coverage regions,
    /// each newly shown surface). Their damage must already be added. A change that
    /// arrives while a generation runs is merged into the next one.
    pub fn layout_change(&mut self, group: GroupId, geometry: G, members: &[SurfaceId]) {
        let g = self.groups.entry(group).or_insert_with(|| GroupState {
            shown: None,
            running: None,
            pending: None,
        });
        match &mut g.pending {
            Some((geo, m)) => {
                *geo = geometry;
                m.extend(members.iter().copied());
            }
            None => g.pending = Some((geometry, members.iter().copied().collect())),
        }
    }

    /// The geometry of the last swapped generation of `group` (what composition uses), or
    /// `None` before the group's first swap (nothing of the group is drawn).
    #[must_use]
    pub fn shown(&self, group: GroupId) -> Option<&G> {
        self.groups.get(&group).and_then(|g| g.shown.as_ref())
    }

    /// Whether a generation of `group` is running.
    #[must_use]
    pub fn generation_running(&self, group: GroupId) -> bool {
        self.groups.get(&group).is_some_and(|g| g.running.is_some())
    }

    /// Whether the surface has been drawn (its first paint released or swapped).
    #[must_use]
    pub fn is_drawn(&self, id: SurfaceId) -> bool {
        self.surfaces.get(&id).is_some_and(|s| s.drawn)
    }

    /// The box of the front texture of a layer (for the composition entry's offset), or
    /// `None` when the layer has no texture.
    #[must_use]
    pub fn front_box(&self, id: SurfaceId, layer: Layer) -> Option<Rect> {
        self.surfaces
            .get(&id)
            .and_then(|s| s.textures.get(&layer))
            .and_then(|t| t.front)
    }

    /// The layers of a surface that have a front texture, the canvas below first.
    #[must_use]
    pub fn front_layers(&self, id: SurfaceId) -> Vec<(Layer, Rect)> {
        self.surfaces.get(&id).map_or_else(Vec::new, |s| {
            s.textures
                .iter()
                .filter_map(|(l, t)| t.front.map(|b| (*l, b)))
                .collect()
        })
    }

    /// The canvases of a surface (the running job's; for tests and probes).
    #[must_use]
    pub fn canvases(&self, id: SurfaceId) -> Option<&CanvasSet> {
        self.surfaces.get(&id).and_then(|s| s.canvases.as_ref())
    }

    /// Whether the surface has a job in flight (painting, uploading, holding or waiting).
    #[must_use]
    pub fn is_busy(&self, id: SurfaceId) -> bool {
        self.surfaces
            .get(&id)
            .is_some_and(|s| s.stage != Stage::Idle)
    }

    /// Whether the scheduler has nothing left to do: no GPU operation queued, no
    /// generation running or pending, and every surface idle (no job in flight), drawn or
    /// released from its first paint, without damage and without images still loading.
    /// A frame run in this state paints no band and uploads nothing.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.pending_ops.is_empty()
            && self
                .groups
                .values()
                .all(|g| g.running.is_none() && g.pending.is_none())
            && self.surfaces.values().all(|s| {
                s.stage == Stage::Idle
                    && s.first.is_none()
                    && s.generation.is_none()
                    && s.damage.is_empty()
                    && !s.images_pending
            })
    }

    /// Bytes of all CPU canvases.
    #[must_use]
    pub fn canvas_bytes(&self) -> u64 {
        self.surfaces
            .values()
            .filter_map(|s| s.canvases.as_ref())
            .map(|c| c.allocated_bytes() as u64)
            .sum()
    }

    /// Bytes of all UI textures.
    #[must_use]
    pub fn texture_bytes(&self) -> u64 {
        self.texture_bytes
    }

    /// Runs one frame at time `now` (monotonic): starts generations and jobs, paints bands
    /// until `budget` is used up (at least one band when any job runs), uploads under the
    /// cap, and swaps.
    pub fn frame(
        &mut self,
        now: Duration,
        source: &mut dyn SnapshotSource,
        budget: &mut dyn PaintBudget,
    ) -> FrameOutput {
        let mut out = FrameOutput {
            ops: std::mem::take(&mut self.pending_ops),
            ..FrameOutput::default()
        };
        let mut left = self.limits.upload_cap_bytes;
        self.start_generations(source);
        self.start_jobs(source, &mut out);
        self.paint(budget, &mut out);
        // First paints and generation work take the cap before content jobs.
        self.upload_priority(&mut left, &mut out);
        self.release_first_paints(now, &mut out);
        self.swap_generations(now, &mut left, &mut out);
        self.upload_content(&mut left, &mut out);
        out.stats.upload_bytes = self.limits.upload_cap_bytes - left;
        out.stats.canvas_bytes = self.canvas_bytes();
        out.stats.texture_bytes = self.texture_bytes;
        out
    }

    fn start_generations(&mut self, source: &mut dyn SnapshotSource) {
        let gids: Vec<GroupId> = self.groups.keys().copied().collect();
        for gid in gids {
            let Some(g) = self.groups.get_mut(&gid) else {
                continue;
            };
            if g.running.is_some() {
                continue;
            }
            let Some((geometry, members)) = g.pending.take() else {
                continue;
            };
            let id = self.next_gen;
            self.next_gen += 1;
            let members: BTreeSet<SurfaceId> = members
                .into_iter()
                .filter(|m| self.surfaces.contains_key(m))
                .collect();
            for m in &members {
                let first = self.surfaces.get(m).is_some_and(|s| s.first.is_some());
                // The snapshot and the damage are taken together, now (Decision 1.5): a
                // member busy with a content job starts its generation job later with
                // them, and damage arriving meanwhile waits for a job after the swap.
                let snap = if first {
                    None
                } else {
                    self.valid_snapshot(source, *m)
                };
                if let Some(s) = self.surfaces.get_mut(m) {
                    s.generation = Some(id);
                    s.gen_first = first;
                    s.gen_job_started = first;
                    s.gen_capture = if first {
                        None
                    } else {
                        let rects = match &snap {
                            Some(sn) => s.damage.take_merged(sn.size),
                            None => {
                                s.damage = super::damage::DamageList::new();
                                Vec::new()
                            }
                        };
                        Some((snap, rects))
                    };
                }
            }
            if let Some(g) = self.groups.get_mut(&gid) {
                g.running = Some(Generation {
                    geometry,
                    members,
                    migrating: false,
                });
            }
        }
    }

    fn in_pending_generation(&self, id: SurfaceId, group: GroupId) -> bool {
        self.groups
            .get(&group)
            .and_then(|g| g.pending.as_ref())
            .is_some_and(|(_, m)| m.contains(&id))
    }

    /// The snapshot of `id`, or `None` when it has nothing to paint. A snapshot whose size
    /// cannot be stored (a side of 0, or over the surface size cap) counts as a categorised
    /// failure and as nothing to paint, so one bad snapshot cannot stall a group.
    fn valid_snapshot(
        &mut self,
        source: &mut dyn SnapshotSource,
        id: SurfaceId,
    ) -> Option<Snapshot> {
        let snap = source.snapshot(id)?;
        let cap = self.limits.max_surface_dim.clamp(1, MAX_CANVAS_DIM);
        let [w, h] = snap.size;
        if (1..=cap).contains(&w) && (1..=cap).contains(&h) {
            return Some(snap);
        }
        self.counters.invalid_snapshots += 1;
        if !self.warned_snapshot {
            self.warned_snapshot = true;
            tracing::warn!(
                surface = id.0,
                width = w,
                height = h,
                "UI surface snapshot size outside 1..={cap}: painted as empty"
            );
        }
        None
    }

    fn start_jobs(&mut self, source: &mut dyn SnapshotSource, out: &mut FrameOutput) {
        let ids: Vec<SurfaceId> = self.surfaces.keys().copied().collect();
        for id in ids {
            let Some(s) = self.surfaces.get(&id) else {
                continue;
            };
            if s.stage != Stage::Idle {
                continue;
            }
            let (kind, snap, captured) = if s.first.is_some() && !s.first_started {
                (Kind::FirstPaint, self.valid_snapshot(source, id), None)
            } else if s.generation.is_some() && !s.gen_job_started {
                let capture = self
                    .surfaces
                    .get_mut(&id)
                    .and_then(|s| s.gen_capture.take());
                match capture {
                    Some((snap, rects)) => (Kind::Generation, snap, Some(rects)),
                    None => (Kind::Generation, self.valid_snapshot(source, id), None),
                }
            } else if s.generation.is_none()
                && !self.in_pending_generation(id, s.group)
                && !s.damage.is_empty()
                && (s.first_started || s.first.is_none())
            {
                (Kind::Content, self.valid_snapshot(source, id), None)
            } else {
                continue;
            };
            let order = self.next_order;
            self.next_order += 1;
            let surface_scale = self.surface_scale.get(&id).copied().unwrap_or([1.0, 1.0]);
            let Some(s) = self.surfaces.get_mut(&id) else {
                continue;
            };
            match kind {
                Kind::FirstPaint => s.first_started = true,
                Kind::Generation => s.gen_job_started = true,
                Kind::Content => {}
            }
            s.refused_layers.clear();
            s.mem_counted = false;
            let before = s.canvases.as_ref().map(CanvasSet::size);
            // A snapshot's size is valid, so the canvases can be made; a failure is treated
            // like nothing to paint all the same.
            let snap = snap.and_then(|snap| {
                let ok = match &mut s.canvases {
                    Some(c) => c.resize(snap.size[0], snap.size[1]).is_ok(),
                    None => match CanvasSet::new(snap.size[0], snap.size[1]) {
                        Ok(c) => {
                            s.canvases = Some(c);
                            true
                        }
                        Err(_) => false,
                    },
                };
                ok.then_some(snap)
            });
            let Some(snap) = snap else {
                // Nothing to paint (Decision 1.1): the surface keeps no canvas, and no
                // texture (Decision 1.2: no allocated tile, no texture and no entry).
                if captured.is_none() {
                    s.damage = super::damage::DamageList::new();
                }
                s.canvases = None;
                s.coverage.clear();
                match kind {
                    Kind::FirstPaint => {}
                    // The texture goes in this frame.
                    Kind::Content => self.finish(id, out),
                    // The texture goes at the swap.
                    Kind::Generation => {
                        s.kind = Kind::Generation;
                        s.stage = Stage::Ready;
                    }
                }
                continue;
            };
            // `resize` keeps the content of an unchanged size: a new size repaints in full.
            let resized = before != Some(snap.size);
            let rects = match captured {
                _ if resized => {
                    // Damage taken with a generation's snapshot is in the full repaint;
                    // later damage stays for the next job.
                    if captured.is_none() {
                        s.damage = super::damage::DamageList::new();
                    }
                    vec![Rect::of_size(snap.size[0], snap.size[1])]
                }
                Some(rects) => rects,
                None => s.damage.take_merged(snap.size),
            };
            if let Some(c) = &mut s.canvases {
                c.set_scale(surface_scale);
                c.retain_nested(&snap.list);
            }
            s.coverage = snap.list.nested().collect();
            s.job = Some(PaintJob::new(snap.list, snap.size, rects));
            s.kind = kind;
            s.stage = Stage::Painting;
            s.order = order;
            s.resized = resized;
            s.done_rects.clear();
            out.stats.jobs_started += 1;
        }
    }

    /// Surfaces in priority order: first paints and generation jobs, then content jobs;
    /// first in, first out within each class.
    fn priority(&self, filter: impl Fn(&SurfaceState) -> bool) -> Vec<SurfaceId> {
        let mut v: Vec<(u8, u64, SurfaceId)> = self
            .surfaces
            .iter()
            .filter(|(_, s)| filter(s))
            .map(|(id, s)| (u8::from(s.kind == Kind::Content), s.order, *id))
            .collect();
        v.sort_unstable();
        v.into_iter().map(|(_, _, id)| id).collect()
    }

    fn paint(&mut self, budget: &mut dyn PaintBudget, out: &mut FrameOutput) {
        let ids = self.priority(|s| s.stage == Stage::Painting);
        // U5: the canvases of all surfaces share the canvas budget; each band may allocate
        // what the other surfaces leave.
        let limit = usize::try_from(self.limits.canvas_bytes).unwrap_or(usize::MAX);
        let mut total = usize::try_from(self.canvas_bytes()).unwrap_or(usize::MAX);
        let mut painted_any = false;
        'jobs: for id in ids {
            loop {
                if painted_any && budget.exhausted() {
                    break 'jobs;
                }
                let Some(s) = self.surfaces.get_mut(&id) else {
                    continue 'jobs;
                };
                let kind = s.kind;
                let (Some(job), Some(set)) = (&mut s.job, &mut s.canvases) else {
                    continue 'jobs;
                };
                let others = total.saturating_sub(set.allocated_bytes());
                set.set_byte_limit(limit.saturating_sub(others));
                let band = job.paint_band(set);
                total = others.saturating_add(set.allocated_bytes());
                if set.take_refused_tiles() > 0 && !s.mem_counted {
                    s.mem_counted = true;
                    self.counters.memory += 1;
                    if !self.warned_memory {
                        self.warned_memory = true;
                        tracing::warn!(
                            surface = id.0,
                            "UI canvas memory budget reached: pixels past it are not painted"
                        );
                    }
                }
                if let Some(b) = band {
                    painted_any = true;
                    out.stats.bands += 1;
                    if kind == Kind::FirstPaint {
                        self.first_paint_band(id, b, out);
                    }
                }
                let Some(s) = self.surfaces.get_mut(&id) else {
                    continue 'jobs;
                };
                if s.job.as_ref().is_none_or(PaintJob::is_complete) {
                    out.stats.jobs_completed += 1;
                    self.complete(id, out);
                    continue 'jobs;
                }
            }
        }
    }

    /// Queues a finished band of a first paint into the front textures, creating a layer's
    /// texture at its first band with content.
    fn first_paint_band(&mut self, id: SurfaceId, band: Rect, out: &mut FrameOutput) {
        let Some(s) = self.surfaces.get_mut(&id) else {
            return;
        };
        let Some(set) = &s.canvases else {
            return;
        };
        let bounds = Rect::of_size(set.size()[0], set.size()[1]);
        let mut creates = Vec::new();
        for (layer, canvas) in set.layers() {
            let has_front = s.textures.get(&layer).is_some_and(|t| t.front.is_some());
            if !has_front {
                let alloc = canvas.allocated_box();
                if alloc.is_empty() {
                    continue;
                }
                let mut rest = Rect::new(bounds.x0, band.y0, bounds.x1, bounds.y1);
                if let Layer::Above(e) = layer {
                    rest = rest.intersect(&s.coverage.get(&e).copied().unwrap_or(Rect::EMPTY));
                }
                let b = alloc.union(&rest).align_out(TILE).intersect(&bounds);
                creates.push((layer, b));
            }
            s.queue.push_back((layer, band));
        }
        s.queue_slot = Slot::Front;
        for (layer, b) in creates {
            if self.texture_bytes + bytes_of(&b) > self.limits.texture_bytes {
                // Retried at later bands (memory may be freed meanwhile), counted once.
                if self
                    .surfaces
                    .get_mut(&id)
                    .is_some_and(|s| s.refused_layers.insert(layer))
                {
                    self.counters.memory += 1;
                }
                continue;
            }
            self.texture_bytes += bytes_of(&b);
            if let Some(s) = self.surfaces.get_mut(&id) {
                s.textures.entry(layer).or_default().front = Some(b);
                // Earlier bands of this layer had no texture to go to (normally they were
                // empty; after a refusal for memory they may not be): upload them too.
                let earlier = Rect::new(b.x0, b.y0, b.x1, band.y0.min(b.y1));
                if !earlier.is_empty() {
                    s.queue.push_front((layer, earlier));
                }
            }
            out.ops.push(GpuOp::Create {
                tex: TexKey {
                    surface: id,
                    layer,
                    slot: Slot::Front,
                },
                tile_box: b,
            });
        }
    }

    /// A job finished painting.
    fn complete(&mut self, id: SurfaceId, out: &mut FrameOutput) {
        let Some(s) = self.surfaces.get_mut(&id) else {
            return;
        };
        let rects = s.job.take().map(|j| j.rects().to_vec()).unwrap_or_default();
        s.done_rects = rects;
        let kind = s.kind;
        let fits = kind != Kind::FirstPaint && self.fits_front(id);
        let Some(s) = self.surfaces.get_mut(&id) else {
            return;
        };
        s.stage = match (kind, fits) {
            // The bands are queued already.
            (Kind::FirstPaint, _) => Stage::Uploading,
            // An empty front queue marks the direct attempt that `upload` makes with the
            // frame's remaining cap.
            (Kind::Content, true) => {
                s.queue.clear();
                s.queue_slot = Slot::Front;
                Stage::Uploading
            }
            (Kind::Generation, true) => Stage::Holding,
            (_, false) => Stage::AwaitingBack,
        };
        if kind == Kind::FirstPaint {
            self.shrink_first_paint(id, out);
        }
    }

    /// The shrink of `render.md` §6.12, once when a first paint completes: a layer's
    /// texture whose allocated tiles' box is at most half of it on an axis is re-created at
    /// that box with one GPU copy; a layer with no tile left loses its texture. Bands still
    /// queued upload into the new texture.
    fn shrink_first_paint(&mut self, id: SurfaceId, out: &mut FrameOutput) {
        let Some(s) = self.surfaces.get_mut(&id) else {
            return;
        };
        let Some(set) = &s.canvases else {
            return;
        };
        let (mut added, mut freed) = (0u64, 0u64);
        let layers: Vec<Layer> = s.textures.keys().copied().collect();
        for layer in layers {
            let alloc = set.layer(layer).map_or(Rect::EMPTY, Canvas::allocated_box);
            let Some(t) = s.textures.get_mut(&layer) else {
                continue;
            };
            let (Some(front), None) = (t.front, t.back) else {
                continue;
            };
            let front_key = TexKey {
                surface: id,
                layer,
                slot: Slot::Front,
            };
            if alloc.is_empty() {
                t.front = None;
                freed += bytes_of(&front);
                out.ops.push(GpuOp::Destroy { tex: front_key });
                s.textures.remove(&layer);
                continue;
            }
            let small = alloc.width() * 2 <= front.width() || alloc.height() * 2 <= front.height();
            if !small || !front.contains_rect(&alloc) {
                continue;
            }
            let back_key = TexKey {
                slot: Slot::Back,
                ..front_key
            };
            out.ops.push(GpuOp::Create {
                tex: back_key,
                tile_box: alloc,
            });
            out.ops.push(GpuOp::Copy {
                from: front_key,
                to: back_key,
            });
            out.ops.push(GpuOp::Promote { surface: id, layer });
            t.front = Some(alloc);
            added += bytes_of(&alloc);
            freed += bytes_of(&front);
        }
        self.texture_bytes = (self.texture_bytes + added).saturating_sub(freed);
    }

    /// Whether the completed job's result fits the current front textures: same canvas
    /// size, every layer with tiles has a front texture covering them, and no texture is at
    /// least twice its tiles' box on an axis.
    fn fits_front(&self, id: SurfaceId) -> bool {
        let Some(s) = self.surfaces.get(&id) else {
            return false;
        };
        if s.resized {
            return false;
        }
        let Some(set) = &s.canvases else {
            return false;
        };
        for (layer, canvas) in set.layers() {
            let alloc = canvas.allocated_box();
            if alloc.is_empty() {
                continue;
            }
            let Some(front) = s.textures.get(&layer).and_then(|t| t.front) else {
                return false;
            };
            if !front.contains_rect(&alloc)
                || alloc.width() * 2 <= front.width()
                || alloc.height() * 2 <= front.height()
            {
                return false;
            }
        }
        true
    }

    /// Byte cost of uploading `rects` of every layer into the front textures (allocated
    /// tiles only; empty tiles are cleared).
    fn direct_cost(&self, id: SurfaceId, rects: &[Rect]) -> u64 {
        let Some(s) = self.surfaces.get(&id) else {
            return 0;
        };
        let Some(set) = &s.canvases else {
            return 0;
        };
        let mut total = 0;
        for (layer, canvas) in set.layers() {
            let Some(front) = s.textures.get(&layer).and_then(|t| t.front) else {
                continue;
            };
            for r in rects {
                let r = r.intersect(&front);
                for (tx, ty) in canvas.tiles_in(&r) {
                    if canvas.is_tile_allocated(tx, ty) {
                        total += bytes_of(&canvas.tile_rect(tx, ty).intersect(&r));
                    }
                }
            }
        }
        total
    }

    /// Queues `rects` of every layer into the given slot's textures, cut per tile.
    fn queue_rects(&mut self, id: SurfaceId, rects: &[Rect], slot: Slot) {
        let Some(s) = self.surfaces.get_mut(&id) else {
            return;
        };
        let Some(set) = &s.canvases else {
            return;
        };
        let mut q = Vec::new();
        for (layer, _) in set.layers() {
            let Some(t) = s.textures.get(&layer) else {
                continue;
            };
            let Some(b) = (match slot {
                Slot::Front => t.front,
                Slot::Back => t.back,
            }) else {
                continue;
            };
            for r in rects {
                let r = r.intersect(&b);
                if !r.is_empty() {
                    q.push((layer, r));
                }
            }
        }
        s.queue.extend(q);
        s.queue_slot = slot;
    }

    /// Creates back textures for the completed job and queues its rectangles. Returns
    /// [`Back::Wait`] (and counts a wait) when they do not fit the texture budget now, and
    /// [`Back::Never`] when they could not fit it even with every other surface's textures
    /// released (the surface's own front textures stay until the swap).
    fn go_back(&mut self, id: SurfaceId, rects: &[Rect], out: &mut FrameOutput) -> Back {
        let Some(s) = self.surfaces.get(&id) else {
            return Back::Never;
        };
        let Some(set) = &s.canvases else {
            return Back::Never;
        };
        let bounds = Rect::of_size(set.size()[0], set.size()[1]);
        let mut backs = Vec::new();
        for (layer, canvas) in set.layers() {
            let alloc = canvas.allocated_box();
            if alloc.is_empty() {
                continue;
            }
            backs.push((layer, alloc.align_out(TILE).intersect(&bounds)));
        }
        let need: u64 = backs.iter().map(|(_, b)| bytes_of(b)).sum();
        let own: u64 = s
            .textures
            .values()
            .flat_map(|t| [t.front, t.back])
            .flatten()
            .map(|b| bytes_of(&b))
            .sum();
        if own.saturating_add(need) > self.limits.texture_bytes {
            return Back::Never;
        }
        if self.texture_bytes + need > self.limits.texture_bytes {
            self.counters.back_texture_waits += 1;
            return Back::Wait;
        }
        self.texture_bytes += need;
        let resized = s.resized;
        let Some(s) = self.surfaces.get_mut(&id) else {
            return Back::Never;
        };
        for (layer, b) in backs {
            let t = s.textures.entry(layer).or_default();
            t.back = Some(b);
            let back = TexKey {
                surface: id,
                layer,
                slot: Slot::Back,
            };
            out.ops.push(GpuOp::Create {
                tex: back,
                tile_box: b,
            });
            // The copy of the front fills the overlap; the rest of the back box comes from
            // the canvas (normally empty tiles, cleared; content only where an earlier
            // texture could not be created).
            let fresh = match t.front {
                Some(f) if !resized => {
                    out.ops.push(GpuOp::Copy {
                        from: TexKey {
                            slot: Slot::Front,
                            ..back
                        },
                        to: back,
                    });
                    b.subtract(&f)
                }
                _ => vec![b],
            };
            for r in rects.iter().map(|r| r.intersect(&b)).chain(fresh) {
                if !r.is_empty() {
                    s.queue.push_back((layer, r));
                }
            }
        }
        s.queue_slot = Slot::Back;
        Back::Started
    }

    /// A completed content or generation job whose back textures can never fit U5 (§7):
    /// the surface's textures are dropped, so it draws nothing instead of waiting forever,
    /// and a generation member counts as ready. Counted as a memory refusal.
    fn refuse_back(&mut self, id: SurfaceId, out: &mut FrameOutput) {
        self.counters.memory += 1;
        if !self.warned_memory {
            self.warned_memory = true;
            tracing::warn!(
                surface = id.0,
                "UI texture memory budget too small for a surface: it draws nothing"
            );
        }
        let Some(s) = self.surfaces.get_mut(&id) else {
            return;
        };
        let mut freed = 0u64;
        for (layer, t) in std::mem::take(&mut s.textures) {
            for (slot, b) in [(Slot::Front, t.front), (Slot::Back, t.back)] {
                if let Some(b) = b {
                    freed += bytes_of(&b);
                    out.ops.push(GpuOp::Destroy {
                        tex: TexKey {
                            surface: id,
                            layer,
                            slot,
                        },
                    });
                }
            }
        }
        s.queue.clear();
        s.done_rects.clear();
        s.stage = if s.kind == Kind::Generation {
            Stage::Ready
        } else {
            Stage::Idle
        };
        self.texture_bytes = self.texture_bytes.saturating_sub(freed);
    }

    /// Drains a surface's upload queue under the cap. Returns whether it is empty.
    fn drain(&mut self, id: SurfaceId, left: &mut u64, out: &mut FrameOutput) -> bool {
        let Some(s) = self.surfaces.get_mut(&id) else {
            return true;
        };
        let Some(set) = &s.canvases else {
            s.queue.clear();
            return true;
        };
        let slot = s.queue_slot;
        while let Some((layer, r)) = s.queue.pop_front() {
            let Some(canvas) = set.layer(layer) else {
                continue;
            };
            let tex = TexKey {
                surface: id,
                layer,
                slot,
            };
            let tbox = s
                .textures
                .get(&layer)
                .and_then(|t| match slot {
                    Slot::Front => t.front,
                    Slot::Back => t.back,
                })
                .unwrap_or(Rect::EMPTY);
            let r = r.intersect(&tbox);
            let mut rest: Vec<(Layer, Rect)> = Vec::new();
            for (tx, ty) in canvas.tiles_in(&r) {
                let cell = canvas.tile_rect(tx, ty).intersect(&r);
                if cell.is_empty() {
                    continue;
                }
                if !rest.is_empty() {
                    rest.push((layer, cell));
                    continue;
                }
                if !canvas.is_tile_allocated(tx, ty) {
                    out.ops.push(GpuOp::Clear { tex, rect: cell });
                    out.stats.clears += 1;
                    continue;
                }
                let need = bytes_of(&cell);
                if need <= *left {
                    let (rect, bytes) = canvas.read_bgra(&cell);
                    *left -= need;
                    out.ops.push(GpuOp::Upload { tex, rect, bytes });
                    continue;
                }
                // Upload the rows that fit, keep the rest.
                let row = u64::from(cell.width()) * 4;
                let rows = i32::try_from(*left / row.max(1)).unwrap_or(0);
                if rows > 0 {
                    let part = Rect::new(cell.x0, cell.y0, cell.x1, cell.y0 + rows);
                    let (rect, bytes) = canvas.read_bgra(&part);
                    *left -= bytes_of(&part);
                    out.ops.push(GpuOp::Upload { tex, rect, bytes });
                    rest.push((layer, Rect::new(cell.x0, cell.y0 + rows, cell.x1, cell.y1)));
                } else {
                    rest.push((layer, cell));
                }
            }
            if !rest.is_empty() {
                for item in rest.into_iter().rev() {
                    s.queue.push_front(item);
                }
                return false;
            }
        }
        true
    }

    /// Uploads of first paints and generation jobs, before any content job: back textures
    /// that did not fit earlier get another try, then queues drain under the cap.
    fn upload_priority(&mut self, left: &mut u64, out: &mut FrameOutput) {
        for id in self.priority(|s| s.stage == Stage::AwaitingBack) {
            let rects = self
                .surfaces
                .get(&id)
                .map(|s| s.done_rects.clone())
                .unwrap_or_default();
            match self.go_back(id, &rects, out) {
                Back::Started => {
                    if let Some(s) = self.surfaces.get_mut(&id) {
                        s.stage = Stage::Uploading;
                    }
                }
                Back::Wait => {}
                Back::Never => self.refuse_back(id, out),
            }
        }
        for id in self.priority(|s| s.stage == Stage::Uploading && s.kind != Kind::Content) {
            if !self.drain(id, left, out) {
                continue;
            }
            let Some(s) = self.surfaces.get_mut(&id) else {
                continue;
            };
            match s.kind {
                // Complete and uploaded: the release check may draw it.
                Kind::FirstPaint if s.job.is_none() => s.stage = Stage::Idle,
                Kind::Generation => s.stage = Stage::Ready,
                _ => {}
            }
        }
        // First paints still painting: drain the bands finished so far.
        for id in self.priority(|s| s.stage == Stage::Painting && s.kind == Kind::FirstPaint) {
            self.drain(id, left, out);
        }
    }

    /// Uploads of content jobs, with what the first paints and generations left of the cap.
    fn upload_content(&mut self, left: &mut u64, out: &mut FrameOutput) {
        for id in self.priority(|s| s.stage == Stage::Uploading && s.kind == Kind::Content) {
            let Some(s) = self.surfaces.get(&id) else {
                continue;
            };
            if s.queue.is_empty() && s.queue_slot == Slot::Front {
                // Direct upload, all or nothing; otherwise through a back texture.
                let rects = s.done_rects.clone();
                let cost = self.direct_cost(id, &rects);
                if cost <= *left {
                    self.queue_rects(id, &rects, Slot::Front);
                    self.drain(id, left, out);
                    self.finish(id, out);
                    continue;
                }
                match self.go_back(id, &rects, out) {
                    Back::Started => {}
                    Back::Wait => {
                        if let Some(s) = self.surfaces.get_mut(&id) {
                            s.stage = Stage::AwaitingBack;
                        }
                        continue;
                    }
                    Back::Never => {
                        self.refuse_back(id, out);
                        continue;
                    }
                }
            }
            if self.drain(id, left, out) {
                self.finish(id, out);
            }
        }
    }

    /// Ends a content job (or a generation member at its swap): back textures are promoted,
    /// layers without tiles lose their textures.
    fn finish(&mut self, id: SurfaceId, out: &mut FrameOutput) {
        let Some(s) = self.surfaces.get_mut(&id) else {
            return;
        };
        let mut freed = 0u64;
        let layers: Vec<Layer> = s.textures.keys().copied().collect();
        for layer in layers {
            let has_tiles = s
                .canvases
                .as_ref()
                .and_then(|c| c.layer(layer))
                .is_some_and(|c| c.allocated_tiles() > 0);
            let Some(t) = s.textures.get_mut(&layer) else {
                continue;
            };
            if let Some(b) = t.back.take() {
                if let Some(f) = t.front {
                    freed += bytes_of(&f);
                }
                t.front = Some(b);
                out.ops.push(GpuOp::Promote { surface: id, layer });
            } else if !has_tiles && let Some(f) = t.front.take() {
                freed += bytes_of(&f);
                out.ops.push(GpuOp::Destroy {
                    tex: TexKey {
                        surface: id,
                        layer,
                        slot: Slot::Front,
                    },
                });
            }
            if t.front.is_none() && t.back.is_none() {
                s.textures.remove(&layer);
            }
        }
        s.stage = Stage::Idle;
        s.done_rects.clear();
        s.queue.clear();
        self.texture_bytes = self.texture_bytes.saturating_sub(freed);
    }

    fn released(&self, s: &SurfaceState, now: Duration) -> bool {
        let Some(shown) = s.first else {
            return false;
        };
        let complete = s.first_started && s.job.is_none() && s.queue.is_empty();
        (complete && !s.images_pending) || now >= shown.saturating_add(self.limits.first_paint_wait)
    }

    fn release_first_paints(&mut self, now: Duration, out: &mut FrameOutput) {
        let ids: Vec<SurfaceId> = self
            .surfaces
            .iter()
            .filter(|(id, s)| {
                s.first.is_some()
                    && s.generation.is_none()
                    && !s.drawn
                    && !self.in_pending_generation(**id, s.group)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let rel = self
                .surfaces
                .get(&id)
                .is_some_and(|s| self.released(s, now));
            if rel && let Some(s) = self.surfaces.get_mut(&id) {
                s.drawn = true;
                s.first = None;
                out.first_drawn.push(id);
            }
        }
    }

    fn member_ready(&self, id: SurfaceId, now: Duration) -> bool {
        let Some(s) = self.surfaces.get(&id) else {
            return true;
        };
        if s.gen_first {
            return s.first.is_none() || self.released(s, now);
        }
        s.gen_job_started && matches!(s.stage, Stage::Holding | Stage::Ready)
    }

    fn swap_generations(&mut self, now: Duration, left: &mut u64, out: &mut FrameOutput) {
        let gids: Vec<GroupId> = self
            .groups
            .iter()
            .filter(|(_, g)| g.running.is_some())
            .map(|(id, _)| *id)
            .collect();
        for gid in gids {
            let Some(members) = self
                .groups
                .get(&gid)
                .and_then(|g| g.running.as_ref())
                .map(|r| r.members.clone())
            else {
                continue;
            };
            if !members.iter().all(|m| self.member_ready(*m, now)) {
                continue;
            }
            let holders: Vec<SurfaceId> = members
                .iter()
                .copied()
                .filter(|m| {
                    self.surfaces
                        .get(m)
                        .is_some_and(|s| s.stage == Stage::Holding)
                })
                .collect();
            let held: u64 = holders
                .iter()
                .map(|h| {
                    let rects = self
                        .surfaces
                        .get(h)
                        .map(|s| s.done_rects.clone())
                        .unwrap_or_default();
                    self.direct_cost(*h, &rects)
                })
                .sum();
            if held > *left {
                // Too much for one frame: the holders move to back textures.
                let migrating = self
                    .groups
                    .get(&gid)
                    .and_then(|g| g.running.as_ref())
                    .is_some_and(|r| r.migrating);
                if !migrating {
                    for h in holders {
                        if let Some(s) = self.surfaces.get_mut(&h) {
                            s.stage = Stage::AwaitingBack;
                        }
                    }
                    if let Some(r) = self.groups.get_mut(&gid).and_then(|g| g.running.as_mut()) {
                        r.migrating = true;
                    }
                }
                continue;
            }
            for h in &holders {
                let rects = self
                    .surfaces
                    .get(h)
                    .map(|s| s.done_rects.clone())
                    .unwrap_or_default();
                self.queue_rects(*h, &rects, Slot::Front);
                self.drain(*h, left, out);
            }
            for m in &members {
                let first = self.surfaces.get(m).is_some_and(|s| s.gen_first);
                if !first {
                    self.finish(*m, out);
                }
                if let Some(s) = self.surfaces.get_mut(m) {
                    s.generation = None;
                    s.gen_first = false;
                    s.gen_job_started = false;
                    s.gen_capture = None;
                    if first {
                        s.first = None;
                        out.first_drawn.push(*m);
                    }
                    s.drawn = true;
                }
            }
            if let Some(g) = self.groups.get_mut(&gid)
                && let Some(r) = g.running.take()
            {
                g.shown = Some(r.geometry);
                out.swapped.push(gid);
                out.stats.generations_swapped += 1;
            }
        }
    }
}
