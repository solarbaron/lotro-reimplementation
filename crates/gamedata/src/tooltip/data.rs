// SPDX-License-Identifier: GPL-3.0-or-later
//! What the tooltip model reads: the [`TooltipData`] trait and its implementation over an
//! install ([`InstallTooltipData`]). The model itself never touches archives, so tests and
//! other backends can supply the same facts from memory.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use tessera_formats::gameplay::items::{ItemSchema, ItemSetView, set_directory};
use tessera_formats::ui_layout::{UiEnum, UiLabels};
use tessera_formats::{
    DataIdMap, Did, EnumMapper, PropertySet, PropertyType, PropertyValue, StringInfo, StringSource,
    UiLayout,
};
use tessera_props::{PropertyRegistry, RecordSource};
use tessera_text::{
    NamedTables, RenderOptions, RichText, STRING_TABLE_MAP_LABEL, Strings, Value, Values,
};

use crate::store::{GameDataStore, InstallSource};

use super::error::TooltipError;

/// The result of rendering a text: `Ok(Some(text))`, `Ok(None)` when the message does not
/// render because a slot has no value (`string-table.md` §7.1, G25) or the result is
/// empty, and `Err(reason)` when the text cannot be found or read (a lookup miss).
pub type TextResult = Result<Option<RichText>, String>;

/// A text to render: a StringInfo, or a token of a named string table given by its key in
/// the named-string-table DID map (`string-table.md` §9.1).
#[derive(Debug, Clone, Copy)]
pub enum TextRef<'a> {
    /// A StringInfo (literal or table reference, with its own arguments).
    Info(&'a StringInfo),
    /// A token of the named table with this key.
    Token {
        /// Key of the table in the named-string-table DID map.
        table_key: u32,
        /// The token.
        token: u32,
    },
}

