// SPDX-License-Identifier: GPL-3.0-or-later
//! The vendor's quantity dialog (`npc-windows.md` §7.5): presets, arrows, typing, the
//! clamp to 1 … *M*, OK and cancel, and the cost display (unit coin price × quantity).

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;

use super::super::npcvocab::NpcVocab;
use super::super::{IconImage, set_icon};
use super::coins::{MoneyDisplay, coin_counts};
use super::kit::{ClickKind, FallbackArt, Listen, click_of, find, icon_part, put_text, text_part};
use super::{NpcAction, NpcTexts, WindowId};

/// Most characters the entry holds (`VendorBuyStack_EntryText`: at most 5).
pub const MAX_ENTRY_CHARS: usize = 5;

/// Gives the dialog's root at least its minimum size (`npc-windows.md` §4: min 256 × 164).
///
/// Tessera rule (OPEN in `npc-windows.md` §4): laid out by its edges from the layout's
/// design size (`ui-layout.md` §8.4), the root of `vendor_buy_stack` can come out with a
/// negative height on screens shorter than the design; the element resize of
/// `ui-behaviour.md` §9.2.1 (limit pass, UB47) then brings it to its minimum, and the
/// dialog is kept on screen.
fn fit_to_limits(ui: &mut Ui, root: ElementKey) {
    let Some(b) = ui.layout_box(root) else { return };
    let p = ui.vocab().pids;
    let (minw, minh) = (ui.prop(root, p.min_width), ui.prop(root, p.min_height));
    let short_w = minw.is_set() && b.w() < minw.int();
    let short_h = minh.is_set() && b.h() < minh.int();
    if !(short_w || short_h || b.w() <= 0 || b.h() <= 0) {
        return;
    }
    ui.resize(root, b.w().max(1), b.h().max(1));
    let nb = ui.layout_box(root).unwrap_or(b);
    let (sw, sh) = ui.screen_size();
    let x = nb.l.min(sw.saturating_sub(nb.w())).max(0);
    let y = nb.t.min(sh.saturating_sub(nb.h())).max(0);
    if (x, y) != (nb.l, nb.t) {
        ui.move_to(root, x, y);
    }
}

/// The dialog's state (§7.5 rules 1–5). *M* is the maximum it was opened with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantityModel {
    max: u32,
    entry: String,
}

/// What OK does (§7.5 rule 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    /// The purchase is made with this quantity and the dialog closes.
    Buy(u32),
    /// The value was corrected (0 → 1, above *M* → *M*); the dialog stays open.
    Corrected,
}

impl QuantityModel {
    /// A dialog with maximum `max` (at least 1). **Tessera rule:** it opens with
    /// quantity 1 (the original's initial value was not traced).
    #[must_use]
    pub fn new(max: u32) -> Self {
        let mut m = Self {
            max: max.max(1),
            entry: String::new(),
        };
        m.set(1);
        m
    }

    /// *M*.
    #[must_use]
    pub fn max(&self) -> u32 {
        self.max
    }

    /// The entry's text.
    #[must_use]
    pub fn entry(&self) -> &str {
        &self.entry
    }

    /// Rule 1: clamps `q` to 1 … *M* and writes it to the entry as a decimal integer.
    pub fn set(&mut self, q: i64) {
        let q = q.clamp(1, i64::from(self.max));
        self.entry = q.to_string();
    }

    /// The entry as a number, when it is one (decimal digits only).
    #[must_use]
    pub fn value(&self) -> Option<u64> {
        if self.entry.is_empty() || !self.entry.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        Some(self.entry.parse::<u64>().unwrap_or(u64::MAX))
    }

    /// Rule 3's current value: the entry's number, 1 when it is not numeric.
    #[must_use]
    pub fn current(&self) -> i64 {
        self.value()
            .map_or(1, |v| i64::try_from(v).unwrap_or(i64::MAX))
    }

    /// The quantity the cost display shows: the entry's value clamped to 1 … *M* (1 for a
    /// non-numeric entry).
    #[must_use]
    pub fn quantity(&self) -> u32 {
        let v = self.current().clamp(1, i64::from(self.max));
        u32::try_from(v).unwrap_or(self.max)
    }

    /// Rule 2: Full sets *M*.
    pub fn full(&mut self) {
        self.set(i64::from(self.max));
    }

    /// Rule 2: Half sets ⌊*M* × 0.5⌋ (then clamped, so at least 1).
    pub fn half(&mut self) {
        self.set(i64::from(self.max / 2));
    }

