// SPDX-License-Identifier: GPL-3.0-or-later
//! Items on a figure: the worn path (wardrobe records) and the held path (an entity chain
//! placed at a holding location), from an item's property set to DIDs (`appearance.md` §4,
//! §5; `setup.md` §3, §13).
//!
//! - [`ItemVisual::from_property_set`] reads the visual properties of an item (0x79 set):
//!   the `Item_WornAppearanceMapList` structs, `PhysObj`, slot, parenting locations,
//!   equipment category and default dye codes.
//! - [`ItemVisual::look`] decides worn versus held for a wearer (`setup.md` §13.3) and picks
//!   the wardrobe struct for the wearer's species and sex, with the species fallback of
//!   `appearance.md` §2.3 ([`WardrobeFallbacks`]).
//! - [`worn_block`] and [`dye_block`] select the blocks of a wardrobe record by the block
//!   rule of §3.2.1 (§4.5, §4.9).
//! - [`HoldingRule`] chooses a held item's location in a **paperdoll** view
//!   (`skeleton-anim.md` §8.5, `appearance.md` §5.3); in the world the stowed and wielded
//!   placements of §5.6 apply instead (`tessera-mesh` assembly). [`npc_held_items`] reads
//!   the props of NPC-like figures (§5.5) and [`npc_cosmetic_inventory_visible`] their
//!   visibility flag (false hides the props, always).

use crate::appearance::{AppearanceBlock, AppearanceRecord};
use crate::did::Did;
use crate::error::{FormatError, Result};
use crate::property::{PropertyNames, PropertyValue};
use crate::property_set::PropertySet;

fn as_u32(v: &PropertyValue) -> Option<u32> {
    v.as_bits().and_then(|b| u32::try_from(b).ok())
}

#[allow(clippy::cast_possible_truncation)]
fn as_f32(v: &PropertyValue) -> Option<f32> {
    v.as_f64().map(|x| x as f32)
}

/// PIDs of the item properties assembly reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemPids {
    /// `Item_WornAppearanceMapList`.
    pub worn_map_list: Option<u32>,
    /// `Item_WornAppearance` (inside the map structs).
    pub worn_appearance: u32,
    /// `Item_AppearanceKey`.
    pub appearance_key: u32,
    /// `Item_SpeciesOfWearer`.
    pub species_of_wearer: u32,
    /// `Item_SexOfWearer`.
    pub sex_of_wearer: u32,
    /// `PhysObj`.
    pub phys_obj: Option<u32>,
    /// `Inventory_DefaultSlot` (a `ContainerSlot` bit field).
    pub default_slot: Option<u32>,
    /// `Inventory_ParentingLocation` (in-world location override).
    pub parenting_location: Option<u32>,
    /// `Inventory_PaperdollParentingLocation` (paperdoll override).
    pub paperdoll_parenting_location: Option<u32>,
    /// `Item_EquipmentCategory`.
    pub equipment_category: Option<u32>,
    /// `Item_ClothingColor`: the default dye code.
    pub clothing_color: Option<u32>,
    /// `Item_ClothingColorSecondary`.
    pub clothing_color_secondary: Option<u32>,
}

impl ItemPids {
    /// Looks the names up.
    ///
    /// # Errors
    ///
    /// [`FormatError::UnknownName`] when one of the four map-struct field names is missing.
    pub fn from_names<N: PropertyNames + ?Sized>(names: &N) -> Result<Self> {
        let need = |n: &'static str| {
            names.pid_of(n).ok_or(FormatError::UnknownName {
                what: "property",
                name: n,
            })
        };
        Ok(Self {
            worn_map_list: names.pid_of("Item_WornAppearanceMapList"),
            worn_appearance: need("Item_WornAppearance")?,
            appearance_key: need("Item_AppearanceKey")?,
            species_of_wearer: need("Item_SpeciesOfWearer")?,
            sex_of_wearer: need("Item_SexOfWearer")?,
            phys_obj: names.pid_of("PhysObj"),
            default_slot: names.pid_of("Inventory_DefaultSlot"),
            parenting_location: names.pid_of("Inventory_ParentingLocation"),
            paperdoll_parenting_location: names.pid_of("Inventory_PaperdollParentingLocation"),
            equipment_category: names.pid_of("Item_EquipmentCategory"),
            clothing_color: names.pid_of("Item_ClothingColor"),
            clothing_color_secondary: names.pid_of("Item_ClothingColorSecondary"),
        })
    }
}

