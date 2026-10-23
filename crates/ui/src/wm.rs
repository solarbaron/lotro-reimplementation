// SPDX-License-Identifier: GPL-3.0-or-later
//! The window manager (`ui-behaviour.md` §8): moving by drag bars, resizing by resize bars,
//! and saved panel locations (`ui-layout.md` §9.9).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::events::{Answer, Message, MsgArg, UiEvent};
use crate::geom::{IBox, add, sub, trunc_f32, trunc_f64};
use crate::scaling::{DBox, RefBox, SavedPlacement, keep_on_screen};
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::vocab::commands;
use crate::widgets::WidgetClass;

/// A running move or resize of a target (`ui-behaviour.md` §8.1, §8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WmOp {
    /// Moving: the recorded pointer and screen position.
    Moving {
        pointer: (i32, i32),
        start: (i32, i32),
        bar: ElementKey,
    },
    /// Resizing: the recorded pointer, displayed size and handle.
    Resizing {
        pointer: (i32, i32),
        size: (i32, i32),
        handle: u8,
        bar: ElementKey,
    },
}

/// One saved panel location (`ui-layout.md` §9.9): two corners in normalized screen
/// coordinates and a visibility byte.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SavedEntry {
    /// Top-left corner, *x* from −1 (left) to +1 (right).
    pub x0: f32,
    /// Top-left corner, *y* from +1 (top) to −1 (bottom).
    pub y0: f32,
    /// Bottom-right corner.
    pub x1: f32,
    /// Bottom-right corner.
    pub y1: f32,
    /// Shown (non-zero) or hidden.
    pub visible: u8,
}

/// Tessera's store of saved panel locations, keyed by (layout identifier, element ID)
/// (`ui-layout.md` §9.9 OPEN: Tessera keeps its own store in its settings).
#[derive(Debug, Clone, Default)]
pub struct SavedStore {
    /// The anchored placements of `ui-scaling.md` §5.8, the current persisted form (it crosses
    /// into Tessera's settings as `[UI.panels]`). When a key has one it is authoritative, so a
    /// placement saved at one window size, scale or aspect reappears at the same visual place —
    /// and fully on screen — at another (§10.4, invariant I8).
    pub placements: BTreeMap<(u32, u32), SavedPlacement>,
    /// The legacy normalized-corner form (`ui-layout.md` §9.9), kept for [`SavedStore::to_xml`]
    /// and as the migration source [`Ui::apply_saved`] falls back to for a key that has only an
    /// old entry (`ui-scaling.md` §5.8 "Migration").
    pub entries: BTreeMap<(u32, u32), SavedEntry>,
    /// Elements registered as saved (`ui-behaviour.md` §8.4 step 1).
    pub(crate) registered: BTreeSet<ElementKey>,
}

/// Converts a normalized corner to pixels (`ui-layout.md` §9.9 step 1), 32-bit floats.
#[must_use]
pub fn normalized_to_pixels(nx: f32, ny: f32, w: i32, h: i32) -> (i32, i32) {
    let (wf, hf) = (w as f32, h as f32);
    let px = ((1.0 / wf + nx) + 1.0) * wf * 0.5;
    let py = (1.0 - (1.0 / hf + ny)) * hf * 0.5;
    (trunc_f32(px), trunc_f32(py))
}

/// The inverse of [`normalized_to_pixels`] at the centre of pixel (`x`, `y`), so that the
/// conversion gives back (`x`, `y`).
#[must_use]
pub fn pixels_to_normalized(x: i32, y: i32, w: i32, h: i32) -> (f32, f32) {
    let (wf, hf) = (w.max(1) as f32, h.max(1) as f32);
    let nx = 2.0 * (x as f32 + 0.5) / wf - 1.0 - 1.0 / wf;
    let ny = 1.0 - 2.0 * (y as f32 + 0.5) / hf - 1.0 / hf;
    (nx, ny)
}

