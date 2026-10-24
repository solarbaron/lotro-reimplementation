// SPDX-License-Identifier: GPL-3.0-or-later
//! The trade window (`social-panels.md` §6, Tessera rules §9 TR9, TR10): both sides'
//! offers as one-column lists of item displays (reusing [`crate::game::items`] slots, so
//! each entry has the item's icon and the examination tooltip; the entry's parts found by
//! ID, §6.2), the wallet and the peer's coin as the money displays of the NPC windows
//! (`npc-windows.md` §9.2–§9.3, [`crate::game::npc::coins`]), the own coin entries with
//! their validation, the accept states and their reset, the trade button's ghosting,
//! cancel, close and completion.

use tessera_formats::property::PropertyValue;

use crate::events::UiEvent;
use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::widgets::WidgetClass;

use super::super::charvocab::CharVocab;
use super::super::chat::notice::{self as n, Notice, TextRef, TextSource, args as A, tables as T};
use super::super::items::{ItemSlots, SlotItem, SlotPlace};
use super::super::npc::NpcTexts;
use super::super::npc::coins::{CoinCounts, CoinKind, MoneyDisplay};
use super::super::npc::kit::FallbackArt;
use super::super::{IconImage, find_below, subtree};
use super::vocab::SocialVocab;
use super::{SocialAction, SocialIntent, ensure_rows, find, game_message, listen_id, set_ref};

/// Entries per side (TR9: the feed's limit of 32, `scenario-feed.md` §6.3; Q8).
pub const MAX_ENTRIES: usize = 32;

/// Characters a coin entry holds (§6.1).
pub const COIN_ENTRY_CHARS: usize = 5;

/// The client container numbers of the trade lists' item slots ([`SlotPlace::Other`]).
pub const OWN_CONTAINER: u32 = 0x5452_0001;
/// The peer side's container number.
pub const PEER_CONTAINER: u32 = 0x5452_0002;

/// A side of the trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The player's side (left).
    Own,
    /// The peer's side (right).
    Peer,
}

/// The value of each coin denomination in the smallest unit (`Currency_Value` of the
/// `GoldCurrency`, `SilverCurrency` and `CopperCurrency` records, EC §7; read from the
/// install by the client).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoinValues {
    /// Gold.
    pub gold: u64,
    /// Silver.
    pub silver: u64,
    /// Copper.
    pub copper: u64,
}

impl CoinValues {
    /// Splits a coin amount into (gold, silver, copper), largest first (TR9, EC §7). A zero
    /// value takes nothing.
    #[must_use]
    pub fn split(&self, coin: u64) -> [u64; 3] {
        let mut rest = coin;
        let mut out = [0u64; 3];
        for (i, v) in [self.gold, self.silver, self.copper]
            .into_iter()
            .enumerate()
        {
            if let Some(q) = rest.checked_div(v) {
                out[i] = q;
                rest -= q * v;
            }
        }
        out
    }

    /// The total of (gold, silver, copper) amounts, or `None` on overflow.
    #[must_use]
    pub fn total(&self, amounts: [u64; 3]) -> Option<u64> {
        let mut t: u64 = 0;
        for (a, v) in amounts
            .into_iter()
            .zip([self.gold, self.silver, self.copper])
        {
            t = t.checked_add(a.checked_mul(v)?)?;
        }
        Some(t)
    }
}

/// One offered entry (TR9: a feed entry, with what the client resolved for it).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TradeEntry {
    /// The display name (an item's name, or a literal ref's text).
    pub name: String,
    /// The item, when the entry is an item instance (none for a literal ref).
    pub item: Option<u64>,
    /// The quantity.
    pub quantity: u32,
    /// The icon (none: no item image, TR9).
    pub icon: Option<IconImage>,
    /// The record's `Item_Quality` key (the name colour, §6.2 Tessera rule); none for a
    /// literal entry or an unknown record.
    pub quality: Option<u32>,
    /// The entry is a literal ref (`scenario-feed.md` §5.2; TR9: `Object_ColorName`, no
    /// icon, no quantity, no tooltip).
    pub literal: bool,
}

