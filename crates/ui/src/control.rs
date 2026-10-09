// SPDX-License-Identifier: GPL-3.0-or-later
//! The plugin-control element API that `tessera-plugins`' `Turbine.UI` bindings drive
//! (`lua-plugin-api.md` §13, decision 0015 §5).
//!
//! Plugin controls and windows are elements of the same tree as the native panels
//! (`lua-plugin-api.md` §13.10): a top-level window is a child of the screen root, a
//! control a child of its parent control. Their events reach the plugin host as
//! [`crate::events::UiEvent::Control`] in the outbox (§9.7, see [`crate::events`]); the
//! binding layer drains the outbox after each call it makes. The plugin layer's own events
//! (`EnabledChanged`, `Closing`/`Closed`, scroll-bar numbers) stay in the binding layer.

use std::sync::Arc;

use tessera_formats::property::PropertyValue;
use tessera_formats::ui_layout::ElementDesc;

use crate::UiError;
use crate::env::ImageRef;
use crate::geom::IBox;
use crate::layout::attachment_step;
use crate::tree::{Element, ElementKey, PluginData};
use crate::ui::{Ui, object_mode};
use crate::widgets::WidgetClass;

/// The plugin control classes the UI distinguishes (`lua-plugin-api.md` §13.2–§13.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ControlKind {
    /// `Turbine.UI.Control` (and classes with no element behaviour of their own).
    #[default]
    Control,
    /// `Turbine.UI.Window`: a root panel.
    Window,
    /// `Turbine.UI.Label`.
    Label,
    /// `Turbine.UI.TextBox`.
    TextBox,
    /// `Turbine.UI.Button`.
    Button,
    /// `Turbine.UI.CheckBox`.
    CheckBox,
    /// `Turbine.UI.ScrollBar` (its numbers live in the binding layer, LPA §9.6).
    ScrollBar,
    /// `Turbine.UI.ListBox`.
    ListBox,
    /// `Turbine.UI.TreeView`.
    TreeView,
}

impl ControlKind {
    fn class(self) -> WidgetClass {
        match self {
            Self::Control | Self::Window | Self::ScrollBar => WidgetClass::Field,
            Self::Label | Self::TextBox => WidgetClass::Text,
            Self::Button | Self::CheckBox => WidgetClass::Button,
            Self::ListBox => WidgetClass::ListBox,
            Self::TreeView => WidgetClass::Tree,
        }
    }
}

/// A background (`lua-plugin-api.md` §13.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Background {
    /// No background.
    None,
    /// A render surface by DID.
    Did(u32),
    /// A file relative to `Plugins/` (existence and format checked by the binding layer).
    File(String),
}

/// Subscriptions of the broadcasts (`lua-plugin-api.md` §9.7): `Update` and the key
/// messages, in subscription order.
#[derive(Debug, Clone, Default)]
pub(crate) struct Subscriptions {
    pub updates: Vec<ElementKey>,
    pub keys: Vec<ElementKey>,
}

impl Subscriptions {
    pub fn forget_set(&mut self, doomed: &std::collections::HashSet<ElementKey>) {
        self.updates.retain(|x| !doomed.contains(x));
        self.keys.retain(|x| !doomed.contains(x));
    }
}

/// First run-time element ID of plugin controls (a counter of the plugin layer,
/// `skin-definition.md` §6.8 note; outside the `UIElementID` key ranges).
pub const PLUGIN_ELEMENT_ID_BASE: u32 = 0x7000_0000;

/// Initial control size (`lua-plugin-api.md` §13.1).
pub const INITIAL_SIZE: i32 = 50;

