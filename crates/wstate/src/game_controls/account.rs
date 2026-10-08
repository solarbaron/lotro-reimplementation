// SPDX-License-Identifier: GPL-3.0-or-later
//! Account and store controls (`game-controls.md` §6). Server rule data: the views
//! give the values; how a server combines them is OPEN (Q11, Q24, Q30).

use tessera_formats::{Did, StringInfo};
use tessera_props::{PropertyRegistry, PropertySet};

use crate::character::Fields;
use crate::error::Result;

/// One `PerkControl_ChannelDataEntry` (§6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PerkChannel {
    /// `PerkControl_Channel`.
    pub channel: Option<u32>,
    /// `PerkControl_UICategory`.
    pub ui_category: Option<u32>,
    /// `PerkControl_Channel_Interval`.
    pub interval: Option<i32>,
    /// `PerkControl_Channel_Cap`.
    pub cap: Option<i32>,
}

/// PerkControl (§6.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PerkControl {
    /// `PerkControl_ChannelData`.
    pub channels: Vec<PerkChannel>,
    /// `Perk_AvailablePerks` (non-null DIDs).
    pub available: Vec<Did>,
}

impl PerkControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] for a property of an unexpected type,
    /// [`crate::WStateError::TooMany`] past the view limit.
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "perk control");
        Ok(Self {
            channels: f.struct_list("PerkControl_ChannelData", |s| {
                Ok(PerkChannel {
                    channel: s.enum_("PerkControl_Channel")?,
                    ui_category: s.enum_("PerkControl_UICategory")?,
                    interval: s.int("PerkControl_Channel_Interval")?,
                    cap: s.int("PerkControl_Channel_Cap")?,
                })
            })?,
            available: f.did_list("Perk_AvailablePerks")?,
        })
    }

    /// GCI8: the channels are pairwise distinct and so are the UI categories.
    #[must_use]
    pub fn keys_are_distinct(&self) -> bool {
        distinct(self.channels.iter().map(|c| c.channel))
            && distinct(self.channels.iter().map(|c| c.ui_category))
    }

    /// Whether `perk`'s channel and UI category are among the control's (GCI8).
    #[must_use]
    pub fn admits(&self, perk: &Perk) -> bool {
        perk.channel
            .is_some_and(|c| self.channels.iter().any(|e| e.channel == Some(c)))
            && perk
                .ui_category
                .is_some_and(|c| self.channels.iter().any(|e| e.ui_category == Some(c)))
    }
}

/// Whether the present values are pairwise distinct and none is absent.
pub(crate) fn distinct<T: Ord>(values: impl Iterator<Item = Option<T>>) -> bool {
    let mut seen = std::collections::BTreeSet::new();
    for v in values {
        let Some(v) = v else { return false };
        if !seen.insert(v) {
            return false;
        }
    }
    true
}

/// A perk (class 1377 here, §6.1).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Perk {
    /// `Perk_Name`.
    pub name: Option<StringInfo>,
    /// `Perk_Desc`.
    pub description: Option<StringInfo>,
    /// `Perk_Icon` (a 0x41 surface).
    pub icon: Option<Did>,
    /// `Perk_Effect`.
    pub effect: Option<Did>,
    /// `Perk_Channel`.
    pub channel: Option<u32>,
    /// `Perk_UICategory`.
    pub ui_category: Option<u32>,
    /// `Perk_SessionPointCost`.
    pub session_point_cost: Option<i32>,
    /// `Perk_MinLevel`.
    pub min_level: Option<i32>,
    /// `Perk_UsableAtLevelCap`.
    pub usable_at_level_cap: Option<bool>,
    /// `WebStoreInventoryItem_DataID`.
    pub store_item: Option<Did>,
}

impl Perk {
    /// Reads a perk's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] for a property of an unexpected type.
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "perk");
        Ok(Self {
            name: f.string_info("Perk_Name")?,
            description: f.string_info("Perk_Desc")?,
            icon: f.did("Perk_Icon")?,
            effect: f.did("Perk_Effect")?,
            channel: f.enum_("Perk_Channel")?,
            ui_category: f.enum_("Perk_UICategory")?,
            session_point_cost: f.int("Perk_SessionPointCost")?,
            min_level: f.int("Perk_MinLevel")?,
            usable_at_level_cap: f.boolean("Perk_UsableAtLevelCap")?,
            store_item: f.did("WebStoreInventoryItem_DataID")?,
        })
    }
}

