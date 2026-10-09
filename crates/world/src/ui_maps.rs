// SPDX-License-Identifier: GPL-3.0-or-later
//! Map-record lookup for the native panels' minimap and world map (`game-panels.md` §8.4.2,
//! §8.5.1, §9.3; `landblock.md` §4.4, §6; `wstate.md` §11.4; `property-system.md` §6.2).
//!
//! This module does only the record I/O: it answers **which** `UI_Map_GameMap` value belongs
//! to a place and hands it back undecoded. Decoding the struct (image, content layers,
//! georeference, fog) and the map pixel transforms are the game UI's (`tessera-ui`,
//! `game::map`), so the rules of GP §8.4.3 and §8.6.1 exist once.
//!
//! - Outdoors: the landblock-properties record `0x804RXXYY` of a block (GP §8.4.2).
//! - Indoors (GP §8.5.1 Tessera rule): the cell's `Dungeon_DID` from the block's
//!   landblock-info record (LB §4.4), the dungeon's property set (a `Dungeon_DID` is a
//!   game-object DID, prefix 0x70; its property set is at DID + 0x09000000, PS §6.2 /
//!   WS §11.4), that set's `Dungeon_MapData` (a 0x78 property set) and its `UI_Map_GameMap`.
//!
//! Every value is read from the user's install at run time; property IDs are looked up by
//! name ([`UiMapPids::from_names`]). Malformed or missing data gives "no map", never a
//! panic. Every answer is cached in bounded caches, **failures included**: a record that
//! cannot be read or decoded is reported once (as an error) and then answers "no map"
//! from the cache, so a caller that asks every frame does not re-read it.
//!
//! [`UiMapReader`] is synchronous. A caller on the main thread runs it on a worker
//! (`apps/client`'s `WorldMaps` does, on the asset worker pool; `docs/ARCHITECTURE.md` §21).

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use tessera_formats::landblock::Cell;
use tessera_formats::property::{PropertySchema, PropertyValue};
use tessera_formats::property_set::{
    PropertySet, PropertySetRecord, is_property_set_did, object_property_set_did,
};
use tessera_formats::{Did, LandblockInfo, LandblockProperties};

use crate::coords::LandblockKey;
use crate::error::{Result, WorldError};
use crate::source::WorldSource;

/// Default number of answers [`UiMapReader`] keeps (each kind separately).
pub const DEFAULT_CACHE_ENTRIES: usize = 256;

/// A `UI_Map_GameMap` value as stored (a struct, `landblock.md` §6.2), shared.
pub type MapValue = Arc<PropertyValue>;

/// The interior cells of one block that name a dungeon: cell number → `Dungeon_DID`.
pub type BlockDungeons = Arc<BTreeMap<u16, Did>>;

/// The property IDs of the map lookups, found by name at run time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UiMapPids {
    /// `UI_Map_GameMap` (the map struct of a landblock-properties record or a map-data set).
    pub game_map: Option<u32>,
    /// `Dungeon_DID` (a landblock-info cell's dungeon, `landblock.md` §4).
    pub dungeon_did: Option<u32>,
    /// `Dungeon_MapData` (a dungeon's map property set, `wstate.md` §11).
    pub dungeon_map_data: Option<u32>,
}

impl UiMapPids {
    /// Looks every PID up with `pid_of(name)`.
    #[must_use]
    pub fn from_names(pid_of: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            game_map: pid_of("UI_Map_GameMap"),
            dungeon_did: pid_of("Dungeon_DID"),
            dungeon_map_data: pid_of("Dungeon_MapData"),
        }
    }
}

/// The `Dungeon_DID` of interior cell `cell` among a landblock-info record's cells
/// (`landblock.md` §4.4; GP §8.5.1 Tessera rule). `None` when no cell has that number, the
/// cell has no property set, or the property is absent or null.
#[must_use]
pub fn cell_dungeon(cells: &[Cell], cell: u16, dungeon_did: u32) -> Option<Did> {
    cells
        .iter()
        .find(|c| c.index == cell)?
        .properties
        .as_ref()?
        .get(dungeon_did)
        .and_then(as_did)
}

