// SPDX-License-Identifier: GPL-3.0-or-later
//! Race and nationality tables, class creation fields, class priority, the creation check
//! and a new character's default property set (`player-character.md` §4.6, §5–§7).

use tessera_formats::{Did, StringInfo};
use tessera_props::{PropertyRegistry, PropertySet, Value};

use super::{Fields, mirror_base};
use crate::error::{Result, WStateError};

/// Race directory (§5.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RaceDirectory {
    /// `RaceTable_RaceTableList`: the race tables in the directory's order.
    pub tables: Vec<Did>,
}

impl RaceDirectory {
    /// Reads the directory's property half.
    ///
    /// # Errors
    ///
    /// [`WStateError::BadProperty`] for a property of an unexpected type,
    /// [`WStateError::TooMany`] past [`super::MAX_VIEW_ENTRIES`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "race directory");
        Ok(Self {
            tables: f.did_list("RaceTable_RaceTableList")?,
        })
    }
}

/// One `RaceTable_GenderStruct` (§5.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GenderEntry {
    /// `RaceTable_GenderType`: the sex key.
    pub gender: u32,
    /// `RaceTable_Gender_PlayerAvatar`: the race avatar (player template).
    pub player_avatar: Option<Did>,
    /// `RaceTable_Gender_AdminAvatar`: its administrator twin.
    pub admin_avatar: Option<Did>,
    /// `RaceTable_GenderNullAppearance`.
    pub null_appearance: Option<Did>,
    /// `RaceTable_RaceSelect_Entity`.
    pub race_select_entity: Option<Did>,
    /// `RaceTable_RaceSelect_Movie_DID`.
    pub movie: Option<Did>,
}

/// One `AdvTable_Trait_Entry` of a race's characteristics (§5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RaceTrait {
    /// `AdvTable_Trait_WC`.
    pub trait_did: Option<Did>,
    /// `AdvTable_Trait_Level`.
    pub level: Option<i32>,
    /// `AdvTable_Trait_Rank`.
    pub rank: Option<i32>,
}

/// Race table (§5.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RaceRecord {
    /// `RaceTable_Race`: the species key.
    pub race: u32,
    /// `RaceTable_GenderList`.
    pub genders: Vec<GenderEntry>,
    /// `RaceTable_ClassList`: allowed class keys.
    pub classes: Vec<u32>,
    /// `RaceTable_NationalityList`: allowed nationality keys.
    pub nationalities: Vec<u32>,
    /// `AdvTable_RaceCharacteristic_List`.
    pub characteristics: Vec<RaceTrait>,
    /// `Travel_PlayerHomeLocationArray`.
    pub home_locations: Vec<Did>,
    /// `RaceTable_StartingEncounter`.
    pub starting_encounter: Option<Did>,
    /// `RaceTable_BTS_StartingEncounter`.
    pub bts_starting_encounter: Option<Did>,
    /// `RaceTable_AllowSkipTutorial`.
    pub allow_skip_tutorial: Option<bool>,
    /// `RaceTable_IsTallRace`.
    pub tall_race: Option<bool>,
    /// `RaceTable_AccomplishmentDirectory`.
    pub accomplishment_directory: Option<Did>,
    /// `RaceTable_UIScene`.
    pub ui_scene: Option<Did>,
    /// `RaceTable_Chargen_Camera`.
    pub chargen_camera: Option<u32>,
    /// `RaceTable_Character_Select_Camera`.
    pub select_camera: Option<u32>,
    /// `RaceTable_Description`.
    pub description: Option<StringInfo>,
    /// `RaceTable_CharGen_ClassSelect_WebstoreItem`.
    pub webstore_item: Option<Did>,
}

