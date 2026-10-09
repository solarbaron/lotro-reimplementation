// SPDX-License-Identifier: GPL-3.0-or-later
//! The property IDs and enum keys assembly reads, looked up in the data at run time.

use tessera_formats::appearance::AppearanceListPids;
use tessera_formats::item::{HoldingRule, WardrobeFallbacks};
use tessera_formats::{Did, EnumMapper, PropertyNames};
use tessera_props::{EnumSource, GameData, PropsError, RecordSource};

/// The three weapon slots whose placement scripts attach an equipped item's entity
/// (`appearance.md` §5.6: `Script_Item_PhysObj_Primary`, `…_Secondary`, `…_Ranged`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaponSlot {
    /// `Weapon_Primary`.
    Primary,
    /// `Weapon_Secondary`.
    Secondary,
    /// `Weapon_Ranged`.
    Ranged,
}

/// Which wearers a placement row applies to (`appearance.md` §5.6 "Species").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeciesMatch {
    /// Every species ("all", and "others" in a row that follows the specific ones).
    Any,
    /// The listed `SpeciesType` keys.
    OneOf(Vec<u32>),
}

impl SpeciesMatch {
    fn matches(&self, species: u32) -> bool {
        match self {
            Self::Any => true,
            Self::OneOf(keys) => keys.contains(&species),
        }
    }
}

/// The skill-mode condition of a row (`appearance.md` §5.6: shields stow on the back only
/// while the figure's `Combat_SkillMode` is `Ranged`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillModeMatch {
    /// Any skill mode.
    Any,
    /// Only while the skill mode is `Ranged`.
    Ranged,
    /// Only while it is not.
    NotRanged,
}

impl SkillModeMatch {
    fn matches(self, ranged: bool) -> bool {
        match self {
            Self::Any => true,
            Self::Ranged => ranged,
            Self::NotRanged => !ranged,
        }
    }
}

/// The scale of a placement (the MeshFX node's `Scale`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlacementScale {
    /// A fixed uniform scale.
    Fixed(f32),
    /// Driven by the wearer's `Script_AdjustableScale_Secondary_Vector` (stowed shields).
    SecondaryVector,
}

/// One MeshFX attachment of a placement script (`appearance.md` §5.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// The `HoldingLocation` key.
    pub location: u32,
    /// Offset in metres in the location's frame (`BoneOffset`).
    pub offset: [f32; 3],
    /// `OffsetRotationYaw`, `…Pitch`, `…Roll` in degrees.
    pub rotation: [f32; 3],
    /// The node's scale.
    pub scale: PlacementScale,
}

/// One row of the stowed or wielded placement tables of `appearance.md` §5.6.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacementRow {
    /// The weapon slot the item is equipped in.
    pub slot: WeaponSlot,
    /// The stowed (out of combat) table, else the wielded one.
    pub stowed: bool,
    /// `Item_EquipmentCategory` bits of the row's categories (key *k* is bit *k* − 1); an
    /// item matches when its bits intersect them.
    pub categories: u64,
    /// Wearers the row applies to.
    pub species: SpeciesMatch,
    /// Skill-mode condition.
    pub skill_mode: SkillModeMatch,
    /// The attachments; empty for a row that draws nothing (an orb while stowed).
    pub placements: Vec<Placement>,
}

/// The stowed and wielded placements of weapons, shields and ranged items
/// (`appearance.md` §5.6, A24): the MeshFX nodes of the six placement scripts of the shared
/// avatar table, as data. Out of combat an equipped item takes its **stowed** placement, in
/// combat its **wielded** one (§5.4 Rule). The first row (in table order) whose slot,
/// family, categories, species and skill mode match decides; an item no row matches
/// (class items, instruments, baubles, effect items, items outside the three weapon slots)
/// is not drawn (§5.3 Decision).
///
/// `Item_EquipmentCategory` is a 64-bit bitfield: an item stores a **bit mask** in which
/// bit *k* − 1 stands for enum key *k* (`property-system.md` §7.3; `appearance.md` §15).
///
/// Not modelled: the cosmetic in-hand row (it lists entity DIDs in the script, not in the
/// spec), the battle-gauntlet effect entity (named by the script), and the exceptions for
/// mounts, stealth and cosmetic locomotion styles that a full script evaluator reproduces.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlacementRule {
    /// `ContainerSlot` bits of `Weapon_Primary`, `Weapon_Secondary`, `Weapon_Ranged`.
    pub slot_bits: [u64; 3],
    /// The rows, in matching order.
    pub rows: Vec<PlacementRow>,
}

