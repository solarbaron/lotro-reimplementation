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

use std::collections::BTreeMap;
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
    painted: BTreeMap<ElementKey, (SurfaceId, Rect)>,
    pub external: BTreeMap<ElementKey, u64>,
    /// Composition input of the last tree change.
    nodes: Option<Vec<CompNode>>,
    /// Transition state of the groups when `nodes` was built.
    running: Vec<(u32, bool)>,
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
            painted: BTreeMap::new(),
            external: BTreeMap::new(),
            nodes: None,
            running: Vec::new(),
        }
    }

    pub fn forget(&mut self, k: ElementKey) {
        if let Some(r) = self.recs.remove(&k) {
            self.scheduler.release_surface(r.id);
        }
        if let Some((s, r)) = self.painted.remove(&k) {
            self.scheduler.add_damage(s, r);
        }
        self.external.remove(&k);
    }
}

fn pow2(v: u32) -> bool {
    v != 0 && v & (v - 1) == 0
}

/// One element of a surface's paint walk.
struct Walk {
    key: ElementKey,
    /// The clip (box ∩ ancestors' clips) in canvas coordinates.
    clip: IBox,
}

impl Ui {
    /// Sets the paint limits (decision 0015 §6–§7), for example the device's largest texture
    /// dimension. Existing surfaces are kept.
    pub fn set_paint_limits(&mut self, limits: PaintLimits) {
        self.painter.config = ComposeConfig::from_limits(&limits);
        self.painter.limits = limits;
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
        self.mark_dirty(k);
    }

    /// A pixel of the canvas below of an element's surface (CPU side, after the last
    /// [`Ui::paint`]), `0xAARRGGBB`; for tests and tools.
    #[must_use]
    pub fn surface_pixel(&self, k: ElementKey, x: i32, y: i32) -> Option<u32> {
        let id = self.painter.recs.get(&k)?.id;
        Some(self.painter.scheduler.canvases(id)?.below().pixel(x, y))
    }

    /// Marks every element that draws an image for repaint: the client calls it when
    /// decoded images arrived (decision 0015 Decision 1.8: a pending image draws nothing
    /// until it arrives).
    pub fn images_arrived(&mut self) {
        for k in self.arena.keys() {
            if let Some(e) = self.el_mut(k)
                && (e.base.is_some() || e.overlay.as_ref().is_some_and(|o| o.image.is_some()))
            {
                e.dirty = true;
            }
        }
        self.painter.touched = true;
    }

