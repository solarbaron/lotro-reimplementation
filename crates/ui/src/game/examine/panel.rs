// SPDX-License-Identifier: GPL-3.0-or-later
//! Building, assembling, showing and pinning examination tooltips on the element tree
//! (`item-tooltips.md` §4–§5, §7–§10).

use std::sync::Arc;

use tessera_formats::did::Did;
use tessera_formats::property::{PropertyArray, PropertyEntry, PropertyValue};
use tessera_formats::ui_layout::ElementDesc;

use crate::env::{ImageRef, ImageState, TextRequest};
use crate::events::UiEvent;
use crate::geom::IBox;
use crate::props::{array_elements, as_key, struct_field};
use crate::tree::{Element, ElementKey};
use crate::ui::{MAX_TREE_DEPTH, Ui, object_mode};
use crate::widgets::WidgetClass;

use super::super::IconImage;
use super::super::charvocab::CharVocab;
use super::super::{find_below, subtree};
use super::ExamError;
use super::assemble::{AsmRow, Assembly, assemble};
use super::input::{
    ExamCoin, ExamKey, ExamLine, ExamRow, ExamTemplate, ExamTooltip, MAX_COMPARISONS,
    MAX_ROW_DEPTH, MAX_ROWS, RowContent,
};
use super::pin::{PinOutcome, PinRegistry};
use super::place::{place, place_group};

/// Default pin offset when the `PinnedTooltip` element carries none (the data's value,
/// `item-tooltips.md` §10: (18, 18)).
const DEFAULT_PIN_OFFSET: i32 = 18;

/// The modulus of the pinned-window counter *k* (`item-tooltips.md` §10 answer (1), IT28;
/// a decision-0012 numeric constant): the *k*-th window opens at the `PinnedTooltip`
/// design position plus *k* × the offset, *k* counting the windows opened so far modulo 20.
pub const PIN_CASCADE_MODULUS: u32 = 20;

/// Size of a coin icon whose image is not loaded yet (Tessera rule).
const COIN_ICON: i32 = 16;

/// Gap between the money row's parts, pixels (Tessera rule).
const MONEY_GAP: i32 = 2;

/// Draw mode of icons and images set by game code: 4, Alphablend (the compositor's Over,
/// `ui-layout.md` §9.6.1), so transparent icon pixels show what lies behind (Tessera rule).
pub(crate) const DRAW_OVER: u32 = 4;

/// A tooltip built on the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltTooltip {
    /// The tooltip element (a root panel attached to the screen root).
    pub frame: ElementKey,
    /// Its list box.
    pub list: ElementKey,
    /// The frame's size after assembly.
    pub size: (i32, i32),
    /// The rows that stayed, in order (§5).
    pub rows: Vec<ElementKey>,
    /// Row templates that could not be created: (template index, reason).
    pub failures: Vec<(usize, String)>,
    /// The kept row element of each row template (by template index, as
    /// [`ExamTooltip::rows`]); `None` for a row §5 removed or that could not be created.
    pub by_template: Vec<Option<ElementKey>>,
}

/// What [`ExamTooltips::show`] showed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownTooltip {
    /// The hover tooltip.
    pub hover: BuiltTooltip,
    /// The comparison tooltips, in order.
    pub comparisons: Vec<BuiltTooltip>,
    /// Screen positions: the hover tooltip first, then the comparisons.
    pub positions: Vec<(i32, i32)>,
}

/// One pinned tooltip window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedWindow {
    /// The `PinnedTooltip` window.
    pub window: ElementKey,
    /// The tooltip inside it.
    pub tooltip: BuiltTooltip,
    /// The buttons that close it (`PinnedTooltipCloseButton`, see [`ExamTooltips::pin`]).
    pub close_buttons: Vec<ElementKey>,
    /// The element IDs whose `Button_click` game-code handler the pin registered (one
    /// registration each), so that they are undone even after the buttons are gone.
    pub registered_ids: Vec<u32>,
}

/// A row while it is filled: its element and what §5 sees.
struct Filled {
    asm: AsmRow,
    /// For a vertical nested list: its row elements in input order.
    children: Vec<(ElementKey, Filled)>,
}

impl Filled {
    fn from_size((w, h): (i32, i32)) -> Self {
        Self::sized(w, h)
    }

    fn sized(w: i32, h: i32) -> Self {
        Self {
            asm: AsmRow::sized(w.max(0), h.max(0)),
            children: Vec::new(),
        }
    }
}

/// The examination tooltip controller: builds tooltips from [`ExamTooltip`] inputs, shows
/// hover tooltips with their comparisons, and keeps the pinned tooltips.
#[derive(Debug, Clone)]
pub struct ExamTooltips {
    vocab: CharVocab,
    pins: PinRegistry<PinnedWindow>,
    pin_layout: Option<u32>,
    /// The element ID of `PinnedTooltipCloseButton` (resolved by the client).
    close_button: Option<u32>,
    /// Pinned windows opened so far, modulo [`PIN_CASCADE_MODULUS`] (§10 answer (1)).
    opened: u32,
}

impl ExamTooltips {
    /// A controller over the given vocabulary.
    #[must_use]
    pub fn new(vocab: CharVocab) -> Self {
        Self {
            vocab,
            pins: PinRegistry::new(),
            pin_layout: None,
            close_button: None,
            opened: 0,
        }
    }

    /// Sets the element ID of `PinnedTooltipCloseButton` (the `UIElementID` key of that
    /// label, `item-tooltips.md` §10 answer (2)): only a click on that button closes the
    /// pinned window it is in. Without it (the label unresolved) every Button of the window
    /// outside the tooltip closes it (Tessera's earlier rule, kept as the fallback).
    pub fn set_close_button_id(&mut self, id: u32) {
        self.close_button = (id != 0).then_some(id);
    }