/// One side's offer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Offer {
    /// The entries, at most [`MAX_ENTRIES`].
    pub entries: Vec<TradeEntry>,
    /// The coin offered.
    pub coin: u64,
    /// Accepted.
    pub ready: bool,
}

/// The coin entry fields (§6.1), in their order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoinField {
    /// Gold.
    Gold,
    /// Silver.
    Silver,
    /// Copper.
    Copper,
}

impl CoinField {
    fn index(self) -> usize {
        match self {
            Self::Gold => 0,
            Self::Silver => 1,
            Self::Copper => 2,
        }
    }
}

/// The trade model (§6.3–§6.6, TR9, TR10).
#[derive(Debug, Clone, PartialEq)]
pub struct Trade {
    window: Option<u64>,
    peer_name: Option<String>,
    own: Offer,
    peer: Offer,
    /// The trade button's highlight (TR10).
    button: bool,
    typed: [String; 3],
    /// Bumped whenever `typed` is set from outside the entries (an opening, an own
    /// `TradeOffer`, a commit), so the window rewrites its entries only then.
    coin_gen: u64,
    wallet: u64,
    values: CoinValues,
}

impl Trade {
    /// A closed trade with the given coin values.
    #[must_use]
    pub fn new(values: CoinValues) -> Self {
        Self {
            window: None,
            peer_name: None,
            own: Offer::default(),
            peer: Offer::default(),
            button: false,
            typed: Default::default(),
            coin_gen: 0,
            wallet: 0,
            values,
        }
    }

    /// The open trade window, if any.
    #[must_use]
    pub fn window(&self) -> Option<u64> {
        self.window
    }

    /// Whether the window is shown.
    #[must_use]
    pub fn shown(&self) -> bool {
        self.window.is_some()
    }

    /// One side's offer.
    #[must_use]
    pub fn offer(&self, side: Side) -> &Offer {
        match side {
            Side::Own => &self.own,
            Side::Peer => &self.peer,
        }
    }

    fn offer_mut(&mut self, side: Side) -> &mut Offer {
        match side {
            Side::Own => &mut self.own,
            Side::Peer => &mut self.peer,
        }
    }

    /// The player's wallet (`SelfTrade_TotalMoney`).
    #[must_use]
    pub fn wallet(&self) -> u64 {
        self.wallet
    }

    /// Sets the player's wallet.
    pub fn set_wallet(&mut self, coin: u64) {
        self.wallet = coin;
    }

    /// The coin values.
    #[must_use]
    pub fn values(&self) -> CoinValues {
        self.values
    }

    /// `TradeShown { window, peer }` (§6.4 "opens", TR9): both sides empty and not ready,
    /// the button off, the coin entries empty; `peer_name` is the name of the peer's entity
    /// as last spawned, if known. One trade window at a time: a new one replaces it.
    pub fn open(&mut self, window: u64, peer_name: Option<&str>) {
        *self = Self {
            window: Some(window),
            peer_name: peer_name.map(str::to_string),
            wallet: self.wallet,
            coin_gen: self.coin_gen.wrapping_add(1),
            ..Self::new(self.values)
        };
    }

    /// The peer label: (UST, 0x0CFA21C2) with the peer's name, or cleared.
    #[must_use]
    pub fn peer_label(&self) -> Option<TextRef> {
        self.peer_name
            .as_ref()
            .map(|p| TextRef::new(T::UI_SECURE_TRADE, 0x0CFA_21C2).text(A::NAME, p.clone()))
    }

