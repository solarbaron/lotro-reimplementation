// SPDX-License-Identifier: GPL-3.0-or-later
//! The vendor's quantity dialog (`npc-windows.md` §7.5): presets, arrows, typing (each
//! character input re-reads the entry, clamps to 1 … *M* and rewrites it), OK and cancel,
//! the OK button's state, the opening quantity remembered per vendor, and the cost display
//! (unit coin price × quantity, `TooExpensive` while the player's coin is below it).

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::vocab::tessera_msg;

use super::super::npcvocab::NpcVocab;
use super::super::{IconImage, set_icon};
use super::coins::MoneyDisplay;
use super::kit::{
    ClickKind, FallbackArt, Listen, click_of, find, ghost, icon_part, put_text, text_part,
};
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

/// Reads an entry's text as a number (§7.5 rule 4): leading `0` characters are skipped,
/// then the value is the unsigned decimal number formed by the leading digits that follow;
/// a text that does not start with a digit reads as 0 (**Tessera rule** for a text that
/// starts with white space or a sign). Saturates instead of overflowing.
#[must_use]
pub fn read_number(text: &str) -> u64 {
    if !text.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        return 0;
    }
    text.trim_start_matches('0')
        .bytes()
        .take_while(u8::is_ascii_digit)
        .fold(0u64, |v, d| {
            v.saturating_mul(10).saturating_add(u64::from(d - b'0'))
        })
}

/// Which vendor a purchase through the dialog was made from (§7.5 rule 8, Tessera rule):
/// the vendor window's entity when the feed names one, else its window ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VendorKey {
    /// The vendor's entity.
    Entity(u64),
    /// The vendor window (no entity named).
    Window(WindowId),
}

/// The vendor and quantity of the last purchase made through the dialog (§7.5 rule 8).
/// It lasts for the client session and is not saved; closing the dialog keeps it, opening
/// the dialog for another vendor forgets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QuantityMemory {
    last: Option<(VendorKey, u32)>,
}

impl QuantityMemory {
    /// The quantity the dialog opens with for `vendor` and maximum `max` (rule 8): the
    /// remembered purchase's quantity (at most `max`) when it was made from the same
    /// vendor, else `max`; opening for another vendor forgets the remembered quantity.
    pub fn opening(&mut self, vendor: VendorKey, max: u32) -> u32 {
        match self.last {
            Some((v, q)) if v == vendor => q.min(max),
            _ => {
                self.last = None;
                max
            }
        }
    }

    /// Remembers a purchase of `quantity` from `vendor` (rule 5 step 4).
    pub fn remember(&mut self, vendor: VendorKey, quantity: u32) {
        self.last = Some((vendor, quantity));
    }

    /// The remembered vendor and quantity.
    #[must_use]
    pub fn last(&self) -> Option<(VendorKey, u32)> {
        self.last
    }
}

/// The dialog's state (§7.5 rules 1–5). *M* is the maximum it was opened with. The entry's
/// text is the quantity (the original keeps none apart from it); [`QuantityModel::quantity`]
/// is the quantity last set (rule 1), which the cost shows and which an empty entry keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantityModel {
    max: u32,
    entry: String,
    quantity: u32,
}

/// What OK does (§7.5 rule 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirm {
    /// The purchase is made with this quantity and the dialog closes (step 4).
    Buy(u32),
    /// The value was corrected (0 → 1, above *M* → *M*); the dialog stays open (step 2).
    Corrected,
    /// Nothing happens (an empty entry, step 1; the cost is `TooExpensive`, step 3).
    Ignored,
}

impl QuantityModel {
    /// A dialog with maximum `max` (at least 1), opened with quantity *M* (rule 8).
    #[must_use]
    pub fn new(max: u32) -> Self {
        let max = max.max(1);
        Self::with_quantity(max, max)
    }

    /// A dialog with maximum `max` (at least 1), opened with `quantity` (set by rule 1, so
    /// clamped to 1 … *M*).
    #[must_use]
    pub fn with_quantity(max: u32, quantity: u32) -> Self {
        let mut m = Self {
            max: max.max(1),
            entry: String::new(),
            quantity: 1,
        };
        m.set(i64::from(quantity));
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

    /// Whether the entry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entry.is_empty()
    }

    /// Rule 1: clamps `q` to 1 … *M*, writes it to the entry as a decimal integer and makes
    /// it the quantity the cost shows.
    pub fn set(&mut self, q: i64) {
        let q = q.clamp(1, i64::from(self.max));
        self.quantity = u32::try_from(q).unwrap_or(self.max);
        self.entry = self.quantity.to_string();
    }

    /// The entry's value ([`read_number`]); `None` for an empty entry.
    #[must_use]
    pub fn value(&self) -> Option<u64> {
        (!self.entry.is_empty()).then(|| read_number(&self.entry))
    }

    /// Rule 3's current value: the entry's value, 1 for an empty entry.
    #[must_use]
    pub fn current(&self) -> i64 {
        self.value()
            .map_or(1, |v| i64::try_from(v).unwrap_or(i64::MAX))
    }