    /// Marks the whole UI for repaint.
    pub fn invalidate_paint(&mut self) {
        for k in self.arena.keys() {
            if let Some(e) = self.el_mut(k) {
                e.dirty = true;
            }
        }
        self.painter.touched = true;
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

    /// Decides the surfaces of this frame: (owner, stand-in, tile) in tree pre-order.
    fn decide_surfaces(&self) -> Vec<(ElementKey, bool, bool)> {
        let mut out = Vec::new();
        // (element, nearest surface above is Tile, surfaced depth)
        let mut stack: Vec<(ElementKey, bool, u32, bool)> = vec![(self.screen, false, 0, false)];
        while let Some((k, under_tile, depth, surface_above)) = stack.pop() {
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
                    out.push((k, false, tile));
                    has = true;
                    tile_here = tile;
                } else if e.root_panel
                    && e.visible
                    && !surface_above
                    && self.object_mode(k) == Some(object_mode::NONE)
                {
                    out.push((k, true, false));
                    has = true;
                }
            }
            if has {
                ndepth += 1;
                above = true;
            }
            for c in e.children.iter().rev() {
                stack.push((*c, tile_here, ndepth, above));
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

    /// The paint walk of a surface: every element drawn into its canvas, in drawing order,
    /// with its canvas box and clip. Nested surfaces appear as `Err(owner)` markers.
    fn surface_walk(
        &self,
        owner: ElementKey,
        surfaces: &BTreeMap<ElementKey, SurfaceRec>,
    ) -> Vec<Result<Walk, ElementKey>> {
        let mut out = Vec::new();
        let Some(b) = self.layout_box(owner) else {
            return out;
        };
        let root_box = IBox::from_xywh(0, 0, b.w(), b.h());
        self.walk_rec(owner, owner, root_box, root_box, surfaces, &mut out, 0);
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_rec(
        &self,
        owner: ElementKey,
        k: ElementKey,
        bx: IBox,
        clip: IBox,
        surfaces: &BTreeMap<ElementKey, SurfaceRec>,
        out: &mut Vec<Result<Walk, ElementKey>>,
        depth: usize,
    ) {
        if depth > crate::ui::MAX_TREE_DEPTH + 2 || out.len() > crate::paint::MAX_OPS {
            return;
        }
        let Some(e) = self.el(k) else { return };
        if !e.visible {
            return;
        }
        if k != owner && surfaces.contains_key(&k) {
            out.push(Err(k));
            return;
        }
        let clip = clip.intersect(&bx);
        if clip.is_empty() {
            return;
        }
        out.push(Ok(Walk { key: k, clip }));
        for c in &e.children {
            if let Some(cb) = self.layout_box(*c) {
                let child_box = cb.offset(bx.l, bx.t);
                self.walk_rec(owner, *c, child_box, clip, surfaces, out, depth + 1);
            }
        }
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
        if let Some(b) = base
            && let Some(op) = CompositeOp::from_draw_mode(b.draw_mode)
            && let Some(img) = self.image_of(&b.image, pending)
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
                Some(r) => self.image_of(r, pending).map(|i| (i, anchor)),
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
        if let Some(pt) = self.prepared_text(k) {
            let (sx, sy) = self
                .el(k)
                .and_then(|e| e.widget.scroll.as_ref())
                .map(|s| s.offset)
                .unwrap_or((0, 0));
            let params = DrawParams {
                origin: [bx.l.saturating_sub(sx), bx.t.saturating_sub(sy)],
                clip: cl,
                outline: pt.outline,
                outline_colour: pt.outline_colour,
                bullet: None,
            };
            let tp = crate::text::draw(&pt.layout, &pt.chars, &pt.fonts, &params);
            let _ = tp.push_to(list, None);
        }
    }

    /// Frame-loop step 8 (decision 0014 D8, decision 0015): layout, damage, paint jobs
    /// within `budget`, uploads, and the composition list. `now` is a monotonic time.
    pub fn paint(&mut self, now: Duration, budget: &mut dyn PaintBudget) -> UiFrame {
        self.flush_list_layouts();
        if self.painter.touched {
            self.sync_surfaces(now);
            self.painter.nodes = None;
        }
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
        if self.painter.nodes.is_none()
            || running != self.painter.running
            || !out.swapped.is_empty()
        {
            self.painter.nodes = Some(self.comp_nodes());
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
        let decided = self.decide_surfaces();
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
                    self.painter.painted.retain(|_, (s, _)| *s != r.id);
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
        let keys: Vec<ElementKey> = new_recs.keys().copied().collect();
        for k in &keys {
            let mut nested_direct: Vec<ElementKey> = Vec::new();
            let mut stack = self.children(*k);
            while let Some(c) = stack.pop() {
                if new_recs.contains_key(&c) {
                    nested_direct.push(c);
                } else {
                    stack.extend(self.children(c));
                }
            }
            let parent = &new_recs[k];
            let pc = ParentCanvas {
                displayed: parent.displayed,
                canvas: parent.canvas,
                canvas_origin: [0, 0],
                stored: parent.canvas,
            };
            let mut cov = Vec::new();
            for n in nested_direct {
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
            let mut stack = self.children(n);
            while let Some(c) = stack.pop() {
                if recs.contains_key(&c) {
                    if self.shown_below(n, c) {
                        nested.push(self.footprint_node(c, recs, depth + 1));
                    }
                } else {
                    stack.extend(self.children(c));
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

    /// Damage of elements whose canvas area or content changed (Decision 1.4).
    fn damage_pass(&mut self) {
        let recs = self.painter.recs.clone();
        let mut seen: BTreeMap<ElementKey, (SurfaceId, Rect)> = BTreeMap::new();
        for (owner, r) in &recs {
            let walk = self.surface_walk(*owner, &recs);
            for w in walk.into_iter().flatten() {
                let area = w.clip.to_rect();
                let dirty = self.el(w.key).is_some_and(|e| e.dirty);
                let prev = self.painter.painted.get(&w.key).copied();
                match prev {
                    Some((s, a)) if s == r.id && a == area && !dirty => {}
                    Some((s, a)) => {
                        self.painter.scheduler.add_damage(s, a);
                        self.painter.scheduler.add_damage(r.id, area);
                    }
                    None => self.painter.scheduler.add_damage(r.id, area),
                }
                seen.insert(w.key, (r.id, area));
            }
        }
        for (k, (s, a)) in &self.painter.painted {
            if !seen.contains_key(k) {
                self.painter.scheduler.add_damage(*s, *a);
            }
        }
        self.painter.painted = seen;
        for k in self.arena.keys() {
            if let Some(e) = self.el_mut(k) {
                e.dirty = false;
            }
        }
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
    fn comp_nodes(&self) -> Vec<CompNode> {
        let mut nodes: Vec<CompNode> = Vec::new();
        // (element, displayed: visible with every ancestor up to the screen root). A
        // surface under a hidden ancestor keeps its canvas but is not shown (decision 0015
        // Decision 1.1, "Surfaces under a hidden ancestor").
        let screen_shown = self.is_visible(self.screen);
        let mut stack: Vec<(ElementKey, bool)> = vec![(self.screen, screen_shown)];
        let mut order: Vec<(ElementKey, bool)> = Vec::new();
        while let Some(x) = stack.pop() {
            order.push(x);
            let (k, shown_here) = x;
            if let Some(e) = self.el(k) {
                for c in e.children.iter().rev() {
                    let child_shown = shown_here && self.is_visible(*c);
                    stack.push((*c, child_shown));
                }
            }
        }
        // Pre-order walk with an explicit parent mapping.
        let mut index_of: BTreeMap<ElementKey, usize> = BTreeMap::new();
        for (k, displayed_flag) in order {
            let mut parent_idx = None;
            let mut cur = self.parent(k);
            while let Some(c) = cur {
                if let Some(i) = index_of.get(&c) {
                    parent_idx = Some(*i);
                    break;
                }
                cur = self.parent(c);
            }
            let Some(e) = self.el(k) else { continue };
            let rec = self.painter.recs.get(&k);
            let ext = self.painter.external.get(&k).copied();
            let window_without =
                rec.is_none() && e.root_panel && parent_idx.is_some() && k != self.screen;
            if rec.is_none() && ext.is_none() && !window_without {
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
                index_of.insert(k, nodes.len());
                nodes.push(n);
            } else if window_without {
                let mut n = CompNode::new(SurfaceId(0), parent_idx, displayed_now);
                n.window = true;
                n.content = NodeContent::None;
                n.shown = displayed_flag;
                index_of.insert(k, nodes.len());
                nodes.push(n);
            }
            if let Some(id) = ext {
                let entry = (id & 0xFFFF_FFFF) as u32 | 0x8000_0000;
                let mut n = CompNode::new(
                    SurfaceId(entry),
                    index_of.get(&k).copied().or(parent_idx),
                    displayed_now,
                );
                n.content = NodeContent::External(id);
                n.shown = displayed_flag;
                n.canvas = self.canvas_size(k);
                nodes.push(n);
            }
        }
        nodes
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
