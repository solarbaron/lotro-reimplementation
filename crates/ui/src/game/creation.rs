// SPDX-License-Identifier: GPL-3.0-or-later
//! Fallback controls of the character creation and selection screens
//! (`character-screens.md` §3–§5): choice lists (races, classes, nationalities,
//! characters), and the appearance controls' inner parts, which the data does not give
//! (§5.2 OPEN; Tessera rule: each template is a labelled row with previous/next arrows for
//! styles and presets, a swatch grid for colours, or a slider for floats).
//!
//! The controls are made of code-made elements placed inside a host element (the
//! control's template instance): Text labels, Buttons with arrow texts, a Field showing the
//! palette image. Their messages reach game code through game-code handlers (registered on
//! code element IDs of a reserved range), so the client passes every [`UiEvent`] to the
//! controls' `handle_event`.

use std::sync::atomic::{AtomicU32, Ordering};

use tessera_formats::did::Did;
use tessera_formats::ui_layout::ElementDesc;

use crate::env::{ImageRef, ImageState};
use crate::events::UiEvent;
use crate::geom::IBox;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::examine::{ExamTemplate, code_element};
use super::subtree;

/// First element ID of code-made controls (Tessera's own range, outside the
/// `UIElementID` key ranges and the plugin controls' range).
pub const CODE_ELEMENT_ID_BASE: u32 = 0x7F00_0000;

static NEXT_CODE_ID: AtomicU32 = AtomicU32::new(CODE_ELEMENT_ID_BASE);

fn next_code_id() -> u32 {
    let id = NEXT_CODE_ID.fetch_add(1, Ordering::Relaxed);
    if id < CODE_ELEMENT_ID_BASE {
        // Wrapped around (after 2^32 − base creations): start the range again.
        NEXT_CODE_ID.store(CODE_ELEMENT_ID_BASE + 1, Ordering::Relaxed);
        return CODE_ELEMENT_ID_BASE;
    }
    id
}

/// Most entries a choice list fills (Tessera cap).
pub const MAX_CHOICES: usize = 1024;

/// A code-made element of `class` at `bx` inside `parent`, with a fresh code element ID.
fn make(ui: &mut Ui, parent: ElementKey, class: WidgetClass, bx: IBox) -> Option<ElementKey> {
    let mut d = ElementDesc {
        element_id: next_code_id(),
        ..ElementDesc::default()
    };
    d.block.rect.x = bx.l;
    d.block.rect.y = bx.t;
    d.block.rect.width = bx.w().max(0);
    d.block.rect.height = bx.h().max(0);
    d.block.has_position = true;
    code_element(ui, parent, class, d)
}

/// Registers a game-code handler for `msg` on `k`'s element ID.
fn listen(ui: &mut Ui, k: ElementKey, msg: u32) {
    if let Some(id) = ui.element_id(k)
        && id != 0
    {
        ui.register_handler(id, msg);
    }
}

fn unlisten(ui: &mut Ui, k: ElementKey, msg: u32) {
    if let Some(id) = ui.element_id(k)
        && id != 0
    {
        ui.unregister_handler(id, msg);
    }
}

// ----- choice lists ----------------------------------------------------------------------

/// One entry of a choice list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Choice {
    /// The label (race, class or nationality name; a character's name).
    pub label: String,
    /// The icon.
    pub icon: Option<ImageRef>,
    /// Whether the entry is offered disabled (shown, not selectable).
    pub disabled: bool,
}

/// The parts of a choice entry that show its label and its icon, by element ID (found
/// below the entry, depth-first). `None`, or an ID not found below the entry, falls back
/// to the rule of [`fill_choices_with`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChoiceParts {
    /// The element showing the label.
    pub name: Option<u32>,
    /// The element showing the icon.
    pub icon: Option<u32>,
}

impl ChoiceParts {
    /// The race button's parts (`character-screens.md` §4.1: icon and name).
    #[must_use]
    pub fn race(ids: &super::charvocab::CharIds) -> Self {
        Self {
            name: Some(ids.race_name),
            icon: Some(ids.race_icon),
        }
    }

