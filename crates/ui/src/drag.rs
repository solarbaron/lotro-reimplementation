// SPDX-License-Identifier: GPL-3.0-or-later
//! Drag and drop of items (`ui-behaviour.md` §7).

use std::collections::BTreeMap;
use std::sync::Arc;

use tessera_formats::property::PropertyValue;

use crate::events::{MsgArg, UiEvent};
use crate::geom::{IBox, trunc_f32};
use crate::tree::{Element, ElementKey};
use crate::ui::{Ui, object_mode};
use crate::vocab::commands;

/// The drag description a game widget fills as a drag source (`ui-behaviour.md` §7.1
/// step 4): an opaque item reference of the client and the slot bits the item fits, which
/// drop targets with an accept mask check (§7.3 step 1, [`Ui::set_drop_mask`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DragPayload {
    /// The client's item reference.
    pub item: u64,
    /// The slot bits the item fits (for an item: its `Inventory_CompatibleSlot`).
    pub fits: u32,
}

/// Drag state.
#[derive(Debug, Clone, Default)]
pub(crate) struct DragState {
    /// Press position of `SelectLeft` (§4.6 step 1).
    pub press: Option<(i32, i32)>,
    /// The drag candidate (§7.1 step 1).
    pub candidate: Option<ElementKey>,
    /// The drag-in-progress state (§7.1 step 3).
    pub in_progress: bool,
    pub source: Option<ElementKey>,
    pub icon: Option<ElementKey>,
    /// The icon was made by the source's container (keys are swallowed, §7.4 step 4).
    pub by_container: bool,
    /// Pointer offset of the grabbed point (§7.2 step 3).
    pub grab: (i32, i32),
    pub target: Option<ElementKey>,
    /// States remembered by feedback (§7.3 step 3).
    pub remembered: BTreeMap<ElementKey, u32>,
    /// The source's drag description (§7.1 step 4).
    pub payload: Option<DragPayload>,
}

impl DragState {
    /// Whether a drag icon exists (an item drag, decision 0014 D7).
    pub fn dragging(&self) -> bool {
        self.icon.is_some()
    }

    /// Whether key commands are swallowed (§5.3 step 4, §7.4 step 4).
    pub fn swallows_keys(&self) -> bool {
        self.icon.is_some() && self.by_container
    }
}

impl Ui {
    /// Whether an item drag is in progress (a drag icon exists).
    #[must_use]
    pub fn item_drag_active(&self) -> bool {
        self.drag.dragging()
    }

    /// The current drop target.
    #[must_use]
    pub fn drop_target(&self) -> Option<ElementKey> {
        self.drag.target
    }

    /// The drag description of the drag in progress, if its source gave one.
    #[must_use]
    pub fn drag_payload(&self) -> Option<DragPayload> {
        self.drag.payload
    }

    /// The drag source of the drag in progress.
    #[must_use]
    pub fn drag_source(&self) -> Option<ElementKey> {
        self.drag.source
    }

    /// Sets the drag description `k` gives when a drag starts from it (§7.1 step 4; game
    /// code, for example an item slot).
    pub fn set_drag_payload(&mut self, k: ElementKey, payload: Option<DragPayload>) {
        if let Some(e) = self.el_mut(k) {
            e.drag_payload = payload;
        }
    }

    /// Names the element whose visual copy is the drag icon when a drag starts from `k`
    /// (§7.1 step 4: a game widget provides its own icon, for an item slot its icon);
    /// `None` leaves the icon to the container (step 5).
    pub fn set_drag_icon_source(&mut self, k: ElementKey, from: Option<ElementKey>) {
        if let Some(e) = self.el_mut(k) {
            e.drag_icon_from = from;
        }
    }

    /// Sets the accept check of a drop target (§7.3 step 1, set by game code): with
    /// `Some(mask)` the element refuses a drag whose description has no bit of `mask` in
    /// [`DragPayload::fits`] (and a drag without a description); `None` accepts every drag
    /// (the default).
    pub fn set_drop_mask(&mut self, k: ElementKey, mask: Option<u32>) {
        if let Some(e) = self.el_mut(k) {
            e.drop_mask = mask;
        }
    }

