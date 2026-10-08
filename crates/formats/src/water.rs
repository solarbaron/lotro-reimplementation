// SPDX-License-Identifier: GPL-3.0-or-later
//! Water tables (prefix 0x16) and fog-zone tables (prefix 0x17) named by the region
//! descriptor (`water.md` §4–§5).
//!
//! Note the crossed DB type codes: prefix 0x16 holds DB type 0x17 WATER_DESC and prefix 0x17
//! holds DB type 0x16 FOG_DESC (`water.md` §3).
//!
//! Both tables list keyed entries after an optional default entry. Look-ups use the
//! **first** entry with a key in stored order and fall back to the default
//! (`water.md` §4.4, §5.3); each table builds a key index when it is decoded. Deciding a
//! landblock's water (§6) and fog zone (§5.3) is left to the consumer.
//!
//! Hostile input (`water.md` §4.4, §11): counts are bounded by the bytes left, bool8 fields
//! other than 0 or 1 fail with [`FormatError::InvalidBool`], and a non-finite water `value`
//! or fog-zone float fails with [`FormatError::Malformed`]. Fog-zone tables are read with the
//! newest layout (every version-dependent field present, §5.2).

use crate::did::{Did, DidKind};
use crate::environment::{put, read_colour};
use crate::error::{FormatError, Result};
use crate::reader::{PREALLOC_CAP, Reader};
use crate::record::{Record, read_prologue};
use crate::terrain_type::Argb;

/// Type prefix of water tables.
pub const WATER_TABLE_PREFIX: u8 = 0x16;

/// Type prefix of fog-zone tables.
pub const FOG_ZONE_TABLE_PREFIX: u8 = 0x17;

/// Stored size of a dry water-type entry (`water.md` §4.2).
pub const DRY_WATER_ENTRY_LEN: usize = 5;

/// Stored size of a wet water-type entry (`water.md` §4.2).
pub const WET_WATER_ENTRY_LEN: usize = 33;

/// Stored size of a fog-zone entry without `value_e` (`water.md` §5.2).
pub const FOG_ZONE_ENTRY_LEN: usize = 123;

/// Number of override triples in a fog-zone entry.
pub const FOG_ZONE_OVERRIDES: usize = 7;

/// Sorted (key, entry position) pairs, one per distinct key, pointing at its first entry in
/// stored order.
fn key_index(keys: impl Iterator<Item = u32>) -> Vec<(u32, u32)> {
    let mut index: Vec<(u32, u32)> = keys
        .enumerate()
        .map(|(i, k)| (k, u32::try_from(i).unwrap_or(u32::MAX)))
        .collect();
    // A stable sort keeps equal keys in stored order; `dedup_by_key` keeps the first.
    index.sort_by_key(|&(k, _)| k);
    index.dedup_by_key(|&mut (k, _)| k);
    index
}

/// [`key_index`] for a decoder: the index (at most one pair per entry) is charged to the
/// reader's budget first.
fn charged_key_index(
    r: &mut Reader<'_>,
    keys: impl ExactSizeIterator<Item = u32>,
) -> Result<Vec<(u32, u32)>> {
    r.charge(keys.len().saturating_mul(std::mem::size_of::<(u32, u32)>()))?;
    Ok(key_index(keys))
}

fn lookup(index: &[(u32, u32)], key: u32) -> Option<usize> {
    index
        .binary_search_by_key(&key, |&(k, _)| k)
        .ok()
        .and_then(|i| index.get(i))
        .map(|&(_, pos)| pos as usize)
}

/// Reads an f32 and rejects NaN and infinities (`water.md` §11).
fn finite_f32(r: &mut Reader<'_>, what: &'static str) -> Result<f32> {
    let offset = r.position();
    let v = r.f32()?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(FormatError::malformed(what, offset, "non-finite float"))
    }
}

// ---------------------------------------------------------------------------------------
// Water table
// ---------------------------------------------------------------------------------------