/// The facts the tooltip model reads (`item-tooltips.md` §3–§8). Every method is a
/// lookup; none may panic on missing or malformed data.
pub trait TooltipData: Send + Sync {
    /// The PID of a property-dictionary name.
    fn pid(&self, name: &str) -> Option<u32>;
    /// The dictionary name of a PID.
    fn property_name(&self, pid: u32) -> Option<String>;
    /// The value type of a PID.
    fn property_type(&self, pid: u32) -> Option<PropertyType>;
    /// The dictionary default of a PID (`property-system.md` §2.2), if it has one.
    fn default_value(&self, pid: u32) -> Option<PropertyValue>;
    /// The display text of `key` in the enum mapper of `pid` (and its base chain);
    /// `None` when there is no display text.
    fn enum_text(&self, pid: u32, key: u32) -> Option<String>;
    /// The key named `name` in the enum mapper of `pid` (and its base chain).
    fn enum_key(&self, pid: u32, name: &str) -> Option<u32>;
    /// An object record's property set; `Ok(None)` when absent.
    fn record(&self, did: Did) -> Result<Option<Arc<PropertySet>>, String>;
    /// The object named `label` in the WEENIECONTENT DID map.
    fn weenie(&self, label: &str) -> Option<Did>;
    /// A UI layout record; `Ok(None)` when absent, `Err` when it does not decode.
    fn layout(&self, did: Did) -> Result<Option<Arc<UiLayout>>, String>;
    /// The layout the `UILAYOUT` DID map names `label`.
    fn layout_by_label(&self, label: &str) -> Option<Did>;
    /// The label of a UI enum key (element IDs, state IDs, element types).
    fn ui_label(&self, which: UiEnum, key: u32) -> Option<String>;
    /// The key of a UI enum label.
    fn ui_key(&self, which: UiEnum, name: &str) -> Option<u32>;
    /// A StringInfo as a nested message value for another message's slot.
    fn text_value(&self, info: &StringInfo) -> Result<Value, String>;
    /// Renders a text with `values` (see [`TextResult`]).
    fn render(&self, text: TextRef<'_>, values: &Values) -> TextResult;
    /// The item set whose member list names `item` (`items.md` §9.9 steps 1–2).
    fn set_of(&self, item: Did) -> Option<Did>;

    /// The root class of an object record (`combat-data.md` §2.1); `None` when unknown. The
    /// default knows none: effect lines then use the base composition
    /// (`skill-tooltips.md` §9.2) and progressions are read as not nested.
    fn root_class(&self, did: Did) -> Option<u32> {
        let _ = did;
        None
    }

    /// `class` and its ancestors, nearest first (the class library's parent map). The default
    /// knows no parents.
    fn class_lineage(&self, class: u32) -> Vec<u32> {
        vec![class]
    }

    /// The internal name (label) of `key` in the enum mapper of `pid` (and its base chain);
    /// not the display text ([`TooltipData::enum_text`]). The default knows none.
    fn enum_label(&self, pid: u32, key: u32) -> Option<String> {
        let _ = (pid, key);
        None
    }

    /// The property set named `label` in the `UNIQUEDB` DID map (`property-system.md` §8.1).
    /// The default knows none.
    fn unique(&self, label: &str) -> Option<Did> {
        let _ = label;
        None
    }

    /// Any property set by DID: an object record's set, or a bare property set (prefixes
    /// 0x47, 0x78, 0x79). The default reads object records only.
    fn property_set(&self, did: Did) -> Result<Option<Arc<PropertySet>>, String> {
        self.record(did)
    }

    /// The dictionary's properties whose names start with `prefix`, as (PID, name), at most
    /// [`MAX_PREFIX_MATCHES`]. The default knows none.
    fn properties_with_prefix(&self, prefix: &str) -> Vec<(u32, String)> {
        let _ = prefix;
        Vec::new()
    }
}

macro_rules! delegate_tooltip_data {
    ($t:ty) => {
        fn pid(&self, name: &str) -> Option<u32> {
            (**self).pid(name)
        }
        fn property_name(&self, pid: u32) -> Option<String> {
            (**self).property_name(pid)
        }
        fn property_type(&self, pid: u32) -> Option<PropertyType> {
            (**self).property_type(pid)
        }
        fn default_value(&self, pid: u32) -> Option<PropertyValue> {
            (**self).default_value(pid)
        }
        fn enum_text(&self, pid: u32, key: u32) -> Option<String> {
            (**self).enum_text(pid, key)
        }
        fn enum_key(&self, pid: u32, name: &str) -> Option<u32> {
            (**self).enum_key(pid, name)
        }
        fn record(&self, did: Did) -> Result<Option<Arc<PropertySet>>, String> {
            (**self).record(did)
        }
        fn weenie(&self, label: &str) -> Option<Did> {
            (**self).weenie(label)
        }
        fn layout(&self, did: Did) -> Result<Option<Arc<UiLayout>>, String> {
            (**self).layout(did)
        }
        fn layout_by_label(&self, label: &str) -> Option<Did> {
            (**self).layout_by_label(label)
        }
        fn ui_label(&self, which: UiEnum, key: u32) -> Option<String> {
            (**self).ui_label(which, key)
        }
        fn ui_key(&self, which: UiEnum, name: &str) -> Option<u32> {
            (**self).ui_key(which, name)
        }
        fn text_value(&self, info: &StringInfo) -> Result<Value, String> {
            (**self).text_value(info)
        }
        fn render(&self, text: TextRef<'_>, values: &Values) -> TextResult {
            (**self).render(text, values)
        }
        fn set_of(&self, item: Did) -> Option<Did> {
            (**self).set_of(item)
        }
        fn root_class(&self, did: Did) -> Option<u32> {
            (**self).root_class(did)
        }
        fn class_lineage(&self, class: u32) -> Vec<u32> {
            (**self).class_lineage(class)
        }
        fn enum_label(&self, pid: u32, key: u32) -> Option<String> {
            (**self).enum_label(pid, key)
        }
        fn unique(&self, label: &str) -> Option<Did> {
            (**self).unique(label)
        }
        fn property_set(&self, did: Did) -> Result<Option<Arc<PropertySet>>, String> {
            (**self).property_set(did)
        }
        fn properties_with_prefix(&self, prefix: &str) -> Vec<(u32, String)> {
            (**self).properties_with_prefix(prefix)
        }
    };
}

/// A shared reference reads through to the data, so one [`InstallTooltipData`] can serve a
/// tooltip engine and the combat data at once.
impl<T: TooltipData + ?Sized> TooltipData for &T {
    delegate_tooltip_data!(&T);
}

impl<T: TooltipData + ?Sized> TooltipData for Arc<T> {
    delegate_tooltip_data!(Arc<T>);
}

/// Most matches [`TooltipData::properties_with_prefix`] returns.
pub const MAX_PREFIX_MATCHES: usize = 256;

/// Most entries kept in each of the per-record caches of [`InstallTooltipData`] before
/// the cache is cleared (bounded memory over a pass over every item).
pub const MAX_CACHED: usize = 32_768;

/// Root-map label of the layout DID map (`ui-layout.md` §3.2).
const UILAYOUT_LABEL: &str = "UILAYOUT";
/// Root-map label of the enum-mapper DID map (`ui-layout.md` §3.3).
const EMAPPER_LABEL: &str = "EMAPPER";
/// Root-map label of the unique-object DID map (`property-system.md` §8.1).
const UNIQUEDB_LABEL: &str = "UNIQUEDB";
/// WEENIECONTENT label of the set directory (`items.md` §9.9).
const SET_DIRECTORY_LABEL: &str = "GameSetDirectory";

type LayoutSlot = Result<Option<Arc<UiLayout>>, String>;
/// A cached object record: root class and property set.
type ObjectSlot = (u32, Arc<PropertySet>);

/// [`TooltipData`] over an install: the store's records and dictionary, the UI layouts and
/// labels, and the strings of one locale.
pub struct InstallTooltipData<'a, S: RecordSource = InstallSource> {
    store: &'a GameDataStore<S>,
    strings: Arc<Strings>,
    registry: Arc<PropertyRegistry>,
    schema: ItemSchema,
    labels: UiLabels,
    uilayout: Option<Arc<DataIdMap>>,
    named: NamedTables,
    layouts: Mutex<HashMap<Did, LayoutSlot>>,
    records: Mutex<HashMap<Did, Option<ObjectSlot>>>,
    bare_sets: Mutex<HashMap<Did, Option<Arc<PropertySet>>>>,
    enum_texts: Mutex<HashMap<(u32, u32), Option<String>>>,
    sets: OnceLock<HashMap<Did, Did>>,
}

