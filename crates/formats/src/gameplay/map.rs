// SPDX-License-Identifier: GPL-3.0-or-later
//! Map data: named places, map notes, dungeon quest notes, travel networks and
//! neighbourhoods.
//!
//! Spec: `docs/specs/map-data.md`.
//!
//! | Item | What | Spec |
//! |---|---|---|
//! | [`PLACES_TABLE_LABEL`] and the other locator constants | Where the three singleton tables and the directory singletons are found | §2.1, §2.2, §11 |
//! | [`PlacesTable`], [`Place`], [`fold_key_unit`], [`place_key_hash`] | The named-places table (0x0E000004 here), case-insensitive lookup | §3 |
//! | [`MapNotesTable`], [`MapNote`], [`NoteBase`], [`NoteLink`] | The map-note table (0x0E000006 here) | §4.1–§4.3, §4.5 |
//! | [`position_is_valid`], [`text_is_valid`], [`MapNote::validity`] | The client's validity rules | §4.4 |
//! | [`MapNote::filing`], [`MapNoteIndex`] | How the client files rows after loading | §4.6 |
//! | [`MapNote::radar_shows`] | The radar's range test | §4.7 |
//! | [`QuestNotesTable`], [`QuestNote`], [`QuestNoteInfo`] | The dungeon quest-note table (0x0E400000 here) | §5 |
//! | [`MapPids`], [`MapNoteProperties`], [`bit_key`] | Map-note properties of game objects, bit-to-key rule | §6 |
//! | [`MapKind`], [`classify_map_kind`], [`ZoneMapData`] | Record kinds of this spec, recognised by their properties; zone map links | §2.3, §7 |
//! | [`TravelWeb`], [`TravelDestination`], [`TravelRoute`], [`TravelSegment`], [`TravelHop`], [`TravelPad`], [`TravelMode`], [`TravelVersioning`], [`StableMaster`], [`route_pattern`] | The travel network | §8 |
//! | [`Neighbourhood`], [`NeighbourhoodTemplate`], [`HouseList`], [`House`], [`NeighbourhoodGate`], [`HousingHook`] | Neighbourhoods and housing | §9 |
//! | [`for_each_place_reference`] | String properties whose value is a place key | §10 |
//! | [`Ripcord`] | RIPCORDS records | §11 |
//! | [`position_flags_consistent`] | The flag rules of positions (MI4) | §12, `property-system.md` §4.6 |
//!
//! Everything is found at run time: tables through UNIQUEDB labels, properties through
//! dictionary labels ([`MapPids`]), kinds by the properties they carry. Class indices and
//! DIDs of one install are never used.
//!
//! Embedded property sets (`extra`, `quest_info`) reject a TSize hint above 22 here
//! (`property-system.md` §5.3 step 1). Two rules of `property-system.md` that these tables
//! depend on are applied by the shared readers, not by this module, and those readers do not
//! follow them yet: the stored StringInfo form of §17.4 (any mode byte, any empty-table hint,
//! the argument table of §17.4.1), read by [`StringInfo::read`], and the 8-byte table-form
//! null entry of §5.2, read by [`PropertySet::read`]. Neither form occurs in this install's
//! tables.

use std::collections::{BTreeMap, HashMap};

use crate::did::Did;
use crate::error::{FormatError, Result};
use crate::gameplay::common::{PropertyAccess, ValueExt};
use crate::property::{
    Position, PropertyEntry, PropertySchema, PropertyStruct, PropertyValue, ValueDecoder,
};
use crate::property_ids;
use crate::property_set::{EntryContext, PropertySet};
use crate::reader::{Reader, TSize, Utf16String, tsize_bucket_count};
use crate::string_info::{STRING_INFO_MAX_DEPTH, StringArgValue, StringInfo, StringSource};

// ---------------------------------------------------------------------------------------------
// §2 Finding the records
// ---------------------------------------------------------------------------------------------

/// UNIQUEDB label of the named-places table (§2.1).
pub const PLACES_TABLE_LABEL: &str = "PlacesTable";
/// UNIQUEDB label of the map-note table (§2.1).
pub const MAP_NOTES_LABEL: &str = "MapNotes";
/// UNIQUEDB label of the dungeon quest-note table (§2.1).
pub const DUNGEON_QUEST_NOTES_LABEL: &str = "DungeonQuestMapNotes";
/// UNIQUEDB key of the named-places table (§2.1; a stable identifier, like the label).
pub const PLACES_TABLE_KEY: u32 = 0x9E;
/// UNIQUEDB key of the map-note table (§2.1).
pub const MAP_NOTES_KEY: u32 = 0x1000_001F;
/// UNIQUEDB key of the dungeon quest-note table (§2.1).
pub const DUNGEON_QUEST_NOTES_KEY: u32 = 0x1000_000F;

/// WEENIECONTENT label of the travel-web directory (§2.2).
pub const TRAVEL_WEB_DIRECTORY_LABEL: &str = "TravelWebDirectory";
/// WEENIECONTENT label of the neighbourhood directory (§2.2).
pub const NEIGHBORHOOD_DIRECTORY_LABEL: &str = "NeighborhoodDirectory";
/// WEENIECONTENT label of the housing-control singleton (§2.2).
pub const HOUSING_CONTROL_LABEL: &str = "HousingControl";
/// Root DID-map label of the RIPCORDS map (§11).
pub const RIPCORDS_LABEL: &str = "RIPCORDS";
/// Root DID-map key of the RIPCORDS map (§11).
pub const RIPCORDS_ROOT_KEY: u32 = 0x1000_0002;

/// Largest TSize hint a reader accepts (`property-system.md` §17.2; §3.1).
pub const MAX_TSIZE_HINT: u8 = 22;

/// Bucket counts of hints 13–22 (`property-system.md` §17.2); hints 0–12 are
/// [`crate::reader::TSIZE_BUCKET_COUNTS`].
const EXTENDED_BUCKET_COUNTS: [u32; 10] = [
    98_299, 196_597, 393_209, 786_431, 1_572_853, 3_145_721, 6_291_449, 12_582_893, 25_165_813,
    50_331_599,
];

/// The bucket count of TSize hint `hint` (0–22), `None` above 22.
#[must_use]
pub fn bucket_count(hint: u8) -> Option<u32> {
    tsize_bucket_count(hint).or_else(|| {
        EXTENDED_BUCKET_COUNTS
            .get(usize::from(hint).checked_sub(13)?)
            .copied()
    })
}

/// Reads a TSize and rejects a hint above [`MAX_TSIZE_HINT`] (§3.1).
fn read_tsize(r: &mut Reader<'_>, what: &'static str) -> Result<TSize> {
    let at = r.position();
    let size = r.tsize()?;
    if size.hint > MAX_TSIZE_HINT {
        return Err(FormatError::malformed(
            what,
            at,
            format!("TSize hint {} exceeds {MAX_TSIZE_HINT}", size.hint),
        ));
    }
    Ok(size)
}

/// Reads an embedded property set (§1: `property-system.md` §5.1 without the DID) and rejects
/// a TSize hint above [`MAX_TSIZE_HINT`] (`property-system.md` §5.3 step 1, §17.2) before
/// decoding the entries.
fn read_embedded_set<S: PropertySchema + ?Sized>(
    r: &mut Reader<'_>,
    schema: &S,
    what: &'static str,
) -> Result<PropertySet> {
    let at = r.position();
    let hint = r.peek(1)?.first().copied().unwrap_or(0);
    if hint > MAX_TSIZE_HINT {
        return Err(FormatError::malformed(
            what,
            at,
            format!("TSize hint {hint} exceeds {MAX_TSIZE_HINT}"),
        ));
    }
    PropertySet::read(r, schema)
}

// ---------------------------------------------------------------------------------------------
// Positions (§4.4.1, PS §4.6)
// ---------------------------------------------------------------------------------------------

/// Tolerance of the client's float comparisons in this spec (§4.4, §4.6, §4.7).
pub const RANGE_EPSILON: f32 = 0.0002;

/// The values of a position, with absent fields set to the values of
/// `property-system.md` §4.6 ("absent fields").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionValues {
    /// Region; 0 = none.
    pub region: u8,
    /// Landblock X and Y; (0xFF, 0xFF) = none.
    pub landblock: (u8, u8),
    /// Instance; 0 = none.
    pub instance: u16,
    /// Cell; 0xFFFF = none, 0 = outdoors.
    pub cell: u16,
    /// x, y, z; (0, 0, 0) when absent.
    pub offset: [f32; 3],
    /// Orientation (w, x, y, z); the identity when absent.
    pub orientation: [f32; 4],
}

impl PositionValues {
    /// The values of `p` (payload bits only, `property-system.md` §4.6.1).
    #[must_use]
    pub fn of(p: &Position) -> Self {
        Self {
            region: p.region().unwrap_or(0),
            landblock: p.landblock().unwrap_or((0xFF, 0xFF)),
            instance: p.instance().unwrap_or(0),
            cell: p.cell().unwrap_or(0xFFFF),
            offset: p.offset().unwrap_or([0.0; 3]),
            orientation: p.orientation().unwrap_or([1.0, 0.0, 0.0, 0.0]),
        }
    }