    /// Rule 2: the third preset sets 1.
    pub fn one(&mut self) {
        self.set(1);
    }

    /// Rule 3: the current value minus 1.
    pub fn decrease(&mut self) {
        self.set(self.current().saturating_sub(1));
    }

    /// Rule 3: the current value plus 1.
    pub fn increase(&mut self) {
        self.set(self.current().saturating_add(1));
    }

    /// Rule 4: typing. The entry keeps the typed text (at most [`MAX_ENTRY_CHARS`]
    /// characters); a numeric text sets the cost's quantity ([`QuantityModel::quantity`])
    /// without rewriting the entry, so that OK can still correct it (the §13.2 example:
    /// typing 80 with *M* = 50, then OK, clamps and stays open).
    pub fn type_text(&mut self, text: &str) {
        self.entry = text.chars().take(MAX_ENTRY_CHARS).collect();
    }

    /// Rule 5: OK (or `AcceptInput`). A value of 0 (or a non-numeric entry, Tessera) is
    /// set to 1 and the dialog stays open; a value above *M* is clamped and it stays open;
    /// otherwise the purchase is made with that quantity (never outside 1 … *M*, NWI8).
    pub fn confirm(&mut self) -> Confirm {
        match self.value() {
            None | Some(0) => {
                self.set(1);
                Confirm::Corrected
            }
            Some(v) if v > u64::from(self.max) => {
                self.set(i64::from(self.max));
                Confirm::Corrected
            }
            Some(v) => Confirm::Buy(u32::try_from(v).unwrap_or(self.max)),
        }
    }
}

/// What the dialog buys: the vendor window, the entry and the unit price.
#[derive(Debug, Clone, PartialEq)]
pub struct QuantityRequest {
    /// The vendor window.
    pub window: WindowId,
    /// The entry number.
    pub entry: u16,
    /// *M* (the item's stack size).
    pub max: u32,
    /// The unit coin price (units of the value-1 currency).
    pub unit_price: u64,
    /// The currency values for splitting the cost (§9.3).
    pub coin_values: Vec<u64>,
    /// The item's name.
    pub name: String,
    /// The item's icon.
    pub icon: Option<IconImage>,
}

/// The quantity dialog window (`VendorBuyStackField`).
#[derive(Debug, Clone)]
pub struct QuantityDialog {
    /// The root.
    pub root: ElementKey,
    /// The model.
    pub model: QuantityModel,
    /// Whether the dialog is open (OK with a purchase and cancel close it).
    pub open: bool,
    /// `VendorBuyStack_EntryText`.
    pub entry: Option<ElementKey>,
    /// The cost's money display.
    pub cost: Option<MoneyDisplay>,
    parts: [Option<ElementKey>; 7],
    item_name: Option<ElementKey>,
    item_icon: Option<ElementKey>,
    request: Option<QuantityRequest>,
    listen: Listen,
}

/// What a UI event did to the quantity dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityOutcome {
    /// OK (or Enter) made the purchase; the dialog closed. The client sends the command.
    Buy(NpcAction),
    /// The close button closed the dialog without buying; the client hides it.
    Closed,
}

/// Indices into [`QuantityDialog::parts`].
const DEC: usize = 0;
const INC: usize = 1;
const FULL: usize = 2;
const HALF: usize = 3;
const ONE: usize = 4;
const OK: usize = 5;
const CANCEL: usize = 6;

impl QuantityDialog {
    /// Finds the dialog's parts below `root` and builds the cost's coin elements.
    pub fn attach(ui: &mut Ui, nv: &NpcVocab, root: ElementKey, art: &FallbackArt) -> Self {
        let ids = nv.ids;
        fit_to_limits(ui, root);
        let f = |ui: &Ui, id: u32| find(ui, root, id);
        let parts = [
            f(ui, ids.quantity_decrease),
            f(ui, ids.quantity_increase),
            f(ui, ids.quantity_full),
            f(ui, ids.quantity_half),
            f(ui, ids.quantity_zero),
            f(ui, ids.quantity_ok),
            f(ui, ids.quantity_cancel),
        ];
        let cost = f(ui, ids.quantity_cost)
            .and_then(|c| MoneyDisplay::find(ui, nv, c))
            .map(|m| MoneyDisplay::attach(ui, nv, m, art));
        let item = f(ui, ids.quantity_item);
        let item_name = item.and_then(|i| text_part(ui, i, Some(ids.item_name)));
        let item_icon = item.and_then(|i| icon_part(ui, i, Some(ids.item_icon)));
        let mut d = Self {
            root,
            model: QuantityModel::new(1),
            open: false,
            entry: f(ui, ids.quantity_entry),
            cost,
            parts,
            item_name,
            item_icon,
            request: None,
            listen: Listen::default(),
        };
        for p in d.parts {
            d.listen.clicks(ui, p);
        }
        if let Some(e) = d.entry {
            let mc = ui.vocab().pids.text_max_chars;
            ui.set_property(
                e,
                mc,
                tessera_formats::property::PropertyValue::Int(MAX_ENTRY_CHARS as i32),
            );
        }
        d
    }

