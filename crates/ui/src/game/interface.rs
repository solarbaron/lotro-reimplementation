// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's own **Interface** window (`ui-scaling.md` §7.2): the required UI-scale slider
//! and Auto toggle, the safe-aspect choice, and the optional text-size, art-filter and
//! per-window-scale controls.
//!
//! The original's Options panel is built by game code whose templates are not named
//! (`ui-layout.md` §17.5 OPEN (d)), so the scale controls live in this window instead
//! (§7.2). It is a Tessera-built control window: a code-made root panel of game-styled
//! controls (a window frame, labels, a slider-mode scroll bar of `ui-widgets.md` §6, and
//! buttons), with the install's art when a skin provides it and Tessera's neutral fallback
//! art ([`FallbackArt`], §7.2 "Tessera's neutral art when the data are missing") otherwise.
//! It is built with the same [`super::npc::kit`] the P3/P5 fallback windows use, so the one
//! layout and paint path draws it.
//!
//! This module is presentation only: it holds no settings and no scale state. The client
//! ([`crate::Ui`] host) fills an [`InterfaceModel`] from its scaling state and settings,
//! opens the window with it, feeds every UI event through [`InterfaceWindow::handle_event`],
//! and applies the returned [`InterfaceAction`] to its scaling state and the `[UI]` keys
//! (`ui-scaling.md` §4.4, §7.1), then calls [`InterfaceWindow::refresh`] with a fresh model.
//! The window never sends anything and never reads the install directly.

use tessera_formats::property::PropertyValue;

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::GameError;
use super::npc::kit::{ClickKind, FNode, FallbackArt, Listen, click_of, colours};
use super::npcvocab::NpcVocab;

// ----- stop ladders (`ui-scaling.md` §4.1, §7.5) ----------------------------------------

/// Number of UI-scale stops (`ui-scaling.md` §4.1): 0.50-4.00 in steps of 0.05 (71 values).
pub const SCALE_STOPS: u8 = 71;
/// Number of text-size stops (`ui-scaling.md` §7.5): 0.80-1.50 in steps of 0.05 (15 values).
pub const TEXT_STOPS: u8 = 15;

/// The UI scale a slider stop stands for (`ui-scaling.md` §4.1): 0.50 + 0.05 × `stop`,
/// clamped to the range.
#[must_use]
pub fn scale_for_stop(stop: u8) -> f32 {
    let stop = stop.min(SCALE_STOPS - 1);
    0.50 + 0.05 * f32::from(stop)
}

/// The slider stop nearest the UI scale `s` (a value exactly halfway rounds up), clamped to
/// 0 … 70 (`ui-scaling.md` §4.1).
#[must_use]
pub fn stop_for_scale(s: f32) -> u8 {
    stop_nearest(s, 0.50, SCALE_STOPS)
}

/// The text-size factor a slider stop stands for (`ui-scaling.md` §7.5): 0.80 + 0.05 × `stop`.
#[must_use]
pub fn text_for_stop(stop: u8) -> f32 {
    let stop = stop.min(TEXT_STOPS - 1);
    0.80 + 0.05 * f32::from(stop)
}

/// The text-size slider stop nearest the factor `x`, clamped to 0 … 14 (`ui-scaling.md` §7.5).
#[must_use]
pub fn stop_for_text(x: f32) -> u8 {
    stop_nearest(x, 0.80, TEXT_STOPS)
}

/// The stop nearest `v` on a ladder of `count` stops starting at `base` with step 0.05 (a
/// value exactly halfway rounds up), clamped to 0 … `count` − 1. A non-finite `v` is 0.
fn stop_nearest(v: f32, base: f32, count: u8) -> u8 {
    if !v.is_finite() {
        return 0;
    }
    // In f64 so the decimal step divides cleanly; +0.5 rounds to nearest, halves up. The
    // 1e-5 index bias absorbs the error of an `f32` decimal after the ×20 of the 0.05 step,
    // so an exact half such as 1.025 rounds up (it is far smaller than one step).
    let i = ((f64::from(v) - f64::from(base)) / 0.05 + 0.5 + 1e-5).floor();
    let i = i.clamp(0.0, f64::from(count - 1));
    i as u8
}

