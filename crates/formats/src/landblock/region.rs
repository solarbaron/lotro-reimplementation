// SPDX-License-Identifier: GPL-3.0-or-later
//! Region descriptors (prefix 0x0F, `client_general.dat`).
//!
//! Spec: `docs/specs/environment.md` §3.1 (layout: fixed fields, nine DID maps, a trailing
//! DID) and §3.2 (the tables each slot names); `docs/specs/landblock.md` §5.1 (slot 7 is the
//! block-map descriptor).
//!
//! The decoder parses the grammar of §3.1 rather than requiring the 114 bytes every record
//! of the observed install has: a DID map is a TSize followed by (u32 key, DID) entries, and
//! the client asks every map for key 0 only, so the typed table accessors return the key-0
//! entry. Only the newest layout (archive version 24 or later) is known to this decoder; the
//! older versions of §3.1 are not distinguished and a payload that does not parse exactly is
//! rejected.

use super::put;
use crate::did::Did;
use crate::error::{FormatError, Result};
use crate::reader::{Reader, TSize, write_vle};
use crate::record::Record;

/// Type prefix of region descriptors.
pub const REGION_DESCRIPTOR_PREFIX: u8 = 0x0F;
/// Stored size of every region descriptor of the observed install (one entry in each of maps
/// 1–7, none in maps 8 and 9). An observation, not a rule: the decoder parses the grammar.
pub const REGION_DESCRIPTOR_LEN: usize = 114;
/// Number of DID maps in a region descriptor (slots 1–7 and the two maps of unknown kind).
pub const REGION_DID_MAPS: usize = 9;
/// Most entries the decoder accepts in one region-descriptor DID map (a decoder limit;
/// every observed map holds 0 or 1, `environment.md` §3.1).
pub const REGION_DID_MAP_CAPACITY: usize = 8;
/// Stored size of one DID-map entry (u32 key, DID).
const DID_MAP_ENTRY_SIZE: usize = 8;
/// Side of a map-coordinate origin step in world units, L (`environment.md` §3.1 fields
/// 5–6, `coordinates.md` §8.1).
const ORIGIN_STEP: f32 = 20.0;
/// Side of a landblock in world units.
const LANDBLOCK_UNITS: f32 = 160.0;

/// The eight per-region tables a region descriptor names (`environment.md` §3.1–§3.2, §4).
///
/// Slots 1–7 are the key-0 entries of the first seven DID maps; slot 8 is the trailing DID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RegionTable {
    /// Slot 1: scene-choice table, prefix 0x11 (`scenery.md` §4.3).
    SceneChoice,
    /// Slot 2: terrain-scene table, prefix 0x12 (`scenery.md` §4.2).
    TerrainScene,
    /// Slot 3: encounter table, prefix 0x14 (layout OPEN).
    Encounter,
    /// Slot 4: sky description, prefix 0x15 (`environment.md` §5.1).
    SkyDescription,
    /// Slot 5: water table, prefix 0x16 (`water.md` §4).
    Water,
    /// Slot 6: fog-zone table, prefix 0x17 (`water.md` §5).
    FogZone,
    /// Slot 7: block-map property descriptor, prefix 0x18 (`landblock.md` §5.2).
    BlockMapDescriptor,
    /// Slot 8 (the trailing DID): terrain type table, prefix 0x19
    /// (`terrain-texturing.md` §3).
    TerrainType,
}

impl RegionTable {
    /// All eight tables in slot order.
    pub const ALL: [Self; 8] = [
        Self::SceneChoice,
        Self::TerrainScene,
        Self::Encounter,
        Self::SkyDescription,
        Self::Water,
        Self::FogZone,
        Self::BlockMapDescriptor,
        Self::TerrainType,
    ];

    /// The slot number 1–8 used by the specs.
    #[must_use]
    pub fn slot(self) -> usize {
        match self {
            Self::SceneChoice => 1,
            Self::TerrainScene => 2,
            Self::Encounter => 3,
            Self::SkyDescription => 4,
            Self::Water => 5,
            Self::FogZone => 6,
            Self::BlockMapDescriptor => 7,
            Self::TerrainType => 8,
        }
    }

