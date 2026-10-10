// SPDX-License-Identifier: GPL-3.0-or-later
//! Equipment slot keys, the paperdoll slot table and slot compatibility
//! (`character-panels.md` §3.3, §6, §8.2, CPI1–CPI3; `items.md` §5.4;
//! `lua-plugin-api.md` §15.1).
//!
//! A `ContainerSlot` value is a bit field; its key *k* (1 … 64) is bit *k* − 1
//! (`property-system.md` §7.3). The feed and the session name equipment slots by key
//! (`scenario-feed.md` §5.5); layouts and item records hold bit fields.

use std::collections::{BTreeMap, BTreeSet};

use tessera_formats::gameplay::items::Slots;
use tessera_formats::ui_layout::UiLayout;
use tessera_formats::{ElementDesc, PropertySet, PropertyValue, StringInfo};

/// Highest `ContainerSlot` key a 64-bit field can hold.
pub const MAX_SLOT_KEY: u32 = 64;

/// The bit of `ContainerSlot` key `key` (1 … 64), or `None` outside that range.
#[must_use]
pub fn slot_bit(key: u32) -> Option<u64> {
    (1..=MAX_SLOT_KEY).contains(&key).then(|| 1u64 << (key - 1))
}

/// The keys of the bits set in `bits`, ascending.
pub fn slot_keys(bits: u64) -> impl Iterator<Item = u32> {
    (1..=MAX_SLOT_KEY).filter(move |&k| bits & (1u64 << (k - 1)) != 0)
}

/// The key of `bits` when exactly one bit is set (a slot element's `Container_Slot`,
/// CPI1), else `None`.
#[must_use]
pub fn single_slot_key(bits: u64) -> Option<u32> {
    (bits.count_ones() == 1).then(|| bits.trailing_zeros() + 1)
}

/// Whether `bits` has key `key` set; `false` for a key outside 1 … 64.
#[must_use]
pub fn has_slot_key(bits: u64, key: u32) -> bool {
    slot_bit(key).is_some_and(|b| bits & b != 0)
}

// -----------------------------------------------------------------------------------------
// Plugin API indices (lua-plugin-api.md §15.1, character-panels.md §8)
// -----------------------------------------------------------------------------------------

/// `Equipment:GetSize()`: the number of API slot values (`lua-plugin-api.md` §15.1, P82).
pub const API_EQUIPMENT_SIZE: u32 = 21;

/// Lowest data key that has an API `Equipment` value (`Head`, key 2).
pub const FIRST_API_EQUIPMENT_KEY: u32 = 2;

/// Highest data key that has an API `Equipment` value (`Mounted` / `Bridle`, key 22).
pub const LAST_API_EQUIPMENT_KEY: u32 = 22;

/// The API `Turbine.Gameplay.Equipment` value of data key `key`: key − 1 for keys 2 … 22
/// (`character-panels.md` §8.2, P82); `None` for every other key (the weapon FX slots
/// 23 … 25 included, which plugins do not see).
#[must_use]
pub fn api_equipment_index(key: u32) -> Option<u32> {
    (FIRST_API_EQUIPMENT_KEY..=LAST_API_EQUIPMENT_KEY)
        .contains(&key)
        .then(|| key - 1)
}

/// The data key of API `Equipment` value `value` as a Lua number: value + 1 for an
/// integer 1 … 21; `None` for a non-integer, a non-finite number or any other value
/// (`Equipment:GetItem` then returns nil, `lua-plugin-api.md` §15.1).
#[must_use]
pub fn key_of_api_equipment_index(value: f64) -> Option<u32> {
    let v = integer_in(value, 1, u64::from(API_EQUIPMENT_SIZE))?;
    u32::try_from(v).ok().map(|v| v + 1)
}

/// The 1-based plugin `Backpack` index of backpack slot `slot` (`character-panels.md`
/// §8.1: index *i* is slot *i* − 1).
#[must_use]
pub fn backpack_index(slot: u16) -> u32 {
    u32::from(slot) + 1
}

/// The backpack slot of plugin index `index` (a Lua number) for a backpack of size
/// `size`: `index` − 1 for an integer `index` in 1 … `size`; `None` otherwise (a
/// non-integer, a non-finite number, out of range; `Backpack:GetItem` then returns nil).
#[must_use]
pub fn slot_of_backpack_index(index: f64, size: u32) -> Option<u16> {
    let v = integer_in(index, 1, u64::from(size))?;
    u16::try_from(v - 1).ok()
}