    /// The flag byte a writer computes from these values (`property-system.md` §4.6).
    #[must_use]
    pub fn expected_flags(&self) -> u8 {
        let mut f = 0u8;
        if self.region != 0 {
            f |= Position::REGION;
        }
        if self.landblock.0 != 0xFF && self.landblock.1 != 0xFF {
            f |= Position::LANDBLOCK;
        }
        if self.instance != 0 {
            f |= Position::INSTANCE;
        }
        if self.cell != 0xFFFF {
            f |= Position::CELL;
        }
        if self.offset != [0.0; 3] {
            f |= Position::OFFSET;
        }
        if self.orientation != [1.0, 0.0, 0.0, 0.0] {
            f |= Position::ORIENTATION;
        }
        if self.region == 1 {
            f |= 0x40;
        }
        if self.cell == 0 {
            f |= 0x80;
        }
        f
    }

    /// The landblock key *X* · 65,536 + *Y* of §4.6 step 2.
    #[must_use]
    pub fn landblock_key(&self) -> u32 {
        (u32::from(self.landblock.0) << 16) | u32::from(self.landblock.1)
    }

    /// Whether the cell is an interior cell (neither outdoors nor absent).
    #[must_use]
    pub fn is_interior(&self) -> bool {
        self.cell != 0 && self.cell != 0xFFFF
    }
}

/// The position test of §4.4.1: region ≠ 0, landblock X and Y ≠ 0xFF, cell ≠ 0xFFFF,
/// x/y/z finite, the orientation finite with |w² + x² + y² + z² − 1| ≤ 0.0002 (summed in
/// f32 in the order w, x, y, z). Absent fields take their "absent" values first.
#[must_use]
pub fn position_is_valid(p: &Position) -> bool {
    let v = PositionValues::of(p);
    if v.region == 0 || v.landblock.0 == 0xFF || v.landblock.1 == 0xFF || v.cell == 0xFFFF {
        return false;
    }
    if !v.offset.iter().all(|c| c.is_finite()) || !v.orientation.iter().all(|c| c.is_finite()) {
        return false;
    }
    let [w, x, y, z] = v.orientation;
    let sum = w * w + x * x + y * y + z * z;
    (sum - 1.0).abs() <= RANGE_EPSILON
}

/// Whether `p` follows the flag rules of `property-system.md` §4.6 (MI4): its stored flag
/// byte is the one a writer computes from its values, and it has no instance.
#[must_use]
pub fn position_flags_consistent(p: &Position) -> bool {
    let v = PositionValues::of(p);
    p.instance().is_none() && p.flags() == v.expected_flags()
}

// ---------------------------------------------------------------------------------------------
// §3 Named places
// ---------------------------------------------------------------------------------------------

/// Smallest stored place entry: a 1-byte name count, a 1-byte position and two u32 slots.
const PLACE_MIN_ENTRY: usize = 10;

/// Lower-cases one UTF-16 code unit of a place key (§3.3.1).
///
/// A–Z map to a–z and other ASCII units are unchanged (what the client does under every
/// locale). Outside ASCII this is the **Tessera rule** of §3.3.1: the Unicode simple
/// lower-case mapping when it is a single unit in the Basic Multilingual Plane, otherwise
/// the unit unchanged; surrogate units are unchanged.
#[must_use]
pub fn fold_key_unit(u: u16) -> u16 {
    match u {
        0x41..=0x5A => u + 0x20,
        0..=0x7F | 0xD800..=0xDFFF => u,
        // The one unit whose full lower-case mapping (what `char::to_lowercase` gives)
        // differs from its simple mapping: capital I with dot above → i.
        0x0130 => 0x0069,
        _ => char::from_u32(u32::from(u)).map_or(u, |c| {
            let mut lower = c.to_lowercase();
            match (lower.next(), lower.next()) {
                (Some(l), None) => u16::try_from(u32::from(l)).unwrap_or(u),
                _ => u,
            }
        }),
    }
}

/// The key of `units` as the client compares and hashes it (§3.3.1): every unit
/// lower-cased, up to the first unit 0.
pub fn fold_key(units: &[u16]) -> impl Iterator<Item = u16> + '_ {
    units
        .iter()
        .copied()
        .take_while(|&u| u != 0)
        .map(fold_key_unit)
}

/// The hash of §3.3: the `records.md` §1.5 name hash over the lower-cased units of `units`
/// (each added with its full 16-bit value), stopping at the first unit 0.
#[must_use]
pub fn place_key_hash(units: &[u16]) -> u32 {
    let mut h: u32 = 0;
    for u in fold_key(units) {
        h = (h << 4).wrapping_add(u32::from(u));
        let g = h & 0xF000_0000;
        h ^= g >> 24;
    }
    h & 0x0FFF_FFFF
}

/// The bucket of `units` for TSize hint `hint` (§3.3); `None` for a hint above 22.
#[must_use]
pub fn place_bucket(units: &[u16], hint: u8) -> Option<u32> {
    Some(place_key_hash(units) % bucket_count(hint)?)
}

/// One entry of the named-places table (§3.2).
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    /// The place key, as stored.
    pub name: Utf16String,
    /// Where the place is.
    pub position: Position,
    /// Area slot: the zone the client reports as the place's area; 0 = none.
    pub area: Did,
    /// Dungeon slot; 0 = none. Equal to `area` in every entry observed.
    pub dungeon: Did,
}

impl Place {
    /// The zone of the place: the area slot, else the dungeon slot, else `None`.
    #[must_use]
    pub fn zone(&self) -> Option<Did> {
        [self.area, self.dungeon].into_iter().find(|d| !d.is_null())
    }
}

/// A warning about the named-places table (§12: MI2, MI3, MI4, MI15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceIssue {
    /// Entry `index` sorts into a lower bucket than its predecessor (MI2). `non_ascii`:
    /// the key has units outside ASCII, whose folding may differ from the writer's.
    OutOfBucketOrder {
        /// Entry index.
        index: usize,
        /// Whether the key has non-ASCII units.
        non_ascii: bool,
    },
    /// Entry `index` has the same key as entry `first`, ignoring case (MI2); lookups find
    /// `first`.
    DuplicateKey {
        /// Entry index.
        index: usize,
        /// The earlier entry with the same key.
        first: usize,
    },
    /// The area and dungeon slots differ (MI3, observed equal everywhere).
    SlotsDiffer {
        /// Entry index.
        index: usize,
    },
    /// The position breaks the flag rules of `property-system.md` §4.6 (MI4).
    PositionFlags {
        /// Entry index.
        index: usize,
    },
    /// The position fails the test of §4.4.1 (MI15).
    InvalidPosition {
        /// Entry index.
        index: usize,
    },
    /// The name contains a code unit 0; it is keyed by the units before it (§3.3.1).
    NameHasNul {
        /// Entry index.
        index: usize,
    },
}

/// The named-places table (§3; `PlacesTable`, 0x0E000004 here): place key → position and
/// zone.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacesTable {
    /// The record's DID.
    pub did: Did,
    /// TSize header: hint and entry count.
    pub size: TSize,
    /// Entries in stored (bucket) order.
    pub places: Vec<Place>,
    /// Folded key → index of the first entry with that key.
    index: HashMap<Vec<u16>, usize>,
}

impl PlacesTable {
    /// Human-readable record name, used in errors.
    pub const NAME: &'static str = "places table";