// ----- choices (`ui-scaling.md` §5.1, §7.6) ---------------------------------------------

/// A safe-aspect choice of the Interface window's choice group (`ui-scaling.md` §5.1, §7.2
/// row 6). A stored custom ratio matches none of these, so no button is marked (§7.2 row 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AspectChoice {
    /// 4:3.
    R4_3,
    /// 16:10.
    R16_10,
    /// 16:9.
    R16_9,
    /// 21:9 (the default).
    R21_9,
    /// No limit (the safe area is the full screen).
    Full,
}

impl AspectChoice {
    /// The five choices in button order.
    pub const ALL: [Self; 5] = [
        Self::R4_3,
        Self::R16_10,
        Self::R16_9,
        Self::R21_9,
        Self::Full,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::R4_3 => "4:3",
            Self::R16_10 => "16:10",
            Self::R16_9 => "16:9",
            Self::R21_9 => "21:9",
            Self::Full => "Full width",
        }
    }
}

/// An art-filter choice (`ui-scaling.md` §7.6, §7.2 row 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtChoice {
    /// Crisp (the default).
    Sharp,
    /// Soft art.
    Smooth,
    /// Pixel art (nearest texel).
    Nearest,
}

impl ArtChoice {
    /// The three choices in button order.
    pub const ALL: [Self; 3] = [Self::Sharp, Self::Smooth, Self::Nearest];

    fn label(self) -> &'static str {
        match self {
            Self::Sharp => "Sharp",
            Self::Smooth => "Smooth",
            Self::Nearest => "Pixel",
        }
    }
}

// ----- the model the client fills -------------------------------------------------------

/// What the Interface window shows (`ui-scaling.md` §7.2 rows). The client fills it from its
/// scaling state and settings; the window never computes it.
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceModel {
    /// The slider stop for the current UI scale *Se* when limited, else *S* (§7.2 rows 2, 4).
    pub scale_stop: u8,
    /// The value label by the slider: `"150 %"`, or `"Auto (150 %)"` while Auto (§7.2 row 2).
    pub scale_label: String,
    /// Whether *S* is **auto** (§7.2 row 3).
    pub auto: bool,
    /// The limit note when the window is too small for *S* (§4.3, §7.2 row 4), else `None`.
    pub limited: Option<String>,
    /// The selected safe aspect, or `None` for a stored custom ratio (§7.2 row 6).
    pub aspect: Option<AspectChoice>,
    /// The text-size slider stop (§7.5, §7.2 row 8).
    pub text_stop: u8,
    /// The text-size value label (`"100 %"`).
    pub text_label: String,
    /// The selected art filter (§7.6, §7.2 row 9).
    pub art: ArtChoice,
}

impl Default for InterfaceModel {
    fn default() -> Self {
        Self {
            scale_stop: stop_for_scale(1.0),
            scale_label: "100 %".to_owned(),
            auto: true,
            limited: None,
            aspect: Some(AspectChoice::R21_9),
            text_stop: stop_for_text(1.0),
            text_label: "100 %".to_owned(),
            art: ArtChoice::Sharp,
        }
    }
}