    /// The quantity last set (rule 1): the one the cost display shows. An empty entry
    /// keeps it (rule 4).
    #[must_use]
    pub fn quantity(&self) -> u32 {
        self.quantity
    }

    /// The cost of [`QuantityModel::quantity`] at `unit_price` (§7.5 Tessera rule).
    #[must_use]
    pub fn cost(&self, unit_price: u64) -> u64 {
        unit_price.saturating_mul(u64::from(self.quantity))
    }

    /// Whether the player's coin is below the cost shown (`TooExpensive`); `false` before
    /// any `WalletChanged { Coin }` (`coin` `None`: no comparison, §7.5 Tessera rule).
    #[must_use]
    pub fn too_expensive(&self, unit_price: u64, coin: Option<u64>) -> bool {
        coin.is_some_and(|c| c < self.cost(unit_price))
    }

    /// Rule 7: the OK button is `ghosted` when the entry is empty or the player's coin is
    /// below the cost shown, `normal` otherwise.
    #[must_use]
    pub fn ok_ghosted(&self, unit_price: u64, coin: Option<u64>) -> bool {
        self.is_empty() || self.too_expensive(unit_price, coin)
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

    /// Rule 3: the entry's value minus 1 (an empty entry counts as 1).
    pub fn decrease(&mut self) {
        self.set(self.current().saturating_sub(1));
    }

    /// Rule 3: the entry's value plus 1 (an empty entry counts as 1).
    pub fn increase(&mut self) {
        self.set(self.current().saturating_add(1));
    }

    /// Rule 4: a character input changed the entry's whole text to `text` (at most
    /// [`MAX_ENTRY_CHARS`] characters are kept). A non-empty text is read
    /// ([`read_number`]) and set (rule 1: clamped, the entry rewritten at once); an empty
    /// text leaves the quantity unchanged and the entry empty. Afterwards the entry is
    /// empty or a decimal integer 1 … *M* without leading zeros (NWI9).
    pub fn type_text(&mut self, text: &str) {
        let text: String = text.chars().take(MAX_ENTRY_CHARS).collect();
        if text.is_empty() {
            self.entry.clear();
        } else {
            self.set(i64::try_from(read_number(&text)).unwrap_or(i64::MAX));
        }
    }

    /// Rule 5: OK (or `AcceptInput`). `too_expensive` is the cost display's state. An empty
    /// entry does nothing; a value of 0 is set to 1 and a value above *M* is clamped, the
    /// dialog staying open; while the cost is `TooExpensive` nothing happens; otherwise the
    /// purchase is made with the entry's value (never outside 1 … *M*, NWI8).
    pub fn confirm(&mut self, too_expensive: bool) -> Confirm {
        let Some(v) = self.value() else {
            return Confirm::Ignored;
        };
        if v == 0 || v > u64::from(self.max) {
            self.set(i64::try_from(v).unwrap_or(i64::MAX));
            return Confirm::Corrected;
        }
        if too_expensive {
            return Confirm::Ignored;
        }
        Confirm::Buy(u32::try_from(v).unwrap_or(self.max))
    }
}

/// What the dialog buys: the vendor window, the entry and the unit price.
#[derive(Debug, Clone, PartialEq)]
pub struct QuantityRequest {
    /// The vendor window.
    pub window: WindowId,
    /// The vendor (rule 8: the remembered quantity is per vendor).
    pub vendor: VendorKey,
    /// The entry number.
    pub entry: u16,
    /// *M* (the item's stack size).
    pub max: u32,
    /// The unit coin price (units of the value-1 currency).
    pub unit_price: u64,
    /// The currency values for splitting the cost (§9.3).
    pub coin_values: Vec<u64>,
    /// The player's coin (the last `WalletChanged { Coin }`; `None` before any).
    pub player_coin: Option<u64>,
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

/// Puts the caret of `k` after its last character (§7.5 Tessera rule after a rewrite).
fn caret_to_end(ui: &mut Ui, k: ElementKey) {
    let n = ui.text_of(k).encode_utf16().count();
    if let Some(e) = ui.el_mut(k) {
        e.text.caret = n;
    }
}

impl QuantityDialog {
    /// Finds the dialog's parts below `root`, builds the cost's coin elements and listens
    /// to the entry's character input (rule 4).
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
            // Rule 4: every character input (`Text_character_input`) re-reads the entry.
            // Tessera's text editing posts that message for inserted characters and its
            // text-changed notification for every edit (deletions included).
            let ci = ui.vocab().msgs.character_input;
            d.listen.on(ui, Some(e), ci);
            d.listen.on(ui, Some(e), tessera_msg::TEXT_CHANGED);
        }
        d
    }