    /// Decodes a complete payload (§3.1). Fails on truncation, a hint above 22, a count
    /// larger than the remaining bytes allow, or trailing bytes (MI1).
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = r.did()?;
        let size = read_tsize(&mut r, Self::NAME)?;
        let count = size.count;
        let mut places: Vec<Place> = r.alloc_vec(count, PLACE_MIN_ENTRY, usize::MAX)?;
        for _ in 0..count {
            let name = r.utf16_string()?;
            let position = Position::read(&mut r)?;
            let area = r.did()?;
            let dungeon = r.did()?;
            r.push(
                &mut places,
                Place {
                    name,
                    position,
                    area,
                    dungeon,
                },
                count,
            )?;
        }
        r.expect_end(Self::NAME)?;
        let mut index = HashMap::new();
        for (i, p) in places.iter().enumerate() {
            let key: Vec<u16> = fold_key(p.name.units()).collect();
            // Key units plus a rough per-entry cost of the map.
            r.charge(key.len().saturating_mul(2).saturating_add(48))?;
            index.entry(key).or_insert(i);
        }
        tracing::debug!(did = %did, places = places.len(), "decoded places table");
        Ok(Self {
            did,
            size,
            places,
            index,
        })
    }

    /// Decodes a payload and checks its DID.
    pub fn parse_expecting(data: &[u8], expected: impl Into<Did>) -> Result<Self> {
        let expected = expected.into();
        let t = Self::parse(data)?;
        if t.did != expected {
            return Err(FormatError::DidMismatch {
                expected: expected.0,
                found: t.did.0,
            });
        }
        Ok(t)
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.places.len()
    }

    /// Whether the table has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.places.is_empty()
    }

    /// Entries in stored order.
    pub fn iter(&self) -> std::slice::Iter<'_, Place> {
        self.places.iter()
    }

    /// Index of the place with key `key`, ignoring case (§3.4); the first stored entry
    /// wins when keys collide.
    #[must_use]
    pub fn find(&self, key: &[u16]) -> Option<usize> {
        let k: Vec<u16> = fold_key(key).collect();
        self.index.get(&k).copied()
    }

    /// The place with key `key`, ignoring case (§3.4).
    #[must_use]
    pub fn get(&self, key: &[u16]) -> Option<&Place> {
        self.places.get(self.find(key)?)
    }

    /// [`PlacesTable::get`] with a Rust string key.
    #[must_use]
    pub fn get_str(&self, key: &str) -> Option<&Place> {
        let units: Vec<u16> = key.encode_utf16().collect();
        self.get(&units)
    }

    /// The client's first lookup: the position of place `key` (§3.4).
    #[must_use]
    pub fn position_of(&self, key: &[u16]) -> Option<Position> {
        self.get(key).map(|p| p.position)
    }

    /// The client's second lookup: the area slot of place `key` (§3.4).
    #[must_use]
    pub fn area_of(&self, key: &[u16]) -> Option<Did> {
        self.get(key).map(|p| p.area)
    }

    /// Whether the entries are in bucket order of the lower-cased key hash (MI2, §3.3).
    #[must_use]
    pub fn is_bucket_ordered(&self) -> Option<bool> {
        let b = bucket_count(self.size.hint)?;
        let mut last = 0u32;
        for p in &self.places {
            let bucket = place_key_hash(p.name.units()) % b;
            if bucket < last {
                return Some(false);
            }
            last = bucket;
        }
        Some(true)
    }

    /// Every warning of §12 that the table alone can check (MI2, MI3 equality, MI4, MI15).
    #[must_use]
    pub fn issues(&self) -> Vec<PlaceIssue> {
        let mut out = Vec::new();
        let buckets = bucket_count(self.size.hint);
        let mut last = 0u32;
        for (index, p) in self.places.iter().enumerate() {
            let units = p.name.units();
            if let Some(b) = buckets {
                let bucket = place_key_hash(units) % b;
                if bucket < last {
                    out.push(PlaceIssue::OutOfBucketOrder {
                        index,
                        non_ascii: units.iter().any(|&u| u > 0x7F),
                    });
                }
                last = last.max(bucket);
            }
            if let Some(first) = self.find(units).filter(|&f| f != index) {
                out.push(PlaceIssue::DuplicateKey { index, first });
            }
            if p.area != p.dungeon {
                out.push(PlaceIssue::SlotsDiffer { index });
            }
            if !position_flags_consistent(&p.position) {
                out.push(PlaceIssue::PositionFlags { index });
            }
            if !position_is_valid(&p.position) {
                out.push(PlaceIssue::InvalidPosition { index });
            }
            if units.contains(&0) {
                out.push(PlaceIssue::NameHasNul { index });
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------
// §4–§5 Map notes and dungeon quest notes
// ---------------------------------------------------------------------------------------------

/// Smallest map-note row (§4.1): 1 + 12 + 4 + 5 + 17 + 12 + 2 bytes.
pub const MAP_NOTE_MIN_ROW: usize = 53;

/// Smallest dungeon quest-note row: a 1-byte position, three DIDs, a null `layers`
/// property and an empty set (Tessera bound, §14.4).
pub const QUEST_NOTE_MIN_ROW: usize = 1 + 12 + 4 + 2;

/// The fields every note row starts with (§4.2 fields 1–5, §5.1 fields 1–5).
#[derive(Debug, Clone, PartialEq)]
pub struct NoteBase {
    /// Where the note is.
    pub position: Position,
    /// The area (sometimes a dungeon) the note lies in; 0 = none.
    pub area: Did,
    /// The dungeon the note lies in; 0 outdoors.
    pub dungeon: Did,
    /// The game object the note marks.
    pub entity: Did,
    /// The content-layer property (list form: PID and value); `None` for a null PID.
    pub layers: Option<PropertyEntry>,
}

impl NoteBase {
    fn read<S: PropertySchema + ?Sized>(r: &mut Reader<'_>, schema: &S) -> Result<Self> {
        let position = Position::read(r)?;
        let area = r.did()?;
        let dungeon = r.did()?;
        let entity = r.did()?;
        let layers = ValueDecoder::new(schema).entry(r, false, 0)?;
        Ok(Self {
            position,
            area,
            dungeon,
            entity,
            layers,
        })
    }

    /// PID of the content-layer property (`Entity_ContentLayers` in every row observed).
    #[must_use]
    pub fn layers_pid(&self) -> Option<u32> {
        self.layers.as_ref().map(|e| e.pid)
    }

    /// The content-layer list: the enum keys of the array elements (ED §4.5.1). Elements
    /// of other types are skipped; a non-array value yields nothing.
    #[must_use]
    pub fn layer_keys(&self) -> Vec<u32> {
        self.layers
            .as_ref()
            .map(|e| e.value.children())
            .unwrap_or_default()
            .iter()
            .filter_map(ValueExt::enum_key)
            .collect()
    }

    /// The position's values with absent fields filled in.
    #[must_use]
    pub fn position_values(&self) -> PositionValues {
        PositionValues::of(&self.position)
    }
}

/// The link block of a link row (§4.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteLink {
    /// Where the link leads.
    pub position: Position,
    /// The zone the link leads into.
    pub target: Did,
}

/// One map-note row (§4.2).
#[derive(Debug, Clone, PartialEq)]
pub struct MapNote {
    /// Position, zone, entity and content layers.
    pub base: NoteBase,
    /// The label; null text in link rows.
    pub text: StringInfo,
    /// Icon override (a render surface) or 0.
    pub icon: Did,
    /// `MapNote_Level` bitfield (§6.3).
    pub level: u32,
    /// `MapNote_Type` bitfield (§6.3). Bit 0 (`Link`) marks a link row.
    pub note_type: u64,
    /// The boolean byte after the type; meaning OPEN (§4.2).
    pub flag_b: bool,
    /// The link block, present exactly when bit 0 of `note_type` is set (§4.3).
    pub link: Option<NoteLink>,
    /// `MapNote_MinRange`, world units.
    pub min_range: f32,
    /// `MapNote_MaxRange`, world units.
    pub max_range: f32,
    /// `MapNote_Discoverable` value; 0 = none.
    pub discovery: u32,
    /// Game-specific properties (§4.5).
    pub extra: PropertySet,
}

/// Why the client reports a map-note row as invalid (§4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteInvalid {
    /// The row's position fails §4.4.1.
    Position,
    /// `type` is 0.
    NoType,
    /// A non-link row whose text fails §4.4.2.
    Text,
    /// A non-link row whose `level` is 0.
    NoLevel,
    /// A non-link row whose `min_range` exceeds `max_range` by more than 0.0002.
    Ranges,
}

/// Where the client files a map-note row after loading (§4.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NoteFiling {
    /// In the list of this dungeon.
    Dungeon(Did),
    /// Under the position's region and landblock key *X* · 65,536 + *Y*.
    Landblock {
        /// Region of the position (0 when absent).
        region: u8,
        /// Landblock key.
        key: u32,
    },
    /// In the list of ranged notes (both ranges non-zero).
    Ranged,
}

/// Answers whether a string table of the active locale holds a token (§4.4.2). Implemented
/// for closures `Fn(Did, u32) -> bool`.
pub trait StringLookup {
    /// Whether table `table` exists and holds `token`.
    fn has_token(&self, table: Did, token: u32) -> bool;
}

impl<F: Fn(Did, u32) -> bool> StringLookup for F {
    fn has_token(&self, table: Did, token: u32) -> bool {
        self(table, token)
    }
}

/// The text test of §4.4.2: a literal passes; a reference passes when its token and table
/// are non-zero, `lookup` finds the token, and every argument passes (the **Tessera rule**
/// of §4.4.2: a nested StringInfo must pass this test, any other argument passes).
#[must_use]
pub fn text_is_valid<L: StringLookup + ?Sized>(text: &StringInfo, lookup: &L) -> bool {
    text_valid_depth(text, lookup, 0)
}

fn text_valid_depth<L: StringLookup + ?Sized>(text: &StringInfo, lookup: &L, depth: usize) -> bool {
    if depth > STRING_INFO_MAX_DEPTH {
        return false;
    }
    match &text.source {
        StringSource::Literal(_) => true,
        StringSource::Table { token, table } => {
            *token != 0
                && !table.is_null()
                && lookup.has_token(*table, *token)
                && text.args.as_ref().is_none_or(|a| {
                    a.values.iter().all(|arg| match &arg.value {
                        StringArgValue::Nested(inner) => text_valid_depth(inner, lookup, depth + 1),
                        _ => true,
                    })
                })
        }
    }
}

/// Whether `text` is a null reference: token 0 and table 0 (`property-system.md` §4.8).
#[must_use]
pub fn is_null_text(text: &StringInfo) -> bool {
    matches!(text.source, StringSource::Table { token: 0, table } if table.is_null())
}

/// The enum key of bit `bit` of a `MapNote_Type` or `MapNote_Level` bitfield: `bit` + 1
/// (`property-system.md` §7.3, §6.3).
#[must_use]
pub const fn bit_key(bit: u32) -> u32 {
    bit + 1
}

/// The enum keys of the set bits of `bits`, in ascending order (§6.3).
pub fn bit_keys(bits: u64) -> impl Iterator<Item = u32> {
    (0..64u32)
        .filter(move |&b| bits & (1u64 << b) != 0)
        .map(bit_key)
}

