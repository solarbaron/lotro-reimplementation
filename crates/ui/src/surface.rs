// SPDX-License-Identifier: GPL-3.0-or-later
//! The tree side of UI rendering (decision 0015 §1–§2): which elements own surfaces, their
//! display lists, damage, layout changes of groups, and the composition list.
//!
//! Frame-loop step 8 calls [`Ui::paint`]. It decides the surfaces (Decision 1.1: the rule of
//! `lua-plugin-api.md` §13.19 for every element, plus alpha-test meters, the tooltip panel
//! and the drag icon; stand-in surfaces for shown mode-0 root panels with no surface above;
//! nothing below a Tile surface), keeps the [`crate::paint::Scheduler`] in step with them,
//! adds damage (Decision 1.4: an element's old and new canvas area whenever it changed or
//! moved; a whole canvas when its size changed; old and new coverage regions of nested
//! entries), records layout changes of groups (Decision 1.5), lets the scheduler paint and
//! upload within the budget, and builds the composition list (Decision 1.6). Display lists
//! are built only when the scheduler starts a job on a surface.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Duration;

use crate::env::{ImageRef, ImageState};
use crate::geom::IBox;
use crate::paint::{
    Addressing, CompNode, ComposeConfig, CompositeOp, CompositionList, DisplayList, EntryTexture,
    FootprintNode, GpuOp, GroupId, ImageLayer, NodeContent, OverlayLayer, PaintBudget,
    PaintFrameStats, PaintLimits, PaintOp, ParentCanvas, Rect, Scheduler, Snapshot, SnapshotSource,
    SurfaceId, build, coverage_region, opacity_byte,
};
use crate::text::DrawParams;
use crate::tree::ElementKey;
use crate::ui::{Ui, object_mode};

/// The mapping from a surface owner's own (owner-relative, UI-unit) layout coordinates to its
/// canvas pixels (`ui-scaling.md` §6.1 canvas sizes, §6.2 element boxes). The surface's canvas
/// is allocated at this physical resolution and composed 1 : 1 onto its physical box (§6.6), so
/// drawing is crisp instead of GPU-upscaled.
///
/// At the **identity** — no UI screen, or `Se` = 1 with no node scale, which is every surface
/// built before scaling is wired in and the C2/C4/C7/C9/C10/C12 configurations — the map is
/// `x ↦ x`, the content scale is `(1, 1)`, and every produced box and canvas size equals
/// today's, so all existing goldens and paint tests stay byte-identical (invariant I2).
#[derive(Debug, Clone, Copy, PartialEq)]
struct CanvasMap {
    screen: Option<crate::scaling::UiScreen>,
    /// The owner's displayed top-left (UI units) and node scale `m`.
    dl: f64,
    dt: f64,
    mx: f64,
    my: f64,
    /// The canvas's physical origin, subtracted so the canvas's top-left maps to (0, 0): the
    /// owner's `(Ex(dl), Ey(dt))`.
    ox: i32,
    oy: i32,
}

impl CanvasMap {
    /// The byte-exact identity (`x ↦ x`).
    fn identity() -> Self {
        Self {
            screen: None,
            dl: 0.0,
            dt: 0.0,
            mx: 1.0,
            my: 1.0,
            ox: 0,
            oy: 0,
        }
    }

    /// Whether a non-identity mapping is in force (a UI screen is set). At the identity every
    /// method below is `x ↦ x` / `(1, 1)` and callers take the current byte-exact paths.
    fn active(&self) -> bool {
        self.screen.is_some()
    }

    /// Maps an owner-relative layout x coordinate to a canvas pixel column (`ui-scaling.md`
    /// §3.6, §6.2): `Ex(dl + x·mx) − ox`.
    fn cx(&self, x: i32) -> i32 {
        match self.screen {
            Some(f) => f.edge_x(self.dl + f64::from(x) * self.mx) - self.ox,
            None => x,
        }
    }

    /// Maps an owner-relative layout y coordinate to a canvas pixel row.
    fn cy(&self, y: i32) -> i32 {
        match self.screen {
            Some(f) => f.edge_y(self.dt + f64::from(y) * self.my) - self.oy,
            None => y,
        }
    }

    /// Maps an owner-relative **layout** inclusive box to its canvas [`Rect`] (half-open, as
    /// [`IBox::to_rect`]).
    fn rect(&self, b: IBox) -> Rect {
        if b.is_empty() {
            return Rect::EMPTY;
        }
        if !self.active() {
            return b.to_rect();
        }
        let r = i64::from(b.r).saturating_add(1);
        let bot = i64::from(b.b).saturating_add(1);
        Rect::new(
            self.cx(b.l),
            self.cy(b.t),
            self.cx(clamp_i32(r)),
            self.cy(clamp_i32(bot)),
        )
    }

    /// Maps an owner-relative layout point (an image/overlay anchor) to a canvas pixel.
    fn point(&self, x: i32, y: i32) -> [i32; 2] {
        [self.cx(x), self.cy(y)]
    }

    /// The content scale `c` = (`mx·Kx`, `my·Ky`) (`ui-scaling.md` §3.4, §6.1): canvas pixels
    /// per owner UI unit, used to resample image sources in the painter and to choose the
    /// target heights of text (§6.4.1).
    fn content_scale(&self) -> [f64; 2] {
        match self.screen {
            Some(f) => [self.mx * f.kx(), self.my * f.ky()],
            None => [1.0, 1.0],
        }
    }
}

/// The colour of the letterbox bars (`ui-scaling.md` §5.4 step 6): opaque black, 0xAARRGGBB.
pub(crate) const LETTERBOX_COLOUR: u32 = 0xFF00_0000;

/// Clamps an `i64` coordinate into `i32` range (bounded canvas arithmetic, §7 hostile input).
fn clamp_i32(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// The geometry of a group as last swapped: each surface's displayed box relative to the
/// root's displayed origin, and its canvas size.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GroupGeometry {
    /// The root's displayed box.
    pub root: Rect,
    /// Per surface: displayed box relative to the root origin, canvas size.
    pub entries: BTreeMap<u32, (Rect, [u32; 2])>,
}

#[derive(Debug, Clone, PartialEq)]
struct SurfaceRec {
    id: SurfaceId,
    group: GroupId,
    canvas: [u32; 2],
    displayed: Rect,
    coverage: Vec<(u32, Rect)>,
    stand_in: bool,
    tile: bool,
    /// What the full pass read from the owner itself ([`Ui::owner_sig`]).
    sig: OwnerSig,
    /// Whether the full pass found surfaces nested in this one (Decision 1.7).
    has_nested: bool,
}

/// What the full surface pass reads from a surface's owner itself, as opposed to its
/// ancestors (a change to one records that ancestor, and `light_sync` then checks this
/// signature for every owner below it, running the full pass when it differs): when a
/// recorded owner's signature, canvas size and displayed box are those of the last full
/// pass, the pass would decide the same surface, group, coverage and composition node for it
/// (apart from the fields [`Ui::light_sync`] patches in place).
#[derive(Debug, Clone, PartialEq)]
struct OwnerSig {
    parent: Option<ElementKey>,
    /// The position among the parent's children (the pre-order of the composition plan).
    index: Option<usize>,
    visible: bool,
    /// Shown with every ancestor up to the screen root (the plan's `shown`, attachment).
    displayed: bool,
    root_panel: bool,
    owns: bool,
    mode: u32,
    bx: IBox,
    physical: Rect,
    map: CanvasMap,
    node_scale: (f32, f32, f32),
}

/// What one frame of painting produced.
#[derive(Debug, Default)]
pub struct UiFrame {
    /// GPU operations for `tessera-render`'s UI textures, in order.
    pub ops: Vec<GpuOp>,
    /// The composition list for the UI pass.
    pub composition: CompositionList,
    /// Statistics.
    pub stats: PaintFrameStats,
    /// Surfaces that existed this frame.
    pub surfaces: usize,
}

/// The painter state kept by the UI.
#[derive(Debug)]
pub(crate) struct Painter {
    pub scheduler: Scheduler<GroupGeometry>,
    pub limits: PaintLimits,
    pub config: ComposeConfig,
    pub touched: bool,
    next_id: u32,
    recs: BTreeMap<ElementKey, SurfaceRec>,
    painted: PaintedMap,
    pub external: BTreeMap<ElementKey, u64>,
    /// Composition input of the last tree change.
    nodes: Option<Vec<CompNode>>,
    /// Transition state of the groups when `nodes` was built.
    running: Vec<(u32, bool)>,
    /// Elements whose image was still loading when their surface's display list was last
    /// built: an image arriving damages them (Decision 1.4, "an image arriving").
    image_waiters: BTreeSet<ElementKey>,
    /// Glyph runs of each text element, reused while its prepared text and draw
    /// parameters are unchanged (`text::draw` is a pure function of them).
    text_runs: HashMap<ElementKey, TextRuns>,
    /// Reused stack of the damage walk.
    walk_stack: Vec<(ElementKey, IBox, IBox, usize)>,
    /// Elements marked paint-dirty since the last damage pass.
    dirty: BTreeSet<ElementKey>,
    /// Something the surface decisions, coverage regions, groups or composition nodes
    /// read changed outside the elements in `dirty` (a rotation held for the first time, a
    /// Stretch scale, external content, the limits, a destroyed surface): the next touched
    /// frame runs the full surface pass.
    structural: bool,
    /// Surfaces whose held rotation turned (already rotated before): their composition
    /// nodes take the new angles without a full pass.
    rotated: Vec<ElementKey>,
    /// The elements that have composition nodes in `nodes`, with their node index.
    node_index: HashMap<ElementKey, usize>,
    /// The elements that get composition nodes, from the last full surface pass.
    plan: Vec<PlanEntry>,
    /// The reference paint step ([`Ui::set_reference_paint`]).
    always_full: bool,
    /// The crisp text path's state (`ui-scaling.md` §6.4, §6.5, §7.1, §7.5).
    pub text: crate::text_ui::TextScaling,
    /// `image_lru_mib` of `ui-scaling.md` §8 S6 once the host set it ([`Ui::set_image_memory`];
    /// until then the glyph share is the host's, [`Ui::set_glyph_cache_budget`]).
    pub image_lru_mib: Option<u64>,
    /// The S6 split last worked out from `image_lru_mib` and *Se* ([`Ui::s6_budget`]); the
    /// default while `image_lru_mib` is unset.
    pub s6: crate::scaling::S6Budget,
    /// The letterbox bars of the last frame that changed the tree, with the UI screen they
    /// were worked out for (`ui-scaling.md` §5.4 step 6): reused while nothing changes.
    letterbox: Option<(
        Option<crate::scaling::UiScreen>,
        Vec<crate::scaling::LetterboxBars>,
    )>,
    /// How many times each surface pass ran ([`Ui::surface_passes`]).
    passes: SurfacePasses,
}

/// How many times [`Ui::paint`] ran each of its surface passes since the UI was made
/// (decision 0015 Decision 1.4–1.6). A frame in which nothing changed runs neither; a frame
/// in which only elements changed whose surfaces stay as they were runs the light pass,
/// which walks only the changed subtrees; the full pass walks the whole tree.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SurfacePasses {
    /// Full surface passes (surface decisions, coverage, groups, every surface's damage).
    pub full: u64,
    /// Light passes (only the damage of the recorded elements).
    pub light: u64,
}

/// Where the damage walk reaches an element: its box in canvas coordinates, its parent's
/// clip and its depth below the surface owner; `None` when the walk does not draw it.
type WalkState = Option<(IBox, IBox, usize)>;

/// A [`Ui::light_sync`] start: a recorded element and, when it is drawn, the surface that draws
/// it, that surface's owner→canvas map ([`CanvasMap`], `ui-scaling.md` §6.2) and the walk state.
type LightStart = (ElementKey, Option<(SurfaceId, CanvasMap, WalkState)>);