impl SavedStore {
    /// The file form of `ui-layout.md` §9.9: one `Layout` element per layout identifier with
    /// `Element` children (`X`, `Y`, `Width`, `Height` as screen fractions, six decimals).
    /// Tessera writes the element `ID` as `0x` and eight upper-case hex digits (the
    /// original's form is OPEN there).
    #[must_use]
    pub fn to_xml(&self) -> String {
        let mut out = String::from("<Layouts>\n");
        let mut current: Option<u32> = None;
        for ((layout, element), e) in &self.entries {
            if current != Some(*layout) {
                if current.is_some() {
                    out.push_str("  </Layout>\n");
                }
                let _ = writeln!(out, "  <Layout ID=\"0x{layout:08X}\">");
                current = Some(*layout);
            }
            let x = (e.x0 + 1.0) * 0.5;
            let y = (1.0 - e.y0) * 0.5;
            let w = (e.x1 + 1.0) * 0.5 - x;
            let h = (1.0 - e.y1) * 0.5 - y;
            let _ = writeln!(
                out,
                "    <Element ID=\"0x{element:08X}\" X=\"{x:.6}\" Y=\"{y:.6}\" Width=\"{w:.6}\" Height=\"{h:.6}\"/>"
            );
        }
        if current.is_some() {
            out.push_str("  </Layout>\n");
        }
        out.push_str("</Layouts>\n");
        out
    }
}

impl Ui {
    /// The saved-location store.
    #[must_use]
    pub fn saved_store(&self) -> &SavedStore {
        &self.saved
    }

    /// Replaces the saved-location store (loaded from Tessera's settings).
    pub fn set_saved_store(&mut self, store: SavedStore) {
        let reg = std::mem::take(&mut self.saved.registered);
        self.saved = store;
        self.saved.registered = reg;
    }

    /// Whether an element is registered as saved (`ui-behaviour.md` §8.4 step 1).
    #[must_use]
    pub fn is_saved_element(&self, k: ElementKey) -> bool {
        self.saved.registered.contains(&k)
    }

    pub(crate) fn saved_flags_changed(&mut self, k: ElementKey) {
        let p = self.v.pids;
        let any = self.prop(k, p.save_loc).bool()
            || self.prop(k, p.save_size).bool()
            || self.prop(k, p.save_visibility).bool();
        if any {
            self.saved.registered.insert(k);
        } else {
            self.saved.registered.remove(&k);
        }
    }

    fn saved_key(&self, k: ElementKey) -> Option<(u32, u32)> {
        let e = self.el(k)?;
        Some((e.layout_did.0, e.element_id))
    }

    /// The reference box *R* a root's anchored placement is relative to (`ui-scaling.md` §5.8):
    /// the full screen *F* for a full-screen root (a root panel that covers its design area with
    /// edges (5, 5, 5, 5)), otherwise the safe area *A*. *R* is returned in UI units, matching
    /// the displayed boxes of §3.6.
    fn placement_reference(&self, k: ElementKey) -> (RefBox, DBox) {
        let full = self.el(k).is_some_and(|e| {
            if !(e.root_panel && e.parent == Some(self.screen) && e.edges == [5, 5, 5, 5]) {
                return false;
            }
            let (dw, dh) = e.layout_size;
            // "Covers the design area" (ui-scaling.md §5.2), inlined so this reads only the
            // element it already borrows.
            dw > 0
                && dh > 0
                && e.design.l <= 0
                && e.design.t <= 0
                && e.design.r >= dw - 1
                && e.design.b >= dh - 1
        });
        if full {
            let (w, h) = self.screen_size;
            (
                RefBox::Screen,
                DBox::new(0.0, 0.0, f64::from(w), f64::from(h)),
            )
        } else {
            (RefBox::Safe, self.safe_area_box())
        }
    }

    /// The reference box in UI units for a stored placement's own [`RefBox`].
    fn reference_box_for(&self, reference: RefBox) -> DBox {
        match reference {
            RefBox::Screen => {
                let (w, h) = self.screen_size;
                DBox::new(0.0, 0.0, f64::from(w), f64::from(h))
            }
            RefBox::Safe => self.safe_area_box(),
        }
    }

