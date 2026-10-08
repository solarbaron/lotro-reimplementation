// SPDX-License-Identifier: GPL-3.0-or-later
//! Pointer routing (`ui-behaviour.md` §4): the point-in-element test, the hit test, hover
//! with its enter/leave chains, capture, presses, releases, clicks, the wheel, the cursor
//! and the pointer lock.

use std::sync::Arc;

use crate::env::ImageState;
use crate::events::{Answer, Command, MsgArg, UiEvent};
use crate::tree::ElementKey;
use crate::ui::{MAX_TREE_DEPTH, Ui};
use crate::vocab::{commands, input_maps};

/// A capture (`ui-behaviour.md` §4.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Capture {
    pub el: ElementKey,
    /// The capture's command set; empty for a capture begun by code.
    pub cmds: Vec<Arc<str>>,
    pub by_code: bool,
}

/// Pointer state.
#[derive(Debug, Clone, Default)]
pub(crate) struct InputState {
    pub pointer: Option<(i32, i32)>,
    /// The pointer left the window; no hit testing until a position inside arrives.
    pub outside: bool,
    /// The pointer lock with its frozen position (decision 0014 D7).
    pub lock: Option<(i32, i32)>,
    pub hit: Option<ElementKey>,
    pub hovered: Option<ElementKey>,
    pub capture: Option<Capture>,
    /// Geometry, visibility or flags changed since the last hit test.
    pub hover_dirty: bool,
    pub cursor_dirty: bool,
    pub cursor: Option<(u32, u32, u32)>,
    /// A cursor set by game code.
    pub cursor_override: Option<(u32, u32, u32)>,
    /// Time of the last pointer move or capture end (`ui-behaviour.md` §6.2 step 1).
    pub hover_time: f64,
    /// The plugin control a `ScrollableControls_Mouse` entry was pushed for (decision 0014
    /// D5 Tessera rule).
    pub plugin_pushed: Option<ElementKey>,
}

impl InputState {
    pub fn forget(&mut self, k: ElementKey) {
        if self.hit == Some(k) {
            self.hit = None;
        }
        if self.hovered == Some(k) {
            self.hovered = None;
        }
        if self.capture.as_ref().is_some_and(|c| c.el == k) {
            self.capture = None;
        }
        if self.plugin_pushed == Some(k) {
            self.plugin_pushed = None;
        }
        self.hover_dirty = true;
    }
}

impl Ui {
    /// The hit element of the last hit test.
    #[must_use]
    pub fn hit_element(&self) -> Option<ElementKey> {
        self.input.hit
    }

    /// The hovered element.
    #[must_use]
    pub fn hovered(&self) -> Option<ElementKey> {
        self.input.hovered
    }

    /// The captured element.
    #[must_use]
    pub fn captured(&self) -> Option<ElementKey> {
        self.input.capture.as_ref().map(|c| c.el)
    }

    /// The pointer position the UI last saw.
    #[must_use]
    pub fn pointer(&self) -> Option<(i32, i32)> {
        self.input.pointer
    }

    /// The cursor chosen by `ui-behaviour.md` §4.8 (`None` = the default cursor).
    #[must_use]
    pub fn cursor(&self) -> Option<(u32, u32, u32)> {
        self.input.cursor
    }

    /// Sets or clears the game-code cursor override (§4.8 item 3).
    pub fn set_cursor_override(&mut self, c: Option<(u32, u32, u32)>) {
        self.input.cursor_override = c;
        self.choose_cursor();
    }

    // ----- hit testing (§4.1–§4.2) -------------------------------------------------------