impl RaceRecord {
    /// Reads a race table's property half.
    ///
    /// # Errors
    ///
    /// [`WStateError::BadProperty`] when `RaceTable_Race` or a gender entry's
    /// `RaceTable_GenderType` is absent, or a property has an unexpected type;
    /// [`WStateError::TooMany`] past [`super::MAX_VIEW_ENTRIES`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "race table");
        let race = f.require("RaceTable_Race", f.enum_("RaceTable_Race")?)?;
        let genders = f.struct_list("RaceTable_GenderList", |g| {
            Ok(GenderEntry {
                gender: g.require("RaceTable_GenderType", g.enum_("RaceTable_GenderType")?)?,
                player_avatar: g.did("RaceTable_Gender_PlayerAvatar")?,
                admin_avatar: g.did("RaceTable_Gender_AdminAvatar")?,
                null_appearance: g.did("RaceTable_GenderNullAppearance")?,
                race_select_entity: g.did("RaceTable_RaceSelect_Entity")?,
                movie: g.did("RaceTable_RaceSelect_Movie_DID")?,
            })
        })?;
        let characteristics = f.struct_list("AdvTable_RaceCharacteristic_List", |t| {
            Ok(RaceTrait {
                trait_did: t.did("AdvTable_Trait_WC")?,
                level: t.int("AdvTable_Trait_Level")?,
                rank: t.int("AdvTable_Trait_Rank")?,
            })
        })?;
        Ok(Self {
            race,
            genders,
            classes: f.enum_list("RaceTable_ClassList")?,
            nationalities: f.enum_list("RaceTable_NationalityList")?,
            characteristics,
            home_locations: f.did_list("Travel_PlayerHomeLocationArray")?,
            starting_encounter: f.did("RaceTable_StartingEncounter")?,
            bts_starting_encounter: f.did("RaceTable_BTS_StartingEncounter")?,
            allow_skip_tutorial: f.boolean("RaceTable_AllowSkipTutorial")?,
            tall_race: f.boolean("RaceTable_IsTallRace")?,
            accomplishment_directory: f.did("RaceTable_AccomplishmentDirectory")?,
            ui_scene: f.did("RaceTable_UIScene")?,
            chargen_camera: f.enum_("RaceTable_Chargen_Camera")?,
            select_camera: f.enum_("RaceTable_Character_Select_Camera")?,
            description: f.string_info("RaceTable_Description")?,
            webstore_item: f.did("RaceTable_CharGen_ClassSelect_WebstoreItem")?,
        })
    }

    /// The gender entry for `sex`, if the race has one (the first in stored order).
    #[must_use]
    pub fn gender(&self, sex: u32) -> Option<&GenderEntry> {
        self.genders.iter().find(|g| g.gender == sex)
    }
}

/// A (species, sex, nationality, class) choice at character creation (§5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CreationChoice {
    /// Species key (`RaceTable_Race`).
    pub species: u32,
    /// Sex key (`RaceTable_GenderType`).
    pub sex: u32,
    /// Nationality key.
    pub nationality: u32,
    /// Class key.
    pub class: u32,
}

/// Outcome of [`check_choice`] (§5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChoiceVerdict {
    /// All three rules hold.
    Valid,
    /// Rule 1: no race table has the species with a gender entry of the sex.
    NoSuchRaceAndSex,
    /// Rule 2: the nationality is not in the race's list.
    NationalityNotAllowed,
    /// Rule 3: the class is not in the race's list.
    ClassNotAllowed,
}

/// Checks a creation choice against the race tables (§5.4). Store items, account tokens
/// and access rights are ignored, as the spec allows for a sandbox without accounts.
#[must_use]
pub fn check_choice(races: &[RaceRecord], choice: &CreationChoice) -> ChoiceVerdict {
    let Some(race) = races
        .iter()
        .find(|r| r.race == choice.species && r.gender(choice.sex).is_some())
    else {
        return ChoiceVerdict::NoSuchRaceAndSex;
    };
    if !race.nationalities.contains(&choice.nationality) {
        return ChoiceVerdict::NationalityNotAllowed;
    }
    if !race.classes.contains(&choice.class) {
        return ChoiceVerdict::ClassNotAllowed;
    }
    ChoiceVerdict::Valid
}

/// Nationality directory (§6.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NationalityDirectory {
    /// `NationalityTable_NationalityTableList`.
    pub tables: Vec<Did>,
}

impl NationalityDirectory {
    /// Reads the directory's property half.
    ///
    /// # Errors
    ///
    /// As [`RaceDirectory::from_properties`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "nationality directory");
        Ok(Self {
            tables: f.did_list("NationalityTable_NationalityTableList")?,
        })
    }
}

/// One `NationalityTable_Naming_Guideline_Struct` (§6.2, §8.3).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NamingGuideline {
    /// `NationalityTable_Sex`.
    pub sex: Option<u32>,
    /// `NationalityTable_Naming_Guideline`.
    pub text: Option<StringInfo>,
}

/// One `AppearanceUI_DyeMapControlOverride` (§6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DyeOverride {
    /// `AppearanceUI_ControlPropertyName`: the appearance control's property.
    pub control_property: Option<u32>,
    /// `AppearanceUI_DyeMapImage`.
    pub image: Option<Did>,
}

/// Nationality table (§6.2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NationalityRecord {
    /// `NationalityTable_Nationality`.
    pub nationality: u32,
    /// `NationalityTable_Name`.
    pub name: Option<StringInfo>,
    /// `NationalityTable_Desc`.
    pub description: Option<StringInfo>,
    /// `NationalityTable_Icon`.
    pub icon: Option<Did>,
    /// `NationalityTable_Naming_Guideline_Array`.
    pub naming_guidelines: Vec<NamingGuideline>,
    /// `NationalityTable_Title`.
    pub title: Option<Did>,
    /// `Reputation_FactionTable`.
    pub factions: Vec<Did>,
    /// `AppearanceUI_DyeMapControlOverrides`.
    pub dye_overrides: Vec<DyeOverride>,
}