    /// `TradeOffer { window, side, entries, coin }` (§8.2, TR9): that side's content is
    /// replaced (at most [`MAX_ENTRIES`] entries), the own coin is written into the coin
    /// entries, then the §6.4 reset: both sides not ready, the button off (SPI8). An offer
    /// for another window is ignored.
    pub fn set_offer(&mut self, window: u64, side: Side, entries: Vec<TradeEntry>, coin: u64) {
        if self.window != Some(window) {
            return;
        }
        let o = self.offer_mut(side);
        o.entries = entries.into_iter().take(MAX_ENTRIES).collect();
        o.coin = coin;
        if side == Side::Own {
            let [g, s, c] = self.values.split(coin);
            self.typed = [g, s, c].map(|v| if v == 0 { String::new() } else { v.to_string() });
            self.coin_gen = self.coin_gen.wrapping_add(1);
        }
        self.own.ready = false;
        self.peer.ready = false;
        self.button = false;
    }

    /// `TradeReady { window, side, ready }` (§6.4, TR10): that side's field; for `Own` the
    /// button's highlight too.
    pub fn set_ready(&mut self, window: u64, side: Side, ready: bool) {
        if self.window != Some(window) {
            return;
        }
        self.offer_mut(side).ready = ready;
        if side == Side::Own {
            self.button = ready;
        }
    }

    /// The trade button's highlight.
    #[must_use]
    pub fn button_on(&self) -> bool {
        self.button
    }

    /// The trade button is ghosted while both offers are empty: no entries, no coin, no
    /// coin typed (§6.4), with the model's coin texts.
    #[must_use]
    pub fn button_ghosted(&self) -> bool {
        self.button_ghosted_with(&self.typed)
    }

    /// [`Trade::button_ghosted`] with the coin entries' texts as they are on screen (typed
    /// but perhaps not yet committed, §6.3).
    #[must_use]
    pub fn button_ghosted_with(&self, typed: &[String; 3]) -> bool {
        self.own.entries.is_empty()
            && self.peer.entries.is_empty()
            && self.own.coin == 0
            && self.peer.coin == 0
            && typed.iter().all(|t| t.trim().is_empty())
    }

    /// The generation of the coin texts: it changes whenever [`Trade::typed`] is set other
    /// than by typing (the trade opens, an own `TradeOffer` arrives, a commit runs). A view
    /// writes the coin entries only when it changed, so a refresh for any other event keeps
    /// what the player is typing.
    #[must_use]
    pub fn coin_generation(&self) -> u64 {
        self.coin_gen
    }

    /// A click on the trade button (TR10): `on` is the button's new highlight; it shows at
    /// once and `SetTradeReady` is sent.
    pub fn click_button(&mut self, on: bool) -> Option<SocialIntent> {
        let window = self.window?;
        self.button = on;
        Some(SocialIntent::SetTradeReady { window, ready: on })
    }

    /// The coin entries' texts (gold, silver, copper).
    #[must_use]
    pub fn typed(&self) -> &[String; 3] {
        &self.typed
    }

    /// Sets a coin entry's text as typed (cut to [`COIN_ENTRY_CHARS`] characters).
    pub fn type_coin(&mut self, field: CoinField, text: &str) {
        self.typed[field.index()] = text.chars().take(COIN_ENTRY_CHARS).collect();
    }

    fn parse(t: &str) -> Option<u64> {
        let t = t.trim();
        if t.is_empty() {
            return Some(0);
        }
        if !t.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        t.parse().ok()
    }

    /// The total of the coin entries (entries that are not numbers count as 0).
    #[must_use]
    pub fn entered_total(&self) -> u64 {
        let a = self.typed.clone().map(|t| Self::parse(&t).unwrap_or(0));
        self.values.total(a).unwrap_or(u64::MAX)
    }

