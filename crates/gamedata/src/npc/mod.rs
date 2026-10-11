// SPDX-License-Identifier: GPL-3.0-or-later
//! The NPC windows' presentation model (decision 0024 P3): dialogue page texts, the
//! comment and choice windows, quest-offer content with its reward rows, vendor, barter,
//! buy-back and loot entries with their prices, coins and the wallet, and the census
//! categories of exit criterion data checks.
//!
//! Spec: `docs/specs/npc-windows.md` §5–§11 (`quests.md` §7, §9 for the records,
//! `item-tooltips.md` §6.2, §6.11 and `icon-layers.md` §3.1 for item names, coins and icon
//! layers). Windows and their controls belong to `tessera-ui` and the client; this module
//! never computes a price, a loot result, a reward amount or a quest state (decision 0024
//! §2 principle 4): prices, stock, quantities and amounts are the feed's.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`NpcPresenter`] | Builds every model below | §5–§9 |
//! | [`DialogueText`], [`DialogueLine`], [`LineCategory`], [`DialogueWindow`], [`dialogue_window`], [`CommentWindow`], [`ChoicePage`], [`ActionEntry`] | Dialogue pages and the three dialogue windows | §5 |
//! | [`QuestOfferView`], [`GroupType`], [`OfferSource`], [`RewardRow`], [`RewardKind`], [`RewardIn`] | The quest-offer window and its reward block | §6, §10.3 |
//! | [`ItemDisplay`], [`IconLayers`], [`EntryCategory`], [`Price`], [`VendorEntryIn`], [`VendorEntry`], [`BarterEntry`], [`CostSlot`], [`VendorWindow`], [`LootEntryIn`], [`LootView`], [`LootSource`] | Vendor, barter, buy-back and loot entries | §7, §8 |
//! | [`Currencies`], [`CoinDisplay`], [`Coin`], [`CoinKind`], [`CoinOptions`] | Splitting coin amounts and which coins show | §9.2, §9.3 |
//! | [`WalletCatalog`], [`WalletItem`], [`Currency`], [`WalletView`], [`WalletCategory`], [`WalletEntry`], [`WalletIcon`] | Wallet items, barter currencies and the wallet window | §9.4–§9.6 |
//! | [`NpcCensus`], [`OfferCategory`], [`OfferFlag`] | Census categories | §11 |
//!
//! # Hostile input
//!
//! Nothing here parses bytes. Work is bounded: at most [`MAX_ENTRIES`] entries per window,
//! [`MAX_WALLET_ITEMS`] wallet items (in the catalog and as distinct wallet currencies),
//! [`MAX_ACTION_ENTRIES`] action entries, nested property structs searched to depth
//! [`MAX_STRUCT_DEPTH`].

mod census;
mod dialogue;
mod entries;
mod money;
mod offer;

#[cfg(test)]
mod tests;

pub use census::{NpcCensus, OfferCategory, OfferFlag, offer_category};
pub use dialogue::{
    ActionEntry, ChoicePage, ChoiceRow, CommentWindow, DialogueLine, DialogueText, DialogueWindow,
    LineCategory, dialogue_window,
};
pub use entries::{
    BarterEntry, CostSlot, EntryCategory, IconLayers, ItemDisplay, LootBackground, LootEntryIn,
    LootRow, LootSource, LootView, Price, VendorEntry, VendorEntryIn, VendorWindow,
};
pub use money::{
    Coin, CoinDisplay, CoinKind, CoinOptions, Currencies, Currency, WalletCatalog, WalletCategory,
    WalletEntry, WalletIcon, WalletItem, WalletView,
};
pub use offer::{GroupType, OfferSource, QuestOfferView, RewardIn, RewardKind, RewardRow};

pub use crate::quest::{
    InstallPresentationData, PlayerContext, PlayerVars, PresentationData, RecordRef,
};

pub(crate) use offer::{offer_line_of, treasure_rows_of};

use std::sync::OnceLock;

use tessera_formats::Did;

use crate::quest::QuestPresenter;

/// Most entries of one window (`scenario-feed.md` §6.3: 1,024 per list).
pub const MAX_ENTRIES: usize = 1_024;
/// Most wallet items read from the wallet control record, and most distinct item
/// currencies a wallet view lists (a Tessera bound).
pub const MAX_WALLET_ITEMS: usize = 16_384;
/// Most action entries read from the choice window's layout (a Tessera bound).
pub const MAX_ACTION_ENTRIES: usize = 1_024;
/// Deepest nested struct searched in a wallet control entry (a Tessera bound).
pub const MAX_STRUCT_DEPTH: usize = 4;
/// Most cost slots of a barter entry (`npc-windows.md` §7.7).
pub const BARTER_COST_SLOTS: usize = 3;