    /// The DID type prefix of the table this slot names (`environment.md` §4).
    #[must_use]
    pub fn prefix(self) -> u8 {
        match self {
            Self::SceneChoice => 0x11,
            Self::TerrainScene => 0x12,
            Self::Encounter => 0x14,
            Self::SkyDescription => 0x15,
            Self::Water => 0x16,
            Self::FogZone => 0x17,
            Self::BlockMapDescriptor => 0x18,
            Self::TerrainType => 0x19,
        }
    }

    /// Human-readable name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::SceneChoice => "scene-choice table",
            Self::TerrainScene => "terrain-scene table",
            Self::Encounter => "encounter table",
            Self::SkyDescription => "sky description",
            Self::Water => "water table",
            Self::FogZone => "fog-zone table",
            Self::BlockMapDescriptor => "block-map descriptor",
            Self::TerrainType => "terrain type table",
        }
    }
}

/// One (key, DID) entry of a region-descriptor DID map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RegionDidMapEntry {
    /// The key (0 in every observed entry; the client asks for key 0 only).
    pub key: u32,
    /// The table DID.
    pub did: Did,
}

/// A DID map of a region descriptor: a TSize and (u32 key, DID) entries
/// (`environment.md` §3.1).
///
/// The entries are stored inline, at most [`REGION_DID_MAP_CAPACITY`] of them (a decoder
/// limit: every observed map holds 0 or 1), so that [`RegionDescriptor`] stays `Copy`. A map
/// with more entries is rejected as [`FormatError::CountLimit`]. The entry count is kept
/// consistent with the stored entries by construction ([`RegionDidMap::new`] and the
/// decoder are the only ways to build one); unused slots are always default, so the
/// derived equality compares (hint, entries).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RegionDidMap {
    hint: u8,
    len: u8,
    slots: [RegionDidMapEntry; REGION_DID_MAP_CAPACITY],
}

impl RegionDidMap {
    /// A map with the given TSize hint and entries; `None` for more than
    /// [`REGION_DID_MAP_CAPACITY`] entries.
    #[must_use]
    pub fn new(hint: u8, entries: &[RegionDidMapEntry]) -> Option<Self> {
        let mut slots = [RegionDidMapEntry::default(); REGION_DID_MAP_CAPACITY];
        slots.get_mut(..entries.len())?.copy_from_slice(entries);
        Some(Self {
            hint,
            len: u8::try_from(entries.len()).ok()?,
            slots,
        })
    }

    /// The stored TSize hint (1 in every observed map).
    #[must_use]
    pub fn hint(&self) -> u8 {
        self.hint
    }

    /// The stored TSize header: the hint and the entry count.
    #[must_use]
    pub fn size(&self) -> TSize {
        TSize {
            hint: self.hint,
            count: u32::from(self.len),
        }
    }

    /// Number of entries (at most [`REGION_DID_MAP_CAPACITY`]).
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries().len()
    }

    /// Whether the map has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Entries in stored order.
    #[must_use]
    pub fn entries(&self) -> &[RegionDidMapEntry] {
        let n = usize::from(self.len).min(REGION_DID_MAP_CAPACITY);
        &self.slots[..n]
    }

    /// The DID stored for `key` (the first entry, if several share it).
    #[must_use]
    pub fn get(&self, key: u32) -> Option<Did> {
        self.entries().iter().find(|e| e.key == key).map(|e| e.did)
    }

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let at = r.position();
        let size = r.tsize()?;
        r.ensure_elements(size.count, DID_MAP_ENTRY_SIZE)?;
        let n = usize::try_from(size.count).unwrap_or(usize::MAX);
        if n > REGION_DID_MAP_CAPACITY {
            return Err(FormatError::CountLimit {
                what: "region-descriptor DID map entries",
                offset: at,
                count: u64::from(size.count),
                limit: REGION_DID_MAP_CAPACITY as u64,
            });
        }
        let mut slots = [RegionDidMapEntry::default(); REGION_DID_MAP_CAPACITY];
        for slot in &mut slots[..n] {
            *slot = RegionDidMapEntry {
                key: r.u32()?,
                did: r.did()?,
            };
        }
        Ok(Self {
            hint: size.hint,
            // n ≤ REGION_DID_MAP_CAPACITY, checked above.
            len: u8::try_from(n).unwrap_or(u8::MAX),
            slots,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.push(self.hint);
        write_vle(out, put::len_u32(self.entries().len()));
        for e in self.entries() {
            put::u32(out, e.key);
            put::u32(out, e.did.0);
        }
    }
}

