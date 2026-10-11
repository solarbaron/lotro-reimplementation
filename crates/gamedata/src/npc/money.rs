// SPDX-License-Identifier: GPL-3.0-or-later
//! Coins and the wallet (`npc-windows.md` §9): splitting a coin amount into the three coin
//! elements, which coins show, the wallet items of the wallet control record, barter
//! currencies and the wallet window's rows.

use std::collections::{BTreeMap, HashMap};

use tessera_formats::gameplay::common::PropertyAccess;
use tessera_formats::property::PropertyEntry;
use tessera_formats::{Did, PropertyValue};
use tessera_text::{RichText, Value, Values};

use super::entries::{IconLayers, render_opt};
use super::{MAX_STRUCT_DEPTH, MAX_WALLET_ITEMS, NpcPresenter, UI_TABLE, tokens};
use crate::quest::{PlayerVars, RecordRef};
use crate::tooltip::rules::CurrencyUnit;
use crate::tooltip::{TextRef, TextResult};

/// WEENIECONTENT label of the currency directory (`economy.md` §7).
const CURRENCY_DIRECTORY: &str = "CurrencyDirectory";
/// WEENIECONTENT label of the wallet control record (§9.4).
const PAPER_ITEM_CONTROL: &str = "PaperItemControl";
/// The wallet category of literal refs (§9.4 rule 3).
const MISC_CATEGORY: &str = "Misc";

/// The coin currencies, largest value first (§9.3 Tessera rule: the property set's list
/// sorted by `Currency_Value`; values below 1 ignored).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Currencies {
    /// The currencies, largest value first.
    pub units: Vec<CurrencyUnit>,
}

impl Currencies {
    /// From (currency, value) pairs in any order.
    #[must_use]
    pub fn new(units: impl IntoIterator<Item = CurrencyUnit>) -> Self {
        let mut units: Vec<CurrencyUnit> = units.into_iter().filter(|u| u.value >= 1).collect();
        units.sort_by(|a, b| b.value.cmp(&a.value).then(a.did.cmp(&b.did)));
        units.dedup_by_key(|u| u.did);
        Self { units }
    }

    /// The counts of `amount` per currency, in [`Self::units`] order (§9.3: amount div
    /// value, keep the remainder).
    #[must_use]
    pub fn split(&self, amount: u64) -> Vec<u64> {
        let mut left = amount;
        self.units
            .iter()
            .map(|u| {
                let v = u64::try_from(u.value).unwrap_or(1).max(1);
                let c = left / v;
                left %= v;
                c
            })
            .collect()
    }

    /// The index of the currency each coin element shows: gold the largest value, silver
    /// the next, copper the smallest (§9.2 Tessera rule).
    fn positions(&self) -> [Option<usize>; 3] {
        match self.units.len() {
            0 => [None, None, None],
            1 => [None, None, Some(0)],
            2 => [Some(0), None, Some(1)],
            n => {
                if n > 3 {
                    tracing::debug!(currencies = n, "more than three coin currencies");
                }
                [Some(0), Some(1), Some(n - 1)]
            }
        }
    }
}

/// A coin element's icon state (§9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoinKind {
    /// `Gold`.
    Gold,
    /// `Silver`.
    Silver,
    /// `Copper`.
    Copper,
}

impl CoinKind {
    /// The icon state label.
    #[must_use]
    pub fn state_label(self) -> &'static str {
        match self {
            Self::Gold => "Gold",
            Self::Silver => "Silver",
            Self::Copper => "Copper",
        }
    }

    /// The `UI` token of the coin's tooltip.
    #[must_use]
    pub fn tooltip_token(self) -> u32 {
        match self {
            Self::Gold => tokens::COIN_TOOLTIPS[0],
            Self::Silver => tokens::COIN_TOOLTIPS[1],
            Self::Copper => tokens::COIN_TOOLTIPS[2],
        }
    }
}

/// One coin element.
#[derive(Debug, Clone, PartialEq)]
pub struct Coin {
    /// The icon state.
    pub kind: CoinKind,
    /// The currency record it shows, if any.
    pub currency: Option<Did>,
    /// The count.
    pub count: u64,
    /// Whether the element is shown.
    pub shown: bool,
    /// The count text (`UI` 0x07480C44), when it renders.
    pub text: Option<RichText>,
}

/// A money display's three coin elements, left to right gold, silver, copper (§9.2).
#[derive(Debug, Clone, PartialEq)]
pub struct CoinDisplay {
    /// The amount.
    pub amount: u64,
    /// Gold, silver, copper.
    pub coins: [Coin; 3],
}

