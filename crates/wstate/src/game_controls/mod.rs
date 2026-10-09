// SPDX-License-Identifier: GPL-3.0-or-later
//! Game-control singletons: the structural kinds of every WEENIECONTENT entry, the
//! generic forms (empty markers, DID sets, manager entities, compiled-only tables) and
//! typed views of the PvP, mount, account, UI and social controls.
//!
//! Spec: `docs/specs/game-controls.md`.
//!
//! No record here needs a byte decoder of its own (§3.1): the property controls are read
//! from their decoded property half ([`tessera_props::PropertySet`], DID + 0x09000000),
//! and the compiled-only forms (§3.3, §4.2, §4.10) from their decoded object state
//! ([`crate::ObjectState`]). Records are found by WEENIECONTENT label and properties by
//! dictionary name at run time. The only build-specific numbers are the class indices of
//! the kind rule (§2.1), kept in one replaceable table ([`ClassIndices`]).
//!
//! | Item | Records | Spec |
//! |---|---|---|
//! | [`EntryKind`], [`ClassIndices`], [`classify`], [`WeenieIndex`] | the ten structural kinds of the WEENIECONTENT entries | §2.1 |
//! | [`SCOPE_LABELS`], [`Group`], [`WEAPON_STYLE_LABELS`] | the 119 labels of this spec | §2.2, §5.5 |
//! | [`is_empty_marker`], [`DidSet`], [`TemplateDiff`], [`entity_name`] | empty markers, DID sets, manager entities | §3.2–§3.4 |
//! | [`SlotSchedule`], [`AdvancementTable`], [`ModifierTable`] | compiled-only tables | §4.2, §4.10 |
//! | [`MountDirectory`], [`MountTrait`], [`MountedCombatControl`], [`DuelTimings`] | mounts, mounted gait multipliers, duel timings | §5 |
//! | [`PerkControl`], [`Perk`], [`CollectionControl`], [`Collection`], [`PaperItemServices`], [`PaperItemService`], [`RandomItemRewardControl`], [`LoginReward`] | account and store | §6 |
//! | [`VitalDisplayOverrideControl`], [`VitalDisplayOverride`], [`item_vdo_unlocks`], [`AlertsControl`], [`ResetScheduleControl`], [`ConsiderationControl`], [`ConsiderationTable`], [`TrackingStateControl`], [`DiscoveryDistanceControl`], [`CombatMusicControl`], [`AggroPingControl`], [`OpenTappingControl`] | UI and system rules | §7 |
//! | [`GuildControl`] | social controls | §8.1 |
//!
//! # Hostile input
//!
//! The views copy scalars and DIDs out of sets and states that the lower layers have
//! already decoded within their own bounds. The choices follow §3.1: every list a view
//! keeps is checked against [`MAX_VIEW_ENTRIES`] before it is collected (Tessera
//! Decision); a present property of the wrong type is [`WStateError::BadProperty`]
//! (Tessera Decision), except for the client's native reads of §5.3.2, §5.7 and §8.1,
//! which treat it as absent and use their fallback (client behaviour); DID lists drop
//! null elements except the consideration lists of §7.5, which keep their 120 positions
//! (Tessera Decision). A compiled-only record of another layout is
//! [`WStateError::BadAttribute`] or [`WStateError::WrongKind`]; nothing indexes by a
//! data-derived value.

mod account;
mod generic;
mod kinds;
mod mounts;
mod ui;

#[cfg(test)]
mod tests;

pub use account::{
    Collection, CollectionControl, LoginReward, PaperItemService, PaperItemServices, Perk,
    PerkChannel, PerkControl, RandomItemRewardControl, RewardTier,
};
pub use generic::{
    ADVANCEMENT_ATTRIBUTE, AdvancementTable, DID_SET_ATTRIBUTE, DidSet, EMPTY_MARKER_STATE_LEN,
    EMPTY_PROPERTY_SET_LEN, ENTITY_NAME_LABEL, MODIFIER_ATTRIBUTE, ModifierTable, SlotSchedule,
    TemplateDiff, entity_name, is_empty_marker,
};
pub use kinds::{ClassIndices, EntryKind, IndexProblem, WeenieEntry, WeenieIndex, classify};
pub use mounts::{
    DuelTimings, DyeSlot, FlankConstraint, GAIT_CANTER, GAIT_GALLOP, GAIT_NEUTRAL, GAIT_TROT,
    GAIT_WALK, MountDirectory, MountTrait, MountTree, MountedCombatControl, TraitAppearance,
    TreeSkills, is_default_attack,
};
pub use ui::{
    AggroPingControl, AlertsControl, CombatMusicControl, ConsiderationControl, ConsiderationTable,
    DiscoveryDistanceControl, GuildControl, OpenTappingControl, PLAYER_NOTE_DEFAULT, ResetSchedule,
    ResetScheduleControl, TrackingStateControl, USAGE_PROPERTY_ARRAY, VDO_UNLOCK_LABEL,
    VitalDisplayOverride, VitalDisplayOverrideControl, bit_keys, item_vdo_unlocks, unlocked_key,
};