    /// The class button's parts (§4.2: icon and name).
    #[must_use]
    pub fn class(ids: &super::charvocab::CharIds) -> Self {
        Self {
            name: Some(ids.class_name),
            icon: Some(ids.class_icon),
        }
    }

    /// The character row's parts (§3: `CharGen_CharacterSelect_Character_Name` and the
    /// class icon `CharGen_CharacterSelect_Character_Class`).
    #[must_use]
    pub fn character(ids: &super::charvocab::CharIds) -> Self {
        Self {
            name: Some(ids.character_name),
            icon: Some(ids.character_class),
        }
    }
}

/// Fills a list box (`CharGen_Race_Select_Listbox`, `CharGen_Class_Select_Listbox`, …)
/// with one entry per choice, created from `template` (the list's entry template, for
/// example `CharGen_Race_Select_Race_Template`), finding each entry's parts by the
/// fallback rule of [`fill_choices_with`] (no named parts). Prefer
/// [`fill_choices_with`] with the [`ChoiceParts`] of the list.
pub fn fill_choices(
    ui: &mut Ui,
    list: ElementKey,
    template: ExamTemplate,
    choices: &[Choice],
) -> Vec<Option<ElementKey>> {
    fill_choices_with(ui, list, template, ChoiceParts::default(), choices)
}

/// Fills a list box with one entry per choice, created from `template`: the label goes to
/// the entry's `parts.name` element and the icon to its `parts.icon` element
/// (`character-screens.md` §3, §4.1, §4.2, §5.1). Without a named part found below the
/// entry, a **Tessera rule** picks it: the label goes to the first `Text` element below
/// the entry (never the entry itself, which is a Button on these screens, nor a Button or
/// Menu part), else to the entry when it shows text itself; the icon goes to the first
/// square element below the entry that is not a text class and not of the entry's own
/// size (a leaf first), so a full-size background or a frame piece never gets it; without
/// one the icon is not shown. Earlier entries are removed. Returns the entries in choice
/// order (a choice whose entry cannot be created is left out of the list and gives
/// `None`).
pub fn fill_choices_with(
    ui: &mut Ui,
    list: ElementKey,
    template: ExamTemplate,
    parts: ChoiceParts,
    choices: &[Choice],
) -> Vec<Option<ElementKey>> {
    ui.list_clear(list);
    let mut out = Vec::with_capacity(choices.len().min(MAX_CHOICES));
    for c in choices.iter().take(MAX_CHOICES) {
        let Ok(k) = ui.create_element(None, Did(template.layout), template.element) else {
            out.push(None);
            continue;
        };
        if ui.list_add(list, k, None).is_err() {
            ui.destroy(k);
            out.push(None);
            continue;
        }
        if let Some(t) = name_part(ui, k, parts.name) {
            ui.set_text(t, Some(c.label.clone()));
        }
        if let Some(img) = &c.icon {
            match icon_part(ui, k, parts.icon) {
                Some(f) => super::set_image_ref(ui, f, img, super::examine::DRAW_OVER),
                None => tracing::debug!("choice entry without an icon part; icon not shown"),
            }
        }
        if c.disabled {
            let ghosted = ui.vocab().states.ghosted;
            ui.switch_state_if_defined(k, ghosted);
        }
        out.push(Some(k));
    }
    out
}

/// Shows `image` on element `k` once at the image's own size from `k`'s top-left corner
/// (clipped to `k`'s box) instead of tiled over `k` (`ui-layout.md` §9.6 draws element
/// images tiled at native size): a code-made child field of the image's size, as an
/// [`super::IconImage::Layers`] icon with one layer. Call [`super::fit_icon_layers`] on `k`
/// once the image has loaded. A **Tessera rule** for choice icons smaller than their part
/// (`character-screens.md` §4.1 OPEN on the race button's parts).
pub fn show_icon_once(ui: &mut Ui, k: ElementKey, image: &ImageRef) {
    super::set_icon(ui, k, Some(&super::IconImage::Layers(vec![image.clone()])));
}

