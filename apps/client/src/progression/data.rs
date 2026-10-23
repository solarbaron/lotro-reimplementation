// SPDX-License-Identifier: GPL-3.0-or-later
//! The install facts the P6 panels read ([`P6Data`]): `tessera-gamedata`'s progression and
//! crafting presenters over the P3 presentation data ([`NpcData`]), or over [`NoData`]
//! when there is no install (synthetic scenarios, `--no-data`), plus the literal recipe
//! definitions a caller registers (`crafting-panels.md` §15.4 OPEN: the feed carries no
//! ingredients for a literal recipe) and the `PROFESSIONS` and `VOCATIONS` DID-map keys of
//! the plugin views (`lua-plugin-api.md` §15.6 P113, read by label at run time).

use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::ui_layout::UiEnum;
use tessera_formats::{
    DataIdMap, Did, PropertySet, PropertyType, PropertyValue, StringInfo, UiLayout,
};
use tessera_gamedata::crafting::{CraftingPresenter, RecipeDef};
use tessera_gamedata::npc::PresentationData;
use tessera_gamedata::progression::ProgressionPresenter;
use tessera_gamedata::quest::RecordRef;
use tessera_gamedata::tooltip::{TextRef, TextResult, TooltipData};
use tessera_text::{Value, Values};

use crate::npc::NpcData;

/// Presentation data with no install: every lookup misses. The presenters over it show
/// literal refs and the literal recipe definitions only.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoData;

impl TooltipData for NoData {
    fn pid(&self, _: &str) -> Option<u32> {
        None
    }
    fn property_name(&self, _: u32) -> Option<String> {
        None
    }
    fn property_type(&self, _: u32) -> Option<PropertyType> {
        None
    }
    fn default_value(&self, _: u32) -> Option<PropertyValue> {
        None
    }
    fn enum_text(&self, _: u32, _: u32) -> Option<String> {
        None
    }
    fn enum_key(&self, _: u32, _: &str) -> Option<u32> {
        None
    }
    fn record(&self, _: Did) -> Result<Option<Arc<PropertySet>>, String> {
        Ok(None)
    }
    fn weenie(&self, _: &str) -> Option<Did> {
        None
    }
    fn layout(&self, _: Did) -> Result<Option<Arc<UiLayout>>, String> {
        Ok(None)
    }
    fn layout_by_label(&self, _: &str) -> Option<Did> {
        None
    }
    fn ui_label(&self, _: UiEnum, _: u32) -> Option<String> {
        None
    }
    fn ui_key(&self, _: UiEnum, _: &str) -> Option<u32> {
        None
    }
    fn text_value(&self, _: &StringInfo) -> Result<Value, String> {
        Err("no install".into())
    }
    fn render(&self, _: TextRef<'_>, _: &Values) -> TextResult {
        Err("no install".into())
    }
    fn set_of(&self, _: Did) -> Option<Did> {
        None
    }
}

impl PresentationData for NoData {
    fn table(&self, _: &str) -> Option<Did> {
        None
    }
    fn text_args(&self, _: TextRef<'_>) -> Result<Vec<u32>, String> {
        Err("no install".into())
    }
    fn enum_name(&self, _: u32, _: u32) -> Option<String> {
        None
    }
}

static NO_DATA: NoData = NoData;

/// The DID-map keys of the plugin views (`lua-plugin-api.md` §15.6 P113): profession and
/// vocation record DID → the API's `Profession` and `Vocation` value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApiKeys {
    /// `PROFESSIONS`: profession DID → key.
    pub professions: HashMap<u32, u32>,
    /// `VOCATIONS`: vocation DID → key.
    pub vocations: HashMap<u32, u32>,
}

impl ApiKeys {
    /// Most entries read from one map (Tessera bound; the install has 10 and 7).
    pub const MAX: usize = 1024;

    /// Reads the two maps through `read` (a DID map by DID) from the root DID map's labels
    /// `PROFESSIONS` and `VOCATIONS`; a map that does not resolve stays empty.
    pub fn read(root: &DataIdMap, read: &dyn Fn(Did) -> Option<DataIdMap>) -> Self {
        let keys = |label: &str| -> HashMap<u32, u32> {
            root.by_label(label)
                .and_then(read)
                .map(|m| {
                    m.entries()
                        .iter()
                        .filter(|e| e.key != 0 && !e.value.is_null())
                        .take(Self::MAX)
                        .map(|e| (e.value.0, e.key))
                        .collect()
                })
                .unwrap_or_default()
        };
        Self {
            professions: keys("PROFESSIONS"),
            vocations: keys("VOCATIONS"),
        }
    }