/// The one-handed category list of §5.6.
const ONE_HANDED: [&str; 8] = [
    "1HSword",
    "1HAxe",
    "1HMace",
    "1HClub",
    "1HHammer",
    "Dagger",
    "Implement1H",
    "Cosmetic",
];
/// The two-handed list of §5.6 (`2HSword` and `2HStaff` have rows of their own).
const TWO_HANDED: [&str; 7] = [
    "2HMace",
    "2HAxe",
    "2HClub",
    "2HHammer",
    "Spear",
    "Halberd",
    "Implement2H",
];
const SHIELDS: [&str; 3] = ["Shield", "HeavyShield", "MediumShield"];
const DWARVES: [&str; 2] = ["Dwarf", "DwarfStoutAxe"];

impl PlacementRule {
    /// Builds the tables of `appearance.md` §5.6 from labels: `slot_bits` gives a
    /// `ContainerSlot` label's bits, `category_bits` an `Item_EquipmentCategory` label's
    /// **bit** (key *k* → bit *k* − 1), `species` a `SpeciesType` label's key and `location`
    /// a `HoldingLocation` label's key. Missing labels leave their categories, species or
    /// attachments out (a row whose location is missing then draws nothing).
    #[must_use]
    pub fn from_labels(
        slot_bits: impl Fn(&str) -> Option<u64>,
        category_bits: impl Fn(&str) -> Option<u64>,
        species: impl Fn(&str) -> Option<u32>,
        location: impl Fn(&str) -> Option<u32>,
    ) -> Self {
        use PlacementScale::{Fixed, SecondaryVector};
        use SkillModeMatch as Sk;
        use WeaponSlot::{Primary, Ranged, Secondary};
        let cats = |labels: &[&str]| {
            labels
                .iter()
                .filter_map(|l| category_bits(l))
                .fold(0u64, |a, b| a | b)
        };
        let one = |labels: &[&str]| {
            SpeciesMatch::OneOf(labels.iter().filter_map(|l| species(l)).collect())
        };
        let at = |label: &str, offset: [f32; 3], rotation: [f32; 3], scale: PlacementScale| {
            location(label).map(|location| Placement {
                location,
                offset,
                rotation,
                scale,
            })
        };
        let plain = |label: &str| at(label, [0.0; 3], [0.0; 3], Fixed(1.0));
        let row =
            |slot, stowed, categories, species, skill_mode, placements: Vec<Option<Placement>>| {
                PlacementRow {
                    slot,
                    stowed,
                    categories,
                    species,
                    skill_mode,
                    placements: placements.into_iter().flatten().collect(),
                }
            };
        let one_handed = cats(&ONE_HANDED);
        let two_handed = cats(&TWO_HANDED);
        let sword2 = cats(&["2HSword"]);
        let staff = cats(&["2HStaff"]);
        let orb = cats(&["Orb"]);
        let shields = cats(&SHIELDS);
        let bow = cats(&["Bow"]);
        let crossbow = cats(&["Crossbow"]);
        let javelin = cats(&["Javelin"]);
        let any = SpeciesMatch::Any;
        let half_turn = [0.0, 180.0, 0.0];
        let quarter_roll = [0.0, 0.0, 90.0];
        let rows = vec![
            // Stowed (`appearance.md` §5.6 "Stowed placements").
            row(
                Primary,
                true,
                one_handed,
                any.clone(),
                Sk::Any,
                vec![plain("Sheath_LHip")],
            ),
            row(
                Primary,
                true,
                sword2,
                any.clone(),
                Sk::Any,
                vec![plain("Sheath_Back_2H_Sword")],
            ),
            row(
                Primary,
                true,
                two_handed,
                any.clone(),
                Sk::Any,
                vec![plain("Sheath_Back_2H")],
            ),
            row(
                Primary,
                true,
                staff,
                one(&DWARVES),
                Sk::Any,
                vec![at(
                    "Sheath_Back_2H",
                    [0.0, 0.35, 0.0],
                    half_turn,
                    Fixed(1.0),
                )],
            ),
            row(
                Primary,
                true,
                staff,
                one(&["Hobbit"]),
                Sk::Any,
                vec![at("Sheath_Back_2H", [0.0, 0.2, 0.0], half_turn, Fixed(1.0))],
            ),
            row(
                Primary,
                true,
                staff,
                any.clone(),
                Sk::Any,
                vec![at("Sheath_Back_2H", [0.0, 0.4, 0.0], half_turn, Fixed(1.0))],
            ),
            row(Primary, true, orb, any.clone(), Sk::Any, vec![]),
            row(
                Secondary,
                true,
                one_handed,
                any.clone(),
                Sk::Any,
                vec![plain("Sheath_RHip")],
            ),
            row(
                Secondary,
                true,
                shields,
                any.clone(),
                Sk::NotRanged,
                vec![at("Wpn_Forearm", [0.0; 3], [0.0; 3], SecondaryVector)],
            ),
            row(
                Secondary,
                true,
                shields,
                any.clone(),
                Sk::Ranged,
                vec![at(
                    "Sheath_Back_Shield",
                    [0.0; 3],
                    [0.0; 3],
                    SecondaryVector,
                )],
            ),
            row(
                Ranged,
                true,
                bow,
                one(&DWARVES),
                Sk::Any,
                vec![at("Sheath_Back_Bow", [0.0; 3], [0.0; 3], Fixed(0.9))],
            ),
            row(
                Ranged,
                true,
                bow,
                one(&["Hobbit", "RiverHobbit"]),
                Sk::Any,
                vec![at("Sheath_Back_Bow", [0.0; 3], [0.0; 3], Fixed(0.8))],
            ),
            row(
                Ranged,
                true,
                bow,
                any.clone(),
                Sk::Any,
                vec![plain("Sheath_Back_Bow")],
            ),
            row(
                Ranged,
                true,
                crossbow,
                any.clone(),
                Sk::Any,
                vec![plain("Sheath_Back_Crossbow")],
            ),
            row(
                Ranged,
                true,
                javelin,
                one(&DWARVES),
                Sk::Any,
                vec![at("Sheath_Back_Bow", [0.0; 3], quarter_roll, Fixed(1.0))],
            ),
            row(
                Ranged,
                true,
                javelin,
                one(&["Hobbit"]),
                Sk::Any,
                vec![at(
                    "Sheath_Back_Bow",
                    [0.03, 0.0, 0.0],
                    quarter_roll,
                    Fixed(1.0),
                )],
            ),
            row(
                Ranged,
                true,
                javelin,
                any.clone(),
                Sk::Any,
                vec![plain("Sheath_Back_Bow")],
            ),
            // Wielded (`appearance.md` §5.6 "Wielded placements").
            row(
                Primary,
                false,
                one_handed | sword2 | two_handed | staff,
                any.clone(),
                Sk::Any,
                vec![plain("Wpn_RHand")],
            ),
            row(
                Primary,
                false,
                orb,
                any.clone(),
                Sk::Any,
                vec![plain("Wpn_RHand"), plain("Wpn_LHand")],
            ),
            row(
                Secondary,
                false,
                one_handed,
                any.clone(),
                Sk::Any,
                vec![plain("Wpn_LOffHand")],
            ),
            row(
                Secondary,
                false,
                shields,
                any.clone(),
                Sk::Any,
                vec![plain("Wpn_Forearm")],
            ),
            row(
                Ranged,
                false,
                bow,
                one(&DWARVES),
                Sk::Any,
                vec![at("Wpn_LHand", [0.0; 3], [0.0; 3], Fixed(0.9))],
            ),
            row(
                Ranged,
                false,
                bow,
                one(&["Hobbit", "RiverHobbit"]),
                Sk::Any,
                vec![at("Wpn_LHand", [0.0; 3], [0.0; 3], Fixed(0.8))],
            ),
            row(
                Ranged,
                false,
                bow,
                any.clone(),
                Sk::Any,
                vec![plain("Wpn_LHand")],
            ),
            row(
                Ranged,
                false,
                crossbow,
                any.clone(),
                Sk::Any,
                vec![plain("Wpn_RHand")],
            ),
            row(
                Ranged,
                false,
                javelin,
                any,
                Sk::Any,
                vec![plain("Wpn_ROffhand")],
            ),
        ];
        Self {
            slot_bits: [
                slot_bits("Weapon_Primary").unwrap_or(0),
                slot_bits("Weapon_Secondary").unwrap_or(0),
                slot_bits("Weapon_Ranged").unwrap_or(0),
            ],
            rows: rows.into_iter().filter(|r| r.categories != 0).collect(),
        }
    }