    /// A coin entry lost focus or received Enter (§6.3, SP32): only `field` is tested and
    /// cleared: a text that is not a number posts (UST, 0x085F80B9) and clears it; else a
    /// total of the three entries above the wallet posts (UST, 0x06195A09) and clears it.
    /// Then, in every case, the amounts present are offered (an entry that is not a number
    /// counts as 0), even when they are still above the wallet (the session decides).
    pub fn commit_coin(&mut self, field: CoinField) -> Vec<SocialAction> {
        let Some(window) = self.window else {
            return Vec::new();
        };
        self.coin_gen = self.coin_gen.wrapping_add(1);
        let mut out = Vec::new();
        let i = field.index();
        if Self::parse(&self.typed[i]).is_none() {
            out.push(SocialAction::Post(Notice::of(
                TextRef::new(T::UI_SECURE_TRADE, 0x085F_80B9),
                n::ERROR,
            )));
            self.typed[i].clear();
        } else if self.entered_total() > self.wallet {
            out.push(SocialAction::Post(Notice::of(
                TextRef::new(T::UI_SECURE_TRADE, 0x0619_5A09),
                n::ERROR,
            )));
            self.typed[i].clear();
        }
        out.push(SocialAction::Intent(SocialIntent::OfferTrade {
            window,
            item: None,
            quantity: 0,
            coin: self.entered_total(),
        }));
        out
    }

    /// An item dropped on the own list (§6.6, TR9): an item of the player's backpack is
    /// offered with its stack's quantity and the entered coin; any other item is refused
    /// locally with a type 1 line in Tessera's words. Nothing shows until the feed answers
    /// with `TradeOffer`.
    pub fn drop_item(&mut self, item: u64, quantity: u32, in_backpack: bool) -> SocialAction {
        let Some(window) = self.window else {
            return SocialAction::Post(Notice::own("No trade is open.", n::ERROR));
        };
        if !in_backpack {
            return SocialAction::Post(Notice::own(
                "Only items from your bags can be offered.",
                n::ERROR,
            ));
        }
        SocialAction::Intent(SocialIntent::OfferTrade {
            window,
            item: Some(item),
            quantity,
            coin: self.entered_total(),
        })
    }

    /// Cancel or the close button (§6.6): `CloseWindow`. When the peer already ended the
    /// trade, [`Trade::closed`] has closed the model and the window only hides (`None`).
    pub fn cancel(&mut self) -> Option<SocialIntent> {
        let window = self.window.take()?;
        Some(SocialIntent::CloseTrade { window })
    }

    /// `WindowClosed { window }` of the trade window (completed or cancelled by either
    /// side): the window hides.
    pub fn closed(&mut self, window: u64) {
        if self.window == Some(window) {
            self.window = None;
        }
    }
}

/// The coin counts of an amount on a money display (§6.1: split by `Currency_Value`,
/// largest first; the largest-value currency to `Gold`, `npc-windows.md` §9.2 Tessera rule
/// (coins)).
#[must_use]
pub fn coin_counts(values: CoinValues, coin: u64) -> CoinCounts {
    let [gold, silver, copper] = values.split(coin);
    CoinCounts {
        gold,
        silver,
        copper,
    }
}

/// The name state of an entry (§6.2 Tessera rule: the state of `item-tooltips.md` §6.2 for
/// the item's `Item_Quality`; any other value, and a literal entry, `Object_ColorName`).
#[must_use]
pub fn name_state(vocab: &SocialVocab, entry: &TradeEntry) -> u32 {
    let st = vocab.states;
    if entry.literal {
        return st.object_name;
    }
    match entry.quality {
        Some(1) => st.quality_legendary,
        Some(2) => st.quality_rare,
        Some(3) => st.quality_incomparable,
        Some(4) => st.quality_uncommon,
        Some(5) => st.quality_common,
        _ => st.object_name,
    }
}

/// Draws a text in its state's colour: the first entry of the effective
/// `UICore_Text_font_colors` (the state's override, else the element's own) becomes the
/// text's `UICore_Text_font_color`, which the client's text shaper reads (`fonts.md` §8.4
/// leaves the colour sources to the colour properties). A text without such an array keeps
/// its colour.
fn apply_state_colour(ui: &mut Ui, vocab: &SocialVocab, k: ElementKey) {
    let p = vocab.pids;
    let first = ui
        .prop(k, p.text_font_colors)
        .value()
        .and_then(|v| crate::props::array_elements(v).first())
        .and_then(|e| {
            crate::props::as_argb(&e.value).or_else(|| {
                crate::props::struct_field(&e.value, p.text_font_color)
                    .and_then(crate::props::as_argb)
            })
        });
    if let Some(argb) = first {
        let v = PropertyValue::Colour(argb.to_le_bytes());
        if ui.prop(k, p.text_font_color).value() != Some(&v) {
            ui.set_property(k, p.text_font_color, v);
        }
    }
}

