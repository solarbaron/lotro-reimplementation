// SPDX-License-Identifier: GPL-3.0-or-later
//! Coin amounts and the money display (`npc-windows.md` §9.2, §9.3): splitting an amount
//! into coins, which coins a display shows, and the `MoneyDisplay` widget with its three
//! coin elements.

use tessera_formats::did::Did;

use crate::tree::ElementKey;
use crate::ui::Ui;
use crate::vocab::Vocab;
use crate::widgets::WidgetClass;

use super::super::npcvocab::NpcVocab;
use super::super::subtree;
use super::NpcTexts;
use super::kit::{FNode, FallbackArt, add_row, colour_value, colours, find, instantiate};

/// Most currencies a split considers (Tessera bound; the install has three).
pub const MAX_CURRENCIES: usize = 16;

/// A coin element's kind (its icon state, §9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoinKind {
    /// `Gold`: the largest-value currency.
    Gold,
    /// `Silver`: the next.
    Silver,
    /// `Copper`: the value-1 currency.
    Copper,
}

impl CoinKind {
    /// The three kinds, largest first.
    pub const ALL: [CoinKind; 3] = [CoinKind::Gold, CoinKind::Silver, CoinKind::Copper];

    fn index(self) -> usize {
        match self {
            Self::Gold => 0,
            Self::Silver => 1,
            Self::Copper => 2,
        }
    }
}

/// The counts of the three coin elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct CoinCounts {
    /// `Gold`.
    pub gold: u64,
    /// `Silver`.
    pub silver: u64,
    /// `Copper`.
    pub copper: u64,
}

impl CoinCounts {
    /// The count of one kind.
    #[must_use]
    pub fn get(&self, kind: CoinKind) -> u64 {
        match kind {
            CoinKind::Gold => self.gold,
            CoinKind::Silver => self.silver,
            CoinKind::Copper => self.copper,
        }
    }

    /// Whether every count is 0.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.gold == 0 && self.silver == 0 && self.copper == 0
    }
}

/// Splits `amount` (in units of the value-1 currency) by currency values (§9.3, the class
/// library's split of `item-tooltips.md` §6.11): the currencies sorted by value, largest
/// first (**Tessera rule** of §9.3 for a property-set reader), values below 1 ignored; each
/// takes `amount div value` and the remainder goes on. Returns (value, count) pairs, at
/// most [`MAX_CURRENCIES`].
#[must_use]
pub fn split(amount: u64, values: &[u64]) -> Vec<(u64, u64)> {
    let mut vs: Vec<u64> = values.iter().copied().filter(|v| *v >= 1).collect();
    vs.sort_unstable_by(|a, b| b.cmp(a));
    vs.dedup();
    vs.truncate(MAX_CURRENCIES);
    let mut rest = amount;
    vs.into_iter()
        .map(|v| {
            let n = rest / v;
            rest %= v;
            (v, n)
        })
        .collect()
}

/// The coin counts of `amount` (§9.2 **Tessera rule (coins)**): the largest-value
/// currency's count goes to `Gold`, the next to `Silver`, the value-1 currency's to
/// `Copper`.
///
/// For currency lists other than the install's three (hostile or future data), Tessera
/// fills `Copper` from the value-1 currency (else the smallest one), `Gold` and `Silver`
/// from the two largest of the others; further currencies are not shown.
#[must_use]
pub fn coin_counts(amount: u64, values: &[u64]) -> CoinCounts {
    let parts = split(amount, values);
    let mut c = CoinCounts::default();
    let Some(copper_at) = parts
        .iter()
        .position(|(v, _)| *v == 1)
        .or_else(|| parts.len().checked_sub(1))
    else {
        return c;
    };
    c.copper = parts[copper_at].1;
    let mut others = parts
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != copper_at)
        .map(|(_, p)| p.1);
    c.gold = others.next().unwrap_or(0);
    c.silver = others.next().unwrap_or(0);
    if others.next().is_some() {
        tracing::debug!("money display: more than three currencies; the rest are not shown");
    }
    c
}