    /// The weapon slot of the `ContainerSlot` bits `slot` (secondary, then ranged, then
    /// primary when several intersect), or `None` outside the three weapon slots.
    #[must_use]
    pub fn weapon_slot(&self, slot: u64) -> Option<WeaponSlot> {
        let [p, s, r] = self.slot_bits;
        if slot & s != 0 {
            Some(WeaponSlot::Secondary)
        } else if slot & r != 0 {
            Some(WeaponSlot::Ranged)
        } else if slot & p != 0 {
            Some(WeaponSlot::Primary)
        } else {
            None
        }
    }

    /// The placement row for an item with `Item_EquipmentCategory` bits `category` equipped
    /// in `slot` (`ContainerSlot` bits) on a wearer of `species`, stowed or wielded, with
    /// the ranged skill mode or not. `None` when no row matches: the item is not drawn.
    #[must_use]
    pub fn row(
        &self,
        slot: u64,
        category: u64,
        species: u32,
        ranged: bool,
        stowed: bool,
    ) -> Option<&PlacementRow> {
        let ws = self.weapon_slot(slot)?;
        self.rows.iter().find(|r| {
            r.slot == ws
                && r.stowed == stowed
                && r.categories & category != 0
                && r.species.matches(species)
                && r.skill_mode.matches(ranged)
        })
    }
}