/// The coin texts of the money displays, rendered once per refresh from the string
/// tables (`npc-windows.md` §9.2: counts through (UI, 0x07480C44) with argument 0x5242A9,
/// the coin icons' tooltips (UI, 0x0135BA5C), (UI, 0x01E4A5BC), (UI, 0x0BE07C7C)).
struct CoinTexts {
    counts: Vec<(u64, String)>,
    tips: [Option<String>; 3],
}

impl CoinTexts {
    fn new(src: &mut dyn TextSource, shown: &[CoinCounts]) -> Self {
        let mut counts: Vec<(u64, String)> = Vec::new();
        for c in shown {
            for v in [c.gold, c.silver, c.copper] {
                if !counts.iter().any(|(n, _)| *n == v) {
                    let t = TextRef::new(T::UI, 0x0748_0C44).int(A::COIN_COUNT, v_i64(v));
                    counts.push((v, t.render(src)));
                }
            }
        }
        let tips = [0x0135_BA5C, 0x01E4_A5BC, 0x0BE0_7C7C].map(|token| {
            Some(TextRef::new(T::UI, token).render(src)).filter(|t| !t.trim().is_empty())
        });
        Self { counts, tips }
    }
}

fn v_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

impl NpcTexts for CoinTexts {
    fn coin_count(&self, count: u64) -> String {
        self.counts
            .iter()
            .find(|(n, _)| *n == count)
            .map(|(_, t)| t.clone())
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| count.to_string())
    }

    fn coin_tooltip(&self, kind: CoinKind) -> Option<String> {
        let i = match kind {
            CoinKind::Gold => 0,
            CoinKind::Silver => 1,
            CoinKind::Copper => 2,
        };
        self.tips[i].clone()
    }
}

/// The trade window on the element tree.
#[derive(Debug, Clone)]
pub struct TradeWindow {
    vocab: SocialVocab,
    /// `SecureTradeField`.
    pub root: ElementKey,
    own_list: Option<ElementKey>,
    peer_list: Option<ElementKey>,
    own_field: Option<ElementKey>,
    peer_field: Option<ElementKey>,
    button: Option<ElementKey>,
    cancel: Option<ElementKey>,
    close: Option<ElementKey>,
    peer_label: Option<ElementKey>,
    /// `SelfTrade_TotalMoney`: the player's money as coin elements (§6.1).
    pub wallet: Option<MoneyDisplay>,
    /// `OtherTrade_MoneyDisplay`: the peer's offered coin as coin elements (§6.1).
    pub peer_money: Option<MoneyDisplay>,
    wait: Option<ElementKey>,
    /// The coin entries' editable texts (gold, silver, copper).
    coins: [Option<ElementKey>; 3],
    /// The [`Trade::coin_generation`] last written into the coin entries.
    coin_gen: Option<u64>,
    /// The item slots of both lists (examination tooltips, icons, quantities).
    pub slots: ItemSlots,
    /// `ItemIconDisplay`'s type (the entry's icon element when `ItemDisplay_Icon` is not
    /// found).
    icon_type: u32,
    /// The entries of the own and peer lists, in list order.
    pub entries: (Vec<ElementKey>, Vec<ElementKey>),
}

impl TradeWindow {
    /// Opens `SecureTradeField` of layout `securetrade`, hidden (TR1), and binds it.
    ///
    /// # Errors
    /// The panel could not be opened.
    pub fn open(
        ui: &mut Ui,
        vocab: &SocialVocab,
        chars: CharVocab,
    ) -> Result<Self, crate::UiError> {
        vocab.register_classes(ui);
        let root = ui.open_panel_hidden_by_key(vocab.layouts.securetrade, vocab.ids.trade)?;
        Ok(Self::attach(ui, vocab, chars, root))
    }

