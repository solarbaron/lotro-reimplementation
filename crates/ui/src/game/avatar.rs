// SPDX-License-Identifier: GPL-3.0-or-later
//! Viewports as hosts of external content (`render.md` §6.10, `ui-widgets.md` §12 Tessera
//! rule) and the paperdoll's `AvatarInfo` (`character-panels.md` §3.2).
//!
//! A `TargetViewport` (a game type) or an engine Viewport shows a 3D scene that the client
//! renders into an offscreen target of the element's layout-box size, in a linear 8-bit
//! RGBA format with premultiplied colour, cleared to transparent black each frame it is
//! drawn; the UI composes it as an external-content entry onto the element's displayed
//! box ([`crate::Ui::set_external_content`]). [`Viewports`] finds the hosts below a panel,
//! registers their content ids and tells the client each target's size.
//!
//! [`AvatarInfo`] adds the paperdoll's turning (Tessera rule of `character-panels.md`
//! §3.2: 15° per repeat of a rotate button, 0.5° per pixel of drag in the viewport).

use tessera_formats::property::PropertyValue;

use crate::events::UiEvent;
use crate::geom::IBox;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::charvocab::CharVocab;
use super::{find_below, subtree};

/// Turn per repeat of a rotate button, degrees (`character-panels.md` §3.2 Tessera rule).
pub const ROTATE_STEP_DEGREES: f32 = 15.0;

/// Turn per pixel of drag in the viewport, degrees (§3.2 Tessera rule).
pub const DRAG_DEGREES_PER_PIXEL: f32 = 0.5;

/// Most viewport hosts one controller keeps (Tessera cap).
pub const MAX_VIEWPORTS: usize = 256;

/// One viewport that hosts external content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportHost {
    /// The element.
    pub element: ElementKey,
    /// The external-content id the client draws into.
    pub id: u64,
    /// `UICore_Viewport_uiscene` (a 0x21 DID; 0 when absent).
    pub scene: u32,
    /// `UICore_Viewport_camera` (an enum key; 0 when absent).
    pub camera: u32,
}

/// What the client renders this frame for one host (`render.md` §6.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportTarget {
    /// The host.
    pub host: ViewportHost,
    /// The target size: the element's layout box.
    pub size: (i32, i32),
    /// The element's displayed box on the screen.
    pub displayed: IBox,
    /// Whether the element is displayed (a hidden host needs no drawing).
    pub displayed_now: bool,
}

/// The viewport hosts.
#[derive(Debug, Clone)]
pub struct Viewports {
    vocab: CharVocab,
    hosts: Vec<ViewportHost>,
    next_id: u64,
}

impl Viewports {
    /// A controller whose content ids start at `first_id` (the client's id space).
    #[must_use]
    pub fn new(vocab: CharVocab, first_id: u64) -> Self {
        Self {
            vocab,
            hosts: Vec::new(),
            next_id: first_id,
        }
    }

    /// The hosts.
    #[must_use]
    pub fn hosts(&self) -> &[ViewportHost] {
        &self.hosts
    }

    /// Registers every viewport at or below `root` (game type `TargetViewport`, or the
    /// engine Viewport class) as an external-content host with a new id. Returns the new
    /// hosts.
    pub fn attach(&mut self, ui: &mut Ui, root: ElementKey) -> Vec<ViewportHost> {
        let t = self.vocab.types.target_viewport;
        let mut new = Vec::new();
        for k in subtree(ui, root) {
            let is_vp = ui.element_type(k) == Some(t) || ui.class(k) == Some(WidgetClass::Viewport);
            if !is_vp || self.hosts.iter().any(|h| h.element == k) {
                continue;
            }
            if self.hosts.len() >= MAX_VIEWPORTS {
                break;
            }
            if let Some(h) = self.attach_one(ui, k) {
                new.push(h);
            }
        }
        new
    }