/// The options of a money display template (§9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MoneyOptions {
    /// `UIComponent_MoneyDisplay_ForceShowAll`.
    pub force_show_all: bool,
    /// `UIComponent_MoneyDisplay_SuppressLeadingZeroes`.
    pub suppress_leading_zeroes: bool,
    /// `UIComponent_MoneyDisplay_LeftJustifiedCoins`.
    pub left_justified: bool,
}

impl MoneyOptions {
    /// Left-justified coins and no other option (the fallback displays' usual options:
    /// their horizontal list lays coins out from the left, gold first).
    pub const LEFT_JUSTIFIED: Self = Self {
        force_show_all: false,
        suppress_leading_zeroes: false,
        left_justified: true,
    };
}

/// Which coins a display shows, in the order gold, silver, copper (§9.2 **Tessera rule
/// (coins)**): a coin with count 0 is hidden, except: with `ForceShowAll` every coin is
/// shown; with `SuppressLeadingZeroes` as well, a zero coin is shown only after a larger
/// non-zero coin. An amount of 0 with `ForceShowAll` shows only the copper coin when
/// `SuppressLeadingZeroes` is set, all three otherwise.
#[must_use]
pub fn visible(counts: CoinCounts, opts: MoneyOptions) -> [bool; 3] {
    let n = [counts.gold, counts.silver, counts.copper];
    if !opts.force_show_all {
        return n.map(|c| c > 0);
    }
    if !opts.suppress_leading_zeroes {
        return [true; 3];
    }
    if counts.is_zero() {
        return [false, false, true];
    }
    let mut seen = false;
    let mut out = [false; 3];
    for (i, c) in n.iter().enumerate() {
        out[i] = *c > 0 || seen;
        seen |= *c > 0;
    }
    out
}

/// One coin element of a money display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoinElement {
    /// The coin element.
    pub element: ElementKey,
    /// `MoneyDisplay_Text`.
    pub text: Option<ElementKey>,
    /// `MoneyDisplay_Icon`.
    pub icon: Option<ElementKey>,
}

/// A money display (§9.2): a horizontal list of three coin elements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoneyDisplay {
    /// The display (a `MoneyDisplay` or `PlayerMoneyDisplay` element).
    pub element: ElementKey,
    /// Its options.
    pub options: MoneyOptions,
    /// The coin elements by kind: gold, silver, copper.
    pub coins: [Option<CoinElement>; 3],
    /// The `TooExpensive` state ID.
    expensive_state: u32,
    shown: Option<(CoinCounts, bool)>,
}

/// Size of a fallback coin element (`MoneyDisplay_CoinTemplate`, 53 × 22).
pub const COIN_SIZE: (i32, i32) = (53, 22);

/// Size of a fallback money display (160 × 22, §9.2).
pub const DISPLAY_SIZE: (i32, i32) = (160, 22);

impl MoneyDisplay {
    /// The money display at or below `k`: `k` when it is of a money-display type, else
    /// the first element of such a type below it.
    #[must_use]
    pub fn find(ui: &Ui, nv: &NpcVocab, k: ElementKey) -> Option<ElementKey> {
        subtree(ui, k)
            .into_iter()
            .find(|e| ui.element_type(*e).is_some_and(|t| nv.is_money_type(t)))
    }