/// A surface owner [`Ui::light_sync`] can take ([`Ui::light_owner`]).
struct LightOwner {
    id: SurfaceId,
    /// The owner's new signature, displayed box and canvas size, when they changed.
    update: Option<OwnerUpdate>,
}

/// What changed about a surface owner taken by [`Ui::light_sync`].
struct OwnerUpdate {
    sig: OwnerSig,
    displayed: Rect,
    canvas: [u32; 2],
    group: GroupId,
    /// For a nested owner: its parent surface's owner and its new coverage region there.
    cover: Option<(ElementKey, Option<Rect>)>,
}

/// What [`Ui::decide_surfaces`] finds.
#[derive(Debug, Default)]
struct Decided {
    /// (owner, stand-in, tile, position among its parent's children) in tree pre-order.
    surfaces: Vec<(ElementKey, bool, bool, Option<usize>)>,
    /// For each surface, its direct nested surfaces in tree pre-order.
    nested: Vec<Vec<ElementKey>>,
    /// The elements that get composition nodes.
    plan: Vec<PlanEntry>,
}

/// An element that gets composition nodes (`Ui::comp_nodes`), in tree pre-order.
#[derive(Debug, Clone, Copy)]
struct PlanEntry {
    key: ElementKey,
    /// Shown with every ancestor up to the screen root.
    shown: bool,
    /// The nearest earlier entry with a node of its own (a surface or a window).
    parent: Option<usize>,
}

/// The drawn text of one element (see [`Painter::text_runs`]).
#[derive(Debug)]
struct TextRuns {
    /// The prepared text the runs were drawn from (kept so that its address stays unique).
    prepared: Arc<crate::env::PreparedText>,
    origin: [i32; 2],
    clip: Rect,
    outline: bool,
    outline_colour: u32,
    /// The operations `TextPaint::push_to` appends, or `None` when it refuses the text.
    ops: Option<Vec<PaintOp>>,
}

/// The canvas area each element was last painted into, by arena index (decision 0015
/// Decision 1.4: the old and new areas of an element that moved or changed).
#[derive(Debug, Default)]
struct PaintedMap {
    slots: Vec<Option<PaintedSlot>>,
    stamp: u32,
    /// Entries held.
    len: usize,
}

#[derive(Debug, Clone, Copy)]
struct PaintedSlot {
    key: ElementKey,
    surface: SurfaceId,
    area: Rect,
    /// The damage pass that last saw the element.
    stamp: u32,
}

impl PaintedMap {
    fn get(&self, k: ElementKey) -> Option<(SurfaceId, Rect)> {
        match self.slots.get(k.index()) {
            Some(Some(s)) if s.key == k => Some((s.surface, s.area)),
            _ => None,
        }
    }

    fn remove(&mut self, k: ElementKey) -> Option<(SurfaceId, Rect)> {
        let slot = self.slots.get_mut(k.index())?;
        match slot {
            Some(s) if s.key == k => {
                self.len = self.len.saturating_sub(1);
                slot.take().map(|s| (s.surface, s.area))
            }
            _ => None,
        }
    }

    /// Drops every entry painted into `surface` (no damage: the surface is gone).
    fn drop_surface(&mut self, surface: SurfaceId) {
        for slot in &mut self.slots {
            if slot.is_some_and(|s| s.surface == surface) {
                *slot = None;
                self.len = self.len.saturating_sub(1);
            }
        }
    }

    fn set(&mut self, k: ElementKey, surface: SurfaceId, area: Rect) {
        let i = k.index();
        if self.slots.len() <= i {
            self.slots.resize(i + 1, None);
        }
        if let Some(slot) = self.slots.get_mut(i) {
            if slot.is_none() {
                self.len += 1;
            }
            *slot = Some(PaintedSlot {
                key: k,
                surface,
                area,
                stamp: self.stamp,
            });
        }
    }
}

impl Painter {
    pub fn new(limits: PaintLimits) -> Self {
        Self {
            scheduler: Scheduler::new(limits),
            config: ComposeConfig::from_limits(&limits),
            limits,
            touched: true,
            next_id: 1,
            recs: BTreeMap::new(),
            painted: PaintedMap::default(),
            external: BTreeMap::new(),
            nodes: None,
            running: Vec::new(),
            image_waiters: BTreeSet::new(),
            text_runs: HashMap::new(),
            walk_stack: Vec::new(),
            dirty: BTreeSet::new(),
            structural: true,
            rotated: Vec::new(),
            node_index: HashMap::new(),
            plan: Vec::new(),
            always_full: false,
            text: crate::text_ui::TextScaling::default(),
            image_lru_mib: None,
            s6: crate::scaling::S6Budget::default(),
            letterbox: None,
            passes: SurfacePasses::default(),
        }
    }

    /// Records a paint-dirty element for the next damage pass.
    pub fn note_dirty(&mut self, k: ElementKey) {
        self.dirty.insert(k);
    }

    /// Makes the next frame with a change run the full surface pass (without asking for
    /// a paint pass itself).
    pub fn note_structure(&mut self) {
        self.structural = true;
    }

    /// Asks for a paint pass that runs the full surface pass.
    pub fn structure_changed(&mut self) {
        self.structural = true;
        self.touched = true;
    }

    /// The rotation held by `k`'s surface changed; `first` when it starts or stops holding
    /// one (its coverage region changes, Decision 1.7).
    pub fn rotation_changed(&mut self, k: ElementKey, first: bool) {
        if first || !self.recs.contains_key(&k) {
            self.structural = true;
        } else {
            self.rotated.push(k);
        }
        self.touched = true;
    }

    pub fn forget(&mut self, k: ElementKey) {
        if let Some(r) = self.recs.remove(&k) {
            self.scheduler.release_surface(r.id);
            self.structural = true;
        }
        if self.node_index.contains_key(&k) || self.external.contains_key(&k) {
            self.structural = true;
        }
        self.dirty.remove(&k);
        if let Some((s, r)) = self.painted.remove(k) {
            self.scheduler.add_damage(s, r);
        }
        self.external.remove(&k);
        self.image_waiters.remove(&k);
        self.text_runs.remove(&k);
        self.text.runs.remove(&k);
        self.text.reflow.remove(&k);
    }
}

fn pow2(v: u32) -> bool {
    v != 0 && v & (v - 1) == 0
}

impl Ui {
    /// Sets the paint limits (decision 0015 §6–§7), for example the device's largest texture
    /// dimension. Existing surfaces are kept.
    pub fn set_paint_limits(&mut self, limits: PaintLimits) {
        self.painter.config = ComposeConfig::from_limits(&limits);
        self.painter.scheduler.set_limits(limits);
        self.painter.limits = limits;
        self.painter.note_structure();
    }

    /// Sets `image_lru_mib`, the S6 memory setting (`ui-scaling.md` §8 S6; default 64): the
    /// total *T* = `image_lru_mib` × max(1, *Se*²) is split again ([`crate::scaling::s6_budget`])
    /// and the glyph share applied to the derived-glyph cache, which evicts down to it. The split is
    /// worked out again on every change of *Se* ([`Self::set_ui_screen`]); the host applies
    /// [`Self::s6_budget`]'s image share to its decoded-image cache. Before the first call
    /// the glyph share is whatever [`Self::set_glyph_cache_budget`] set (8 MiB by default).
    pub fn set_image_memory(&mut self, image_lru_mib: u64) {
        self.painter.image_lru_mib = Some(image_lru_mib);
        self.apply_s6();
    }

    /// The S6 split in force (`ui-scaling.md` §8 S6): the total, the glyph caches' share and
    /// the decoded and scaled images' share.
    #[must_use]
    pub fn s6_budget(&self) -> crate::scaling::S6Budget {
        self.painter.s6
    }

    /// Works the S6 split out for the current *Se* (1 without a UI screen) and applies the
    /// glyph share, once the host has set `image_lru_mib`.
    pub(crate) fn apply_s6(&mut self) {
        let Some(mib) = self.painter.image_lru_mib else {
            return;
        };
        let se = self.ui_screen.map_or(1.0, |f| f.effective_scale());
        let b = crate::scaling::s6_budget(mib, se);
        self.painter.s6 = b;
        self.painter
            .text
            .set_glyph_share(usize::try_from(b.glyphs).unwrap_or(usize::MAX));
    }

    /// Sets (or removes) the worker pool on which paint jobs with a damage above
    /// [`PaintLimits::worker_min_px`] physical pixels run (`ui-scaling.md` §8 S1: the paint
    /// leaves the frame thread so the UI's main-thread time stays within U1). Without one
    /// every job paints on the frame thread in bands, as before. The pool lives on across
    /// [`Self::set_paint_limits`].
    pub fn set_paint_spawner(&mut self, spawner: Option<Arc<dyn crate::paint::PaintSpawner>>) {
        self.painter.scheduler.set_spawner(spawner);
    }

    /// Whether the surface of `k` has its paint job on a worker now (S1).
    #[must_use]
    pub fn paint_on_worker(&self, k: ElementKey) -> bool {
        self.painter
            .recs
            .get(&k)
            .is_some_and(|r| self.painter.scheduler.is_on_worker(r.id))
    }

    /// Whether painting has settled: no tree change waits for the next paint step and the
    /// paint scheduler is idle (every surface drawn, no job, upload or generation left).
    /// Tests wait on this instead of a fixed number of frames, since the paint step works
    /// in time slices and a loaded machine needs more frames for the same work.
    #[must_use]
    pub fn paint_settled(&self) -> bool {
        !self.painter.touched && self.painter.dirty.is_empty() && self.painter.scheduler.is_idle()
    }

    /// How many times each surface pass ran ([`SurfacePasses`]): benchmarks and tests check
    /// that a frame with no change runs neither and that a steady change runs the light one.
    #[must_use]
    pub fn surface_passes(&self) -> SurfacePasses {
        self.painter.passes
    }

    /// The reference paint step: the full surface pass on every frame with a change
    /// (instead of only the damage of the changed elements when nothing structural
    /// changed), and text drawn again for every display list (no glyph-run cache). Both
    /// give the same frames; tests and benchmarks compare them.
    #[doc(hidden)]
    pub fn set_reference_paint(&mut self, on: bool) {
        self.painter.always_full = on;
        self.painter.text_runs.clear();
        // The scaled runs stay for hit tests (§6.4.6); `always_full` keeps any from being
        // reused for drawing.
    }

    /// Sets the art filter `ui_art_filter` (`ui-scaling.md` §6.3, §7.6): the column of the §6.3
    /// filter table images are resampled with when they are painted (composition keeps
    /// `ui_scale_filter`, §6.6). At a scale of exactly 1 every filter is the exact copy, so
    /// surfaces at the identity do not change. Applied live: every surface repaints.
    pub fn set_art_filter(&mut self, filter: crate::paint::ArtFilter) {
        if self.painter.scheduler.set_art_filter(filter) {
            self.invalidate_paint();
        }
    }

    /// The art filter of [`Ui::set_art_filter`].
    #[must_use]
    pub fn art_filter(&self) -> crate::paint::ArtFilter {
        self.painter.scheduler.art_filter()
    }

    /// Registers GPU-drawn external content for a viewport element (decision 0015
    /// Decision 1.7; `ui-widgets.md` §12 Tessera rule), or removes it.
    pub fn set_external_content(&mut self, k: ElementKey, id: Option<u64>) {
        match id {
            Some(i) => {
                self.painter.external.insert(k, i);
            }
            None => {
                self.painter.external.remove(&k);
            }
        }
        self.painter.structure_changed();
        self.mark_dirty(k);
    }

    /// A pixel of the canvas below of an element's surface (CPU side, after the last
    /// [`Ui::paint`]), `0xAARRGGBB`; for tests and tools.
    #[must_use]
    pub fn surface_pixel(&self, k: ElementKey, x: i32, y: i32) -> Option<u32> {
        let id = self.painter.recs.get(&k)?.id;
        Some(self.painter.scheduler.canvases(id)?.below().pixel(x, y))
    }