    /// Records the element's current location in the store as an anchored placement
    /// (`ui-scaling.md` §5.8 "Saving"): the anchor and offset come from where its displayed box
    /// lies in its reference box *R*, so the distance it keeps from an edge (or from the centre)
    /// is held in UI units across every configuration (invariant I8). The size is stored only
    /// with `UICore_Element_save_size` and the visibility only with
    /// `UICore_Element_save_visibility`. The anchored form is authoritative, so any stale legacy
    /// entry for the key is dropped.
    pub fn record_saved(&mut self, k: ElementKey) {
        let Some(key) = self.saved_key(k) else { return };
        let b = self.displayed_box(k);
        let (reference, r) = self.placement_reference(k);
        let p = self.v.pids;
        let size = if self.prop(k, p.save_size).bool() {
            Some((b.w(), b.h()))
        } else {
            None
        };
        let shown = if self.prop(k, p.save_visibility).bool() {
            Some(self.is_visible(k))
        } else {
            None
        };
        let box_ = DBox::new(
            f64::from(b.l),
            f64::from(b.t),
            f64::from(b.w()),
            f64::from(b.h()),
        );
        let placement = SavedPlacement::from_box(reference, &r, &box_, size, shown);
        self.saved.placements.insert(key, placement);
        self.saved.entries.remove(&key);
    }

    /// Applies the store to an element (`ui-layout.md` §9.9 steps 1–5, `ui-scaling.md` §5.8
    /// "Applying"). An anchored placement (§5.8) is preferred; a key that has only a legacy
    /// normalized-corner entry is applied by the old rule (the §5.8 "Migration" fallback).
    /// Returns whether a placement applied.
    pub fn apply_saved(&mut self, k: ElementKey) -> bool {
        self.apply_saved_with(k, true)
    }

    /// Applies the store to a root panel again after a change of *F* or *A*
    /// (`ui-scaling.md` §4.5 step 2, §5.8 "Applying"): its position and size, then
    /// keep-on-screen. The visibility a placement carries is left alone here: it is applied
    /// when the panel is opened ([`Ui::apply_saved`]), and code that shows or hides the panel
    /// since (a target frame without a target) keeps that state across a window resize.
    pub(crate) fn reapply_saved(&mut self, k: ElementKey) -> bool {
        self.apply_saved_with(k, false)
    }

    fn apply_saved_with(&mut self, k: ElementKey, visibility: bool) -> bool {
        let p = self.v.pids;
        let (sl, ss, sv) = (
            self.prop(k, p.save_loc).bool(),
            self.prop(k, p.save_size).bool(),
            self.prop(k, p.save_visibility).bool(),
        );
        if !(sl || ss || sv) {
            return false;
        }
        let sv = sv && visibility;
        if !(sl || ss || sv) {
            return false;
        }
        // `ui-scaling.md` §5.2: no saved placement is applied to a fit root (its displayed box
        // may reach beyond F by design).
        if self.ui_screen.is_some() && self.is_fit_root(k) {
            return false;
        }
        let Some(key) = self.saved_key(k) else {
            return false;
        };
        if let Some(placement) = self.saved.placements.get(&key).copied() {
            self.apply_placement(k, &placement, sl, ss, sv);
            self.keep_root_on_screen(k);
            self.post(k, self.v.msgs.saved_location_applied, MsgArg::default());
            return true;
        }
        let Some(e) = self.saved.entries.get(&key).copied() else {
            return false;
        };
        let (w, h) = self.screen_size;
        let (x0, y0) = normalized_to_pixels(e.x0, e.y0, w, h);
        let (x1, y1) = normalized_to_pixels(e.x1, e.y1, w, h);
        if sv {
            self.set_shown(k, e.visible != 0);
        }
        if sl {
            self.move_to(k, x0, y0);
        }
        if ss {
            self.resize(k, add(sub(x1, x0), 1), add(sub(y1, y0), 1));
        }
        self.keep_root_on_screen(k);
        self.post(k, self.v.msgs.saved_location_applied, MsgArg::default());
        true
    }