/// The element of entry `k` that shows the label ([`fill_choices_with`]).
fn name_part(ui: &Ui, k: ElementKey, id: Option<u32>) -> Option<ElementKey> {
    if let Some(e) = id.and_then(|id| super::find_below(ui, k, id)) {
        return Some(e);
    }
    subtree(ui, k)
        .into_iter()
        .skip(1)
        .find(|e| ui.class(*e) == Some(WidgetClass::Text))
        .or_else(|| ui.class(k).is_some_and(WidgetClass::is_text).then_some(k))
}

/// The element of entry `k` that shows the icon ([`fill_choices_with`]).
fn icon_part(ui: &Ui, k: ElementKey, id: Option<u32>) -> Option<ElementKey> {
    if let Some(e) = id.and_then(|id| super::find_below(ui, k, id)) {
        return Some(e);
    }
    let root = ui.layout_box(k).unwrap_or_default();
    let squares: Vec<ElementKey> = subtree(ui, k)
        .into_iter()
        .skip(1)
        .filter(|e| !ui.class(*e).is_some_and(WidgetClass::is_text))
        .filter(|e| {
            ui.layout_box(*e).is_some_and(|b| {
                b.w() > 0 && b.w() == b.h() && (b.w(), b.h()) != (root.w(), root.h())
            })
        })
        .collect();
    squares
        .iter()
        .copied()
        .find(|e| ui.children(*e).is_empty())
        .or_else(|| squares.first().copied())
}

/// The index of `list`'s selected entry among `entries` (as [`fill_choices`] returned
/// them).
#[must_use]
pub fn selected_choice(ui: &Ui, list: ElementKey, entries: &[Option<ElementKey>]) -> Option<usize> {
    let sel = ui.list_selected(list)?;
    entries.iter().position(|e| *e == Some(sel))
}

/// Selects entry `i` of `entries` (without notifying).
pub fn select_choice(ui: &mut Ui, list: ElementKey, entries: &[Option<ElementKey>], i: usize) {
    let e = entries.get(i).copied().flatten();
    ui.list_select(list, e, false);
}

// ----- previous / next ------------------------------------------------------------------

/// Width of an arrow button of the fallback controls, pixels (Tessera rule).
const ARROW_W: i32 = 20;

/// A labelled row with previous and next arrows (styles and presets, §5.2 Tessera rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stepper {
    /// The host element.
    pub host: ElementKey,
    /// The label.
    pub label: Option<ElementKey>,
    /// The previous button.
    pub prev: ElementKey,
    /// The value text between the arrows.
    pub value: Option<ElementKey>,
    /// The next button.
    pub next: ElementKey,
    index: usize,
    count: usize,
}

impl Stepper {
    /// Builds the row inside `host`: the label on the left half, then "<", the value text
    /// and ">" across the right half, all as tall as the host. `count` choices, starting
    /// at `index` (clamped).
    pub fn build(
        ui: &mut Ui,
        host: ElementKey,
        label: &str,
        count: usize,
        index: usize,
    ) -> Option<Self> {
        let b = ui.layout_box(host)?;
        let (w, h) = (b.w().max(0), b.h().max(0));
        let half = w / 2;
        let label_el = make(ui, host, WidgetClass::Text, IBox::from_xywh(0, 0, half, h));
        if let Some(l) = label_el {
            ui.set_text(l, Some(label.to_string()));
        }
        let prev = make(
            ui,
            host,
            WidgetClass::Button,
            IBox::from_xywh(half, 0, ARROW_W, h),
        )?;
        let value_w = (w - half - 2 * ARROW_W).max(0);
        let value = make(
            ui,
            host,
            WidgetClass::Text,
            IBox::from_xywh(half + ARROW_W, 0, value_w, h),
        );
        let next = make(
            ui,
            host,
            WidgetClass::Button,
            IBox::from_xywh(half + ARROW_W + value_w, 0, ARROW_W, h),
        )?;
        ui.set_text(prev, Some("<".to_string()));
        ui.set_text(next, Some(">".to_string()));
        let mut s = Self::bind(ui, host, prev, next, value, count);
        s.label = label_el;
        s.index = index.min(count.saturating_sub(1));
        s.show(ui);
        Some(s)
    }