    /// A pixel of one canvas of surface `id` (CPU side, after the last [`Ui::paint`]): the
    /// canvas below or a canvas above a nested entry (the textures a composition-list
    /// entry names), `0xAARRGGBB`; `None` when the surface or the layer has no canvas. For
    /// tests and tools that compose the composition list on the CPU.
    #[must_use]
    pub fn surface_layer_pixel(
        &self,
        id: SurfaceId,
        layer: crate::paint::Layer,
        x: i32,
        y: i32,
    ) -> Option<u32> {
        Some(
            self.painter
                .scheduler
                .canvases(id)?
                .layer(layer)?
                .pixel(x, y),
        )
    }

    /// Marks the elements that wait for an image for repaint: the client calls it when
    /// decoded images arrived (decision 0015 Decision 1.8: a pending image draws nothing
    /// until it arrives).
    ///
    /// Only the elements whose image was still loading when their surface's display list
    /// was last built are damaged: an element drawn with its image ready draws the same
    /// pixels again, so repainting it would change nothing (Decision 1.4 names "an image
    /// arriving" as damage of the elements that wait for it).
    pub fn images_arrived(&mut self) {
        let waiters = std::mem::take(&mut self.painter.image_waiters);
        for k in waiters {
            self.note_paint_dirty(k);
        }
        // Asks for a paint pass even with no waiter, as before: a pass that is due for
        // other reasons (a Stretch scale, a first rotation) runs then.
        self.painter.touched = true;
    }

    /// Marks the whole UI for repaint.
    pub fn invalidate_paint(&mut self) {
        for k in self.arena.keys() {
            self.note_paint_dirty(k);
        }
        self.painter.structure_changed();
    }

    /// The surface of an element, if it has one this frame.
    #[must_use]
    pub fn surface_of(&self, k: ElementKey) -> Option<SurfaceId> {
        self.painter.recs.get(&k).map(|r| r.id)
    }

    /// Logs the nested surfaces of a newly opened panel (decision 0015 Decision 1.7 (b)).
    pub(crate) fn panel_opened(&mut self, k: ElementKey) {
        let mut nested = 0u32;
        let mut stack = self.children(k);
        while let Some(c) = stack.pop() {
            if self.wants_surface(c) {
                nested += 1;
            }
            if stack.len() < 1 << 16 {
                stack.extend(self.children(c));
            }
        }
        tracing::info!(
            panel = self.element_id(k).unwrap_or(0),
            nested_surfaces = nested,
            "panel opened"
        );
    }

    /// Decides the surfaces of this frame in one walk of the tree in pre-order (Decision
    /// 1.1): each surface as (owner, stand-in, tile), the direct nested surfaces of each
    /// (Decision 1.7), and the elements that get composition nodes (Decision 1.6).
    fn decide_surfaces(&self) -> Decided {
        let mut out = Decided::default();
        // (element, nearest surface above is Tile, surfaced depth, a surface above, shown
        // with every ancestor, nearest plan entry with a node of its own, index of the
        // nearest surface above in `out.surfaces`, position among the parent's children)
        type Item = (
            ElementKey,
            bool,
            u32,
            bool,
            bool,
            Option<usize>,
            Option<usize>,
            Option<usize>,
        );
        let screen_shown = self.is_visible(self.screen);
        let screen_index = self.child_index(self.screen, None);
        let mut stack: Vec<Item> = vec![(
            self.screen,
            false,
            0,
            false,
            screen_shown,
            None,
            None,
            screen_index,
        )];
        while let Some((k, under_tile, depth, surface_above, shown, plan_parent, surface, index)) =
            stack.pop()
        {
            let Some(e) = self.el(k) else { continue };
            let mut has = false;
            let mut tile_here = under_tile;
            let mut ndepth = depth;
            let mut above = surface_above;
            if k != self.screen && !under_tile && depth <= self.painter.limits.max_depth {
                if self.owns_surface(k) {
                    let tile = self.surface_mode(k) == object_mode::TILE
                        && pow2(e.bx.w().max(0) as u32)
                        && pow2(e.bx.h().max(0) as u32);
                    out.surfaces.push((k, false, tile, index));
                    has = true;
                    tile_here = tile;
                } else if e.root_panel
                    && e.visible
                    && !surface_above
                    && self.object_mode(k) == Some(object_mode::NONE)
                {
                    out.surfaces.push((k, true, false, index));
                    has = true;
                }
            }
            let mut child_surface = surface;
            if has {
                ndepth += 1;
                above = true;
                let here = out.surfaces.len() - 1;
                out.nested.push(Vec::new());
                if let Some(nested) = surface.and_then(|i| out.nested.get_mut(i)) {
                    nested.push(k);
                }
                child_surface = Some(here);
            }
            // Composition nodes: a surface, a window drawn without one below a node, and
            // external content (`comp_nodes`).
            let window_without = !has && e.root_panel && plan_parent.is_some() && k != self.screen;
            let external = self.painter.external.contains_key(&k);
            let mut child_plan = plan_parent;
            if has || window_without || external {
                out.plan.push(PlanEntry {
                    key: k,
                    shown,
                    parent: plan_parent,
                });
                if has || window_without {
                    child_plan = Some(out.plan.len() - 1);
                }
            }
            for (i, c) in e.children.iter().enumerate().rev() {
                let child_shown = shown && self.is_visible(*c);
                stack.push((
                    *c,
                    tile_here,
                    ndepth,
                    above,
                    child_shown,
                    child_plan,
                    child_surface,
                    Some(i),
                ));
            }
        }
        out
    }

    /// The canvas size of a surface (decision 0015 §2.1, `ui-scaling.md` §6.1): the owner's
    /// **physical box** in window pixels, so the canvas is painted at the content scale and
    /// composed 1 : 1 (§6.6). At the identity (no UI screen, or `Se` = 1 with no node scale)
    /// this is the owner's layout size unchanged (invariant I2). Clamped to the device's largest
    /// texture dimension (and so to [`crate::scaling::MAX_WINDOW_PX`], §6.1).
    fn canvas_size(&self, k: ElementKey) -> [u32; 2] {
        let b = self.layout_box(k).unwrap_or_default();
        let m = self.canvas_map(k);
        let r = m.rect(IBox::from_xywh(0, 0, b.w(), b.h()));
        let max = self.painter.limits.max_surface_dim.max(1);
        [r.width().min(max), r.height().min(max)]
    }

    /// The owner→canvas mapping of a surface (`ui-scaling.md` §6.1, §6.2). It is the identity
    /// when no UI screen is set ([`Ui::set_ui_screen`]) and, in effect, when `Se` = 1 with no
    /// node scale on or above the owner (then `Kx` = `Ky` = 1 and `m` = 1, so every coordinate
    /// maps to itself).
    fn canvas_map(&self, k: ElementKey) -> CanvasMap {
        let Some(screen) = self.ui_screen else {
            return CanvasMap::identity();
        };
        // §3.4, §6.2: the owner's real displayed top-left (not rounded to whole UI units), so
        // the canvas origin is the column and row of its physical box (§3.6).
        let (d, _) = self.real_edges(k);
        let (mx, my, _) = self.node_scale_at_or_above(k);
        CanvasMap {
            screen: Some(screen),
            dl: d.l,
            dt: d.t,
            mx: f64::from(mx),
            my: f64::from(my),
            ox: screen.edge_x(d.l),
            oy: screen.edge_y(d.t),
        }
    }

    fn to_paint_rect(b: &IBox) -> Rect {
        b.to_rect()
    }