    /// The point-in-element test (§4.1) for a point relative to `k`'s parent.
    fn point_in(&mut self, k: ElementKey, x: i32, y: i32) -> bool {
        let b = self.displayed_rel(k);
        if !b.contains(x, y) {
            return false;
        }
        // Hit masks: none occurs in the data; their pass rule is OPEN (§4.1). Tessera
        // ignores a mask whose image is not loaded and otherwise tests alpha > 0.
        let (mask, thr, base) = match self.el(k) {
            Some(e) => (
                e.hit_mask,
                e.effective.get(&self.v.pids.alpha_threshold).cloned(),
                e.base.clone(),
            ),
            None => return false,
        };
        let (rx, ry) = (x.saturating_sub(b.l), y.saturating_sub(b.t));
        if mask != 0
            && let ImageState::Ready(img) = self.env.image(&crate::env::ImageRef::Did(mask))
            && let Some(px) = img.pixel(
                rx.rem_euclid(img.width().max(1) as i32) as u32,
                ry.rem_euclid(img.height().max(1) as i32) as u32,
            )
            && px >> 24 == 0
        {
            return false;
        }
        if let (Some(t), Some(base)) = (thr, base) {
            let threshold = crate::props::as_int(&t);
            let (ex, ey, ez) = self.node_scale_at_or_above(k);
            let differs = |v: f32| (v - 1.0).abs() > 0.0002;
            let (mut px, mut py) = (rx, ry);
            if differs(ex) || differs(ey) || differs(ez) {
                px = crate::geom::trunc_f32(px as f32 / if ex == 0.0 { 1.0 } else { ex });
                py = crate::geom::trunc_f32(py as f32 / if ey == 0.0 { 1.0 } else { ey });
            }
            if let ImageState::Ready(img) = self.env.image(&base.image) {
                let (ox, oy) = self.el(k).map(|e| e.image_offset).unwrap_or((0, 0));
                let ix = px.saturating_sub(ox).rem_euclid(img.width().max(1) as i32) as u32;
                let iy = py.saturating_sub(oy).rem_euclid(img.height().max(1) as i32) as u32;
                // OPEN (§4.1): ≥ or >. Tessera: alpha ≥ threshold passes.
                let a = img.pixel(ix, iy).map_or(0, |p| (p >> 24) as i32);
                if a < threshold {
                    return false;
                }
            }
        }
        true
    }

    fn hit_rec(&mut self, k: ElementKey, x: i32, y: i32, depth: usize) -> Option<ElementKey> {
        if depth > MAX_TREE_DEPTH + 2 {
            return None;
        }
        let e = self.el(k)?;
        if !e.visible || e.fade.is_some() || e.drag_icon {
            return None;
        }
        let b = self.displayed_rel(k);
        let (px, py) = (x.saturating_sub(b.l), y.saturating_sub(b.t));
        let kids = self.children(k);
        for c in kids.iter().rev() {
            if self.point_in(*c, px, py)
                && let Some(h) = self.hit_rec(*c, px, py, depth + 1)
            {
                return Some(h);
            }
        }
        let e = self.el(k)?;
        (e.mouse_visible || e.blocks_clicks).then_some(k)
    }

    /// The hit test (§4.2) at a screen position, against the current geometry (decision
    /// 0014 D8: pending list layouts are completed first).
    pub fn hit_test(&mut self, x: i32, y: i32) -> Option<ElementKey> {
        self.flush_list_layouts();
        let root = self.screen;
        let rb = self.displayed_rel(root);
        if !rb.contains(x, y) {
            return None;
        }
        // The fade test of §4.2 step 1 runs in `hit_rec`, the root included.
        self.hit_rec(root, x, y, 0)
    }

    // ----- hover (§4.4) ------------------------------------------------------------------

    fn chain(&self, k: Option<ElementKey>) -> Vec<ElementKey> {
        let mut v = Vec::new();
        let mut cur = k;
        while let Some(c) = cur {
            if !self.exists(c) || v.len() > MAX_TREE_DEPTH * 2 {
                break;
            }
            v.push(c);
            cur = self.parent(c);
        }
        v
    }