/// Property IDs and enum keys assembly needs, looked up by name in the data. Every field
/// is public so that tests and fuzzers can build one by hand; [`AssemblyVocabulary::default`]
/// knows nothing (no dyes, no holding locations, no NPC properties).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AssemblyVocabulary {
    /// The appearance-list properties, to read instance lists from object and instance
    /// sets (`appearance.md` §3.2, §7.4).
    pub lists: Option<AppearanceListPids>,
    /// `AppearanceKey` `ClothingColor` (the first dye channel, §4.9).
    pub clothing_color: Option<u32>,
    /// `AppearanceKey` `ClothingColorSecondary`.
    pub clothing_color_secondary: Option<u32>,
    /// `MeshTypeID` `UpperBody` (the humanoid-rig rule of §7.3).
    pub upper_body: Option<u32>,
    /// Wardrobe-body fallbacks (§2.3 Decision).
    pub fallbacks: WardrobeFallbacks,
    /// Paperdoll locations per equipment slot (§5.3; `skeleton-anim.md` §8.5), for
    /// paperdoll views only; assembly places held items by `placement`.
    pub holding: HoldingRule,
    /// Stowed and wielded placements in the world (§5.4 Rule, §5.6).
    pub placement: PlacementRule,
    /// `GOP_AvatarUpdateVisible`: the toggle property the option store answers
    /// (§3.3 Tessera rule).
    pub avatar_update_visible: Option<u32>,
    /// `NPC_Cosmetic_Inventory_Visible`: false hides the figure's props (§5.5 Rule).
    pub npc_props_visible: Option<u32>,
    /// `Script_AdjustableScale_Secondary_Vector`: the scale of a stowed shield (§5.6).
    pub secondary_scale: Option<u32>,
    /// (`NPC_Cosmetic_Inventory_Item<n>`, `NPC_Cosmetic_Inventory_HoldingLocation<n>`) for
    /// n = 1–4 (§5.5); index 0 is prop 1.
    pub npc_items: Vec<(u32, Option<u32>)>,
    /// `NPC_StartingIdleAnimation`.
    pub idle_animation: Option<u32>,
    /// `Appearance_RandomAppearanceType`.
    pub random_appearance_type: Option<u32>,
    /// Random-appearance values that mean "none" (`Undef`; 0 is always treated so).
    pub random_appearance_none: Vec<u32>,
    /// `Render_AdjustableScale` (instance scale, §6.1).
    pub render_scale: Option<u32>,
    /// `AC_BoneScale_Test` (§6.2).
    pub bone_scale: Option<u32>,
    /// `AC_Avatar_Height` (§6.2).
    pub avatar_height: Option<u32>,
    /// `AC_BoneSlider_Value_Array` (§6.3): the property whose value the figure's slider
    /// bytes replace when it drives a SliderMeshDeformer node's string.
    pub slider_values: Option<u32>,
}

impl AssemblyVocabulary {
    /// The paperdoll pairs of `appearance.md` §5.3 / `skeleton-anim.md` §8.5: (`ContainerSlot`
    /// label, `HoldingLocation` label), as the spec lists them. (The inventory-control
    /// object holds them in `InventoryControl_SlotToPaperdollLocation_MappingList`, §14;
    /// how a reader finds that object is not specified, so the labels are used.)
    pub const SLOT_LOCATIONS: [(&'static str, &'static str); 3] = [
        ("Weapon_Primary", "Wpn_RHand"),
        ("Weapon_Secondary", "Wpn_LOffHand"),
        ("Weapon_Ranged", "Sheath_Back_Bow"),
    ];