/// `value` as an integer in `lo ..= hi`, if it is one.
fn integer_in(value: f64, lo: u64, hi: u64) -> Option<u64> {
    if !value.is_finite() || value.fract() != 0.0 || value < 1.0 {
        return None;
    }
    // Finite, integral and at least 1; values beyond u64 saturate and fail the range test.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let v = value as u64;
    (lo..=hi).contains(&v).then_some(v)
}

/// The API `Equipment` enumeration (`lua-plugin-api.md` Appendix A): (value, name), the
/// aliases `Shield` and `Instrument` included, without `Undefined`. These are Tessera's
/// plugin API names, used only to check CPI3 against the install's mapper names.
pub const API_EQUIPMENT_NAMES: [(u32, &str); 23] = [
    (1, "Head"),
    (2, "Chest"),
    (3, "Legs"),
    (4, "Gloves"),
    (5, "Boots"),
    (6, "Shoulder"),
    (7, "Back"),
    (8, "Bracelet1"),
    (9, "Bracelet2"),
    (10, "Necklace"),
    (11, "Ring1"),
    (12, "Ring2"),
    (13, "Earring1"),
    (14, "Earring2"),
    (15, "Pocket"),
    (16, "PrimaryWeapon"),
    (17, "SecondaryWeapon"),
    (17, "Shield"),
    (18, "Instrument"),
    (18, "RangedWeapon"),
    (19, "CraftTool"),
    (20, "Class"),
    (21, "Bridle"),
];

/// The API names whose data mapper name differs in form (CPI3: (API name, mapper name)).
const API_NAME_ALIASES: [(&str, &str); 5] = [
    ("Pocket", "Pocket1"),
    ("PrimaryWeapon", "Weapon_Primary"),
    ("SecondaryWeapon", "Weapon_Secondary"),
    ("RangedWeapon", "Weapon_Ranged"),
    ("Bridle", "Mounted"),
];

/// CPI3: for every API `Equipment` value 1 … 21, the data key value + 1 must have a
/// `ContainerSlot` mapper name equal to one of the value's API names (directly or through
/// the aliases of CPI3). `mapper_name` gives the mapper's name of a key. Returns the
/// values that fail, each with the mapper name found.
pub fn check_api_names(mapper_name: impl Fn(u32) -> Option<String>) -> Vec<(u32, Option<String>)> {
    let mut by_value: BTreeMap<u32, Vec<&str>> = BTreeMap::new();
    for (v, n) in API_EQUIPMENT_NAMES {
        by_value.entry(v).or_default().push(n);
    }
    let mut failures = Vec::new();
    for (value, names) in by_value {
        let found = mapper_name(value + 1);
        let ok = found.as_deref().is_some_and(|f| {
            names.iter().any(|n| {
                *n == f
                    || API_NAME_ALIASES
                        .iter()
                        .any(|(api, data)| api == n && *data == f)
            })
        });
        if !ok {
            failures.push((value, found));
        }
    }
    failures
}

// -----------------------------------------------------------------------------------------
// The slot mapper's special keys and slot compatibility (§6)
// -----------------------------------------------------------------------------------------

/// The `ContainerSlot` labels with a meaning of their own (`items.md` §5.4,
/// `appearance.md` §4.1): the `Equipment` flag of an equippable default slot and the two
/// non-equipment places `Backpack` and `Overflow`.
pub const EQUIPMENT_FLAG_LABEL: &str = "Equipment";
/// See [`EQUIPMENT_FLAG_LABEL`].
pub const BACKPACK_SLOT_LABEL: &str = "Backpack";
/// See [`EQUIPMENT_FLAG_LABEL`].
pub const OVERFLOW_SLOT_LABEL: &str = "Overflow";

/// What the slot rules of §6 need from the install: the key of the `Equipment` flag and
/// the equipment slot keys (the keys of the paperdoll slot table, [`PaperdollTable::keys`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SlotRules {
    /// Key of the slot mapper's `Equipment` label, if the install has it.
    pub equipment_flag: Option<u32>,
    /// The equipment slot keys.
    pub equipment_keys: BTreeSet<u32>,
}

impl SlotRules {
    /// Whether an item with `slots` may be dropped on the paperdoll slot of `key`: the
    /// key's bit is set in its `Inventory_CompatibleSlot` (§6 row 1). An item without the
    /// property fits nowhere.
    #[must_use]
    pub fn fits(&self, slots: &Slots, key: u32) -> bool {
        self.equipment_keys.contains(&key) && slots.compatible.is_some_and(|b| has_slot_key(b, key))
    }