/// Packs a plugin `Color` (A, R, G, B as 32-bit floats) by `lua-plugin-api.md` §13.17:
/// each × 255.0 truncated (NaN and out-of-range → −2³¹), combined with shifts and ORs in
/// 32-bit arithmetic, without clamping.
#[must_use]
pub fn plugin_colour(a: f32, r: f32, g: f32, b: f32) -> u32 {
    let c = |v: f32| -> i32 {
        let x = v * 255.0;
        if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
            i32::MIN
        } else {
            x as i32
        }
    };
    let mut v = c(a) as u32;
    v = (v << 8) | c(r) as u32;
    v = (v << 8) | c(g) as u32;
    v = (v << 8) | c(b) as u32;
    v
}

impl Ui {
    fn pd(&self, k: ElementKey) -> Option<&PluginData> {
        self.el(k).and_then(|e| e.plugin.as_deref())
    }

    fn pd_mut(&mut self, k: ElementKey) -> Option<&mut PluginData> {
        self.el_mut(k).and_then(|e| e.plugin.as_deref_mut())
    }

    /// Whether an element is a plugin control.
    #[must_use]
    pub fn is_control(&self, k: ElementKey) -> bool {
        self.pd(k).is_some()
    }

    /// Creates a plugin control of `kind` for `apartment` (`lua-plugin-api.md` §13.1): at
    /// (0, 0), 50 × 50, hidden, with no background and no overlay. A window is a root panel
    /// attached to the screen root; any other control has no parent yet.
    ///
    /// # Errors
    /// [`UiError::TooManyElements`] past the element cap.
    pub fn create_control(
        &mut self,
        kind: ControlKind,
        apartment: u32,
    ) -> Result<ElementKey, UiError> {
        if self.arena.len() >= crate::ui::MAX_ELEMENTS {
            return Err(UiError::TooManyElements);
        }
        let order = self.next_plugin_order;
        self.next_plugin_order = self.next_plugin_order.wrapping_add(1);
        let element_id = PLUGIN_ELEMENT_ID_BASE.wrapping_add(order);
        let class = kind.class();
        let mut desc = ElementDesc {
            element_id,
            element_type: 0,
            read_order: order,
            ..ElementDesc::default()
        };
        desc.block.rect.width = INITIAL_SIZE;
        desc.block.rect.height = INITIAL_SIZE;
        desc.block.has_position = true;
        let mut e = Element::new(element_id, class, Arc::new(desc));
        e.created = true;
        e.visible = false;
        e.accepts_double_click = true;
        e.mouse_visible = true;
        e.root_panel = kind == ControlKind::Window;
        e.plugin = Some(Box::new(PluginData {
            kind,
            apartment,
            mouse_visible: true,
            enabled: true,
            blend_mode: 2,
            back_blend_mode: 2,
            ref_r: IBox::from_xywh(0, 0, INITIAL_SIZE, INITIAL_SIZE),
            ..PluginData::default()
        }));
        let k = self.insert_element(e);
        if kind == ControlKind::Window {
            self.set_property(k, self.v.pids.activatable, PropertyValue::Bool(true));
            if let Err(err) = self.attach_raw(k, self.screen) {
                self.destroy(k);
                return Err(err);
            }
        }
        Ok(k)
    }

    /// Destroys a plugin control and everything below it.
    pub fn destroy_control(&mut self, k: ElementKey) {
        if self.is_control(k) {
            self.destroy(k);
        }
    }

    /// Destroys every control of an apartment (unload, `lua-plugin-api.md` §6.3).
    pub fn destroy_apartment(&mut self, apartment: u32) {
        let keys: Vec<ElementKey> = self
            .arena
            .keys()
            .into_iter()
            .filter(|k| self.pd(*k).is_some_and(|p| p.apartment == apartment))
            .collect();
        for k in keys {
            self.destroy(k);
        }
    }