    /// Binds a window the client opened (after [`SocialVocab::register_classes`]): finds
    /// the parts, builds the two money displays' coin elements, makes the own list a drop
    /// container and registers the handlers.
    pub fn attach(ui: &mut Ui, vocab: &SocialVocab, chars: CharVocab, root: ElementKey) -> Self {
        let ids = vocab.ids;
        let f = |id: u32| find(ui, root, id);
        // §6.1, SP31: the editable texts by ID; a tree without them (not the install's)
        // falls back to the field's editable text.
        let coin = |field_id: u32, entry_id: u32| {
            let field = f(field_id)?;
            find(ui, field, entry_id)
                .or_else(|| {
                    subtree(ui, field)
                        .into_iter()
                        .find(|k| ui.prop(*k, ui.vocab().pids.text_editable).bool())
                })
                .or_else(|| {
                    subtree(ui, field)
                        .into_iter()
                        .skip(1)
                        .find(|k| ui.el(*k).is_some_and(|e| e.class == WidgetClass::Text))
                })
        };
        let coins = [
            coin(ids.coin_gold, ids.coin_gold_entry),
            coin(ids.coin_silver, ids.coin_silver_entry),
            coin(ids.coin_copper, ids.coin_copper_entry),
        ];
        let art = FallbackArt::default();
        let mut money = |id: u32| {
            let k = find(ui, root, id)?;
            let k = MoneyDisplay::find(ui, &vocab.npc, k).unwrap_or(k);
            Some(MoneyDisplay::attach(ui, &vocab.npc, k, &art))
        };
        let wallet = money(ids.trade_wallet);
        let peer_money = money(ids.trade_peer_money);
        let f = |id: u32| find(ui, root, id);
        let w = Self {
            vocab: vocab.clone(),
            root,
            own_list: f(ids.trade_own_list),
            peer_list: f(ids.trade_peer_list),
            own_field: f(ids.trade_own_field),
            peer_field: f(ids.trade_peer_field),
            button: f(ids.trade_button),
            cancel: f(ids.trade_cancel),
            close: f(ids.trade_close),
            peer_label: f(ids.trade_peer_label),
            wallet,
            peer_money,
            wait: f(ids.trade_wait),
            coins,
            coin_gen: None,
            icon_type: chars.types.item_icon_display,
            slots: ItemSlots::new(chars),
            entries: (Vec::new(), Vec::new()),
        };
        let ms = ui.vocab().msgs;
        for id in [ids.trade_button, ids.trade_cancel, ids.trade_close] {
            listen_id(ui, id, ms.button_click);
        }
        for c in w.coins.iter().flatten() {
            if let Some(id) = ui.element_id(*c) {
                listen_id(ui, id, ms.set_focus);
            }
        }
        if let Some(l) = w.own_list {
            let pids = ui.vocab().pids;
            ui.set_property(l, pids.container, PropertyValue::Bool(true));
            ui.recompute_mouse_visible(l);
        }
        w
    }

    /// The coin entry texts as the player typed them (gold, silver, copper).
    #[must_use]
    pub fn coin_texts(&self, ui: &mut Ui) -> [String; 3] {
        self.coins
            .map(|c| c.map(|k| ui.text_of(k)).unwrap_or_default())
    }