/// The fields of a water type that has water (`water.md` §4.2 fields 3–6).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WetWater {
    /// Surface record (prefix 0x31) of the water surface (`water.md` §8.2).
    pub surface: Did,
    /// Four further DIDs (no reader found; 0 in every observed entry).
    pub extra: [Did; 4],
    /// Entity descriptor (prefix 0x47) of the water-volume entity (`water.md` §7.1).
    pub volume_entity: Did,
    /// Copied into the landblock's water state; no reader found (24.0 observed). Finite in
    /// a decoded entry: the decoder rejects a non-finite value (`water.md` §11), so a caller
    /// that builds an entry must keep it finite for [`WaterTable::to_bytes`] to produce
    /// bytes that decode again.
    pub value: f32,
}

/// A water-type entry (`water.md` §4.2): 5 bytes when dry, 33 when wet.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WaterType {
    /// Water-type number, compared with the block map's type value (`water.md` §6.2).
    pub key: u32,
    /// `None` when the entry is "dry" (no water); otherwise its fields.
    pub wet: Option<WetWater>,
}

impl WaterType {
    /// Whether this type means "no water".
    #[must_use]
    pub fn is_dry(&self) -> bool {
        self.wet.is_none()
    }

    /// Whether the entry's encoding decodes again: dry, or a finite `value` (`water.md`
    /// §11).
    #[must_use]
    pub fn is_encodable(&self) -> bool {
        self.wet.is_none_or(|w| w.value.is_finite())
    }

    /// Stored size in bytes.
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        if self.is_dry() {
            DRY_WATER_ENTRY_LEN
        } else {
            WET_WATER_ENTRY_LEN
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let key = r.u32()?;
        let dry = r.bool8()?;
        if dry {
            return Ok(Self { key, wet: None });
        }
        let surface = r.did()?;
        let extra = [r.did()?, r.did()?, r.did()?, r.did()?];
        let volume_entity = r.did()?;
        let value = finite_f32(r, "water type value")?;
        Ok(Self {
            key,
            wet: Some(WetWater {
                surface,
                extra,
                volume_entity,
                value,
            }),
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        debug_assert!(
            self.is_encodable(),
            "water type {} has a non-finite value; its encoding would not decode",
            self.key
        );
        put::u32(out, self.key);
        put::bool8(out, self.wet.is_none());
        if let Some(w) = &self.wet {
            put::did(out, w.surface);
            for d in w.extra {
                put::did(out, d);
            }
            put::did(out, w.volume_entity);
            put::f32(out, w.value);
        }
    }
}

/// A water table (prefix 0x16, DB type 0x17 WATER_DESC; `water.md` §4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WaterTable {
    /// Own DID.
    pub did: Did,
    /// Offset 4 (no reader found; 0 observed).
    pub reserved: u32,
    /// Block-map key whose value is the water level (`water.md` §6.1).
    pub level_key: u32,
    /// Block-map key whose value is the water type (`water.md` §6.1).
    pub type_key: u32,
    /// The default entry, used when the type matches no listed entry.
    pub default: Option<WaterType>,
    entries: Vec<WaterType>,
    /// (key, position) of the first entry of each key, sorted by key.
    index: Vec<(u32, u32)>,
}

impl WaterTable {
    /// A table with these fields; the key index is built here.
    #[must_use]
    pub fn new(
        did: Did,
        reserved: u32,
        level_key: u32,
        type_key: u32,
        default: Option<WaterType>,
        entries: Vec<WaterType>,
    ) -> Self {
        let index = key_index(entries.iter().map(|e| e.key));
        Self {
            did,
            reserved,
            level_key,
            type_key,
            default,
            entries,
            index,
        }
    }

    /// The listed entries in stored order (the default is not among them).
    #[must_use]
    pub fn entries(&self) -> &[WaterType] {
        &self.entries
    }

    /// The first listed entry with `key`, without the default fallback.
    #[must_use]
    pub fn get(&self, key: u32) -> Option<&WaterType> {
        lookup(&self.index, key).and_then(|i| self.entries.get(i))
    }

    /// The entry a block-map type value selects: the first listed entry with that key, else
    /// the default (`water.md` §4.1, §6.2). `None` if neither exists.
    #[must_use]
    pub fn select(&self, key: u32) -> Option<&WaterType> {
        self.get(key).or(self.default.as_ref())
    }

