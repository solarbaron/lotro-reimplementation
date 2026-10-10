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
/// (class items, baubles, effect items, items outside the three weapon slots) is not drawn
/// (§5.3 Decision). A ranged-slot instrument has no row: while stowed the assembler
/// evaluates the sheathed-instrument script instead (§5.6 "Stowed instruments"), and while
/// wielded it is not drawn.
///
/// `Item_EquipmentCategory` is a 64-bit bitfield: an item stores a **bit mask** in which
/// bit *k* − 1 stands for enum key *k* (`property-system.md` §7.3; `appearance.md` §15).
///
/// Not modelled (§5.6 "What is normative" prefers evaluating the scripts): the cosmetic
/// in-hand rows and the fishing-pole case, which apply only while a cosmetic override is
/// set (Tessera sets none, §5.6), the stowed battle-gauntlet effect entity (stored in the
/// script's node, found through a case the spec does not name; OPEN in `appearance.md`
/// §15), and the exceptions for stealth and cosmetic locomotion styles that a full script
/// evaluator reproduces. The mounted exception of the stowed shield is applied by the
/// assembler ([`super::CharacterSpec::mounted`]).
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

/// The key of the sheathed-instrument script of the avatars' binding tables
/// (`appearance.md` §5.6 "Stowed instruments", A38; this install's script key, given by the
/// spec without its label, see the OPEN of `appearance.md` §15 of 2026-10-09).
pub const SHEATHED_INSTRUMENT_SCRIPT: u32 = 0x1000_2BA0;

/// The properties of one creature cosmetic slot (`appearance.md` §5.5, A39):
/// `Inventory_Cosmetic_Slot<n>_…`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CosmeticSlotPids {
    /// `…_Entity`: the prop (0 means the slot is empty).
    pub entity: Option<u32>,
    /// `…_HoldingLocation`.
    pub holding_location: Option<u32>,
    /// `…_Scale` (vector3, default (1, 1, 1)).
    pub scale: Option<u32>,
    /// `…_OffsetRotationYaw`.
    pub yaw: Option<u32>,
    /// `…_OffsetRotationPitch`.
    pub pitch: Option<u32>,
    /// `…_OffsetRotationRoll`.
    pub roll: Option<u32>,
    /// `…_Hide`.
    pub hide: Option<u32>,
}

/// What the creature cosmetic slots of `appearance.md` §5.5 need: the four slots, the
/// figure's sheath state and the conditions of script 0x100007C3, as PIDs, enum keys and
/// holding-location keys looked up by label.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CosmeticSlotVocabulary {
    /// Slots 1–4.
    pub slots: Vec<CosmeticSlotPids>,
    /// `Inventory_Cosmetic_IsSheathed`.
    pub is_sheathed: Option<u32>,
    /// Its `Sheathed` key.
    pub sheathed: Option<u32>,
    /// Its `Combat_Defined` key.
    pub combat_defined: Option<u32>,
    /// `Combat_Violent`.
    pub combat_violent: Option<u32>,
    /// `LifeState`.
    pub life_state: Option<u32>,
    /// Its `Dead` and `Incapacitated` keys.
    pub life_state_down: Vec<u32>,
    /// `Agent_ClassificationFilter`.
    pub classification: Option<u32>,
    /// Its `MonsterPlayer` key.
    pub monster_player: Option<u32>,
    /// `HoldingLocation` keys: `Sheath_Back_Shield`, `Wpn_LHand`, `Wpn_LOffHand`,
    /// `Wpn_RHand`, `Wpn_Forearm`.
    pub sheath_back_shield: Option<u32>,
    /// See [`CosmeticSlotVocabulary::sheath_back_shield`].
    pub left_hand: Option<u32>,
    /// See [`CosmeticSlotVocabulary::sheath_back_shield`].
    pub left_off_hand: Option<u32>,
    /// See [`CosmeticSlotVocabulary::sheath_back_shield`].
    pub right_hand: Option<u32>,
    /// See [`CosmeticSlotVocabulary::sheath_back_shield`].
    pub forearm: Option<u32>,
}

/// One placement of a creature cosmetic slot (`appearance.md` §5.5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CosmeticPlacement {
    /// The prop entity.
    pub entity: Did,
    /// The `HoldingLocation` key it is attached at.
    pub location: u32,
    /// `BoneOffset`, metres.
    pub offset: [f32; 3],
    /// Yaw, pitch, roll, degrees.
    pub rotation: [f32; 3],
    /// Per-axis scale.
    pub scale: [f32; 3],
}

impl CosmeticSlotVocabulary {
    /// Whether the slot props are drawn on a figure whose values `prop` reads (§5.5:
    /// nothing while `LifeState` is `Dead` or `Incapacitated`; a figure whose
    /// `Agent_ClassificationFilter` is `MonsterPlayer` takes another branch, not covered).
    pub fn applies(&self, prop: &dyn Fn(Option<u32>) -> Option<u32>) -> bool {
        let down = prop(self.life_state).is_some_and(|v| self.life_state_down.contains(&v));
        let monster =
            self.monster_player.is_some() && prop(self.classification) == self.monster_player;
        !down && !monster
    }