impl MapNote {
    fn read<S: PropertySchema + ?Sized>(r: &mut Reader<'_>, schema: &S) -> Result<Self> {
        let base = NoteBase::read(r, schema)?;
        r.charge(std::mem::size_of::<StringInfo>())?;
        let text = StringInfo::read(r)?;
        let icon = r.did()?;
        let level = r.u32()?;
        let note_type = r.u64()?;
        let flag_b = r.bool8()?;
        let link = if note_type & 1 != 0 {
            let position = Position::read(r)?;
            let target = r.did()?;
            Some(NoteLink { position, target })
        } else {
            None
        };
        let min_range = r.f32()?;
        let max_range = r.f32()?;
        let discovery = r.u32()?;
        let extra = read_embedded_set(r, schema, "map-note extra set")?;
        Ok(Self {
            base,
            text,
            icon,
            level,
            note_type,
            flag_b,
            link,
            min_range,
            max_range,
            discovery,
            extra,
        })
    }

    /// Whether this is a link row: bit 0 of `note_type` (§4.3).
    #[must_use]
    pub fn is_link(&self) -> bool {
        self.note_type & 1 != 0
    }

    /// The single set bit of `note_type`, or `None` when zero or several bits are set.
    #[must_use]
    pub fn type_bit(&self) -> Option<u32> {
        self.note_type
            .is_power_of_two()
            .then(|| self.note_type.trailing_zeros())
    }

    /// The enum keys of `note_type` (§6.3).
    pub fn type_keys(&self) -> impl Iterator<Item = u32> {
        bit_keys(self.note_type)
    }

    /// The enum keys of `level` (§6.3).
    pub fn level_keys(&self) -> impl Iterator<Item = u32> {
        bit_keys(u64::from(self.level))
    }

    /// The client's validity rules (§4.4). `lookup` answers the string-table part of the
    /// text test (§4.4.2).
    pub fn validity<L: StringLookup + ?Sized>(
        &self,
        lookup: &L,
    ) -> std::result::Result<(), NoteInvalid> {
        if !position_is_valid(&self.base.position) {
            return Err(NoteInvalid::Position);
        }
        if self.note_type == 0 {
            return Err(NoteInvalid::NoType);
        }
        if !self.is_link() {
            if !text_is_valid(&self.text, lookup) {
                return Err(NoteInvalid::Text);
            }
            if self.level == 0 {
                return Err(NoteInvalid::NoLevel);
            }
            if self.min_range - self.max_range > RANGE_EPSILON {
                return Err(NoteInvalid::Ranges);
            }
        }
        Ok(())
    }

    /// Where the client files the row (§4.6 steps 1–3).
    #[must_use]
    pub fn filing(&self) -> NoteFiling {
        if !self.base.dungeon.is_null() {
            return NoteFiling::Dungeon(self.base.dungeon);
        }
        if self.max_range.abs() <= RANGE_EPSILON || self.min_range.abs() <= RANGE_EPSILON {
            let v = self.base.position_values();
            return NoteFiling::Landblock {
                region: v.region,
                key: v.landblock_key(),
            };
        }
        NoteFiling::Ranged
    }

    /// The zone entry a link row adds (§4.6): its target and link block, when the target is
    /// non-zero and not the row's own dungeon and both positions pass §4.4.1.
    #[must_use]
    pub fn zone_link(&self) -> Option<&NoteLink> {
        let link = self.link.as_ref()?;
        (!link.target.is_null()
            && link.target != self.base.dungeon
            && position_is_valid(&self.base.position)
            && position_is_valid(&link.position))
        .then_some(link)
    }

    /// Whether the radar range-tests this note: both ranges exceed 0.0002 in magnitude
    /// (§4.7 step 1).
    #[must_use]
    pub fn is_ranged(&self) -> bool {
        self.min_range.abs() > RANGE_EPSILON && self.max_range.abs() > RANGE_EPSILON
    }

    /// The radar's range test (§4.7): `distance_squared` is the squared straight-line
    /// distance from the radar's reference position, in world units. Notes that are not
    /// ranged always show; a ranged note shows when `min_range` ≤ *d* ≤ `max_range`
    /// (compared squared, in f32).
    #[must_use]
    pub fn radar_shows(&self, distance_squared: f32) -> bool {
        if !self.is_ranged() {
            return true;
        }
        !(distance_squared > self.max_range * self.max_range
            || distance_squared < self.min_range * self.min_range)
    }
}

/// Squared straight-line distance between two points of one world frame (§4.7 step 2).
#[must_use]
pub fn distance_squared(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

/// Reads `DID, u32 count, rows` (§4.1, §5.1), bounding the count by `min_row`.
fn read_rows<T>(
    data: &[u8],
    what: &'static str,
    min_row: usize,
    mut row: impl FnMut(&mut Reader<'_>) -> Result<T>,
) -> Result<(Did, Vec<T>)> {
    let mut r = Reader::new(data);
    let did = r.did()?;
    let count = r.u32()?;
    let mut rows = r.alloc_vec(count, min_row, usize::MAX)?;
    for _ in 0..count {
        let item = row(&mut r)?;
        r.push(&mut rows, item, count)?;
    }
    r.expect_end(what)?;
    r.log_unknown_hints(what);
    Ok((did, rows))
}

fn check_did(found: Did, expected: Did) -> Result<()> {
    if found == expected {
        Ok(())
    } else {
        Err(FormatError::DidMismatch {
            expected: expected.0,
            found: found.0,
        })
    }
}

/// The map-note table (§4; `MapNotes`, 0x0E000006 here).
#[derive(Debug, Clone, PartialEq)]
pub struct MapNotesTable {
    /// The record's DID.
    pub did: Did,
    /// Rows in stored order.
    pub notes: Vec<MapNote>,
}

impl MapNotesTable {
    /// Human-readable record name, used in errors.
    pub const NAME: &'static str = "map-note table";

    /// Decodes a complete payload (§4.1). `schema` types the `layers` property and the
    /// `extra` set. Fails on truncation, a row count above remaining / 53, a `flag_b`
    /// above 1 (MI5), a property error, or trailing bytes (MI1).
    pub fn parse<S: PropertySchema + ?Sized>(data: &[u8], schema: &S) -> Result<Self> {
        let (did, notes) = read_rows(data, Self::NAME, MAP_NOTE_MIN_ROW, |r| {
            MapNote::read(r, schema)
        })?;
        tracing::debug!(did = %did, rows = notes.len(), "decoded map-note table");
        Ok(Self { did, notes })
    }

    /// Decodes a payload and checks its DID.
    pub fn parse_expecting<S: PropertySchema + ?Sized>(
        data: &[u8],
        expected: impl Into<Did>,
        schema: &S,
    ) -> Result<Self> {
        let t = Self::parse(data, schema)?;
        check_did(t.did, expected.into())?;
        Ok(t)
    }

    /// Number of rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.notes.len()
    }

    /// Whether the table has no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    /// Rows in stored order.
    pub fn iter(&self) -> std::slice::Iter<'_, MapNote> {
        self.notes.iter()
    }

    /// The client's filing of every row (§4.6).
    #[must_use]
    pub fn index(&self) -> MapNoteIndex {
        MapNoteIndex::build(&self.notes)
    }
}

/// The lists the client builds from the map-note table (§4.6). Values are row indices in
/// stored order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapNoteIndex {
    /// Rows filed under a dungeon (step 1).
    pub by_dungeon: BTreeMap<Did, Vec<usize>>,
    /// Rows filed under (region, landblock key) (step 2).
    pub by_landblock: BTreeMap<(u8, u32), Vec<usize>>,
    /// Ranged rows (step 3).
    pub ranged: Vec<usize>,
    /// Link rows registered under the zone they lead into.
    pub zone_links: BTreeMap<Did, Vec<usize>>,
}

impl MapNoteIndex {
    /// Files every row of `notes` (§4.6).
    #[must_use]
    pub fn build(notes: &[MapNote]) -> Self {
        let mut ix = Self::default();
        for (i, n) in notes.iter().enumerate() {
            match n.filing() {
                NoteFiling::Dungeon(d) => ix.by_dungeon.entry(d).or_default().push(i),
                NoteFiling::Landblock { region, key } => {
                    ix.by_landblock.entry((region, key)).or_default().push(i);
                }
                NoteFiling::Ranged => ix.ranged.push(i),
            }
            if let Some(link) = n.zone_link() {
                ix.zone_links.entry(link.target).or_default().push(i);
            }
        }
        ix
    }

    /// Rows filed under `region` and landblock (`x`, `y`).
    #[must_use]
    pub fn in_landblock(&self, region: u8, x: u8, y: u8) -> &[usize] {
        let key = (u32::from(x) << 16) | u32::from(y);
        self.by_landblock
            .get(&(region, key))
            .map_or(&[], Vec::as_slice)
    }

    /// Rows filed under dungeon `dungeon`.
    #[must_use]
    pub fn in_dungeon(&self, dungeon: Did) -> &[usize] {
        self.by_dungeon.get(&dungeon).map_or(&[], Vec::as_slice)
    }

    /// Link rows that lead into `zone`.
    #[must_use]
    pub fn links_into(&self, zone: Did) -> &[usize] {
        self.zone_links.get(&zone).map_or(&[], Vec::as_slice)
    }
}

/// One dungeon quest-note row (§5.1).
#[derive(Debug, Clone, PartialEq)]
pub struct QuestNote {
    /// Position, zone, entity and content layers.
    pub base: NoteBase,
    /// Quest-dispenser properties (§5.2).
    pub quest_info: PropertySet,
}