    /// [`ExamTooltips::set_close_button_id`] by value.
    #[must_use]
    pub fn with_close_button_id(mut self, id: u32) -> Self {
        self.set_close_button_id(id);
        self
    }

    /// The pinned-window counter *k* the next pin uses (§10 answer (1)).
    #[must_use]
    pub fn pin_counter(&self) -> u32 {
        self.opened
    }

    /// Sets the examination layout whose `PinnedTooltip` element pinned tooltips open
    /// from (`item-tooltips.md` §10; the layout of `UILAYOUT` label `examination`,
    /// 0x22000000 here). Without it a pin opens `PinnedTooltip` from the tooltip
    /// element's own layout, which fails for `KinshipCharter_ExamineLayout` (its layout
    /// 0x220008BE has no `PinnedTooltip`, §4.1).
    pub fn set_pin_layout(&mut self, layout: u32) {
        self.pin_layout = (layout != 0).then_some(layout);
    }

    /// [`ExamTooltips::set_pin_layout`] by value.
    #[must_use]
    pub fn with_pin_layout(mut self, layout: u32) -> Self {
        self.set_pin_layout(layout);
        self
    }

    /// The vocabulary.
    #[must_use]
    pub fn vocab(&self) -> &CharVocab {
        &self.vocab
    }

    /// The pinned tooltips in pin order.
    pub fn pinned(&self) -> impl Iterator<Item = &(ExamKey, PinnedWindow)> {
        self.pins.iter()
    }

    /// Builds one tooltip (`item-tooltips.md` §3.3 step 4, §4, §5): creates the tooltip
    /// element from `input.frame` as a root panel attached to the screen root, creates one
    /// row per entry template of its list box (or per [`ExamTooltip::templates`]), fills
    /// each from `input.rows`, measures the rows, assembles the list by §5 and sizes the
    /// list and the frame. The frame is placed at (0, 0).
    ///
    /// # Errors
    /// [`ExamError::Ui`] when the frame cannot be created, [`ExamError::NoListBox`] when it
    /// holds no list box.
    pub fn build(&self, ui: &mut Ui, input: &ExamTooltip) -> Result<BuiltTooltip, ExamError> {
        let mut removed = Vec::new();
        let built = self.build_deferred(ui, input, &mut removed);
        ui.destroy_many(&removed);
        built
    }

    /// [`ExamTooltips::build`], leaving the rows §5 step 1 removes in `removed` (detached
    /// from the list's assembly, still alive) for the caller to destroy in one batch.
    fn build_deferred(
        &self,
        ui: &mut Ui,
        input: &ExamTooltip,
        removed: &mut Vec<ElementKey>,
    ) -> Result<BuiltTooltip, ExamError> {
        let frame = ui.create_element(None, Did(input.frame.layout), input.frame.element)?;
        if let Some(e) = ui.el_mut(frame) {
            e.root_panel = true;
            e.screen_level = true;
        }
        let pids = self.vocab.pids;
        if input.key.record != 0 {
            ui.set_property(
                frame,
                pids.tooltip_did,
                PropertyValue::DidRef(Did(input.key.record)),
            );
        }
        if input.key.instance != 0 {
            ui.set_property(
                frame,
                pids.tooltip_iid,
                PropertyValue::Int64(input.key.instance as i64),
            );
        }
        let Some(list) = self.find_list_box(ui, frame) else {
            ui.destroy(frame);
            return Err(ExamError::NoListBox);
        };
        if let Err(e) = ui.attach_raw(frame, ui.screen) {
            ui.destroy(frame);
            return Err(e.into());
        }
        let built = self.fill_list(ui, frame, list, input, removed);
        Ok(built)
    }

    /// Shows the hover tooltip of `input` with its comparison tooltips (§8, §9): builds
    /// them, places the hover tooltip by §9 at `pointer` and the comparisons by the §8.3
    /// Tessera rule, and makes them the UI's showing tooltip, so the hiding rules of
    /// `ui-behaviour.md` §6.6 apply. A comparison tooltip that cannot be built is left out.
    ///
    /// # Errors
    /// As [`ExamTooltips::build`] for the hover tooltip.
    pub fn show(
        &self,
        ui: &mut Ui,
        input: &ExamTooltip,
        pointer: (i32, i32),
    ) -> Result<ShownTooltip, ExamError> {
        ui.hide_tooltip();
        // The removed rows of every tooltip go in one batch at the end.
        let mut removed = Vec::new();
        let hover = match self.build_deferred(ui, input, &mut removed) {
            Ok(b) => b,
            Err(e) => {
                ui.destroy_many(&removed);
                return Err(e);
            }
        };
        let mut comparisons = Vec::new();
        for c in input.comparisons.iter().take(MAX_COMPARISONS) {
            let c = ExamTooltip {
                comparisons: Vec::new(),
                ..c.clone()
            };
            match self.build_deferred(ui, &c, &mut removed) {
                Ok(b) => comparisons.push(b),
                Err(e) => tracing::debug!("comparison tooltip not built: {e}"),
            }
        }
        ui.destroy_many(&removed);
        let cs = ui.cursor_size();
        let screen = ui.screen_size();
        let first = place(pointer, (cs, cs), hover.size, screen);
        let sizes: Vec<(i32, i32)> = std::iter::once(hover.size)
            .chain(comparisons.iter().map(|c| c.size))
            .collect();
        let positions = place_group(first, &sizes, screen);
        for (b, p) in std::iter::once(&hover)
            .chain(comparisons.iter())
            .zip(&positions)
        {
            ui.move_to(b.frame, p.0, p.1);
        }
        ui.set_tooltip_panels(hover.frame, comparisons.iter().map(|c| c.frame).collect());
        Ok(ShownTooltip {
            hover,
            comparisons,
            positions,
        })
    }