    /// Registers one element as a host.
    pub fn attach_one(&mut self, ui: &mut Ui, k: ElementKey) -> Option<ViewportHost> {
        if !ui.exists(k) || self.hosts.len() >= MAX_VIEWPORTS {
            return None;
        }
        if let Some(h) = self.hosts.iter().find(|h| h.element == k) {
            return Some(*h);
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        ui.set_external_content(k, Some(id));
        let p = self.vocab.pids;
        let h = ViewportHost {
            element: k,
            id,
            scene: ui.prop(k, p.viewport_scene).key(),
            camera: ui.prop(k, p.viewport_camera).key(),
        };
        self.hosts.push(h);
        Some(h)
    }

    /// Removes a host and its external content.
    pub fn detach(&mut self, ui: &mut Ui, k: ElementKey) {
        if self.hosts.iter().any(|h| h.element == k) {
            ui.set_external_content(k, None);
        }
        self.hosts.retain(|h| h.element != k);
    }

    /// Forgets hosts whose elements were destroyed.
    pub fn prune(&mut self, ui: &Ui) {
        self.hosts.retain(|h| ui.exists(h.element));
    }

    /// What to render this frame: one target per live host.
    #[must_use]
    pub fn targets(&self, ui: &Ui) -> Vec<ViewportTarget> {
        self.hosts
            .iter()
            .filter(|h| ui.exists(h.element))
            .map(|h| {
                let b = ui.layout_box(h.element).unwrap_or_default();
                ViewportTarget {
                    host: *h,
                    size: (b.w().max(0), b.h().max(0)),
                    displayed: ui.displayed_box(h.element),
                    displayed_now: ui.is_displayed(h.element),
                }
            })
            .collect()
    }
}

/// The paperdoll (`AvatarInfo`, `character-panels.md` §3.2): its parts and the figure's
/// turn.
#[derive(Debug, Clone, PartialEq)]
pub struct AvatarInfo {
    /// The `AvatarInfo` element.
    pub element: ElementKey,
    /// `UIComponent_AvatarInfo_Viewport`.
    pub viewport: Option<ElementKey>,
    /// `UIComponent_AvatarInfo_LeftButton`.
    pub left: Option<ElementKey>,
    /// `UIComponent_AvatarInfo_RightButton`.
    pub right: Option<ElementKey>,
    /// `UIComponent_AvatarInfo_SlotField`.
    pub slot_field: Option<ElementKey>,
    /// `UIComponent_AvatarInfo_EnableMouseDragRotate`.
    pub drag_rotate: bool,
    /// The figure's turn about its vertical axis, degrees in [0, 360); the right button
    /// and a drag to the right increase it.
    pub yaw: f32,
    drag_from: Option<i32>,
}

impl AvatarInfo {
    /// Finds the parts the element names by property, registers the game-code handlers
    /// the turning needs (rotate buttons' `Button_hot_click` and `Button_click`, the
    /// viewport's `Element_mouse_press`), and starts at yaw 0.
    pub fn attach(ui: &mut Ui, vocab: &CharVocab, element: ElementKey) -> Self {
        let p = vocab.pids;
        let part = |ui: &Ui, pid: u32| {
            let id = ui.prop(element, pid).key();
            (id != 0).then(|| find_below(ui, element, id)).flatten()
        };
        let viewport = part(ui, p.avatar_viewport);
        let left = part(ui, p.avatar_left);
        let right = part(ui, p.avatar_right);
        let slot_field = part(ui, p.avatar_slot_field);
        let drag_rotate = ui.prop(element, p.avatar_drag_rotate).bool();
        let msgs = ui.vocab().msgs;
        for b in [left, right].into_iter().flatten() {
            if let Some(id) = ui.element_id(b) {
                ui.register_handler(id, msgs.button_hot_click);
                ui.register_handler(id, msgs.button_click);
            }
        }
        if drag_rotate
            && let Some(v) = viewport
            && let Some(id) = ui.element_id(v)
        {
            // Also makes the viewport mouse-visible (`ui-behaviour.md` §4.3 item 2).
            ui.register_handler(id, msgs.mouse_press);
        }
        Self {
            element,
            viewport,
            left,
            right,
            slot_field,
            drag_rotate,
            yaw: 0.0,
            drag_from: None,
        }
    }

    fn turn(&mut self, degrees: f32) {
        if degrees.is_finite() {
            self.yaw = (self.yaw + degrees).rem_euclid(360.0);
        }
    }

    /// Handles a UI event: a rotate button's repeat or click turns the figure by
    /// [`ROTATE_STEP_DEGREES`]; a press in the viewport starts a drag turn. Returns
    /// whether the yaw changed or a drag started.
    pub fn handle_event(&mut self, ui: &Ui, ev: &UiEvent) -> bool {
        let UiEvent::Game { message } = ev else {
            return false;
        };
        let msgs = ui.vocab().msgs;
        let src = Some(message.source);
        if message.msg == msgs.button_hot_click || message.msg == msgs.button_click {
            if src == self.left {
                self.turn(-ROTATE_STEP_DEGREES);
                return true;
            }
            if src == self.right {
                self.turn(ROTATE_STEP_DEGREES);
                return true;
            }
            return false;
        }
        if message.msg == msgs.mouse_press && self.drag_rotate && src == self.viewport {
            let left = message
                .arg
                .command
                .as_ref()
                .is_some_and(|c| c.mouse_button() == 1);
            if left {
                self.drag_from = ui.pointer().map(|p| p.0);
                return true;
            }
        }
        false
    }

    /// The drag turn: while the viewport holds the capture, every pixel of horizontal
    /// pointer movement turns the figure by [`DRAG_DEGREES_PER_PIXEL`]. Call after pointer
    /// moves (or once per frame). Returns whether the yaw changed.
    pub fn update(&mut self, ui: &Ui) -> bool {
        let Some(from) = self.drag_from else {
            return false;
        };
        if self.viewport.is_none() || ui.captured() != self.viewport {
            self.drag_from = None;
            return false;
        }
        let Some((x, _)) = ui.pointer() else {
            return false;
        };
        if x == from {
            return false;
        }
        self.turn((i64::from(x) - i64::from(from)) as f32 * DRAG_DEGREES_PER_PIXEL);
        self.drag_from = Some(x);
        true
    }

    /// Whether a drag turn is running.
    #[must_use]
    pub fn dragging(&self) -> bool {
        self.drag_from.is_some()
    }
}

/// Sets a viewport's scene property (for a viewport whose scene the client chooses, such
/// as the creation screens' figure, `character-screens.md` §2.1).
pub fn set_viewport_scene(ui: &mut Ui, vocab: &CharVocab, k: ElementKey, scene: u32) {
    ui.set_property(
        k,
        vocab.pids.viewport_scene,
        PropertyValue::DidRef(tessera_formats::did::Did(scene)),
    );
}