impl<S: RecordSource> std::fmt::Debug for InstallTooltipData<'_, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallTooltipData")
            .field("named_tables", &self.named.tables().len())
            .field("uilayout", &self.uilayout.is_some())
            .finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl<'a, S: RecordSource> InstallTooltipData<'a, S> {
    /// Reads the dictionary, the UI enum labels, the layout map and the named string tables
    /// of the install behind `store`; `strings` gives the locale's texts.
    ///
    /// # Errors
    ///
    /// [`TooltipError::DataUnavailable`] when the property dictionary cannot be read:
    /// nothing can be decoded without it.
    pub fn new(store: &'a GameDataStore<S>, strings: Arc<Strings>) -> Result<Self, TooltipError> {
        let gd = store.object_store().game_data();
        let registry = gd
            .registry()
            .map_err(|e| TooltipError::DataUnavailable(format!("property dictionary: {e}")))?
            .clone();
        let labels = UiLabels::load(
            |l| gd.lookup_path(&[EMAPPER_LABEL, l]).ok().flatten(),
            |d| gd.enum_mapper(d).ok().flatten().map(|m| (*m).clone()),
        );
        let uilayout = gd.did_map_by_label(UILAYOUT_LABEL).ok().flatten();
        let named = if strings.named_tables().tables().is_empty() {
            gd.did_map_by_label(STRING_TABLE_MAP_LABEL)
                .ok()
                .flatten()
                .map(|m| NamedTables::from_did_map(&m))
                .unwrap_or_default()
        } else {
            strings.named_tables().clone()
        };
        let schema = ItemSchema::resolve(registry.dictionary());
        Ok(Self {
            store,
            strings,
            registry,
            schema,
            labels,
            uilayout,
            named,
            layouts: Mutex::new(HashMap::new()),
            records: Mutex::new(HashMap::new()),
            bare_sets: Mutex::new(HashMap::new()),
            enum_texts: Mutex::new(HashMap::new()),
            sets: OnceLock::new(),
        })
    }

    /// The strings in use.
    #[must_use]
    pub fn strings(&self) -> &Arc<Strings> {
        &self.strings
    }

    fn mapper(&self, did: Did) -> Option<Arc<EnumMapper>> {
        self.store
            .object_store()
            .game_data()
            .enum_mapper(did)
            .ok()
            .flatten()
    }

    fn build_sets(&self) -> HashMap<Did, Did> {
        let mut out = HashMap::new();
        let Some(dir_did) = self.weenie(SET_DIRECTORY_LABEL) else {
            return out;
        };
        let Ok(Some(dir)) = self.record(dir_did) else {
            return out;
        };
        for set_did in set_directory(&*dir, &self.schema.sets) {
            let Ok(Some(set)) = self.record(set_did) else {
                continue;
            };
            for member in ItemSetView::new(&set, &self.schema).members() {
                out.entry(member).or_insert(set_did);
            }
        }
        out
    }

    /// An object record's root class and property set, cached.
    fn object(&self, did: Did) -> Result<Option<ObjectSlot>, String> {
        if let Some(r) = lock(&self.records).get(&did) {
            return Ok(r.clone());
        }
        if !tessera_formats::gameplay::common::is_object_did(did) {
            return Ok(None);
        }
        let rec = self
            .store
            .object(did)
            .map_err(|e| e.to_string())?
            .map(|r| (r.root.root_class, Arc::new(r.properties)));
        let mut cache = lock(&self.records);
        if cache.len() >= MAX_CACHED {
            cache.clear();
        }
        cache.insert(did, rec.clone());
        Ok(rec)
    }

    fn table_did(&self, key: u32) -> Option<u32> {
        self.named.by_key(key)
    }

    /// G25: every slot of the message must have a value (or a StringInfo argument).
    fn all_slots_filled(
        msg: &tessera_text::Message,
        values: &Values,
        info: Option<&StringInfo>,
    ) -> bool {
        msg.arg_ids().iter().all(|id| {
            let in_values = values
                .get(*id)
                .is_some_and(|v| !matches!(v, Value::Unresolved(_)));
            let in_info = info
                .and_then(|i| i.args.as_ref())
                .is_some_and(|a| a.values.iter().any(|v| v.var_id == *id));
            in_values || in_info
        })
    }
}