/// What a UI event in the Interface window asks the client to do (`ui-scaling.md` §4.5,
/// §7.1-§7.6). The client applies it to its scaling state and the `[UI]` keys, live (§4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceAction {
    /// The scale slider moved: set *S* to [`scale_for_stop`] of this stop (§7.2 row 2).
    SetScaleStop(u8),
    /// The Auto toggle: set *S* to auto (`true`) or to the slider's current fixed value
    /// (`false`) (§7.2 row 3).
    SetAuto(bool),
    /// A safe-aspect button: set `ui_safe_aspect` (§5.1, §7.2 row 6).
    SetAspect(AspectChoice),
    /// "Reset panel positions": clear every `[UI.panels]` entry and re-lay the roots
    /// (§7.1, §7.2 row 7).
    ResetPanels,
    /// The text-size slider moved (§7.5, §7.2 row 8).
    SetTextStop(u8),
    /// An art-filter button (§7.6, §7.2 row 9).
    SetArt(ArtChoice),
    /// "Reset window scales": clear every `[UI.group_scale]` entry (§7.4, §7.2 row 10).
    ResetWindowScales,
    /// The close box: close the window.
    Close,
}

// ----- element IDs (Tessera's own range, above the kit's auto range) --------------------

/// Base element ID of the Interface window's parts: above the P3 auto range
/// ([`super::npc::kit`] ends at `0x7EFF_0000`) and clear of [`super::ICON_LAYER_ID`] and the
/// page/option IDs.
const ID_BASE: u32 = 0x7EFF_FD00;
const ID_ROOT: u32 = ID_BASE;
const ID_CLOSE: u32 = ID_BASE + 1;
const ID_SCALE_BAR: u32 = ID_BASE + 2;
const ID_SCALE_VALUE: u32 = ID_BASE + 3;
const ID_AUTO: u32 = ID_BASE + 4;
const ID_LIMITED: u32 = ID_BASE + 5;
const ID_RESET_PANELS: u32 = ID_BASE + 6;
const ID_TEXT_BAR: u32 = ID_BASE + 7;
const ID_TEXT_VALUE: u32 = ID_BASE + 8;
const ID_RESET_SCALES: u32 = ID_BASE + 9;
const ID_ASPECT: u32 = ID_BASE + 0x10;
const ID_ART: u32 = ID_BASE + 0x20;

/// The window's design size, in UI units (a safe-area panel; `ui-scaling.md` §7.2).
const WIN_W: i32 = 380;
const WIN_H: i32 = 430;

// ----- the window -----------------------------------------------------------------------

/// The Interface window controller (`ui-scaling.md` §7.2). Built once and kept open; the
/// client feeds it events and refreshes it after a live change.
#[derive(Debug, Clone)]
pub struct InterfaceWindow {
    /// The root panel.
    pub root: ElementKey,
    close: Option<ElementKey>,
    scale_bar: Option<ElementKey>,
    scale_value: Option<ElementKey>,
    auto: Option<ElementKey>,
    limited: Option<ElementKey>,
    reset_panels: Option<ElementKey>,
    text_bar: Option<ElementKey>,
    text_value: Option<ElementKey>,
    reset_scales: Option<ElementKey>,
    aspect: [Option<ElementKey>; 5],
    art: [Option<ElementKey>; 3],
    /// The model as last shown, so the choice markers and the Auto toggle know their state
    /// without the client passing it in on every event.
    shown: InterfaceModel,
    listen: Listen,
}