    /// Applies one anchored placement (`ui-scaling.md` §5.8 "Applying"): visibility, then the
    /// saved layout size (through the ordinary resize so the displayed extent is right before
    /// positioning), then the position computed back from the anchor and offset against the
    /// current reference box and displayed size. The displayed top-left is rounded to the
    /// nearest integer (halves up) and carried to the layout position through the current
    /// displayed box, exactly as [`Ui::keep_root_on_screen`] does, so for a root panel (parent
    /// = the screen root, node scale 1) it is exact. At the configuration it was saved at the
    /// result reproduces the same box (invariant I2 / I8).
    fn apply_placement(
        &mut self,
        k: ElementKey,
        placement: &SavedPlacement,
        sl: bool,
        ss: bool,
        sv: bool,
    ) {
        if sv && let Some(shown) = placement.shown {
            self.set_shown(k, shown);
        }
        if ss && let Some((w, h)) = placement.size {
            self.resize(k, w, h);
        }
        if sl {
            let r = self.reference_box_for(placement.reference);
            let d = self.displayed_box(k);
            let (x, y) = placement.position(&r, f64::from(d.w()), f64::from(d.h()));
            let (xi, yi) = (trunc_f64(x + 0.5), trunc_f64(y + 0.5));
            if xi != d.l || yi != d.t {
                let b = self.layout_box(k).unwrap_or_default();
                self.move_to(k, add(b.l, sub(xi, d.l)), add(b.t, sub(yi, d.t)));
            }
        }
    }

    /// Brings a root panel back inside the screen *F* (`ui-scaling.md` §5.8 "Keep-on-screen":
    /// after a root panel is laid out, item 1; after a saved placement is applied, item 2;
    /// after a change of *Se*, *F*, *A* or a per-window scale, items 3 and 5), through
    /// [`keep_on_screen`]. A panel
    /// already inside *F* does not move, so at the identity (`Se` = 1 with *F* = the screen
    /// size) this is a no-op and existing behaviour is unchanged. Non-root elements keep the
    /// layout's own clamping; a root panel's parent is the screen root (no node scale), so its
    /// displayed box is its absolute layout box and the clamp is exact in integers.
    pub(crate) fn keep_root_on_screen(&mut self, k: ElementKey) {
        if !self.el(k).is_some_and(|e| e.root_panel) {
            return;
        }
        // Fit roots are exempt (`ui-scaling.md` §5.2, §5.8): clipped to F, never moved.
        if self.ui_screen.is_some() && self.is_fit_root(k) {
            return;
        }
        let d = self.displayed_box(k);
        let (wu, hu) = self.screen_size;
        let (nx, ny) = keep_on_screen(
            f64::from(d.l),
            f64::from(d.t),
            f64::from(d.w()),
            f64::from(d.h()),
            wu,
            hu,
        );
        let (nx, ny) = (trunc_f64(nx + 0.5), trunc_f64(ny + 0.5));
        if nx != d.l || ny != d.t {
            let b = self.layout_box(k).unwrap_or_default();
            self.move_to(k, add(b.l, sub(nx, d.l)), add(b.t, sub(ny, d.t)));
        }
    }

    // ----- drag bars and resize bars -----------------------------------------------------

    fn bar_target(&self, bar: ElementKey) -> Option<ElementKey> {
        let p = self.v.pids;
        match self.class(bar)? {
            WidgetClass::Dragbar => {
                let id = self.prop(bar, p.dragbar_ancestor).key();
                if id != 0 {
                    let mut cur = self.parent(bar);
                    while let Some(c) = cur {
                        if self.element_id(c) == Some(id) {
                            return Some(c);
                        }
                        cur = self.parent(c);
                    }
                }
                self.parent(bar)
            }
            WidgetClass::ResizeBar => {
                let id = self.prop(bar, p.resize_target).key();
                if id != 0
                    && let Some(t) = self.find_nearest(bar, id)
                {
                    return Some(t);
                }
                self.parent(bar)
            }
            _ => None,
        }
    }

    /// The resize handle of a resize bar (`ui-behaviour.md` §8.2 step 1), 0 for none.
    #[must_use]
    pub fn resize_handle(&self, bar: ElementKey) -> u8 {
        let p = self.v.pids;
        let r = self.prop(bar, p.resize_border_right).bool();
        let l = self.prop(bar, p.resize_border_left).bool();
        let t = self.prop(bar, p.resize_border_top).bool();
        let b = self.prop(bar, p.resize_border_bottom).bool();
        match (r, l, t, b) {
            (true, _, true, _) => 3,
            (true, _, false, true) => 5,
            (true, _, false, false) => 4,
            (false, true, true, _) => 1,
            (false, true, false, true) => 7,
            (false, true, false, false) => 8,
            (false, false, true, _) => 2,
            (false, false, false, true) => 6,
            _ => 0,
        }
    }

