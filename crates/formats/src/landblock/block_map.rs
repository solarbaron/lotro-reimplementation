// SPDX-License-Identifier: GPL-3.0-or-later
//! Block maps `0x801RXXYY`, block-map descriptors (prefix 0x18) and region descriptors
//! (prefix 0x0F).
//!
//! Spec: `docs/specs/landblock.md` §5; `docs/specs/environment.md` §3.1 (region
//! descriptor layout, slot 7 = the block-map descriptor).

use super::{put, read_landblock_did};
use crate::did::{Did, LandblockId, LandblockKind};
use crate::error::{FormatError, Result};
use crate::property::{PropertySchema, PropertyValue};
use crate::reader::{Reader, TSize, keys_in_bucket_order};
use crate::record::Record;

/// Type prefix of block-map descriptors.
pub const BLOCK_MAP_DESCRIPTOR_PREFIX: u8 = 0x18;
/// Type prefix of region descriptors.
pub const REGION_DESCRIPTOR_PREFIX: u8 = 0x0F;
/// Stored size of a region descriptor (every observed record).
pub const REGION_DESCRIPTOR_LEN: usize = 114;

/// One (key, palette index) pair of a block map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BlockMapEntry {
    /// Key selecting a descriptor entry ([`BlockMapProperty::key`]).
    pub key: u32,
    /// Index into that entry's palette.
    pub index: u16,
}

/// A block map `0x801RXXYY` (§5.3): per-landblock (key, index) pairs into the region's
/// [`BlockMapDescriptor`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlockMap {
    /// Own DID.
    pub did: Did,
    /// Stored TSize header.
    pub size: TSize,
    /// Entries in stored (bucket) order.
    pub entries: Vec<BlockMapEntry>,
}

impl BlockMap {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "block map";

    /// The index stored for `key`.
    #[must_use]
    pub fn get(&self, key: u32) -> Option<u16> {
        self.entries.iter().find(|e| e.key == key).map(|e| e.index)
    }

    /// Whether the keys are in bucket order for the header's hint (`None` for an unknown
    /// hint).
    #[must_use]
    pub fn is_bucket_ordered(&self) -> Option<bool> {
        keys_in_bucket_order(self.size.hint, self.entries.iter().map(|e| e.key))
    }

    /// The landblock this record describes.
    #[must_use]
    pub fn landblock(&self) -> Option<LandblockId> {
        LandblockId::from_did(self.did)
    }

    /// Re-encodes the record (byte-exact for a decoded record).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + 6 * self.entries.len());
        put::u32(&mut out, self.did.0);
        out.push(self.size.hint);
        crate::reader::write_vle(&mut out, put::len_u32(self.entries.len()));
        for e in &self.entries {
            put::u32(&mut out, e.key);
            put::u16(&mut out, e.index);
        }
        out
    }

    /// The first deviation from the observed invariants (BM1 order), or `None`.
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        (self.is_bucket_ordered() == Some(false)).then_some("keys not in bucket order")
    }
}

impl Record<'_> for BlockMap {
    const NAME: &'static str = BlockMap::NAME;

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_landblock_did(&mut r, LandblockKind::BlockMap, Self::NAME)?;
        let size = r.tsize()?;
        let mut entries = r.alloc_vec(size.count, 6, usize::MAX)?;
        for _ in 0..size.count {
            let e = BlockMapEntry {
                key: r.u32()?,
                index: r.u16()?,
            };
            r.push(&mut entries, e, size.count)?;
        }
        r.expect_end(Self::NAME)?;
        r.log_unknown_hints(Self::NAME);
        Ok(Self { did, size, entries })
    }

    fn did(&self) -> Did {
        self.did
    }
}

/// One palette value of a descriptor entry: the stored PID and its value. A null element
/// (PID 0; Reported only) keeps its slot with no value so indices stay aligned.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteValue {
    /// Stored PID (equal to the entry's `pid` in every observed record).
    pub pid: u32,
    /// The value, decoded with the PID's type; `None` for a null element.
    pub value: Option<PropertyValue>,
}

/// One entry of a block-map descriptor (§5.2): a property and its palette.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockMapProperty {
    /// Property ID of the values.
    pub pid: u32,
    /// Second copy of `pid`.
    pub pid_echo: u32,
    /// The block-map key that selects this entry.
    pub key: u32,
    /// 1, except 0 for the one float-valued property (meaning OPEN).
    pub flag: u8,
    /// The palette that block-map indices select from.
    pub values: Vec<PaletteValue>,
}

impl BlockMapProperty {
    /// Palette value `index`, or `None` when out of range (not an error, §5.4) or null.
    #[must_use]
    pub fn value(&self, index: u16) -> Option<&PropertyValue> {
        self.values.get(usize::from(index))?.value.as_ref()
    }
}

/// One resolved block-map pair (§5.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedBlockMapValue<'d> {
    /// The block-map key.
    pub key: u32,
    /// The stored index.
    pub index: u16,
    /// The described property, or `None` for a key without a descriptor entry (§5.6).
    pub pid: Option<u32>,
    /// The palette value; `None` for an unknown key or an out-of-range index.
    pub value: Option<&'d PropertyValue>,
}