/// One `Item_WornAppearanceMapStruct` (`setup.md` §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WornAppearanceRef {
    /// The wardrobe record (0x20).
    pub appearance: Did,
    /// The entry key in that record.
    pub key: u32,
    /// `Item_SpeciesOfWearer`: a `SpeciesType` key.
    pub species: u32,
    /// `Item_SexOfWearer`: 0x1000 male, 0x2000 female.
    pub sex: u32,
}

/// The visual properties of an item.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ItemVisual {
    /// The wardrobe structs, one per supported (species, sex).
    pub worn: Vec<WornAppearanceRef>,
    /// `PhysObj`: the entity drawn when the item is held (or lies on the ground).
    pub phys_obj: Option<Did>,
    /// `Inventory_DefaultSlot` bits.
    pub default_slot: Option<u64>,
    /// `Inventory_ParentingLocation`: a `HoldingLocation` key.
    pub parenting_location: Option<u32>,
    /// `Inventory_PaperdollParentingLocation`: a `HoldingLocation` key.
    pub paperdoll_parenting_location: Option<u32>,
    /// `Item_EquipmentCategory`, read as its full bit pattern (`PropertyValue::as_bits`):
    /// the property is a 64-bit bitfield (type code 14, a u64 bit mask,
    /// `property-system.md` §4.1) in which bit *k* − 1 stands for enum key *k* (§7.3), with
    /// bits above 31 in use. `appearance.md` §15 (the `Item_EquipmentCategory` storage OPEN,
    /// answered 2026-10-07) confirms the bitfield reading.
    pub equipment_category: Option<u64>,
    /// `Item_ClothingColor`: default dye code of the first channel (§4.9).
    pub clothing_color: Option<f32>,
    /// `Item_ClothingColorSecondary`: default dye code of the second channel.
    pub clothing_color_secondary: Option<f32>,
}

/// How an item appears on a wearer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ItemLook {
    /// Drawn through a wardrobe record's parts (`appearance.md` §4).
    Worn {
        /// The map struct chosen for the wearer.
        reference: WornAppearanceRef,
        /// Dye code of the first channel (`None`: no dye layer).
        dye: Option<f32>,
        /// Dye code of the second channel.
        secondary_dye: Option<f32>,
    },
    /// Drawn through its `PhysObj` chain at a holding location (§5).
    Held {
        /// The entity to resolve (`setup.md` §3.1).
        phys_obj: Did,
    },
    /// Nothing to draw (jewellery, items without either path).
    Invisible,
}

impl ItemVisual {
    /// Reads the visual properties from an item property set.
    #[must_use]
    pub fn from_property_set(set: &PropertySet, pids: &ItemPids) -> Self {
        let get = |p: Option<u32>| p.and_then(|p| set.get(p));
        let mut worn = Vec::new();
        for el in get(pids.worn_map_list)
            .map(PropertyValue::children)
            .unwrap_or(&[])
        {
            let Some(s) = el.value.as_struct() else {
                continue;
            };
            let (Some(appearance), Some(key)) = (
                s.get(pids.worn_appearance).and_then(PropertyValue::as_did),
                s.get(pids.appearance_key).and_then(as_u32),
            ) else {
                continue;
            };
            worn.push(WornAppearanceRef {
                appearance,
                key,
                species: s.get(pids.species_of_wearer).and_then(as_u32).unwrap_or(0),
                sex: s.get(pids.sex_of_wearer).and_then(as_u32).unwrap_or(0),
            });
        }
        Self {
            worn,
            phys_obj: get(pids.phys_obj)
                .and_then(PropertyValue::as_did)
                .filter(|d| !d.is_null()),
            default_slot: get(pids.default_slot).and_then(PropertyValue::as_bits),
            parenting_location: get(pids.parenting_location).and_then(as_u32),
            paperdoll_parenting_location: get(pids.paperdoll_parenting_location).and_then(as_u32),
            equipment_category: get(pids.equipment_category).and_then(PropertyValue::as_bits),
            clothing_color: get(pids.clothing_color).and_then(as_f32),
            clothing_color_secondary: get(pids.clothing_color_secondary).and_then(as_f32),
        }
    }