    /// A stepper over existing buttons and value text (when a template has them).
    pub fn bind(
        ui: &mut Ui,
        host: ElementKey,
        prev: ElementKey,
        next: ElementKey,
        value: Option<ElementKey>,
        count: usize,
    ) -> Self {
        let click = ui.vocab().msgs.button_click;
        listen(ui, prev, click);
        listen(ui, next, click);
        Self {
            host,
            label: None,
            prev,
            value,
            next,
            index: 0,
            count,
        }
    }

    /// The current index.
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    /// The number of choices.
    #[must_use]
    pub fn count(&self) -> usize {
        self.count
    }

    /// Sets the index (clamped) and the number of choices.
    pub fn set(&mut self, ui: &mut Ui, index: usize, count: usize) {
        self.count = count;
        self.index = index.min(count.saturating_sub(1));
        self.show(ui);
    }

    /// Sets the value text (for example a preset's name); `None` shows "index / count".
    pub fn set_value_text(&mut self, ui: &mut Ui, text: Option<String>) {
        if let Some(v) = self.value {
            let t = text.unwrap_or_else(|| self.default_text());
            ui.set_text(v, Some(t));
        }
    }

    fn default_text(&self) -> String {
        if self.count == 0 {
            String::new()
        } else {
            format!("{} / {}", self.index + 1, self.count)
        }
    }

    fn show(&self, ui: &mut Ui) {
        if let Some(v) = self.value {
            ui.set_text(v, Some(self.default_text()));
        }
    }

    /// A click on an arrow moves to the previous or next choice, wrapping around. Returns
    /// the new index when it changed.
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> Option<usize> {
        let UiEvent::Game { message } = ev else {
            return None;
        };
        if message.msg != ui.vocab().msgs.button_click || self.count == 0 {
            return None;
        }
        let new = if message.source == self.prev {
            (self.index + self.count - 1) % self.count
        } else if message.source == self.next {
            (self.index + 1) % self.count
        } else {
            return None;
        };
        if new == self.index {
            return None;
        }
        self.index = new;
        self.show(ui);
        Some(new)
    }

    /// Removes the game-code handlers (before the host is destroyed).
    pub fn release(&self, ui: &mut Ui) {
        let click = ui.vocab().msgs.button_click;
        unlisten(ui, self.prev, click);
        unlisten(ui, self.next, click);
    }
}

// ----- slider ---------------------------------------------------------------------------

/// Height of the fallback slider's track, pixels (Tessera rule).
const TRACK_H: i32 = 6;

/// Width of the fallback slider's thumb, pixels (Tessera rule).
const THUMB_W: i32 = 10;

/// A slider for a float in [min, max] (bone scaling, §5.2 Tessera rule).
#[derive(Debug, Clone, PartialEq)]
pub struct Slider {
    /// The host element.
    pub host: ElementKey,
    /// The track (a press on it moves the thumb there).
    pub track: ElementKey,
    /// The thumb.
    pub thumb: ElementKey,
    min: f32,
    max: f32,
    value: f32,
    dragging: bool,
}