    /// The accept check of §7.3 step 1 for `k` and the current drag.
    fn accepts_drop(&self, k: ElementKey) -> bool {
        let p = self.v.pids;
        if !self.prop(k, p.container).bool() || self.prop(k, p.disallow_drag_in).bool() {
            return false;
        }
        match self.el(k).and_then(|e| e.drop_mask) {
            None => true,
            Some(mask) => self.drag.payload.is_some_and(|pl| pl.fits & mask != 0),
        }
    }

    pub(crate) fn drag_forget(&mut self, k: ElementKey) {
        if self.drag.candidate == Some(k) {
            self.drag.candidate = None;
        }
        if self.drag.source == Some(k) {
            self.drag.source = None;
        }
        if self.drag.icon == Some(k) {
            self.drag.icon = None;
        }
        if self.drag.target == Some(k) {
            self.drag.target = None;
        }
        self.drag.remembered.remove(&k);
    }

    /// The press part of §4.6 step 1: clear leftover state and record the press position.
    pub(crate) fn drag_press(&mut self, pos: Option<(i32, i32)>) {
        let icon = self.drag.icon;
        self.drag = DragState {
            press: pos,
            ..DragState::default()
        };
        if let Some(i) = icon {
            self.destroy(i);
        }
    }

    /// Pointer moves (§7.1 step 2, §7.2 step 3).
    pub(crate) fn drag_pointer_moved(&mut self, x: i32, y: i32) {
        if let Some(icon) = self.drag.icon {
            let (gx, gy) = self.drag.grab;
            self.move_to(icon, x.saturating_sub(gx), y.saturating_sub(gy));
            return;
        }
        let Some(cand) = self.drag.candidate else {
            return;
        };
        if self.drag.in_progress {
            return;
        }
        let held = self
            .input
            .capture
            .as_ref()
            .is_some_and(|c| c.cmds.iter().any(|n| &**n == commands::SELECT_LEFT));
        let Some((px, py)) = self.drag.press else {
            return;
        };
        let (sw, sh) = self.screen_size;
        if !held || px < 0 || py < 0 || px >= sw || py >= sh {
            return;
        }
        let (dx, dy) = (i64::from(x) - i64::from(px), i64::from(y) - i64::from(py));
        if dx * dx + dy * dy < 16 {
            return;
        }
        self.drag_start(cand, (px, py));
    }

    fn drag_start(&mut self, cand: ElementKey, press: (i32, i32)) {
        // Step 3.
        self.drag.in_progress = true;
        let pid = self.v.pids.dragable;
        let mut src = Some(cand);
        let mut n = 0;
        while let Some(s) = src {
            if self.prop(s, pid).bool() {
                break;
            }
            n += 1;
            if n > crate::ui::MAX_TREE_DEPTH * 2 {
                src = None;
                break;
            }
            src = self.parent(s);
        }
        let Some(source) = src else {
            let b = self.displayed_box(cand);
            self.post(
                cand,
                self.v.msgs.not_dragable,
                MsgArg {
                    pos: (press.0.saturating_sub(b.l), press.1.saturating_sub(b.t)),
                    ..MsgArg::default()
                },
            );
            return;
        };
        self.drag.source = Some(source);
        self.drag.payload = self.el(source).and_then(|e| e.drag_payload);
        if self.el(source).is_some_and(|e| e.plugin.is_some()) {
            self.emit(UiEvent::Control {
                control: source,
                event: crate::events::ControlEvent::DragStart,
            });
        }
        // Step 4: a game widget may provide its own icon (a copy of the element it
        // names); the base element provides none.
        if let Some(from) = self
            .el(source)
            .and_then(|e| e.drag_icon_from)
            .filter(|f| self.exists(*f))
        {
            if from == source {
                // Step 6: the icon would be the source itself.
                return;
            }
            let icon = self.icon_copy(from, true);
            self.drag.by_container = false;
            self.drag_icon_setup(source, icon, press);
            return;
        }
        // Step 5: the container.
        let Some(container) = self.parent(source) else {
            return;
        };
        if self.prop(container, self.v.pids.disallow_drag_out).bool() {
            return;
        }
        if !self.exists(source) {
            return;
        }
        let icon = self.icon_copy(source, false);
        self.drag.by_container = true;
        // Step 6 / §7.2.
        self.drag_icon_setup(source, icon, press);
    }