    /// The pin toggle (`item-tooltips.md` §10, with the Tessera rule for its input:
    /// `TOOLTIP_DETACH` while an item tooltip shows). For an item already pinned its
    /// window closes ([`PinOutcome::Unpinned`]); with 20 pinned the request is refused
    /// ([`PinOutcome::Refused`], the client shows a UI message); otherwise the
    /// examination layout's `PinnedTooltip` element (from the layout of
    /// [`ExamTooltips::set_pin_layout`], else from `input.frame`'s layout) opens as a
    /// window holding a copy of the tooltip, its list box grown to the tooltip's size and
    /// the window by the same amount (§10 answers (3), (5)). The *k*-th window opened
    /// opens at the `PinnedTooltip` root's design position plus *k* ×
    /// (`UI_Examination_Tooltip_PinnedOffset_X`, `…_Y`), *k* counting the windows opened
    /// so far modulo [`PIN_CASCADE_MODULUS`] (0 for the first; §10 answer (1), IT28); it does
    /// not follow the hover tooltip, and `_pointer` is not used.
    ///
    /// Only a click on the window's `PinnedTooltipCloseButton`
    /// ([`ExamTooltips::set_close_button_id`]) closes it (§10 answer (2)); its
    /// `Button_click` reaches [`ExamTooltips::handle_event`] through a game-code handler.
    ///
    /// # Errors
    /// [`ExamError::Ui`] when the window or the tooltip cannot be created,
    /// [`ExamError::NoListBox`] when the window has no list box.
    pub fn pin(
        &mut self,
        ui: &mut Ui,
        input: &ExamTooltip,
        _pointer: (i32, i32),
    ) -> Result<PinOutcome, ExamError> {
        self.prune(ui);
        match self.pins.decide(input.key) {
            PinOutcome::Unpinned => {
                self.unpin(ui, input.key);
                return Ok(PinOutcome::Unpinned);
            }
            PinOutcome::Refused => return Ok(PinOutcome::Refused),
            PinOutcome::Pinned => {}
        }
        let window = match self.pin_layout {
            Some(l) => ui.open_panel(Did(l), self.vocab.ids.pinned_tooltip)?,
            None => ui.open_panel(Did(input.frame.layout), self.vocab.ids.pinned_tooltip)?,
        };
        // §10 answer (1): the window's own design position.
        let anchor = ui
            .design_box(window)
            .or_else(|| ui.layout_box(window))
            .map_or((0, 0), |b| (b.l, b.t));
        let host = match subtree(ui, window)
            .into_iter()
            .skip(1)
            .find(|k| ui.class(*k).is_some_and(WidgetClass::is_list))
        {
            Some(h) => h,
            None => {
                ui.destroy(window);
                return Err(ExamError::NoListBox);
            }
        };
        let tip = ExamTooltip {
            comparisons: Vec::new(),
            ..input.clone()
        };
        let tooltip = match self.build(ui, &tip) {
            Ok(t) => t,
            Err(e) => {
                ui.destroy(window);
                return Err(e);
            }
        };
        if let Some(e) = ui.el_mut(tooltip.frame) {
            e.root_panel = false;
            e.screen_level = false;
        }
        if let Err(e) = ui.list_add(host, tooltip.frame, None) {
            ui.destroy(window);
            ui.destroy(tooltip.frame);
            return Err(e.into());
        }
        // The list box grows to hold the tooltip (the painter clips to it), and the window
        // by the same amount, so its other parts keep their distance to its edges.
        let hb = ui.layout_box(host).unwrap_or_default();
        let wb = ui.layout_box(window).unwrap_or_default();
        let (dw, dh) = (
            tooltip.size.0.saturating_sub(hb.w()).max(0),
            tooltip.size.1.saturating_sub(hb.h()).max(0),
        );
        ui.set_layout_box(
            window,
            IBox::from_xywh(
                wb.l,
                wb.t,
                wb.w().saturating_add(dw),
                wb.h().saturating_add(dh),
            ),
        );
        // After the window: its relayout may move the list box by its edge modes.
        let hb = ui.layout_box(host).unwrap_or(hb);
        ui.set_layout_box(
            host,
            IBox::from_xywh(
                hb.l,
                hb.t,
                hb.w().max(tooltip.size.0),
                hb.h().max(tooltip.size.1),
            ),
        );
        let p = self.vocab.pids;
        let off_x = ui.prop(window, p.pinned_offset_x);
        let off_y = ui.prop(window, p.pinned_offset_y);
        let ox = if off_x.is_set() {
            off_x.int()
        } else {
            DEFAULT_PIN_OFFSET
        };
        let oy = if off_y.is_set() {
            off_y.int()
        } else {
            DEFAULT_PIN_OFFSET
        };
        let k = i32::try_from(self.opened % PIN_CASCADE_MODULUS).unwrap_or(0);
        ui.move_to(
            window,
            anchor.0.saturating_add(ox.saturating_mul(k)),
            anchor.1.saturating_add(oy.saturating_mul(k)),
        );
        let in_tooltip = subtree(ui, tooltip.frame);
        let close_button = self.close_button;
        let close_buttons: Vec<ElementKey> = subtree(ui, window)
            .into_iter()
            .filter(|k| !in_tooltip.contains(k))
            .filter(|k| match close_button {
                Some(id) => ui.element_id(*k) == Some(id),
                None => ui.class(*k) == Some(WidgetClass::Button),
            })
            .collect();
        let click = ui.vocab().msgs.button_click;
        let mut registered_ids = Vec::with_capacity(close_buttons.len());
        for b in &close_buttons {
            if let Some(id) = ui.element_id(*b) {
                ui.register_handler(id, click);
                registered_ids.push(id);
            }
        }
        self.pins.insert(
            input.key,
            PinnedWindow {
                window,
                tooltip,
                close_buttons,
                registered_ids,
            },
        );
        self.opened = (self.opened + 1) % PIN_CASCADE_MODULUS;
        Ok(PinOutcome::Pinned)
    }