/// The dungeon quest-note table (§5; `DungeonQuestMapNotes`, 0x0E400000 here).
#[derive(Debug, Clone, PartialEq)]
pub struct QuestNotesTable {
    /// The record's DID.
    pub did: Did,
    /// Rows in stored order.
    pub notes: Vec<QuestNote>,
}

impl QuestNotesTable {
    /// Human-readable record name, used in errors.
    pub const NAME: &'static str = "dungeon quest-note table";

    /// Decodes a complete payload (§5.1). Fails on truncation, a row count above
    /// remaining / [`QUEST_NOTE_MIN_ROW`], a property error, or trailing bytes (MI1).
    pub fn parse<S: PropertySchema + ?Sized>(data: &[u8], schema: &S) -> Result<Self> {
        let (did, notes) = read_rows(data, Self::NAME, QUEST_NOTE_MIN_ROW, |r| {
            let base = NoteBase::read(r, schema)?;
            let quest_info = read_embedded_set(r, schema, "quest-note quest_info set")?;
            Ok(QuestNote { base, quest_info })
        })?;
        tracing::debug!(did = %did, rows = notes.len(), "decoded dungeon quest-note table");
        Ok(Self { did, notes })
    }

    /// Decodes a payload and checks its DID.
    pub fn parse_expecting<S: PropertySchema + ?Sized>(
        data: &[u8],
        expected: impl Into<Did>,
        schema: &S,
    ) -> Result<Self> {
        let t = Self::parse(data, schema)?;
        check_did(t.did, expected.into())?;
        Ok(t)
    }

    /// Number of rows.
    #[must_use]
    pub fn len(&self) -> usize {
        self.notes.len()
    }

    /// Whether the table has no rows.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    /// Rows grouped by their non-zero dungeon, as the client files them (§5.3).
    #[must_use]
    pub fn by_dungeon(&self) -> BTreeMap<Did, Vec<usize>> {
        let mut out: BTreeMap<Did, Vec<usize>> = BTreeMap::new();
        for (i, n) in self.notes.iter().enumerate() {
            if !n.base.dungeon.is_null() {
                out.entry(n.base.dungeon).or_default().push(i);
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------
// Property IDs (§5.2, §6–§11)
// ---------------------------------------------------------------------------------------------

property_ids! {
    /// Every property this spec names, resolved from dictionary labels at run time.
    pub struct MapPids {
        // §6.1 map-note properties
        note_type = "MapNote_Type",
        note_level = "MapNote_Level",
        note_text = "MapNote_StringInfo",
        note_icon = "MapNote_Icon",
        note_off_screen_icon = "MapNote_OffScreenIcon",
        note_mouse_over_icon = "MapNote_MouseOverIcon",
        note_select_icon = "MapNote_SelectIcon",
        note_min_range = "MapNote_MinRange",
        note_max_range = "MapNote_MaxRange",
        note_discoverable = "MapNote_Discoverable",
        note_enabled = "MapNote_Enabled",
        note_linkable = "MapNote_Linkable",
        note_visible_externally = "MapNote_VisibleExternally",
        note_game_specific = "MapNote_GameSpecific",
        content_layers = "Entity_ContentLayers",
        landmark_music = "Landmark_Music",
        // §5.2 quest-dispenser properties
        role_constraint = "QuestDispenser_RoleConstraint",
        active_quests = "QuestDispenser_ActiveQuests",
        dispenser_quest = "QuestDispenser_Quest",
        dispenser_role_name = "QuestDispenser_RoleName",
        // §7, WS §11.1 zones
        area_territory = "Area_Territory",
        area_region = "Area_Region",
        area_region_id = "Area_RegionID",
        area_map_data = "Area_MapData",
        dungeon_name = "Dungeon_Name",
        dungeon_map_data = "Dungeon_MapData",
        dungeon_minimap_data = "Dungeon_MiniMapData",
        game_map = "UI_Map_GameMap",
        // §8.2 travel-web record
        home_location = "TravelHomeLocation",
        home_locations = "TravelHomeLocationsArray",
        destination_records = "TravelDestinationRecordArray",
        destination_route = "TravelDestinationRoute",
        destination_cost = "TravelDestinationCost",
        destination_min_level = "TravelDestinationMinLevel",
        // §8.3 destination
        display_name = "TravelDisplayName",
        is_swift = "TravelDestinationIsSwiftTravel",
        allow_paper_items = "TravelDestinationAllowPaperItems",
        // §8.4 route
        name = "Name",
        route_destination = "TravelRoute_Destination",
        route_mode = "TravelRoute_TravelMode",
        route_actions = "TravelRoute_ActionArray",
        min_level = "Usage_MinLevel",
        allowed_while_mounted = "Usage_AllowedWhileMounted",
        requires_subscriber = "Usage_RequiresSubscriberOrUnsub",
        quest_requirements = "Usage_QuestRequirements",
        quest_id = "Usage_QuestID",
        quest_status = "Usage_QuestStatus",
        quest_operator = "Usage_Operator",
        required_faction = "Usage_RequiredFaction",
        required_faction_did = "Usage_RequiredFaction_DataID",
        required_faction_tier = "Usage_RequiredFaction_Tier",
        permission = "Usage_Permission",
        // §8.5 actions
        segment_head = "TravelSegment_HeadName",
        segment_subsegments = "TravelSegment_SubSegmentArray",
        hop_delay = "TravelHop_Delay",
        hop_scene = "TravelHop_SceneID",
        pad_location = "TravelPad_Location",
        segment_node_head = "Segment_HeadName",
        // §8.6 travel mode
        mode_path_node_distance = "TravelMode_PathNodeDistance",
        mode_turn_speed = "TravelMode_TurnSpeed",
        mode_turn_acceleration = "TravelMode_TurnAcceleration",
        mode_move_acceleration = "TravelMode_MoveAcceleration",
        mode_walk_distance = "TravelMode_WalkDistance",
        mode_effect = "TravelMode_TravelEffectDID",
        mode_script_pants = "TravelMode_ScriptPants",
        // §8.7 stations, NPCs, versioning, home tables
        web_array = "TravelWebArray",
        web_wc = "TravelWebWC",
        discounts = "Travel_DiscountArray",
        sell_multiplier = "TravelWebSellMultiplier",
        must_be_discovered = "Travel_Stablemaster_MustBeDiscovered",
        versioning_pairs = "TravelVersioning_DestinationPairArray",
        versioning_slow = "TravelVersioning_SlowDestination",
        versioning_swift = "TravelVersioning_SwiftDestination",
        player_home_locations = "Travel_PlayerHomeLocationArray",
        // §9 neighbourhoods and housing
        neighborhood_name = "Neighborhood_Name",
        neighborhood_template = "Neighborhood_NeighborhoodTemplate",
        neighborhood_directory = "NeighborhoodDirectory_NeighborhoodArray",
        template_name = "NeighborhoodTemplate_Name",
        template_house_list = "NeighborhoodTemplate_HouseList",
        template_boundaries = "NeighborhoodTemplate_Boundaries",
        template_telepad = "NeighborhoodTemplate_Telepad",
        template_boot_position = "NeighborhoodTemplate_BootPosition",
        template_scene = "NeighborhoodTemplate_SceneID",
        template_initial_instances = "NeighborhoodTemplate_InitialInstances",
        template_occupancy_threshold = "NeighborhoodTemplate_OccupancyThresholdPercentage",
        template_rollout_step = "NeighborhoodTemplate_RolloutStep",
        house_array = "HouseList_HouseArray",
        house_address = "House_Address",
        house_description = "House_Description",
        house_template = "House_NeighborhoodTemplate",
        house_telepad = "House_Telepad",
        house_type = "House_Type",
        house_value_tier = "House_ValueTier",
        house_is_kinship = "House_IsKinshipHouse",
        house_is_premium = "House_IsPremiumHouse",
        house_characteristics = "House_CharacteristicArray",
        gate_template = "NeighborhoodGate_NeighborhoodTemplate",
        gate_telepad = "NeighborhoodGate_Telepad",
        hook_id = "HousingHook_ID",
        hook_category = "HousingHook_HookCategory",
        hook_is_property = "HousingHook_IsPropertyHook",
        gate_from_weenie = "HousingSystem_NeighborhoodGateDID",
        purchase_template = "HousingSystem_PurchaseTemplateDID",
        house_keeper_house = "HouseKeeper_HouseDID",
        // §2.4 other edges
        quest_landmark = "QuestEvent_LandmarkDID",
        // §11 ripcords
        ripcord_world_property = "RipCord_WorldProperty",
        ripcord_table_property = "RipCord_WorldPropertyTableProperty",
        ripcord_array_element = "RipCord_WorldPropertyArrayElement",
        ripcord_category = "RipCord_Category",
        ripcord_name = "RipCord_Name",
        ripcord_help = "RipCord_Help",
        ripcord_ui_element_id = "RipCord_UIElement_ID",
        ripcord_ui_element_layout = "RipCord_UIElement_Layout",
    }
}

// ---------------------------------------------------------------------------------------------
// §2.3 Kinds
// ---------------------------------------------------------------------------------------------

/// The record kinds of §2.3 (Tessera's names), recognised by their properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MapKind {
    /// A travel station (`TravelHomeLocation`, `TravelDestinationRecordArray`).
    TravelWeb,
    /// A travel destination (`TravelDisplayName`, `TravelDestinationIsSwiftTravel`).
    TravelDestination,
    /// A route (`TravelRoute_Destination`, `TravelRoute_ActionArray`).
    TravelRoute,
    /// A route segment (`TravelSegment_HeadName` or `TravelSegment_SubSegmentArray`).
    TravelSegment,
    /// A route hop (`TravelHop_Delay`).
    TravelHop,
    /// A route pad (`TravelPad_Location` and no other property).
    TravelPad,
    /// A travel mode (`TravelMode_PathNodeDistance`).
    TravelMode,
    /// The travel-versioning record (`TravelVersioning_DestinationPairArray`).
    TravelVersioning,
    /// A landmark (`MapNote_Type`, `Landmark_Music`).
    Landmark,
    /// A neighbourhood (`Neighborhood_NeighborhoodTemplate`).
    Neighbourhood,
    /// A neighbourhood template (`NeighborhoodTemplate_HouseList`).
    NeighbourhoodTemplate,
    /// A house list (`HouseList_HouseArray`).
    HouseList,
    /// A house (`House_NeighborhoodTemplate`, `House_Type`).
    House,
    /// A neighbourhood gate (`NeighborhoodGate_NeighborhoodTemplate`).
    NeighbourhoodGate,
    /// A housing hook (`HousingHook_ID`).
    HousingHook,
    /// A ripcord (`RipCord_WorldProperty`).
    Ripcord,
    /// An area (`Area_Territory`, `wstate.md` §11.2 step 1).
    Area,
    /// A territory (`Area_Region`, `wstate.md` §11.1).
    Territory,
    /// A region (`Area_RegionID`, `wstate.md` §11.1).
    Region,
    /// A dungeon (`Dungeon_Name`, `wstate.md` §11.1).
    Dungeon,
}

/// The kind of `set` (§2.3) by its properties, trying the rules in the order of
/// [`MapKind`]'s variants. Never returns [`MapKind::TravelPad`]: a pad is a set whose
/// **only** property is `TravelPad_Location`, which needs the set's entry count; use
/// [`classify_set`] for a [`PropertySet`]. `None` for other sets, including NPCs that carry
/// `TravelPad_Location` among other properties.
#[must_use]
pub fn classify_map_kind<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Option<MapKind> {
    use MapKind as K;
    let rules: [(bool, MapKind); 19] = [
        (
            set.has(p.home_location) && set.has(p.destination_records),
            K::TravelWeb,
        ),
        (
            set.has(p.display_name) && set.has(p.is_swift),
            K::TravelDestination,
        ),
        (
            set.has(p.route_destination) && set.has(p.route_actions),
            K::TravelRoute,
        ),
        (
            set.has(p.segment_head) || set.has(p.segment_subsegments),
            K::TravelSegment,
        ),
        (set.has(p.hop_delay), K::TravelHop),
        (set.has(p.mode_path_node_distance), K::TravelMode),
        (set.has(p.versioning_pairs), K::TravelVersioning),
        (
            set.has(p.note_type) && set.has(p.landmark_music),
            K::Landmark,
        ),
        (set.has(p.neighborhood_template), K::Neighbourhood),
        (set.has(p.template_house_list), K::NeighbourhoodTemplate),
        (set.has(p.house_array), K::HouseList),
        (set.has(p.house_template) && set.has(p.house_type), K::House),
        (set.has(p.gate_template), K::NeighbourhoodGate),
        (set.has(p.hook_id), K::HousingHook),
        (set.has(p.ripcord_world_property), K::Ripcord),
        (set.has(p.area_territory), K::Area),
        (set.has(p.area_region), K::Territory),
        (set.has(p.area_region_id), K::Region),
        (set.has(p.dungeon_name), K::Dungeon),
    ];
    rules.into_iter().find(|&(ok, _)| ok).map(|(_, k)| k)
}

/// [`classify_map_kind`] for a property set, recognising a pad first: a set whose only
/// property is `TravelPad_Location` (§2.3).
#[must_use]
pub fn classify_set(set: &PropertySet, p: &MapPids) -> Option<MapKind> {
    if set.len() == 1 && set.has(p.pad_location) {
        return Some(MapKind::TravelPad);
    }
    classify_map_kind(set, p)
}

/// The map-data links of a zone (§7): the 0x78 sets holding `UI_Map_GameMap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZoneMapData {
    /// `Dungeon_MapData` or `Area_MapData` (non-null).
    pub map: Option<Did>,
    /// `Dungeon_MiniMapData` (non-null).
    pub minimap: Option<Did>,
}

impl ZoneMapData {
    /// Reads the links of a dungeon or area set.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Self {
        Self {
            map: set
                .did_nonnull(p.dungeon_map_data)
                .or_else(|| set.did_nonnull(p.area_map_data)),
            minimap: set.did_nonnull(p.dungeon_minimap_data),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// §5.2, §6 Typed views over notes and entities
// ---------------------------------------------------------------------------------------------

/// One quest role of `QuestDispenser_ActiveQuests` (§5.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestRole {
    /// `QuestDispenser_Quest`: a quest record.
    pub quest: Option<Did>,
    /// `QuestDispenser_RoleName`.
    pub role_name: Option<String>,
}

/// The quest-dispenser properties of a dungeon quest-note row (§5.2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuestNoteInfo {
    /// `QuestDispenser_RoleConstraint`.
    pub role_constraint: Option<String>,
    /// `QuestDispenser_ActiveQuests`, in stored order.
    pub active_quests: Vec<QuestRole>,
}

impl QuestNoteInfo {
    /// Reads the view from a row's `quest_info` set.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Self {
        Self {
            role_constraint: set.string(p.role_constraint).map(str::to_owned),
            active_quests: set
                .structs(p.active_quests)
                .map(|s| QuestRole {
                    quest: s.did_nonnull(p.dispenser_quest),
                    role_name: s.string(p.dispenser_role_name).map(str::to_owned),
                })
                .collect(),
        }
    }
}

/// The map-note properties of a game object (§6.1), for comparison with the table rows
/// (§6.2; the rows are authoritative).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MapNoteProperties {
    /// `MapNote_Type` bits.
    pub note_type: Option<u64>,
    /// `MapNote_Level` bits.
    pub level: Option<u64>,
    /// `MapNote_StringInfo`.
    pub text: Option<StringInfo>,
    /// `MapNote_Icon` (non-null).
    pub icon: Option<Did>,
    /// `MapNote_MinRange`.
    pub min_range: Option<f32>,
    /// `MapNote_MaxRange`.
    pub max_range: Option<f32>,
    /// `MapNote_Discoverable`.
    pub discoverable: Option<u32>,
    /// `MapNote_Enabled`.
    pub enabled: Option<bool>,
    /// `MapNote_Linkable`.
    pub linkable: Option<bool>,
}