    /// Re-encodes the record (`water.md` §4.1); byte-exact for a decoded record except that
    /// the count is written as a shortest VLE. The output decodes again when every entry
    /// [`is_encodable`](WaterType::is_encodable) (always true for a decoded table); a
    /// non-finite `value` is written as stored and makes the decoder reject the bytes
    /// (debug builds assert against it).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            23 + self
                .entries
                .iter()
                .map(WaterType::encoded_len)
                .sum::<usize>(),
        );
        put::did(&mut out, self.did);
        put::u32(&mut out, self.reserved);
        put::u32(&mut out, self.level_key);
        put::u32(&mut out, self.type_key);
        put::bool8(&mut out, self.default.is_some());
        if let Some(d) = &self.default {
            d.write(&mut out);
        }
        put::vle_count(&mut out, self.entries.len());
        for e in &self.entries {
            e.write(&mut out);
        }
        out
    }
}

impl Record<'_> for WaterTable {
    const NAME: &'static str = "water table";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::WaterTable)?;
        let reserved = r.u32()?;
        let level_key = r.u32()?;
        let type_key = r.u32()?;
        let default = if r.bool8()? {
            Some(WaterType::read(&mut r)?)
        } else {
            None
        };
        let count = r.vle()?;
        let mut entries = r.alloc_vec(count, DRY_WATER_ENTRY_LEN, PREALLOC_CAP)?;
        for _ in 0..count {
            let e = WaterType::read(&mut r)?;
            r.push(&mut entries, e, count)?;
        }
        r.expect_end(Self::NAME)?;
        let index = charged_key_index(&mut r, entries.iter().map(|e| e.key))?;
        Ok(Self {
            did,
            reserved,
            level_key,
            type_key,
            default,
            entries,
            index,
        })
    }
}

/// Decodes a water table payload (for fuzz targets and tools).
pub fn decode_water_table(data: &[u8]) -> Result<WaterTable> {
    WaterTable::parse(data)
}

// ---------------------------------------------------------------------------------------
// Fog-zone table
// ---------------------------------------------------------------------------------------

/// An override triple of a fog-zone entry (`water.md` §5.2 field 2, 9 bytes; meaning OPEN).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FogOverride {
    /// Mode byte (3, 4 or 6 observed).
    pub mode: u8,
    /// First u32 (0 observed).
    pub a: u32,
    /// Second u32 (0 observed).
    pub b: u32,
}

/// A fog-zone entry (`water.md` §5.2; meanings OPEN unless stated).
///
/// Every f32 (`near`, `far`, `amount`, `value_60`, `light_a`, `light_b`) is finite in a
/// decoded entry, and the decoder rejects a non-finite one (`water.md` §11). The fields are
/// public and not checked on construction: a caller that builds an entry must keep them
/// finite, or [`FogZoneTable::to_bytes`] produces bytes that its own decoder rejects
/// ([`FogZone::is_encodable`] tells).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FogZone {
    /// Key compared with the block map's zone value.
    pub key: u32,
    /// Seven override triples.
    pub overrides: [FogOverride; FOG_ZONE_OVERRIDES],
    /// Fog near distance.
    pub near: f32,
    /// Fog far distance.
    pub far: f32,
    /// Fog amount.
    pub amount: f32,
    /// Field 6 (1.0 observed).
    pub value_60: f32,
    /// Raw `active` byte; the client uses a zone only when it is non-zero (BIN).
    pub active: u8,
    /// Fog colour.
    pub colour: Argb,
    /// Field 9.
    pub light_a: f32,
    /// Field 10.
    pub colour_a1: Argb,
    /// Field 11 (archive version ≥ 4; always present in the newest layout).
    pub colour_a2: Argb,
    /// Field 12.
    pub light_b: f32,
    /// Field 13.
    pub colour_b: Argb,
    /// Field 14 (version ≥ 2).
    pub colour_c: Argb,
    /// Field 15 (version ≥ 2).
    pub flag_c: bool,
    /// Field 16.
    pub colour_d: Argb,
    /// Fields 17–18 (version ≥ 3): the DID `did_e` and the u32 `value_e`, which is stored
    /// exactly when `did_e` is not 0 (`water.md` §5.2). `None` when the stored `did_e` is 0.
    /// A decoded `Some` always has a non-zero DID; [`FogZoneTable::to_bytes`] writes a
    /// `Some` with a zero DID as `did_e` 0 without `value_e`, so it decodes as `None`.
    pub e: Option<(Did, u32)>,
    /// Field 19.
    pub flag_f: bool,
    /// Field 20.
    pub value_g: u8,
}