/// A block-map descriptor (prefix 0x18; §5.2), shared by the landblocks of one or more
/// regions.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockMapDescriptor {
    /// Own DID.
    pub did: Did,
    /// u32 after the DID; 0 in every observed record.
    pub reserved: u32,
    /// Stored TSize header.
    pub size: TSize,
    /// Entries in stored order.
    pub entries: Vec<BlockMapProperty>,
}

impl BlockMapDescriptor {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "block-map descriptor";

    /// Decodes a complete payload; `schema` (normally the property dictionary) supplies
    /// each palette value's type. The payload must be consumed exactly.
    pub fn parse<S: PropertySchema + ?Sized>(data: &[u8], schema: &S) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = r.did()?;
        if did.prefix() != BLOCK_MAP_DESCRIPTOR_PREFIX {
            return Err(FormatError::WrongRecordType {
                did: did.0,
                expected: Self::NAME,
            });
        }
        let reserved = r.u32()?;
        let size = r.tsize()?;
        // pid + echo + key + flag + value count.
        let mut entries = r.alloc_vec(size.count, 17, usize::MAX)?;
        for _ in 0..size.count {
            let e = read_property(&mut r, schema)?;
            r.push(&mut entries, e, size.count)?;
        }
        r.expect_end(Self::NAME)?;
        r.log_unknown_hints(Self::NAME);
        Ok(Self {
            did,
            reserved,
            size,
            entries,
        })
    }

    /// [`BlockMapDescriptor::parse`], also checking the DID against the one looked up.
    pub fn parse_expecting<S: PropertySchema + ?Sized>(
        data: &[u8],
        expected: impl Into<Did>,
        schema: &S,
    ) -> Result<Self> {
        let expected = expected.into();
        let rec = Self::parse(data, schema)?;
        if rec.did != expected {
            return Err(FormatError::DidMismatch {
                expected: expected.0,
                found: rec.did.0,
            });
        }
        Ok(rec)
    }

    /// The entry selected by block-map key `key` (the first, if several share it). A
    /// linear search: for many keys use [`BlockMapDescriptor::resolve`].
    #[must_use]
    pub fn entry(&self, key: u32) -> Option<&BlockMapProperty> {
        self.entries.iter().find(|e| e.key == key)
    }

    /// The entry describing property `pid`.
    #[must_use]
    pub fn entry_for_pid(&self, pid: u32) -> Option<&BlockMapProperty> {
        self.entries.iter().find(|e| e.pid == pid)
    }

    /// Resolves every pair of `map` (§5.4), in the map's order. Unknown keys and
    /// out-of-range indices resolve to no value; that is not an error.
    ///
    /// Costs O((D + M) log D) for D descriptor entries and M map pairs: the descriptor's
    /// keys are indexed once per call, so a hostile descriptor and map cannot make a
    /// landblock load quadratic. With duplicate descriptor keys the first entry wins, as in
    /// [`BlockMapDescriptor::entry`].
    #[must_use]
    pub fn resolve<'d>(&'d self, map: &BlockMap) -> Vec<ResolvedBlockMapValue<'d>> {
        // (key, entry position), sorted; ties keep stored order so the first entry wins.
        let mut by_key: Vec<(u32, usize)> = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.key, i))
            .collect();
        by_key.sort_unstable();
        let lookup = |key: u32| {
            let at = by_key.partition_point(|&(k, _)| k < key);
            match by_key.get(at) {
                Some(&(k, i)) if k == key => self.entries.get(i),
                _ => None,
            }
        };
        map.entries
            .iter()
            .map(|e| {
                let p = lookup(e.key);
                ResolvedBlockMapValue {
                    key: e.key,
                    index: e.index,
                    pid: p.map(|p| p.pid),
                    value: p.and_then(|p| p.value(e.index)),
                }
            })
            .collect()
    }

    /// The value of property `pid` for the landblock whose block map is `map`.
    #[must_use]
    pub fn value_of<'d>(&'d self, map: &BlockMap, pid: u32) -> Option<&'d PropertyValue> {
        let p = self.entry_for_pid(pid)?;
        p.value(map.get(p.key)?)
    }

    /// The first deviation from the observed invariants (BM3, reserved word), or `None`.
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        if self.reserved != 0 {
            return Some("reserved word not 0");
        }
        for e in &self.entries {
            if e.pid_echo != e.pid {
                return Some("PID echo differs (BM3)");
            }
            if e.values.iter().any(|v| v.pid != 0 && v.pid != e.pid) {
                return Some("palette value PID differs from entry PID (BM3)");
            }
        }
        None
    }
}