/// Every cell of a landblock-info record's cells that names a dungeon (`Dungeon_DID`
/// present and not null), by cell number. The first cell with a number wins.
#[must_use]
pub fn cell_dungeons(cells: &[Cell], dungeon_did: u32) -> BTreeMap<u16, Did> {
    let mut out = BTreeMap::new();
    for c in cells {
        if let Some(d) = c
            .properties
            .as_ref()
            .and_then(|p| p.get(dungeon_did))
            .and_then(as_did)
        {
            out.entry(c.index).or_insert(d);
        }
    }
    out
}

/// The property-set DID of a dungeon named by `Dungeon_DID`: the game object's property set
/// at DID + 0x09000000 for a game-object DID (prefixes 0x70–0x76, `property-system.md` §6.2,
/// `wstate.md` §11.4); a DID that already is a property-set DID (0x78–0x7F) unchanged;
/// `None` for anything else.
#[must_use]
pub fn dungeon_property_set_did(dungeon: Did) -> Option<Did> {
    if is_property_set_did(dungeon) {
        Some(dungeon)
    } else {
        object_property_set_did(dungeon)
    }
}

/// A small bounded cache: insertion order eviction.
#[derive(Debug, Clone)]
struct Bounded<K, V> {
    map: BTreeMap<K, V>,
    order: VecDeque<K>,
    cap: usize,
}

impl<K: Ord + Copy, V: Clone> Bounded<K, V> {
    fn new(cap: usize) -> Self {
        Self {
            map: BTreeMap::new(),
            order: VecDeque::new(),
            cap: cap.max(1),
        }
    }

    fn get(&self, k: K) -> Option<V> {
        self.map.get(&k).cloned()
    }

    fn put(&mut self, k: K, v: V) {
        if self.map.insert(k, v).is_none() {
            self.order.push_back(k);
        }
        while self.map.len() > self.cap {
            match self.order.pop_front() {
                Some(old) => {
                    self.map.remove(&old);
                }
                None => break,
            }
        }
    }
}

/// Reads map records for the minimap and the world map, synchronously, with bounded
/// caches: a block's map, a block's whole cell → dungeon table (one landblock-info parse
/// per block, however many of its cells are asked for) and a dungeon's map are each
/// remembered, failures as "no map".
pub struct UiMapReader {
    source: Arc<dyn WorldSource>,
    schema: SchemaRef,
    pids: UiMapPids,
    blocks: Bounded<u32, Option<MapValue>>,
    cells: Bounded<u32, BlockDungeons>,
    dungeons: Bounded<u32, Option<MapValue>>,
    reads: u64,
    failures: u64,
}

/// Where the reader's property schema lives.
enum SchemaRef {
    Own(Arc<dyn PropertySchema + Send + Sync>),
    Context(Arc<crate::context::WorldContext>),
}

impl SchemaRef {
    fn get(&self) -> &(dyn PropertySchema + Send + Sync) {
        match self {
            Self::Own(s) => &**s,
            Self::Context(c) => c.schema(),
        }
    }
}

impl std::fmt::Debug for UiMapReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiMapReader")
            .field("pids", &self.pids)
            .field("blocks", &self.blocks.map.len())
            .field("cells", &self.cells.map.len())
            .field("dungeons", &self.dungeons.map.len())
            .field("reads", &self.reads)
            .field("failures", &self.failures)
            .finish_non_exhaustive()
    }
}

impl UiMapReader {
    /// A reader over `source` decoding property sets with `schema` (normally the property
    /// dictionary), keeping up to `cache_entries` answers of each kind.
    #[must_use]
    pub fn new(
        source: Arc<dyn WorldSource>,
        schema: Arc<dyn PropertySchema + Send + Sync>,
        pids: UiMapPids,
        cache_entries: usize,
    ) -> Self {
        Self::with_schema(source, SchemaRef::Own(schema), pids, cache_entries)
    }

    /// A reader over a world context's source and schema, with the PIDs looked up by
    /// `pid_of`, keeping [`DEFAULT_CACHE_ENTRIES`] answers of each kind.
    #[must_use]
    pub fn from_context(
        ctx: Arc<crate::context::WorldContext>,
        pid_of: impl Fn(&str) -> Option<u32>,
    ) -> Self {
        let mut pids = UiMapPids::from_names(pid_of);
        if pids.game_map.is_none() {
            pids.game_map = ctx.pids().landblock.ui_map_game_map;
        }
        let source = Arc::clone(ctx.source());
        Self::with_schema(source, SchemaRef::Context(ctx), pids, DEFAULT_CACHE_ENTRIES)
    }