    /// `SetParent` (§13.1): `None` detaches (a window goes back to the screen root).
    ///
    /// # Errors
    /// [`UiError::Cycle`] when the parent is the control or below it, [`UiError::TooDeep`].
    pub fn control_set_parent(
        &mut self,
        k: ElementKey,
        parent: Option<ElementKey>,
    ) -> Result<(), UiError> {
        let is_window = self.pd(k).is_some_and(|p| p.kind == ControlKind::Window);
        match parent {
            Some(p) => {
                if p == k {
                    return Err(UiError::Cycle);
                }
                self.attach_raw(k, p)?;
            }
            None => {
                if is_window {
                    self.attach_raw(k, self.screen)?;
                } else {
                    self.detach_raw(k);
                }
            }
        }
        self.capture_refs(k);
        self.attachment_immediate(k);
        Ok(())
    }

    /// `GetParent`: the parent control (`None` for a top-level window or a detached control).
    #[must_use]
    pub fn control_parent(&self, k: ElementKey) -> Option<ElementKey> {
        self.parent(k).filter(|p| *p != self.screen)
    }

    /// The child controls (`GetControls`), in sibling order.
    #[must_use]
    pub fn control_children(&self, k: ElementKey) -> Vec<ElementKey> {
        self.children(k)
            .into_iter()
            .filter(|c| self.is_control(*c))
            .collect()
    }

    // ----- geometry ----------------------------------------------------------------------

    /// `GetPosition` relative to the parent (a window: the screen).
    #[must_use]
    pub fn control_position(&self, k: ElementKey) -> (i32, i32) {
        self.layout_box(k).map_or((0, 0), |b| (b.l, b.t))
    }

    /// `GetSize`.
    #[must_use]
    pub fn control_size(&self, k: ElementKey) -> (i32, i32) {
        self.layout_box(k).map_or((0, 0), |b| (b.w(), b.h()))
    }

    /// `SetPosition` (§13.1; `PositionChanged` when the position changed, §9.6).
    pub fn control_set_position(&mut self, k: ElementKey, x: i32, y: i32) {
        if !self.is_control(k) {
            return;
        }
        self.move_to(k, x, y);
        self.capture_refs(k);
    }

    /// The size limits of §13.15 applied to a requested size.
    fn apply_limits(&self, k: ElementKey, w: i32, h: i32) -> (i32, i32) {
        let Some(p) = self.pd(k) else { return (w, h) };
        let [minw, minh, maxw, maxh] = p.limits;
        let clamp = |v: i32, min: Option<i32>, max: Option<i32>| {
            let mut v = v;
            if let Some(m) = max
                && v > m
            {
                v = m;
            }
            if let Some(m) = min
                && v < m
            {
                v = m;
            }
            v
        };
        (clamp(w, minw, maxw), clamp(h, minh, maxh))
    }

    /// `SetSize` (§13.15: limits per axis, maximum then minimum; `SizeChanged` only when the
    /// size changed; attachments re-captured and the immediate layout run, §13.16).
    pub fn control_set_size(&mut self, k: ElementKey, w: i32, h: i32) {
        if !self.is_control(k) {
            return;
        }
        let (w, h) = self.apply_limits(k, w, h);
        let surface_stretch = self.owns_surface(k) && self.surface_mode(k) == object_mode::STRETCH;
        if surface_stretch {
            // A Stretch surface keeps its layout box; the scale changes (UB §9.2.1).
            self.resize(k, w, h);
        } else {
            let b = self.layout_box(k).unwrap_or_default();
            self.set_layout_box(k, IBox::from_xywh(b.l, b.t, w, h));
        }
        let max = self.painter.limits.max_surface_dim as i32;
        if w > max || h > max {
            self.emit(crate::events::UiEvent::Warning {
                what: format!("control larger than the largest texture ({w} x {h})"),
            });
        }
        self.capture_refs(k);
        self.attachment_immediate(k);
    }

    /// Stores size limits (`[min width, min height, max width, max height]`, `None` = not
    /// set). Setting a limit does not resize the control (§13.15).
    pub fn control_set_limits(&mut self, k: ElementKey, limits: [Option<i32>; 4]) {
        if let Some(p) = self.pd_mut(k) {
            p.limits = limits;
        }
    }