    /// A visual copy of `from` for a drag icon: its resolved descriptor without children,
    /// with its current base image and overlay (§7.1 step 5 OPEN, Tessera rule); with
    /// `with_state` (a game widget's own icon, step 4) also its run-time properties and
    /// text.
    fn icon_copy(&mut self, from: ElementKey, with_state: bool) -> ElementKey {
        let (desc, class) = self
            .el(from)
            .map(|e| (e.desc.clone(), e.class))
            .unwrap_or_else(|| {
                (
                    Arc::new(tessera_formats::ui_layout::ElementDesc::default()),
                    crate::widgets::WidgetClass::Field,
                )
            });
        let mut icon_el = Element::new(desc.element_id, class, Arc::clone(&desc));
        icon_el.effective = Ui::compute_effective(&icon_el);
        if let Some(se) = self.el(from) {
            icon_el.base = se.base.clone();
            icon_el.overlay = se.overlay.clone();
            icon_el.bx = se.bx;
            if with_state {
                icon_el.runtime = se.runtime.clone();
                icon_el.effective = se.effective.clone();
                icon_el.text.runtime = se.text.runtime.clone();
                icon_el.text.dirty = true;
            }
        }
        icon_el.created = true;
        icon_el.drag_icon = true;
        icon_el.root_panel = true;
        icon_el.mouse_visible = false;
        let icon = self.insert_element(icon_el);
        if with_state {
            // The game widget's icon may be drawn by children (the layers of
            // `crate::game::IconImage::Layers`): copy the ones with an image, one level.
            for c in self
                .children(from)
                .into_iter()
                .take(crate::game::MAX_ICON_LAYERS)
            {
                let Some(ce) = self.el(c).filter(|e| e.base.is_some()) else {
                    continue;
                };
                let mut copy = Element::new(ce.element_id, ce.class, Arc::clone(&ce.desc));
                copy.effective = ce.effective.clone();
                copy.base = ce.base.clone();
                copy.overlay = ce.overlay.clone();
                copy.bx = ce.bx;
                copy.created = true;
                let k = self.insert_element(copy);
                if self.attach_raw(k, icon).is_err() {
                    self.destroy(k);
                }
            }
        }
        icon
    }

    fn drag_icon_setup(&mut self, source: ElementKey, icon: ElementKey, press: (i32, i32)) {
        let p = self.v.pids;
        // Size (§7.2 step 2).
        let src_box = self.layout_box(source).unwrap_or_default();
        let allow = self.prop(source, p.allow_stretch_on_drag).bool();
        let src_mode = if self.owns_surface(source) {
            self.surface_mode(source)
        } else {
            self.root_panel_of(source)
                .map_or(object_mode::CANVAS_RESIZE, |r| self.surface_mode(r))
        };
        let (w, h) = if self.drag.by_container && allow && src_mode == object_mode::STRETCH {
            self.set_property(
                icon,
                p.object_mode,
                PropertyValue::Enum(object_mode::STRETCH),
            );
            (src_box.w(), src_box.h())
        } else {
            let (ex, ey, _) = self
                .root_panel_of(source)
                .map_or((1.0, 1.0, 1.0), |r| self.node_scale_at_or_above(r));
            let ib = self.layout_box(icon).unwrap_or_default();
            (trunc_f32(ib.w() as f32 * ex), trunc_f32(ib.h() as f32 * ey))
        };
        // Position (§7.2 step 3).
        let sb = self.displayed_box(source);
        let grab = (press.0.saturating_sub(sb.l), press.1.saturating_sub(sb.t));
        let (px, py) = self.input.pointer.unwrap_or(press);
        // Step 4: front-most z, inserted in front.
        let front_z = self
            .children(self.screen)
            .last()
            .and_then(|c| self.z_level(*c))
            .unwrap_or(0);
        if let Some(e) = self.el_mut(icon) {
            e.z = front_z;
            e.read_order = u32::MAX;
            e.bx = IBox::from_xywh(px.saturating_sub(grab.0), py.saturating_sub(grab.1), w, h)
                .clamped();
            e.screen_level = true;
            e.plugin = None;
        }
        let screen = self.screen;
        if let Some(e) = self.el_mut(icon) {
            e.parent = Some(screen);
        }
        if let Some(r) = self.el_mut(screen) {
            r.children.push(icon);
        }
        self.mark_dirty(icon);
        self.drag.icon = Some(icon);
        self.drag.grab = grab;
        self.switch_capture(icon, true);
        self.update_hover();
        self.post(icon, self.v.msgs.drag_global_begin, MsgArg::value(1));
        self.emit(UiEvent::DragBegan { source });
    }