    fn with_schema(
        source: Arc<dyn WorldSource>,
        schema: SchemaRef,
        pids: UiMapPids,
        cache_entries: usize,
    ) -> Self {
        Self {
            source,
            schema,
            pids,
            blocks: Bounded::new(cache_entries),
            cells: Bounded::new(cache_entries),
            dungeons: Bounded::new(cache_entries),
            reads: 0,
            failures: 0,
        }
    }

    /// The PIDs in use.
    #[must_use]
    pub fn pids(&self) -> &UiMapPids {
        &self.pids
    }

    /// Records read from the source so far (each cache miss reads at most one or two).
    #[must_use]
    pub fn reads(&self) -> u64 {
        self.reads
    }

    /// Lookups that failed (an unreadable or undecodable record); each failure is
    /// reported once and then cached as "no map".
    #[must_use]
    pub fn failures(&self) -> u64 {
        self.failures
    }

    /// The `UI_Map_GameMap` value of block `key`'s landblock-properties record (GP §8.4.2):
    /// `Ok(None)` when the block has no record or the record has no map.
    ///
    /// # Errors
    /// [`WorldError::Asset`] when the record cannot be read, [`WorldError::Format`] when it
    /// does not decode. The failure is cached as `None`: later calls answer `Ok(None)`
    /// without reading again.
    pub fn landblock_map(&mut self, key: LandblockKey) -> Result<Option<MapValue>> {
        let did = key.properties_did();
        if let Some(v) = self.blocks.get(did.0) {
            return Ok(v);
        }
        let r = self.read_landblock_map(did);
        let v = self.settle(&r);
        self.blocks.put(did.0, v);
        r
    }

    /// The cached answer of [`UiMapReader::landblock_map`], if any (no I/O).
    #[must_use]
    pub fn cached_landblock_map(&self, key: LandblockKey) -> Option<Option<MapValue>> {
        self.blocks.get(key.properties_did().0)
    }

    fn read_landblock_map(&mut self, did: Did) -> Result<Option<MapValue>> {
        self.reads += 1;
        let Some(bytes) = self.source.read(did)? else {
            return Ok(None);
        };
        let rec = LandblockProperties::parse_expecting(&bytes, did, self.schema.get())
            .map_err(|e| WorldError::format(did, LandblockProperties::NAME, e))?;
        Ok(self.game_map_of(&rec.properties))
    }

    /// The interior cells of block `key` that name a dungeon (`landblock.md` §4.4), read
    /// from its landblock-info record once and cached per block. Empty when the block has
    /// no record, no cells, or the `Dungeon_DID` PID is unknown.
    ///
    /// # Errors
    /// As [`UiMapReader::landblock_map`] (the failure is cached as an empty table).
    pub fn block_dungeons(&mut self, key: LandblockKey) -> Result<BlockDungeons> {
        let did = key.info_did();
        if let Some(v) = self.cells.get(did.0) {
            return Ok(v);
        }
        let r = self.read_block_dungeons(did);
        let v = match &r {
            Ok(v) => Arc::clone(v),
            Err(_) => {
                self.failures += 1;
                BlockDungeons::default()
            }
        };
        self.cells.put(did.0, v);
        r
    }

    fn read_block_dungeons(&mut self, did: Did) -> Result<BlockDungeons> {
        let Some(pid) = self.pids.dungeon_did else {
            return Ok(BlockDungeons::default());
        };
        self.reads += 1;
        let Some((bytes, stamp)) = self.source.read_stamped(did)? else {
            return Ok(BlockDungeons::default());
        };
        let info = LandblockInfo::parse_expecting(&bytes, did, stamp, self.schema.get())
            .map_err(|e| WorldError::format(did, LandblockInfo::NAME, e))?;
        Ok(Arc::new(
            info.cells
                .as_ref()
                .map(|c| cell_dungeons(&c.items, pid))
                .unwrap_or_default(),
        ))
    }