    /// The wardrobe struct for (`species`, `sex`): an exact match, else the struct of the
    /// species sharing the wearer's wardrobe body, same sex (`appearance.md` §2.3
    /// Decision).
    #[must_use]
    pub fn worn_for(
        &self,
        species: u32,
        sex: u32,
        fallbacks: &WardrobeFallbacks,
    ) -> Option<&WornAppearanceRef> {
        let find = |sp: u32| self.worn.iter().find(|w| w.species == sp && w.sex == sex);
        find(species).or_else(|| fallbacks.fallback(species).and_then(find))
    }

    /// Worn versus held for a wearer (`setup.md` §13.3): an item with a wardrobe struct for
    /// the wearer is worn; otherwise an item with a `PhysObj` is held; otherwise invisible.
    /// A dye code of 0.0 means "no dye" (`material.md` §12.4).
    #[must_use]
    pub fn look(&self, species: u32, sex: u32, fallbacks: &WardrobeFallbacks) -> ItemLook {
        if let Some(r) = self.worn_for(species, sex, fallbacks) {
            let dye = |c: Option<f32>| c.filter(|c| *c != 0.0 && !c.is_nan());
            return ItemLook::Worn {
                reference: *r,
                dye: dye(self.clothing_color),
                secondary_dye: dye(self.clothing_color_secondary),
            };
        }
        match self.phys_obj {
            Some(p) if self.worn.is_empty() => ItemLook::Held { phys_obj: p },
            // An item worn by other bodies but not by this wearer shows nothing on it
            // (`appearance.md` §10: "item not drawn").
            _ => ItemLook::Invisible,
        }
    }
}

/// Species whose wardrobe body is another species' (`appearance.md` §2.3 Decision):
/// (species, species to borrow wardrobe structs from).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WardrobeFallbacks {
    /// (species, fallback species) pairs.
    pub pairs: Vec<(u32, u32)>,
}

impl WardrobeFallbacks {
    /// The `SpeciesType` label pairs of `appearance.md` §2.3: Beorning → Human, RiverHobbit
    /// → Human, HighElf → Elf, DwarfStoutAxe → Dwarf.
    pub const LABELS: [(&'static str, &'static str); 4] = [
        ("Beorning", "Human"),
        ("RiverHobbit", "Human"),
        ("HighElf", "Elf"),
        ("DwarfStoutAxe", "Dwarf"),
    ];

    /// Builds the table from the `SpeciesType` labels ([`WardrobeFallbacks::LABELS`]);
    /// `key_of` looks a label up in the mapper (with its base chain). Pairs whose labels are
    /// missing are left out.
    #[must_use]
    pub fn from_species_labels(key_of: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            pairs: Self::LABELS
                .iter()
                .filter_map(|(a, b)| Some((key_of(a)?, key_of(b)?)))
                .collect(),
        }
    }

    /// The species whose structs `species` borrows.
    #[must_use]
    pub fn fallback(&self, species: u32) -> Option<u32> {
        self.pairs
            .iter()
            .find(|(s, _)| *s == species)
            .map(|&(_, f)| f)
    }
}

/// The modifier of a worn item's own element (`appearance.md` §4.7 Decision): its wardrobe
/// record with key `Item_AppearanceKey` and modifier 1.0.
pub const WORN_MODIFIER: f32 = 1.0;

/// The block a worn item draws: the block modifier [`WORN_MODIFIER`] selects in the entry
/// for `key` by the rule of `appearance.md` §3.2.1 (the `variant` 1.0 block, which every
/// entry a struct finds has, §4.5). `None` when the record lacks the key (a failure
/// category of §10: the item is not drawn for that body) or no block is at or below 1.0.
#[must_use]
pub fn worn_block(record: &AppearanceRecord, key: u32) -> Option<&AppearanceBlock> {
    record.entry(key)?.block_for_modifier(WORN_MODIFIER)
}