/// A region descriptor (prefix 0x0F; `environment.md` §3.1). It names the region's
/// per-region tables (slots 1–8, [`RegionTable`]) and stores the map-coordinate parameters
/// (`coordinates.md` §8.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionDescriptor {
    /// Own DID.
    pub did: Did,
    /// The landblock region number *R* this descriptor describes.
    pub region: u32,
    /// Offset 8: a render surface (prefix 0x41), the region's map image (Inferred).
    pub map_image: Did,
    /// Offset 12 (2 observed; meaning OPEN).
    pub value_12: u32,
    /// Map-coordinate origin along X and Y in steps of 20 world units (1,468, 1,244
    /// observed).
    pub map_origin: [u32; 2],
    /// Added to the map X and Y coordinates (0.0 observed).
    pub map_offset: [f32; 2],
    /// Map units per world unit (0.005 observed).
    pub map_scale: f32,
    /// The nine DID maps in stored order: index 0–6 are slots 1–7, index 7 and 8 are the
    /// two maps of unknown kind (empty in every observed record).
    pub maps: [RegionDidMap; REGION_DID_MAPS],
    /// The trailing DID: slot 8, the terrain type table (prefix 0x19).
    pub trailing: Did,
}

impl RegionDescriptor {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "region descriptor";

    /// The DID that `table` names: the key-0 entry of its DID map (slots 1–7) or the
    /// trailing DID (slot 8). `None` when the map has no key-0 entry or the DID is null.
    ///
    /// The DID's prefix is not checked here; [`RegionDescriptor::first_unusual_field`]
    /// reports a slot whose DID has an unexpected prefix, and the table decoders check
    /// their own prefix.
    #[must_use]
    pub fn table(&self, table: RegionTable) -> Option<Did> {
        let did = match table {
            RegionTable::TerrainType => self.trailing,
            t => self.maps.get(t.slot() - 1)?.get(0)?,
        };
        (!did.is_null()).then_some(did)
    }

    /// Every table with the DID its slot names, in slot order.
    #[must_use]
    pub fn tables(&self) -> [(RegionTable, Option<Did>); 8] {
        RegionTable::ALL.map(|t| (t, self.table(t)))
    }

    /// The first table DID (in slot order) whose type prefix is `prefix`. Prefer the typed
    /// accessors, which follow the client's slot rule.
    #[must_use]
    pub fn table_with_prefix(&self, prefix: u8) -> Option<Did> {
        self.tables()
            .into_iter()
            .filter_map(|(_, d)| d)
            .find(|d| d.prefix() == prefix)
    }

    /// Slot 1: the scene-choice table (0x11).
    #[must_use]
    pub fn scene_choice_table(&self) -> Option<Did> {
        self.table(RegionTable::SceneChoice)
    }

    /// Slot 2: the terrain-scene table (0x12).
    #[must_use]
    pub fn terrain_scene_table(&self) -> Option<Did> {
        self.table(RegionTable::TerrainScene)
    }

    /// Slot 3: the encounter table (0x14).
    #[must_use]
    pub fn encounter_table(&self) -> Option<Did> {
        self.table(RegionTable::Encounter)
    }

    /// Slot 4: the sky description (0x15).
    #[must_use]
    pub fn sky_description(&self) -> Option<Did> {
        self.table(RegionTable::SkyDescription)
    }

    /// Slot 5: the water table (0x16).
    #[must_use]
    pub fn water_table(&self) -> Option<Did> {
        self.table(RegionTable::Water)
    }

    /// Slot 6: the fog-zone table (0x17).
    #[must_use]
    pub fn fog_zone_table(&self) -> Option<Did> {
        self.table(RegionTable::FogZone)
    }

    /// Slot 7: the block-map property descriptor (0x18, `landblock.md` §5.1).
    #[must_use]
    pub fn block_map_descriptor(&self) -> Option<Did> {
        self.table(RegionTable::BlockMapDescriptor)
    }

    /// Slot 8: the terrain type table (0x19).
    #[must_use]
    pub fn terrain_type_table(&self) -> Option<Did> {
        self.table(RegionTable::TerrainType)
    }

    /// The map-coordinate origin in world units: steps × 20 (`environment.md` §3.1), exact
    /// in f64.
    #[must_use]
    pub fn map_origin_units(&self) -> [f64; 2] {
        let step = f64::from(ORIGIN_STEP);
        [
            f64::from(self.map_origin[0]) * step,
            f64::from(self.map_origin[1]) * step,
        ]
    }

