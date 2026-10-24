// SPDX-License-Identifier: GPL-3.0-or-later
//! The install facts the P1 panels read (decision 0024 P1): the bag layout and the
//! paperdoll slot table (`character-panels.md` §3.3, §4.2), the slot rules of §6, the item
//! tooltip engine (`item-tooltips.md`, `tessera-gamedata`), and per item record the values
//! the item slots and the plugin views show ([`ItemFacts`]: names, slots, icon layers,
//! quality, stack size; `lua-plugin-api.md` §15.1, `icon-layers.md` §3).
//!
//! Loaded once per run, off the frame ([`CharacterData::open`]). The tooltip engine
//! borrows its store for the run's lifetime: the store is allocated once and never freed
//! (one per opened install).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use tessera_formats::gameplay::items::{ItemSchema, ItemView, Slots};
use tessera_formats::{Did, PropertySet, PropertyValue, StringInfo};
use tessera_gamedata::GameDataStore;
use tessera_gamedata::inventory::{
    BagLayout, PaperdollTable, SlotRules, equipment_flag_key, load_inventory_control,
    load_paperdoll,
};
use tessera_gamedata::tooltip::{InstallTooltipData, TooltipData, TooltipEngine};
use tessera_material::icon::IconStack;
use tessera_plugins::ItemInfoData;
use tessera_text::{NamedTables, RenderOptions, STRING_TABLE_MAP_LABEL, Strings, Values};
use tessera_ui::ImageRef;
use tessera_ui::game::IconImage;

use super::state::NO_INSTALL_BAGS;

/// Most item records whose [`ItemFacts`] are kept (the cache is emptied when full).
pub const MAX_CACHED_FACTS: usize = 16_384;

/// The values of one item record the slots and the plugin views show.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemFacts {
    /// The plugin views' `ItemInfo` values (`lua-plugin-api.md` §15.1).
    pub info: Arc<ItemInfoData>,
    /// The composed icon's layers (`icon-layers.md` §3.1), bottom first; `None` without
    /// an image layer.
    pub icon: Option<IconImage>,
    /// `Inventory_CompatibleSlot`, `Inventory_DefaultSlot`, `Inventory_PrecludedSlot`.
    pub slots: Slots,
    /// `Usage_MinLevel` (the unusable overlay, `character-panels.md` §5).
    pub min_level: Option<i32>,
    /// `Usage_RequiredClassList` (the unusable overlay).
    pub required_classes: Vec<u32>,
    /// `Item_WearState` key.
    pub wear_state: Option<u32>,
    /// `Icon_Layer_ImageDID` (`Item:GetIconImageID()` before an instance override).
    pub icon_image: u32,
    /// "Read *P*" (`icon-layers.md` §3.1) of the record's item layer properties by name
    /// (the instance override excluded), for composing an instance's icon.
    pub icon_layers: Vec<(&'static str, Option<Did>)>,
}

impl ItemFacts {
    /// The facts of a literal item (`scenario-feed.md` §5.2): its text as the name, nothing
    /// else.
    #[must_use]
    pub fn literal(text: &str) -> Self {
        Self {
            info: Arc::new(ItemInfoData::literal(text)),
            icon: None,
            slots: Slots::default(),
            min_level: None,
            required_classes: Vec::new(),
            wear_state: None,
            icon_image: 0,
            icon_layers: Vec::new(),
        }
    }
}

/// The install facts of the P1 panels (module docs).
pub struct CharacterData {
    store: &'static GameDataStore,
    strings: Arc<Strings>,
    engine: TooltipEngine<InstallTooltipData<'static>>,
    schema: ItemSchema,
    bags: BagLayout,
    base_backpack_slots: Option<u32>,
    paperdoll: PaperdollTable,
    rules: SlotRules,
    icon_pids: Vec<(&'static str, u32)>,
    /// Facts by record; `None` remembers a record that is absent or does not decode.
    facts: Mutex<HashMap<Did, Option<Arc<ItemFacts>>>>,
    /// The coin denominations' `Currency_Value`s, largest first (`economy.md` §7: the
    /// currency directory's three coins), when the install has exactly three.
    coin_values: Option<[u64; 3]>,
    /// The worth row's coin images, gold, silver, copper (`item-tooltips.md` §6.11 answer:
    /// the icon states of the examination coin element).
    coin_icons: [Option<Did>; 3],
}

/// The `Currency_Value`s of the currency directory's entries (`economy.md` §7), largest
/// first, when there are exactly three positive ones (gold, silver, copper).
fn coin_values(data: &dyn TooltipData) -> Option<[u64; 3]> {
    let dir = data.weenie("CurrencyDirectory")?;
    let dir = data.record(dir).ok()??;
    let list_pid = data.pid("Currency_CurrencyList")?;
    let value_pid = data.pid("Currency_Value")?;
    let PropertyValue::Array(list) = dir.get(list_pid)? else {
        return None;
    };
    let mut values: Vec<u64> = list
        .elements
        .iter()
        .take(16)
        .filter_map(|e| match e.value {
            PropertyValue::DidRef(d) => data.record(d).ok().flatten(),
            _ => None,
        })
        .filter_map(|c| match c.get(value_pid) {
            Some(PropertyValue::Int(v)) if *v > 0 => u64::try_from(*v).ok(),
            _ => None,
        })
        .collect();
    values.sort_unstable_by(|a, b| b.cmp(a));
    values.dedup();
    <[u64; 3]>::try_from(values).ok()
}

impl std::fmt::Debug for CharacterData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CharacterData")
            .field("bags", &self.bags)
            .field("paperdoll_slots", &self.paperdoll.slots.len())
            .finish_non_exhaustive()
    }
}