    /// Whether `key` is a slot of the `Equipment` container: a key of the paperdoll page
    /// ([`SlotRules::equipment_keys`]) or a key with an API `Equipment` value (2 … 22,
    /// `character-panels.md` §8.2), which adds key 22 (`Mounted`), a slot that is not on
    /// the page.
    #[must_use]
    pub fn is_equipment_key(&self, key: u32) -> bool {
        self.equipment_keys.contains(&key) || api_equipment_index(key).is_some()
    }

    /// The key an activated backpack item is equipped to (§6 row 3): when its
    /// `Inventory_DefaultSlot` has the `Equipment` bit, the lowest key set in it that is a
    /// slot of the `Equipment` container ([`SlotRules::is_equipment_key`], so `Mounted`
    /// counts although the page does not show it); otherwise (or when no such key is set)
    /// `None`, and activation uses the item.
    #[must_use]
    pub fn default_equip_key(&self, slots: &Slots) -> Option<u32> {
        let default = slots.default?;
        let flag = self.equipment_flag?;
        if !has_slot_key(default, flag) {
            return None;
        }
        slot_keys(default).find(|k| *k != flag && self.is_equipment_key(*k))
    }
}

// -----------------------------------------------------------------------------------------
// Paperdoll slot table (§3.3)
// -----------------------------------------------------------------------------------------

/// One slot of the equipment page (§3.3).
#[derive(Debug, Clone, PartialEq)]
pub struct PaperdollSlot {
    /// The slot element's ID (`UIElementID`).
    pub element: u32,
    /// The slot element's label, when the labels are known.
    pub label: Option<String>,
    /// Its parent field's ID.
    pub parent: u32,
    /// The `Container_Slot` value as stored (one bit in the data, CPI1).
    pub bits: u64,
    /// The key of that bit, when exactly one is set.
    pub key: Option<u32>,
    /// The parent field's position inside the slot field (design units).
    pub position: (i32, i32),
    /// `UI_Item_DisplayEmptyOverride`: the tooltip text while the slot is empty.
    pub empty_tooltip: Option<StringInfo>,
}

/// The equipment page's slot table (§3.3), in the order of the slot field's children.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PaperdollTable {
    /// The slots found under the slot field.
    pub slots: Vec<PaperdollSlot>,
    /// The element IDs of `UI_Inventory_EquipmentSlotList`, in stored order.
    pub listed: Vec<u32>,
}

/// The property IDs the slot table reads from layout elements.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaperdollPids {
    /// `Container_Slot`.
    pub container_slot: Option<u32>,
    /// `UI_Item_DisplayEmptyOverride`.
    pub empty_override: Option<u32>,
    /// `UI_Inventory_EquipmentSlotList`.
    pub slot_list: Option<u32>,
}

impl PaperdollPids {
    /// Looks the three names up with `pid` (normally the dictionary).
    pub fn resolve(pid: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            container_slot: pid("Container_Slot"),
            empty_override: pid("UI_Item_DisplayEmptyOverride"),
            slot_list: pid("UI_Inventory_EquipmentSlotList"),
        }
    }
}

/// A broken paperdoll invariant (CPI1, CPI2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaperdollProblem {
    /// CPI1: a slot element's `Container_Slot` does not have exactly one bit set.
    NotOneBit {
        /// The slot element.
        element: u32,
        /// The value.
        bits: u64,
    },
    /// CPI1: two slot elements share a bit.
    DuplicateKey {
        /// The key.
        key: u32,
    },
    /// CPI2: an element of `UI_Inventory_EquipmentSlotList` is not a slot element of the
    /// slot field.
    ListedNotSlot {
        /// The listed element ID.
        element: u32,
    },
    /// The slot field has no slot at all.
    Empty,
}

/// Most slots read from one slot field (hostile-input bound; the data has 23).
pub const MAX_PAPERDOLL_SLOTS: usize = 256;

/// The key-like value of a property (enum, bit field, integer, DID, PID).
fn key_value(v: &PropertyValue) -> Option<u64> {
    match v {
        PropertyValue::Enum(e) | PropertyValue::PropertyId(e) | PropertyValue::Bitfield32(e) => {
            Some(u64::from(*e))
        }
        PropertyValue::Bitfield64(b) => Some(*b),
        PropertyValue::BitfieldAny(b) => {
            let mut out = [0u8; 8];
            for (o, i) in out.iter_mut().zip(&b.bytes) {
                *o = *i;
            }
            Some(u64::from_le_bytes(out))
        }
        PropertyValue::Int(i) => u64::try_from(*i).ok(),
        PropertyValue::Int64(i) => u64::try_from(*i).ok(),
        PropertyValue::DidRef(d) => Some(u64::from(d.0)),
        _ => None,
    }
}

