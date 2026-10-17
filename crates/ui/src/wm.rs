// SPDX-License-Identifier: GPL-3.0-or-later
//! The window manager (`ui-behaviour.md` §8): moving by drag bars, resizing by resize bars,
//! and saved panel locations (`ui-layout.md` §9.9).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::events::{Answer, Message, MsgArg, UiEvent};
use crate::geom::{IBox, add, sub, trunc_f32, trunc_f64};
use crate::scaling::keep_on_screen;
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
    /// The entries.
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

    /// Records the element's current location in the store.
    pub fn record_saved(&mut self, k: ElementKey) {
        let Some(key) = self.saved_key(k) else { return };
        let b = self.displayed_box(k);
        let (w, h) = self.screen_size;
        let (x0, y0) = pixels_to_normalized(b.l, b.t, w, h);
        let (x1, y1) = pixels_to_normalized(b.r, b.b, w, h);
        let visible = u8::from(self.is_visible(k));
        self.saved.entries.insert(
            key,
            SavedEntry {
                x0,
                y0,
                x1,
                y1,
                visible,
            },
        );
    }

    /// Applies the store to an element (`ui-layout.md` §9.9 steps 1–5). Returns whether an
    /// entry applied.
    pub fn apply_saved(&mut self, k: ElementKey) -> bool {
        let p = self.v.pids;
        let (sl, ss, sv) = (
            self.prop(k, p.save_loc).bool(),
            self.prop(k, p.save_size).bool(),
            self.prop(k, p.save_visibility).bool(),
        );
        if !(sl || ss || sv) {
            return false;
        }
        let Some(key) = self.saved_key(k) else {
            return false;
        };
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

    /// Brings a root panel back inside the screen *F* after a saved placement is applied
    /// (`ui-scaling.md` §5.8 "Keep-on-screen", item 2), through [`keep_on_screen`]. A panel
    /// already inside *F* does not move, so at the identity (`Se` = 1 with *F* = the screen
    /// size) this is a no-op and existing behaviour is unchanged. Non-root elements keep the
    /// layout's own clamping; a root panel's parent is the screen root (no node scale), so its
    /// displayed box is its absolute layout box and the clamp is exact in integers.
    fn keep_root_on_screen(&mut self, k: ElementKey) {
        if !self.el(k).is_some_and(|e| e.root_panel) {
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