impl MapNoteProperties {
    /// Reads the view from an object's property set.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Self {
        Self {
            note_type: set.bitfield(p.note_type),
            level: set.bitfield(p.note_level),
            text: set.string_info(p.note_text).cloned(),
            icon: set.did_nonnull(p.note_icon),
            min_range: set.float(p.note_min_range),
            max_range: set.float(p.note_max_range),
            discoverable: set.enum_key(p.note_discoverable),
            enabled: set.boolean(p.note_enabled),
            linkable: set.boolean(p.note_linkable),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// §8 Travel network
// ---------------------------------------------------------------------------------------------

/// One `TravelDestinationRecord` of a station (§8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TravelOffer {
    /// `TravelDestinationRoute`: the route sold.
    pub route: Did,
    /// `TravelDestinationCost`: the price.
    pub cost: Option<i64>,
    /// `TravelDestinationMinLevel` (defined, never stored here).
    pub min_level: Option<i64>,
}

/// A travel-web record: one station (§8.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TravelWeb {
    /// `TravelHomeLocation`: the station's own destination.
    pub home: Did,
    /// `TravelHomeLocationsArray`: other destinations (or routes) treated as this station.
    pub home_aliases: Vec<Did>,
    /// `TravelDestinationRecordArray`: the routes sold, in stored order. Records without a
    /// route DID are skipped.
    pub offers: Vec<TravelOffer>,
}

impl TravelWeb {
    /// What errors name.
    pub const WHAT: &'static str = "travel-web record";

    /// Reads the view; `TravelHomeLocation` and `TravelDestinationRecordArray` are required.
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Result<Self> {
        let home = set.require_did(p.home_location, Self::WHAT)?;
        let records = set.require_array(p.destination_records, Self::WHAT)?;
        let offers = records
            .elements
            .iter()
            .filter_map(|e| e.value.as_struct())
            .filter_map(|s| {
                Some(TravelOffer {
                    route: s.did_nonnull(p.destination_route)?,
                    cost: s.integer(p.destination_cost),
                    min_level: s.integer(p.destination_min_level),
                })
            })
            .collect();
        Ok(Self {
            home,
            home_aliases: set.dids(p.home_locations).collect(),
            offers,
        })
    }
}

/// A travel destination (§8.3).
#[derive(Debug, Clone, PartialEq)]
pub struct TravelDestination {
    /// `TravelDisplayName`.
    pub display_name: Option<StringInfo>,
    /// `TravelDestinationIsSwiftTravel`.
    pub is_swift: bool,
    /// `TravelDestinationAllowPaperItems`.
    pub allow_paper_items: Option<bool>,
}