    fn fill_list(&mut self, ui: &mut Ui, side: Side, offer: &Offer) {
        let (list, container) = match side {
            Side::Own => (self.own_list, OWN_CONTAINER),
            Side::Peer => (self.peer_list, PEER_CONTAINER),
        };
        let Some(list) = list else { return };
        let rows = ensure_rows(
            ui,
            list,
            self.vocab.layouts.itemdisplay,
            self.vocab.ids.item_medium,
            offer.entries.len(),
        );
        let icon_type = self.icon_type;
        let ids = self.vocab.ids;
        for (i, (k, e)) in rows.iter().zip(&offer.entries).enumerate() {
            // §6.2, TR9: the parts by ID, never by type order (the quantity overlay is a
            // Text before the name). The name in its quality state.
            if let Some(t) = find_below(ui, *k, ids.item_name) {
                ui.set_text(t, Some(e.name.clone()));
                ui.switch_state_if_defined(t, name_state(&self.vocab, e));
                apply_state_colour(ui, &self.vocab, t);
            }
            // The icon: `ItemDisplay_Icon` (else the first `ItemIconDisplay`, else the
            // entry) is the item slot; its quantity overlay is the slot's.
            let icon = find_below(ui, *k, ids.item_icon)
                .or_else(|| {
                    subtree(ui, *k)
                        .into_iter()
                        .skip(1)
                        .find(|x| ui.element_type(*x) == Some(icon_type))
                })
                .unwrap_or(*k);
            let place = SlotPlace::Other {
                container,
                index: u32::try_from(i).unwrap_or(u32::MAX),
            };
            self.slots.attach(ui, icon, place);
            // TR9: a literal entry has no item image, no quantity and no tooltip.
            let (icon_image, quantity) = if e.literal {
                (None, 0)
            } else {
                (e.icon.clone(), i64::from(e.quantity))
            };
            self.slots.set(
                ui,
                place,
                Some(SlotItem {
                    item: e.item.unwrap_or(0),
                    icon: icon_image,
                    quantity,
                    ..SlotItem::default()
                }),
            );
            // No entry is a drag source: drops are accepted only from the player's own
            // containers and an offer has no removal (§6.6, Q11), so neither side's entries
            // hand out an item. A literal ref has no item to examine either (TR9).
            ui.set_drag_payload(icon, None);
            let dragable = ui.vocab().pids.dragable;
            ui.set_property(icon, dragable, PropertyValue::Bool(false));
            if e.item.is_none() || e.literal {
                ui.set_examine(icon, false);
            }
        }
        match side {
            Side::Own => self.entries.0 = rows,
            Side::Peer => self.entries.1 = rows,
        }
        self.slots.prune(ui);
    }

    /// Shows the model: hidden without a trade; else the peer label, both lists, the
    /// wallet and the peer's coin, the own coin entries (written only when the model's coin
    /// texts changed, [`Trade::coin_generation`], so text being typed survives a refresh for
    /// another event), the side fields' states, the trade button's highlight and ghosting
    /// (from the entries' texts on screen); the wait overlay hidden.
    pub fn refresh(&mut self, ui: &mut Ui, model: &Trade, src: &mut dyn TextSource) {
        if !model.shown() {
            ui.set_shown(self.root, false);
            return;
        }
        let st = ui.vocab().states;
        set_ref(ui, self.peer_label, model.peer_label().as_ref(), src);
        self.fill_list(ui, Side::Own, model.offer(Side::Own));
        self.fill_list(ui, Side::Peer, model.offer(Side::Peer));
        // §6.1, TR9: the wallet and the peer's coin as coin elements.
        let values = model.values();
        let wallet = coin_counts(values, model.wallet());
        let peer = coin_counts(values, model.offer(Side::Peer).coin);
        if self.wallet.is_some() || self.peer_money.is_some() {
            let texts = CoinTexts::new(src, &[wallet, peer]);
            if let Some(d) = self.wallet.as_mut() {
                d.set(ui, wallet, false, &texts);
            }
            if let Some(d) = self.peer_money.as_mut() {
                d.set(ui, peer, false, &texts);
            }
        }
        if self.coin_gen != Some(model.coin_generation()) {
            self.write_coins(ui, model);
        }
        let on_screen = self.coin_texts(ui);
        for (f, side) in [(self.own_field, Side::Own), (self.peer_field, Side::Peer)] {
            if let Some(k) = f {
                let s = if model.offer(side).ready {
                    st.highlight
                } else {
                    st.normal
                };
                ui.switch_state_if_defined(k, s);
            }
        }
        if let Some(b) = self.button {
            // §6.4: the ghosted flag (`UICore_Button_ghosted`) and the boolean button's
            // highlight, set as properties so the button chooses its state (UW §7.3).
            let p = ui.vocab().pids;
            for (pid, on) in [
                (p.button_ghosted, model.button_ghosted_with(&on_screen)),
                (p.button_highlighted, model.button_on()),
            ] {
                if ui.prop(b, pid).bool() != on {
                    ui.set_property(b, pid, PropertyValue::Bool(on));
                }
            }
        }
        if let Some(w) = self.wait {
            ui.set_shown(w, false);
        }
        ui.set_shown(self.root, true);
    }