impl InterfaceWindow {
    /// Opens the window centred on the current screen, with `art` (empty art draws the
    /// labels and button text on plain fields, §7.2) and `model` as its first state.
    ///
    /// # Errors
    /// [`GameError`] when the UI refuses the root (the element cap).
    pub fn open(ui: &mut Ui, art: &FallbackArt, model: &InterfaceModel) -> Result<Self, GameError> {
        let nv = NpcVocab::default();
        let node = root_node(ui, &nv, art, model);
        let root = super::npc::kit::open_root(ui, &node)?;
        // Centre the window on the screen (a safe-area panel opens at the safe area's centre,
        // `ui-scaling.md` §7.2; the exact safe box is the layout system's, so Tessera's own
        // window centres on the screen at open and then saves its placement on a move).
        let (sw, sh) = ui.screen_size();
        let x = (sw - WIN_W).max(0) / 2;
        let y = (sh - WIN_H).max(0) / 2;
        ui.move_to(root, x, y);
        let f = |ui: &Ui, id: u32| super::find_below(ui, root, id);
        let aspect = AspectChoice::ALL.map(|c| f(ui, ID_ASPECT + aspect_index(c)));
        let art_parts = ArtChoice::ALL.map(|c| f(ui, ID_ART + art_index(c)));
        let mut w = Self {
            root,
            close: f(ui, ID_CLOSE),
            scale_bar: f(ui, ID_SCALE_BAR),
            scale_value: f(ui, ID_SCALE_VALUE),
            auto: f(ui, ID_AUTO),
            limited: f(ui, ID_LIMITED),
            reset_panels: f(ui, ID_RESET_PANELS),
            text_bar: f(ui, ID_TEXT_BAR),
            text_value: f(ui, ID_TEXT_VALUE),
            reset_scales: f(ui, ID_RESET_SCALES),
            aspect,
            art: art_parts,
            shown: model.clone(),
            listen: Listen::default(),
        };
        // Clicks on the buttons; the slider's new-position message on the two bars.
        for b in [w.close, w.auto, w.reset_panels, w.reset_scales]
            .into_iter()
            .chain(w.aspect)
            .chain(w.art)
        {
            w.listen.clicks(ui, b);
        }
        // Each slider-mode bar: the new-position message (the stop index it posts when moved,
        // `ui-widgets.md` §6.5), and a press handler so the bar is mouse-visible and its native
        // slider jump runs on a press (the press event itself the window ignores).
        let new_pos = ui.vocab().msgs.sb_new_position;
        let press = ui.vocab().msgs.mouse_press;
        for bar in [w.scale_bar, w.text_bar] {
            w.listen.on(ui, bar, new_pos);
            w.listen.on(ui, bar, press);
        }
        w.refresh(ui, model);
        Ok(w)
    }

    /// The outcome of a UI event: the scale slider and text slider post a new-position
    /// message (the stop index), the toggle and buttons click. `None` for an event the
    /// window does not own.
    pub fn handle_event(&mut self, ui: &Ui, ev: &UiEvent) -> Option<InterfaceAction> {
        // The slider-mode scroll bars post `Scrollbar_new_position` with the stop index in
        // `value2` (`ui-widgets.md` §6.5).
        if let UiEvent::Game { message } = ev
            && message.msg == ui.vocab().msgs.sb_new_position
        {
            let stop = message.arg.value2;
            if Some(message.source) == self.scale_bar && stop >= 0 {
                let stop = clamp_stop(stop, SCALE_STOPS);
                return Some(InterfaceAction::SetScaleStop(stop));
            }
            if Some(message.source) == self.text_bar && stop >= 0 {
                let stop = clamp_stop(stop, TEXT_STOPS);
                return Some(InterfaceAction::SetTextStop(stop));
            }
            return None;
        }
        // Button and left clicks.
        let (src, kind) = click_of(ui, ev)?;
        if !matches!(kind, ClickKind::Button | ClickKind::Left) {
            return None;
        }
        let hit = |k: Option<ElementKey>| {
            k.is_some_and(|k| ui.exists(src) && ui.is_ancestor_or_self(k, src))
        };
        if hit(self.close) {
            return Some(InterfaceAction::Close);
        }
        if hit(self.auto) {
            return Some(InterfaceAction::SetAuto(!self.shown.auto));
        }
        if hit(self.reset_panels) {
            return Some(InterfaceAction::ResetPanels);
        }
        if hit(self.reset_scales) {
            return Some(InterfaceAction::ResetWindowScales);
        }
        for (i, b) in self.aspect.iter().enumerate() {
            if hit(*b) {
                return Some(InterfaceAction::SetAspect(AspectChoice::ALL[i]));
            }
        }
        for (i, b) in self.art.iter().enumerate() {
            if hit(*b) {
                return Some(InterfaceAction::SetArt(ArtChoice::ALL[i]));
            }
        }
        None
    }