impl NationalityRecord {
    /// Reads a nationality table's property half.
    ///
    /// # Errors
    ///
    /// [`WStateError::BadProperty`] when `NationalityTable_Nationality` is absent or a
    /// property has an unexpected type; [`WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "nationality table");
        Ok(Self {
            nationality: f.require(
                "NationalityTable_Nationality",
                f.enum_("NationalityTable_Nationality")?,
            )?,
            name: f.string_info("NationalityTable_Name")?,
            description: f.string_info("NationalityTable_Desc")?,
            icon: f.did("NationalityTable_Icon")?,
            naming_guidelines: f.struct_list("NationalityTable_Naming_Guideline_Array", |g| {
                Ok(NamingGuideline {
                    sex: g.enum_("NationalityTable_Sex")?,
                    text: g.string_info("NationalityTable_Naming_Guideline")?,
                })
            })?,
            title: f.did("NationalityTable_Title")?,
            factions: f.did_list("Reputation_FactionTable")?,
            dye_overrides: f.struct_list("AppearanceUI_DyeMapControlOverrides", |o| {
                Ok(DyeOverride {
                    control_property: o.pid("AppearanceUI_ControlPropertyName")?,
                    image: o.did("AppearanceUI_DyeMapImage")?,
                })
            })?,
        })
    }

    /// The sexes of the naming guidelines, in stored order.
    #[must_use]
    pub fn naming_sexes(&self) -> Vec<u32> {
        self.naming_guidelines
            .iter()
            .filter_map(|g| g.sex)
            .collect()
    }
}

/// Level-table directory (§7.5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LevelTableDirectory {
    /// `AdvTable_LevelTableList`: the class tables.
    pub class_tables: Vec<Did>,
}

impl LevelTableDirectory {
    /// Reads the directory's property half.
    ///
    /// # Errors
    ///
    /// As [`RaceDirectory::from_properties`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "level-table directory");
        Ok(Self {
            class_tables: f.did_list("AdvTable_LevelTableList")?,
        })
    }
}

/// One `AdvTable_StartingInventory_Entry` (§7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StartingItem {
    /// `AdvTable_StartingInventory_Item`.
    pub item: Option<Did>,
    /// `AdvTable_StartingInventory_Quantity`.
    pub quantity: Option<i32>,
    /// `AdvTable_StartingInventory_StartsEquipped`.
    pub starts_equipped: Option<bool>,
    /// `AdvTable_StartingInventory_RequiredRace`: 0 or absent for any race.
    pub required_race: Option<u32>,
    /// `AdvTable_StartingInventory_ContainerSlot` (advanced-start kits only).
    pub container_slot: Option<u64>,
}

impl StartingItem {
    /// Whether a new character of `species` receives this entry (the spec's reading of the
    /// names, §7.1: the required race is 0 or the species).
    #[must_use]
    pub fn applies_to(&self, species: u32) -> bool {
        matches!(self.required_race, None | Some(0)) || self.required_race == Some(species)
    }
}

fn starting_items(f: &Fields<'_>) -> Result<Vec<StartingItem>> {
    f.struct_list("AdvTable_StartingInventory_List", |e| {
        Ok(StartingItem {
            item: e.did("AdvTable_StartingInventory_Item")?,
            quantity: e.int("AdvTable_StartingInventory_Quantity")?,
            starts_equipped: e.boolean("AdvTable_StartingInventory_StartsEquipped")?,
            required_race: e.enum_("AdvTable_StartingInventory_RequiredRace")?,
            container_slot: e.bits("AdvTable_StartingInventory_ContainerSlot")?,
        })
    })
}

/// One `AdvTable_AdvancedCharacterStart_AdvancedTierCASI_List` entry (§7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AdvancedStart {
    /// `AdvTable_AdvancedCharacterStart_Tier`.
    pub tier: Option<u32>,
    /// `AdvTable_AdvancedCharacterStart_CharacterStartInfo`: the kit.
    pub kit: Option<Did>,
}