/// CollectionControl (§6.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CollectionControl {
    /// `CollectionControl_CollectionList` (non-null DIDs).
    pub collections: Vec<Did>,
}

impl CollectionControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`], [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "collection control");
        Ok(Self {
            collections: f.did_list("CollectionControl_CollectionList")?,
        })
    }
}

/// A collection (class 3996 here, §6.2). The completion rule is OPEN (Q30).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Collection {
    /// `Collection_Name`.
    pub name: Option<StringInfo>,
    /// `Collection_Category`.
    pub category: Option<u32>,
    /// `Collection_DID_PieceList` (skills).
    pub pieces: Vec<Did>,
    /// `Collection_TreasureDID`.
    pub treasure: Option<Did>,
    /// `Usage_Permission`.
    pub usage_permission: Option<Did>,
    /// `Usage_RequiredRaces` (species keys).
    pub required_races: Vec<u32>,
}

impl Collection {
    /// Reads a collection's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`], [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "collection");
        Ok(Self {
            name: f.string_info("Collection_Name")?,
            category: f.enum_("Collection_Category")?,
            pieces: f.did_list("Collection_DID_PieceList")?,
            treasure: f.did("Collection_TreasureDID")?,
            usage_permission: f.did("Usage_Permission")?,
            required_races: f.enum_list("Usage_RequiredRaces")?,
        })
    }
}

/// The service fields of the paper-item control (§6.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PaperItemServices {
    /// `PaperItemServicesArray` (non-null DIDs).
    pub services: Vec<Did>,
    /// `PaperItemControl_PVPCurrency`.
    pub pvp_currency: Option<Did>,
    /// `PaperItemControl_WalletUpgrade_WebstoreItem`.
    pub wallet_upgrade: Option<Did>,
}

impl PaperItemServices {
    /// Reads the service fields of the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`], [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "paper-item control");
        Ok(Self {
            services: f.did_list("PaperItemServicesArray")?,
            pvp_currency: f.did("PaperItemControl_PVPCurrency")?,
            wallet_upgrade: f.did("PaperItemControl_WalletUpgrade_WebstoreItem")?,
        })
    }
}

/// A paper-item service entry (§6.3). What a service does is OPEN (Q11).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PaperItemService {
    /// `PaperItemServiceEntry_Type`.
    pub service_type: Option<u32>,
    /// `PaperItemServiceEntry_PaperItemDID` (the currency paid).
    pub paper_item: Option<Did>,
    /// `PaperItemServiceEntry_Cost_RipCordID`.
    pub cost: Option<Did>,
    /// `PaperItemServiceEntry_Enabled_RipCordID`.
    pub enabled: Option<Did>,
    /// `PaperItemServiceEntry_PurchaseDescription`.
    pub purchase_description: Option<StringInfo>,
    /// `PaperItemServiceEntry_PurchasedMessage`.
    pub purchased_message: Option<StringInfo>,
    /// `PaperItemService_TemporaryServiceEffect`.
    pub temporary_effect: Option<Did>,
    /// `PaperItemService_InductionActionDID`.
    pub induction_action: Option<Did>,
}

impl PaperItemService {
    /// Reads a service entry's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`] for a property of an unexpected type.
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "paper-item service");
        Ok(Self {
            service_type: f.enum_("PaperItemServiceEntry_Type")?,
            paper_item: f.did("PaperItemServiceEntry_PaperItemDID")?,
            cost: f.did("PaperItemServiceEntry_Cost_RipCordID")?,
            enabled: f.did("PaperItemServiceEntry_Enabled_RipCordID")?,
            purchase_description: f.string_info("PaperItemServiceEntry_PurchaseDescription")?,
            purchased_message: f.string_info("PaperItemServiceEntry_PurchasedMessage")?,
            temporary_effect: f.did("PaperItemService_TemporaryServiceEffect")?,
            induction_action: f.did("PaperItemService_InductionActionDID")?,
        })
    }
}

/// One `RandomItemReward_Tier_Entry` (§6.6).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RewardTier {
    /// `RandomItemReward_Tier_Title`.
    pub title: Option<StringInfo>,
    /// `RandomItemReward_Tier_Description`.
    pub description: Option<StringInfo>,
    /// `RandomItemReward_Tier_NPC` (an NPC template).
    pub npc: Option<Did>,
    /// `RandomItemReward_Tier_NPC_Icon` (a 0x41 surface).
    pub npc_icon: Option<Did>,
    /// `RandomItemReward_Tier_VXP` (the reward item).
    pub reward_item: Option<Did>,
}

