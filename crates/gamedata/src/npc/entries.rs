// SPDX-License-Identifier: GPL-3.0-or-later
//! Vendor, barter, buy-back and loot entries (`npc-windows.md` §7, §8): item names, icon
//! layers, prices as coins or barter costs, stock and quantities. Prices and quantities are
//! the feed's; nothing here computes one.

use tessera_formats::gameplay::common::PropertyAccess;
use tessera_formats::{Did, PropertySet, PropertyValue};
use tessera_text::RichText;

use super::money::CoinDisplay;
use super::{BARTER_COST_SLOTS, CoinOptions, MAX_ENTRIES, NpcPids, NpcPresenter, tokens};
use crate::quest::{PlayerVars, PresentationData, RecordRef};
use crate::tooltip::rules::{OBJECT_COLOR_STATE, quality_state_label};
use crate::tooltip::{TextRef, TextResult};

/// The layers of an item icon, bottom to top (`icon-layers.md` §3.1, §3.2). The client
/// composes them (`tessera-material`); a layer of DID 0 is skipped there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconLayers {
    /// `Icon_Layer_BackgroundDID` (the bottom layer; may be 0).
    pub background: Did,
    /// `Icon_Layer_UnderlayDID`, when present and non-zero.
    pub underlay: Option<Did>,
    /// `Icon_Layer_ShadowDID`, when present and non-zero.
    pub shadow: Option<Did>,
    /// `Icon_Layer_ImageDID` (may be 0).
    pub image: Did,
    /// `Icon_Layer_OverlayDID`, when present and non-zero.
    pub overlay: Option<Did>,
}

/// "Read *P*" of `icon-layers.md` §3.1: absent → `None`; a DID → it; another type → 0.
fn read_layer(set: &PropertySet, pid: tessera_formats::gameplay::common::Pid) -> Option<Did> {
    match set.get_pid(pid)? {
        PropertyValue::DidRef(d) => Some(*d),
        _ => Some(Did(0)),
    }
}

impl IconLayers {
    /// The layers of a static item record (§3.1 without an element override): `None` when
    /// the item has no icon (no Image or no Background property).
    #[must_use]
    pub(crate) fn of(set: &PropertySet, p: &NpcPids) -> Option<Self> {
        let image = read_layer(set, p.icon_image)?;
        let optional = |pid| read_layer(set, pid).filter(|d| !d.is_null());
        let shadow = optional(p.icon_shadow);
        let underlay = optional(p.icon_underlay);
        let background = read_layer(set, p.icon_background)?;
        Some(Self {
            background,
            underlay,
            shadow,
            image,
            overlay: optional(p.icon_overlay),
        })
    }

    /// The non-zero layers bottom to top.
    #[must_use]
    pub fn stack(&self) -> Vec<Did> {
        [
            Some(self.background),
            self.underlay,
            self.shadow,
            Some(self.image),
            self.overlay,
        ]
        .into_iter()
        .flatten()
        .filter(|d| !d.is_null())
        .collect()
    }
}

/// The census category of a list entry (`npc-windows.md` §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EntryCategory {
    /// Name and icon.
    Shown,
    /// No icon is drawn: `icon-layers.md` §3.1 gives none (no Image or no Background
    /// property) or every layer is null (`npc-windows.md` §11; see the OPEN there).
    NoIcon,
    /// No `Name`, or it renders empty.
    NoName,
    /// A literal ref (placeholder icon, literal name).
    Literal,
    /// The record cannot be read.
    RecordUnreadable,
}

/// An item as a list entry shows it: name, colour state, icon and stack size.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemDisplay {
    /// The item.
    pub what: RecordRef,
    /// The name (`Name`; the literal for a literal ref).
    pub name: Option<RichText>,
    /// The name's colour state (`item-tooltips.md` §6.2).
    pub quality_state: &'static str,
    /// The icon layers; `None` shows the placeholder icon.
    pub icon: Option<IconLayers>,
    /// `Inventory_MaxQuantity` (default 1): the stack size of §7.4.
    pub stack_size: i32,
    /// The census category.
    pub category: EntryCategory,
}

/// A feed price (`scenario-feed.md` §9.1 `Price`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Price {
    /// Coins, in units of the smallest denomination.
    pub coin: u64,
    /// Item costs (barter), in order.
    pub items: Vec<(RecordRef, u32)>,
}

/// A feed vendor or buy-back entry (`VendorShown`, `BuybackShown`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendorEntryIn {
    /// The entry number.
    pub entry: u16,
    /// The item.
    pub what: RecordRef,
    /// The unit price.
    pub price: Price,
    /// Stock (`None`: unlimited; vendor entries only).
    pub stock: Option<u32>,
    /// Quantity (buy-back entries only).
    pub quantity: Option<u32>,
}