    /// Closes the pinned tooltip of `key`. Returns whether one was open.
    pub fn unpin(&mut self, ui: &mut Ui, key: ExamKey) -> bool {
        match self.pins.remove(key) {
            Some(w) => {
                self.release_window(ui, &w);
                true
            }
            None => false,
        }
    }

    fn release_window(&self, ui: &mut Ui, w: &PinnedWindow) {
        let click = ui.vocab().msgs.button_click;
        for id in &w.registered_ids {
            ui.unregister_handler(*id, click);
        }
        if ui.exists(w.window) {
            ui.destroy(w.window);
        }
    }

    /// Forgets pinned windows that were destroyed elsewhere (a closed panel) and removes
    /// their close buttons' game-code handlers.
    pub fn prune(&mut self, ui: &mut Ui) {
        let gone = self.pins.remove_where(|_, w| !ui.exists(w.window));
        for w in gone {
            self.release_window(ui, &w);
        }
    }

    /// Handles a UI event: a `Button_click` of a pinned window's close button closes that
    /// window. Returns whether the event was used.
    pub fn handle_event(&mut self, ui: &mut Ui, ev: &UiEvent) -> bool {
        let UiEvent::Game { message } = ev else {
            return false;
        };
        if message.msg != ui.vocab().msgs.button_click {
            return false;
        }
        let key = self
            .pins
            .iter()
            .find(|(_, w)| w.close_buttons.contains(&message.source))
            .map(|(k, _)| *k);
        match key {
            Some(k) => self.unpin(ui, k),
            None => false,
        }
    }

    // ----- building ------------------------------------------------------------------

    /// The tooltip's list box: the first element below the frame of type
    /// `ExaminationListBox`, else the first with entry templates.
    fn find_list_box(&self, ui: &Ui, frame: ElementKey) -> Option<ElementKey> {
        let t = self.vocab.types.exam_list_box;
        let lb = ui.vocab().pids.lb_templates;
        let below: Vec<ElementKey> = subtree(ui, frame).into_iter().skip(1).collect();
        below
            .iter()
            .copied()
            .find(|k| ui.element_type(*k) == Some(t))
            .or_else(|| below.into_iter().find(|k| ui.prop(*k, lb).is_set()))
    }

    /// The entry templates of a list (`UICore_ListBox_entry_templates`), each as a
    /// (layout, element) pair, `None` for a malformed one.
    fn templates_of(ui: &Ui, list: ElementKey) -> Vec<Option<ExamTemplate>> {
        let p = ui.vocab().pids;
        let Some(v) = ui.prop(list, p.lb_templates).value() else {
            return Vec::new();
        };
        array_elements(v)
            .iter()
            .take(MAX_ROWS)
            .map(|e| {
                let layout = struct_field(&e.value, p.lb_template_layout).map(as_key)?;
                let element = struct_field(&e.value, p.lb_template_element).map(as_key)?;
                Some(ExamTemplate { layout, element })
            })
            .collect()
    }

    /// The width limit of a list box: its `max_width`, else its design width (§4.1 OPEN,
    /// Tessera rule).
    fn width_limit(ui: &Ui, list: ElementKey) -> i32 {
        let mw = ui.prop(list, ui.vocab().pids.max_width);
        if mw.is_set() {
            mw.int().max(0)
        } else {
            ui.design_box(list).map_or(0, |b| b.w().max(0))
        }
    }

    fn fill_list(
        &self,
        ui: &mut Ui,
        frame: ElementKey,
        list: ElementKey,
        input: &ExamTooltip,
        removed: &mut Vec<ElementKey>,
    ) -> BuiltTooltip {
        // A list box created as an engine ListBox may have autofilled rows; start empty.
        ui.list_clear(list);
        let templates: Vec<Option<ExamTemplate>> = match &input.templates {
            Some(t) => t.iter().take(MAX_ROWS).copied().map(Some).collect(),
            None => Self::templates_of(ui, list),
        };
        let limit = Self::width_limit(ui, list);
        let mut failures = Vec::new();
        let mut rows: Vec<(ElementKey, Filled)> = Vec::new();
        let mut row_template: Vec<usize> = Vec::new();
        let empty = ExamRow::default();
        for (i, t) in templates.iter().enumerate() {
            let Some(t) = t else {
                failures.push((i, "malformed entry template".to_string()));
                continue;
            };
            let k = match ui.create_element(Some(list), Did(t.layout), t.element) {
                Ok(k) => k,
                Err(e) => {
                    failures.push((i, e.to_string()));
                    continue;
                }
            };
            let row = input.rows.get(i).unwrap_or(&empty);
            let filled = self.fill_row(ui, k, row, limit, 0);
            rows.push((k, filled));
            row_template.push(i);
        }
        let lb = ui.layout_box(list).unwrap_or_default();
        let min_w = {
            let m = ui.prop(list, ui.vocab().pids.min_width);
            if m.is_set() { m.int() } else { 0 }
        };
        let asm_rows: Vec<AsmRow> = rows.iter().map(|(_, f)| f.asm.clone()).collect();
        let a = assemble(&asm_rows, lb.w(), min_w, limit);
        let height = kept_height(&rows, &a, 0);
        // Step 7: the list box and its frame at the resulting size.
        let fb = ui.layout_box(frame).unwrap_or_default();
        let (lx, ly) = (lb.l.max(0), lb.t.max(0));
        let fw = a.width.saturating_add(lx.saturating_mul(2));
        let fh = height.saturating_add(ly.saturating_mul(2));
        ui.set_property(
            frame,
            ui.vocab().pids.object_mode,
            PropertyValue::Enum(object_mode::CANVAS_RESIZE),
        );
        ui.set_layout_box(frame, IBox::from_xywh(fb.l, fb.t, fw, fh));
        ui.set_layout_box(list, IBox::from_xywh(lx, ly, a.width, height));
        // The rows last: the frame's relayout may move children by their edge modes.
        // §5 step 1's removals go to the caller, destroyed in one batch (one pass over the
        // UI, not one per row).
        let kept = self.apply_assembly(ui, &rows, &a, 0, removed);
        let mut by_template = vec![None; templates.len()];
        for ((k, _), t) in rows.iter().zip(&row_template) {
            if kept.contains(k)
                && let Some(slot) = by_template.get_mut(*t)
            {
                *slot = Some(*k);
            }
        }
        BuiltTooltip {
            frame,
            list,
            size: (fw, fh),
            rows: kept,
            failures,
            by_template,
        }
    }