    /// Opens the dialog for a purchase (§7.4 case 2) with *M* = the request's maximum and
    /// the opening quantity of rule 8 (`memory`). Opening it while it is open first closes
    /// the open one without buying.
    pub fn open(
        &mut self,
        ui: &mut Ui,
        request: QuantityRequest,
        memory: &mut QuantityMemory,
        texts: &dyn NpcTexts,
    ) {
        self.open = false;
        let start = memory.opening(request.vendor, request.max);
        self.model = QuantityModel::with_quantity(request.max, start);
        put_text(ui, self.item_name, &request.name);
        if let Some(ic) = self.item_icon {
            set_icon(ui, ic, request.icon.as_ref());
        }
        self.request = Some(request);
        self.open = true;
        self.refresh(ui, texts);
    }

    /// The player's coin changed (`WalletChanged { Coin }`): the cost display's
    /// `TooExpensive` state and the OK button's state are chosen again.
    pub fn set_player_coin(&mut self, ui: &mut Ui, coin: Option<u64>, texts: &dyn NpcTexts) {
        if let Some(r) = self.request.as_mut() {
            if r.player_coin == coin {
                return;
            }
            r.player_coin = coin;
        }
        if self.open {
            self.refresh(ui, texts);
        }
    }

    /// The cost shown (unit coin price × quantity; 0 before the dialog was opened).
    #[must_use]
    pub fn cost(&self) -> u64 {
        self.request
            .as_ref()
            .map_or(0, |r| self.model.cost(r.unit_price))
    }

    /// Whether the cost display is in state `TooExpensive`.
    #[must_use]
    pub fn too_expensive(&self) -> bool {
        self.request
            .as_ref()
            .is_some_and(|r| self.model.too_expensive(r.unit_price, r.player_coin))
    }

    /// Whether the OK button is `ghosted` (rule 7).
    #[must_use]
    pub fn ok_ghosted(&self) -> bool {
        match self.request.as_ref() {
            Some(r) => self.model.ok_ghosted(r.unit_price, r.player_coin),
            None => self.model.is_empty(),
        }
    }

    /// Writes the entry (the caret after its last character when rewritten), the cost
    /// (§7.5 Tessera rule: unit coin price × quantity, `TooExpensive` while the player's
    /// coin is below it) and the OK button's state (rule 7).
    fn refresh(&mut self, ui: &mut Ui, texts: &dyn NpcTexts) {
        if let Some(e) = self.entry
            && ui.text_of(e) != self.model.entry()
        {
            ui.set_text(e, Some(self.model.entry().to_string()));
            caret_to_end(ui, e);
        }
        if let (Some(c), Some(r)) = (self.cost.as_mut(), self.request.as_ref()) {
            c.set_cost(
                ui,
                self.model.cost(r.unit_price),
                &r.coin_values,
                r.player_coin,
                texts,
            );
        }
        ghost(ui, self.parts[OK], self.ok_ghosted());
    }

    /// Rule 4: reads the entry's text into the model when it differs from the model's
    /// entry (a character input changed it), then rewrites the entry, the cost and the OK
    /// state.
    fn sync_entry(&mut self, ui: &mut Ui, texts: &dyn NpcTexts) {
        if let Some(e) = self.entry {
            let typed = ui.text_of(e);
            if typed != self.model.entry() {
                self.model.type_text(&typed);
                self.refresh(ui, texts);
            }
        }
    }

    /// The outcome of a UI event: a character input re-reads the entry (rule 4), presets
    /// and arrows change the quantity (`None`), OK buys ([`QuantityOutcome::Buy`]; or does
    /// nothing or corrects the value and stays open, `None`), the close button closes
    /// without buying ([`QuantityOutcome::Closed`]). A purchase is remembered in `memory`
    /// (rule 8).
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        texts: &dyn NpcTexts,
        memory: &mut QuantityMemory,
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
            OK => return self.confirm(ui, texts, memory).map(QuantityOutcome::Buy),
            CANCEL => {
                self.open = false;
                return Some(QuantityOutcome::Closed);
            }
            _ => return None,
        }
        self.refresh(ui, texts);
        None
    }

    /// OK or `AcceptInput` (Enter) while the dialog has focus (rule 5), in any OK state:
    /// reads the entry's text first (rule 4); an empty entry or a `TooExpensive` cost does
    /// nothing; a value of 0 or above *M* is corrected and the dialog stays open (`None`);
    /// otherwise the purchase is made, remembered in `memory` (rule 8), and the dialog
    /// closes.
    pub fn confirm(
        &mut self,
        ui: &mut Ui,
        texts: &dyn NpcTexts,
        memory: &mut QuantityMemory,
    ) -> Option<NpcAction> {
        if !self.open {
            return None;
        }
        self.sync_entry(ui, texts);
        let too_expensive = self.too_expensive();
        match self.model.confirm(too_expensive) {
            Confirm::Corrected => {
                self.refresh(ui, texts);
                None
            }
            Confirm::Ignored => None,
            Confirm::Buy(q) => {
                let r = self.request.as_ref()?;
                self.open = false;
                memory.remember(r.vendor, q);
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