fn read_property<S: PropertySchema + ?Sized>(
    r: &mut Reader<'_>,
    schema: &S,
) -> Result<BlockMapProperty> {
    let pid = r.u32()?;
    let pid_echo = r.u32()?;
    let key = r.u32()?;
    let flag = r.u8()?;
    let count = r.u32()?;
    let mut values = r.alloc_vec(count, 4, usize::MAX)?;
    for _ in 0..count {
        let pid_at = r.position();
        let vpid = r.u32()?;
        let value = if vpid == 0 {
            None
        } else {
            let ty = schema
                .property_type(vpid)
                .ok_or(FormatError::UnknownProperty {
                    pid: vpid,
                    offset: pid_at,
                })?;
            let value_at = r.position();
            Some(
                PropertyValue::read_with(r, ty, schema).map_err(|e| match e {
                    e @ FormatError::PropertyValue { .. } => e,
                    e => FormatError::PropertyValue {
                        pid: vpid,
                        type_code: ty.code(),
                        offset: value_at,
                        source: Box::new(e),
                    },
                })?,
            )
        };
        r.push(&mut values, PaletteValue { pid: vpid, value }, count)?;
    }
    Ok(BlockMapProperty {
        pid,
        pid_echo,
        key,
        flag,
        values,
    })
}

/// One 10-byte table slot of a region descriptor: two bytes (1, 1 observed), a u32 (0
/// observed) and the table DID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RegionTableSlot {
    /// Two leading bytes (meaning OPEN).
    pub head: [u8; 2],
    /// u32 before the DID (0 observed; meaning OPEN).
    pub value: u32,
    /// The table's DID.
    pub did: Did,
}

/// A region descriptor (prefix 0x0F, 114 bytes; `environment.md` §3.1). It names the
/// region's per-region tables, among them the block-map descriptor, and stores the
/// map-coordinate constants (`coordinates.md` §8.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionDescriptor {
    /// Own DID.
    pub did: Did,
    /// The landblock region number *R* this descriptor describes.
    pub region: u32,
    /// Offset 8 (8.0 or 8.85 observed; meaning OPEN).
    pub value_8: f32,
    /// Offset 12 (2 observed; meaning OPEN).
    pub value_12: u32,
    /// Map-coordinate origin in 20-unit landcells (1,468, 1,244 observed).
    pub map_origin: [u32; 2],
    /// Offsets 24 and 28 (0 observed).
    pub reserved: [u32; 2],
    /// Map units per world unit (0.005 observed).
    pub map_scale: f32,
    /// The seven 10-byte table slots at offsets 36–105 (slot index 6 = the block-map
    /// descriptor in every observed record).
    pub slots: [RegionTableSlot; 7],
    /// The four bytes before the eighth DID (01 00 01 00 observed).
    pub eighth_head: [u8; 4],
    /// The eighth table DID (offset 110; a 0x19 table in every observed record).
    pub eighth: Did,
}

impl RegionDescriptor {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "region descriptor";

    /// All eight table DIDs in stored order.
    #[must_use]
    pub fn table_dids(&self) -> [Did; 8] {
        let mut out = [Did::default(); 8];
        for (o, s) in out.iter_mut().zip(&self.slots) {
            *o = s.did;
        }
        out[7] = self.eighth;
        out
    }

    /// The first table whose DID has type prefix `prefix` (for example 0x18 for the
    /// block-map descriptor, §5.1). Looking tables up by prefix rather than slot keeps
    /// working if the slot order ever changes.
    #[must_use]
    pub fn table(&self, prefix: u8) -> Option<Did> {
        self.table_dids()
            .into_iter()
            .find(|d| !d.is_null() && d.prefix() == prefix)
    }

    /// The region's block-map descriptor (§5.1).
    #[must_use]
    pub fn block_map_descriptor(&self) -> Option<Did> {
        self.table(BLOCK_MAP_DESCRIPTOR_PREFIX)
    }

    /// The map-coordinate origin in world units: landcells × 20 (`coordinates.md` §8.1).
    #[must_use]
    pub fn map_origin_units(&self) -> [f64; 2] {
        [
            f64::from(self.map_origin[0]) * 20.0,
            f64::from(self.map_origin[1]) * 20.0,
        ]
    }

    /// The first deviation from the observed values, or `None`.
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        if self.slot_of_block_map() != Some(6) {
            return Some("block-map descriptor not in slot 7");
        }
        if self.slots.iter().any(|s| s.head != [1, 1] || s.value != 0) {
            return Some("table slot head differs from (1, 1, 0)");
        }
        None
    }

    fn slot_of_block_map(&self) -> Option<usize> {
        self.slots
            .iter()
            .position(|s| s.did.prefix() == BLOCK_MAP_DESCRIPTOR_PREFIX)
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
        let value_8 = r.f32()?;
        let value_12 = r.u32()?;
        let map_origin = [r.u32()?, r.u32()?];
        let reserved = [r.u32()?, r.u32()?];
        let map_scale = r.f32()?;
        let mut slots = [RegionTableSlot::default(); 7];
        for s in &mut slots {
            *s = RegionTableSlot {
                head: r.array()?,
                value: r.u32()?,
                did: r.did()?,
            };
        }
        let eighth_head = r.array()?;
        let eighth = r.did()?;
        r.expect_end(Self::NAME)?;
        Ok(Self {
            did,
            region,
            value_8,
            value_12,
            map_origin,
            reserved,
            map_scale,
            slots,
            eighth_head,
            eighth,
        })
    }

    fn did(&self) -> Did {
        self.did
    }
}