    /// Runs the hit test at the current pointer and updates the hover state (§4.4).
    pub(crate) fn update_hover(&mut self) {
        self.input.hover_dirty = false;
        let hit = match (self.input.pointer, self.input.lock, self.input.outside) {
            (Some((x, y)), None, false) => self.hit_test(x, y),
            _ => None,
        };
        self.input.hit = hit;
        let candidate = match &self.input.capture {
            Some(c) => (hit == Some(c.el)).then_some(c.el),
            None => hit.filter(|h| self.mouse_visible(*h)),
        };
        if candidate != self.input.hovered {
            let old = self.input.hovered;
            let mut oc = self.chain(old);
            let mut nc = self.chain(candidate);
            while oc.len() > 1 && nc.len() > 1 && oc.last() == nc.last() {
                oc.pop();
                nc.pop();
            }
            let mo = self.v.msgs.mouse_over;
            for e in &oc {
                self.post(*e, mo, MsgArg::value(0));
            }
            for e in &nc {
                self.post(*e, mo, MsgArg::value(1));
            }
            self.tooltip_hover_changed();
            if let Some(o) = old.filter(|o| self.exists(*o)) {
                self.post(o, self.v.msgs.mouse_over_top, MsgArg::value(0));
                if self.input.plugin_pushed == Some(o) {
                    self.band_pop_map(o, input_maps::SCROLLABLE_MOUSE);
                    self.input.plugin_pushed = None;
                }
            }
            let new = candidate.filter(|c| self.mouse_visible(*c));
            self.input.hovered = new;
            if let Some(n) = new {
                self.post(n, self.v.msgs.mouse_over_top, MsgArg::value(1));
                if self.el(n).is_some_and(|e| e.plugin.is_some()) {
                    self.band_push_map(n, input_maps::SCROLLABLE_MOUSE, 0);
                    self.input.plugin_pushed = Some(n);
                }
            }
            self.choose_cursor();
        }
        if self.drag.dragging() {
            self.drag_update_target();
        }
    }

    /// The step-7 hover refresh (decision 0014 D8 step 7 item 1): when geometry, visibility
    /// or flags changed since the last hit test, hit-test again and update the hover.
    pub fn hover_refresh(&mut self) {
        self.apply_change_requests();
        if self.input.hover_dirty {
            self.update_hover();
        }
        if self.input.cursor_dirty {
            self.choose_cursor();
        }
    }

    /// The cursor choice (§4.8).
    pub(crate) fn choose_cursor(&mut self) {
        self.input.cursor_dirty = false;
        let cap = self.captured().and_then(|c| self.cursor_of(c));
        let hov = self.input.hovered.and_then(|h| self.cursor_of(h));
        let c = cap.or(hov).or(self.input.cursor_override);
        if c != self.input.cursor {
            self.input.cursor = c;
            self.emit(UiEvent::Cursor { cursor: c });
        }
    }

    // ----- pointer input -----------------------------------------------------------------

    /// A pointer move to (`x`, `y`) in screen pixels (§4.4, §4.6 "Pointer moves", §6.2
    /// step 1, §7.1 step 2).
    pub fn pointer_moved(&mut self, x: i32, y: i32) {
        if self.input.lock.is_some() {
            return;
        }
        let (x, y) = (crate::geom::clamp_coord(x), crate::geom::clamp_coord(y));
        self.input.pointer = Some((x, y));
        let (sw, sh) = self.screen_size;
        if self.input.outside && x >= 0 && y >= 0 && x < sw && y < sh {
            self.input.outside = false;
        }
        self.input.hover_time = self.now;
        self.update_hover();
        self.drag_pointer_moved(x, y);
        self.wm_pointer_moved(x, y);
        let target = match &self.input.capture {
            Some(c) => Some(c.el),
            None if !self.drag.dragging() => self.input.hovered,
            None => None,
        };
        if let Some(t) = target.filter(|t| self.exists(*t)) {
            let e = self.el(t);
            let send =
                e.is_some_and(|e| !e.pressed.is_empty() || e.wants_moves || e.plugin.is_some());
            if send {
                let b = self.displayed_box(t);
                self.post(
                    t,
                    self.v.msgs.mouse_move,
                    MsgArg {
                        pos: (x.saturating_sub(b.l), y.saturating_sub(b.t)),
                        ..MsgArg::default()
                    },
                );
            }
        }
    }

    /// The pointer left the window (§4.9 step 2).
    pub fn pointer_left(&mut self) {
        self.input.outside = true;
        self.update_hover();
    }

    /// The window lost focus: nothing is hovered (and the input layer releases every hold,
    /// whose Ends arrive as releases).
    pub fn focus_lost(&mut self) {
        self.pointer_left();
    }