impl CoinDisplay {
    /// The shown coins, left to right.
    pub fn shown(&self) -> impl Iterator<Item = &Coin> {
        self.coins.iter().filter(|c| c.shown)
    }
}

/// The options of a money display template (§9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CoinOptions {
    /// `UIComponent_MoneyDisplay_ForceShowAll`.
    pub force_show_all: bool,
    /// `UIComponent_MoneyDisplay_SuppressLeadingZeroes`.
    pub suppress_leading_zeroes: bool,
}

/// Which coin elements show (§9.2 Tessera rule).
#[must_use]
pub(crate) fn coin_visibility(counts: [u64; 3], amount: u64, o: CoinOptions) -> [bool; 3] {
    let mut shown = [false; 3];
    for i in 0..3 {
        shown[i] = if counts[i] > 0 {
            true
        } else if !o.force_show_all {
            false
        } else if !o.suppress_leading_zeroes {
            true
        } else if amount == 0 {
            i == 2
        } else {
            counts[..i].iter().any(|&c| c > 0)
        };
    }
    shown
}

/// A wallet item of the wallet control record (§9.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalletItem {
    /// The item record.
    pub did: Did,
    /// `PaperItemControl_CapDefault`, when the entry has one.
    pub cap: Option<i64>,
    /// Listed in `PaperItemControl_Free_PaperItemArray`.
    pub free: bool,
}

/// The wallet items (§9.4), in the record's order (free list first).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WalletCatalog {
    items: Vec<WalletItem>,
    index: HashMap<Did, usize>,
}

impl WalletCatalog {
    /// From items (the first entry of a DID wins).
    #[must_use]
    pub fn new(items: impl IntoIterator<Item = WalletItem>) -> Self {
        let mut out = Self::default();
        for it in items.into_iter().take(MAX_WALLET_ITEMS) {
            if !out.index.contains_key(&it.did) {
                out.index.insert(it.did, out.items.len());
                out.items.push(it);
            }
        }
        out
    }

    /// The items, in the order listed (first entry of a DID).
    #[must_use]
    pub fn items(&self) -> &[WalletItem] {
        &self.items
    }

    /// The wallet item `did`, if listed.
    #[must_use]
    pub fn get(&self, did: Did) -> Option<&WalletItem> {
        self.index.get(&did).and_then(|&i| self.items.get(i))
    }

    /// Whether a barter cost item is a wallet item, whose held amount is the wallet's
    /// (§9.5); other cost items are counted in the backpack.
    #[must_use]
    pub fn is_wallet_item(&self, did: Did) -> bool {
        self.index.contains_key(&did)
    }
}

/// A feed currency (`scenario-feed.md` §9.1 `Currency`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Currency {
    /// Coins.
    Coin,
    /// A wallet item.
    Item(RecordRef),
}

/// A wallet entry's icon (§9.4 rule 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletIcon {
    /// `PaperItem_SmallIcon`.
    Small(Did),
    /// The composed item icon, scaled to 16 × 16.
    Composed(IconLayers),
    /// No icon (a literal ref, or no icon layers).
    None,
}

/// One wallet entry (§9.4).
#[derive(Debug, Clone, PartialEq)]
pub struct WalletEntry {
    /// The item.
    pub what: RecordRef,
    /// The name.
    pub name: Option<RichText>,
    /// The icon.
    pub icon: WalletIcon,
    /// The amount (`PaperItemEntry_Total`, a decimal integer).
    pub amount: u64,
    /// State `ItemCapped`: the amount is at or above the cap.
    pub capped: bool,
}

/// One wallet category row with its entries.
#[derive(Debug, Clone, PartialEq)]
pub struct WalletCategory {
    /// The `PaperItem_Category` key (`None`: none known).
    pub key: Option<u32>,
    /// The title: the display text, else the label.
    pub title: String,
    /// Whether the category has no display text (census flag `NoCategoryText`).
    pub no_display_text: bool,
    /// Entries sorted by name (case-insensitive), then DID.
    pub entries: Vec<WalletEntry>,
}

/// The wallet window's rows (§9.4 Tessera rule).
#[derive(Debug, Clone, PartialEq)]
pub struct WalletView {
    /// The money row (`MoneyDisplay_PlayerMoneyWalletTemplate`: every coin shown).
    pub money: CoinDisplay,
    /// Categories in ascending key, each with at least one entry.
    pub categories: Vec<WalletCategory>,
}