    /// Applies §5 to the rows: collects the removed ones in `removed` (the caller
    /// destroys them in one batch), stacks the kept ones from the top, stretches the wide
    /// ones. Returns the kept rows.
    fn apply_assembly(
        &self,
        ui: &mut Ui,
        rows: &[(ElementKey, Filled)],
        a: &Assembly,
        depth: usize,
        removed: &mut Vec<ElementKey>,
    ) -> Vec<ElementKey> {
        let kept_set: std::collections::HashSet<usize> = a.kept.iter().copied().collect();
        for (i, (k, _)) in rows.iter().enumerate() {
            if !kept_set.contains(&i) {
                removed.push(*k);
            }
        }
        let mut y = 0i32;
        let mut kept = Vec::with_capacity(a.kept.len());
        for (i, nested) in a.kept.iter().zip(&a.nested) {
            let Some((k, f)) = rows.get(*i) else { continue };
            let (mut w, mut h) = (f.asm.width, f.asm.height);
            if let Some(n) = nested.as_ref().filter(|_| depth < MAX_ROW_DEPTH) {
                let _ = self.apply_assembly(ui, &f.children, n, depth + 1, removed);
                w = n.width;
                h = kept_height(&f.children, n, depth + 1);
            }
            if a.stretched.contains(i) {
                w = a.width;
                self.stretch_pair(ui, *k, w);
            }
            ui.set_layout_box(*k, IBox::from_xywh(0, y, w.max(0), h.max(0)));
            if a.stretched.contains(i) {
                self.stretch_pair(ui, *k, w);
            }
            y = y.saturating_add(h.max(0));
            kept.push(*k);
        }
        kept
    }

    /// Puts a two-column row's right child at the row's right edge (§4.3).
    fn stretch_pair(&self, ui: &mut Ui, k: ElementKey, width: i32) {
        let p = self.vocab.pids;
        let rid = ui.prop(k, p.row_right).key();
        if rid == 0 {
            return;
        }
        if let Some(r) = find_below(ui, k, rid) {
            let b = ui.layout_box(r).unwrap_or_default();
            ui.set_layout_box(
                r,
                IBox::from_xywh(width.saturating_sub(b.w()).max(0), b.t, b.w(), b.h()),
            );
        }
    }

    /// Fills one row element from `row` and measures it.
    fn fill_row(
        &self,
        ui: &mut Ui,
        k: ElementKey,
        row: &ExamRow,
        wrap: i32,
        depth: usize,
    ) -> Filled {
        let p = self.vocab.pids;
        let design = ui.design_box(k).unwrap_or_default();
        let stretch = ui.prop(k, p.stretch_wide).bool();
        let remove_hidden = ui.prop(k, p.remove_when_hidden).bool();
        if let Some(s) = row.state {
            ui.switch_state_if_defined(k, s);
        }
        if row.hidden {
            ui.set_shown(k, false);
        }
        let separator =
            ui.prop(k, p.separator).bool() || matches!(row.content, RowContent::Separator);
        let mut f = if separator {
            Filled {
                asm: AsmRow::separator(design.w().max(0), design.h().max(0)),
                children: Vec::new(),
            }
        } else if depth >= MAX_ROW_DEPTH {
            Filled::sized(0, 0)
        } else {
            match &row.content {
                RowContent::Empty | RowContent::Separator => self.fill_empty(ui, k, row, wrap),
                RowContent::Text(t) => self.fill_text_row(ui, k, t, row.colour, wrap),
                RowContent::Image(img) => {
                    super::super::set_image_ref(ui, k, img, DRAW_OVER);
                    let (w, h) = image_size(ui, img).unwrap_or((design.w(), design.h()));
                    Filled::sized(w, h)
                }
                RowContent::Icon { image, ghosted } => {
                    self.fill_icon(ui, k, image.as_ref(), *ghosted);
                    Filled::sized(design.w(), design.h())
                }
                RowContent::Money { label, coins } => {
                    self.fill_money(ui, k, label.as_deref(), coins, row.colour, wrap)
                }
                RowContent::Lines { header, lines } => {
                    self.fill_lines(ui, k, header.as_ref(), lines, row.colour, wrap)
                }
                RowContent::Pair { left, right } => self.fill_pair(ui, k, left, right, wrap, depth),
                RowContent::List(rows) => self.fill_nested(ui, k, rows, wrap, depth),
            }
        };
        f.asm.stretch_wide = stretch;
        f.asm.remove_when_hidden = remove_hidden;
        f.asm.hidden = row.hidden;
        f.asm.separator = separator;
        f
    }

    /// A row without content. An examination widget shows nothing (§5 step 1: filling a
    /// row with no content clears its text). A plain engine `Text` row has no provider
    /// and shows its own `UICore_Text_entry` (for example `CurrentlyEquipped_Label`, §4.2,
    /// §8.1), coloured as the input says.
    fn fill_empty(&self, ui: &mut Ui, k: ElementKey, row: &ExamRow, wrap: i32) -> Filled {
        if ui.element_type(k) == Some(ui.vocab().types.text) {
            let own = ui.text_of(k);
            if !own.is_empty() {
                self.set_colour(ui, k, row.colour);
                return Filled::from_size(measure(ui, k, &own, wrap));
            }
        }
        self.clear_row(ui, k);
        Filled::sized(0, 0)
    }