    /// Shows `model`: the two sliders' positions (set silently, no message), the value
    /// labels, the Auto toggle, the limit note and the two choice groups (`ui-scaling.md`
    /// §7.2). Call after the client applied a change live.
    pub fn refresh(&mut self, ui: &mut Ui, model: &InterfaceModel) {
        set_bar_stop(ui, self.scale_bar, model.scale_stop, SCALE_STOPS);
        set_bar_stop(ui, self.text_bar, model.text_stop, TEXT_STOPS);
        put(ui, self.scale_value, &model.scale_label);
        put(ui, self.text_value, &model.text_label);
        put(ui, self.auto, &format!("{} Automatic", check(model.auto)));
        if let Some(l) = self.limited {
            match &model.limited {
                Some(text) => {
                    ui.set_text(l, Some(text.clone()));
                    ui.set_shown(l, true);
                }
                None => {
                    ui.set_text(l, Some(String::new()));
                    ui.set_shown(l, false);
                }
            }
        }
        for (i, b) in self.aspect.iter().enumerate() {
            let c = AspectChoice::ALL[i];
            let on = model.aspect == Some(c);
            put(ui, *b, &choice_label(c.label(), on));
        }
        for (i, b) in self.art.iter().enumerate() {
            let c = ArtChoice::ALL[i];
            let on = model.art == c;
            put(ui, *b, &choice_label(c.label(), on));
        }
        self.shown = model.clone();
    }

    /// The UI-scale slider's scroll-bar element (`ui-scaling.md` §7.2 row 2), for keyboard
    /// stepping and tests.
    #[must_use]
    pub fn scale_bar(&self) -> Option<ElementKey> {
        self.scale_bar
    }

    /// The text-size slider's scroll-bar element (`ui-scaling.md` §7.2 row 8).
    #[must_use]
    pub fn text_bar(&self) -> Option<ElementKey> {
        self.text_bar
    }

    /// Closes the window: releases the handlers and destroys the tree.
    pub fn close(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
        if ui.exists(self.root) {
            ui.destroy(self.root);
        }
    }
}

/// Clamps a stop index from a message into 0 … `count` − 1.
fn clamp_stop(stop: i64, count: u8) -> u8 {
    stop.clamp(0, i64::from(count - 1)) as u8
}

fn aspect_index(c: AspectChoice) -> u32 {
    AspectChoice::ALL.iter().position(|a| *a == c).unwrap_or(0) as u32
}

fn art_index(c: ArtChoice) -> u32 {
    ArtChoice::ALL.iter().position(|a| *a == c).unwrap_or(0) as u32
}

/// A check-box marker for a toggle's label.
fn check(on: bool) -> &'static str {
    if on { "[x]" } else { "[ ]" }
}

/// A choice button's label, marked when selected (neutral fallback, no highlight art needed).
fn choice_label(text: &str, on: bool) -> String {
    if on {
        format!("> {text}")
    } else {
        format!("  {text}")
    }
}

/// Sets a slider-mode scroll bar to a stop without posting a new-position message (so a
/// refresh after a live change never loops): the current-stop and position properties are
/// set directly and the bar re-lays itself (`ui-widgets.md` §6.4-§6.5).
fn set_bar_stop(ui: &mut Ui, bar: Option<ElementKey>, stop: u8, count: u8) {
    let Some(bar) = bar else { return };
    if !ui.exists(bar) {
        return;
    }
    let p = ui.vocab().pids;
    let q = if count > 1 {
        f32::from(stop) / f32::from(count - 1)
    } else {
        0.0
    };
    ui.set_property(bar, p.sb_current_stop, PropertyValue::Int(i32::from(stop)));
    ui.set_property(bar, p.sb_position, PropertyValue::Float(q));
}

/// Sets an element's text when it exists.
fn put(ui: &mut Ui, k: Option<ElementKey>, text: &str) {
    if let Some(k) = k {
        ui.set_text(k, Some(text.to_owned()));
    }
}

// ----- the tree -------------------------------------------------------------------------