fn finish(rendered: tessera_text::Rendered) -> TextResult {
    if let Some(miss) = rendered.miss() {
        return Err(miss.to_string());
    }
    if rendered.text.plain_text().trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(rendered.text))
}

impl<S: RecordSource> TooltipData for InstallTooltipData<'_, S> {
    fn pid(&self, name: &str) -> Option<u32> {
        self.registry.pid(name)
    }

    fn property_name(&self, pid: u32) -> Option<String> {
        self.registry.name(pid).map(str::to_owned)
    }

    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        self.registry.property_type(pid)
    }

    fn default_value(&self, pid: u32) -> Option<PropertyValue> {
        self.registry.definition(pid)?.default.clone()
    }

    fn enum_text(&self, pid: u32, key: u32) -> Option<String> {
        if let Some(t) = lock(&self.enum_texts).get(&(pid, key)) {
            return t.clone();
        }
        let text = (|| {
            let mapper = self.registry.enum_source(pid)?.did();
            let label = self
                .store
                .object_store()
                .game_data()
                .enum_label(mapper, key)
                .ok()?;
            let display = label.display?;
            let r = self
                .strings
                .resolve(&display, &Values::new(), &RenderOptions::default());
            if r.miss().is_some() {
                return None;
            }
            let t = r.plain_text();
            let t = t.trim();
            (!t.is_empty()).then(|| t.to_owned())
        })();
        let mut cache = lock(&self.enum_texts);
        if cache.len() >= MAX_CACHED {
            cache.clear();
        }
        cache.insert((pid, key), text.clone());
        text
    }

    fn enum_key(&self, pid: u32, name: &str) -> Option<u32> {
        let mapper = self.registry.enum_source(pid)?.did();
        EnumMapper::key_in_chain_from(mapper, |d| self.mapper(d), name)
    }

    fn record(&self, did: Did) -> Result<Option<Arc<PropertySet>>, String> {
        Ok(self.object(did)?.map(|(_, s)| s))
    }

    fn root_class(&self, did: Did) -> Option<u32> {
        self.object(did).ok().flatten().map(|(c, _)| c)
    }

    fn class_lineage(&self, class: u32) -> Vec<u32> {
        match self.store.class_library() {
            Ok(lib) => {
                use tessera_formats::gameplay::common::ClassParents;
                lib.lineage(class).take(64).collect()
            }
            Err(_) => vec![class],
        }
    }

    fn enum_label(&self, pid: u32, key: u32) -> Option<String> {
        let mapper = self.registry.enum_source(pid)?.did();
        let label = self
            .store
            .object_store()
            .game_data()
            .enum_label(mapper, key)
            .ok()?;
        label.name.map(|n| n.to_string())
    }

    fn unique(&self, label: &str) -> Option<Did> {
        self.store
            .object_store()
            .game_data()
            .lookup_path(&[UNIQUEDB_LABEL, label])
            .ok()
            .flatten()
            .filter(|d| !d.is_null())
    }

    fn property_set(&self, did: Did) -> Result<Option<Arc<PropertySet>>, String> {
        if tessera_formats::gameplay::common::is_object_did(did) {
            return self.record(did);
        }
        if let Some(s) = lock(&self.bare_sets).get(&did) {
            return Ok(s.clone());
        }
        if !tessera_formats::is_property_set_did(did) {
            return Ok(None);
        }
        let set = match self.store.object_store().game_data().read(did) {
            Ok(None) => None,
            Ok(Some(bytes)) => Some(Arc::new(
                tessera_formats::PropertySetRecord::parse_prefix(&bytes, self.registry.as_ref())
                    .map_err(|e| e.to_string())?
                    .0
                    .set,
            )),
            Err(e) => return Err(e.to_string()),
        };
        let mut cache = lock(&self.bare_sets);
        if cache.len() >= MAX_CACHED {
            cache.clear();
        }
        cache.insert(did, set.clone());
        Ok(set)
    }

    fn properties_with_prefix(&self, prefix: &str) -> Vec<(u32, String)> {
        self.registry
            .iter()
            .filter_map(|(d, n)| {
                n.filter(|n| n.starts_with(prefix))
                    .map(|n| (d.pid, n.to_owned()))
            })
            .take(MAX_PREFIX_MATCHES)
            .collect()
    }

    fn weenie(&self, label: &str) -> Option<Did> {
        self.store.object_store().weenie(label).ok().flatten()
    }

    fn layout(&self, did: Did) -> Result<Option<Arc<UiLayout>>, String> {
        if let Some(l) = lock(&self.layouts).get(&did) {
            return l.clone();
        }
        let gd = self.store.object_store().game_data();
        let loaded: LayoutSlot = match gd.read(did) {
            Ok(None) => Ok(None),
            Ok(Some(bytes)) => UiLayout::parse_expecting(&bytes, did, self.registry.as_ref())
                .map(|l| Some(Arc::new(l)))
                .map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        // Install layouts are few (hundreds), but requests can name any DID (override pairs,
        // instance properties): bounded like the other caches.
        let mut cache = lock(&self.layouts);
        if cache.len() >= MAX_CACHED {
            cache.clear();
        }
        cache.insert(did, loaded.clone());
        loaded
    }

    fn layout_by_label(&self, label: &str) -> Option<Did> {
        self.uilayout.as_ref()?.by_label(label)
    }

    fn ui_label(&self, which: UiEnum, key: u32) -> Option<String> {
        self.labels.label(which, key).map(str::to_owned)
    }

    fn ui_key(&self, which: UiEnum, name: &str) -> Option<u32> {
        self.labels.key(which, name)
    }

    fn text_value(&self, info: &StringInfo) -> Result<Value, String> {
        self.strings.to_value(info).map_err(|m| m.to_string())
    }

    fn render(&self, text: TextRef<'_>, values: &Values) -> TextResult {
        let opts = RenderOptions::default();
        match text {
            TextRef::Info(info) => {
                if let StringSource::Table { token, table } = &info.source {
                    let msg = self
                        .strings
                        .message(table.0, *token)
                        .map_err(|m| m.to_string())?;
                    if !Self::all_slots_filled(&msg, values, Some(info)) {
                        return Ok(None);
                    }
                }
                finish(self.strings.resolve(info, values, &opts))
            }
            TextRef::Token { table_key, token } => {
                let table = self
                    .table_did(table_key)
                    .ok_or_else(|| format!("no named string table with key {table_key:#010x}"))?;
                let msg = self
                    .strings
                    .message(table, token)
                    .map_err(|m| m.to_string())?;
                if !Self::all_slots_filled(&msg, values, None) {
                    return Ok(None);
                }
                finish(self.strings.render_entry(table, token, values, &opts))
            }
        }
    }

    fn set_of(&self, item: Did) -> Option<Did> {
        self.sets
            .get_or_init(|| self.build_sets())
            .get(&item)
            .copied()
    }
}