/// The dye block for code `code` in the dye entry `dye_key` (the `ClothingColor` or
/// `ClothingColorSecondary` entry) of the **same** wardrobe record (§4.9), by the block rule
/// of §3.2.1. `None` for code 0.0 (no dye element, §4.9 Decision), a missing entry or a
/// code below every block.
#[must_use]
pub fn dye_block(record: &AppearanceRecord, dye_key: u32, code: f32) -> Option<&AppearanceBlock> {
    if code == 0.0 {
        return None;
    }
    record.entry(dye_key)?.dye_block(code)
}

/// Holding-location choice for held items in a **paperdoll** view (`skeleton-anim.md` §8.5,
/// `appearance.md` §5.3): a paperdoll location per equipment slot, replaced by the item's
/// own `Inventory_PaperdollParentingLocation`. In the world the placement scripts of
/// `appearance.md` §5.6 decide instead; `Inventory_ParentingLocation` is read by no script
/// and has no known role there.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HoldingRule {
    /// (`ContainerSlot` bits, `HoldingLocation` key) pairs: the inventory-control object's
    /// slot-to-paperdoll mapping.
    pub slot_locations: Vec<(u64, u32)>,
}

impl HoldingRule {
    /// The paperdoll location for `item`: its `Inventory_PaperdollParentingLocation` when
    /// present, else the paperdoll location of the first slot pair whose bits intersect the
    /// item's default slot. `None` when nothing applies (class-slot items and instruments
    /// have no location in the data).
    #[must_use]
    pub fn paperdoll_location(&self, item: &ItemVisual) -> Option<u32> {
        item.paperdoll_parenting_location.or_else(|| {
            let slot = item.default_slot?;
            self.slot_locations
                .iter()
                .find(|(bits, _)| bits & slot != 0)
                .map(|&(_, l)| l)
        })
    }
}

/// A held prop of an NPC-like figure (`appearance.md` §5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcHeldItem {
    /// Which of the four item properties (1–4).
    pub index: u8,
    /// The prop entity (0x47).
    pub entity: Did,
    /// Its `HoldingLocation` key, if set.
    pub location: Option<u32>,
}

/// Reads `NPC_Cosmetic_Inventory_Item1` … `4` with `NPC_Cosmetic_Inventory_HoldingLocation1`
/// … `4` from an NPC or instance property set (`appearance.md` §5.5). Null items are
/// skipped.
///
/// `NPC_Cosmetic_Inventory_Visible` is not applied here: read it with
/// [`npc_cosmetic_inventory_visible`]; while it is false the props are not drawn
/// (`appearance.md` §5.5 Rule).
#[must_use]
pub fn npc_held_items<N: PropertyNames + ?Sized>(set: &PropertySet, names: &N) -> Vec<NpcHeldItem> {
    let mut out = Vec::new();
    for i in 1..=4u8 {
        let item = names
            .pid_of(&format!("NPC_Cosmetic_Inventory_Item{i}"))
            .and_then(|p| set.get(p))
            .and_then(PropertyValue::as_did)
            .filter(|d| !d.is_null());
        let Some(entity) = item else {
            continue;
        };
        let location = names
            .pid_of(&format!("NPC_Cosmetic_Inventory_HoldingLocation{i}"))
            .and_then(|p| set.get(p))
            .and_then(as_u32);
        out.push(NpcHeldItem {
            index: i,
            entity,
            location,
        });
    }
    out
}

/// The value of `NPC_Cosmetic_Inventory_Visible` in `set` (`appearance.md` §5.5), or `None`
/// when the set does not carry it (the dictionary default, true, then applies). False hides
/// the figure's cosmetic props, always (§5.5 Rule, §14).
#[must_use]
pub fn npc_cosmetic_inventory_visible<N: PropertyNames + ?Sized>(
    set: &PropertySet,
    names: &N,
) -> Option<bool> {
    names
        .pid_of("NPC_Cosmetic_Inventory_Visible")
        .and_then(|p| set.get(p))
        .and_then(PropertyValue::as_f64)
        .map(|v| v != 0.0)
}