fn get(set: &PropertySet, pid: Option<u32>) -> Option<&PropertyValue> {
    set.get(pid?)
}

impl PaperdollTable {
    /// Reads the slot table from the slot field `field` (the element labelled
    /// `CharacterEquipmentPage_SlotField`): every child (a parent field) whose own child
    /// carries `Container_Slot` is a slot; the parent's position is the slot's. Only the
    /// elements' own properties are read (not their base elements'). At most
    /// [`MAX_PAPERDOLL_SLOTS`] slots are kept. `label` names an element ID.
    pub fn from_slot_field(
        field: &ElementDesc,
        pids: &PaperdollPids,
        label: impl Fn(u32) -> Option<String>,
    ) -> Self {
        let mut out = Self {
            listed: get(&field.block.properties, pids.slot_list)
                .map(|v| {
                    v.children()
                        .iter()
                        .filter_map(|e| key_value(&e.value))
                        .filter_map(|k| u32::try_from(k).ok())
                        .take(MAX_PAPERDOLL_SLOTS)
                        .collect()
                })
                .unwrap_or_default(),
            ..Self::default()
        };
        for parent in field.children_in_read_order() {
            for slot in parent.children_in_read_order() {
                if out.slots.len() >= MAX_PAPERDOLL_SLOTS {
                    tracing::warn!("paperdoll slot field has too many slots; cut");
                    return out;
                }
                let Some(bits) =
                    get(&slot.block.properties, pids.container_slot).and_then(key_value)
                else {
                    continue;
                };
                out.slots.push(PaperdollSlot {
                    element: slot.element_id,
                    label: label(slot.element_id),
                    parent: parent.element_id,
                    bits,
                    key: single_slot_key(bits),
                    position: (parent.block.rect.x, parent.block.rect.y),
                    empty_tooltip: match get(&slot.block.properties, pids.empty_override) {
                        Some(PropertyValue::StringInfo(s)) => Some((**s).clone()),
                        _ => None,
                    },
                });
            }
        }
        out
    }

    /// Finds the element `field_id` anywhere in `layout` and reads it with
    /// [`PaperdollTable::from_slot_field`]; `None` when the layout has no such element.
    pub fn from_layout(
        layout: &UiLayout,
        field_id: u32,
        pids: &PaperdollPids,
        label: impl Fn(u32) -> Option<String>,
    ) -> Option<Self> {
        let mut found = None;
        for top in &layout.elements {
            top.walk(&mut |e, _| {
                if found.is_none() && e.element_id == field_id {
                    found = Some(e);
                }
            });
            if found.is_some() {
                break;
            }
        }
        found.map(|f| Self::from_slot_field(f, pids, label))
    }

    /// The equipment slot keys of the table (slots with exactly one bit), ascending.
    #[must_use]
    pub fn keys(&self) -> BTreeSet<u32> {
        self.slots.iter().filter_map(|s| s.key).collect()
    }

    /// The slot of data key `key`.
    #[must_use]
    pub fn slot(&self, key: u32) -> Option<&PaperdollSlot> {
        self.slots.iter().find(|s| s.key == Some(key))
    }

    /// The broken invariants CPI1 and CPI2 (empty when the table is sound).
    #[must_use]
    pub fn problems(&self) -> Vec<PaperdollProblem> {
        let mut out = Vec::new();
        if self.slots.is_empty() {
            out.push(PaperdollProblem::Empty);
        }
        let mut seen = BTreeSet::new();
        for s in &self.slots {
            match s.key {
                None => out.push(PaperdollProblem::NotOneBit {
                    element: s.element,
                    bits: s.bits,
                }),
                Some(k) if !seen.insert(k) => out.push(PaperdollProblem::DuplicateKey { key: k }),
                Some(_) => {}
            }
        }
        let elements: BTreeSet<u32> = self.slots.iter().map(|s| s.element).collect();
        for &e in &self.listed {
            if !elements.contains(&e) {
                out.push(PaperdollProblem::ListedNotSlot { element: e });
            }
        }
        out
    }

    /// [`SlotRules`] for this table and the install's `Equipment` flag key.
    #[must_use]
    pub fn rules(&self, equipment_flag: Option<u32>) -> SlotRules {
        SlotRules {
            equipment_flag,
            equipment_keys: self.keys(),
        }
    }
}