    /// Clears an empty row's text (§5 step 1: filling with no content clears the text).
    fn clear_row(&self, ui: &mut Ui, k: ElementKey) {
        if ui.class(k).is_some_and(WidgetClass::is_text) {
            ui.set_text(k, Some(String::new()));
        }
    }

    /// The text element of a row: the row itself when it is a Text, else its first Text
    /// descendant.
    pub(crate) fn text_target(ui: &Ui, k: ElementKey) -> Option<ElementKey> {
        subtree(ui, k)
            .into_iter()
            .find(|c| ui.class(*c).is_some_and(WidgetClass::is_text))
    }

    fn fill_text_row(
        &self,
        ui: &mut Ui,
        k: ElementKey,
        text: &str,
        colour: Option<u32>,
        wrap: i32,
    ) -> Filled {
        if text.is_empty() {
            self.clear_row(ui, k);
            return Filled::sized(0, 0);
        }
        let Some(t) = Self::text_target(ui, k) else {
            tracing::debug!("examination row without a text element; text not shown");
            let d = ui.design_box(k).unwrap_or_default();
            return Filled::sized(d.w(), d.h());
        };
        self.set_colour(ui, t, colour);
        ui.set_text(t, Some(text.to_string()));
        let fit = ui.element_type(t) == Some(self.vocab.types.exam_name);
        let b = ui.layout_box(t).unwrap_or_default();
        // The name (`Name_Base`, fit mode `Desc`) wraps within its own layout width wherever
        // it sits (§5 and §7 answers, IT25; Tessera rule of Q22). Another text below the row
        // wraps within what is left of the limit after its offset.
        let wrap = match fit.then(|| Self::layout_width(ui, t)).flatten() {
            Some(own) => own,
            None if t != k => wrap.saturating_sub(b.l.max(0)).max(1),
            None => wrap,
        };
        let (w, h) = if fit {
            self.fit_fonts(ui, t, text, wrap)
        } else {
            measure(ui, t, text, wrap)
        };
        if t != k {
            ui.set_layout_box(t, IBox::from_xywh(b.l, b.t, w, h));
            return Filled::sized(b.l.max(0).saturating_add(w), b.t.max(0).saturating_add(h));
        }
        Filled::sized(w, h)
    }

    /// Sets the text colour the client's shaper reads.
    fn set_colour(&self, ui: &mut Ui, k: ElementKey, colour: Option<u32>) {
        if let Some(c) = colour {
            ui.set_property(
                k,
                self.vocab.pids.text_font_color,
                PropertyValue::Colour(c.to_le_bytes()),
            );
        }
    }

    /// The name row's font fit (§6.2 step 3, IT8): when the text needs more height than
    /// the row's designed height, it is laid out again with each font of
    /// `UICore_Text_fonts` in order until one fits; if none fits the last stays.
    fn fit_fonts(&self, ui: &mut Ui, k: ElementKey, text: &str, wrap: i32) -> (i32, i32) {
        let limit = ui.design_box(k).map_or(0, |b| b.h());
        let first = measure(ui, k, text, wrap);
        if first.1 <= limit {
            return first;
        }
        let fonts_pid = ui.vocab().pids.text_fonts;
        let fonts: Vec<u32> = ui
            .prop(k, fonts_pid)
            .value()
            .map(|v| {
                array_elements(v)
                    .iter()
                    .map(|e| as_key(&e.value))
                    .filter(|d| *d != 0)
                    .take(16)
                    .collect()
            })
            .unwrap_or_default();
        let mut last = first;
        for font in fonts {
            let d = PropertyValue::DidRef(Did(font));
            ui.set_property(k, self.vocab.pids.text_font, d.clone());
            ui.set_property(
                k,
                fonts_pid,
                PropertyValue::Array(PropertyArray {
                    count: 1,
                    elements: vec![PropertyEntry {
                        pid: self.vocab.pids.text_font,
                        value: d,
                    }],
                }),
            );
            last = measure(ui, k, text, wrap);
            if last.1 <= limit {
                break;
            }
        }
        last
    }

    fn fill_icon(&self, ui: &mut Ui, k: ElementKey, image: Option<&IconImage>, ghosted: bool) {
        let t = self.vocab.types.exam_icon;
        let target = subtree(ui, k)
            .into_iter()
            .find(|c| ui.element_type(*c) == Some(t))
            .unwrap_or(k);
        super::super::set_icon(ui, target, image);
        if ghosted {
            ui.switch_state_if_defined(k, self.vocab.states.ghosted);
            if target != k {
                ui.switch_state_if_defined(target, self.vocab.states.ghosted);
            }
        }
    }

    /// A code-made Text element below `parent`, styled like `style` (its descriptor
    /// without children, at (0, 0)).
    fn code_text(
        ui: &mut Ui,
        parent: ElementKey,
        style: Option<Arc<ElementDesc>>,
    ) -> Option<ElementKey> {
        let mut desc = style.map(|d| (*d).clone()).unwrap_or_default();
        desc.children.clear();
        desc.children_size.count = 0;
        desc.states.clear();
        desc.states_size.count = 0;
        desc.element_id = 0;
        desc.block.rect.x = 0;
        desc.block.rect.y = 0;
        desc.block.has_position = true;
        desc.block.media.clear();
        code_element(ui, parent, WidgetClass::Text, desc)
    }

