// SPDX-License-Identifier: GPL-3.0-or-later
//! The property IDs and enum keys assembly reads, looked up in the data at run time.

use tessera_formats::appearance::AppearanceListPids;
use tessera_formats::item::{HoldingRule, ItemVisual, WardrobeFallbacks};
use tessera_formats::{Did, EnumMapper, PropertyNames};
use tessera_props::{EnumSource, GameData, PropsError, RecordSource};

/// Stowing locations by equipment category (`appearance.md` §5.4 Decision): one-handed
/// weapons in the primary slot → `Sheath_LHip`, in the secondary slot → `Sheath_RHip`;
/// `2HSword` → `Sheath_Back_2H_Sword`; other two-handed weapons, staves, spears and
/// halberds → `Sheath_Back_2H`; `Bow` → `Sheath_Back_Bow`; `Crossbow` →
/// `Sheath_Back_Crossbow`; shields → `Sheath_Back_Shield`; everything else (orbs, javelins,
/// categories the Decision does not name) stays in hand.
///
/// `Item_EquipmentCategory` is a 64-bit bitfield: an item stores a **bit mask**, in which
/// bit *k* − 1 stands for enum key *k* (`property-system.md` §7.3). The rule therefore holds
/// category masks, not enum keys, and matches an item when the masks intersect.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StowRule {
    /// `ContainerSlot` bits of `Weapon_Primary`.
    pub primary_slot: u64,
    /// `ContainerSlot` bits of `Weapon_Secondary`.
    pub secondary_slot: u64,
    /// `Item_EquipmentCategory` bits of the one-handed weapons, together.
    pub one_handed: u64,
    /// Location of a one-handed weapon in the primary slot.
    pub primary_one_handed: Option<u32>,
    /// Location of a one-handed weapon in the secondary slot.
    pub secondary_one_handed: Option<u32>,
    /// (`Item_EquipmentCategory` bits, location) for the other stowed categories; the first
    /// pair whose bits intersect the item's wins.
    pub by_category: Vec<(u64, u32)>,
}

impl StowRule {
    /// One-handed `Item_EquipmentCategory` labels (`appearance.md` §5.2).
    pub const ONE_HANDED: [&'static str; 6] =
        ["1HSword", "1HAxe", "Dagger", "1HMace", "1HHammer", "1HClub"];

    /// (category label, `HoldingLocation` label) pairs of the §5.4 Decision besides the
    /// one-handed weapons.
    pub const BY_CATEGORY: [(&'static str, &'static str); 12] = [
        ("2HSword", "Sheath_Back_2H_Sword"),
        ("2HAxe", "Sheath_Back_2H"),
        ("2HHammer", "Sheath_Back_2H"),
        ("2HClub", "Sheath_Back_2H"),
        ("2HStaff", "Sheath_Back_2H"),
        ("Spear", "Sheath_Back_2H"),
        ("Halberd", "Sheath_Back_2H"),
        ("Bow", "Sheath_Back_Bow"),
        ("Crossbow", "Sheath_Back_Crossbow"),
        ("Shield", "Sheath_Back_Shield"),
        ("MediumShield", "Sheath_Back_Shield"),
        ("HeavyShield", "Sheath_Back_Shield"),
    ];

    /// Builds the rule from labels: `slot_bits` gives a `ContainerSlot` label's bits,
    /// `category_bits` an `Item_EquipmentCategory` label's **bit** (key *k* → bit *k* − 1),
    /// `location` a `HoldingLocation` label's key. Missing labels leave their entries out.
    #[must_use]
    pub fn from_labels(
        slot_bits: impl Fn(&str) -> Option<u64>,
        category_bits: impl Fn(&str) -> Option<u64>,
        location: impl Fn(&str) -> Option<u32>,
    ) -> Self {
        Self {
            primary_slot: slot_bits("Weapon_Primary").unwrap_or(0),
            secondary_slot: slot_bits("Weapon_Secondary").unwrap_or(0),
            one_handed: Self::ONE_HANDED
                .iter()
                .filter_map(|l| category_bits(l))
                .fold(0, |a, b| a | b),
            primary_one_handed: location("Sheath_LHip"),
            secondary_one_handed: location("Sheath_RHip"),
            by_category: Self::BY_CATEGORY
                .iter()
                .filter_map(|(c, l)| Some((category_bits(c)?, location(l)?)))
                .filter(|(bits, _)| *bits != 0)
                .collect(),
        }
    }

    /// The stowing location of `item` in its default slot ([`StowRule::location_in`]).
    #[must_use]
    pub fn location(&self, item: &ItemVisual) -> Option<u32> {
        self.location_in(item, item.default_slot)
    }

    /// The stowing location of `item` equipped in `slot` (`ContainerSlot` bits; `None`
    /// means its default slot), or `None` when it stays in hand (or the rule does not know
    /// its category). A one-handed weapon goes by the slot: secondary, then primary.
    #[must_use]
    pub fn location_in(&self, item: &ItemVisual, slot: Option<u64>) -> Option<u32> {
        let category = item.equipment_category?;
        if category == 0 {
            return None;
        }
        if category & self.one_handed != 0 {
            let slot = slot.or(item.default_slot).unwrap_or(0);
            if slot & self.secondary_slot != 0 {
                return self.secondary_one_handed;
            }
            if slot & self.primary_slot != 0 {
                return self.primary_one_handed;
            }
            return None;
        }
        self.by_category
            .iter()
            .find(|(bits, _)| bits & category != 0)
            .map(|&(_, l)| l)
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
    /// Holding locations per equipment slot (§5.3; `skeleton-anim.md` §8.5 world rule).
    pub holding: HoldingRule,
    /// Stowing locations (§5.4 Decision).
    pub stow: StowRule,
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
}

impl AssemblyVocabulary {
    /// The paperdoll pairs of `appearance.md` §5.3 / `skeleton-anim.md` §8.5: (`ContainerSlot`
    /// label, `HoldingLocation` label). The inventory-control object holds them, but its
    /// struct's field names are not specified (`appearance.md` §14 OPEN), so the pairs are
    /// built from the labels the specs give.
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
            stow: StowRule::from_labels(slot_bits, category, location),
            holding,
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