impl FogZone {
    /// Whether the client would use this zone (`active` ≠ 0, `water.md` §5.3).
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active != 0
    }

    /// Stored size in bytes (123, or 127 with `value_e`).
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        FOG_ZONE_ENTRY_LEN + if self.stored_e().is_some() { 4 } else { 0 }
    }

    /// Whether every f32 of the entry is finite, so that its encoding decodes again
    /// (`water.md` §11).
    #[must_use]
    pub fn is_encodable(&self) -> bool {
        [
            self.near,
            self.far,
            self.amount,
            self.value_60,
            self.light_a,
            self.light_b,
        ]
        .iter()
        .all(|v| v.is_finite())
    }

    /// Fields 17–18 as they are stored: `None` also for a `Some` with a zero DID.
    fn stored_e(&self) -> Option<(Did, u32)> {
        self.e.filter(|(d, _)| !d.is_null())
    }

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        const WHAT: &str = "fog-zone float";
        let key = r.u32()?;
        let mut overrides = [FogOverride::default(); FOG_ZONE_OVERRIDES];
        for o in &mut overrides {
            *o = FogOverride {
                mode: r.u8()?,
                a: r.u32()?,
                b: r.u32()?,
            };
        }
        let near = finite_f32(r, WHAT)?;
        let far = finite_f32(r, WHAT)?;
        let amount = finite_f32(r, WHAT)?;
        let value_60 = finite_f32(r, WHAT)?;
        let active = r.u8()?;
        let colour = read_colour(r)?;
        let light_a = finite_f32(r, WHAT)?;
        let colour_a1 = read_colour(r)?;
        let colour_a2 = read_colour(r)?;
        let light_b = finite_f32(r, WHAT)?;
        let colour_b = read_colour(r)?;
        let colour_c = read_colour(r)?;
        let flag_c = r.bool8()?;
        let colour_d = read_colour(r)?;
        let did_e = r.did()?;
        let e = if did_e.is_null() {
            None
        } else {
            Some((did_e, r.u32()?))
        };
        let flag_f = r.bool8()?;
        let value_g = r.u8()?;
        Ok(Self {
            key,
            overrides,
            near,
            far,
            amount,
            value_60,
            active,
            colour,
            light_a,
            colour_a1,
            colour_a2,
            light_b,
            colour_b,
            colour_c,
            flag_c,
            colour_d,
            e,
            flag_f,
            value_g,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        debug_assert!(
            self.is_encodable(),
            "fog-zone entry {} has a non-finite float; its encoding would not decode",
            self.key
        );
        put::u32(out, self.key);
        for o in &self.overrides {
            put::u8(out, o.mode);
            put::u32(out, o.a);
            put::u32(out, o.b);
        }
        put::f32s(out, &[self.near, self.far, self.amount, self.value_60]);
        put::u8(out, self.active);
        put::colour(out, self.colour);
        put::f32(out, self.light_a);
        put::colour(out, self.colour_a1);
        put::colour(out, self.colour_a2);
        put::f32(out, self.light_b);
        put::colour(out, self.colour_b);
        put::colour(out, self.colour_c);
        put::bool8(out, self.flag_c);
        put::colour(out, self.colour_d);
        // `value_e` is written exactly when `did_e` is not 0, so the output always decodes.
        match self.stored_e() {
            Some((d, v)) => {
                put::did(out, d);
                put::u32(out, v);
            }
            None => put::did(out, Did(0)),
        }
        put::bool8(out, self.flag_f);
        put::u8(out, self.value_g);
    }
}