/// Errors of [`CharacterData::open`].
#[derive(Debug, thiserror::Error)]
pub enum CharacterDataError {
    /// The install's records cannot be opened.
    #[error("install: {0}")]
    Install(#[from] tessera_gamedata::GameDataError),
    /// The string tables cannot be opened.
    #[error("strings: {0}")]
    Strings(String),
    /// The tooltip engine's data cannot be read.
    #[error("tooltips: {0}")]
    Tooltips(#[from] tessera_gamedata::tooltip::TooltipError),
}

/// Renders a StringInfo to plain text with the locale's strings (no values); empty when it
/// does not render.
#[must_use]
pub fn render_plain(strings: &Strings, info: &StringInfo) -> String {
    strings
        .resolve(info, &Values::new(), &RenderOptions::default())
        .plain_text()
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl CharacterData {
    /// Opens the install at `dir`: its records, strings, bag layout, paperdoll table and
    /// tooltip engine. Missing `InventoryControl` values fall back to
    /// [`NO_INSTALL_BAGS`] (logged).
    ///
    /// # Errors
    ///
    /// [`CharacterDataError`] when the records, the strings or the property dictionary
    /// cannot be read.
    pub fn open(dir: &Path) -> Result<Self, CharacterDataError> {
        let store = GameDataStore::open(dir)?;
        let strings = Strings::open_install(dir, None)
            .map_err(|e| CharacterDataError::Strings(e.to_string()))?;
        // The named-table map (`string-table.md` §9.1) finds tables by STRINGTABLE key: the
        // P4 lines and labels (`chat-window.md` §15.10 CT12) need it.
        let strings = if strings.named_tables().tables().is_empty() {
            let named = store
                .object_store()
                .game_data()
                .did_map_by_label(STRING_TABLE_MAP_LABEL)
                .ok()
                .flatten()
                .map(|m| NamedTables::from_did_map(&m))
                .unwrap_or_default();
            strings.with_named_tables(named)
        } else {
            strings
        };
        Self::from_store(store, Arc::new(strings))
    }

    /// The facts over an opened store with `strings` (the store lives for the rest of the
    /// process, module docs).
    ///
    /// # Errors
    ///
    /// As [`CharacterData::open`].
    pub fn from_store(
        store: GameDataStore,
        strings: Arc<Strings>,
    ) -> Result<Self, CharacterDataError> {
        let store: &'static GameDataStore =
            Box::leak(Box::new(store.with_strings(Arc::clone(&strings))));
        let os = store.object_store();
        let control = load_inventory_control(os);
        let base_backpack_slots = control.as_ref().ok().and_then(|c| c.base_backpack_slots);
        let bags = match control.as_ref().ok().and_then(BagLayout::from_control) {
            Some(b) => b,
            None => {
                tracing::warn!(
                    error = ?control.err().map(|e| e.to_string()),
                    "InventoryControl values unavailable; one bag shows every slot"
                );
                NO_INSTALL_BAGS
            }
        };
        let paperdoll = load_paperdoll(os).unwrap_or_else(|e| {
            tracing::warn!("paperdoll slot table unavailable: {e}");
            PaperdollTable::default()
        });
        let flag = equipment_flag_key(os).ok().flatten();
        let rules = paperdoll.rules(flag);
        let registry = os.game_data().registry().map_err(|e| {
            CharacterDataError::Tooltips(tessera_gamedata::tooltip::TooltipError::DataUnavailable(
                e.to_string(),
            ))
        })?;
        let dict = registry.dictionary();
        let schema = ItemSchema::resolve(dict);
        let icon_pids = [
            tessera_material::icon::names::IMAGE_OVERRIDE_CHECKPOINTED,
            tessera_material::icon::names::IMAGE,
            tessera_material::icon::names::BACKGROUND,
            tessera_material::icon::names::UNDERLAY,
            tessera_material::icon::names::SHADOW,
            tessera_material::icon::names::OVERLAY,
        ]
        .into_iter()
        .filter_map(|n| Some((n, tessera_formats::PropertyNames::pid_of(dict, n)?)))
        .collect();
        let data = InstallTooltipData::new(store, Arc::clone(&strings))?;
        let coin_values = coin_values(&data);
        if coin_values.is_none() {
            tracing::info!("coin denominations unavailable; trade coin is shown unsplit");
        }
        let coin_icons = tessera_gamedata::tooltip::layout::coin_icon_images(&data);
        if coin_icons.iter().any(Option::is_none) {
            tracing::info!("worth-row coin images not all found; coins without one show none");
        }
        Ok(Self {
            store,
            strings,
            engine: TooltipEngine::new(data),
            schema,
            bags,
            base_backpack_slots,
            paperdoll,
            rules,
            icon_pids,
            facts: Mutex::new(HashMap::new()),
            coin_values,
            coin_icons,
        })
    }

    /// The coin denominations' values, largest first (gold, silver, copper; `economy.md`
    /// §7), when the install has them.
    #[must_use]
    pub fn coin_values(&self) -> Option<[u64; 3]> {
        self.coin_values
    }

    /// The worth row's coin images, gold, silver, copper (`item-tooltips.md` §6.11), each
    /// when the install's money-display layout names it.
    #[must_use]
    pub fn coin_icons(&self) -> [Option<Did>; 3] {
        self.coin_icons
    }

    /// The install's record store.
    #[must_use]
    pub fn store(&self) -> &'static GameDataStore {
        self.store
    }

    /// The locale's strings.
    #[must_use]
    pub fn strings(&self) -> &Arc<Strings> {
        &self.strings
    }

    /// The item tooltip engine.
    #[must_use]
    pub fn engine(&self) -> &TooltipEngine<InstallTooltipData<'static>> {
        &self.engine
    }

    /// The bag layout (`character-panels.md` §4.3).
    #[must_use]
    pub fn bags(&self) -> BagLayout {
        self.bags
    }

    /// `InventoryControl_BaseBackpackSlots`, when the install has it.
    #[must_use]
    pub fn base_backpack_slots(&self) -> Option<u32> {
        self.base_backpack_slots
    }

    /// The paperdoll slot table (§3.3).
    #[must_use]
    pub fn paperdoll(&self) -> &PaperdollTable {
        &self.paperdoll
    }

    /// The slot rules of §6.
    #[must_use]
    pub fn rules(&self) -> &SlotRules {
        &self.rules
    }

    /// The PID of a property-dictionary name.
    #[must_use]
    pub fn pid(&self, name: &str) -> Option<u32> {
        tessera_gamedata::tooltip::TooltipData::pid(self.engine.data(), name)
    }

    /// The item schema of the install's dictionary.
    #[must_use]
    pub fn schema(&self) -> &ItemSchema {
        &self.schema
    }

    /// The facts of item record `did` (cached, also when the record is absent or does not
    /// decode: `None`, so a missing record is read once).
    #[must_use]
    pub fn item(&self, did: Did) -> Option<Arc<ItemFacts>> {
        if let Some(f) = lock(&self.facts).get(&did) {
            return f.clone();
        }
        let facts = match self.store.object(did) {
            Ok(Some(r)) => Some(Arc::new(self.facts_of(did, &r.properties))),
            Ok(None) => None,
            Err(e) => {
                tracing::debug!("item {did}: {e}");
                None
            }
        };
        let mut cache = lock(&self.facts);
        if cache.len() >= MAX_CACHED_FACTS {
            cache.clear();
        }
        cache.insert(did, facts.clone());
        facts
    }

    /// The PID of icon property `name` (one of the item layer names).
    #[must_use]
    pub fn icon_pid(&self, name: &str) -> Option<u32> {
        self.icon_pids
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, p)| *p)
    }

    /// "Read *P*" of `icon-layers.md` §3.1 on a record's properties.
    fn did_of(&self, set: &PropertySet, name: &str) -> Option<Did> {
        read_icon_property(set.get(self.icon_pid(name)?))
    }

    /// The item icon of record `did` as an item instance with `instance` properties shows
    /// it (`icon-layers.md` §3.1 step 1): the instance's
    /// `Icon_Layer_ImageOverrideDID_Checkpointed` when it carries one, else the record's
    /// `Icon_Layer_ImageDID`; the other layers from the record. `None` without an icon or
    /// without the record.
    #[must_use]
    pub fn instance_icon(
        &self,
        did: Did,
        instance: &dyn Fn(u32) -> Option<Did>,
    ) -> Option<IconImage> {
        let facts = self.item(did)?;
        let override_pid =
            self.icon_pid(tessera_material::icon::names::IMAGE_OVERRIDE_CHECKPOINTED);
        let Some(over) = override_pid.and_then(instance) else {
            return facts.icon.clone();
        };
        let stack = IconStack::item(|n| {
            if n == tessera_material::icon::names::IMAGE_OVERRIDE_CHECKPOINTED {
                Some(over)
            } else {
                facts
                    .icon_layers
                    .iter()
                    .find(|(k, _)| *k == n)
                    .and_then(|(_, d)| *d)
            }
        });
        icon_of(&stack)
    }

    /// The facts of a record's property set.
    fn facts_of(&self, did: Did, set: &PropertySet) -> ItemFacts {
        let view = ItemView::new(set, &self.schema);
        let text = |info: Option<&StringInfo>| info.map(|i| render_plain(&self.strings, i));
        let name = text(view.name()).unwrap_or_default();
        let plural = text(view.plural_name()).filter(|t| !t.is_empty());
        // Static item data: the instance override is not read (§3.1 step 1).
        let stack = IconStack::item(|n| {
            if n == tessera_material::icon::names::IMAGE_OVERRIDE_CHECKPOINTED {
                None
            } else {
                self.did_of(set, n)
            }
        });
        let icon = icon_of(&stack);
        let image = |n: &str| self.did_of(set, n).map_or(0, |d| d.0);
        let requirements = view.requirements();
        ItemFacts {
            info: Arc::new(ItemInfoData {
                record: Some(did.0),
                name,
                description: text(view.description()).unwrap_or_default(),
                plural_name: plural,
                category: view.item_class().unwrap_or(0),
                durability: view.durability().durability_key.unwrap_or(0),
                quality: view.quality().unwrap_or(0),
                max_stack_size: view
                    .max_quantity()
                    .and_then(|q| u32::try_from(q).ok())
                    .filter(|q| *q > 0),
                unique: view.unique().unwrap_or(false),
                icon_image_id: image(tessera_material::icon::names::IMAGE),
                background_image_id: image(tessera_material::icon::names::BACKGROUND),
                underlay_image_id: image(tessera_material::icon::names::UNDERLAY),
                shadow_image_id: image(tessera_material::icon::names::SHADOW),
            }),
            icon,
            slots: view.slots(),
            min_level: requirements.min_level,
            required_classes: requirements.classes,
            wear_state: view.wear_state(),
            icon_image: image(tessera_material::icon::names::IMAGE),
            icon_layers: [
                tessera_material::icon::names::IMAGE,
                tessera_material::icon::names::BACKGROUND,
                tessera_material::icon::names::UNDERLAY,
                tessera_material::icon::names::SHADOW,
                tessera_material::icon::names::OVERLAY,
            ]
            .into_iter()
            .map(|n| (n, self.did_of(set, n)))
            .collect(),
        }
    }
}

