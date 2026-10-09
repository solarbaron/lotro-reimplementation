// SPDX-License-Identifier: GPL-3.0-or-later
//! The user's data that skins are resolved against (`skin-definition.md` §5, §6.1, §6.2,
//! §8.4): the art-asset and element label chains, the base art table, the game skins and
//! the property IDs that panel overrides read and write.
//!
//! Everything is found at run time through labels: the root DID map label `EMAPPER` with
//! the entries `ArtAssetID` and `UIElementID` (§5.1), the root DID map label `UNIQUEDB`
//! with the entries `BaseSkinMappings` (§6.1) and `MonsterPlaySkin` (§6.2), and property
//! names through the property dictionary. No DID, enum key or property ID is hard-coded.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use tessera_formats::enum_mapper::ENUM_BASE_CHAIN_LIMIT;
use tessera_formats::{Did, EnumMapper};
use tessera_props::{GameData, PropertyRegistry, PropertySet, PropsError, RecordSource, Value};

/// Root DID map label of the enum mappers (§5.1).
pub const EMAPPER_LABEL: &str = "EMAPPER";
/// `EMAPPER` entry of the art-asset names (§5.1).
pub const ART_ASSET_LABEL: &str = "ArtAssetID";
/// `EMAPPER` entry of the element names (§5.1).
pub const ELEMENT_ID_LABEL: &str = "UIElementID";
/// Root DID map label of the unique database objects (§6.1).
pub const UNIQUEDB_LABEL: &str = "UNIQUEDB";
/// `UNIQUEDB` entry of the base art table (§6.1).
pub const BASE_SKIN_LABEL: &str = "BaseSkinMappings";
/// `UNIQUEDB` entries of the data-defined game skins known to this build (§6.2).
pub const GAME_SKIN_LABELS: &[&str] = &["MonsterPlaySkin"];

/// Property names used by the skin system (§6.1, §6.2, §8.4).
pub mod names {
    /// Array of mapping structs in the base table and in game skins.
    pub const MAPPING_ARRAY: &str = "UISkin_MappingArray";
    /// The struct type of each element of the mapping array (§11 SI3).
    pub const MAPPING_STRUCT: &str = "UISkin_MappingStruct";
    /// Art-asset field of a mapping struct.
    pub const ART_ASSET_FIELD: &str = "UISkin_UIArtAssetID";
    /// Render-surface field of a mapping struct.
    pub const DATA_ID_FIELD: &str = "UISkin_DataID";
    /// Skin ID of a game skin.
    pub const SKIN_ID: &str = "UISkin_SkinID";
    /// Position protection.
    pub const CANNOT_BE_MOVED: &str = "UISkin_CannotBeMoved";
    /// Size protection.
    pub const CANNOT_BE_RESIZED: &str = "UISkin_CannotBeResized";
    /// Edge-mode protection.
    pub const CANNOT_BE_DETACHED: &str = "UISkin_CannotBeDetached";
    /// UI-scaler origin, X.
    pub const SCALER_ORIGIN_X: &str = "UIComponent_UIScaler_Origin_X";
    /// UI-scaler origin, Y.
    pub const SCALER_ORIGIN_Y: &str = "UIComponent_UIScaler_Origin_Y";
}

/// Turns the symbolic names of a definition file into values (§5.2).
///
/// Implemented by [`SkinTables`]; tests and fuzz targets use [`LabelIndex`] pairs or their
/// own resolvers.
pub trait NameResolver {
    /// The art-asset value of an art-asset name (`Mapping` `ArtAssetID`), or `None`.
    fn art_asset(&self, name: &str) -> Option<u32>;
    /// The element ID value of an element name (`Element` `ID`), or `None`.
    fn element(&self, name: &str) -> Option<u32>;
}

/// Labels of one enum mapper chain, looked up ignoring ASCII case (§5.2).
///
/// Built nearest mapper first; within the chain the first label that folds to a given text
/// wins, which matches the original's search order (the mapper, then its base, …). A name
/// with a character outside ASCII never resolves (§5.2 step 1).
#[derive(Debug, Clone, Default)]
pub struct LabelIndex {
    by_name: HashMap<Box<str>, u32>,
    by_key: HashMap<u32, Box<str>>,
}