/// A fog-zone table (prefix 0x17, DB type 0x16 FOG_DESC; `water.md` §5).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FogZoneTable {
    /// Own DID.
    pub did: Did,
    /// Offset 4 (0 observed).
    pub reserved: u32,
    /// Block-map key whose value selects the fog zone (`water.md` §5.3).
    pub zone_key: u32,
    /// The default entry, used when the zone value matches no listed entry.
    pub default: Option<FogZone>,
    entries: Vec<FogZone>,
    /// (key, position) of the first entry of each key, sorted by key.
    index: Vec<(u32, u32)>,
}

impl FogZoneTable {
    /// A table with these fields; the key index is built here.
    #[must_use]
    pub fn new(
        did: Did,
        reserved: u32,
        zone_key: u32,
        default: Option<FogZone>,
        entries: Vec<FogZone>,
    ) -> Self {
        let index = key_index(entries.iter().map(|e| e.key));
        Self {
            did,
            reserved,
            zone_key,
            default,
            entries,
            index,
        }
    }

    /// The listed entries in stored order (the default is not among them).
    #[must_use]
    pub fn entries(&self) -> &[FogZone] {
        &self.entries
    }

    /// The first listed entry with `key`, without the default fallback.
    #[must_use]
    pub fn get(&self, key: u32) -> Option<&FogZone> {
        lookup(&self.index, key).and_then(|i| self.entries.get(i))
    }

    /// The entry a block-map zone value selects: the first listed entry with that key, else
    /// the default (`water.md` §5.3). `None` if neither exists.
    #[must_use]
    pub fn select(&self, key: u32) -> Option<&FogZone> {
        self.get(key).or(self.default.as_ref())
    }

    /// Whether any entry (the default included) is active. When none is, no landblock can
    /// have a fog zone (`water.md` §5.3, §5.4).
    #[must_use]
    pub fn any_active(&self) -> bool {
        self.default
            .iter()
            .chain(&self.entries)
            .any(FogZone::is_active)
    }

    /// Re-encodes the record (`water.md` §5.1); byte-exact for a decoded record except that
    /// the count is written as a shortest VLE. An entry whose `e` is `Some` with a zero DID
    /// is written without `value_e` ([`FogZone::e`]). The output decodes again when every
    /// entry [`is_encodable`](FogZone::is_encodable) (always true for a decoded table); a
    /// non-finite float is written as stored and makes the decoder reject the bytes (debug
    /// builds assert against it).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            14 + FOG_ZONE_ENTRY_LEN + self.entries.iter().map(FogZone::encoded_len).sum::<usize>(),
        );
        put::did(&mut out, self.did);
        put::u32(&mut out, self.reserved);
        put::u32(&mut out, self.zone_key);
        put::bool8(&mut out, self.default.is_some());
        if let Some(d) = &self.default {
            d.write(&mut out);
        }
        put::vle_count(&mut out, self.entries.len());
        for e in &self.entries {
            e.write(&mut out);
        }
        out
    }
}

impl Record<'_> for FogZoneTable {
    const NAME: &'static str = "fog-zone table";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::FogZoneTable)?;
        let reserved = r.u32()?;
        let zone_key = r.u32()?;
        let default = if r.bool8()? {
            Some(FogZone::read(&mut r)?)
        } else {
            None
        };
        let count = r.vle()?;
        let mut entries = r.alloc_vec(count, FOG_ZONE_ENTRY_LEN, PREALLOC_CAP)?;
        for _ in 0..count {
            let e = FogZone::read(&mut r)?;
            r.push(&mut entries, e, count)?;
        }
        r.expect_end(Self::NAME)?;
        let index = charged_key_index(&mut r, entries.iter().map(|e| e.key))?;
        Ok(Self {
            did,
            reserved,
            zone_key,
            default,
            entries,
            index,
        })
    }
}

/// Decodes a fog-zone table payload (for fuzz targets and tools).
pub fn decode_fog_zone_table(data: &[u8]) -> Result<FogZoneTable> {
    FogZoneTable::parse(data)
}