    /// Handles a UI event: the trade button (TR10), cancel and close (§6.6), a coin entry
    /// losing focus (§6.3), an item dropped on the own list (§6.6: the client checks it
    /// and calls [`Trade::drop_item`]).
    pub fn handle_event(
        &mut self,
        ui: &mut Ui,
        ev: &UiEvent,
        model: &mut Trade,
    ) -> Vec<SocialAction> {
        if let UiEvent::DragEnded {
            source,
            target: Some(t),
            accepted: true,
        } = ev
            && let Some(l) = self.own_list
            && ui.is_ancestor_or_self(l, *t)
        {
            return ui
                .el(*source)
                .and_then(|e| e.drag_payload)
                .map(|p| SocialAction::TradeDrop { item: p.item })
                .into_iter()
                .collect();
        }
        let Some(m) = game_message(ev).cloned() else {
            return Vec::new();
        };
        let ms = ui.vocab().msgs;
        if m.msg == ms.button_click {
            if Some(m.source) == self.button {
                let on = ui.prop(m.source, ui.vocab().pids.button_highlighted).bool();
                return model
                    .click_button(on)
                    .map(SocialAction::Intent)
                    .into_iter()
                    .collect();
            }
            if Some(m.source) == self.cancel || Some(m.source) == self.close {
                let out = model
                    .cancel()
                    .map(SocialAction::Intent)
                    .into_iter()
                    .collect();
                ui.set_shown(self.root, false);
                return out;
            }
        }
        if m.msg == ms.set_focus
            && m.arg.value == 0
            && let Some(i) = self.coins.iter().position(|c| *c == Some(m.source))
        {
            return self.commit(ui, model, i);
        }
        Vec::new()
    }

    /// Enter in a coin entry (§6.3): the client calls this when `AcceptInput` reaches a
    /// focused coin entry. Returns `None` when `k` is not a coin entry.
    pub fn accept_input(
        &mut self,
        ui: &mut Ui,
        k: ElementKey,
        model: &mut Trade,
    ) -> Option<Vec<SocialAction>> {
        let i = self.coins.iter().position(|c| *c == Some(k))?;
        Some(self.commit(ui, model, i))
    }

    fn commit(&mut self, ui: &mut Ui, model: &mut Trade, i: usize) -> Vec<SocialAction> {
        let texts = self.coin_texts(ui);
        for (f, t) in [CoinField::Gold, CoinField::Silver, CoinField::Copper]
            .into_iter()
            .zip(&texts)
        {
            // An entry the player did not edit keeps the model's text: an own coin from the
            // feed may be longer than the entry holds, and cutting it would offer another
            // amount.
            if *t != model.typed()[f.index()] {
                model.type_coin(f, t);
            }
        }
        let field = [CoinField::Gold, CoinField::Silver, CoinField::Copper][i];
        let out = model.commit_coin(field);
        self.write_coins(ui, model);
        out
    }

    fn write_coins(&mut self, ui: &mut Ui, model: &Trade) {
        for (c, t) in self.coins.iter().zip(model.typed()) {
            if let Some(k) = c {
                ui.set_text(*k, Some(t.clone()));
            }
        }
        self.coin_gen = Some(model.coin_generation());
    }
}