    /// Builds the display's coin elements (§9.2): reads its options; creates exactly three
    /// coin elements with icon states `Copper`, `Silver`, `Gold` in that order, each
    /// hidden, appended to the list, or inserted at the front when the coins are
    /// left-justified. The coin element is the display's `…_CoinDisplay_ElementID` of
    /// `…_CoinDisplay_Layout`; without them (or when it cannot be created) a fallback coin
    /// element with `art`'s coin images. Earlier coin elements are removed.
    pub fn attach(ui: &mut Ui, nv: &NpcVocab, element: ElementKey, art: &FallbackArt) -> Self {
        let p = nv.pids;
        let options = MoneyOptions {
            force_show_all: ui.prop(element, p.force_show_all).bool(),
            suppress_leading_zeroes: ui.prop(element, p.suppress_leading_zeroes).bool(),
            left_justified: ui.prop(element, p.left_justified).bool(),
        };
        if ui.class(element).is_some_and(WidgetClass::is_list) {
            ui.list_clear(element);
        }
        let layout = ui.prop(element, p.coin_layout).key();
        let coin_id = ui.prop(element, p.coin_element).key();
        let mut coins: [Option<CoinElement>; 3] = [None; 3];
        // Creation order Copper, Silver, Gold (BIN NW18); left-justified coins are inserted
        // at the front, which leaves them in the order Gold, Silver, Copper.
        let mut order = [CoinKind::Copper, CoinKind::Silver, CoinKind::Gold];
        if options.left_justified {
            order.reverse();
        }
        for kind in order {
            let made = if layout != 0 && coin_id != 0 {
                ui.create_element(None, Did(layout), coin_id).ok()
            } else {
                None
            };
            let made = match made {
                Some(k) => Some(k),
                None => {
                    let v = ui.vocab().clone();
                    instantiate(ui, element, &fallback_coin(&v, nv, art))
                }
            };
            let Some(k) = made else { continue };
            if !add_row(ui, element, k) {
                continue;
            }
            let text = find(ui, k, nv.ids.coin_text);
            let icon = find(ui, k, nv.ids.coin_icon);
            if let Some(i) = icon {
                ui.switch_state_if_defined(i, state_of(nv, kind));
            }
            ui.set_shown(k, false);
            coins[kind.index()] = Some(CoinElement {
                element: k,
                text,
                icon,
            });
        }
        Self {
            element,
            options,
            coins,
            expensive_state: nv.states.too_expensive,
            shown: None,
        }
    }

    /// Shows `counts` (§9.2): each coin's count text through the client's format, the
    /// coins chosen by [`visible`], and the `TooExpensive` state (red counts) when
    /// `too_expensive` (set by the window, never by the display), else `normal`.
    pub fn set(
        &mut self,
        ui: &mut Ui,
        counts: CoinCounts,
        too_expensive: bool,
        texts: &dyn NpcTexts,
    ) {
        if self.shown == Some((counts, too_expensive)) {
            return;
        }
        self.shown = Some((counts, too_expensive));
        let vis = visible(counts, self.options);
        let st = ui.vocab().states;
        for kind in CoinKind::ALL {
            let Some(c) = self.coins[kind.index()] else {
                continue;
            };
            if let Some(t) = c.text {
                ui.set_text(t, Some(texts.coin_count(counts.get(kind))));
            }
            if let Some(i) = c.icon {
                ui.set_tooltip_text(i, texts.coin_tooltip(kind));
            }
            let s = if too_expensive {
                self.expensive_state
            } else {
                st.normal
            };
            for e in [Some(c.element), c.text].into_iter().flatten() {
                ui.switch_state_if_defined(e, s);
            }
            ui.set_shown(c.element, vis[kind.index()]);
        }
    }

    /// Shows the cost `amount` (split by `values`, §9.3) and switches the display itself to
    /// `TooExpensive` while the player's `coin` is below it, `normal` otherwise (`npc-windows.md`
    /// §7.5 last paragraph; `coin` `None`, before any `WalletChanged { Coin }`: no comparison,
    /// `normal`). Returns whether it is `TooExpensive`.
    pub fn set_cost(
        &mut self,
        ui: &mut Ui,
        amount: u64,
        values: &[u64],
        coin: Option<u64>,
        texts: &dyn NpcTexts,
    ) -> bool {
        let too_expensive = coin.is_some_and(|c| c < amount);
        self.set(ui, coin_counts(amount, values), too_expensive, texts);
        too_expensive
    }