    /// Turns the pointer lock on with the frozen position, or off (decision 0014 D7,
    /// `ui-behaviour.md` §4.9 step 1). While it is on nothing is hit-tested or hovered.
    pub fn set_pointer_lock(&mut self, frozen: Option<(i32, i32)>) {
        let frozen =
            frozen.map(|(x, y)| (crate::geom::clamp_coord(x), crate::geom::clamp_coord(y)));
        self.input.lock = frozen;
        if let Some(f) = frozen {
            self.input.pointer = Some(f);
        }
        self.update_hover();
    }

    /// Whether the pointer lock is on.
    #[must_use]
    pub fn pointer_locked(&self) -> bool {
        self.input.lock.is_some()
    }

    fn rel_pos(&self, k: ElementKey) -> (i32, i32) {
        let (x, y) = self.input.pointer.unwrap_or((0, 0));
        let b = self.displayed_box(k);
        (x.saturating_sub(b.l), y.saturating_sub(b.t))
    }

    /// The press of a mouse-button command (§4.6). Returns whether it is consumed
    /// (decision 0014 D7: the press had a hit element or a capture).
    pub fn mouse_press(&mut self, c: &Command) -> bool {
        if self.input.lock.is_some() {
            return false;
        }
        self.update_hover();
        let hit = self.input.hit;
        let had_capture = self.input.capture.is_some();
        let left = &*c.name == commands::SELECT_LEFT;
        // Step 1.
        if left && !self.drag.dragging() {
            self.drag_press(self.input.pointer);
        }
        // Step 2 (capture, §4.5 step 1).
        match &mut self.input.capture {
            None => {
                if let Some(h) = self.input.hovered {
                    self.input.capture = Some(Capture {
                        el: h,
                        cmds: vec![c.name.clone()],
                        by_code: false,
                    });
                }
            }
            Some(cap) => {
                if !cap.cmds.is_empty() && !cap.cmds.contains(&c.name) {
                    cap.cmds.push(c.name.clone());
                }
            }
        }
        if left && !self.drag.dragging() {
            self.drag.candidate = self.input.hovered;
        }
        // Step 3: activation.
        if !had_capture {
            let r = hit.and_then(|h| self.root_panel_of(h));
            if r != self.focus.active || r.is_none() {
                match r {
                    Some(r) if self.prop(r, self.v.pids.activatable).bool() => self.activate(r),
                    _ => self.deactivate_active(),
                }
            }
        }
        // Step 4.
        let target = self
            .input
            .capture
            .as_ref()
            .map(|c| c.el)
            .or(self.input.hovered);
        if let Some(t) = target.filter(|t| self.exists(*t)) {
            let already = self.el(t).is_some_and(|e| e.pressed.contains(&c.name));
            if already {
                self.release_on(t, c, true);
            } else {
                if let Some(e) = self.el_mut(t)
                    && e.pressed.len() < 64
                {
                    e.pressed.push(c.name.clone());
                }
                let pos = self.rel_pos(t);
                self.post(
                    t,
                    self.v.msgs.mouse_press,
                    MsgArg {
                        command: Some(c.clone()),
                        pos,
                        ..MsgArg::default()
                    },
                );
            }
        }
        self.choose_cursor();
        hit.is_some() || self.input.capture.is_some()
    }

    /// Step 3 of the release rules on `t` (§4.6): `MouseUp`, then a click when `t` is the
    /// hovered element.
    fn release_on(&mut self, t: ElementKey, c: &Command, allow_click: bool) {
        let had = self.el_mut(t).is_some_and(|e| {
            let had = e.pressed.contains(&c.name);
            e.pressed.retain(|p| *p != c.name);
            had
        });
        if !had {
            return;
        }
        let pos = self.rel_pos(t);
        self.post(
            t,
            self.v.msgs.mouse_release,
            MsgArg {
                command: Some(c.clone()),
                pos,
                ..MsgArg::default()
            },
        );
        if allow_click && self.input.hovered == Some(t) && self.exists(t) {
            let accepts = self.el(t).is_some_and(|e| e.accepts_double_click);
            let (msg, cmd) = if c.is_double() && accepts {
                (self.v.msgs.mouse_dbl_click, c.clone())
            } else {
                (self.v.msgs.mouse_click, c.single_click())
            };
            self.post(
                t,
                msg,
                MsgArg {
                    command: Some(cmd),
                    pos,
                    ..MsgArg::default()
                },
            );
        }
    }