    /// The `Dungeon_DID` of interior cell `cell` of block `key` (`landblock.md` §4.4):
    /// `Ok(None)` for cell 0 or 0xFFFF, a block without an info record or cells, or a cell
    /// without the property.
    ///
    /// # Errors
    /// As [`UiMapReader::block_dungeons`].
    pub fn cell_dungeon(&mut self, key: LandblockKey, cell: u16) -> Result<Option<Did>> {
        if cell == 0 || cell == 0xFFFF || self.pids.dungeon_did.is_none() {
            return Ok(None);
        }
        Ok(self.block_dungeons(key)?.get(&cell).copied())
    }

    /// The map of dungeon `dungeon` (GP §8.5.1 Tessera rule): the `UI_Map_GameMap` of the
    /// set its property set's `Dungeon_MapData` names. `dungeon` is the `Dungeon_DID` value
    /// (a game-object DID; see [`dungeon_property_set_did`]). `Ok(None)` when the dungeon
    /// or its map data is missing or has no map.
    ///
    /// # Errors
    /// As [`UiMapReader::landblock_map`] (the failure is cached as `None`).
    pub fn dungeon_map(&mut self, dungeon: Did) -> Result<Option<MapValue>> {
        if let Some(v) = self.dungeons.get(dungeon.0) {
            return Ok(v);
        }
        let r = self.read_dungeon_map(dungeon);
        let v = self.settle(&r);
        self.dungeons.put(dungeon.0, v);
        r
    }

    fn read_dungeon_map(&mut self, dungeon: Did) -> Result<Option<MapValue>> {
        let Some(map_pid) = self.pids.dungeon_map_data else {
            return Ok(None);
        };
        let Some(set_did) = dungeon_property_set_did(dungeon) else {
            return Ok(None);
        };
        let Some(d) = self.property_set(set_did)? else {
            return Ok(None);
        };
        let Some(map_did) = d.get(map_pid).and_then(as_did) else {
            return Ok(None);
        };
        let Some(m) = self.property_set(map_did)? else {
            return Ok(None);
        };
        Ok(self.game_map_of(&m))
    }

    /// The map for an interior position (GP §8.5.1): the cell's dungeon, then its map.
    ///
    /// # Errors
    /// As [`UiMapReader::landblock_map`].
    pub fn interior_map(&mut self, key: LandblockKey, cell: u16) -> Result<Option<MapValue>> {
        match self.cell_dungeon(key, cell)? {
            Some(d) => self.dungeon_map(d),
            None => Ok(None),
        }
    }

    /// The cached answer of [`UiMapReader::interior_map`], if every step is cached (no I/O).
    #[must_use]
    pub fn cached_interior_map(&self, key: LandblockKey, cell: u16) -> Option<Option<MapValue>> {
        if cell == 0 || cell == 0xFFFF || self.pids.dungeon_did.is_none() {
            return Some(None);
        }
        match self.cells.get(key.info_did().0)?.get(&cell) {
            None => Some(None),
            Some(d) => self.dungeons.get(d.0),
        }
    }

    /// The cache value for a lookup result: the answer, or `None` for a failure (counted).
    fn settle(&mut self, r: &Result<Option<MapValue>>) -> Option<MapValue> {
        match r {
            Ok(v) => v.clone(),
            Err(_) => {
                self.failures += 1;
                None
            }
        }
    }

    /// The `UI_Map_GameMap` value of a set, when it is a struct.
    fn game_map_of(&self, set: &PropertySet) -> Option<MapValue> {
        let v = set.get(self.pids.game_map?)?;
        v.as_struct()?;
        Some(Arc::new(v.clone()))
    }

    /// A property-set record (0x78–0x7F) by DID, `Ok(None)` when it is missing or the DID
    /// is not a property-set DID.
    fn property_set(&mut self, did: Did) -> Result<Option<PropertySet>> {
        if !is_property_set_did(did) {
            return Ok(None);
        }
        self.reads += 1;
        let Some(bytes) = self.source.read(did)? else {
            return Ok(None);
        };
        let rec = PropertySetRecord::parse_expecting(&bytes, did, self.schema.get())
            .map_err(|e| WorldError::format(did, PropertySetRecord::NAME, e))?;
        Ok(Some(rec.set))
    }
}