impl LabelIndex {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The labels of `chain` (nearest mapper first).
    pub fn from_chain<'a>(chain: impl IntoIterator<Item = &'a EnumMapper>) -> Self {
        let mut out = Self::new();
        for mapper in chain {
            for (key, name) in mapper.names() {
                out.insert(*key, name);
            }
        }
        out
    }

    /// Adds one label; an earlier label with the same folded text, or an earlier label of
    /// the same key, is kept.
    pub fn insert(&mut self, key: u32, label: &str) {
        if !label.is_ascii() {
            return;
        }
        if let Entry::Vacant(v) = self.by_name.entry(label.to_ascii_lowercase().into()) {
            v.insert(key);
        }
        self.by_key.entry(key).or_insert_with(|| label.into());
    }

    /// The key of `name`, ignoring ASCII case.
    #[must_use]
    pub fn key(&self, name: &str) -> Option<u32> {
        if !name.is_ascii() {
            return None;
        }
        self.by_name
            .get(name.to_ascii_lowercase().as_str())
            .copied()
    }

    /// The label of `key` (nearest mapper first).
    #[must_use]
    pub fn label(&self, key: u32) -> Option<&str> {
        self.by_key.get(&key).map(AsRef::as_ref)
    }

    /// Every (key, label) pair, nearest mapper's label per key, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &str)> + '_ {
        self.by_key.iter().map(|(&k, l)| (k, l.as_ref()))
    }

    /// Number of distinct folded labels.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// Whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

/// The base art table (§6.1): render-surface DID ↔ art-asset value, one-to-one in practice.
///
/// Keyed by DID as the original keys it; a repeated DID keeps its first entry. The reverse
/// direction (art-asset value → DID, first entry wins) serves tests and tools.
#[derive(Debug, Clone, Default)]
pub struct BaseTable {
    by_did: HashMap<u32, u32>,
    by_art: HashMap<u32, u32>,
}

impl BaseTable {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds (DID → art-asset value). Ignored, returning `false`, when the DID is 0, the
    /// art-asset value is 0, or the DID is already present (§6.1: first wins).
    pub fn insert(&mut self, did: u32, art_asset: u32) -> bool {
        if did == 0 || art_asset == 0 {
            return false;
        }
        match self.by_did.entry(did) {
            Entry::Occupied(_) => {
                tracing::debug!(
                    did = format_args!("{did:#010x}"),
                    "repeated DID in the base art table"
                );
                false
            }
            Entry::Vacant(v) => {
                v.insert(art_asset);
                self.by_art.entry(art_asset).or_insert(did);
                true
            }
        }
    }

    /// The art-asset value of a render-surface DID (§6.3 step 2).
    #[must_use]
    pub fn art_asset(&self, did: u32) -> Option<u32> {
        self.by_did.get(&did).copied()
    }

    /// The base DID of an art-asset value.
    #[must_use]
    pub fn did(&self, art_asset: u32) -> Option<u32> {
        self.by_art.get(&art_asset).copied()
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_did.len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_did.is_empty()
    }

    /// All (DID, art-asset value) pairs, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.by_did.iter().map(|(&d, &a)| (d, a))
    }
}

/// A data-defined game skin (§6.2): art-asset value → replacement DID.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameSkin {
    /// The `UNIQUEDB` label it was loaded from.
    pub label: String,
    /// Its skin ID (`UISkin_SkinID`), 0 when absent.
    pub skin_id: u32,
    map: HashMap<u32, u32>,
}

impl GameSkin {
    /// An empty game skin.
    #[must_use]
    pub fn new(label: impl Into<String>, skin_id: u32) -> Self {
        Self {
            label: label.into(),
            skin_id,
            map: HashMap::new(),
        }
    }

    /// Adds (art-asset value → replacement DID); the first mapping of a value wins and
    /// zero values are ignored.
    pub fn insert(&mut self, art_asset: u32, did: u32) -> bool {
        if art_asset == 0 || did == 0 {
            return false;
        }
        match self.map.entry(art_asset) {
            Entry::Occupied(_) => false,
            Entry::Vacant(v) => {
                v.insert(did);
                true
            }
        }
    }

    /// The replacement DID of an art-asset value.
    #[must_use]
    pub fn replacement(&self, art_asset: u32) -> Option<u32> {
        self.map.get(&art_asset).copied()
    }

    /// Number of mappings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether it has no mapping.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Property IDs that applying a panel override reads and writes (§8.4), resolved by name.
/// A `None` protection never protects; a `None` origin property is not written.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GeometryPids {
    /// `UISkin_CannotBeMoved`.
    pub cannot_be_moved: Option<u32>,
    /// `UISkin_CannotBeResized`.
    pub cannot_be_resized: Option<u32>,
    /// `UISkin_CannotBeDetached`.
    pub cannot_be_detached: Option<u32>,
    /// `UIComponent_UIScaler_Origin_X`.
    pub scaler_origin_x: Option<u32>,
    /// `UIComponent_UIScaler_Origin_Y`.
    pub scaler_origin_y: Option<u32>,
}