    /// Drag-bar and resize-bar messages (§8.1, §8.2).
    pub(crate) fn wm_message(&mut self, bar: ElementKey, m: &Message) -> Answer {
        if m.source != bar {
            return Answer::NotHandled;
        }
        let ms = self.v.msgs;
        let left = m
            .arg
            .command
            .as_ref()
            .is_some_and(|c| &*c.name == commands::SELECT_LEFT);
        let Some(target) = self.bar_target(bar) else {
            return Answer::NotHandled;
        };
        if m.msg == ms.mouse_press && left {
            if self.el(target).is_some_and(|e| e.wm.is_some()) {
                return Answer::NotHandled;
            }
            let pointer = self.input.pointer.unwrap_or((0, 0));
            let op = if self.class(bar) == Some(WidgetClass::Dragbar) {
                let d = self.displayed_box(target);
                WmOp::Moving {
                    pointer,
                    start: (d.l, d.t),
                    bar,
                }
            } else {
                let handle = self.resize_handle(bar);
                if handle == 0 {
                    return Answer::NotHandled;
                }
                let d = self.displayed_box(target);
                WmOp::Resizing {
                    pointer,
                    size: (d.w(), d.h()),
                    handle,
                    bar,
                }
            };
            if let Some(e) = self.el_mut(target) {
                e.wm = Some(op);
            }
            if let Some(e) = self.el_mut(bar) {
                e.widget.bar_active = true;
            }
            self.switch_state_if_defined(bar, self.v.states.normal_pressed);
            return Answer::Handled;
        }
        if m.msg == ms.mouse_release && left {
            let op = self.el(target).and_then(|e| e.wm);
            let Some(op) = op else {
                return Answer::NotHandled;
            };
            if let Some(e) = self.el_mut(target) {
                e.wm = None;
            }
            if let Some(e) = self.el_mut(bar) {
                e.widget.bar_active = false;
            }
            self.switch_state_if_defined(bar, self.v.states.normal);
            let p = self.v.pids;
            let notify = match op {
                WmOp::Moving { .. } => self.prop(target, p.save_loc).bool(),
                WmOp::Resizing { .. } => self.prop(target, p.save_size).bool(),
            };
            if notify {
                // `ui-scaling.md` §5.8 "Saving": the end of a user move or resize of an element
                // with the save properties records its anchored placement, so a placement applied
                // again after a change of F or A puts the panel where the user left it.
                self.record_saved(target);
                self.emit(UiEvent::LayoutChanged { element: target });
            }
            return Answer::Handled;
        }
        Answer::NotHandled
    }

    /// Follows the pointer for every running move or resize (§8.1 step 3, §8.2 step 4).
    pub(crate) fn wm_pointer_moved(&mut self, x: i32, y: i32) {
        let Some(cap) = self.captured() else { return };
        if !self.el(cap).is_some_and(|e| e.widget.bar_active) {
            return;
        }
        let Some(target) = self.bar_target(cap) else {
            return;
        };
        let Some(op) = self.el(target).and_then(|e| e.wm) else {
            return;
        };
        match op {
            WmOp::Moving { pointer, start, .. } => self.move_step(target, pointer, start, (x, y)),
            WmOp::Resizing {
                pointer,
                size,
                handle,
                ..
            } => self.resize_step(target, pointer, size, handle, (x, y)),
        }
    }

    /// A screen position of a target converted to its layout position in its parent.
    fn screen_to_layout(&self, k: ElementKey, x: i32, y: i32) -> (i32, i32) {
        let Some(parent) = self.parent(k) else {
            return (x, y);
        };
        let pd = self.displayed_box(parent);
        let (px, py, _) = self.node_scale_at_or_above(parent);
        let (rx, ry) = (sub(x, pd.l), sub(y, pd.t));
        let lx = if px == 1.0 {
            rx
        } else {
            trunc_f32(rx as f32 / px)
        };
        let ly = if py == 1.0 {
            ry
        } else {
            trunc_f32(ry as f32 / py)
        };
        (lx, ly)
    }