    fn fill_lines(
        &self,
        ui: &mut Ui,
        k: ElementKey,
        header: Option<&ExamLine>,
        lines: &[ExamLine],
        colour: Option<u32>,
        wrap: i32,
    ) -> Filled {
        // Lines of an earlier fill are gone: the row is new. The line style: the row's
        // first entry template when it has one, else the row itself.
        let template = Self::templates_of(ui, k).into_iter().flatten().next();
        let style = ui.el(k).map(|e| e.desc.clone());
        let mut y = 0i32;
        let mut w = 0i32;
        let all = header
            .into_iter()
            .chain(lines.iter())
            .take(MAX_ROWS)
            .filter(|l| !l.text.is_empty());
        for line in all {
            let made = template.and_then(|t| {
                let e = ui.create_element(Some(k), Did(t.layout), t.element).ok()?;
                if ui.class(e).is_some_and(WidgetClass::is_text) {
                    Some(e)
                } else {
                    ui.destroy(e);
                    None
                }
            });
            let Some(e) = made.or_else(|| Self::code_text(ui, k, style.clone())) else {
                break;
            };
            if let Some(s) = line.state {
                ui.switch_state_if_defined(e, s);
            }
            self.set_colour(ui, e, line.colour.or(colour));
            ui.set_text(e, Some(line.text.clone()));
            let (lw, lh) = measure(ui, e, &line.text, wrap);
            ui.set_layout_box(e, IBox::from_xywh(0, y, lw, lh));
            y = y.saturating_add(lh);
            w = w.max(lw);
        }
        Filled::sized(w, y)
    }

    fn fill_money(
        &self,
        ui: &mut Ui,
        k: ElementKey,
        label: Option<&str>,
        coins: &[ExamCoin],
        colour: Option<u32>,
        wrap: i32,
    ) -> Filled {
        // §6.11 widget: no coins, nothing to show.
        if coins.is_empty() {
            return Filled::sized(0, 0);
        }
        let style = ui.el(k).map(|e| e.desc.clone());
        let mut parts: Vec<(ElementKey, i32, i32)> = Vec::new();
        let texts = label
            .filter(|l| !l.is_empty())
            .map(|l| (l, None))
            .into_iter()
            .chain(
                coins
                    .iter()
                    .take(MAX_ROWS)
                    .map(|c| (c.text.as_str(), Some(c))),
            );
        for (text, coin) in texts {
            let Some(e) = Self::code_text(ui, k, style.clone()) else {
                break;
            };
            self.set_colour(ui, e, colour);
            ui.set_text(e, Some(text.to_string()));
            let (w, th) = measure(ui, e, text, wrap);
            parts.push((e, w, th));
            let Some(c) = coin else { continue };
            let (iw, ih) = c
                .icon
                .as_ref()
                .and_then(|i| image_size(ui, i))
                .unwrap_or((COIN_ICON, COIN_ICON));
            let Some(icon) = code_element(ui, k, WidgetClass::Field, sized_desc(iw, ih)) else {
                break;
            };
            if let Some(img) = &c.icon {
                super::super::set_image_ref(ui, icon, img, DRAW_OVER);
            }
            parts.push((icon, iw, ih));
        }
        let mut h = 0i32;
        for (_, _, ph) in &parts {
            h = h.max(*ph);
        }
        // Lay the parts out on one line, centred vertically.
        let mut px = 0i32;
        for (e, w, ph) in &parts {
            let top = (h - ph) / 2;
            ui.set_layout_box(*e, IBox::from_xywh(px, top, *w, *ph));
            px = px.saturating_add(*w).saturating_add(MONEY_GAP);
        }
        Filled::sized(px.saturating_sub(MONEY_GAP).max(0), h)
    }

    fn fill_pair(
        &self,
        ui: &mut Ui,
        k: ElementKey,
        left: &ExamRow,
        right: &ExamRow,
        wrap: i32,
        depth: usize,
    ) -> Filled {
        let p = self.vocab.pids;
        let side = |pid: u32, row: &ExamRow, ui: &mut Ui| -> Option<(ElementKey, i32, i32)> {
            let id = ui.prop(k, pid).key();
            let c = (id != 0).then(|| find_below(ui, k, id)).flatten()?;
            let f = self.fill_row(ui, c, row, wrap, depth + 1);
            Some((c, f.asm.width, f.asm.height))
        };
        let l = side(p.row_left, left, ui);
        let r = side(p.row_right, right, ui);
        let lw = l.map_or(0, |s| if s.1 > 0 && s.2 > 0 { s.1 } else { 0 });
        let rw = r.map_or(0, |s| if s.1 > 0 && s.2 > 0 { s.1 } else { 0 });
        let h = l
            .map_or(0, |s| if s.1 > 0 { s.2 } else { 0 })
            .max(r.map_or(0, |s| if s.1 > 0 { s.2 } else { 0 }));
        if let Some((c, _, ch)) = l {
            ui.set_layout_box(c, IBox::from_xywh(0, 0, lw, ch.max(0)));
        }
        if let Some((c, _, ch)) = r {
            ui.set_layout_box(c, IBox::from_xywh(lw, 0, rw, ch.max(0)));
        }
        Filled::sized(lw.saturating_add(rw), h)
    }