impl GeometryPids {
    /// Resolves the five names through the property dictionary.
    #[must_use]
    pub fn from_registry(registry: &PropertyRegistry) -> Self {
        Self {
            cannot_be_moved: registry.pid(names::CANNOT_BE_MOVED),
            cannot_be_resized: registry.pid(names::CANNOT_BE_RESIZED),
            cannot_be_detached: registry.pid(names::CANNOT_BE_DETACHED),
            scaler_origin_x: registry.pid(names::SCALER_ORIGIN_X),
            scaler_origin_y: registry.pid(names::SCALER_ORIGIN_Y),
        }
    }
}

/// Property IDs of the mapping records (§6.1, §6.2), resolved by name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MappingPids {
    /// `UISkin_MappingArray`.
    pub mapping_array: Option<u32>,
    /// `UISkin_UIArtAssetID`.
    pub art_asset: Option<u32>,
    /// `UISkin_DataID`.
    pub data_id: Option<u32>,
    /// `UISkin_SkinID`.
    pub skin_id: Option<u32>,
}

impl MappingPids {
    /// Resolves the four names through the property dictionary.
    #[must_use]
    pub fn from_registry(registry: &PropertyRegistry) -> Self {
        Self {
            mapping_array: registry.pid(names::MAPPING_ARRAY),
            art_asset: registry.pid(names::ART_ASSET_FIELD),
            data_id: registry.pid(names::DATA_ID_FIELD),
            skin_id: registry.pid(names::SKIN_ID),
        }
    }

    /// The (art-asset value, DID) pairs of a mapping record in stored order: the elements
    /// of the mapping array that are structs holding an enum art-asset value other than 0
    /// and a DID reference other than the null DID; other elements are skipped (§6.1).
    #[must_use]
    pub fn pairs(&self, set: &PropertySet) -> Vec<(u32, u32)> {
        let (Some(array), Some(art_pid), Some(did_pid)) =
            (self.mapping_array, self.art_asset, self.data_id)
        else {
            return Vec::new();
        };
        let Some(elements) = set.get(array).and_then(Value::as_array) else {
            return Vec::new();
        };
        elements
            .iter()
            .filter_map(|p| {
                let s = p.value.as_struct()?;
                let art = s.get(art_pid)?.as_enum().filter(|&a| a != 0)?;
                let did = s.get(did_pid)?.as_did().filter(|d| !d.is_null())?;
                Some((art, did.0))
            })
            .collect()
    }
}