    /// The stored limits.
    #[must_use]
    pub fn control_limits(&self, k: ElementKey) -> [Option<i32>; 4] {
        self.pd(k).map_or([None; 4], |p| p.limits)
    }

    /// `AttachEdges` (§13.16): stores the four types, captures the references and runs the
    /// layout step once.
    pub fn control_attach_edges(&mut self, k: ElementKey, edges: [i32; 4]) {
        if let Some(p) = self.pd_mut(k) {
            p.attach = edges;
            p.attached = true;
        }
        self.capture_refs(k);
        self.attachment_immediate(k);
    }

    /// `GetEdgeAttachments`.
    #[must_use]
    pub fn control_edges(&self, k: ElementKey) -> [i32; 4] {
        self.pd(k).map_or([0; 4], |p| p.attach)
    }

    /// Captures the references R and Q (§13.16) when the control has a parent and has
    /// attachments.
    fn capture_refs(&mut self, k: ElementKey) {
        let Some(parent) = self.parent(k) else { return };
        let attached = self.pd(k).is_some_and(|p| p.attached);
        if !attached {
            return;
        }
        let b = self.layout_box(k).unwrap_or_default();
        let pb = self.layout_box(parent).unwrap_or_default();
        if let Some(p) = self.pd_mut(k) {
            p.ref_r = b;
            p.ref_q = Some((pb.w(), pb.h()));
        }
    }

    /// The immediate layout after `AttachEdges`, a plugin resize or a re-parent: the
    /// control's own layout step, with `SizeChanged` suppressed for it and below.
    fn attachment_immediate(&mut self, k: ElementKey) {
        if !self.pd(k).is_some_and(|p| p.attached) || self.parent(k).is_none() {
            return;
        }
        let prev = self.suppress_size;
        self.suppress_size = true;
        self.attachment_layout(k, true);
        self.suppress_size = prev;
    }

    /// The layout step of `lua-plugin-api.md` §13.16 for a plugin control.
    pub(crate) fn attachment_layout(&mut self, k: ElementKey, _immediate: bool) {
        let Some(p) = self.pd(k).cloned() else { return };
        let Some(parent) = self.parent(k) else { return };
        let cur = self.layout_box(k).unwrap_or_default();
        if p.attach == [0; 4] && !(cur.w() == 0 && cur.h() == 0) {
            return;
        }
        let pb = self.layout_box(parent).unwrap_or_default();
        let q = p.ref_q.unwrap_or_else(|| {
            self.pd(parent)
                .map_or((pb.w(), pb.h()), |pp| (pp.ref_r.w(), pp.ref_r.h()))
        });
        let nb = attachment_step(p.attach, p.ref_r, q, cur, pb.w(), pb.h());
        let (w, h) = self.apply_limits(k, nb.w(), nb.h());
        let notify = !self.suppress_size;
        self.set_layout_box_with(k, IBox::from_xywh(nb.l, nb.t, w, h), notify);
    }

    // ----- tree order, visibility, input flags -------------------------------------------

    /// `SetZOrder` (§13.14): a higher `ZOrder` is above a lower one, so the element's
    /// z-level (lower in front) is −`n`.
    pub fn control_set_z_order(&mut self, k: ElementKey, n: i32) {
        if let Some(p) = self.pd_mut(k) {
            p.z_order = n;
        }
        self.set_z(k, n.checked_neg().unwrap_or(i32::MAX));
    }

    /// `GetZOrder`.
    #[must_use]
    pub fn control_z_order(&self, k: ElementKey) -> i32 {
        self.pd(k).map_or(0, |p| p.z_order)
    }

    /// `SetVisible` (§13.1; `VisibleChanged` when the own flag changed). Hiding the active
    /// window deactivates it; showing does not activate (§13.14 Tessera rule). A rotation
    /// the window remembered from its own surface is restored on show (§13.20 step 3).
    pub fn control_set_visible(&mut self, k: ElementKey, v: bool) {
        if !self.is_control(k) {
            return;
        }
        let was = self.is_visible(k);
        self.set_shown(k, v);
        if was != v {
            let r = self.pd(k).map_or([0.0; 3], |p| p.remembered_rotation);
            if let Some(e) = self.el_mut(k) {
                e.rotation = if v { r } else { [0.0; 3] };
            }
        }
    }