    fn move_step(
        &mut self,
        target: ElementKey,
        pointer: (i32, i32),
        start: (i32, i32),
        now: (i32, i32),
    ) {
        let mut x = add(start.0, sub(now.0, pointer.0));
        let mut y = add(start.1, sub(now.1, pointer.1));
        let d = self.displayed_box(target);
        let (w, h) = (d.w(), d.h());
        let pb = match self.parent(target) {
            Some(p) => self.displayed_box(p),
            None => IBox::from_xywh(0, 0, self.screen_size.0, self.screen_size.1),
        };
        x = x.max(pb.l);
        x = x.min(add(sub(pb.r, w), 1));
        y = y.max(pb.t);
        y = y.min(add(sub(pb.b, h), 1));
        let (lx, ly) = self.screen_to_layout(target, x, y);
        self.move_to(target, lx, ly);
    }

    /// One resize step (§8.2 step 4, with the Tessera rule under a UI scale).
    fn resize_step(
        &mut self,
        target: ElementKey,
        pointer: (i32, i32),
        size: (i32, i32),
        handle: u8,
        now: (i32, i32),
    ) {
        let p = self.v.pids;
        let (mut w0, mut h0) = size;
        let (mut dx, mut dy) = (sub(now.0, pointer.0), sub(now.1, pointer.1));
        let (ex, ey, _) = self.node_scale_at_or_above(target);
        if ex != 1.0 || ey != 1.0 {
            let div = |v: i32, s: f32| trunc_f32(v as f32 / if s == 0.0 { 1.0 } else { s });
            w0 = div(w0, ex);
            dx = div(dx, ex);
            h0 = div(h0, ey);
            dy = div(dy, ey);
        }
        // Step 4.1.
        let mut w = w0;
        let mut h = h0;
        if matches!(handle, 1 | 7 | 8) {
            w = sub(w0, dx);
        }
        if matches!(handle, 3..=5) {
            w = add(w0, dx);
        }
        if matches!(handle, 1..=3) {
            h = sub(h0, dy);
        }
        if matches!(handle, 5..=7) {
            h = add(h0, dy);
        }
        // Step 4.2.
        let mut width_drives = matches!(handle, 1 | 4 | 5 | 8);
        // Step 4.3.
        let (maxw, minw, maxh, minh) = (
            self.prop(target, p.max_width),
            self.prop(target, p.min_width),
            self.prop(target, p.max_height),
            self.prop(target, p.min_height),
        );
        if maxw.is_set() && w > maxw.int() {
            w = maxw.int();
            width_drives = true;
        }
        if minw.is_set() && w < minw.int() {
            w = minw.int();
            width_drives = true;
        }
        if maxh.is_set() && h > maxh.int() {
            h = maxh.int();
            width_drives = false;
        }
        if minh.is_set() && h < minh.int() {
            h = minh.int();
            width_drives = false;
        }
        // Step 4.4.
        let ratio = self.prop(target, p.resize_aspect_ratio);
        if ratio.is_set() && ratio.float() != 0.0 && ratio.float().is_finite() {
            if width_drives {
                h = trunc_f32(w as f32 / ratio.float());
            } else {
                w = trunc_f32(h as f32 * ratio.float());
            }
        }
        // Step 4.5: anchoring on the layout box.
        let b = self.layout_box(target).unwrap_or_default();
        let (mut l, mut t, mut r, mut bm) = (b.l, b.t, b.r, b.b);
        if matches!(handle, 1 | 7 | 8) {
            l = add(sub(r, w), 1);
        }
        if matches!(handle, 3..=5) {
            r = sub(add(l, w), 1);
        }
        if matches!(handle, 1..=3) {
            t = add(sub(bm, h), 1);
        }
        if matches!(handle, 5..=7) {
            bm = sub(add(t, h), 1);
        }
        // Step 4.6.
        let (sw, sh) = self.screen_size;
        l = l.max(0);
        if !self.prop(target, p.ignore_resize_width_cap).bool() {
            r = r.min(sub(sw, 1));
        }
        t = t.max(0);
        if !self.prop(target, p.ignore_resize_height_cap).bool() {
            bm = bm.min(sub(sh, 1));
        }
        // Step 4.7.
        self.move_to(target, l, t);
        self.resize(target, add(sub(r, l), 1), add(sub(bm, t), 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scaling::{Anchor, UiScreen, safe_area};

    const A21: Option<f64> = Some(2.4);

    /// The displayed top-left [`Ui::apply_placement`] computes for a window of displayed size
    /// `w` × `h` under reference box `r`, with the same rounding and keep-on-screen clamp the
    /// method applies (the geometry of `ui-scaling.md` §5.8 "Applying", without a live tree).
    fn restore(p: &SavedPlacement, r: &DBox, w: i32, h: i32, wu: i32, hu: i32) -> (i32, i32) {
        let (x, y) = p.position(r, f64::from(w), f64::from(h));
        let (x, y) = keep_on_screen(x, y, f64::from(w), f64::from(h), wu, hu);
        (trunc_f64(x + 0.5), trunc_f64(y + 0.5))
    }

    /// Save → resize (another window size / scale / aspect) → restore keeps a safe-area window
    /// the same UI-unit distance from the edge it was docked to, and fully on screen
    /// (`ui-scaling.md` §5.8, §10.4; invariant I8). A fixed 200 × 100 window docked near the top
    /// right at a wide 16:9 screen reappears 20 units from the right and 40 from the top at a
    /// narrower one.
    #[test]
    fn save_resize_restore_round_trip_two_configs() {
        // Config A: F = A = 1920 × 1080 (16:9, so the 21:9 safe area is the whole screen).
        let fa = UiScreen::new(1920, 1080, 1.0).unwrap();
        let (wua, hua) = fa.ui_size();
        let ra = safe_area(&fa, A21);
        assert_eq!(ra, DBox::new(0.0, 0.0, 1920.0, 1080.0));
        let box_a = DBox::new(1700.0, 40.0, 200.0, 100.0);
        let p = SavedPlacement::from_box(RefBox::Safe, &ra, &box_a, Some((200, 100)), Some(true));
        assert_eq!(p.ax, Anchor::End);
        assert!((p.dx - 20.0).abs() < 1e-9);
        assert_eq!(p.ay, Anchor::Start);
        assert!((p.dy - 40.0).abs() < 1e-9);

        // I2/I8: restored at the same config, it lands on the same pixel box.
        let (xa, ya) = restore(&p, &ra, 200, 100, wua, hua);
        assert_eq!((xa, ya), (1700, 40));

        // Config B: F = A = 1536 × 864 (Se = 1.25, still 16:9). Same window size.
        let fb = UiScreen::new(1920, 1080, 1.25).unwrap();
        let (wub, hub) = fb.ui_size();
        assert_eq!((wub, hub), (1536, 864));
        let rb = safe_area(&fb, A21);
        let (xb, yb) = restore(&p, &rb, 200, 100, wub, hub);
        // Still 20 units from the right (1536 − (1316 + 200) = 20) and 40 from the top.
        assert_eq!((xb, yb), (1316, 40));
        assert_eq!(wub - (xb + 200), 20);
        assert!(
            xb >= 0 && yb >= 0 && xb + 200 <= wub && yb + 100 <= hub,
            "on screen"
        );
    }

    /// A window saved near the right edge of a wide screen is brought fully on screen by
    /// keep-on-screen when restored on a narrower one that cannot hold the full offset
    /// (`ui-scaling.md` §5.8 item 2, §10.3).
    #[test]
    fn restore_clamps_oversize_offset_on_screen() {
        // A window whose saved right-edge distance (dx) exceeds the narrow screen's slack.
        let fa = UiScreen::new(2560, 1440, 1.0).unwrap();
        let ra = safe_area(&fa, A21);
        // Docked at the far right: box (2360, 20, 200 × 100) → dx = 2560 − 2560 = 0.
        let box_a = DBox::new(2360.0, 20.0, 200.0, 100.0);
        let p = SavedPlacement::from_box(RefBox::Safe, &ra, &box_a, None, None);
        assert_eq!(p.ax, Anchor::End);
        // Restore on a 320-wide toy screen: the 200-wide window cannot sit at dx = 0 off the
        // right without overflow; keep-on-screen pulls it back to x = 120.
        let rb = DBox::new(0.0, 0.0, 320.0, 200.0);
        let (xb, _yb) = restore(&p, &rb, 200, 100, 320, 200);
        assert_eq!(xb, 120);
        assert!(xb + 200 <= 320, "right edge inside the narrow screen");
    }
}