/// Searches `entries` (and nested structs up to [`MAX_STRUCT_DEPTH`]) for the wallet item
/// DID and its cap.
fn wallet_fields(
    entries: &[PropertyEntry],
    item: Option<u32>,
    cap: Option<u32>,
    depth: usize,
    out: &mut (Option<Did>, Option<i64>),
) {
    for e in entries {
        if Some(e.pid) == item
            && let PropertyValue::DidRef(d) = e.value
            && out.0.is_none()
        {
            out.0 = Some(d);
        }
        if Some(e.pid) == cap && out.1.is_none() {
            out.1 = match e.value {
                PropertyValue::Int(i) => Some(i64::from(i)),
                PropertyValue::Int64(i) => Some(i),
                _ => None,
            };
        }
        if depth < MAX_STRUCT_DEPTH
            && let PropertyValue::Struct(s) = &e.value
        {
            wallet_fields(&s.fields, item, cap, depth + 1, out);
        }
    }
}

impl NpcPresenter<'_> {
    /// The coin currencies of the install (§9.3), read once.
    #[must_use]
    pub fn currencies(&self) -> &Currencies {
        self.currencies.get_or_init(|| {
            let p = self.pids();
            let data = self.data();
            let Some(dir) = data.weenie(CURRENCY_DIRECTORY) else {
                return Currencies::default();
            };
            let Ok(Some(set)) = data.record(dir) else {
                return Currencies::default();
            };
            Currencies::new(set.dids(p.currency_list).take(64).filter_map(|did| {
                let c = data.record(did).ok()??;
                let value = match c.get_pid(p.currency_value)? {
                    PropertyValue::Int(i) => i64::from(*i),
                    PropertyValue::Int64(i) => *i,
                    _ => return None,
                };
                Some(CurrencyUnit { did, value })
            }))
        })
    }

    /// The count text of a coin (`UI` 0x07480C44 with 0x5242A9).
    pub fn coin_text(&self, count: u64) -> TextResult {
        let mut v = Values::new();
        v.insert(
            tokens::VAR_COIN_COUNT,
            Value::Integer(i64::try_from(count).unwrap_or(i64::MAX)),
        );
        match self.ui_table {
            Some(t) => self.data().render(
                TextRef::Info(&crate::quest::token_info(t, tokens::COIN_COUNT)),
                &v,
            ),
            None => Err(format!("no `{UI_TABLE}` string table")),
        }
    }

    /// The money display of `amount` (§9.2, §9.3).
    #[must_use]
    pub fn coins(&self, amount: u64, options: CoinOptions) -> CoinDisplay {
        let cur = self.currencies();
        let split = cur.split(amount);
        let pos = cur.positions();
        let counts = pos.map(|p| p.and_then(|i| split.get(i).copied()).unwrap_or(0));
        let shown = coin_visibility(counts, amount, options);
        let kinds = [CoinKind::Gold, CoinKind::Silver, CoinKind::Copper];
        let coins = std::array::from_fn(|i| Coin {
            kind: kinds[i],
            currency: pos[i].map(|j| cur.units[j].did),
            count: counts[i],
            shown: shown[i],
            text: if shown[i] {
                self.coin_text(counts[i]).ok().flatten()
            } else {
                None
            },
        });
        CoinDisplay { amount, coins }
    }

    /// The wallet items of the wallet control record (§9.4), read once.
    #[must_use]
    pub fn wallet_catalog(&self) -> &WalletCatalog {
        self.wallet.get_or_init(|| {
            let p = self.pids();
            let data = self.data();
            let Some(did) = data.weenie(PAPER_ITEM_CONTROL) else {
                return WalletCatalog::default();
            };
            let Ok(Some(set)) = data.record(did) else {
                return WalletCatalog::default();
            };
            let mut items = Vec::new();
            for (list, free) in [(p.paper_free, true), (p.paper_list, false)] {
                for e in set.elements(list).iter().take(MAX_WALLET_ITEMS) {
                    let mut found = (None, None);
                    match &e.value {
                        PropertyValue::DidRef(d) => found.0 = Some(*d),
                        PropertyValue::Struct(s) => wallet_fields(
                            &s.fields,
                            p.paper_item.id(),
                            p.paper_cap.id(),
                            0,
                            &mut found,
                        ),
                        _ => {}
                    }
                    if let (Some(d), cap) = found
                        && !d.is_null()
                    {
                        items.push(WalletItem { did: d, cap, free });
                    }
                }
            }
            WalletCatalog::new(items)
        })
    }

    /// The category of a wallet item and its title: (key, title, has display text).
    #[must_use]
    pub fn wallet_category(&self, key: Option<u32>) -> (String, bool) {
        let Some(pid) = self.pids().paper_category.id() else {
            return (String::new(), false);
        };
        let Some(k) = key else {
            return (String::new(), false);
        };
        match self.data().enum_text(pid, k) {
            Some(t) => (t, true),
            None => (self.data().enum_name(pid, k).unwrap_or_default(), false),
        }
    }

    /// One wallet entry of a record (name, icon, category key, cap).
    fn wallet_entry(
        &self,
        what: &RecordRef,
        amount: u64,
        player: &PlayerVars,
    ) -> (Option<u32>, WalletEntry) {
        let p = self.pids();
        let misc = || {
            p.paper_category
                .id()
                .and_then(|pid| self.data().enum_key(pid, MISC_CATEGORY))
        };
        match what {
            RecordRef::Literal(s) => (
                misc(),
                WalletEntry {
                    what: what.clone(),
                    name: Some(RichText::plain(s)),
                    icon: WalletIcon::None,
                    amount,
                    capped: false,
                },
            ),
            RecordRef::Record(did) => {
                let set = self.data().record(*did).ok().flatten();
                // A record without `PaperItem_Category` (a feed currency that is not a
                // wallet item) goes under `Misc`, as a literal ref does (Tessera rule,
                // `npc-windows.md` §9.4 OPEN).
                let key = set
                    .as_ref()
                    .and_then(|s| s.enum_key(p.paper_category))
                    .or_else(misc);
                let name = set
                    .as_ref()
                    .and_then(|s| render_opt(self.data(), s.string_info(p.name), player));
                let icon = set
                    .as_ref()
                    .and_then(|s| {
                        s.did_nonnull(p.paper_small_icon)
                            .map(WalletIcon::Small)
                            .or_else(|| IconLayers::of(s, p).map(WalletIcon::Composed))
                    })
                    .unwrap_or(WalletIcon::None);
                let cap = self.wallet_catalog().get(*did).and_then(|w| w.cap);
                (
                    key,
                    WalletEntry {
                        what: what.clone(),
                        name,
                        icon,
                        amount,
                        capped: cap.is_some_and(|c| i128::from(amount) >= i128::from(c)),
                    },
                )
            }
        }
    }

    /// The wallet window (§9.4 Tessera rule) from `WalletChanged` amounts in arrival order
    /// (the last amount of each currency counts; at most [`MAX_WALLET_ITEMS`] distinct item
    /// currencies) and the search text.
    #[must_use]
    pub fn wallet_view(
        &self,
        amounts: &[(Currency, u64)],
        search: &str,
        player: &PlayerVars,
    ) -> WalletView {
        let coin = amounts
            .iter()
            .rev()
            .find(|(c, _)| *c == Currency::Coin)
            .map_or(0, |(_, a)| *a);
        let money = self.coins(
            coin,
            CoinOptions {
                force_show_all: true,
                suppress_leading_zeroes: false,
            },
        );
        let needle = crate::quest::lower(search);
        // The last amount per currency over the whole slice; at most MAX_WALLET_ITEMS
        // distinct currencies (later new ones are ignored, updates of kept ones apply).
        let mut latest: Vec<(&RecordRef, u64)> = Vec::new();
        let mut slot_of: HashMap<&RecordRef, usize> = HashMap::new();
        let mut dropped = 0usize;
        for (c, a) in amounts {
            if let Currency::Item(r) = c {
                if let Some(&i) = slot_of.get(r) {
                    latest[i].1 = *a;
                } else if latest.len() < MAX_WALLET_ITEMS {
                    slot_of.insert(r, latest.len());
                    latest.push((r, *a));
                } else {
                    dropped += 1;
                }
            }
        }
        if dropped > 0 {
            tracing::debug!(dropped, "wallet currencies beyond the bound left out");
        }
        let mut groups: BTreeMap<Option<u32>, Vec<WalletEntry>> = BTreeMap::new();
        for (r, a) in latest {
            if a == 0 {
                continue;
            }
            let (key, entry) = self.wallet_entry(r, a, player);
            let name = entry
                .name
                .as_ref()
                .map(|n| crate::quest::lower(&n.plain_text()))
                .unwrap_or_default();
            if !needle.is_empty() && !name.contains(&needle) {
                continue;
            }
            groups.entry(key).or_default().push(entry);
        }
        let categories = groups
            .into_iter()
            .map(|(key, mut entries)| {
                entries.sort_by_cached_key(|e| {
                    (
                        e.name
                            .as_ref()
                            .map(|n| crate::quest::lower(&n.plain_text()))
                            .unwrap_or_default(),
                        e.what.did().map_or(u32::MAX, |d| d.0),
                    )
                });
                let (title, has_text) = self.wallet_category(key);
                WalletCategory {
                    key,
                    title,
                    no_display_text: !has_text,
                    entries,
                }
            })
            .collect();
        WalletView { money, categories }
    }
}