    /// `IsDisplayed`: the control and every ancestor visible, below the screen root.
    #[must_use]
    pub fn control_is_displayed(&self, k: ElementKey) -> bool {
        self.is_displayed(k)
    }

    /// `SetMouseVisible` (§13.11): a direct write of the mouse-visible flag.
    pub fn control_set_mouse_visible(&mut self, k: ElementKey, v: bool) {
        if let Some(p) = self.pd_mut(k) {
            p.mouse_visible = v;
        }
        self.set_mouse_visible_flag(k, v);
    }

    /// `SetEnabled` (the plugin layer keeps the flag; `Button.Click` needs it).
    pub fn control_set_enabled(&mut self, k: ElementKey, v: bool) {
        if let Some(p) = self.pd_mut(k) {
            p.enabled = v;
        }
    }

    /// `SetAllowDrop`.
    pub fn control_set_allow_drop(&mut self, k: ElementKey, v: bool) {
        if let Some(p) = self.pd_mut(k) {
            p.allow_drop = v;
        }
        self.set_property(k, self.v.pids.container, PropertyValue::Bool(v));
    }

    /// `SetWantsUpdates` (§9.4): subscribes the control to the `Update` broadcast.
    pub fn control_set_wants_updates(&mut self, k: ElementKey, v: bool) {
        if let Some(p) = self.pd_mut(k) {
            p.wants_updates = v;
        }
        self.subs.updates.retain(|x| *x != k);
        if v && self.is_control(k) {
            self.subs.updates.push(k);
        }
    }

    /// `SetWantsKeyEvents` (§13.13).
    pub fn control_set_wants_keys(&mut self, k: ElementKey, v: bool) {
        if let Some(p) = self.pd_mut(k) {
            p.wants_keys = v;
        }
        self.subs.keys.retain(|x| *x != k);
        if v && self.is_control(k) {
            self.subs.keys.push(k);
        }
    }

    /// The `Update` broadcast's subscribers, in subscription order (decision 0014 D8 step 7
    /// item 3: the plugin host calls their `Update` handlers).
    #[must_use]
    pub fn update_subscribers(&self) -> Vec<ElementKey> {
        self.subs.updates.clone()
    }

    /// The key broadcast's subscribers, in subscription order.
    #[must_use]
    pub fn key_subscribers(&self) -> Vec<ElementKey> {
        self.subs.keys.clone()
    }

    // ----- look (§13.17–§13.19) ----------------------------------------------------------

    /// `SetBackground` (§13.8, §13.17): removes the base image, then sets the new one with
    /// the stored blend operation. A DID is drawn unskinned until the next skin switch.
    pub fn control_set_background(&mut self, k: ElementKey, bg: Background) {
        let mode = self.pd(k).map_or(2, |p| p.blend_mode);
        self.clear_base(k);
        match bg {
            Background::None => {}
            Background::Did(d) => {
                if d != 0 {
                    self.set_base(k, ImageRef::Did(d), d, true, mode);
                }
            }
            Background::File(f) => {
                if !f.is_empty() {
                    self.set_base(k, ImageRef::File(f), 0, false, mode);
                }
            }
        }
        let img = self.base_image(k).map(|b| b.0);
        if let Some(p) = self.pd_mut(k) {
            p.background = img;
        }
    }

    /// `SetBlendMode` (§13.17).
    ///
    /// # Errors
    /// [`UiError::BadBlendMode`] for a value above 8.
    pub fn control_set_blend_mode(&mut self, k: ElementKey, n: u32) -> Result<(), UiError> {
        if n > 8 {
            return Err(UiError::BadBlendMode(n));
        }
        if let Some(p) = self.pd_mut(k) {
            p.blend_mode = n;
        }
        if let Some(e) = self.el_mut(k)
            && let Some(b) = e.base.as_mut()
        {
            b.draw_mode = n;
            e.dirty = true;
        }
        self.painter.touched = true;
        Ok(())
    }