/// The window's one text colour pid (the shaper reads `UICore_Text_font_color`).
fn colour_pid(nv: &NpcVocab) -> u32 {
    nv.pids.text_font_color
}

/// A label (`ui-scaling.md` §7.2).
fn label(nv: &NpcVocab, id: u32, x: i32, y: i32, w: i32, text: &str) -> FNode {
    FNode::text(id, x, y, w, 20, colour_pid(nv), colours::TITLE, text)
}

/// A button with the window's art.
#[allow(clippy::too_many_arguments)]
fn button(
    ui: &Ui,
    nv: &NpcVocab,
    art: &FallbackArt,
    id: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    text: &str,
) -> FNode {
    FNode::button(
        id,
        x,
        y,
        w,
        h,
        (&art.button, &art.button_ghosted),
        colour_pid(nv),
        ui.vocab().states.ghosted,
        text,
    )
}

/// A horizontal slider-mode scroll bar of `count` stops (`ui-widgets.md` §6.5,
/// `ui-scaling.md` §7.2 row 2), positioned at `stop`, with a thumb (element ID `widget`).
#[allow(clippy::too_many_arguments)]
fn slider(
    ui: &Ui,
    art: &FallbackArt,
    id: u32,
    x: i32,
    y: i32,
    w: i32,
    stop: u8,
    count: u8,
) -> FNode {
    let p = ui.vocab().pids;
    let thumb_id = ui.vocab().ids.widget;
    let q = if count > 1 {
        f32::from(stop) / f32::from(count - 1)
    } else {
        0.0
    };
    let h = 16;
    FNode::new(WidgetClass::Scrollbar, id, x, y, w, h)
        .image(&art.panel)
        .prop(p.sb_horizontal, PropertyValue::Bool(true))
        .prop(p.sb_num_stops, PropertyValue::Int(i32::from(count)))
        .prop(p.sb_slider, PropertyValue::Bool(true))
        .prop(p.sb_current_stop, PropertyValue::Int(i32::from(stop)))
        .prop(p.sb_position, PropertyValue::Float(q))
        .child(FNode::field(thumb_id, 0, 0, 12, h).image(&art.button))
}