    /// Builds the display list of a surface (decision 0015 Decision 1.3, `ui-layout.md`
    /// §9.3, §9.6). Returns the list and whether images are still pending.
    fn build_display_list(&mut self, owner: ElementKey) -> (DisplayList, bool) {
        let recs = std::mem::take(&mut self.painter.recs);
        let mut list = DisplayList::new();
        let mut pending = false;
        let m = self.canvas_map(owner);
        if let Some(b) = self.layout_box(owner) {
            let root_box = IBox::from_xywh(0, 0, b.w(), b.h());
            self.emit_rec(
                owner,
                owner,
                root_box,
                root_box,
                &recs,
                &mut list,
                &mut pending,
                0,
                &m,
            );
        }
        self.painter.recs = recs;
        (list, pending)
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_rec(
        &mut self,
        owner: ElementKey,
        k: ElementKey,
        bx: IBox,
        clip: IBox,
        recs: &BTreeMap<ElementKey, SurfaceRec>,
        list: &mut DisplayList,
        pending: &mut bool,
        depth: usize,
        m: &CanvasMap,
    ) {
        if depth > crate::ui::MAX_TREE_DEPTH + 2 {
            return;
        }
        let Some(e) = self.el(k) else { return };
        if !e.visible {
            return;
        }
        if k != owner
            && let Some(r) = recs.get(&k)
        {
            if let Some(cov) = recs
                .get(&owner)
                .and_then(|o| o.coverage.iter().find(|(id, _)| *id == r.id.0))
                .map(|(_, c)| *c)
            {
                // `cov` is already in canvas pixels (`coverage_region`), so it is not mapped.
                let _ = list.push(PaintOp::Nested {
                    entry: r.id.0,
                    coverage: cov,
                });
            }
            return;
        }
        let clip = clip.intersect(&bx);
        if clip.is_empty() {
            return;
        }
        let after = e.draw_after_children;
        let children = e.children.clone();
        if after {
            self.emit_children(owner, &children, bx, clip, recs, list, pending, depth, m);
            self.emit_own(k, bx, clip, list, pending, m);
        } else {
            self.emit_own(k, bx, clip, list, pending, m);
            self.emit_external(k, bx, recs, owner, list, m);
            self.emit_children(owner, &children, bx, clip, recs, list, pending, depth, m);
        }
        if after {
            self.emit_external(k, bx, recs, owner, list, m);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_children(
        &mut self,
        owner: ElementKey,
        children: &[ElementKey],
        bx: IBox,
        clip: IBox,
        recs: &BTreeMap<ElementKey, SurfaceRec>,
        list: &mut DisplayList,
        pending: &mut bool,
        depth: usize,
        m: &CanvasMap,
    ) {
        for c in children {
            if let Some(cb) = self.layout_box(*c) {
                let child_box = cb.offset(bx.l, bx.t);
                self.emit_rec(
                    owner,
                    *c,
                    child_box,
                    clip,
                    recs,
                    list,
                    pending,
                    depth + 1,
                    m,
                );
            }
        }
    }

    fn emit_external(
        &mut self,
        k: ElementKey,
        bx: IBox,
        recs: &BTreeMap<ElementKey, SurfaceRec>,
        owner: ElementKey,
        list: &mut DisplayList,
        m: &CanvasMap,
    ) {
        if let Some(id) = self.painter.external.get(&k).copied() {
            let entry = (id & 0xFFFF_FFFF) as u32 | 0x8000_0000;
            let _ = recs;
            let _ = owner;
            let _ = list.push(PaintOp::Nested {
                entry,
                coverage: m.rect(bx),
            });
        }
    }

    fn image_of(
        &mut self,
        r: &ImageRef,
        pending: &mut bool,
    ) -> Option<Arc<tessera_material::ui_image::UiImage>> {
        if let ImageRef::File(path) = r
            && path.starts_with(crate::game::combat::look::PROCEDURAL_PREFIX)
        {
            return self.procedural_image(path);
        }
        match self.env.image(r) {
            ImageState::Ready(i) => Some(i),
            ImageState::Pending => {
                *pending = true;
                None
            }
            ImageState::Failed => None,
        }
    }

    /// An element's own content: base image, overlay, text (`ui-layout.md` §9.6,
    /// `lua-plugin-api.md` §13.17 order).
    fn emit_own(
        &mut self,
        k: ElementKey,
        bx: IBox,
        clip: IBox,
        list: &mut DisplayList,
        pending: &mut bool,
        m: &CanvasMap,
    ) {
        let Some(e) = self.el(k) else { return };
        let base = e.base.clone();
        let overlay = e.overlay.clone();
        let (ox, oy) = e.image_offset;
        // Geometry is mapped to canvas pixels (`ui-scaling.md` §6.2); image sources stay
        // native (the painter resamples them by the content scale). At the identity the map is
        // `x ↦ x`, so these equal `to_paint_rect` and the layout anchor (invariant I2).
        let rect = m.rect(bx);
        let cl = m.rect(clip);
        let anchor = m.point(bx.l.saturating_add(ox), bx.t.saturating_add(oy));
        let mut waiting = false;
        if let Some(b) = base
            && let Some(op) = CompositeOp::from_draw_mode(b.draw_mode)
            && let Some(img) = self.image_of(&b.image, &mut waiting)
        {
            let o = opacity_byte(
                self.prop(k, self.v.pids.alphablendmod)
                    .float()
                    .clamp(0.0, 1.0),
            );
            let fit = self.backdrop_fit(k, &img, bx, m);
            let layer = PaintOp::Image(ImageLayer {
                rect,
                clip: cl,
                anchor: fit.map_or(anchor, |f| f.0),
                image: img,
                op,
                opacity: o,
            });
            let _ = match fit {
                Some((_, f)) => list.push_cover(layer, f),
                None => list.push(layer),
            };
        }
        if let Some(ov) = overlay
            && let Some(op) = CompositeOp::from_draw_mode(ov.draw_mode)
        {
            let mut fit = None;
            let image = match &ov.image {
                Some(r) => self.image_of(r, &mut waiting).map(|i| {
                    fit = self.backdrop_fit(k, &i, bx, m);
                    (i, fit.map_or(anchor, |f| f.0))
                }),
                None => None,
            };
            if ov.image.is_none() || image.is_some() {
                let layer = PaintOp::Overlay(OverlayLayer {
                    rect,
                    clip: cl,
                    colour: ov.colour,
                    op,
                    image,
                });
                let _ = match fit {
                    Some((_, f)) => list.push_cover(layer, f),
                    None => list.push(layer),
                };
            }
        }
        if waiting {
            *pending = true;
            self.painter.image_waiters.insert(k);
        }
        if let Some(pt) = self.prepared_text(k) {
            let scaled = if m.active() {
                Some(self.text_params_of(k, None))
            } else {
                self.text_scaling_params(k)
            };
            if let Some(params) = scaled {
                self.emit_text_scaled(k, &pt, bx, clip, list, m, params);
                return;
            }
            let (sx, sy) = self
                .el(k)
                .and_then(|e| e.widget.scroll.as_ref())
                .map(|s| s.offset)
                .unwrap_or((0, 0));
            let origin = [bx.l.saturating_sub(sx), bx.t.saturating_sub(sy)];
            let cl_layout = Self::to_paint_rect(&clip);
            let cached = self.painter.text_runs.get(&k).filter(|c| {
                !self.painter.always_full
                    && Arc::ptr_eq(&c.prepared, &pt)
                    && c.origin == origin
                    && c.clip == cl_layout
                    && c.outline == pt.outline
                    && c.outline_colour == pt.outline_colour
            });
            if cached.is_none() {
                let params = DrawParams {
                    origin,
                    clip: cl_layout,
                    outline: pt.outline,
                    outline_colour: pt.outline_colour,
                    bullet: None,
                };
                let tp = crate::text::draw(&pt.layout, &pt.chars, &pt.fonts, &params);
                // The operations `push_to` appends to an empty list; it refuses the whole
                // text (glyph cap, operation cap) or appends all of it.
                let mut own = DisplayList::new();
                let ops = tp.push_to(&mut own, None).ok().map(|()| own.ops().to_vec());
                self.painter.text_runs.insert(
                    k,
                    TextRuns {
                        prepared: Arc::clone(&pt),
                        origin,
                        clip: cl_layout,
                        outline: pt.outline,
                        outline_colour: pt.outline_colour,
                        ops,
                    },
                );
            }
            // A text drawn on this path is at the identity: no physical layout is kept.
            self.painter.text.runs.remove(&k);
            if let Some(ops) = self.painter.text_runs.get(&k).and_then(|c| c.ops.as_ref())
                && list.len().saturating_add(ops.len()) <= crate::paint::MAX_OPS
            {
                for op in ops {
                    let _ = list.push(op.clone());
                }
            }
        }
    }

    /// The backdrop cover fit of an image on element `k` (`ui-scaling.md` §5.5): for an element
    /// in object mode Stretch with no edge in mode 6 and no stretch vector, whose layout box the
    /// run-time layout resized away from its design size, an image whose native size is the
    /// design size is drawn as one copy scaled uniformly by max(current ÷ design) per axis,
    /// centred and clipped (then resampled at the content scale, §6.3). Returns the canvas
    /// position of texel (0, 0) of the centred copy and the fit; `None` when the image tiles
    /// as `ui-layout.md` §9.6 says.
    fn backdrop_fit(
        &self,
        k: ElementKey,
        img: &tessera_material::ui_image::UiImage,
        bx: IBox,
        m: &CanvasMap,
    ) -> Option<([i32; 2], crate::paint::CoverFit)> {
        let e = self.el(k)?;
        if self.object_mode(k)? != object_mode::STRETCH || e.edges.contains(&6) || e.scale.is_some()
        {
            return None;
        }
        let native = [
            i32::try_from(img.width()).ok()?,
            i32::try_from(img.height()).ok()?,
        ];
        let design = [e.design.w(), e.design.h()];
        let current = [bx.w(), bx.h()];
        let f = crate::scaling::image::backdrop_cover(native, design, current)?;
        let cs = m.content_scale();
        let (ox, oy) = e.image_offset;
        // The copy's top-left relative to the element, in layout units: centred.
        let off = [
            (f64::from(current[0]) - f64::from(design[0]) * f) / 2.0 + f64::from(ox),
            (f64::from(current[1]) - f64::from(design[1]) * f) / 2.0 + f64::from(oy),
        ];
        let to_px = |v: f64, k: f64| -> i32 {
            (v * k)
                .round()
                .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
        };
        let anchor = [
            m.cx(bx.l).saturating_add(to_px(off[0], cs[0])),
            m.cy(bx.t).saturating_add(to_px(off[1], cs[1])),
        ];
        Some((
            anchor,
            crate::paint::CoverFit {
                scale: [f * cs[0], f * cs[1]],
            },
        ))
    }

    /// The crisp text path (`ui-scaling.md` §6.4): the element's text laid out in canvas pixels
    /// at the content scale with glyphs chosen or derived for their target height, drawn 1 : 1.
    ///
    /// Inputs in layout units are mapped to canvas pixels by the surface's edge mapping (§6.2):
    /// the element's origin (minus its scroll offsets), its clip, and (with a registered
    /// layout-parameter reader, whose output for the element is `params`) its width, height and
    /// margins; the content scale `c` is the surface's (§3.4) and the text factor *x* is the one
    /// that reaches the element (§7.5). With `c × x` = 1, game fonts and a pure translation
    /// (`Se` = 1 with no node scale) the shaper's layout is drawn unchanged, so every canvas at
    /// the identity is byte-identical (invariants I2, I7). Deriving glyphs and laying out run
    /// inside the paint slice and are counted in the frame's figures
    /// (`PaintFrameStats::glyphs_derived`, S7).
    ///
    /// **Stretched surfaces** (§6.1): a node scale that differs between the axes is a stretch
    /// vector *σ* (the uniform kinds of §3.4 scale both axes alike). The text is then laid out
    /// and drawn at the uniform content scale (*Se*) and the result is stretched by *σ* around
    /// the text origin, as the picture stretch §6.1 applies at composition
    /// ([`crate::text::stretch_text_ops`]); the hit test divides by *σ* (§6.4.6).
    ///
    /// The physical layout is cached apart from the draw: a scroll step or a move (origin or
    /// clip only) draws the cached layout again without laying the text out again.
    #[allow(clippy::too_many_arguments)]
    fn emit_text_scaled(
        &mut self,
        k: ElementKey,
        pt: &Arc<crate::env::PreparedText>,
        bx: IBox,
        clip: IBox,
        list: &mut DisplayList,
        m: &CanvasMap,
        params: Option<crate::text::LayoutParams>,
    ) {
        let started = std::time::Instant::now();
        let derived_before = self.painter.text.cache.insertions();
        let (sx, sy) = self
            .el(k)
            .and_then(|e| e.widget.scroll.as_ref())
            .map(|s| s.offset)
            .unwrap_or((0, 0));
        let cs = m.content_scale();
        let stretched = m.active() && (m.mx - m.my).abs() > 1e-6;
        let (kx, ky) = m.screen.map_or((1.0, 1.0), |f| (f.kx(), f.ky()));
        let (c, sigma) = if stretched {
            (ky, [m.mx, m.my])
        } else {
            (cs[1], [1.0, 1.0])
        };
        let x = self.text_factor_for(k, params.as_ref());
        let origin = m.point(bx.l.saturating_sub(sx), bx.t.saturating_sub(sy));
        let clip_c = m.rect(clip);
        let round_px = |v: i32, f: f64| -> i32 {
            (f64::from(v) * f)
                .round()
                .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
        };
        // Layout-unit offsets inside the element to (unstretched) canvas pixels.
        let fx = |v: i32| {
            if stretched {
                round_px(v, kx)
            } else {
                m.cx(bx.l.saturating_add(v)).saturating_sub(m.cx(bx.l))
            }
        };
        let fy = |v: i32| {
            if stretched {
                round_px(v, ky)
            } else {
                m.cy(bx.t.saturating_add(v)).saturating_sub(m.cy(bx.t))
            }
        };
        let box_phys = [fx(bx.w()), fy(bx.h())];
        let layout_key = crate::text_ui::LayoutKey {
            c: c.to_bits(),
            x: x.to_bits(),
            sigma: sigma.map(f64::to_bits),
            params: params.clone(),
            box_phys,
            epoch: self.painter.text.epoch,
        };
        let draw_key = crate::text_ui::DrawKey {
            origin,
            clip: clip_c,
            outline: pt.outline,
            outline_colour: pt.outline_colour,
        };
        let cached = self.painter.text.runs.get(&k).filter(|r| {
            !self.painter.always_full && Arc::ptr_eq(&r.prepared, pt) && r.layout_key == layout_key
        });
        let draw_fresh = cached.is_some_and(|r| r.draw_key == draw_key);
        let cached_layout = cached.map(|r| Arc::clone(&r.layout));
        if !draw_fresh {
            let rf = self.painter.text.resolve_fonts(&pt.fonts, c, x);
            let ctx = rf.ctx(c, x);
            let layout = match cached_layout {
                Some(l) => l,
                None => {
                    let unit_scale = cs == [1.0, 1.0] && (x - 1.0).abs() < 1e-12;
                    let suffix = pt.layout.truncation.as_ref().map(|t| t.suffix.as_slice());
                    let layout = if unit_scale && !rf.replaced {
                        Ok(pt.layout.clone())
                    } else {
                        match &params {
                            Some(p) => {
                                let pp = crate::text::physical_params(p, fx, fy, c);
                                crate::text::layout_scaled(&pt.chars, &rf.fonts, &pp, suffix, &ctx)
                            }
                            None => crate::text::relayout_lines(
                                &pt.layout, &pt.chars, &rf.fonts, &ctx, fx, fy, c,
                            ),
                        }
                    };
                    Arc::new(layout.unwrap_or_else(|e| {
                        tracing::debug!(error = %e, "scaled text layout refused");
                        crate::text::TextLayout::default()
                    }))
                }
            };
            // In a stretched surface the text is drawn unstretched around its origin, inside
            // the clip taken back by σ, and stretched afterwards.
            let unstretch = |v: i32, o: i32, s: f64, up: bool| -> i32 {
                let d = (f64::from(v) - f64::from(o)) / s;
                let d = if up { d.ceil() } else { d.floor() };
                (f64::from(o) + d).clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
            };
            let draw_clip = if stretched {
                Rect::new(
                    unstretch(clip_c.x0, origin[0], sigma[0], false),
                    unstretch(clip_c.y0, origin[1], sigma[1], false),
                    unstretch(clip_c.x1, origin[0], sigma[0], true),
                    unstretch(clip_c.y1, origin[1], sigma[1], true),
                )
            } else {
                clip_c
            };
            let dp = DrawParams {
                origin,
                clip: draw_clip,
                outline: pt.outline,
                outline_colour: pt.outline_colour,
                bullet: None,
            };
            let tp = crate::text::draw_scaled(
                &layout,
                &pt.chars,
                &rf.fonts,
                &dp,
                &ctx,
                &mut self.painter.text.cache,
            );
            let mut own = DisplayList::new();
            let mut ops = tp.push_to(&mut own, None).ok().map(|()| own.ops().to_vec());
            if stretched && let Some(o) = &ops {
                match crate::text::stretch_text_ops(o, origin, sigma, clip_c) {
                    Some(st) => ops = Some(st),
                    None => tracing::debug!("stretched text kept unstretched"),
                }
            }
            let at = m.point(bx.l, bx.t);
            self.painter.text.runs.insert(
                k,
                crate::text_ui::ScaledRuns {
                    prepared: Arc::clone(pt),
                    layout_key,
                    draw_key,
                    layout,
                    ops,
                    text_offset: [
                        origin[0].saturating_sub(at[0]),
                        origin[1].saturating_sub(at[1]),
                    ],
                    sigma,
                },
            );
        }
        self.painter.text_runs.remove(&k);
        if let Some(ops) = self.painter.text.runs.get(&k).and_then(|r| r.ops.as_ref())
            && list.len().saturating_add(ops.len()) <= crate::paint::MAX_OPS
        {
            for op in ops {
                let _ = list.push(op.clone());
            }
        }
        let t = &mut self.painter.text;
        t.derived = t
            .derived
            .saturating_add(t.cache.insertions().saturating_sub(derived_before));
        if !draw_fresh {
            let ns = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
            t.derive_ns = t.derive_ns.saturating_add(ns);
        }
    }

    /// Frame-loop step 8 (decision 0014 D8, decision 0015): layout, damage, paint jobs
    /// within `budget`, uploads, and the composition list. `now` is a monotonic time.
    pub fn paint(&mut self, now: Duration, budget: &mut dyn PaintBudget) -> UiFrame {
        self.flush_list_layouts();
        if self.painter.touched {
            // Every change the letterbox bars depend on (a layout box, visibility, parent,
            // property or the UI screen) touches the tree: work them out again.
            self.painter.letterbox = None;
        }
        if self.painter.touched {
            // The full surface pass when anything it decides may have changed; otherwise
            // only the damage of the recorded elements (same result, see `light_sync`).
            if self.painter.structural || self.painter.always_full || !self.light_sync() {
                self.sync_surfaces(now);
                self.painter.nodes = None;
                self.painter.passes.full = self.painter.passes.full.saturating_add(1);
            } else {
                self.painter.passes.light = self.painter.passes.light.saturating_add(1);
            }
            self.painter.structural = false;
        }
        // Cleared before the paint jobs, not after them: a change the jobs make (a text
        // cut to fit sets its truncated-text tooltip while its display list is built,
        // `ui-behaviour.md` §10.2) marks its element dirty for the next frame, which must
        // then run the surface pass, or the frame never settles.
        self.painter.touched = false;
        let rotated = std::mem::take(&mut self.painter.rotated);
        // Paint jobs.
        let mut sched = std::mem::replace(
            &mut self.painter.scheduler,
            Scheduler::new(self.painter.limits),
        );
        let (out, pending) = {
            let mut src = Source {
                ui: self,
                pending: Vec::new(),
            };
            let out = sched.frame(now, &mut src, budget);
            (out, src.pending)
        };
        for (s, p) in pending {
            sched.set_images_pending(s, p);
        }
        self.painter.scheduler = sched;
        // Composition: the nodes change only with the tree or a group's transition state.
        let running: Vec<(u32, bool)> = self
            .painter
            .recs
            .values()
            .filter(|r| r.group.0 == r.id.0)
            .map(|r| {
                (
                    r.group.0,
                    self.painter.scheduler.generation_running(r.group),
                )
            })
            .collect();
        let patched = self.painter.nodes.is_some()
            && running == self.painter.running
            && out.swapped.is_empty()
            && self.patch_rotations(&rotated);
        if !patched {
            let (nodes, index) = self.comp_nodes();
            self.painter.nodes = Some(nodes);
            self.painter.node_index = index;
            self.painter.running = running;
        }
        let mut composition = match &self.painter.nodes {
            Some(nodes) => {
                build(nodes, &self.painter.scheduler, &self.painter.config).unwrap_or_default()
            }
            None => CompositionList::default(),
        };
        let screen = self.ui_screen;
        let bars = match self.painter.letterbox.take() {
            Some((s, bars)) if s == screen => bars,
            _ => self.letterbox_bars(),
        };
        self.add_letterbox_fills(&mut composition, &bars);
        self.painter.letterbox = Some((screen, bars));
        let mut stats = out.stats;
        stats.glyphs_derived =
            u32::try_from(std::mem::take(&mut self.painter.text.derived)).unwrap_or(u32::MAX);
        stats.glyph_derive_ns = std::mem::take(&mut self.painter.text.derive_ns);
        // S7: the derivations alone on this thread, and the ladder steps of the workers.
        let t = &mut self.painter.text;
        (stats.glyph_derivation_ns, stats.glyph_derivation_max_ns) =
            t.cache.take_derivation_times();
        stats.ladder_steps = std::mem::take(&mut t.ladder_steps);
        stats.ladder_step_ns = std::mem::take(&mut t.ladder_step_ns);
        stats.ladder_step_max_ns = std::mem::take(&mut t.ladder_step_max_ns);
        UiFrame {
            ops: out.ops,
            composition,
            stats,
            surfaces: self.painter.recs.len(),
        }
    }

    /// Surfaces, coverage regions, layout changes and damage (run when the tree changed).
    fn sync_surfaces(&mut self, now: Duration) {
        // Surfaces of this frame.
        let Decided {
            surfaces: decided,
            nested: decided_nested,
            plan,
        } = self.decide_surfaces();
        self.painter.plan = plan;
        let mut new_recs: BTreeMap<ElementKey, SurfaceRec> = BTreeMap::new();
        let mut group_of: BTreeMap<ElementKey, GroupId> = BTreeMap::new();
        for ((k, stand_in, tile, index), nested) in decided.iter().zip(decided_nested.iter()) {
            // Group: the nearest surface above, else itself.
            let mut g = None;
            let mut cur = self.parent(*k);
            while let Some(c) = cur {
                if let Some(gg) = group_of.get(&c) {
                    g = Some(*gg);
                    break;
                }
                cur = self.parent(c);
            }
            let keep = self.painter.recs.get(k).filter(|r| match g {
                Some(gg) => gg == r.group,
                None => r.group.0 == r.id.0,
            });
            let id = match keep {
                Some(r) => r.id,
                None => {
                    let id = SurfaceId(self.painter.next_id);
                    self.painter.next_id = self.painter.next_id.wrapping_add(1).max(1);
                    id
                }
            };
            let group = g.unwrap_or(GroupId(id.0));
            group_of.insert(*k, group);
            new_recs.insert(
                *k,
                SurfaceRec {
                    id,
                    group,
                    canvas: self.canvas_size(*k),
                    displayed: self.displayed_box(*k).to_rect(),
                    coverage: Vec::new(),
                    stand_in: *stand_in,
                    tile: *tile,
                    sig: self.owner_sig(*k, *index),
                    has_nested: !nested.is_empty(),
                },
            );
        }
        // Release vanished surfaces (or ones that changed group).
        let old = std::mem::take(&mut self.painter.recs);
        for (k, r) in &old {
            match new_recs.get(k) {
                Some(n) if n.id == r.id => {}
                _ => {
                    self.painter.scheduler.release_surface(r.id);
                    self.painter.painted.drop_surface(r.id);
                }
            }
        }
        let now_secs = now;
        for (k, r) in &new_recs {
            if old.get(k).is_none_or(|o| o.id != r.id) {
                let _ = self
                    .painter
                    .scheduler
                    .create_surface(r.id, r.group, now_secs);
            }
        }
        // Coverage regions of nested entries (Decision 1.7).
        // The direct nested surfaces of each surface, in the order of a walk of its subtree
        // from the last child (the decision walk finds them in the opposite order).
        let nested_of: BTreeMap<ElementKey, &Vec<ElementKey>> = decided
            .iter()
            .zip(decided_nested.iter())
            .map(|((k, _, _, _), n)| (*k, n))
            .collect();
        let keys: Vec<ElementKey> = new_recs.keys().copied().collect();
        let mut nested_direct: Vec<ElementKey> = Vec::new();
        for k in &keys {
            nested_direct.clear();
            if let Some(n) = nested_of.get(k) {
                nested_direct.extend(n.iter().rev());
            }
            let parent = &new_recs[k];
            let pc = ParentCanvas {
                displayed: parent.displayed,
                canvas: parent.canvas,
                canvas_origin: [0, 0],
                stored: parent.canvas,
            };
            let mut cov = Vec::new();
            for &n in &nested_direct {
                if !self.shown_below(*k, n) {
                    continue;
                }
                let fp = self.footprint_node(n, &new_recs, 0);
                if let Some(r) = coverage_region(&fp, &pc) {
                    cov.push((new_recs[&n].id.0, r));
                }
            }
            if let Some(r) = new_recs.get_mut(k) {
                r.coverage = cov;
            }
        }
        // The per-surface content scale `c` the painter resamples by (`ui-scaling.md` §6.1):
        // the owner's content scale, `(1, 1)` at the identity so the crisp native paths run.
        for (k, r) in &new_recs {
            let c = self.canvas_map(*k).content_scale();
            self.painter.scheduler.set_surface_scale(r.id, c);
        }
        // Layout changes and damage.
        let mut changed_groups: BTreeMap<GroupId, Vec<SurfaceId>> = BTreeMap::new();
        for (k, r) in &new_recs {
            if let Some(o) = old.get(k).filter(|o| o.id == r.id) {
                if o.canvas != r.canvas {
                    self.painter.scheduler.damage_all(r.id);
                    changed_groups.entry(r.group).or_default().push(r.id);
                }
                if o.coverage != r.coverage {
                    for (_, c) in o.coverage.iter().chain(r.coverage.iter()) {
                        self.painter.scheduler.add_damage(r.id, *c);
                    }
                    changed_groups.entry(r.group).or_default().push(r.id);
                }
                let nested_moved = o.displayed != r.displayed && r.group.0 != r.id.0;
                if nested_moved {
                    changed_groups.entry(r.group).or_default();
                }
            } else {
                changed_groups.entry(r.group).or_default().push(r.id);
            }
        }
        self.painter.recs = new_recs;
        self.damage_pass();
        for (g, members) in changed_groups {
            let geo = self.group_geometry(g);
            self.painter.scheduler.layout_change(g, geo, &members);
        }
    }

    /// Appends the children of `k` in sibling order (no copy of the list).
    fn push_children(&self, k: ElementKey, out: &mut Vec<ElementKey>) {
        if let Some(e) = self.el(k) {
            out.extend_from_slice(&e.children);
        }
    }

    /// Whether `n` and every element between it and its ancestor `above` are visible (a
    /// nested surface under a hidden intermediate element covers nothing, decision 0015
    /// Decision 1.1).
    fn shown_below(&self, above: ElementKey, n: ElementKey) -> bool {
        let mut cur = Some(n);
        let mut depth = 0;
        while let Some(c) = cur {
            if c == above {
                return true;
            }
            if !self.is_visible(c) || depth > crate::ui::MAX_TREE_DEPTH + 2 {
                return false;
            }
            depth += 1;
            cur = self.parent(c);
        }
        false
    }

    fn footprint_node(
        &self,
        n: ElementKey,
        recs: &BTreeMap<ElementKey, SurfaceRec>,
        depth: usize,
    ) -> FootprintNode {
        let mut nested = Vec::new();
        if depth < 8 {
            let mut stack = Vec::new();
            self.push_children(n, &mut stack);
            while let Some(c) = stack.pop() {
                if recs.contains_key(&c) {
                    if self.shown_below(n, c) {
                        nested.push(self.footprint_node(c, recs, depth + 1));
                    }
                } else {
                    self.push_children(c, &mut stack);
                }
            }
        }
        let rot = self.el(n).map_or([0.0; 3], |e| e.rotation);
        FootprintNode {
            displayed: self.displayed_box(n).to_rect(),
            own_rotation: crate::paint::holds_rotation(rot),
            window: self.el(n).is_some_and(|e| e.root_panel),
            nested,
        }
    }

    /// Called when `k` is attached, detached or moves among its siblings. When its subtree
    /// holds anything the full surface pass decides ([`Ui::is_structural`]), the next pass
    /// with a change is the full one: the pre-order of the composition plan may change,
    /// and [`Ui::light_sync`] checks only each nested surface owner's own inputs. A subtree
    /// not attached below the screen root has nothing in the plan (attaching it later
    /// comes here again).
    pub(crate) fn note_order_change(&mut self, k: ElementKey) {
        /// Most elements checked before the full pass is asked for anyway.
        const MAX_CHECK: usize = 4_096;
        if !self.is_ancestor_or_self(self.screen, k) {
            return;
        }
        let mut stack = vec![k];
        let mut n = 0usize;
        while let Some(c) = stack.pop() {
            n += 1;
            if n > MAX_CHECK || self.is_structural(c) {
                self.painter.note_structure();
                return;
            }
            self.push_children(c, &mut stack);
        }
    }

    /// The position of `k` among its parent's children. `hint` is a position it may have
    /// (the one the full pass recorded, or the one its walk found): when the child there is
    /// `k` that is the answer, without searching the parent's list (whose length a plugin
    /// controls; a search per surface owner would make the surface passes quadratic in it).
    fn child_index(&self, k: ElementKey, hint: Option<usize>) -> Option<usize> {
        let pe = self.el(k)?.parent.and_then(|p| self.el(p))?;
        if let Some(i) = hint
            && pe.children.get(i) == Some(&k)
        {
            return Some(i);
        }
        pe.children.iter().position(|c| *c == k)
    }

    /// The owner's own inputs to the full surface pass ([`OwnerSig`]); `index` as
    /// [`Ui::child_index`] takes it.
    fn owner_sig(&self, k: ElementKey, index: Option<usize>) -> OwnerSig {
        let e = self.el(k);
        let parent = e.and_then(|e| e.parent);
        OwnerSig {
            parent,
            index: self.child_index(k, index),
            visible: e.is_some_and(|e| e.visible),
            displayed: self.is_displayed(k),
            root_panel: e.is_some_and(|e| e.root_panel),
            owns: self.owns_surface(k),
            mode: self.surface_mode(k),
            bx: e.map(|e| e.bx).unwrap_or_default(),
            physical: self.physical_box(k),
            map: self.canvas_map(k),
            node_scale: self.node_scale_at_or_above(k),
        }
    }

    /// The surface of an element that owns one whose decision the full pass would not
    /// change: not the screen root, a drag icon or external content, with a composition
    /// node, and with the canvas size, displayed box and [`OwnerSig`] of the last full pass.
    /// Its content can then be damaged by the light pass like any other element's (its own
    /// surface draws it from its root box).
    ///
    /// With `recorded` (a recorded owner, whose own subtree the light pass walks), changes
    /// of the owner's own geometry are taken too, when the full pass would answer them
    /// without deciding anything else again:
    ///
    /// - a new node scale and owner→canvas map with the same canvas size and displayed box
    ///   (a node-scale animation): the surface takes its new content scale;
    /// - for an owner with no surface nested in it that is a group root, or nested directly
    ///   in a group root, also a move and a new canvas size (floating text that rises and
    ///   grows): its displayed box and composition node move; a new canvas size repaints it
    ///   in full; as a nested entry its coverage region in its parent changes in place, and
    ///   its group gets the layout change the full pass gives it (Decision 1.5, 1.7). A
    ///   coverage region that appears or vanishes, deeper nesting, tile addressing and
    ///   stand-ins are left to the full pass.
    fn light_owner(&self, k: ElementKey, recorded: bool) -> Option<LightOwner> {
        let e = self.el(k)?;
        if k == self.screen || e.drag_icon || self.painter.external.contains_key(&k) {
            return None;
        }
        let r = self.painter.recs.get(&k)?;
        if !self.painter.node_index.contains_key(&k) {
            return None;
        }
        let canvas = self.canvas_size(k);
        let displayed = self.displayed_box(k).to_rect();
        let sig = self.owner_sig(k, r.sig.index);
        if r.sig == sig && r.displayed == displayed && r.canvas == canvas {
            return Some(LightOwner {
                id: r.id,
                update: None,
            });
        }
        if !recorded || r.tile || r.stand_in {
            return None;
        }
        // A Tile-mode owner whose new size has power-of-two sides would be decided as a Tile
        // surface ([`Ui::decide_surfaces`]): its addressing and its subtree's surfaces change.
        let tile = sig.mode == object_mode::TILE
            && pow2(sig.bx.w().max(0) as u32)
            && pow2(sig.bx.h().max(0) as u32);
        if tile != r.tile {
            return None;
        }
        // The signature apart from the owner's own geometry and scale.
        let same_rest = OwnerSig {
            map: r.sig.map,
            node_scale: r.sig.node_scale,
            bx: r.sig.bx,
            physical: r.sig.physical,
            ..sig.clone()
        } == r.sig;
        if !same_rest {
            return None;
        }
        let update = |cover| {
            Some(LightOwner {
                id: r.id,
                update: Some(OwnerUpdate {
                    sig: sig.clone(),
                    displayed,
                    canvas,
                    group: r.group,
                    cover,
                }),
            })
        };
        if r.displayed == displayed && r.canvas == canvas && sig.bx == r.sig.bx {
            return update(None);
        }
        if r.has_nested {
            return None;
        }
        if r.group.0 == r.id.0 {
            return update(None);
        }
        // Nested: the parent surface must be its group's root, and the coverage region must
        // stay in its parent's list (in place) or stay out of it.
        let mut cur = self.parent(k);
        let mut n = 0usize;
        let parent = loop {
            let c = cur?;
            if self.painter.recs.contains_key(&c) {
                break c;
            }
            n += 1;
            if n > crate::ui::MAX_TREE_DEPTH * 4 {
                return None;
            }
            cur = self.parent(c);
        };
        let pr = self.painter.recs.get(&parent)?;
        if pr.group != r.group || pr.group.0 != pr.id.0 {
            return None;
        }
        let pc = ParentCanvas {
            displayed: pr.displayed,
            canvas: pr.canvas,
            canvas_origin: [0, 0],
            stored: pr.canvas,
        };
        let region = if self.shown_below(parent, k) {
            let fp = self.footprint_node(k, &self.painter.recs, 0);
            coverage_region(&fp, &pc)
        } else {
            None
        };
        let listed = pr.coverage.iter().any(|(id, _)| *id == r.id.0);
        if listed != region.is_some() {
            return None;
        }
        update(Some((parent, region)))
    }

    /// Whether `k` takes part in what the full surface pass decides: a surface owner (or
    /// an element that would own one), a root panel (stand-in surfaces, clip windows), an
    /// element with a composition node or external content, the screen root, or an element
    /// that no longer exists.
    fn is_structural(&self, k: ElementKey) -> bool {
        let Some(e) = self.el(k) else { return true };
        k == self.screen
            || e.root_panel
            || e.drag_icon
            || self.painter.recs.contains_key(&k)
            || self.painter.node_index.contains_key(&k)
            || self.painter.external.contains_key(&k)
            || self.owns_surface(k)
    }

    /// The damage pass for a frame in which only the recorded paint-dirty elements changed
    /// and the surface decisions stay: no recorded element, nor anything below one, takes
    /// part in them ([`Ui::is_structural`]), except surface owners whose own inputs to the
    /// full pass are unchanged ([`Ui::light_owner`]; a recorded owner may also change the
    /// geometry that function lists, which is applied here as the full pass would).
    /// Then the surfaces, their canvas sizes, displayed boxes, coverage regions, groups
    /// and composition nodes are those of the last full pass (or updated here), and an
    /// element's canvas area can only have changed if it or an ancestor was recorded (every
    /// change of a layout box, visibility, parent or sibling order records the element it
    /// changes; one that moves a subtree holding a surface asks for the full pass,
    /// [`Ui::note_order_change`]). So walking the subtrees of the recorded elements, with
    /// the clip and depth their ancestors give and leaving nested surfaces to their own
    /// walks, finds exactly the damage of the full walk. Returns `false`, having changed
    /// nothing, when the full pass is needed.
    fn light_sync(&mut self) -> bool {
        /// Most elements below the recorded ones checked before the full pass is cheaper.
        const MAX_LIGHT: usize = 4_096;
        let recorded: Vec<ElementKey> = self.painter.dirty.iter().copied().collect();
        let mut stack: Vec<ElementKey> = Vec::new();
        let mut below = 0usize;
        // Where each walk starts: the element, and the surface that draws it with the walk
        // state the full walk gives it, or `None` when it is not drawn.
        let mut starts: Vec<LightStart> = Vec::new();
        // Recorded surface owners handled here ([`Ui::light_owner`]): their nodes are patched.
        let mut owners: Vec<ElementKey> = Vec::new();
        // Recorded owners whose scale, displayed box or canvas size changed.
        let mut rescaled: Vec<(ElementKey, SurfaceId, OwnerUpdate)> = Vec::new();
        for &k in &recorded {
            let own = self.light_owner(k, true);
            if own.is_none() && self.is_structural(k) {
                return false;
            }
            stack.clear();
            self.push_children(k, &mut stack);
            while let Some(c) = stack.pop() {
                below += 1;
                if below > MAX_LIGHT {
                    return false;
                }
                if self.is_structural(c) {
                    // A recorded element has its own iteration of this loop, which takes
                    // its own geometry changes or asks for the full pass; its walk covers
                    // its subtree.
                    if self.painter.dirty.contains(&c) {
                        continue;
                    }
                    // A nested surface the full pass would decide as before: the walk below
                    // skips it.
                    if self.light_owner(c, false).is_some() {
                        continue;
                    }
                    return false;
                }
                self.push_children(c, &mut stack);
            }
            if let Some(LightOwner { id, update }) = own {
                if let Some(u) = update {
                    rescaled.push((k, id, u));
                }
                // Its own surface draws it from its root box, as in the full damage walk.
                let Some(b) = self.el(k).map(|e| e.bx) else {
                    return false;
                };
                let root_box = IBox::from_xywh(0, 0, b.w(), b.h());
                starts.push((
                    k,
                    Some((id, self.canvas_map(k), Some((root_box, root_box, 0)))),
                ));
                owners.push(k);
                continue;
            }
            // The path up to the surface that draws `k`.
            let mut path: Vec<ElementKey> = Vec::new();
            let mut cur = self.parent(k);
            let mut owner = None;
            let mut covered = false;
            while let Some(c) = cur {
                if self.painter.recs.contains_key(&c) {
                    // A recorded owner is handled here (or the full pass runs): its walk
                    // includes this subtree.
                    covered = self.painter.dirty.contains(&c);
                    owner = Some(c);
                    break;
                }
                if self.painter.dirty.contains(&c) {
                    covered = true;
                    break;
                }
                path.push(c);
                if path.len() > crate::ui::MAX_TREE_DEPTH * 4 {
                    return false;
                }
                cur = self.parent(c);
            }
            if covered {
                // An ancestor's walk includes this subtree.
                continue;
            }
            let drawn = owner.and_then(|o| {
                let id = self.painter.recs.get(&o)?.id;
                let m = self.canvas_map(o);
                Some((id, m, self.walk_state_below(o, &path, k)))
            });
            starts.push((k, drawn));
        }
        // With fewer drawn items than the cap no surface walk is ever cut by it, in the full
        // pass or here.
        if self
            .painter
            .painted
            .len
            .saturating_add(below)
            .saturating_add(recorded.len())
            .saturating_add(self.painter.recs.len())
            > crate::paint::MAX_OPS
        {
            return false;
        }
        let mut painted = std::mem::take(&mut self.painter.painted);
        let mut walk: Vec<(ElementKey, WalkState)> = Vec::new();
        let mut cleaned: Vec<ElementKey> = Vec::new();
        for (k, drawn) in starts {
            let Some((owner_id, canvas_map, start @ Some(_))) = drawn else {
                walk.clear();
                walk.push((k, None));
                self.retire_walk(&mut walk, &mut painted);
                continue;
            };
            walk.clear();
            walk.push((k, start));
            while let Some((n, st)) = walk.pop() {
                // A nested surface draws itself (its owner is checked above), as in the
                // full damage walk.
                if n != k && self.painter.recs.contains_key(&n) {
                    continue;
                }
                let Some(e) = self.arena.get(n) else { continue };
                let drawn = st.and_then(|(bx, clip, depth)| {
                    if depth > crate::ui::MAX_TREE_DEPTH + 2 || !e.visible {
                        return None;
                    }
                    let clip = clip.intersect(&bx);
                    (!clip.is_empty()).then_some((bx, clip, depth))
                });
                match drawn {
                    Some((bx, clip, depth)) => {
                        let area = canvas_map.rect(clip);
                        match painted.get(n) {
                            Some((s, a)) if s == owner_id && a == area && !e.paint_dirty => {}
                            Some((s, a)) => {
                                self.painter.scheduler.add_damage(s, a);
                                self.painter.scheduler.add_damage(owner_id, area);
                            }
                            None => self.painter.scheduler.add_damage(owner_id, area),
                        }
                        painted.set(n, owner_id, area);
                        if e.paint_dirty {
                            cleaned.push(n);
                        }
                        for c in e.children.iter().rev() {
                            if let Some(cb) = self.arena.get(*c).map(|ce| ce.bx) {
                                walk.push((*c, Some((cb.offset(bx.l, bx.t), clip, depth + 1))));
                            }
                        }
                    }
                    None => {
                        if let Some((s, a)) = painted.remove(n) {
                            self.painter.scheduler.add_damage(s, a);
                        }
                        for c in e.children.iter().rev() {
                            walk.push((*c, None));
                        }
                    }
                }
            }
        }
        self.painter.painted = painted;
        cleaned.extend(std::mem::take(&mut self.painter.dirty));
        for k in cleaned {
            if let Some(e) = self.el_mut(k) {
                e.paint_dirty = false;
            }
        }
        // A changed owner's surface record, content scale, composition node, its parent's
        // coverage regions and its group's layout change, as the full pass sets them
        // (`sync_surfaces`: damage, then the layout changes).
        let mut changed_groups: BTreeMap<GroupId, Vec<SurfaceId>> = BTreeMap::new();
        let mut old_cover: BTreeMap<ElementKey, Vec<(u32, Rect)>> = BTreeMap::new();
        for (k, id, u) in rescaled {
            self.painter
                .scheduler
                .set_surface_scale(id, u.sig.map.content_scale());
            let Some(r) = self.painter.recs.get_mut(&k) else {
                continue;
            };
            let moved = r.displayed != u.displayed;
            let resized = r.canvas != u.canvas;
            // The node's destination is the owner's physical box.
            let new_dest = r.sig.physical != u.sig.physical;
            r.displayed = u.displayed;
            r.canvas = u.canvas;
            r.sig = u.sig;
            if moved || resized || new_dest {
                self.painter.nodes = None;
            }
            if resized {
                self.painter.scheduler.damage_all(id);
                changed_groups.entry(u.group).or_default().push(id);
            }
            if let Some((parent, region)) = u.cover {
                if moved {
                    changed_groups.entry(u.group).or_default();
                }
                if let Some(pr) = self.painter.recs.get_mut(&parent) {
                    old_cover
                        .entry(parent)
                        .or_insert_with(|| pr.coverage.clone());
                    if let Some(region) = region
                        && let Some(entry) = pr.coverage.iter_mut().find(|(i, _)| *i == id.0)
                    {
                        entry.1 = region;
                    }
                }
            }
        }
        for (parent, old) in old_cover {
            let Some(pr) = self.painter.recs.get(&parent) else {
                continue;
            };
            if pr.coverage != old {
                let (pid, group) = (pr.id, pr.group);
                let rects: Vec<Rect> = old
                    .iter()
                    .chain(pr.coverage.iter())
                    .map(|(_, c)| *c)
                    .collect();
                for c in rects {
                    self.painter.scheduler.add_damage(pid, c);
                }
                changed_groups.entry(group).or_default().push(pid);
            }
        }
        for (g, members) in changed_groups {
            let geo = self.group_geometry(g);
            self.painter.scheduler.layout_change(g, geo, &members);
        }
        // The node fields of a recorded owner that its signature does not decide, as
        // `comp_nodes` sets them.
        for k in owners {
            let opacity = self.prop(k, self.v.pids.object_opacity).float();
            let alpha_threshold = self.meter_alpha_threshold(k).unwrap_or(0.0);
            let rotation = self.el(k).map(|e| e.rotation);
            if let Some(&i) = self.painter.node_index.get(&k)
                && let Some(n) = self.painter.nodes.as_mut().and_then(|n| n.get_mut(i))
            {
                n.opacity = opacity;
                n.alpha_threshold = alpha_threshold;
                if let Some(r) = rotation {
                    n.rotation = r;
                }
            }
        }
        true
    }

    /// Retires every element of the walk's subtrees: its old area is damaged and it is
    /// no longer painted. Nested surfaces are left to their own walks, as in the full
    /// damage walk.
    fn retire_walk(&mut self, walk: &mut Vec<(ElementKey, WalkState)>, painted: &mut PaintedMap) {
        while let Some((n, _)) = walk.pop() {
            if self.painter.recs.contains_key(&n) {
                continue;
            }
            if let Some((s, a)) = painted.remove(n) {
                self.painter.scheduler.add_damage(s, a);
            }
            if let Some(e) = self.arena.get(n) {
                for c in e.children.iter().rev() {
                    walk.push((*c, None));
                }
            }
        }
    }

    /// The walk state the full damage walk gives `k` (its box in canvas coordinates, its
    /// parent's clip and its depth) when `owner`'s walk reaches it through `path` (the
    /// elements between them, nearest to `k` first), or `None` when the walk stops before
    /// `k` (a hidden element, an empty clip, the depth cap).
    fn walk_state_below(&self, owner: ElementKey, path: &[ElementKey], k: ElementKey) -> WalkState {
        let oe = self.el(owner)?;
        if !oe.visible {
            return None;
        }
        let root = IBox::from_xywh(0, 0, oe.bx.w(), oe.bx.h());
        let mut bx = root;
        let mut clip = root.intersect(&root);
        if clip.is_empty() {
            return None;
        }
        let mut depth = 0usize;
        for &n in path.iter().rev() {
            let e = self.el(n)?;
            depth += 1;
            bx = e.bx.offset(bx.l, bx.t);
            if depth > crate::ui::MAX_TREE_DEPTH + 2 || !e.visible {
                return None;
            }
            clip = clip.intersect(&bx);
            if clip.is_empty() {
                return None;
            }
        }
        let kb = self.el(k)?.bx;
        Some((kb.offset(bx.l, bx.t), clip, depth + 1))
    }

    /// Damage of elements whose canvas area or content changed (Decision 1.4).
    ///
    /// Walks every surface's elements in drawing order, as the painter sees them: an
    /// element is drawn into a surface when it and every element between it and the owner
    /// are visible, its clip (its box and its ancestors' boxes, in canvas coordinates) is
    /// not empty, and no nested surface lies on the way (a nested surface's owner is a
    /// marker and ends the walk there). At most [`crate::paint::MAX_OPS`] items are walked
    /// per surface. An element whose area changed, or that is dirty, damages its old and
    /// new areas; an element no longer drawn damages its old area.
    fn damage_pass(&mut self) {
        let recs = std::mem::take(&mut self.painter.recs);
        let mut painted = std::mem::take(&mut self.painter.painted);
        let mut stack = std::mem::take(&mut self.painter.walk_stack);
        painted.stamp = painted.stamp.wrapping_add(1);
        let stamp = painted.stamp;
        let mut cleaned: Vec<ElementKey> = Vec::new();
        for (owner, r) in &recs {
            let Some(b) = self.arena.get(*owner).map(|e| e.bx) else {
                continue;
            };
            let root_box = IBox::from_xywh(0, 0, b.w(), b.h());
            // Damage is in canvas pixels (`ui-scaling.md` §6.2), mapped by the same owner→canvas
            // transform the display list uses, so damage aligns with painting at every scale. At
            // the identity this is `clip.to_rect()` (invariant I2).
            let m = self.canvas_map(*owner);
            let mut walked = 0usize;
            stack.clear();
            stack.push((*owner, root_box, root_box, 0));
            while let Some((k, bx, clip, depth)) = stack.pop() {
                if depth > crate::ui::MAX_TREE_DEPTH + 2 || walked > crate::paint::MAX_OPS {
                    continue;
                }
                let Some(e) = self.arena.get(k) else { continue };
                if !e.visible {
                    continue;
                }
                if k != *owner && recs.contains_key(&k) {
                    walked += 1;
                    continue;
                }
                let clip = clip.intersect(&bx);
                if clip.is_empty() {
                    continue;
                }
                walked += 1;
                let area = m.rect(clip);
                let dirty = e.paint_dirty;
                match painted.get(k) {
                    Some((s, a)) if s == r.id && a == area && !dirty => {}
                    Some((s, a)) => {
                        self.painter.scheduler.add_damage(s, a);
                        self.painter.scheduler.add_damage(r.id, area);
                    }
                    None => self.painter.scheduler.add_damage(r.id, area),
                }
                painted.set(k, r.id, area);
                if dirty {
                    cleaned.push(k);
                }
                for c in e.children.iter().rev() {
                    if let Some(cb) = self.arena.get(*c).map(|ce| ce.bx) {
                        stack.push((*c, cb.offset(bx.l, bx.t), clip, depth + 1));
                    }
                }
            }
        }
        // Elements no longer drawn anywhere.
        for slot in &mut painted.slots {
            if let Some(p) = slot
                && p.stamp != stamp
            {
                self.painter.scheduler.add_damage(p.surface, p.area);
                *slot = None;
                painted.len = painted.len.saturating_sub(1);
            }
        }
        // Dirty flags are read only by the damage passes. Clearing the recorded and the
        // drawn ones is enough: any other element with the flag still set has never been
        // drawn since it was created, so it has no old area and is damaged when it is
        // drawn in any case.
        cleaned.extend(std::mem::take(&mut self.painter.dirty));
        for k in cleaned {
            if let Some(e) = self.el_mut(k) {
                e.paint_dirty = false;
            }
        }
        self.painter.painted = painted;
        self.painter.walk_stack = stack;
        self.painter.recs = recs;
    }

    fn group_geometry(&self, g: GroupId) -> GroupGeometry {
        let mut geo = GroupGeometry::default();
        let root = self.painter.recs.values().find(|r| r.id.0 == g.0);
        if let Some(r) = root {
            geo.root = r.displayed;
        }
        for r in self.painter.recs.values().filter(|r| r.group == g) {
            geo.entries.insert(
                r.id.0,
                (r.displayed.translate(-geo.root.x0, -geo.root.y0), r.canvas),
            );
        }
        geo
    }

    /// The composition input: surfaces, stand-ins, windows without a surface below one and
    /// external content, in tree pre-order (Decision 1.6).
    fn comp_nodes(&self) -> (Vec<CompNode>, HashMap<ElementKey, usize>) {
        let mut nodes: Vec<CompNode> = Vec::new();
        let mut index: HashMap<ElementKey, usize> = HashMap::new();
        // The elements with nodes, in tree pre-order, as the last full surface pass found
        // them (`decide_surfaces`): each with whether it is shown with every ancestor up to
        // the screen root (a surface under a hidden ancestor keeps its canvas but is not
        // shown, decision 0015 Decision 1.1) and the nearest earlier entry with a node.
        // Between full passes nothing they depend on changes (`light_sync`).
        let plan = &self.painter.plan;
        // Per entry, the node index its descendants take as their parent.
        let mut own: Vec<Option<usize>> = Vec::with_capacity(plan.len());
        for entry in plan {
            let (k, displayed_flag) = (entry.key, entry.shown);
            let parent_idx = entry.parent.and_then(|i| own.get(i).copied().flatten());
            // The index the descendants see: this element's node, if it gets one.
            let mut own_idx = parent_idx;
            let Some(e) = self.el(k) else {
                own.push(own_idx);
                continue;
            };
            let rec = self.painter.recs.get(&k);
            let ext = self.painter.external.get(&k).copied();
            let window_without =
                rec.is_none() && e.root_panel && parent_idx.is_some() && k != self.screen;
            if rec.is_none() && ext.is_none() && !window_without {
                own.push(own_idx);
                continue;
            }
            // The owner's physical box (`ui-scaling.md` §3.6), from its real displayed box.
            let physical_now = self.physical_box(k);
            if let Some(r) = rec {
                // The canvas's destination: the owner's physical box. While a generation is
                // shown, the swapped geometry (UI units) decides instead.
                let mut dest = physical_now;
                // While a generation runs, the picture drawn is the shown generation's, painted
                // at its canvas size: that size is composed (resampled) onto the destination
                // (`ui-scaling.md` §4.5 step 3), not the size the running job paints at.
                let mut shown_canvas = None;
                let mut shown = displayed_flag;
                if self.painter.scheduler.generation_running(r.group)
                    && let Some(geo) = self.painter.scheduler.shown(r.group)
                {
                    match geo.entries.get(&r.id.0) {
                        Some((rel, canvas)) => {
                            shown_canvas = Some(*canvas);
                            let root_now = self
                                .painter
                                .recs
                                .values()
                                .find(|x| x.id.0 == r.group.0)
                                .map_or(geo.root, |x| x.displayed);
                            let origin = if root_now.width() == geo.root.width()
                                && root_now.height() == geo.root.height()
                            {
                                (root_now.x0, root_now.y0)
                            } else {
                                (geo.root.x0, geo.root.y0)
                            };
                            dest = self.to_physical_rect(rel.translate(origin.0, origin.1));
                        }
                        None => shown = false,
                    }
                }
                let addressing = if r.tile {
                    let (ex, ey, _) = self.node_scale_at_or_above(k);
                    Addressing::Tile {
                        node_scale: [ex, ey],
                    }
                } else {
                    Addressing::Fit
                };
                let rotation = e.rotation;
                // The destination is the surface's physical box (`ui-scaling.md` §3.6, §6.6):
                // the canvas (painted in UI units) is composed onto it, 1 : 1 at `Se` = 1
                // (invariant I2) and GPU-scaled with `ui_scale_filter` at other scales.
                let mut n = CompNode::new(r.id, parent_idx, dest);
                n.window = e.root_panel;
                n.content = NodeContent::Canvas;
                n.shown = shown;
                n.canvas = shown_canvas.unwrap_or(r.canvas);
                n.addressing = addressing;
                n.opacity = self.prop(k, self.v.pids.object_opacity).float();
                n.stand_in = r.stand_in;
                n.rotation = rotation;
                n.alpha_threshold = self.meter_alpha_threshold(k).unwrap_or(0.0);
                own_idx = Some(nodes.len());
                index.insert(k, nodes.len());
                nodes.push(n);
            } else if window_without {
                let mut n = CompNode::new(SurfaceId(0), parent_idx, physical_now);
                n.window = true;
                n.content = NodeContent::None;
                n.shown = displayed_flag;
                own_idx = Some(nodes.len());
                index.insert(k, nodes.len());
                nodes.push(n);
            }
            if let Some(id) = ext {
                let entry = (id & 0xFFFF_FFFF) as u32 | 0x8000_0000;
                let mut n = CompNode::new(SurfaceId(entry), own_idx, physical_now);
                n.content = NodeContent::External(id);
                n.shown = displayed_flag;
                n.canvas = self.canvas_size(k);
                nodes.push(n);
            }
            own.push(own_idx);
        }
        (nodes, index)
    }

    /// Adds the letterbox bars of `ui-scaling.md` §5.4 step 6 ([`Ui::letterbox_bars`]) to the
    /// composition list as opaque black [`EntryTexture::Fill`] entries, each group's bars
    /// directly below the picture of its lowest shown member: before the first entry of that
    /// member's subtree in the drawing order (its own canvas, or, while that is not drawn yet,
    /// whatever of its subtree or the elements after it is drawn first), so a member that has
    /// not been painted yet still gets its bars. A member without a composition node (past the
    /// depth cap, below a Tile surface) draws no bars. The fills count toward the entry cap:
    /// those past it are dropped and counted in `counters.entries`. Nothing is added at the
    /// identity (no UI screen), so lists without a UI screen are unchanged (invariant I2).
    /// `bars` are those of [`Ui::letterbox_bars`], kept from the last frame that changed the
    /// tree (the scan over the fit groups does not run every frame).
    fn add_letterbox_fills(
        &self,
        list: &mut CompositionList,
        bars: &[crate::scaling::LetterboxBars],
    ) {
        if bars.is_empty() {
            return;
        }
        let Some(nodes) = &self.painter.nodes else {
            return;
        };
        // Drawing-order rank of every entry, doubled so that a canvas above (drawn after the
        // subtree of its nested entry, before the next node) sits at an odd rank between: the
        // ranks of the entries rise monotonically (pre-order with each canvas above after the
        // subtree it follows), so "the first entry of rank ≥ the member's" is the place.
        let mut end: Vec<usize> = (1..=nodes.len()).collect();
        for (j, n) in nodes.iter().enumerate().rev() {
            if let Some(p) = n.parent
                && p < j
                && let Some(ej) = end.get(j).copied()
                && let Some(ep) = end.get_mut(p)
            {
                *ep = (*ep).max(ej);
            }
        }
        let mut by_id: HashMap<u32, usize> = HashMap::with_capacity(nodes.len());
        let mut by_external: HashMap<u64, usize> = HashMap::new();
        for (j, n) in nodes.iter().enumerate() {
            match n.content {
                NodeContent::Canvas => {
                    by_id.entry(n.id.0).or_insert(j);
                }
                NodeContent::External(id) => {
                    by_id.entry(n.id.0).or_insert(j);
                    by_external.entry(id).or_insert(j);
                }
                NodeContent::None => {}
            }
        }
        let rank = |e: &crate::paint::Entry| -> Option<usize> {
            match e.texture {
                EntryTexture::Surface {
                    surface,
                    layer: crate::paint::Layer::Below,
                } => by_id.get(&surface.0).map(|j| 2 * j),
                EntryTexture::Surface {
                    layer: crate::paint::Layer::Above(nested),
                    ..
                } => by_id
                    .get(&nested)
                    .and_then(|j| end.get(*j))
                    .map(|e| (2 * e).saturating_sub(1)),
                EntryTexture::External(id) => by_external.get(&id).map(|j| 2 * j),
                EntryTexture::Fill { .. } => None,
            }
        };
        // (insertion index, member rank, bars) in insertion order.
        let mut inserts: Vec<(usize, usize, &[Rect])> = Vec::with_capacity(bars.len());
        for g in bars {
            let Some(&j) = self.painter.node_index.get(&g.below) else {
                tracing::trace!("letterbox bars of a member without a composition node skipped");
                continue;
            };
            let target = 2 * j;
            let at = list
                .entries
                .iter()
                .position(|e| rank(e).is_some_and(|r| r >= target))
                .unwrap_or(list.entries.len());
            inserts.push((at, target, g.rects.as_slice()));
        }
        inserts.sort_by_key(|(at, target, _)| (*at, *target));
        let room = self
            .painter
            .config
            .max_entries
            .saturating_sub(list.entries.len());
        let wanted: usize = inserts
            .iter()
            .map(|(_, _, r)| r.iter().filter(|r| !r.is_empty()).count())
            .sum();
        if wanted == 0 {
            return;
        }
        let mut left = room;
        let filter = self.painter.config.filter;
        let fill = |r: &Rect| crate::paint::Entry {
            texture: EntryTexture::Fill {
                colour: LETTERBOX_COLOUR,
            },
            source: crate::paint::Source::Straight,
            canvas: [r.width(), r.height()],
            tex_offset: [0, 0],
            dest: *r,
            addressing: Addressing::Fit,
            filter,
            opacity: 1.0,
            alpha_threshold: 0.0,
            rotations: 0..0,
            clips: 0..0,
        };
        let old = std::mem::take(&mut list.entries);
        let mut out = Vec::with_capacity(old.len().saturating_add(wanted.min(room)));
        let mut next = inserts.iter().peekable();
        for (i, e) in old.into_iter().enumerate() {
            while let Some((_, _, rects)) = next.next_if(|(at, _, _)| *at == i) {
                for r in rects.iter().filter(|r| !r.is_empty()) {
                    if left > 0 {
                        out.push(fill(r));
                        left -= 1;
                    }
                }
            }
            out.push(e);
        }
        for (_, _, rects) in next {
            for r in rects.iter().filter(|r| !r.is_empty()) {
                if left > 0 {
                    out.push(fill(r));
                    left -= 1;
                }
            }
        }
        let dropped = wanted.saturating_sub(room);
        if dropped > 0 {
            list.counters.entries = list.counters.entries.saturating_add(dropped as u64);
            tracing::debug!(dropped, "letterbox fills past the entry cap dropped");
        }
        list.entries = out;
    }

    /// Gives the composition nodes of `rotated` surfaces their current rotation (what
    /// [`Ui::comp_nodes`] would read). `false` when a surface has no node: the nodes are
    /// then built again.
    fn patch_rotations(&mut self, rotated: &[ElementKey]) -> bool {
        for k in rotated {
            let Some(rotation) = self.el(*k).map(|e| e.rotation) else {
                return false;
            };
            let Some(&i) = self.painter.node_index.get(k) else {
                return false;
            };
            match self.painter.nodes.as_mut().and_then(|n| n.get_mut(i)) {
                Some(n) => n.rotation = rotation,
                None => return false,
            }
        }
        true
    }
}

/// The scheduler's view of the UI's display lists.
struct Source<'a> {
    ui: &'a mut Ui,
    pending: Vec<(SurfaceId, bool)>,
}

impl SnapshotSource for Source<'_> {
    fn snapshot(&mut self, surface: SurfaceId) -> Option<Snapshot> {
        let owner = self
            .ui
            .painter
            .recs
            .iter()
            .find(|(_, r)| r.id == surface)
            .map(|(k, r)| (*k, r.canvas))?;
        let (k, size) = owner;
        if size[0] == 0 || size[1] == 0 {
            return None;
        }
        let (list, pending) = self.ui.build_display_list(k);
        self.pending.push((surface, pending));
        Some(Snapshot {
            list: Arc::new(list),
            size,
        })
    }
}