impl Slider {
    /// Builds a slider across `host`, vertically centred, for values in [`min`, `max`]
    /// (swapped when reversed; non-finite bounds become 0 and 1), starting at `value`.
    pub fn build(ui: &mut Ui, host: ElementKey, min: f32, max: f32, value: f32) -> Option<Self> {
        let b = ui.layout_box(host)?;
        let (w, h) = (b.w().max(0), b.h().max(0));
        let top = (h - TRACK_H).max(0) / 2;
        let track = make(
            ui,
            host,
            WidgetClass::Field,
            IBox::from_xywh(0, top, w, TRACK_H),
        )?;
        let thumb = make(
            ui,
            host,
            WidgetClass::Button,
            IBox::from_xywh(0, 0, THUMB_W, h),
        )?;
        ui.set_text(thumb, Some("|".to_string()));
        let press = ui.vocab().msgs.mouse_press;
        listen(ui, track, press);
        listen(ui, thumb, press);
        let (mut lo, mut hi) = (
            if min.is_finite() { min } else { 0.0 },
            if max.is_finite() { max } else { 1.0 },
        );
        if lo > hi {
            std::mem::swap(&mut lo, &mut hi);
        }
        let mut s = Self {
            host,
            track,
            thumb,
            min: lo,
            max: hi,
            value: lo,
            dragging: false,
        };
        s.set_value(ui, value);
        Some(s)
    }

    /// The value.
    #[must_use]
    pub fn value(&self) -> f32 {
        self.value
    }

    /// The range.
    #[must_use]
    pub fn range(&self) -> (f32, f32) {
        (self.min, self.max)
    }

    /// Sets the value (clamped to the range) and moves the thumb.
    pub fn set_value(&mut self, ui: &mut Ui, v: f32) {
        let v = if v.is_finite() { v } else { self.min };
        self.value = v.clamp(self.min, self.max);
        let span = self.travel(ui);
        let f = if self.max > self.min {
            (self.value - self.min) / (self.max - self.min)
        } else {
            0.0
        };
        let x = (f * span as f32).round() as i32;
        let tb = ui.layout_box(self.thumb).unwrap_or_default();
        ui.set_layout_box(self.thumb, IBox::from_xywh(x, tb.t, tb.w(), tb.h()));
    }

    /// The thumb's travel in pixels.
    fn travel(&self, ui: &Ui) -> i32 {
        let w = ui.layout_box(self.track).map_or(0, |b| b.w());
        (w - THUMB_W).max(0)
    }

    /// The value at screen x `x`: the thumb centred on it, clamped.
    fn value_at(&self, ui: &Ui, x: i32) -> f32 {
        let tb = ui.displayed_box(self.track);
        let span = self.travel(ui);
        if span <= 0 {
            return self.min;
        }
        let rel =
            (i64::from(x) - i64::from(tb.l) - i64::from(THUMB_W / 2)).clamp(0, i64::from(span));
        self.min + (self.max - self.min) * (rel as f32 / span as f32)
    }

    /// A left press on the track or the thumb moves the thumb under the pointer and starts
    /// a drag. Returns the new value when it changed.
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> Option<f32> {
        let UiEvent::Game { message } = ev else {
            return None;
        };
        if message.msg != ui.vocab().msgs.mouse_press
            || (message.source != self.track && message.source != self.thumb)
        {
            return None;
        }
        let left = message
            .arg
            .command
            .as_ref()
            .is_some_and(|c| c.mouse_button() == 1);
        if !left {
            return None;
        }
        self.dragging = true;
        let x = ui.pointer()?.0;
        self.move_to_x(ui, x)
    }

    fn move_to_x(&mut self, ui: &mut Ui, x: i32) -> Option<f32> {
        let v = self.value_at(ui, x);
        let old = self.value;
        self.set_value(ui, v);
        (self.value != old).then_some(self.value)
    }

    /// The drag: while the track or the thumb holds the capture, the thumb follows the
    /// pointer. Call after pointer moves. Returns the new value when it changed.
    pub fn update(&mut self, ui: &mut Ui) -> Option<f32> {
        if !self.dragging {
            return None;
        }
        let cap = ui.captured();
        if cap != Some(self.track) && cap != Some(self.thumb) {
            self.dragging = false;
            return None;
        }
        let x = ui.pointer()?.0;
        self.move_to_x(ui, x)
    }

    /// Removes the game-code handlers (before the host is destroyed).
    pub fn release(&self, ui: &mut Ui) {
        let press = ui.vocab().msgs.mouse_press;
        unlisten(ui, self.track, press);
        unlisten(ui, self.thumb, press);
    }
}