    /// Opens the dialog for a purchase (§7.4 case 2) with *M* = the request's maximum.
    pub fn open(&mut self, ui: &mut Ui, request: QuantityRequest, texts: &dyn NpcTexts) {
        self.model = QuantityModel::new(request.max);
        put_text(ui, self.item_name, &request.name);
        if let Some(ic) = self.item_icon {
            set_icon(ui, ic, request.icon.as_ref());
        }
        self.request = Some(request);
        self.open = true;
        self.refresh(ui, texts);
    }

    /// Writes the entry and the cost (§7.5 Tessera rule: unit coin price × quantity).
    fn refresh(&mut self, ui: &mut Ui, texts: &dyn NpcTexts) {
        if let Some(e) = self.entry
            && ui.text_of(e) != self.model.entry()
        {
            ui.set_text(e, Some(self.model.entry().to_string()));
        }
        if let (Some(c), Some(r)) = (self.cost.as_mut(), self.request.as_ref()) {
            let total = r
                .unit_price
                .saturating_mul(u64::from(self.model.quantity()));
            c.set(ui, coin_counts(total, &r.coin_values), false, texts);
        }
    }

    /// Reads the entry's typed text into the model (rule 4) when it differs from the
    /// model's entry.
    fn sync_entry(&mut self, ui: &mut Ui, texts: &dyn NpcTexts) {
        if let Some(e) = self.entry {
            let typed = ui.text_of(e);
            if typed != self.model.entry() {
                self.model.type_text(&typed);
                self.refresh(ui, texts);
            }
        }
    }

    /// The outcome of a UI event: presets and arrows change the quantity (`None`), OK buys
    /// ([`QuantityOutcome::Buy`]; or corrects the value and stays open, `None`), the close
    /// button closes without buying ([`QuantityOutcome::Closed`]). The entry's typed text
    /// is read on every event (rule 4).
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        texts: &dyn NpcTexts,
    ) -> Option<QuantityOutcome> {
        if !self.open {
            return None;
        }
        self.sync_entry(ui, texts);
        let (src, kind) = click_of(ui, ev)?;
        if !matches!(kind, ClickKind::Button | ClickKind::Left) {
            return None;
        }
        let i = self
            .parts
            .iter()
            .position(|p| p.is_some_and(|p| ui.exists(src) && ui.is_ancestor_or_self(p, src)))?;
        match i {
            DEC => self.model.decrease(),
            INC => self.model.increase(),
            FULL => self.model.full(),
            HALF => self.model.half(),
            ONE => self.model.one(),
            OK => return self.confirm(ui, texts).map(QuantityOutcome::Buy),
            CANCEL => {
                self.open = false;
                return Some(QuantityOutcome::Closed);
            }
            _ => return None,
        }
        self.refresh(ui, texts);
        None
    }

    /// OK or `AcceptInput` (Enter) while the dialog has focus (rule 5): reads the entry's
    /// typed text first (rule 4), then buys with that quantity and closes, or corrects the
    /// value (0 → 1, above *M* → *M*) and stays open (`None`).
    pub fn confirm(&mut self, ui: &mut Ui, texts: &dyn NpcTexts) -> Option<NpcAction> {
        if !self.open {
            return None;
        }
        self.sync_entry(ui, texts);
        match self.model.confirm() {
            Confirm::Corrected => {
                self.refresh(ui, texts);
                None
            }
            Confirm::Buy(q) => {
                self.open = false;
                let r = self.request.as_ref()?;
                Some(NpcAction::Buy {
                    window: r.window,
                    entry: r.entry,
                    quantity: q,
                })
            }
        }
    }

    /// Closes the dialog without buying (its vendor window closed, §7.5 Tessera rule).
    pub fn cancel(&mut self) {
        self.open = false;
    }

    /// Removes the game-code handlers.
    pub fn detach(&mut self, ui: &mut Ui) {
        self.listen.release(ui);
    }
}