pub use crate::character::MAX_VIEW_ENTRIES;
use crate::error::WStateError;

/// The group of §2.2 a label belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Group {
    /// Empty markers, DID sets and manager entities (§3).
    Generic,
    /// PvP and monster play (§4).
    PvpMonsterPlay,
    /// Mounts, default attacks and combat rules (§5).
    MountsCombat,
    /// Account and store controls (§6).
    AccountStore,
    /// UI and system rules (§7).
    UiSystem,
    /// Social managers, data side (§8).
    Social,
}

/// The 17 weapon-style default-attack labels (§5.5).
pub const WEAPON_STYLE_LABELS: [&str; 17] = [
    "UNARMED",
    "1H",
    "1H_1H",
    "1H_Shield",
    "2H",
    "BOW",
    "CROSSBOW",
    "JAVELIN",
    "MOUNTED_UNARMED",
    "MOUNTED_1H",
    "MOUNTED_1H_1H",
    "MOUNTED_1H_Shield",
    "MOUNTED_2H",
    "MOUNTED_BOW",
    "MOUNTED_CROSSBOW",
    "MOUNTED_JAVELIN",
    "MOUNTED_RANGED_FALLBACK",
];

/// The 14 manager-entity labels (§3.4).
pub const MANAGER_LABELS: [&str; 14] = [
    "PVPManager",
    "GuildManager",
    "FellowshipManager",
    "FellowshipMakerManager",
    "MailManager",
    "AuctionManager",
    "HousingAuthority",
    "CharacterSheetManager",
    "PopulationTrackingManager",
    "RepositoryManager",
    "WorldJoinManager",
    "TrackingManager",
    "ScriptManager",
    "NeighborManager",
];

/// The six DID-set labels (§3.3).
pub const DID_SET_LABELS: [&str; 6] = [
    "WebStore",
    "MountAppearanceTraits",
    "LoginRewards",
    "BigBattles",
    "StorePropertyManipulator",
    "LootGenControl_RewardOnceTGPs",
];