    /// The drop target update of every hover update during a drag (§7.3).
    pub(crate) fn drag_update_target(&mut self) {
        let mut cur = self.input.hit;
        let mut found = None;
        let mut n = 0;
        while let Some(c) = cur {
            if self.accepts_drop(c) {
                found = Some(c);
                break;
            }
            n += 1;
            if n > crate::ui::MAX_TREE_DEPTH * 2 {
                break;
            }
            cur = self.parent(c);
        }
        if found != self.drag.target {
            let old = self.drag.target;
            self.drag.target = found;
            if let Some(o) = old.filter(|o| self.exists(*o)) {
                self.post(o, self.v.msgs.drag_over, MsgArg::value(0));
            }
            if let Some(nw) = found {
                self.post(nw, self.v.msgs.drag_over, MsgArg::value(1));
            }
        }
    }

    /// Feedback states (§7.3 step 3).
    pub(crate) fn drag_feedback(&mut self, k: ElementKey, entered: bool) {
        let p = self.v.pids;
        if !(self.prop(k, p.container).bool() && self.prop(k, p.drag_rollover).bool()) {
            return;
        }
        let st = self.v.states;
        if entered {
            let cur = self.state(k).unwrap_or(0);
            self.drag.remembered.insert(k, cur);
            let to = if self.prop(k, p.disallow_drag_in).bool() {
                st.drag_rollover_reject
            } else {
                st.drag_rollover_accept
            };
            self.switch_state(k, to);
        } else if let Some(s) = self.drag.remembered.remove(&k) {
            self.switch_state(k, s);
        }
    }

    /// The drop at the release of `SelectLeft` (§7.4).
    pub(crate) fn drag_drop(&mut self) {
        let p = self.v.pids;
        let source = self.drag.source;
        let icon = self.drag.icon;
        // Step 1.
        self.input.capture = None;
        self.update_hover();
        let target = self.drag.target;
        let mut accepted = false;
        if let Some(t) = target.filter(|t| self.exists(*t)) {
            let das = self.prop(t, p.drag_accepted_state);
            if das.is_set() {
                self.switch_state(t, das.key());
            } else if let Some(s) = self.drag.remembered.remove(&t) {
                self.switch_state(t, s);
            }
            self.post(t, self.v.msgs.stop_drag_item, MsgArg::default());
            accepted = self.drag.target == Some(t);
            if self.el(t).is_some_and(|e| e.plugin.is_some()) {
                self.emit(UiEvent::Control {
                    control: t,
                    event: crate::events::ControlEvent::DragDrop,
                });
            }
        }
        // Step 2.
        if let Some(c) = source.and_then(|s| self.parent(s)) {
            let msg = if accepted {
                self.v.msgs.drag_item_removed
            } else {
                self.v.msgs.stop_drag_item
            };
            self.post(c, msg, MsgArg::default());
        }
        // Step 3.
        if let Some(i) = icon {
            self.post(i, self.v.msgs.drag_global_end, MsgArg::default());
            self.destroy(i);
        }
        self.input.hover_time = self.now;
        if let Some(s) = source {
            self.emit(UiEvent::DragEnded {
                source: s,
                target,
                accepted,
            });
        }
        self.drag = DragState::default();
        self.update_hover();
    }
}