    /// Builds the vocabulary from name lookups: `names` gives PIDs, `enum_key(property,
    /// label)` the key of `label` in the enum mapper `property` is labelled by (with its
    /// base chain), and `mesh_type(label)` a `MeshTypeID` key. Bitfield labels become bits
    /// by the rule of `property-system.md` §7.3 (key *k* is bit *k* − 1).
    #[must_use]
    pub fn from_lookup<N: PropertyNames + ?Sized>(
        names: &N,
        enum_key: impl Fn(&str, &str) -> Option<u32>,
        mesh_type: impl Fn(&str) -> Option<u32>,
    ) -> Self {
        // property-system.md §7.3: bitfield key k is bit k − 1.
        let bit = |property: &str, label: &str| {
            enum_key(property, label)
                .and_then(|k| k.checked_sub(1))
                .filter(|b| *b < 64)
                .map(|b| 1u64 << b)
        };
        let slot_bits = |label: &str| bit("Inventory_DefaultSlot", label);
        let location = |label: &str| enum_key("Inventory_ParentingLocation", label);
        let category = |label: &str| bit("Item_EquipmentCategory", label);
        let species = |label: &str| enum_key("RaceTable_Race", label);
        let holding = HoldingRule {
            slot_locations: Self::SLOT_LOCATIONS
                .iter()
                .filter_map(|(s, l)| Some((slot_bits(s)?, location(l)?)))
                .collect(),
        };
        let mut random_appearance_none = vec![0];
        random_appearance_none
            .extend(enum_key("Appearance_RandomAppearanceType", "Undef").filter(|k| *k != 0));
        Self {
            lists: AppearanceListPids::from_names(names).ok(),
            clothing_color: enum_key("Appearance_Key", "ClothingColor"),
            clothing_color_secondary: enum_key("Appearance_Key", "ClothingColorSecondary"),
            upper_body: mesh_type("UpperBody"),
            fallbacks: WardrobeFallbacks::from_species_labels(|l| enum_key("RaceTable_Race", l)),
            placement: PlacementRule::from_labels(slot_bits, category, species, location),
            holding,
            avatar_update_visible: names.pid_of("GOP_AvatarUpdateVisible"),
            npc_props_visible: names.pid_of("NPC_Cosmetic_Inventory_Visible"),
            secondary_scale: names.pid_of("Script_AdjustableScale_Secondary_Vector"),
            npc_items: (1..=4)
                .filter_map(|i| {
                    let item = names.pid_of(&format!("NPC_Cosmetic_Inventory_Item{i}"))?;
                    Some((
                        item,
                        names.pid_of(&format!("NPC_Cosmetic_Inventory_HoldingLocation{i}")),
                    ))
                })
                .collect(),
            idle_animation: names.pid_of("NPC_StartingIdleAnimation"),
            random_appearance_type: names.pid_of("Appearance_RandomAppearanceType"),
            random_appearance_none,
            render_scale: names.pid_of("Render_AdjustableScale"),
            bone_scale: names.pid_of("AC_BoneScale_Test"),
            avatar_height: names.pid_of("AC_Avatar_Height"),
            slider_values: names.pid_of("AC_BoneSlider_Value_Array"),
        }
    }

    /// [`AssemblyVocabulary::from_lookup`] against an install: PIDs from the property
    /// registry, enum keys from the mapper each property's definition names, `MeshTypeID`
    /// from the `EMAPPER` DID map.
    ///
    /// # Errors
    ///
    /// When the property registry or the root DID map cannot be loaded.
    pub fn from_game_data<S: RecordSource>(data: &GameData<S>) -> Result<Self, PropsError> {
        let registry = data.registry()?;
        let fetch = |d: Did| data.enum_mapper(d).ok().flatten();
        let enum_key = |property: &str, label: &str| {
            let pid = registry.pid(property)?;
            match registry.enum_source(pid)? {
                EnumSource::Mapper(m) => EnumMapper::key_in_chain_from(m, fetch, label),
                EnumSource::DidMap(_) => None,
            }
        };
        let mesh_types = data.lookup_path(&["EMAPPER", "MeshTypeID"])?;
        let mesh_type = |label: &str| EnumMapper::key_in_chain_from(mesh_types?, fetch, label);
        Ok(Self::from_lookup(
            registry.dictionary(),
            enum_key,
            mesh_type,
        ))
    }

    /// Whether `value` of `Appearance_RandomAppearanceType` names a random appearance.
    #[must_use]
    pub fn is_random_appearance(&self, value: u32) -> bool {
        !self.random_appearance_none.contains(&value) && value != 0
    }
}