/// The `UI` string table (coin tooltips, coin counts, loot button labels).
pub const UI_TABLE: &str = "UI";

/// Tokens of `npc-windows.md`.
pub mod tokens {
    /// `UI` coin-count format (§9.2; `item-tooltips.md` §6.11).
    pub const COIN_COUNT: u32 = 0x0748_0C44;
    /// Its count variable.
    pub const VAR_COIN_COUNT: u32 = 0x0052_42A9;
    /// `UI` coin tooltips: gold, silver, copper (§9.2).
    pub const COIN_TOOLTIPS: [u32; 3] = [0x0135_BA5C, 0x01E4_A5BC, 0x0BE0_7C7C];
    /// `UI` loot button label with at least one listed entry (§8.3).
    pub const LOOT_ALL: u32 = 0x0A5A_16DC;
    /// `UI` loot button label with no listed entry (§8.3).
    pub const LOOT_EMPTY: u32 = 0x02E1_5B29;
}

tessera_formats::property_ids! {
    /// Properties the NPC-window models read.
    pub(crate) struct NpcPids {
        name = "Name",
        description = "Description",
        quality = "Item_Quality",
        max_quantity = "Inventory_MaxQuantity",
        icon_background = "Icon_Layer_BackgroundDID",
        icon_underlay = "Icon_Layer_UnderlayDID",
        icon_shadow = "Icon_Layer_ShadowDID",
        icon_image = "Icon_Layer_ImageDID",
        icon_overlay = "Icon_Layer_OverlayDID",
        currency_list = "Currency_CurrencyList",
        currency_value = "Currency_Value",
        paper_category = "PaperItem_Category",
        paper_small_icon = "PaperItem_SmallIcon",
        paper_free = "PaperItemControl_Free_PaperItemArray",
        paper_list = "PaperItemControl_PaperItemArray",
        paper_item = "PaperItemControl_Item",
        paper_cap = "PaperItemControl_CapDefault",
        trait_name = "Trait_Name",
        trait_icon = "Trait_Icon",
        trait_overlay = "Trait_Static_Icon_Overlay",
        title_string = "Title_String",
        emote_command = "Emote_CommandString",
        faction_name = "Reputation_Faction_Name",
        comment_content = "NPCComment_Content",
        action_entries = "ActionPickerUI_ActionEntryArray",
        action = "ActionPickerUI_Action",
        action_template = "ActionPickerUI_ListEntryTemplate",
        action_text_property = "ActionPickerUI_EntryTextProperty",
        action_default_text = "ActionPickerUI_DefaultTextEntry",
        skill_name = "Skill_Name",
        effect_name = "Effect_Name",
        travel_name = "TravelDisplayName",
        area_name = "Area_Name",
        dungeon_name = "Dungeon_Name",
        recipe_name = "CraftRecipe_Name",
        profession_name = "CraftProfession_Name",
        instance_name = "PrivateEncounterTemplate_Name",
    }
}

/// The NPC windows' presentation over a [`PresentationData`]: dialogue, offers, entries,
/// coins and the wallet. It owns a [`QuestPresenter`] (names, levels, offer lines), reachable
/// through [`NpcPresenter::quest`]; a client that needs both uses that one rather than
/// building a second, so the arc directory and the other caches are loaded once.
pub struct NpcPresenter<'d> {
    quest: QuestPresenter<'d>,
    ui_table: Option<Did>,
    currencies: OnceLock<Currencies>,
    wallet: OnceLock<WalletCatalog>,
    actions: OnceLock<Vec<ActionEntry>>,
}

impl std::fmt::Debug for NpcPresenter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NpcPresenter")
            .field("ui_table", &self.ui_table)
            .finish_non_exhaustive()
    }
}

impl<'d> NpcPresenter<'d> {
    /// Resolves labels and tables once.
    #[must_use]
    pub fn new(data: &'d dyn PresentationData) -> Self {
        Self {
            quest: QuestPresenter::new(data),
            ui_table: data.table(UI_TABLE),
            currencies: OnceLock::new(),
            wallet: OnceLock::new(),
            actions: OnceLock::new(),
        }
    }

    /// The quest presenter it uses (names, levels, offer lines).
    #[must_use]
    pub fn quest(&self) -> &QuestPresenter<'d> {
        &self.quest
    }

    /// The data source.
    #[must_use]
    pub fn data(&self) -> &'d dyn PresentationData {
        self.quest.data()
    }

    pub(crate) fn pids(&self) -> &NpcPids {
        self.quest.npc_pids()
    }

    /// The player variables for `context`.
    #[must_use]
    pub fn player(&self, context: PlayerContext) -> PlayerVars {
        self.quest.player(context)
    }
}