/// Builds the window tree (`ui-scaling.md` §7.2 rows).
fn root_node(ui: &Ui, nv: &NpcVocab, art: &FallbackArt, model: &InterfaceModel) -> FNode {
    let margin = 16;
    let inner = WIN_W - 2 * margin;
    let mut root = FNode::field(ID_ROOT, 0, 0, WIN_W, WIN_H).image(&art.window);
    // Title bar and close box.
    root = root.child(
        FNode::field(0, 0, 0, WIN_W, 26)
            .image(&art.title_bar)
            .child(FNode::text(
                0,
                margin,
                4,
                inner - 24,
                20,
                colour_pid(nv),
                colours::TITLE,
                "Interface",
            ))
            .child(button(
                ui,
                nv,
                art,
                ID_CLOSE,
                WIN_W - margin - 20,
                3,
                20,
                20,
                "X",
            )),
    );
    let mut y = 40;
    // Row 1: "Interface scale".
    root = root.child(label(nv, 0, margin, y, inner, "Interface scale"));
    y += 24;
    // Row 2: the scale slider and its value label.
    root = root.child(slider(
        ui,
        art,
        ID_SCALE_BAR,
        margin,
        y + 2,
        inner - 70,
        model.scale_stop,
        SCALE_STOPS,
    ));
    root = root.child(label(
        nv,
        ID_SCALE_VALUE,
        WIN_W - margin - 62,
        y,
        62,
        &model.scale_label,
    ));
    y += 28;
    // Row 3: the Auto toggle.
    root = root.child(button(
        ui,
        nv,
        art,
        ID_AUTO,
        margin,
        y,
        inner,
        22,
        &format!("{} Automatic", check(model.auto)),
    ));
    y += 26;
    // Row 4: the limit note (hidden unless limited).
    let lim = model.limited.clone().unwrap_or_default();
    let mut lim_node = FNode::text(
        ID_LIMITED,
        margin,
        y,
        inner,
        20,
        colour_pid(nv),
        colours::YELLOW,
        &lim,
    );
    if model.limited.is_none() {
        lim_node = lim_node.hidden();
    }
    root = root.child(lim_node);
    y += 26;
    // Row 5: "Wide-screen area".
    root = root.child(label(nv, 0, margin, y, inner, "Wide-screen area"));
    y += 24;
    // Row 6: the five safe-aspect choice buttons.
    let n = AspectChoice::ALL.len() as i32;
    let gap = 6;
    let bw = (inner - gap * (n - 1)) / n;
    for (i, c) in AspectChoice::ALL.into_iter().enumerate() {
        let x = margin + (bw + gap) * i as i32;
        let on = model.aspect == Some(c);
        root = root.child(button(
            ui,
            nv,
            art,
            ID_ASPECT + i as u32,
            x,
            y,
            bw,
            24,
            &choice_label(c.label(), on),
        ));
    }
    y += 32;
    // Row 7: "Reset panel positions".
    root = root.child(button(
        ui,
        nv,
        art,
        ID_RESET_PANELS,
        margin,
        y,
        inner,
        24,
        "Reset panel positions",
    ));
    y += 34;
    // Row 8 (optional): the text-size slider.
    root = root.child(label(nv, 0, margin, y, 90, "Text size"));
    root = root.child(slider(
        ui,
        art,
        ID_TEXT_BAR,
        margin + 96,
        y + 2,
        inner - 96 - 62,
        model.text_stop,
        TEXT_STOPS,
    ));
    root = root.child(label(
        nv,
        ID_TEXT_VALUE,
        WIN_W - margin - 56,
        y,
        56,
        &model.text_label,
    ));
    y += 28;
    // Row 9 (optional): the art-filter choice group.
    root = root.child(label(nv, 0, margin, y, 60, "Art"));
    let na = ArtChoice::ALL.len() as i32;
    let abw = (inner - 64 - gap * (na - 1)) / na;
    for (i, c) in ArtChoice::ALL.into_iter().enumerate() {
        let x = margin + 64 + (abw + gap) * i as i32;
        let on = model.art == c;
        root = root.child(button(
            ui,
            nv,
            art,
            ID_ART + i as u32,
            x,
            y,
            abw,
            24,
            &choice_label(c.label(), on),
        ));
    }
    y += 32;
    // Row 10 (optional): "Reset window scales".
    root = root.child(button(
        ui,
        nv,
        art,
        ID_RESET_SCALES,
        margin,
        y,
        inner,
        24,
        "Reset window scales",
    ));
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_stop_round_trips() {
        assert!((scale_for_stop(0) - 0.50).abs() < 1e-6);
        assert!((scale_for_stop(10) - 1.00).abs() < 1e-6);
        assert!((scale_for_stop(SCALE_STOPS - 1) - 4.00).abs() < 1e-6);
        assert_eq!(stop_for_scale(0.50), 0);
        assert_eq!(stop_for_scale(1.00), 10);
        assert_eq!(stop_for_scale(2.00), 30);
        assert_eq!(stop_for_scale(4.00), 70);
        // Out of range clamps; halfway rounds up.
        assert_eq!(stop_for_scale(0.10), 0);
        assert_eq!(stop_for_scale(9.0), 70);
        assert_eq!(stop_for_scale(1.025), 11);
    }

    #[test]
    fn text_stop_round_trips() {
        assert!((text_for_stop(0) - 0.80).abs() < 1e-6);
        assert!((text_for_stop(TEXT_STOPS - 1) - 1.50).abs() < 1e-6);
        assert_eq!(stop_for_text(1.00), 4);
        assert_eq!(stop_for_text(0.80), 0);
        assert_eq!(stop_for_text(1.50), 14);
        assert_eq!(stop_for_text(2.0), 14);
    }
}