/// The character-creation fields of a class table (§7.1, §7.2, §7.4, §10.6).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClassCreationInfo {
    /// `AdvTable_Class`: the class key.
    pub class: Option<u32>,
    /// `AdvTable_StartingInventory_List`.
    pub starting_inventory: Vec<StartingItem>,
    /// `AdvTable_AdvancedCharacterStart_AdvancedTierCASI_List`.
    pub advanced_starts: Vec<AdvancedStart>,
    /// `AdvTable_PipType`: the class's own pip.
    pub own_pip: Option<u32>,
    /// `PipConfig_Pip_List`.
    pub pips: Vec<u32>,
    /// `XP_EarnedPointsToLevel_Table`: the experience table (§9.2).
    pub experience_table: Option<Did>,
    /// `AdvTable_InnateDefaultSkill`.
    pub innate_skill: Option<Did>,
    /// `AdvTable_ClassOutfitAllowedSlots`.
    pub outfit_slots: Option<u64>,
}

impl ClassCreationInfo {
    /// Reads the creation fields from a class table's property half.
    ///
    /// # Errors
    ///
    /// [`WStateError::BadProperty`] for a property of an unexpected type;
    /// [`WStateError::TooMany`].
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "class table");
        Ok(Self {
            class: f.enum_("AdvTable_Class")?,
            starting_inventory: starting_items(&f)?,
            advanced_starts: f.struct_list(
                "AdvTable_AdvancedCharacterStart_AdvancedTierCASI_List",
                |e| {
                    Ok(AdvancedStart {
                        tier: e.enum_("AdvTable_AdvancedCharacterStart_Tier")?,
                        kit: e.did("AdvTable_AdvancedCharacterStart_CharacterStartInfo")?,
                    })
                },
            )?,
            own_pip: f.enum_("AdvTable_PipType")?,
            pips: f.enum_list("PipConfig_Pip_List")?,
            experience_table: f.did("XP_EarnedPointsToLevel_Table")?,
            innate_skill: f.did("AdvTable_InnateDefaultSkill")?,
            outfit_slots: f.bits("AdvTable_ClassOutfitAllowedSlots")?,
        })
    }

    /// The starting items a new character of `species` receives (§7.1, reading of the
    /// names), stored order.
    pub fn starting_items_for(&self, species: u32) -> impl Iterator<Item = &StartingItem> {
        self.starting_inventory
            .iter()
            .filter(move |e| e.applies_to(species))
    }

    /// Reads the inventory of an advanced-start kit (§7.2).
    ///
    /// # Errors
    ///
    /// As [`ClassCreationInfo::from_properties`].
    pub fn kit_inventory(set: &PropertySet, reg: &PropertyRegistry) -> Result<Vec<StartingItem>> {
        starting_items(&Fields::new(set, reg, "advanced-start kit"))
    }
}

/// Class priority control (§7.3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClassPriority {
    /// (`CharGen_ClassPriority_ClassType`, `CharGen_ClassPriority_PriorityValue`) in
    /// stored order.
    pub entries: Vec<(u32, i32)>,
}

impl ClassPriority {
    /// Reads the control's property half.
    ///
    /// # Errors
    ///
    /// [`WStateError::BadProperty`] when an entry lacks either field.
    pub fn from_properties(set: &PropertySet, reg: &PropertyRegistry) -> Result<Self> {
        let f = Fields::new(set, reg, "class priority control");
        Ok(Self {
            entries: f.struct_list("CharGen_ClassPriority_Array", |e| {
                Ok((
                    e.require(
                        "CharGen_ClassPriority_ClassType",
                        e.enum_("CharGen_ClassPriority_ClassType")?,
                    )?,
                    e.require(
                        "CharGen_ClassPriority_PriorityValue",
                        e.int("CharGen_ClassPriority_PriorityValue")?,
                    )?,
                ))
            })?,
        })
    }

    /// Class keys in ascending priority value (Decision D3); equal values keep stored
    /// order.
    #[must_use]
    pub fn display_order(&self) -> Vec<u32> {
        let mut v = self.entries.clone();
        v.sort_by_key(|&(_, p)| p);
        v.into_iter().map(|(c, _)| c).collect()
    }
}

/// A new character's default property set from its race avatar (Decision D1, §4.6 steps
/// 2–3): the avatar's set without mirror properties ([`super::mirror_base`]), with `Sex`
/// set to `sex`. Class, nationality, appearance and inventory (steps 4–6) are added by the
/// caller.
///
/// # Errors
///
/// [`WStateError::BadProperty`] when the dictionary has no `Sex` property.
pub fn default_character_properties(
    avatar: &PropertySet,
    reg: &PropertyRegistry,
    sex: u32,
) -> Result<PropertySet> {
    let sex_pid = reg.pid("Sex").ok_or(WStateError::BadProperty {
        record: "avatar template",
        property: "Sex",
        problem: "is not in the property dictionary",
    })?;
    let mut out: PropertySet = avatar
        .iter()
        .filter(|p| !reg.name(p.pid).is_some_and(|n| mirror_base(n).is_some()))
        .cloned()
        .collect();
    out.insert(sex_pid, Value::Enum(sex));
    Ok(out)
}