    /// Whether the figure's props take the sheathed placements (§5.5: `IsSheathed`
    /// `Sheathed`, or `Combat_Defined` while `Combat_Violent` is false).
    pub fn sheathed(&self, sheath_state: Option<u32>, violent: bool) -> bool {
        match sheath_state {
            Some(v) if Some(v) == self.sheathed => true,
            Some(v) if Some(v) == self.combat_defined => !violent,
            _ => false,
        }
    }

    /// The placement of a slot with prop `entity` held unsheathed at `location`, with its
    /// scale and rotations (§5.5): sheathed, the fixed placement at `Sheath_Back_Shield`
    /// chosen by the location (only the scale follows the slot; location 0 draws nothing,
    /// any other location takes the unsheathed placement); unsheathed, the slot's location,
    /// rotations and scale with offset 0.
    #[must_use]
    pub fn placement(
        &self,
        entity: Did,
        location: u32,
        scale: [f32; 3],
        rotation: [f32; 3],
        sheathed: bool,
    ) -> Option<CosmeticPlacement> {
        if location == 0 {
            return None;
        }
        let unsheathed = CosmeticPlacement {
            entity,
            location,
            offset: [0.0; 3],
            rotation,
            scale,
        };
        if !sheathed {
            return Some(unsheathed);
        }
        let back = self.sheath_back_shield;
        let fixed = |offset: [f32; 3], rotation: [f32; 3]| {
            back.map(|location| CosmeticPlacement {
                entity,
                location,
                offset,
                rotation,
                scale,
            })
        };
        let at = |k: Option<u32>| k == Some(location);
        if at(self.left_hand) {
            fixed([0.0; 3], [-30.0, 0.0, 90.0])
        } else if at(self.left_off_hand) {
            fixed([-0.15, 0.2, 0.0], [210.0, 0.0, 0.0])
        } else if at(self.right_hand) {
            fixed([0.15, 0.2, 0.0], [-210.0, 0.0, 0.0])
        } else if at(self.forearm) {
            fixed([0.0; 3], [0.0; 3])
        } else {
            Some(unsheathed)
        }
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
    /// `GOP_AvatarFaceCompositing`: the face-compositing graphics option the species tables
    /// test before attaching a face compositor (§3.5).
    pub avatar_face_compositing: Option<u32>,
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
    /// `Item_EquipmentCategory` bit of `Instrument` (§5.6 "Stowed instruments").
    pub instrument_category: u64,
    /// The sheathed-instrument script key ([`SHEATHED_INSTRUMENT_SCRIPT`]).
    pub sheathed_instrument_script: Option<u32>,
    /// `Item_Music_Instrument_Type` (the item's instrument type).
    pub item_instrument_type: Option<u32>,
    /// `Music_Cached_Instrument_Type`: set to the ranged item's instrument type (§5.6
    /// Decision of 2026-10-09).
    pub cached_instrument_type: Option<u32>,
    /// `Script_Item_PhysObj_Ranged`: the ranged item's entity (§5.6).
    pub physobj_ranged: Option<u32>,
    /// Creature cosmetic slots (§5.5).
    pub cosmetic: CosmeticSlotVocabulary,
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
            avatar_face_compositing: names.pid_of("GOP_AvatarFaceCompositing"),
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
            instrument_category: category("Instrument").unwrap_or(0),
            sheathed_instrument_script: Some(SHEATHED_INSTRUMENT_SCRIPT),
            item_instrument_type: names.pid_of("Item_Music_Instrument_Type"),
            cached_instrument_type: names.pid_of("Music_Cached_Instrument_Type"),
            physobj_ranged: names.pid_of("Script_Item_PhysObj_Ranged"),
            cosmetic: CosmeticSlotVocabulary {
                slots: (1..=4)
                    .map(|n| {
                        let p = |field: &str| {
                            names.pid_of(&format!("Inventory_Cosmetic_Slot{n}_{field}"))
                        };
                        CosmeticSlotPids {
                            entity: p("Entity"),
                            holding_location: p("HoldingLocation"),
                            scale: p("Scale"),
                            yaw: p("OffsetRotationYaw"),
                            pitch: p("OffsetRotationPitch"),
                            roll: p("OffsetRotationRoll"),
                            hide: p("Hide"),
                        }
                    })
                    .collect(),
                is_sheathed: names.pid_of("Inventory_Cosmetic_IsSheathed"),
                sheathed: enum_key("Inventory_Cosmetic_IsSheathed", "Sheathed"),
                combat_defined: enum_key("Inventory_Cosmetic_IsSheathed", "Combat_Defined"),
                combat_violent: names.pid_of("Combat_Violent"),
                life_state: names.pid_of("LifeState"),
                life_state_down: ["Dead", "Incapacitated"]
                    .iter()
                    .filter_map(|l| enum_key("LifeState", l))
                    .collect(),
                classification: names.pid_of("Agent_ClassificationFilter"),
                monster_player: enum_key("Agent_ClassificationFilter", "MonsterPlayer"),
                sheath_back_shield: location("Sheath_Back_Shield"),
                left_hand: location("Wpn_LHand"),
                left_off_hand: location("Wpn_LOffHand"),
                right_hand: location("Wpn_RHand"),
                forearm: location("Wpn_Forearm"),
            },
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