    /// Map coordinates `[longitude, latitude]` (map X, map Y) of block-local (x, y) in
    /// landblock (X, Y), evaluated exactly as `coordinates.md` §8.2 step 3 prescribes: every
    /// operation in f32, in this order, per axis (X0 the stored origin, s the stored scale,
    /// Δ the stored offset):
    ///
    /// a = (f32(X) · 160 + x) · s; b = (f32(X0) · 20) · s; result = (Δ − b) + a.
    ///
    /// Positive longitude is east, positive latitude is north; the display rule is §8.3.
    /// The exclusions of §8.2 steps 1–2 (region 0, landblock coordinate 0xFF, cell 0xFFFF,
    /// no descriptor for the region) are the caller's: this helper computes the formula for
    /// whatever it is given. Non-finite stored values give non-finite results
    /// ([`RegionDescriptor::first_unusual_field`] reports them).
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // §8.2: the origin is converted to f32.
    pub fn map_coordinates(&self, landblock: [u8; 2], local: [f32; 2]) -> [f32; 2] {
        let s = self.map_scale;
        let axis = |k: usize| {
            let a = (f32::from(landblock[k]) * LANDBLOCK_UNITS + local[k]) * s;
            let b = (self.map_origin[k] as f32 * ORIGIN_STEP) * s;
            (self.map_offset[k] - b) + a
        };
        [axis(0), axis(1)]
    }

    /// Re-encodes the record (byte-exact for a decoded record whose VLE counts use the
    /// shortest form, as every observed record does).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(REGION_DESCRIPTOR_LEN);
        put::u32(&mut out, self.did.0);
        put::u32(&mut out, self.region);
        put::u32(&mut out, self.map_image.0);
        put::u32(&mut out, self.value_12);
        put::u32(&mut out, self.map_origin[0]);
        put::u32(&mut out, self.map_origin[1]);
        put::f32s(&mut out, &self.map_offset);
        put::f32s(&mut out, &[self.map_scale]);
        for m in &self.maps {
            m.write(&mut out);
        }
        put::u32(&mut out, self.trailing.0);
        out
    }

    /// The first deviation from the values observed in every record of the install
    /// (`environment.md` §3.1–§3.2), or `None`: maps 1–7 hold exactly one entry, with key
    /// 0, maps 8–9 are empty, every slot names a DID with the prefix of its table, and the
    /// map scale is a positive finite number.
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        for m in &self.maps[..7] {
            if !matches!(m.entries(), [e] if e.key == 0) {
                return Some("table DID map does not hold exactly one key-0 entry");
            }
        }
        if self.maps[7..].iter().any(|m| !m.entries().is_empty()) {
            return Some("DID map 8 or 9 is not empty");
        }
        for (t, did) in self.tables() {
            if did.is_none_or(|d| d.prefix() != t.prefix()) {
                return Some("table slot names a DID with an unexpected prefix");
            }
        }
        if !(self.map_scale.is_finite() && self.map_scale > 0.0) {
            return Some("map scale not a positive finite number");
        }
        if self.map_offset.iter().any(|v| !v.is_finite()) {
            return Some("map offset not finite");
        }
        None
    }
}

impl Record<'_> for RegionDescriptor {
    const NAME: &'static str = RegionDescriptor::NAME;

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = r.did()?;
        if did.prefix() != REGION_DESCRIPTOR_PREFIX {
            return Err(FormatError::WrongRecordType {
                did: did.0,
                expected: Self::NAME,
            });
        }
        let region = r.u32()?;
        let map_image = r.did()?;
        let value_12 = r.u32()?;
        let map_origin = [r.u32()?, r.u32()?];
        let map_offset = [r.f32()?, r.f32()?];
        let map_scale = r.f32()?;
        let mut maps: [RegionDidMap; REGION_DID_MAPS] = Default::default();
        for m in &mut maps {
            *m = RegionDidMap::read(&mut r)?;
        }
        let trailing = r.did()?;
        r.expect_end(Self::NAME)?;
        r.log_unknown_hints(Self::NAME);
        Ok(Self {
            did,
            region,
            map_image,
            value_12,
            map_origin,
            map_offset,
            map_scale,
            maps,
            trailing,
        })
    }

    fn did(&self) -> Did {
        self.did
    }
}