/// A non-null DID value.
fn as_did(v: &PropertyValue) -> Option<Did> {
    match v {
        PropertyValue::DidRef(d) if !d.is_null() => Some(*d),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::MemoryWorldSource;
    use crate::testkit::pid::{UI_MAP_BLOCK_OFFSET_X, UI_MAP_GAME_MAP, UI_MAP_MAP_IMAGE};
    use crate::testkit::{self, TestValue};
    use tessera_formats::property::{PropertyEntry, PropertyType};

    const DUNGEON_DID: u32 = 0x1000_0204;
    const MAP_DATA: u32 = 0x1000_0205;

    fn pids() -> UiMapPids {
        UiMapPids {
            game_map: Some(UI_MAP_GAME_MAP),
            dungeon_did: Some(DUNGEON_DID),
            dungeon_map_data: Some(MAP_DATA),
        }
    }

    fn schema() -> Arc<dyn PropertySchema + Send + Sync> {
        let mut s = testkit::schema();
        s.insert(DUNGEON_DID, PropertyType::from_code(20));
        s.insert(MAP_DATA, PropertyType::from_code(20));
        Arc::new(s)
    }

    fn game_map(image: u32, bx: i32) -> TestValue {
        TestValue::Struct(vec![
            (UI_MAP_MAP_IMAGE, TestValue::Did(image)),
            (UI_MAP_BLOCK_OFFSET_X, TestValue::Int(bx)),
        ])
    }

    /// The image DID of a `UI_Map_GameMap` value.
    fn image(v: &PropertyValue) -> Option<u32> {
        match v.as_struct()?.get(UI_MAP_MAP_IMAGE)? {
            PropertyValue::DidRef(d) => Some(d.0),
            _ => None,
        }
    }

    /// A landblock-info record with the given cells (`landblock.md` §4.4): each cell has no
    /// position fields, no neighbours or lists, and a property set (`Dungeon_DID` when
    /// given).
    fn info_with_cells(key: LandblockKey, cells: &[(u16, Option<u32>)]) -> Vec<u8> {
        // The empty record: did, flags, size hint, TSize (hint 0, count 0), the rest.
        let empty = testkit::landblock_info(key, &[]);
        let mut out = empty[..12].to_vec();
        out.push(0);
        out.push(u8::try_from(cells.len()).expect("few cells"));
        for (index, dungeon) in cells {
            out.extend_from_slice(&index.to_le_bytes());
            out.push(0); // position flags: no fields
            out.extend_from_slice(&0x04u32.to_le_bytes()); // property set present
            out.extend_from_slice(&0x0100_0001u32.to_le_bytes()); // geometry
            out.extend_from_slice(&0u32.to_le_bytes()); // neighbours
            out.extend_from_slice(&0u32.to_le_bytes()); // list A
            out.extend_from_slice(&0u32.to_le_bytes()); // list B
            let entries: Vec<(u32, TestValue)> = dungeon
                .iter()
                .map(|d| (DUNGEON_DID, TestValue::Did(*d)))
                .collect();
            out.extend_from_slice(&testkit::property_set(&entries));
        }
        out.extend_from_slice(&empty[14..]);
        out
    }

    #[test]
    fn reads_landblock_maps_and_caches() {
        let key = LandblockKey::new(1, 10, 20).unwrap();
        let empty = LandblockKey::new(1, 11, 20).unwrap();
        let mut src = MemoryWorldSource::new();
        src.insert(
            key.properties_did(),
            testkit::landblock_properties(key, &[(UI_MAP_GAME_MAP, game_map(0x4100_0010, 10))]),
            Some(39),
        );
        let mut r = UiMapReader::new(Arc::new(src), schema(), pids(), 4);
        assert_eq!(r.cached_landblock_map(key), None);
        let m = r.landblock_map(key).unwrap().expect("a map");
        assert_eq!(image(&m), Some(0x4100_0010));
        assert_eq!(r.cached_landblock_map(key), Some(Some(Arc::clone(&m))));
        let reads = r.reads();
        assert_eq!(r.landblock_map(key).unwrap(), Some(m));
        assert_eq!(r.reads(), reads, "answered from the cache");
        assert_eq!(r.landblock_map(empty).unwrap(), None);
        // The cache stays bounded.
        for x in 0..20 {
            let k = LandblockKey::new(2, x, 0).unwrap();
            assert_eq!(r.landblock_map(k).unwrap(), None);
        }
        assert!(r.blocks.map.len() <= 4);
    }

    #[test]
    fn a_failure_is_reported_once_then_cached() {
        let key = LandblockKey::new(1, 3, 4).unwrap();
        let mut src = MemoryWorldSource::new();
        src.insert(key.properties_did(), vec![0xFFu8; 7], None);
        src.insert(key.info_did(), vec![0xFFu8; 64], Some(39));
        let mut r = UiMapReader::new(Arc::new(src), schema(), pids(), 4);
        assert!(matches!(
            r.landblock_map(key),
            Err(WorldError::Format { .. })
        ));
        assert!(r.interior_map(key, 0x100).is_err());
        assert_eq!(r.failures(), 2);
        let reads = r.reads();
        // Regression (review): failed lookups were re-read and re-parsed on every call.
        for cell in [0x100, 0x100, 0x101, 0x102] {
            assert_eq!(r.interior_map(key, cell).unwrap(), None);
        }
        for _ in 0..100 {
            assert_eq!(r.landblock_map(key).unwrap(), None);
        }
        assert_eq!(r.reads(), reads, "no record read again");
        assert_eq!(r.failures(), 2);
    }

    #[test]
    fn dungeon_did_maps_to_its_property_set() {
        assert_eq!(
            dungeon_property_set_did(Did(0x7002_2EDB)),
            Some(Did(0x7902_2EDB))
        );
        assert_eq!(
            dungeon_property_set_did(Did(0x7900_0100)),
            Some(Did(0x7900_0100))
        );
        assert_eq!(dungeon_property_set_did(Did(0x4100_0000)), None);
        assert_eq!(dungeon_property_set_did(Did(0x7700_0001)), None);
    }

    /// Regression (review): a cell's `Dungeon_DID` is a game-object DID (0x70); the reader
    /// looked the 0x70 DID up as a property set and never found a map on real data.
    #[test]
    fn interior_maps_follow_cell_dungeon_and_map_data() {
        let key = LandblockKey::new(1, 5, 6).unwrap();
        let dungeon = 0x7000_0100u32;
        let dungeon_set = 0x7900_0100u32;
        let map_data = 0x7800_0200u32;
        let mut src = MemoryWorldSource::new();
        src.insert(
            key.info_did(),
            info_with_cells(
                key,
                &[(0x100, Some(dungeon)), (0x101, None), (0x102, Some(0))],
            ),
            Some(39),
        );
        src.insert(
            Did(dungeon_set),
            testkit::property_set_record(dungeon_set, &[(MAP_DATA, TestValue::Did(map_data))]),
            None,
        );
        src.insert(
            Did(map_data),
            testkit::property_set_record(map_data, &[(UI_MAP_GAME_MAP, game_map(0x4100_0020, 3))]),
            None,
        );
        let mut r = UiMapReader::new(Arc::new(src), schema(), pids(), 8);
        let table = r.block_dungeons(key).unwrap();
        assert_eq!(table.len(), 1, "only cell 0x100 names a dungeon");
        assert_eq!(r.cell_dungeon(key, 0x100).unwrap(), Some(Did(dungeon)));
        assert_eq!(r.cell_dungeon(key, 0x101).unwrap(), None);
        assert_eq!(r.cell_dungeon(key, 0x102).unwrap(), None, "null DID");
        assert_eq!(r.cached_interior_map(key, 0x100), None, "map not read yet");
        let m = r.interior_map(key, 0x100).unwrap().expect("an indoor map");
        assert_eq!(image(&m), Some(0x4100_0020));
        assert_eq!(r.cached_interior_map(key, 0x100), Some(Some(m)));
        assert_eq!(r.cached_interior_map(key, 0x101), Some(None));
        // The stored dungeon DID itself is not a property set: the 0x70 → 0x79 rule finds it.
        assert!(r.dungeon_map(Did(dungeon)).unwrap().is_some());
        assert!(r.dungeon_map(Did(dungeon_set)).unwrap().is_some());
        assert_eq!(r.dungeon_map(Did(0x7000_0101)).unwrap(), None);
        assert_eq!(r.dungeon_map(Did(0x4100_0000)).unwrap(), None);
        // Outdoor cells have no dungeon.
        assert_eq!(r.cell_dungeon(key, 0).unwrap(), None);
        assert_eq!(r.cell_dungeon(key, 0xFFFF).unwrap(), None);
        assert_eq!(r.failures(), 0);
    }

    /// Regression (review): each new cell of a block re-read and re-parsed the block's
    /// landblock-info record.
    #[test]
    fn one_info_read_per_block() {
        let key = LandblockKey::new(1, 7, 8).unwrap();
        let cells: Vec<(u16, Option<u32>)> = (0x100..0x140)
            .map(|c| (c, Some(0x7000_0000 + u32::from(c))))
            .collect();
        let mut src = MemoryWorldSource::new();
        src.insert(key.info_did(), info_with_cells(key, &cells), Some(39));
        let mut r = UiMapReader::new(Arc::new(src), schema(), pids(), 8);
        for (c, d) in &cells {
            assert_eq!(r.cell_dungeon(key, *c).unwrap(), d.map(Did));
        }
        assert_eq!(r.reads(), 1);
        // A block without an info record: one read, then cached.
        let none = LandblockKey::new(1, 7, 9).unwrap();
        for c in 0x100..0x140 {
            assert_eq!(r.interior_map(none, c).unwrap(), None);
        }
        assert_eq!(r.reads(), 2);
    }

    #[test]
    fn cell_dungeon_lookup() {
        let mut props = PropertySet::default();
        props.entries.push(PropertyEntry {
            pid: DUNGEON_DID,
            value: PropertyValue::DidRef(Did(0x7000_0005)),
        });
        let cells = vec![
            Cell {
                index: 0x100,
                ..Cell::default()
            },
            Cell {
                index: 0x101,
                properties: Some(props),
                ..Cell::default()
            },
        ];
        assert_eq!(
            cell_dungeon(&cells, 0x101, DUNGEON_DID),
            Some(Did(0x7000_0005))
        );
        assert_eq!(cell_dungeon(&cells, 0x100, DUNGEON_DID), None);
        assert_eq!(cell_dungeon(&cells, 0x102, DUNGEON_DID), None);
        let all = cell_dungeons(&cells, DUNGEON_DID);
        assert_eq!(
            all.into_iter().collect::<Vec<_>>(),
            vec![(0x101, Did(0x7000_0005))]
        );
    }

    #[test]
    fn from_context_uses_the_context_source_and_pids() {
        let key = LandblockKey::new(1, 10, 20).unwrap();
        let mut src = testkit::SyntheticWorld::rectangle(1, 10..=10, 20..=20).build();
        src.insert(
            key.properties_did(),
            testkit::landblock_properties(key, &[(UI_MAP_GAME_MAP, game_map(0x4100_0030, 10))]),
            Some(39),
        );
        let ctx = Arc::new(
            crate::WorldContext::load(Arc::new(src), crate::BundleOptions::default())
                .expect("synthetic context"),
        );
        // No dungeon PIDs: the map PID comes from the context's landblock PIDs.
        let mut r = UiMapReader::from_context(ctx, |_| None);
        assert_eq!(r.pids().game_map, Some(UI_MAP_GAME_MAP));
        assert_eq!(r.pids().dungeon_did, None);
        let m = r.landblock_map(key).unwrap().expect("a map");
        assert_eq!(image(&m), Some(0x4100_0030));
        assert_eq!(r.interior_map(key, 0x100).unwrap(), None);
        assert_eq!(r.reads(), 1, "no info read without the Dungeon_DID PID");
    }

    #[test]
    fn a_map_that_is_not_a_struct_is_no_map() {
        let key = LandblockKey::new(1, 1, 2).unwrap();
        let mut src = MemoryWorldSource::new();
        src.insert(
            key.properties_did(),
            testkit::landblock_properties(key, &[(UI_MAP_MAP_IMAGE, TestValue::Did(0x4100_0001))]),
            Some(39),
        );
        let mut p = pids();
        // Point the map PID at a DID-valued property.
        p.game_map = Some(UI_MAP_MAP_IMAGE);
        let mut r = UiMapReader::new(Arc::new(src), schema(), p, 4);
        assert_eq!(r.landblock_map(key).unwrap(), None);
    }
}