    /// `GetBlendMode`.
    #[must_use]
    pub fn control_blend_mode(&self, k: ElementKey) -> u32 {
        self.pd(k).map_or(2, |p| p.blend_mode)
    }

    /// `SetBackColor(c)` with `c` = (A, R, G, B), or `None` (§13.17).
    pub fn control_set_back_colour(&mut self, k: ElementKey, c: Option<[f32; 4]>) {
        let Some(p) = self.pd_mut(k) else { return };
        match c {
            Some(col) => {
                p.back_colour = col;
                p.back_colour_set = true;
                let mode = p.back_blend_mode;
                self.set_overlay_colour(k, Some((col, mode)));
            }
            None => {
                p.back_colour = [0.0; 4];
                p.back_colour_set = false;
                self.set_overlay_colour(k, None);
            }
        }
    }

    /// `GetBackColor` as (A, R, G, B).
    #[must_use]
    pub fn control_back_colour(&self, k: ElementKey) -> [f32; 4] {
        self.pd(k).map_or([0.0; 4], |p| p.back_colour)
    }

    /// `SetBackColorBlendMode` (§13.17): re-creates the overlay fill with the stored colour,
    /// even when no back colour was set.
    ///
    /// # Errors
    /// [`UiError::BadBlendMode`] for a value above 8.
    pub fn control_set_back_colour_blend_mode(
        &mut self,
        k: ElementKey,
        n: u32,
    ) -> Result<(), UiError> {
        if n > 8 {
            return Err(UiError::BadBlendMode(n));
        }
        let Some(p) = self.pd_mut(k) else {
            return Ok(());
        };
        p.back_blend_mode = n;
        let col = p.back_colour;
        self.set_overlay_colour(k, Some((col, n)));
        Ok(())
    }

    /// `GetBackColorBlendMode`.
    #[must_use]
    pub fn control_back_colour_blend_mode(&self, k: ElementKey) -> u32 {
        self.pd(k).map_or(2, |p| p.back_blend_mode)
    }

    fn set_overlay_colour(&mut self, k: ElementKey, c: Option<([f32; 4], u32)>) {
        let ov = c.map(|(col, mode)| crate::tree::Overlay {
            image: None,
            colour: plugin_colour(col[0], col[1], col[2], col[3]),
            draw_mode: mode,
        });
        if let Some(e) = self.el_mut(k)
            && e.overlay != ov
        {
            e.overlay = ov;
            e.dirty = true;
        }
        self.painter.touched = true;
    }

    /// `SetStretchMode(n)` (§13.18): stores `n` as `UICore_Element_ObjectMode`.
    pub fn control_set_stretch_mode(&mut self, k: ElementKey, n: i32) {
        if let Some(p) = self.pd_mut(k) {
            p.stretch_set = true;
        }
        self.set_property(k, self.v.pids.object_mode, PropertyValue::Enum(n as u32));
        self.input.hover_dirty = true;
    }

    /// `GetStretchMode` (§13.18): the property, else 0.
    #[must_use]
    pub fn control_stretch_mode(&self, k: ElementKey) -> i32 {
        self.object_mode(k).map_or(0, |m| m as i32)
    }

    /// `SetOpacity` (§13.19): `UICore_Element_ObjectOpacity`, not clamped.
    pub fn control_set_opacity(&mut self, k: ElementKey, o: f32) {
        self.set_property(k, self.v.pids.object_opacity, PropertyValue::Float(o));
    }

    /// `GetOpacity`.
    #[must_use]
    pub fn control_opacity(&self, k: ElementKey) -> f32 {
        self.prop(k, self.v.pids.object_opacity).float()
    }