    fn fill_nested(
        &self,
        ui: &mut Ui,
        k: ElementKey,
        rows: &[ExamRow],
        wrap: i32,
        depth: usize,
    ) -> Filled {
        let horizontal = ui.prop(k, ui.vocab().pids.lb_horizontal).bool();
        let templates = Self::templates_of(ui, k);
        let empty = ExamRow::default();
        if horizontal {
            // The list grows sideways to fit its non-empty entries (§4.3). Each entry wraps
            // by its own fit mode (§7 answer, IT25): the name (`Name_Base`, mode `Desc`)
            // within its own layout width (Tessera rule of Q22: 180 here, less its
            // margins), whatever the entries before it; any other entry within what is left
            // of the width limit after the entries before it.
            let mut x = 0i32;
            let mut h = 0i32;
            let mut gone = Vec::new();
            for (i, t) in templates.iter().enumerate() {
                let Some(t) = t else { continue };
                let Ok(c) = ui.create_element(Some(k), Did(t.layout), t.element) else {
                    continue;
                };
                let row = rows.get(i).unwrap_or(&empty);
                let own = self.own_wrap(ui, c);
                let left = own.unwrap_or_else(|| wrap.saturating_sub(x).max(1));
                let f = self.fill_row(ui, c, row, left, depth + 1);
                if f.asm.width <= 0 || f.asm.height <= 0 {
                    gone.push(c);
                    continue;
                }
                let w = f.asm.width.min(left);
                ui.set_layout_box(c, IBox::from_xywh(x, 0, w, f.asm.height));
                x = x.saturating_add(w);
                h = h.max(f.asm.height);
            }
            ui.destroy_many(&gone);
            return Filled::sized(x, h);
        }
        let mut children: Vec<(ElementKey, Filled)> = Vec::new();
        for (i, t) in templates.iter().enumerate() {
            let Some(t) = t else { continue };
            let Ok(c) = ui.create_element(Some(k), Did(t.layout), t.element) else {
                continue;
            };
            let row = rows.get(i).unwrap_or(&empty);
            let f = self.fill_row(ui, c, row, wrap, depth + 1);
            children.push((c, f));
        }
        let width = children.iter().map(|(_, f)| f.asm.width).max().unwrap_or(0);
        let mut f = Filled::sized(width, children.iter().map(|(_, f)| f.asm.height).sum());
        f.asm.list = Some(children.iter().map(|(_, c)| c.asm.clone()).collect());
        f.children = children;
        f
    }
}

impl ExamTooltips {
    /// The wrap width of an entry of a horizontal list that wraps by its own width (§7
    /// answer, IT25): the name entry (an `ExaminationName` text, `Name_Base`, whose fit mode
    /// is `Desc`) wraps within its layout width (Tessera rule of `item-tooltips.md` Q22);
    /// `None` for any other entry, or a name without a positive width.
    ///
    /// The element's `UICore_Text_fit_to_text` mode is not read (the vocabularies carry no
    /// PID for it): the `Desc` rule is applied to `ExaminationName` texts, every other text
    /// keeps the list's limit (the `Parent` reading; implementer OPEN).
    fn own_wrap(&self, ui: &Ui, entry: ElementKey) -> Option<i32> {
        let name = self.vocab.types.exam_name;
        let t = Self::text_target(ui, entry)?;
        if ui.element_type(t) != Some(name) && ui.element_type(entry) != Some(name) {
            return None;
        }
        Self::layout_width(ui, entry)
    }

    /// `k`'s layout width (its design width before layout); `None` when not positive.
    fn layout_width(ui: &Ui, k: ElementKey) -> Option<i32> {
        let w = ui
            .layout_box(k)
            .or_else(|| ui.design_box(k))
            .map_or(0, |b| b.w());
        (w > 0).then_some(w)
    }
}

/// The height of the rows `a` keeps, nested lists by their own assembly.
fn kept_height(rows: &[(ElementKey, Filled)], a: &Assembly, depth: usize) -> i32 {
    let mut y = 0i32;
    for (i, nested) in a.kept.iter().zip(&a.nested) {
        let Some((_, f)) = rows.get(*i) else { continue };
        let h = match nested {
            Some(n) if depth < MAX_ROW_DEPTH => kept_height(&f.children, n, depth + 1),
            _ => f.asm.height,
        };
        y = y.saturating_add(h.max(0));
    }
    y
}

/// A descriptor of `w` × `h` at (0, 0) with nothing else.
fn sized_desc(w: i32, h: i32) -> ElementDesc {
    let mut d = ElementDesc::default();
    d.block.rect.width = w.max(0);
    d.block.rect.height = h.max(0);
    d.block.has_position = true;
    d
}

/// A code-made element of `class` from `desc`, attached to `parent`.
pub(crate) fn code_element(
    ui: &mut Ui,
    parent: ElementKey,
    class: WidgetClass,
    desc: ElementDesc,
) -> Option<ElementKey> {
    if ui.element_count() >= crate::ui::MAX_ELEMENTS || ui.depth(parent) >= MAX_TREE_DEPTH {
        return None;
    }
    let mut e = Element::new(desc.element_id, class, Arc::new(desc));
    e.effective = Ui::compute_effective(&e);
    e.created = true;
    let k = ui.insert_element(e);
    if ui.attach_raw(k, parent).is_err() {
        ui.destroy(k);
        return None;
    }
    ui.recompute_mouse_visible(k);
    Some(k)
}

/// The decoded size of an image, when it is loaded.
fn image_size(ui: &mut Ui, img: &ImageRef) -> Option<(i32, i32)> {
    match ui.env.image(img) {
        ImageState::Ready(i) => Some((
            i32::try_from(i.width()).unwrap_or(i32::MAX),
            i32::try_from(i.height()).unwrap_or(i32::MAX),
        )),
        _ => None,
    }
}

/// Measures `text` in element `k` with wrap width `wrap` (`fonts.md` §7; the extent
/// includes the element's margins). Without shaped text (fonts not loaded) the element's
/// designed size stands in.
fn measure(ui: &mut Ui, k: ElementKey, text: &str, wrap: i32) -> (i32, i32) {
    let props = ui.properties(k);
    let b = ui.layout_box(k).unwrap_or_default();
    let req = TextRequest {
        text,
        props: &props,
        size: (b.w(), b.h()),
        wrap_width: Some(wrap.max(1)),
    };
    match ui.env.shape_text(&req) {
        Some(p) => (p.extent.0.clamp(0, wrap.max(1)), p.extent.1.max(0)),
        None => {
            let d = ui.design_box(k).unwrap_or(b);
            (d.w().max(0), d.h().max(0))
        }
    }
}