/// "Read *P*" of `icon-layers.md` §3.1: absent → `None`; a DID → that DID (even 0); any
/// other type → `Some(Did(0))`.
#[must_use]
pub fn read_icon_property(v: Option<&PropertyValue>) -> Option<Did> {
    match v? {
        PropertyValue::DidRef(d) => Some(*d),
        _ => Some(Did(0)),
    }
}

/// The composed icon of a stack (`None` when it is empty: no icon).
fn icon_of(stack: &IconStack) -> Option<IconImage> {
    (!stack.is_empty()).then(|| {
        IconImage::Layers(
            stack
                .entries()
                .iter()
                .map(|e| ImageRef::Did(e.did.0))
                .collect(),
        )
    })
}

/// The bag layout of a run: the install's, or [`NO_INSTALL_BAGS`] without one.
#[must_use]
pub fn bags_of(data: Option<&CharacterData>) -> BagLayout {
    data.map_or(NO_INSTALL_BAGS, CharacterData::bags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_properties_read_as_icon_layers_md_says() {
        // §3.1 "Read P": absent → absent; a DID → the DID (even 0); another type → 0.
        assert_eq!(read_icon_property(None), None);
        assert_eq!(
            read_icon_property(Some(&PropertyValue::DidRef(Did(0)))),
            Some(Did(0))
        );
        assert_eq!(
            read_icon_property(Some(&PropertyValue::DidRef(Did(0x4100_0001)))),
            Some(Did(0x4100_0001))
        );
        assert_eq!(
            read_icon_property(Some(&PropertyValue::Int(7))),
            Some(Did(0))
        );
        // A present Image of 0 is skipped, the other layers still compose (§3.1 step 1).
        let stack = IconStack::item(|n| match n {
            tessera_material::icon::names::IMAGE => Some(Did(0)),
            tessera_material::icon::names::BACKGROUND => Some(Did(0x4100_0002)),
            _ => None,
        });
        assert!(icon_of(&stack).is_some());
    }
}
