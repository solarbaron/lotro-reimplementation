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
    Addressing, CompNode, ComposeConfig, CompositeOp, CompositionList, DisplayList, FootprintNode,
    GpuOp, GroupId, ImageLayer, NodeContent, OverlayLayer, PaintBudget, PaintFrameStats,
    PaintLimits, PaintOp, ParentCanvas, Rect, Scheduler, Snapshot, SnapshotSource, SurfaceId,
    build, coverage_region, opacity_byte,
};
use crate::text::DrawParams;
use crate::tree::ElementKey;
use crate::ui::{Ui, object_mode};

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
}

/// Where the damage walk reaches an element: its box in canvas coordinates, its parent's
/// clip and its depth below the surface owner; `None` when the walk does not draw it.
type WalkState = Option<(IBox, IBox, usize)>;

/// What [`Ui::decide_surfaces`] finds.
#[derive(Debug, Default)]
struct Decided {
    /// (owner, stand-in, tile) in tree pre-order.
    surfaces: Vec<(ElementKey, bool, bool)>,
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
        self.painter.limits = limits;
        self.painter.note_structure();
    }

    /// Whether painting has settled: no tree change waits for the next paint step and the
    /// paint scheduler is idle (every surface drawn, no job, upload or generation left).
    /// Tests wait on this instead of a fixed number of frames, since the paint step works
    /// in time slices and a loaded machine needs more frames for the same work.
    #[must_use]
    pub fn paint_settled(&self) -> bool {
        !self.painter.touched && self.painter.dirty.is_empty() && self.painter.scheduler.is_idle()
    }

    /// The reference paint step: the full surface pass on every frame with a change
    /// (instead of only the damage of the changed elements when nothing structural
    /// changed), and text drawn again for every display list (no glyph-run cache). Both
    /// give the same frames; tests and benchmarks compare them.
    #[doc(hidden)]
    pub fn set_reference_paint(&mut self, on: bool) {
        self.painter.always_full = on;
        self.painter.text_runs.clear();
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
        // nearest surface above in `out.surfaces`)
        type Item = (
            ElementKey,
            bool,
            u32,
            bool,
            bool,
            Option<usize>,
            Option<usize>,
        );
        let screen_shown = self.is_visible(self.screen);
        let mut stack: Vec<Item> = vec![(self.screen, false, 0, false, screen_shown, None, None)];
        while let Some((k, under_tile, depth, surface_above, shown, plan_parent, surface)) =
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
                    out.surfaces.push((k, false, tile));
                    has = true;
                    tile_here = tile;
                } else if e.root_panel
                    && e.visible
                    && !surface_above
                    && self.object_mode(k) == Some(object_mode::NONE)
                {
                    out.surfaces.push((k, true, false));
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
            for c in e.children.iter().rev() {
                let child_shown = shown && self.is_visible(*c);
                stack.push((
                    *c,
                    tile_here,
                    ndepth,
                    above,
                    child_shown,
                    child_plan,
                    child_surface,
                ));
            }
        }
        out
    }

    /// The canvas size of a surface (decision 0015 §2.1).
    fn canvas_size(&self, k: ElementKey) -> [u32; 2] {
        let b = self.layout_box(k).unwrap_or_default();
        let max = self.painter.limits.max_surface_dim.max(1);
        [
            (b.w().max(0) as u32).min(max),
            (b.h().max(0) as u32).min(max),
        ]
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
            self.emit_children(owner, &children, bx, clip, recs, list, pending, depth);
            self.emit_own(k, bx, clip, list, pending);
        } else {
            self.emit_own(k, bx, clip, list, pending);
            self.emit_external(k, bx, recs, owner, list);
            self.emit_children(owner, &children, bx, clip, recs, list, pending, depth);
        }
        if after {
            self.emit_external(k, bx, recs, owner, list);
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
    ) {
        for c in children {
            if let Some(cb) = self.layout_box(*c) {
                let child_box = cb.offset(bx.l, bx.t);
                self.emit_rec(owner, *c, child_box, clip, recs, list, pending, depth + 1);
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
    ) {
        if let Some(id) = self.painter.external.get(&k).copied() {
            let entry = (id & 0xFFFF_FFFF) as u32 | 0x8000_0000;
            let _ = recs;
            let _ = owner;
            let _ = list.push(PaintOp::Nested {
                entry,
                coverage: bx.to_rect(),
            });
        }
    }

    fn image_of(
        &mut self,
        r: &ImageRef,
        pending: &mut bool,
    ) -> Option<Arc<tessera_material::ui_image::UiImage>> {
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
    ) {
        let Some(e) = self.el(k) else { return };
        let base = e.base.clone();
        let overlay = e.overlay.clone();
        let (ox, oy) = e.image_offset;
        let rect = Self::to_paint_rect(&bx);
        let cl = Self::to_paint_rect(&clip);
        let anchor = [bx.l.saturating_add(ox), bx.t.saturating_add(oy)];
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
            let _ = list.push(PaintOp::Image(ImageLayer {
                rect,
                clip: cl,
                anchor,
                image: img,
                op,
                opacity: o,
            }));
        }
        if let Some(ov) = overlay
            && let Some(op) = CompositeOp::from_draw_mode(ov.draw_mode)
        {
            let image = match &ov.image {
                Some(r) => self.image_of(r, &mut waiting).map(|i| (i, anchor)),
                None => None,
            };
            if ov.image.is_none() || image.is_some() {
                let _ = list.push(PaintOp::Overlay(OverlayLayer {
                    rect,
                    clip: cl,
                    colour: ov.colour,
                    op,
                    image,
                }));
            }
        }
        if waiting {
            *pending = true;
            self.painter.image_waiters.insert(k);
        }
        if let Some(pt) = self.prepared_text(k) {
            let (sx, sy) = self
                .el(k)
                .and_then(|e| e.widget.scroll.as_ref())
                .map(|s| s.offset)
                .unwrap_or((0, 0));
            let origin = [bx.l.saturating_sub(sx), bx.t.saturating_sub(sy)];
            let cached = self.painter.text_runs.get(&k).filter(|c| {
                !self.painter.always_full
                    && Arc::ptr_eq(&c.prepared, &pt)
                    && c.origin == origin
                    && c.clip == cl
                    && c.outline == pt.outline
                    && c.outline_colour == pt.outline_colour
            });
            if cached.is_none() {
                let params = DrawParams {
                    origin,
                    clip: cl,
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
                        clip: cl,
                        outline: pt.outline,
                        outline_colour: pt.outline_colour,
                        ops,
                    },
                );
            }
            if let Some(ops) = self.painter.text_runs.get(&k).and_then(|c| c.ops.as_ref())
                && list.len().saturating_add(ops.len()) <= crate::paint::MAX_OPS
            {
                for op in ops {
                    let _ = list.push(op.clone());
                }
            }
        }
    }

    /// Frame-loop step 8 (decision 0014 D8, decision 0015): layout, damage, paint jobs
    /// within `budget`, uploads, and the composition list. `now` is a monotonic time.
    pub fn paint(&mut self, now: Duration, budget: &mut dyn PaintBudget) -> UiFrame {
        self.flush_list_layouts();
        if self.painter.touched {
            // The full surface pass when anything it decides may have changed; otherwise
            // only the damage of the recorded elements (same result, see `light_sync`).
            if self.painter.structural || self.painter.always_full || !self.light_sync() {
                self.sync_surfaces(now);
                self.painter.nodes = None;
            }
            self.painter.structural = false;
        }
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
        let composition = match &self.painter.nodes {
            Some(nodes) => {
                build(nodes, &self.painter.scheduler, &self.painter.config).unwrap_or_default()
            }
            None => CompositionList::default(),
        };
        self.painter.touched = false;
        UiFrame {
            ops: out.ops,
            composition,
            stats: out.stats,
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
        for (k, stand_in, tile) in &decided {
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
            .map(|((k, _, _), n)| (*k, n))
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
    /// and none of them, nor anything below them, takes part in the surface decisions
    /// ([`Ui::is_structural`]). Then the surfaces, their canvas sizes, displayed boxes,
    /// coverage regions, groups and composition nodes are those of the last full pass, and
    /// an element's canvas area can only have changed if it or an ancestor was recorded
    /// (every change of a layout box, visibility, parent or sibling order records the
    /// element it changes). So walking the subtrees of the recorded elements, with the
    /// clip and depth their ancestors give, finds exactly the damage of the full walk.
    /// Returns `false`, having changed nothing, when the full pass is needed.
    fn light_sync(&mut self) -> bool {
        /// Most elements below the recorded ones checked before the full pass is cheaper.
        const MAX_LIGHT: usize = 4_096;
        let recorded: Vec<ElementKey> = self.painter.dirty.iter().copied().collect();
        let mut stack: Vec<ElementKey> = Vec::new();
        let mut below = 0usize;
        // Where each walk starts: the element, and the surface that draws it with the walk
        // state the full walk gives it, or `None` when it is not drawn.
        let mut starts: Vec<(ElementKey, Option<(SurfaceId, WalkState)>)> = Vec::new();
        for &k in &recorded {
            if self.is_structural(k) {
                return false;
            }
            stack.clear();
            self.push_children(k, &mut stack);
            while let Some(c) = stack.pop() {
                below += 1;
                if below > MAX_LIGHT || self.is_structural(c) {
                    return false;
                }
                self.push_children(c, &mut stack);
            }
            // The path up to the surface that draws `k`.
            let mut path: Vec<ElementKey> = Vec::new();
            let mut cur = self.parent(k);
            let mut owner = None;
            let mut covered = false;
            while let Some(c) = cur {
                if self.painter.recs.contains_key(&c) {
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
                Some((id, self.walk_state_below(o, &path, k)))
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
            let Some((owner_id, start @ Some(_))) = drawn else {
                walk.clear();
                walk.push((k, None));
                self.retire_walk(&mut walk, &mut painted);
                continue;
            };
            walk.clear();
            walk.push((k, start));
            while let Some((n, st)) = walk.pop() {
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
                        let area = clip.to_rect();
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
        true
    }

    /// Retires every element of the walk's subtrees: its old area is damaged and it is
    /// no longer painted.
    fn retire_walk(&mut self, walk: &mut Vec<(ElementKey, WalkState)>, painted: &mut PaintedMap) {
        while let Some((n, _)) = walk.pop() {
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
                let area = clip.to_rect();
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
            let displayed_now = self.displayed_box(k).to_rect();
            if let Some(r) = rec {
                let mut displayed = displayed_now;
                let mut shown = displayed_flag;
                if self.painter.scheduler.generation_running(r.group)
                    && let Some(geo) = self.painter.scheduler.shown(r.group)
                {
                    match geo.entries.get(&r.id.0) {
                        Some((rel, _)) => {
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
                            displayed = rel.translate(origin.0, origin.1);
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
                let mut n = CompNode::new(r.id, parent_idx, displayed);
                n.window = e.root_panel;
                n.content = NodeContent::Canvas;
                n.shown = shown;
                n.canvas = r.canvas;
                n.addressing = addressing;
                n.opacity = self.prop(k, self.v.pids.object_opacity).float();
                n.stand_in = r.stand_in;
                n.rotation = rotation;
                n.alpha_threshold = self.meter_alpha_threshold(k).unwrap_or(0.0);
                own_idx = Some(nodes.len());
                index.insert(k, nodes.len());
                nodes.push(n);
            } else if window_without {
                let mut n = CompNode::new(SurfaceId(0), parent_idx, displayed_now);
                n.window = true;
                n.content = NodeContent::None;
                n.shown = displayed_flag;
                own_idx = Some(nodes.len());
                index.insert(k, nodes.len());
                nodes.push(n);
            }
            if let Some(id) = ext {
                let entry = (id & 0xFFFF_FFFF) as u32 | 0x8000_0000;
                let mut n = CompNode::new(SurfaceId(entry), own_idx, displayed_now);
                n.content = NodeContent::External(id);
                n.shown = displayed_flag;
                n.canvas = self.canvas_size(k);
                nodes.push(n);
            }
            own.push(own_idx);
        }
        (nodes, index)
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