    /// `Window:SetRotation` (§13.20 steps 1–3; decision 0015 §2.3). Returns whether a
    /// target surface took the angles (only then `GetRotation` changes).
    pub fn window_set_rotation(&mut self, k: ElementKey, angles: [f32; 3]) -> bool {
        let angles = angles.map(|a| if a.is_finite() { a } else { 0.0 });
        // Step 2.
        if !self.owns_surface(k) {
            let nonzero_set = self.pd(k).is_some_and(|p| p.stretch_set)
                && self.object_mode(k).is_some_and(|m| m != 0);
            if !nonzero_set {
                self.set_property(
                    k,
                    self.v.pids.object_mode,
                    PropertyValue::Enum(object_mode::STRETCH),
                );
            }
        }
        // Step 3.
        let mut target = None;
        let mut cur = Some(k);
        while let Some(c) = cur {
            if self.owns_surface(c) {
                target = Some(c);
                break;
            }
            cur = self.parent(c);
        }
        let Some(t) = target else { return false };
        let own = t == k;
        let held = self.el(t).map_or([0.0; 3], |e| e.rotation);
        let unchanged = held
            .iter()
            .zip(angles.iter())
            .all(|(a, b)| (a - b).abs() <= 0.0002);
        if !unchanged {
            if let Some(e) = self.el_mut(t) {
                e.rotation = angles;
            }
            self.painter.touched = true;
        }
        if let Some(p) = self.pd_mut(k) {
            p.rotation = angles;
            p.rotation_set = true;
            if own {
                p.remembered_rotation = angles;
            }
        }
        true
    }

    /// `Window:GetRotation`.
    #[must_use]
    pub fn window_rotation(&self, k: ElementKey) -> [f32; 3] {
        self.pd(k).map_or([0.0; 3], |p| p.rotation)
    }

    /// `Window:SetScale(f)` (§13.21 rules 4–5): `f` ≤ 0 is an error; the window is marked,
    /// `f` becomes its own scale and the panel-scale rule applies.
    ///
    /// # Errors
    /// [`UiError::BadScale`] for `f` ≤ 0 or not finite.
    pub fn window_set_scale(&mut self, k: ElementKey, f: f32, global: bool) -> Result<(), UiError> {
        if f.is_nan() || f <= 0.0 || !f.is_finite() {
            return Err(UiError::BadScale);
        }
        if self.control_parent(k).is_some() {
            return Ok(());
        }
        let s = if global {
            f * self.settings.scale_global
        } else {
            f
        };
        self.scale_panel(k, s, true);
        Ok(())
    }

    /// `PointToScreen`.
    #[must_use]
    pub fn point_to_screen(&self, k: ElementKey, x: i32, y: i32) -> (i32, i32) {
        let b = self.displayed_box(k);
        (b.l.saturating_add(x), b.t.saturating_add(y))
    }

    /// `PointToClient`.
    #[must_use]
    pub fn point_to_client(&self, k: ElementKey, x: i32, y: i32) -> (i32, i32) {
        let b = self.displayed_box(k);
        (x.saturating_sub(b.l), y.saturating_sub(b.t))
    }

    /// `GetMousePosition` relative to the control.
    #[must_use]
    pub fn control_mouse_position(&self, k: ElementKey) -> (i32, i32) {
        self.pointer_rel(k)
    }

    /// `Control:Focus()` (§13.13): remembered on its window; moves focus now when the window
    /// is active.
    pub fn control_focus(&mut self, k: ElementKey) {
        let Some(w) = self.root_panel_of(k) else {
            return;
        };
        if let Some(p) = self.pd_mut(w) {
            p.focus_control = Some(k);
        }
        if self.focus.active == Some(w) {
            self.set_focus(Some(k));
        }
    }

    /// `HasFocus`.
    #[must_use]
    pub fn control_has_focus(&self, k: ElementKey) -> bool {
        self.focus.focus == Some(k)
    }
}