impl TravelDestination {
    /// What errors name.
    pub const WHAT: &'static str = "travel destination";

    /// Reads the view; `TravelDestinationIsSwiftTravel` is required.
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Result<Self> {
        Ok(Self {
            display_name: set.string_info(p.display_name).cloned(),
            is_swift: set.require_bool(p.is_swift, Self::WHAT)?,
            allow_paper_items: set.boolean(p.allow_paper_items),
        })
    }
}

/// One `Usage_QuestRequirement` of a route (§8.4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestRequirement {
    /// `Usage_QuestID`.
    pub quest: Option<Did>,
    /// `Usage_QuestStatus` (meaning OPEN, §8.8).
    pub status: Option<u32>,
    /// `Usage_Operator` (meaning OPEN, §8.8).
    pub operator: Option<u32>,
}

/// `Usage_RequiredFaction` of a route (§8.4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactionRequirement {
    /// `Usage_RequiredFaction_DataID`.
    pub faction: Option<Did>,
    /// `Usage_RequiredFaction_Tier`.
    pub tier: Option<u32>,
}

/// A travel route (§8.4).
#[derive(Debug, Clone, PartialEq)]
pub struct TravelRoute {
    /// `Name`.
    pub name: Option<StringInfo>,
    /// `TravelRoute_Destination`.
    pub destination: Did,
    /// `TravelRoute_TravelMode`.
    pub mode: Option<Did>,
    /// `TravelRoute_ActionArray`, in travel order (null DIDs skipped).
    pub actions: Vec<Did>,
    /// `Usage_MinLevel`.
    pub min_level: Option<i64>,
    /// `Usage_AllowedWhileMounted`.
    pub allowed_while_mounted: Option<bool>,
    /// `Usage_RequiresSubscriberOrUnsub`.
    pub requires_subscriber: Option<bool>,
    /// `Usage_QuestRequirements`.
    pub quest_requirements: Vec<QuestRequirement>,
    /// `Usage_RequiredFaction`.
    pub required_faction: Option<FactionRequirement>,
    /// `Usage_Permission`.
    pub permission: Option<Did>,
}

impl TravelRoute {
    /// What errors name.
    pub const WHAT: &'static str = "travel route";

    /// Reads the view; `TravelRoute_Destination` and `TravelRoute_ActionArray` are required.
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Result<Self> {
        let destination = set.require_did(p.route_destination, Self::WHAT)?;
        let actions = set.require_array(p.route_actions, Self::WHAT)?;
        Ok(Self {
            name: set.string_info(p.name).cloned(),
            destination,
            mode: set.did_nonnull(p.route_mode),
            actions: actions
                .elements
                .iter()
                .filter_map(ValueExt::did_nonnull)
                .collect(),
            min_level: set.integer(p.min_level),
            allowed_while_mounted: set.boolean(p.allowed_while_mounted),
            requires_subscriber: set.boolean(p.requires_subscriber),
            quest_requirements: set
                .structs(p.quest_requirements)
                .map(|s| QuestRequirement {
                    quest: s.did_nonnull(p.quest_id),
                    status: s.enum_key(p.quest_status),
                    operator: s.enum_key(p.quest_operator),
                })
                .collect(),
            required_faction: set
                .structure(p.required_faction)
                .map(|s| FactionRequirement {
                    faction: s.did_nonnull(p.required_faction_did),
                    tier: s.enum_key(p.required_faction_tier),
                }),
            permission: set.did_nonnull(p.permission),
        })
    }
}

/// The kind of one route action (§8.4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouteAction {
    /// A segment (S).
    Segment,
    /// A hop (H).
    Hop,
    /// A pad (P).
    Pad,
}

impl RouteAction {
    /// The action kind of `kind`, if it is one.
    #[must_use]
    pub fn of(kind: MapKind) -> Option<Self> {
        match kind {
            MapKind::TravelSegment => Some(Self::Segment),
            MapKind::TravelHop => Some(Self::Hop),
            MapKind::TravelPad => Some(Self::Pad),
            _ => None,
        }
    }
}

/// The pattern of an action list in the notation of §8.4.2: `S` a segment, `S+` two or more
/// segments in a row, `H` a hop, `P` a pad, separated by spaces.
#[must_use]
pub fn route_pattern(actions: &[RouteAction]) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let mut i = 0;
    while let Some(&a) = actions.get(i) {
        let mut run = 1;
        if a == RouteAction::Segment {
            while actions.get(i + run) == Some(&RouteAction::Segment) {
                run += 1;
            }
        }
        parts.push(match (a, run) {
            (RouteAction::Segment, 1) => "S",
            (RouteAction::Segment, _) => "S+",
            (RouteAction::Hop, _) => "H",
            (RouteAction::Pad, _) => "P",
        });
        i += run;
    }
    parts.join(" ")
}

/// A route segment (§8.5.1): a head name, or sub-segments, never both (MI11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TravelSegment {
    /// `TravelSegment_HeadName`: the `Segment_HeadName` of the path's first SegmentNode.
    pub head_name: Option<String>,
    /// `TravelSegment_SubSegmentArray`: segments ridden in order.
    pub sub_segments: Vec<Did>,
}

impl TravelSegment {
    /// Reads the view.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Self {
        Self {
            head_name: set.string(p.segment_head).map(str::to_owned),
            sub_segments: set.dids(p.segment_subsegments).collect(),
        }
    }

    /// Whether the segment has exactly one of a head name and sub-segments (MI11).
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.head_name.is_some() != !self.sub_segments.is_empty()
    }
}

/// A route hop (§8.5.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TravelHop {
    /// `TravelHop_Delay` (unit OPEN).
    pub delay: Option<f32>,
    /// `TravelHop_SceneID`.
    pub scene: Option<Did>,
}

impl TravelHop {
    /// Reads the view.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Self {
        Self {
            delay: set.float(p.hop_delay),
            scene: set.did_nonnull(p.hop_scene),
        }
    }
}

/// A route pad (§8.5.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TravelPad {
    /// `TravelPad_Location`: a place key (§3).
    pub location: String,
}

impl TravelPad {
    /// What errors name.
    pub const WHAT: &'static str = "travel pad";

    /// Reads the view; `TravelPad_Location` is required.
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Result<Self> {
        let location = set.require_as(
            p.pad_location,
            Self::WHAT,
            crate::PropertyType::String,
            ValueExt::string,
        )?;
        Ok(Self {
            location: location.to_owned(),
        })
    }
}

/// A travel mode (§8.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TravelMode {
    /// `TravelMode_PathNodeDistance`.
    pub path_node_distance: Option<f32>,
    /// `TravelMode_TurnSpeed`.
    pub turn_speed: Option<f32>,
    /// `TravelMode_TurnAcceleration`.
    pub turn_acceleration: Option<f32>,
    /// `TravelMode_MoveAcceleration`.
    pub move_acceleration: Option<f32>,
    /// `TravelMode_WalkDistance`.
    pub walk_distance: Option<f32>,
    /// `TravelMode_TravelEffectDID`.
    pub effect: Option<Did>,
    /// `TravelMode_ScriptPants`.
    pub script_pants: Option<u32>,
}

impl TravelMode {
    /// Reads the view.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Self {
        Self {
            path_node_distance: set.float(p.mode_path_node_distance),
            turn_speed: set.float(p.mode_turn_speed),
            turn_acceleration: set.float(p.mode_turn_acceleration),
            move_acceleration: set.float(p.mode_move_acceleration),
            walk_distance: set.float(p.mode_walk_distance),
            effect: set.did_nonnull(p.mode_effect),
            script_pants: set.enum_key(p.mode_script_pants),
        }
    }
}

/// The travel-versioning record (§8.7): (slow, swift) destination pairs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TravelVersioning {
    /// The pairs, in stored order.
    pub pairs: Vec<(Option<Did>, Option<Did>)>,
}

impl TravelVersioning {
    /// Reads the view.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Self {
        Self {
            pairs: set
                .structs(p.versioning_pairs)
                .map(|s| {
                    (
                        s.did_nonnull(p.versioning_slow),
                        s.did_nonnull(p.versioning_swift),
                    )
                })
                .collect(),
        }
    }
}

/// The travel properties of a stable-master NPC (§8.7).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StableMaster {
    /// `TravelWebWC`: the station.
    pub station: Option<Did>,
    /// `TravelPad_Location`: where the NPC stands (a place key).
    pub pad: Option<String>,
    /// `Travel_DiscountArray`: PIDs of discount properties (use OPEN, §8.8).
    pub discounts: Vec<u32>,
    /// `TravelWebSellMultiplier` (use OPEN, §8.8).
    pub sell_multiplier: Option<f32>,
    /// `Travel_Stablemaster_MustBeDiscovered`.
    pub must_be_discovered: Option<bool>,
}

impl StableMaster {
    /// Reads the view; `None` unless the set names a station.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Option<Self> {
        let station = set.did_nonnull(p.web_wc)?;
        Some(Self {
            station: Some(station),
            pad: set.string(p.pad_location).map(str::to_owned),
            discounts: set
                .elements(p.discounts)
                .iter()
                .filter_map(ValueExt::property_ref)
                .collect(),
            sell_multiplier: set.float(p.sell_multiplier),
            must_be_discovered: set.boolean(p.must_be_discovered),
        })
    }
}

/// The stations of the `TravelWebDirectory` singleton (§8.7), in stored order.
#[must_use]
pub fn travel_directory<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Vec<Did> {
    set.dids(p.web_array).collect()
}