/// A vendor or buy-back entry as shown (§7.2).
#[derive(Debug, Clone, PartialEq)]
pub struct VendorEntry {
    /// The entry number.
    pub entry: u16,
    /// The item.
    pub item: ItemDisplay,
    /// The unit price in coins (`MoneyDisplay_MoneyTemplate`: zero coins hidden).
    pub money: CoinDisplay,
    /// State `TooExpensive`: the coin price exceeds the player's last coin amount.
    pub too_expensive: bool,
    /// Stock shown in the quantity overlay, when present.
    pub stock: Option<u32>,
    /// The buy-back quantity, when present.
    pub quantity: Option<u32>,
    /// Stock 0 ghosts the Buy button while the entry is selected.
    pub buy_ghosted: bool,
}

/// One barter cost slot (§7.7 Tessera rule).
#[derive(Debug, Clone, PartialEq)]
pub enum CostSlot {
    /// An item cost: the item, the required quantity (the info text) and whether the player
    /// holds fewer (the info text takes the `TooExpensive` colour).
    Item {
        /// The cost item.
        item: ItemDisplay,
        /// The required quantity.
        quantity: u32,
        /// The player's held amount, as the caller reported it.
        held: u64,
        /// Held fewer than required.
        short: bool,
    },
    /// The coin price as one more cost.
    Coin {
        /// The coins.
        money: CoinDisplay,
        /// More than the player's coins.
        short: bool,
    },
}

/// A barter entry (§7.7).
#[derive(Debug, Clone, PartialEq)]
pub struct BarterEntry {
    /// The entry number.
    pub entry: u16,
    /// The reward (the entry's `what`).
    pub reward: ItemDisplay,
    /// The drawn cost slots (at most three).
    pub slots: Vec<CostSlot>,
    /// Costs beyond three slots (listed in the reward's tooltip).
    pub overflow: Vec<CostSlot>,
    /// Stock, when present.
    pub stock: Option<u32>,
}

/// Which window a `VendorShown` opens (§7.3 Tessera rule) with its entries in feed order.
#[derive(Debug, Clone, PartialEq)]
pub enum VendorWindow {
    /// The vendor window.
    Vendor(Vec<VendorEntry>),
    /// The barter window (some entry has an item cost).
    Barter(Vec<BarterEntry>),
}

/// A feed loot entry (`LootShown`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootEntryIn {
    /// The entry number.
    pub entry: u16,
    /// The item.
    pub what: RecordRef,
    /// The quantity.
    pub quantity: u32,
}

/// The kind of a loot window's source entity (§8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootSource {
    /// A creature, non-player or player entity.
    Agent,
    /// An object, or no source.
    Object,
}

/// `ExternalContainerBackground` state (§8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootBackground {
    /// `container_corpse`.
    Corpse,
    /// `container_chest`.
    Chest,
}

impl LootBackground {
    /// The state label.
    #[must_use]
    pub fn state_label(self) -> &'static str {
        match self {
            Self::Corpse => "container_corpse",
            Self::Chest => "container_chest",
        }
    }
}

/// One loot row.
#[derive(Debug, Clone, PartialEq)]
pub struct LootRow {
    /// The entry number.
    pub entry: u16,
    /// The item.
    pub item: ItemDisplay,
    /// The quantity (overlay above 1).
    pub quantity: u32,
}

/// The loot window's content (§8.2, §8.3).
#[derive(Debug, Clone, PartialEq)]
pub struct LootView {
    /// The coin row (first), when `coin` is above 0.
    pub coin: Option<CoinDisplay>,
    /// Rows of the untaken entries, in entry order.
    pub rows: Vec<LootRow>,
    /// The background state.
    pub background: LootBackground,
    /// The `UI` token of the Loot All button's label (rule 2; the coin row not counted).
    pub button_label: u32,
}