    /// The release of a mouse-button command (§4.6; during the pointer lock, decision 0014
    /// D7 Known difference 16).
    pub fn mouse_release(&mut self, c: &Command) -> bool {
        let locked = self.input.lock.is_some();
        let target = self
            .input
            .capture
            .as_ref()
            .map(|c| c.el)
            .or(self.input.hovered);
        // Step 1: capture ends first.
        let mut ended = false;
        if let Some(cap) = &mut self.input.capture
            && cap.cmds.contains(&c.name)
        {
            cap.cmds.retain(|x| *x != c.name);
            if cap.cmds.is_empty() && !cap.by_code {
                self.input.capture = None;
                ended = true;
            }
        }
        if ended {
            self.input.hover_time = self.now;
            self.update_hover();
        } else if !locked {
            self.update_hover();
        }
        // Step 2.
        if &*c.name == commands::SELECT_LEFT {
            if self.drag.dragging() {
                self.drag_drop();
            }
            self.drag.candidate = None;
            self.drag.press = None;
        }
        // Step 3.
        if let Some(t) = target.filter(|t| self.exists(*t)) {
            self.release_on(t, c, !locked);
        }
        self.choose_cursor();
        target.is_some()
    }

    /// A one-shot Impulse mouse-button command (toggle type 2): delivered to the element
    /// under the pointer as a notification that the plugin layer ignores
    /// (`lua-plugin-api.md` §13.12.1); no capture and no pressed set. Its End is ignored.
    pub fn mouse_impulse(&mut self, c: &Command) -> bool {
        if self.input.lock.is_some() {
            return false;
        }
        self.update_hover();
        let hit = self.input.hit;
        if let Some(t) = self
            .input
            .capture
            .as_ref()
            .map(|c| c.el)
            .or(self.input.hovered)
        {
            let pos = self.rel_pos(t);
            self.post(
                t,
                crate::vocab::tessera_msg::IMPULSE,
                MsgArg {
                    command: Some(c.clone()),
                    pos,
                    ..MsgArg::default()
                },
            );
        }
        hit.is_some() || self.input.capture.is_some()
    }

    /// A wheel command (§4.7). Returns whether a scroll host stopped it (consumed).
    pub fn wheel(&mut self, c: &Command) -> bool {
        if self.input.lock.is_some() {
            return false;
        }
        self.update_hover();
        let target = self.input.capture.as_ref().map(|c| c.el).or(self.input.hit);
        let Some(t) = target.filter(|t| self.exists(*t)) else {
            return false;
        };
        let pos = self.rel_pos(t);
        let a = self.post(
            t,
            self.v.msgs.mouse_scroll,
            MsgArg {
                command: Some(c.clone()),
                pos,
                ..MsgArg::default()
            },
        );
        a == Some(Answer::Stop)
    }

    /// Begins a capture by code (`ui-behaviour.md` §4.5 step 1, step 4: the old capture's
    /// pressed commands are released on it first).
    pub fn capture_by_code(&mut self, k: ElementKey) {
        self.switch_capture(k, true);
    }

    pub(crate) fn switch_capture(&mut self, k: ElementKey, by_code: bool) {
        if let Some(old) = self.input.capture.take() {
            if old.el != k {
                let pressed = self
                    .el(old.el)
                    .map(|e| e.pressed.clone())
                    .unwrap_or_default();
                for p in pressed {
                    let c = Command { name: p, id: None };
                    self.release_on(old.el, &c, false);
                }
            }
            if !by_code {
                self.input.capture = Some(Capture {
                    el: k,
                    cmds: old.cmds,
                    by_code,
                });
                self.update_hover();
                return;
            }
        }
        self.input.capture = Some(Capture {
            el: k,
            cmds: Vec::new(),
            by_code,
        });
        self.update_hover();
    }

    /// Ends a capture begun by code.
    pub fn release_capture(&mut self) {
        if self.input.capture.take().is_some() {
            self.input.hover_time = self.now;
            self.update_hover();
        }
    }
}
