// SPDX-License-Identifier: GPL-3.0-or-later
//! Reading the inventory facts from the install: `InventoryControl` (`character-panels.md`
//! §4.2), the slot mapper's special keys and names, and the paperdoll slot table of the
//! equipment page's layout (§3.3).

use tessera_formats::gameplay::common::Pid;
use tessera_formats::ui_layout::{UiEnum, UiLabels, UiLayout};
use tessera_props::RecordSource;
use tessera_wstate::ObjectStore;

use super::model::InventoryControl;
use super::slots::{EQUIPMENT_FLAG_LABEL, PaperdollPids, PaperdollTable};
use crate::error::{GameDataError, Result};
use crate::vocab::{label_key, mapper_chain};

/// WEENIECONTENT label of the inventory control record (`items.md` §6.4).
pub const INVENTORY_CONTROL_LABEL: &str = "InventoryControl";

/// `UILAYOUT` label of the equipment page's layout (`character-panels.md` §1).
pub const EQUIPMENT_PAGE_LAYOUT_LABEL: &str = "characterequipmentpage";

/// Element label of the slot field (`character-panels.md` §3.3).
pub const SLOT_FIELD_LABEL: &str = "CharacterEquipmentPage_SlotField";

/// Root-map label of the layout DID map (`ui-layout.md` §3.2).
const UILAYOUT_LABEL: &str = "UILAYOUT";
/// Root-map label of the enum-mapper DID map (`ui-layout.md` §3.3).
const EMAPPER_LABEL: &str = "EMAPPER";

/// The property whose mapper names the `ContainerSlot` keys (`items.md` §5.4).
const SLOT_PROPERTY: &str = "Inventory_DefaultSlot";

/// Reads `InventoryControl` (`character-panels.md` §4.2) through its WEENIECONTENT label.
///
/// # Errors
///
/// [`GameDataError::MissingRecord`] when the install has no such record; the object
/// layer's errors when it cannot be read.
pub fn load_inventory_control<S: RecordSource>(store: &ObjectStore<S>) -> Result<InventoryControl> {
    let did = store
        .weenie(INVENTORY_CONTROL_LABEL)?
        .ok_or_else(|| GameDataError::MissingRecord(INVENTORY_CONTROL_LABEL.into()))?;
    let set = store
        .properties(did)?
        .ok_or_else(|| GameDataError::MissingRecord(INVENTORY_CONTROL_LABEL.into()))?;
    let reg = store.game_data().registry()?;
    let int = |name: &str| {
        set.get_by_name(reg, name)
            .and_then(tessera_props::Value::as_i32)
            .and_then(|v| u32::try_from(v).ok())
    };
    Ok(InventoryControl {
        base_backpack_slots: int("InventoryControl_BaseBackpackSlots"),
        slots_per_bag: int("InventoryControl_SlotsPerBag"),
        max_bags: int("InventoryControl_MaxNumBags"),
        slots_per_pouch: int("InventoryControl_SlotsPerPouch"),
        equipment_slots: int("InventoryControl_NumEquipmentSlots"),
    })
}

/// The `ContainerSlot` key of `label` (for example [`EQUIPMENT_FLAG_LABEL`]) in the
/// mapper the dictionary names for `Inventory_DefaultSlot`, base chain included.
///
/// # Errors
///
/// The property layer's error when the dictionary cannot be read.
pub fn slot_label_key<S: RecordSource>(store: &ObjectStore<S>, label: &str) -> Result<Option<u32>> {
    let data = store.game_data();
    let dict = data.registry()?.dictionary();
    Ok(label_key(
        data,
        dict,
        Pid::resolve(dict, SLOT_PROPERTY),
        label,
    ))
}

/// The key of the `Equipment` flag ([`slot_label_key`] of [`EQUIPMENT_FLAG_LABEL`]).
///
/// # Errors
///
/// As [`slot_label_key`].
pub fn equipment_flag_key<S: RecordSource>(store: &ObjectStore<S>) -> Result<Option<u32>> {
    slot_label_key(store, EQUIPMENT_FLAG_LABEL)
}

/// The `ContainerSlot` mapper's name of each key (nearest mapper of the chain first), for
/// [`super::check_api_names`] and display.
///
/// # Errors
///
/// As [`slot_label_key`].
pub fn slot_names<S: RecordSource>(
    store: &ObjectStore<S>,
) -> Result<std::collections::BTreeMap<u32, String>> {
    let data = store.game_data();
    let dict = data.registry()?.dictionary();
    let mut out = std::collections::BTreeMap::new();
    let Some(start) =
        tessera_formats::gameplay::common::enum_mapper_of(dict, Pid::resolve(dict, SLOT_PROPERTY))
    else {
        return Ok(out);
    };
    for m in mapper_chain(data, start) {
        for (k, n) in m.names() {
            out.entry(*k).or_insert_with(|| n.clone());
        }
    }
    Ok(out)
}

/// Reads the paperdoll slot table (§3.3) from the layout the `UILAYOUT` map names
/// [`EQUIPMENT_PAGE_LAYOUT_LABEL`], at the element labelled [`SLOT_FIELD_LABEL`].
///
/// # Errors
///
/// [`GameDataError::MissingRecord`] when the layout or the element is missing;
/// [`GameDataError::View`] when the layout does not decode; the property layer's errors.
pub fn load_paperdoll<S: RecordSource>(store: &ObjectStore<S>) -> Result<PaperdollTable> {
    let gd = store.game_data();
    let registry = gd.registry()?.clone();
    let labels = UiLabels::load(
        |l| gd.lookup_path(&[EMAPPER_LABEL, l]).ok().flatten(),
        |d| gd.enum_mapper(d).ok().flatten().map(|m| (*m).clone()),
    );
    let missing = |what: &str| GameDataError::MissingRecord(what.into());
    let layout_did = gd
        .did_map_by_label(UILAYOUT_LABEL)?
        .and_then(|m| m.by_label(EQUIPMENT_PAGE_LAYOUT_LABEL))
        .ok_or_else(|| missing(EQUIPMENT_PAGE_LAYOUT_LABEL))?;
    let bytes = gd
        .read(layout_did)?
        .ok_or_else(|| missing(EQUIPMENT_PAGE_LAYOUT_LABEL))?;
    let layout = UiLayout::parse_expecting(&bytes, layout_did, registry.as_ref()).map_err(|e| {
        GameDataError::View {
            did: layout_did,
            reason: e.to_string(),
        }
    })?;
    let field = labels
        .key(UiEnum::ElementId, SLOT_FIELD_LABEL)
        .ok_or_else(|| missing(SLOT_FIELD_LABEL))?;
    let pids = PaperdollPids::resolve(|n| registry.pid(n));
    PaperdollTable::from_layout(&layout, field, &pids, |id| {
        labels.element(id).map(str::to_owned)
    })
    .ok_or_else(|| missing(SLOT_FIELD_LABEL))
}