// ----- swatches -------------------------------------------------------------------------

/// A colour picker over a palette image (dye-map controls, §5.2 Tessera rule: "colours:
/// any palette pixel"): the image shown at its own size, a click picks the pixel under the
/// pointer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwatchGrid {
    /// The host element.
    pub host: ElementKey,
    /// The field showing the palette.
    pub field: ElementKey,
    /// The marker of the picked pixel.
    pub marker: Option<ElementKey>,
    /// The palette image.
    pub palette: ImageRef,
    picked: Option<(u32, u32)>,
}

impl SwatchGrid {
    /// Builds the grid in `host`'s top-left corner, sized to the palette image (to
    /// `fallback` while the image is not loaded).
    pub fn build(
        ui: &mut Ui,
        host: ElementKey,
        palette: ImageRef,
        fallback: (i32, i32),
    ) -> Option<Self> {
        let (w, h) = match ui.env.image(&palette) {
            ImageState::Ready(i) => (
                i32::try_from(i.width()).unwrap_or(i32::MAX),
                i32::try_from(i.height()).unwrap_or(i32::MAX),
            ),
            _ => fallback,
        };
        let field = make(ui, host, WidgetClass::Field, IBox::from_xywh(0, 0, w, h))?;
        super::set_image_ref(ui, field, &palette, super::examine::DRAW_OVER);
        let marker = make(ui, field, WidgetClass::Text, IBox::from_xywh(0, 0, 8, 8));
        if let Some(m) = marker {
            ui.set_text(m, Some("+".to_string()));
            ui.set_shown(m, false);
        }
        let click = ui.vocab().msgs.mouse_click;
        listen(ui, field, click);
        Some(Self {
            host,
            field,
            marker,
            palette,
            picked: None,
        })
    }

    /// The picked pixel.
    #[must_use]
    pub fn picked(&self) -> Option<(u32, u32)> {
        self.picked
    }

    /// Marks pixel (`x`, `y`) as picked (clamped to the field).
    pub fn pick(&mut self, ui: &mut Ui, x: i32, y: i32) -> (u32, u32) {
        let b = ui.layout_box(self.field).unwrap_or_default();
        let px = x.clamp(0, (b.w() - 1).max(0)) as u32;
        let py = y.clamp(0, (b.h() - 1).max(0)) as u32;
        self.picked = Some((px, py));
        if let Some(m) = self.marker {
            let mb = ui.layout_box(m).unwrap_or_default();
            ui.set_layout_box(
                m,
                IBox::from_xywh(
                    px as i32 - mb.w() / 2,
                    py as i32 - mb.h() / 2,
                    mb.w(),
                    mb.h(),
                ),
            );
            ui.set_shown(m, true);
        }
        (px, py)
    }

    /// The palette colour at the picked pixel, `0xAARRGGBB`, when the image is loaded.
    #[must_use]
    pub fn colour(&self, ui: &mut Ui) -> Option<u32> {
        let (x, y) = self.picked?;
        match ui.env.image(&self.palette) {
            ImageState::Ready(i) => i.pixel(x, y),
            _ => None,
        }
    }

    /// A left click on the palette picks the pixel under the pointer. Returns it.
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> Option<(u32, u32)> {
        let UiEvent::Game { message } = ev else {
            return None;
        };
        if message.msg != ui.vocab().msgs.mouse_click || message.source != self.field {
            return None;
        }
        let left = message
            .arg
            .command
            .as_ref()
            .is_some_and(|c| c.mouse_button() == 1);
        if !left {
            return None;
        }
        Some(self.pick(ui, message.arg.pos.0, message.arg.pos.1))
    }

    /// Removes the game-code handler (before the host is destroyed).
    pub fn release(&self, ui: &mut Ui) {
        let click = ui.vocab().msgs.mouse_click;
        unlisten(ui, self.field, click);
    }
}