impl NpcPresenter<'_> {
    /// The display of an item (name, colour state, icon, stack size, category).
    #[must_use]
    pub fn item_display(&self, what: &RecordRef, player: &PlayerVars) -> ItemDisplay {
        item_display_of(self.data(), self.pids(), what, player)
    }

    /// The vendor or barter window of a `VendorShown` (§7.2–§7.3, §7.7). `wallet_coin` is
    /// the player's last `WalletChanged { Coin }` amount; `held` gives the player's held
    /// amount of a cost item (a wallet item's last amount, or the backpack's sum).
    #[must_use]
    pub fn vendor_window(
        &self,
        entries: &[VendorEntryIn],
        wallet_coin: Option<u64>,
        held: &dyn Fn(&RecordRef) -> u64,
        player: &PlayerVars,
    ) -> VendorWindow {
        let entries = &entries[..entries.len().min(MAX_ENTRIES)];
        if entries.iter().any(|e| !e.price.items.is_empty()) {
            VendorWindow::Barter(
                entries
                    .iter()
                    .map(|e| self.barter_entry(e, wallet_coin, held, player))
                    .collect(),
            )
        } else {
            VendorWindow::Vendor(
                entries
                    .iter()
                    .map(|e| self.vendor_entry(e, wallet_coin, player))
                    .collect(),
            )
        }
    }

    /// One vendor or buy-back entry (§7.2, §7.8).
    #[must_use]
    pub fn vendor_entry(
        &self,
        e: &VendorEntryIn,
        wallet_coin: Option<u64>,
        player: &PlayerVars,
    ) -> VendorEntry {
        VendorEntry {
            entry: e.entry,
            item: self.item_display(&e.what, player),
            money: self.coins(e.price.coin, CoinOptions::default()),
            too_expensive: wallet_coin.is_some_and(|w| e.price.coin > w),
            stock: e.stock,
            quantity: e.quantity,
            buy_ghosted: e.stock == Some(0),
        }
    }

    fn barter_entry(
        &self,
        e: &VendorEntryIn,
        wallet_coin: Option<u64>,
        held: &dyn Fn(&RecordRef) -> u64,
        player: &PlayerVars,
    ) -> BarterEntry {
        let mut costs: Vec<CostSlot> = e
            .price
            .items
            .iter()
            .take(MAX_ENTRIES)
            .map(|(what, quantity)| {
                let h = held(what);
                CostSlot::Item {
                    item: self.item_display(what, player),
                    quantity: *quantity,
                    held: h,
                    short: h < u64::from(*quantity),
                }
            })
            .collect();
        if e.price.coin > 0 {
            costs.push(CostSlot::Coin {
                money: self.coins(e.price.coin, CoinOptions::default()),
                short: wallet_coin.is_some_and(|w| e.price.coin > w),
            });
        }
        let overflow = if costs.len() > BARTER_COST_SLOTS {
            costs.split_off(BARTER_COST_SLOTS)
        } else {
            Vec::new()
        };
        BarterEntry {
            entry: e.entry,
            reward: self.item_display(&e.what, player),
            slots: costs,
            overflow,
            stock: e.stock,
        }
    }

    /// The loot window of a `LootShown` with its untaken entries (§8.2, §8.3).
    #[must_use]
    pub fn loot_view(
        &self,
        untaken: &[LootEntryIn],
        coin: u64,
        source: LootSource,
        player: &PlayerVars,
    ) -> LootView {
        let rows: Vec<LootRow> = untaken
            .iter()
            .take(MAX_ENTRIES)
            .map(|e| LootRow {
                entry: e.entry,
                item: self.item_display(&e.what, player),
                quantity: e.quantity,
            })
            .collect();
        LootView {
            coin: (coin > 0).then(|| self.coins(coin, CoinOptions::default())),
            button_label: if rows.is_empty() {
                tokens::LOOT_EMPTY
            } else {
                tokens::LOOT_ALL
            },
            rows,
            background: match source {
                LootSource::Agent => LootBackground::Corpse,
                LootSource::Object => LootBackground::Chest,
            },
        }
    }
}

/// The display of an item over the data (shared with the reward rows).
pub(crate) fn item_display_of(
    data: &dyn PresentationData,
    p: &NpcPids,
    what: &RecordRef,
    player: &PlayerVars,
) -> ItemDisplay {
    let did = match what {
        RecordRef::Literal(s) => {
            return ItemDisplay {
                what: what.clone(),
                name: Some(RichText::plain(s)),
                quality_state: OBJECT_COLOR_STATE,
                icon: None,
                stack_size: 1,
                category: EntryCategory::Literal,
            };
        }
        RecordRef::Record(d) => *d,
    };
    let set = match data.record(did) {
        Ok(Some(s)) => s,
        Ok(None) | Err(_) => {
            return ItemDisplay {
                what: what.clone(),
                name: None,
                quality_state: OBJECT_COLOR_STATE,
                icon: None,
                stack_size: 1,
                category: EntryCategory::RecordUnreadable,
            };
        }
    };
    let name = render_opt(data, set.string_info(p.name), player);
    let icon = IconLayers::of(&set, p);
    let category = if name.is_none() {
        EntryCategory::NoName
    } else if icon.is_none_or(|l| l.stack().is_empty()) {
        EntryCategory::NoIcon
    } else {
        EntryCategory::Shown
    };
    ItemDisplay {
        what: what.clone(),
        name,
        quality_state: quality_state_label(set.enum_key(p.quality)),
        icon,
        stack_size: set.int(p.max_quantity).unwrap_or(1),
        category,
    }
}

/// Renders an optional StringInfo with the player variables; `None` for absent, failing or
/// empty texts.
pub(crate) fn render_opt(
    data: &dyn PresentationData,
    info: Option<&tessera_formats::StringInfo>,
    player: &PlayerVars,
) -> Option<RichText> {
    let r: TextResult = data.render(TextRef::Info(info?), &player.values);
    r.ok()
        .flatten()
        .filter(|t| !t.plain_text().trim().is_empty())
}