    /// The `Profession` value of a ref (`None` for a literal or a DID the map lacks).
    #[must_use]
    pub fn profession(&self, r: &RecordRef) -> Option<u32> {
        match r {
            RecordRef::Record(d) => self.professions.get(&d.0).copied(),
            RecordRef::Literal(_) => None,
        }
    }

    /// The `Vocation` value of a ref (0 for none, a literal or a DID the map lacks).
    #[must_use]
    pub fn vocation(&self, r: Option<&RecordRef>) -> u32 {
        match r {
            Some(RecordRef::Record(d)) => self.vocations.get(&d.0).copied().unwrap_or(0),
            _ => 0,
        }
    }
}

/// The P6 panels' install facts (module docs).
pub struct P6Data {
    npc: Option<Arc<NpcData>>,
    data: &'static dyn PresentationData,
    progression: ProgressionPresenter<'static>,
    crafting: CraftingPresenter<'static>,
    keys: ApiKeys,
}

impl std::fmt::Debug for P6Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("P6Data")
            .field("install", &self.npc.is_some())
            .finish_non_exhaustive()
    }
}

impl P6Data {
    /// No install: the presenters show literal refs and registered definitions only.
    #[must_use]
    pub fn fallback() -> Self {
        Self {
            npc: None,
            data: &NO_DATA,
            progression: ProgressionPresenter::new(&NO_DATA),
            crafting: CraftingPresenter::new(&NO_DATA),
            keys: ApiKeys::default(),
        }
    }

    /// The same facts with the plugin views' keys `keys` (tests and synthetic runs whose refs
    /// are records the caller maps itself).
    #[must_use]
    pub fn with_keys(mut self, keys: ApiKeys) -> Self {
        self.keys = keys;
        self
    }

    /// The presenters over the P3 facts of an install, with the plugin views' keys.
    #[must_use]
    pub fn install(npc: Arc<NpcData>, keys: ApiKeys) -> Self {
        let data = npc.data();
        Self {
            progression: ProgressionPresenter::new(data),
            crafting: CraftingPresenter::new(data),
            npc: Some(npc),
            data,
            keys,
        }
    }

    /// Reads the plugin views' keys from the install's root DID map (`records.md` §8.2).
    #[must_use]
    pub fn read_keys(npc: &NpcData) -> ApiKeys {
        let store = npc.character().store();
        let gd = store.object_store().game_data();
        let read =
            |d: Did| -> Option<DataIdMap> { gd.did_map(d).ok().flatten().map(|m| (*m).clone()) };
        match read(tessera_props::ROOT_DID_MAP) {
            Some(root) => ApiKeys::read(&root, &read),
            None => ApiKeys::default(),
        }
    }

    /// Whether the facts come from an install.
    #[must_use]
    pub fn has_install(&self) -> bool {
        self.npc.is_some()
    }

    /// The P3 facts, with an install.
    #[must_use]
    pub fn npc(&self) -> Option<&Arc<NpcData>> {
        self.npc.as_ref()
    }

    /// The presentation data (records, strings, labels; misses without an install).
    #[must_use]
    pub fn data(&self) -> &'static dyn PresentationData {
        self.data
    }

    /// The progression presenter (trait trees, trait tooltips, trainer rows).
    #[must_use]
    pub fn progression(&self) -> &ProgressionPresenter<'static> {
        &self.progression
    }

    /// The crafting presenter (recipe tree, recipe page, trainers, bars, result lines).
    #[must_use]
    pub fn crafting(&self) -> &CraftingPresenter<'static> {
        &self.crafting
    }

    /// The plugin views' keys.
    #[must_use]
    pub fn keys(&self) -> &ApiKeys {
        &self.keys
    }

    /// Registers a literal recipe definition (ingredients, results, one-use, cooldown) under
    /// the literal's text (`crafting-panels.md` §15.4 OPEN; tests and tools).
    pub fn define(&mut self, literal: &str, def: RecipeDef) {
        self.crafting.define(literal, def);
    }

    /// A record's rendered `Name` (or the literal's text).
    #[must_use]
    pub fn name_of(&self, r: &RecordRef) -> Option<String> {
        self.crafting.name_of(r)
    }
}