// ---------------------------------------------------------------------------------------------
// §9 Neighbourhoods and housing
// ---------------------------------------------------------------------------------------------

/// The neighbourhoods of the `NeighborhoodDirectory` singleton (§9.2), in stored order.
#[must_use]
pub fn neighbourhood_directory<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Vec<Did> {
    set.dids(p.neighborhood_directory).collect()
}

/// A neighbourhood: one named copy of a template (§9.2).
#[derive(Debug, Clone, PartialEq)]
pub struct Neighbourhood {
    /// `Neighborhood_Name`.
    pub name: Option<StringInfo>,
    /// `Neighborhood_NeighborhoodTemplate`.
    pub template: Did,
}

impl Neighbourhood {
    /// What errors name.
    pub const WHAT: &'static str = "neighbourhood";

    /// Reads the view; the template is required.
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Result<Self> {
        Ok(Self {
            name: set.string_info(p.neighborhood_name).cloned(),
            template: set.require_did(p.neighborhood_template, Self::WHAT)?,
        })
    }
}

/// A neighbourhood template (§9.3).
#[derive(Debug, Clone, PartialEq)]
pub struct NeighbourhoodTemplate {
    /// `NeighborhoodTemplate_Name`.
    pub name: Option<StringInfo>,
    /// `NeighborhoodTemplate_HouseList`.
    pub house_list: Did,
    /// `NeighborhoodTemplate_Boundaries`: landblock positions (region and landblock only).
    pub boundaries: Vec<Position>,
    /// `NeighborhoodTemplate_Telepad`: entrance place key.
    pub telepad: Option<String>,
    /// `NeighborhoodTemplate_BootPosition`: exit place key.
    pub boot_position: Option<String>,
    /// `NeighborhoodTemplate_SceneID`.
    pub scene: Option<Did>,
    /// `NeighborhoodTemplate_InitialInstances`.
    pub initial_instances: Option<i64>,
    /// `NeighborhoodTemplate_OccupancyThresholdPercentage`.
    pub occupancy_threshold: Option<f32>,
    /// `NeighborhoodTemplate_RolloutStep`.
    pub rollout_step: Option<i64>,
}

impl NeighbourhoodTemplate {
    /// What errors name.
    pub const WHAT: &'static str = "neighbourhood template";

    /// Reads the view; the house list is required.
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Result<Self> {
        Ok(Self {
            name: set.string_info(p.template_name).cloned(),
            house_list: set.require_did(p.template_house_list, Self::WHAT)?,
            boundaries: set
                .elements(p.template_boundaries)
                .iter()
                .filter_map(|e| e.position().copied())
                .collect(),
            telepad: set.string(p.template_telepad).map(str::to_owned),
            boot_position: set.string(p.template_boot_position).map(str::to_owned),
            scene: set.did_nonnull(p.template_scene),
            initial_instances: set.integer(p.template_initial_instances),
            occupancy_threshold: set.float(p.template_occupancy_threshold),
            rollout_step: set.integer(p.template_rollout_step),
        })
    }
}

/// A house list (§9.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HouseList {
    /// `HouseList_HouseArray`, in stored order.
    pub houses: Vec<Did>,
}

impl HouseList {
    /// Reads the view.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Self {
        Self {
            houses: set.dids(p.house_array).collect(),
        }
    }
}

/// A house (§9.4).
#[derive(Debug, Clone, PartialEq)]
pub struct House {
    /// `House_Address`.
    pub address: Option<StringInfo>,
    /// `House_Description`.
    pub description: Option<StringInfo>,
    /// `House_NeighborhoodTemplate`.
    pub template: Did,
    /// `House_Telepad`: the plot's place key.
    pub telepad: Option<String>,
    /// `House_Type`.
    pub house_type: Option<u32>,
    /// `House_ValueTier`.
    pub value_tier: Option<u32>,
    /// `House_IsKinshipHouse`.
    pub is_kinship: Option<bool>,
    /// `House_IsPremiumHouse`.
    pub is_premium: Option<bool>,
    /// `House_CharacteristicArray`.
    pub characteristics: Vec<Did>,
}

impl House {
    /// What errors name.
    pub const WHAT: &'static str = "house";

    /// Reads the view; the template is required.
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Result<Self> {
        Ok(Self {
            address: set.string_info(p.house_address).cloned(),
            description: set.string_info(p.house_description).cloned(),
            template: set.require_did(p.house_template, Self::WHAT)?,
            telepad: set.string(p.house_telepad).map(str::to_owned),
            house_type: set.enum_key(p.house_type),
            value_tier: set.enum_key(p.house_value_tier),
            is_kinship: set.boolean(p.house_is_kinship),
            is_premium: set.boolean(p.house_is_premium),
            characteristics: set.dids(p.house_characteristics).collect(),
        })
    }
}

/// A neighbourhood gate (§9.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeighbourhoodGate {
    /// `NeighborhoodGate_NeighborhoodTemplate`.
    pub template: Did,
    /// `NeighborhoodGate_Telepad`: a place key.
    pub telepad: Option<String>,
}

impl NeighbourhoodGate {
    /// What errors name.
    pub const WHAT: &'static str = "neighbourhood gate";

    /// Reads the view; the template is required.
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Result<Self> {
        Ok(Self {
            template: set.require_did(p.gate_template, Self::WHAT)?,
            telepad: set.string(p.gate_telepad).map(str::to_owned),
        })
    }
}

/// A housing hook (§9.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HousingHook {
    /// `HousingHook_ID`.
    pub id: Option<u32>,
    /// `HousingHook_HookCategory` bits (bit *i* is enum key *i* + 1).
    pub category: Option<u64>,
    /// `HousingHook_IsPropertyHook`.
    pub is_property_hook: Option<bool>,
}

impl HousingHook {
    /// Reads the view.
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Self {
        Self {
            id: set.enum_key(p.hook_id),
            category: set.bitfield(p.hook_category),
            is_property_hook: set.boolean(p.hook_is_property),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// §10 Place references, §11 ripcords
// ---------------------------------------------------------------------------------------------

/// One string value that names a place (§10).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaceReference<'a> {
    /// PID of the string's own entry.
    pub pid: u32,
    /// PID of the enclosing struct or array; `None` at top level.
    pub parent: Option<u32>,
    /// The string as stored.
    pub key: &'a str,
    /// Index of the place in the table.
    pub place: usize,
}

/// Calls `f` for every string value of `set` (nested values included) that is a key of
/// `places`, ignoring case (§10: how the place-naming properties were found).
pub fn for_each_place_reference<'a>(
    set: &'a PropertySet,
    places: &PlacesTable,
    mut f: impl FnMut(PlaceReference<'a>),
) {
    set.walk(|v| {
        let PropertyValue::String(s) = &v.entry.value else {
            return;
        };
        let units: Vec<u16> = s.encode_utf16().collect();
        if let Some(place) = places.find(&units) {
            let parent = match v.context {
                EntryContext::TopLevel => None,
                EntryContext::StructField { parent } | EntryContext::ArrayElement { parent } => {
                    Some(parent)
                }
            };
            f(PlaceReference {
                pid: v.entry.pid,
                parent,
                key: s,
                place,
            });
        }
    });
}

/// A ripcord record (§11).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ripcord {
    /// `RipCord_WorldProperty`: the world property switched (a PID).
    pub world_property: Option<u32>,
    /// `RipCord_WorldPropertyTableProperty` (second ripcord class), as an integer.
    pub table_property: Option<u64>,
    /// `RipCord_WorldPropertyArrayElement` (second ripcord class), as an integer.
    pub array_element: Option<u64>,
    /// `RipCord_Category`.
    pub category: Option<u32>,
    /// `RipCord_Name`.
    pub name: Option<String>,
    /// `RipCord_Help`.
    pub help: Option<String>,
    /// `RipCord_UIElement_ID`, as an integer.
    pub ui_element_id: Option<u64>,
    /// `RipCord_UIElement_Layout`, as an integer.
    pub ui_element_layout: Option<u64>,
}

impl Ripcord {
    /// Reads the view. Fields whose value type the spec does not give are read as the
    /// integer bit pattern of a scalar ([`PropertyValue::as_bits`]).
    #[must_use]
    pub fn read<A: PropertyAccess + ?Sized>(set: &A, p: &MapPids) -> Self {
        let bits = |pid| set.get_pid(pid).and_then(PropertyValue::as_bits);
        Self {
            world_property: set
                .property_ref(p.ripcord_world_property)
                .or_else(|| bits(p.ripcord_world_property).and_then(|b| u32::try_from(b).ok())),
            table_property: bits(p.ripcord_table_property),
            array_element: bits(p.ripcord_array_element),
            category: set.enum_key(p.ripcord_category),
            name: set.string(p.ripcord_name).map(str::to_owned),
            help: set.string(p.ripcord_help).map(str::to_owned),
            ui_element_id: bits(p.ripcord_ui_element_id),
            ui_element_layout: bits(p.ripcord_ui_element_layout),
        }
    }
}

/// The struct fields of a `MapNote_GameSpecific` value in a row's `extra` set (§4.5).
#[must_use]
pub fn game_specific<'a>(extra: &'a PropertySet, p: &MapPids) -> Option<&'a PropertyStruct> {
    extra.structure(p.note_game_specific)
}