/// The 119 labels of this spec (§2.2), each once, with the group of its section. A label
/// in two groups of the spec's table (MountAppearanceTraits, LoginRewards) is listed under
/// the section that describes its data. The labels are DID-map labels of the install
/// (interoperability identifiers); their DIDs are read at run time.
pub const SCOPE_LABELS: [(&str, Group); 119] = {
    use Group::{AccountStore as A, Generic as G, MountsCombat as M, PvpMonsterPlay as P};
    use Group::{Social as S, UiSystem as U};
    [
        // §3.2: the 38 dialog markers.
        ("Dialog_AbandonChanneling", G),
        ("Dialog_AbandonPrivateEncounter", G),
        ("Dialog_AbandonQuest", G),
        ("Dialog_CreateRelationship", G),
        ("Dialog_DisenchantItemConfirmation", G),
        ("Dialog_DuelingInvite", G),
        ("Dialog_DyeItemConfirmation", G),
        ("Dialog_FellowRecruitConfirmation", G),
        ("Dialog_FellowshipMakerConfirmationDialogCallback", G),
        ("Dialog_FellowshipVoteDialogCallback", G),
        ("Dialog_FinishSessionQuest", G),
        ("Dialog_GuildConfirmation", G),
        ("Dialog_GuildCreationConfirmation", G),
        ("Dialog_GuildThemeChangeConfirmation", G),
        ("Dialog_HousingWritConfirmation", G),
        ("Dialog_IATargetUsageConfirmation", G),
        ("Dialog_ItemAdvancementBind", G),
        ("Dialog_LeaveMP", G),
        ("Dialog_LeavePrivateEncounter", G),
        ("Dialog_Logout", G),
        ("Dialog_MasterLootConfirmation", G),
        ("Dialog_PerkConfirmation", G),
        ("Dialog_PetRenameConfirmation", G),
        ("Dialog_PrivateEncounter", G),
        ("Dialog_ReadyCheck", G),
        ("Dialog_RecallConfirmation", G),
        ("Dialog_ReportSpammerConfirmation", G),
        ("Dialog_SellAllConfirmation", G),
        ("Dialog_ShareQuest", G),
        ("Dialog_SkillConfirmationDialogCallback", G),
        ("Dialog_SummonConfirmation", G),
        ("Dialog_TraceryUpgradeItemConfirmation", G),
        ("Dialog_UsageConfirmation", G),
        ("Dialog_WardrobeAddDyeConfirmation", G),
        ("Dialog_WardrobeItemRemovalConfirmation", G),
        ("Dialog_WJDequeueDialogCallback", G),
        ("Dialog_WJQueueDialogCallback", G),
        ("Dialog_WJRequestDialogCallback", G),
        // §3.3: DID sets (MountAppearanceTraits under §5, LoginRewards under §6).
        ("WebStore", G),
        ("BigBattles", G),
        ("StorePropertyManipulator", G),
        ("LootGenControl_RewardOnceTGPs", G),
        // §3.4: manager entities.
        ("PVPManager", G),
        ("GuildManager", G),
        ("FellowshipManager", G),
        ("FellowshipMakerManager", G),
        ("MailManager", G),
        ("AuctionManager", G),
        ("HousingAuthority", G),
        ("CharacterSheetManager", G),
        ("PopulationTrackingManager", G),
        ("RepositoryManager", G),
        ("WorldJoinManager", G),
        ("TrackingManager", G),
        ("ScriptManager", G),
        ("NeighborManager", G),
        // §4.
        ("PVPControl", P),
        ("PVPVirtueControl", P),
        ("PVPCombatControl", P),
        ("PVPFeatControl", P),
        ("GloryControl", P),
        ("MonsterPlayControl", P),
        ("MPAdvancementControl", P),
        ("MPLevelTableDirectory", P),
        ("MonsterPlayLevelDirectory", P),
        ("PvPPlayLevelDirectory", P),
        // §5.
        ("MountDirectory", M),
        ("MountAppearanceTraits", M),
        ("MountedCombatControl", M),
        ("TravelMountSpeedBoostOverride", M),
        ("UNARMED", M),
        ("1H", M),
        ("1H_1H", M),
        ("1H_Shield", M),
        ("2H", M),
        ("BOW", M),
        ("CROSSBOW", M),
        ("JAVELIN", M),
        ("MOUNTED_UNARMED", M),
        ("MOUNTED_1H", M),
        ("MOUNTED_1H_1H", M),
        ("MOUNTED_1H_Shield", M),
        ("MOUNTED_2H", M),
        ("MOUNTED_BOW", M),
        ("MOUNTED_CROSSBOW", M),
        ("MOUNTED_JAVELIN", M),
        ("MOUNTED_RANGED_FALLBACK", M),
        ("WeaponDamageVarianceTable", M),
        ("CombatControl", M),
        ("CombatStateControl", M),
        ("ResistControl", M),
        // §6.
        ("PerkControl", A),
        ("CollectionControl", A),
        ("PaperItemControl", A),
        ("TurbinePointsControl", A),
        ("RewardTrackControl", A),
        ("RandomItemRewardControl", A),
        ("LoginRewards", A),
        // §7.
        ("gmVitalDisplayOverrideControl", U),
        ("AlertsControl", U),
        ("ResetScheduleControl", U),
        ("ConsiderationControl", U),
        ("DistantBattleUnitFormationControl", U),
        ("AggroPingControl", U),
        ("CombatMusicControl", U),
        ("DiscoveryDistanceControl", U),
        ("TrackingStateControl", U),
        ("ShapeshiftControl", U),
        ("PetControl", U),
        ("MonsterPetControl", U),
        ("PetChannelControl", U),
        ("PruneInfo_Skill", U),
        ("NewbieCinematicEncounter", U),
        ("OpenTappingControl", U),
        ("GRSControl", U),
        // §8.
        ("GuildControl", S),
        ("MailControl", S),
        ("AuctionControl", S),
        ("HousingControl", S),
    ]
};

/// The group of `label` in [`SCOPE_LABELS`], or `None` outside this spec's scope.
#[must_use]
pub fn scope_group(label: &str) -> Option<Group> {
    SCOPE_LABELS
        .iter()
        .find(|(l, _)| *l == label)
        .map(|&(_, g)| g)
}

fn bad_property(
    record: &'static str,
    property: &'static str,
    problem: &'static str,
) -> WStateError {
    WStateError::BadProperty {
        record,
        property,
        problem,
    }
}