/// Why the skin tables could not be loaded.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SkinDataError {
    /// The property layer failed (a record could not be read or decoded).
    #[error(transparent)]
    Props(#[from] PropsError),
    /// A labelled record the skin system needs is not in the data.
    #[error("{0} not found in the data")]
    Missing(&'static str),
}

/// Everything a skin is resolved against: label chains, base table, game skins and the
/// geometry property IDs.
#[derive(Debug, Clone, Default)]
pub struct SkinTables {
    /// Art-asset names (`EMAPPER` / `ArtAssetID` chain).
    pub art_assets: LabelIndex,
    /// Element names (`EMAPPER` / `UIElementID` chain).
    pub elements: LabelIndex,
    /// The base art table (§6.1).
    pub base: BaseTable,
    /// Game skins (§6.2), decoded but never activated by the offline client (D13).
    pub game_skins: Vec<GameSkin>,
    /// Property IDs for §8.4.
    pub geometry: GeometryPids,
}

impl NameResolver for SkinTables {
    fn art_asset(&self, name: &str) -> Option<u32> {
        self.art_assets.key(name)
    }

    fn element(&self, name: &str) -> Option<u32> {
        self.elements.key(name)
    }
}

/// Resolves two [`LabelIndex`]es as (art assets, elements).
impl NameResolver for (LabelIndex, LabelIndex) {
    fn art_asset(&self, name: &str) -> Option<u32> {
        self.0.key(name)
    }

    fn element(&self, name: &str) -> Option<u32> {
        self.1.key(name)
    }
}

impl SkinTables {
    /// Loads the tables from the user's data (read-only).
    ///
    /// # Errors
    ///
    /// [`SkinDataError::Missing`] when the art-asset chain, the element chain or the base
    /// table is absent; [`SkinDataError::Props`] when a record fails to read or decode. A
    /// missing game skin or property name is not an error (game skins are optional, and an
    /// unnamed protection never applies).
    pub fn load<S: RecordSource>(data: &GameData<S>) -> Result<Self, SkinDataError> {
        let registry = data.registry()?.clone();
        let art_assets = load_chain(data, ART_ASSET_LABEL)?
            .ok_or(SkinDataError::Missing("EMAPPER/ArtAssetID enum mapper"))?;
        let elements = load_chain(data, ELEMENT_ID_LABEL)?
            .ok_or(SkinDataError::Missing("EMAPPER/UIElementID enum mapper"))?;
        let pids = MappingPids::from_registry(&registry);

        let base_did = data
            .lookup_path(&[UNIQUEDB_LABEL, BASE_SKIN_LABEL])?
            .ok_or(SkinDataError::Missing("UNIQUEDB/BaseSkinMappings"))?;
        let base_set = data
            .property_set(base_did)?
            .ok_or(SkinDataError::Missing("BaseSkinMappings property set"))?;
        let mut base = BaseTable::new();
        for (art, did) in pids.pairs(&base_set) {
            base.insert(did, art);
        }

        let mut game_skins = Vec::new();
        for label in GAME_SKIN_LABELS {
            let Some(did) = data.lookup_path(&[UNIQUEDB_LABEL, label])? else {
                continue;
            };
            let set = match data.property_set(did) {
                Ok(Some(set)) => set,
                Ok(None) => continue,
                Err(e) => {
                    tracing::warn!(label, error = %e, "game skin record not readable");
                    continue;
                }
            };
            let skin_id = pids
                .skin_id
                .and_then(|p| set.get(p))
                .and_then(Value::as_enum)
                .unwrap_or(0);
            let mut skin = GameSkin::new(*label, skin_id);
            for (art, did) in pids.pairs(&set) {
                skin.insert(art, did);
            }
            game_skins.push(skin);
        }

        Ok(Self {
            art_assets,
            elements,
            base,
            game_skins,
            geometry: GeometryPids::from_registry(&registry),
        })
    }
}

/// The label index of the `EMAPPER` entry `label` and its base chain; `None` when the
/// entry or its mapper is absent.
fn load_chain<S: RecordSource>(
    data: &GameData<S>,
    label: &str,
) -> Result<Option<LabelIndex>, SkinDataError> {
    let Some(mut did) = data.lookup_path(&[EMAPPER_LABEL, label])? else {
        return Ok(None);
    };
    let mut chain = Vec::new();
    let mut seen: Vec<Did> = Vec::new();
    while !did.is_null() && chain.len() < ENUM_BASE_CHAIN_LIMIT && !seen.contains(&did) {
        seen.push(did);
        let mapper = match data.enum_mapper(did) {
            Ok(Some(m)) => m,
            Ok(None) => break,
            Err(e) if chain.is_empty() => return Err(e.into()),
            Err(e) => {
                tracing::debug!(error = %e, "enum mapper base chain cut short");
                break;
            }
        };
        did = mapper.base;
        chain.push(mapper);
    }
    if chain.is_empty() {
        return Ok(None);
    }
    Ok(Some(LabelIndex::from_chain(
        chain.iter().map(AsRef::as_ref),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_index_folds_ascii_case_first_wins() {
        let mut ix = LabelIndex::new();
        ix.insert(5, "BasePanel_TopLeft");
        ix.insert(6, "basepanel_topleft");
        ix.insert(7, "Ünïcode");
        assert_eq!(ix.key("BASEPANEL_TOPLEFT"), Some(5));
        assert_eq!(ix.key("Ünïcode"), None);
        assert_eq!(ix.label(5), Some("BasePanel_TopLeft"));
        assert_eq!(ix.len(), 1);
    }

    #[test]
    fn base_table_first_did_wins_and_zero_ignored() {
        let mut t = BaseTable::new();
        assert!(t.insert(0x4100_0001, 0x1000_0001));
        assert!(!t.insert(0x4100_0001, 0x1000_0002));
        assert!(!t.insert(0, 1));
        assert!(!t.insert(1, 0));
        assert_eq!(t.art_asset(0x4100_0001), Some(0x1000_0001));
        assert_eq!(t.did(0x1000_0001), Some(0x4100_0001));
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn game_skin_first_mapping_wins() {
        let mut g = GameSkin::new("x", 1);
        assert!(g.insert(10, 20));
        assert!(!g.insert(10, 30));
        assert_eq!(g.replacement(10), Some(20));
    }
}