    /// Whether the display is in state `TooExpensive` (as last set).
    #[must_use]
    pub fn is_too_expensive(&self) -> bool {
        self.shown.is_some_and(|(_, e)| e)
    }

    /// The counts last shown.
    #[must_use]
    pub fn counts(&self) -> Option<CoinCounts> {
        self.shown.map(|(c, _)| c)
    }

    /// Whether the coin of `kind` is shown.
    #[must_use]
    pub fn coin_shown(&self, ui: &Ui, kind: CoinKind) -> bool {
        self.coins[kind.index()].is_some_and(|c| ui.is_visible(c.element))
    }
}

/// The icon state of a coin kind.
#[must_use]
pub fn state_of(nv: &NpcVocab, kind: CoinKind) -> u32 {
    match kind {
        CoinKind::Gold => nv.states.gold,
        CoinKind::Silver => nv.states.silver,
        CoinKind::Copper => nv.states.copper,
    }
}

/// Sets or clears the `TooExpensive` state of every coin element of `d` (§7.2).
pub fn set_too_expensive(ui: &mut Ui, nv: &NpcVocab, d: &MoneyDisplay, on: bool) {
    let s = if on {
        nv.states.too_expensive
    } else {
        ui.vocab().states.normal
    };
    for c in d.coins.iter().flatten() {
        for e in [Some(c.element), c.text].into_iter().flatten() {
            ui.switch_state_if_defined(e, s);
        }
    }
}

/// A fallback coin element (53 × 22, `MoneyDisplay_CoinTemplate`): the count text with the
/// `normal` and `TooExpensive` colours of the template, and a 14 × 14 icon with one state
/// per coin kind (the coin art is drawn at its own size, so it should be 14 × 14).
pub(crate) fn fallback_coin(v: &Vocab, nv: &NpcVocab, art: &FallbackArt) -> FNode {
    let cp = nv.pids.text_font_color;
    let st_normal = v.states.normal;
    let text = FNode::text(nv.ids.coin_text, 2, 3, 28, 16, cp, colours::COIN, "")
        .state(st_normal, None, vec![(cp, colour_value(colours::COIN))])
        .state(
            nv.states.too_expensive,
            None,
            vec![(cp, colour_value(colours::RED))],
        );
    let icon = FNode::field(nv.ids.coin_icon, 33, 4, 14, 14)
        .state(nv.states.gold, art.coin_gold.as_ref(), vec![])
        .state(nv.states.silver, art.coin_silver.as_ref(), vec![])
        .state(nv.states.copper, art.coin_copper.as_ref(), vec![]);
    FNode::field(0, 0, 0, COIN_SIZE.0, COIN_SIZE.1)
        .state(st_normal, None, vec![])
        .state(nv.states.too_expensive, None, vec![])
        .child(text)
        .child(icon)
}

/// A fallback money display: a horizontal ListBox of `DISPLAY_SIZE`, at most three
/// columns, laying out only visible coins, typed as `MoneyDisplay` so that
/// [`MoneyDisplay::find`] finds it; options as given.
pub(crate) fn fallback_display(
    v: &Vocab,
    nv: &NpcVocab,
    id: u32,
    x: i32,
    y: i32,
    opts: MoneyOptions,
) -> FNode {
    use tessera_formats::property::PropertyValue as V;
    let p = v.pids;
    FNode::new(
        WidgetClass::ListBox,
        id,
        x,
        y,
        DISPLAY_SIZE.0,
        DISPLAY_SIZE.1,
    )
    .typed(nv.types.money_display)
    .prop(p.lb_horizontal, V::Bool(true))
    .prop(p.lb_max_columns, V::Int(3))
    .prop(p.lb_only_visible, V::Bool(true))
    .prop(nv.pids.force_show_all, V::Bool(opts.force_show_all))
    .prop(
        nv.pids.suppress_leading_zeroes,
        V::Bool(opts.suppress_leading_zeroes),
    )
    .prop(nv.pids.left_justified, V::Bool(opts.left_justified))
}