/// RandomItemRewardControl (§6.6).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RandomItemRewardControl {
    /// Every array-valued property of the control whose elements are tier entries, by
    /// PID: (PID, tiers). The client picks the array by the PID a login reward names
    /// (§6.6); `RandomItemReward_Silver_Category` and `…_Gold_Category` observed.
    pub categories: Vec<(u32, Vec<RewardTier>)>,
    /// `RandomItemRewardControl_ChanceToShowRarest` (its use is OPEN, Q12).
    pub chance_to_show_rarest: Option<f32>,
}

impl RandomItemRewardControl {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`], [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "random-item reward control");
        let mut categories = Vec::new();
        for name in [
            "RandomItemReward_Silver_Category",
            "RandomItemReward_Gold_Category",
        ] {
            let Some(pid) = reg.pid(name) else { continue };
            if f.get(name).is_none() {
                continue;
            }
            let tiers = f.struct_list(name, |s| {
                Ok(RewardTier {
                    title: s.string_info("RandomItemReward_Tier_Title")?,
                    description: s.string_info("RandomItemReward_Tier_Description")?,
                    npc: s.did("RandomItemReward_Tier_NPC")?,
                    npc_icon: s.did("RandomItemReward_Tier_NPC_Icon")?,
                    reward_item: s.did("RandomItemReward_Tier_VXP")?,
                })
            })?;
            categories.push((pid, tiers));
        }
        Ok(Self {
            categories,
            chance_to_show_rarest: f.float("RandomItemRewardControl_ChanceToShowRarest")?,
        })
    }

    /// The tiers of the category property `pid` (a login reward's
    /// `LoginReward_ControlCategory_PropertyName`, §6.6). The listing order the client
    /// uses is OPEN (Q12); this is the stored order.
    #[must_use]
    pub fn tiers(&self, pid: u32) -> Option<&[RewardTier]> {
        self.categories
            .iter()
            .find(|(p, _)| *p == pid)
            .map(|(_, t)| t.as_slice())
    }
}

/// A login reward (class 3731 here, §6.7).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoginReward {
    /// `LoginReward_ResetSchedule`.
    pub reset_schedule: Option<u32>,
    /// `LoginReward_AllowedAccountTypes` (first 64 bits).
    pub allowed_account_types: Option<u64>,
    /// `LoginReward_AlertState`.
    pub alert_state: Option<u32>,
    /// `LoginReward_ControlCategory_PropertyName`: the PID of the
    /// [`RandomItemRewardControl`] category to show.
    pub control_category: Option<u32>,
    /// `LoginReward_TreasureTableArray` (non-null DIDs).
    pub treasure_tables: Vec<Did>,
    /// `LoginReward_TreasureTableIndex_WorldProperty`.
    pub treasure_index_property: Option<u32>,
    /// `LoginReward_XPProgressionsProgression`.
    pub xp_progressions: Option<Did>,
    /// `LoginReward_XPMultiplier_RipCord`.
    pub xp_multiplier: Option<Did>,
    /// `LoginReward_RerollCostOverride_RipCord`.
    pub reroll_cost: Option<Did>,
}

impl LoginReward {
    /// Reads a login reward's property half.
    ///
    /// # Errors
    ///
    /// [`crate::WStateError::BadProperty`], [`crate::WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "login reward");
        Ok(Self {
            reset_schedule: f.enum_("LoginReward_ResetSchedule")?,
            allowed_account_types: f.bits("LoginReward_AllowedAccountTypes")?,
            alert_state: f.enum_("LoginReward_AlertState")?,
            control_category: f.pid("LoginReward_ControlCategory_PropertyName")?,
            treasure_tables: f.did_list("LoginReward_TreasureTableArray")?,
            treasure_index_property: f.pid("LoginReward_TreasureTableIndex_WorldProperty")?,
            xp_progressions: f.did("LoginReward_XPProgressionsProgression")?,
            xp_multiplier: f.did("LoginReward_XPMultiplier_RipCord")?,
            reroll_cost: f.did("LoginReward_RerollCostOverride_RipCord")?,
        })
    }
}
