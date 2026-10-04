// SPDX-License-Identifier: GPL-3.0-or-later
//! Decoders for the three record types of the material chain (`docs/specs/material.md`
//! §4–§7):
//!
//! | Prefix | Type | Spec |
//! |---|---|---|
//! | 0x31 | [`SurfaceRecord`] (render surface descriptor) | §4 |
//! | 0x30 | [`MaterialRecord`] (partial parameter set) | §6 |
//! | 0x2B | [`ShaderRecord`] (parameter interface, permutations) | §7 |
//!
//! All three use the parameter entry of §5 ([`ParamEntry`]). Every decoder takes a
//! complete payload, checks its own DID prefix, consumes the payload exactly and treats
//! it as hostile: counts are checked against the bytes left before anything is
//! allocated, and retained memory is charged against the [`tessera_formats::Reader`]
//! budget. Compiled program blobs inside shader records are skipped by their size fields
//! and never kept (§7.4, §11.1).

mod params;
mod shader;

use std::collections::HashSet;
use std::mem::size_of;

use tessera_formats::{Did, FormatError, Reader, Record};

pub use params::{
    BINDING_SIZE, Binding, FloatCurve, GROUP_COLOR, GROUP_FLAG, GROUP_FLOAT, GROUP_TEXTURE,
    MAX_CURVE_MODE, ParamEntry, ParamValue, VALUE_TYPE_COLOR, VALUE_TYPE_CURVE, VALUE_TYPE_WORD,
    group_of,
};
pub use shader::{
    AddressMode, BlendFactor, BlendOp, CullMode, ExtraStateItem, FilterMode, PERMUTATION_KEY_WORDS,
    Permutation, Programs, RENDER_STATE_SIZE, RenderState, SAMPLER_STATE_SIZE, Sampler,
    SamplerState, ShaderRecord,
};

/// Prefix of surface records.
pub const SURFACE_RECORD_PREFIX: u8 = 0x31;
/// Prefix of materials.
pub const MATERIAL_PREFIX: u8 = 0x30;
/// Prefix of shader records.
pub const SHADER_PREFIX: u8 = 0x2B;
/// Prefix of texture lists.
pub const TEXTURE_LIST_PREFIX: u8 = 0x40;

/// Validates `count` elements of at least `min_size` input bytes each against the bytes
/// left, charges their in-memory size to the reader's budget and returns an empty vector
/// with exactly that capacity.
pub(crate) fn alloc_vec<T>(
    r: &mut Reader<'_>,
    count: u32,
    min_size: usize,
) -> tessera_formats::Result<Vec<T>> {
    r.ensure_elements(count, min_size)?;
    let n = usize::try_from(count).unwrap_or(usize::MAX);
    r.charge(n.saturating_mul(size_of::<T>()))?;
    Ok(Vec::with_capacity(n))
}

/// Reads a record's DID prologue and checks its prefix.
fn read_did(r: &mut Reader<'_>, prefix: u8, what: &'static str) -> tessera_formats::Result<Did> {
    let did = r.did()?;
    if did.prefix() != prefix {
        return Err(FormatError::WrongRecordType {
            did: did.0,
            expected: what,
        });
    }
    Ok(did)
}

/// A surface record (prefix 0x31, `material.md` §4): the shader record and the ordered
/// materials one submesh is drawn with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceRecord {
    /// The record's own DID (0x31…).
    pub did: Did,
    /// The shader record (0x2B…). Never zero in observed data.
    pub shader: Did,
    /// Binding key: 1, 2 or 0x1000xxxx; matched by appearance groups (§9.3).
    pub binding_key: u32,
    /// Material DIDs (0x30…), applied in order (§9.2). 0–3 observed.
    pub materials: Vec<Did>,
    /// Trailing u16 (1 mostly, also 0 and 256; meaning OPEN, §4.2).
    pub tail: u16,
}

impl SurfaceRecord {
    /// Fixed part of the record: DID, shader, key, count and tail.
    pub const FIXED_SIZE: usize = 18;

    /// Whether a field differs from what every observed record has (still decodes): a
    /// shader DID without prefix 0x2B, a material DID without prefix 0x30, or more than
    /// three materials.
    #[must_use]
    pub fn has_unusual_fields(&self) -> bool {
        self.shader.prefix() != SHADER_PREFIX
            || self.materials.len() > 3
            || self.materials.iter().any(|m| m.prefix() != MATERIAL_PREFIX)
    }
}

impl Record<'_> for SurfaceRecord {
    const NAME: &'static str = "surface record";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> tessera_formats::Result<Self> {
        let mut r = Reader::new(data);
        let did = read_did(&mut r, SURFACE_RECORD_PREFIX, Self::NAME)?;
        let shader = r.did()?;
        let binding_key = r.u32()?;
        let count = r.u32()?;
        // MT1: the size is fixed by the count, so check it exactly before reading on.
        let expected = usize::try_from(count)
            .ok()
            .and_then(|n| n.checked_mul(4))
            .and_then(|n| n.checked_add(Self::FIXED_SIZE));
        match expected {
            Some(e) if e == data.len() => {}
            Some(e) if e < data.len() => {
                return Err(FormatError::TrailingBytes {
                    what: Self::NAME,
                    offset: e,
                    count: data.len() - e,
                });
            }
            _ => {
                return Err(FormatError::Truncated {
                    offset: r.position(),
                    needed: expected.map_or(usize::MAX, |e| e.saturating_sub(r.position())),
                    available: r.remaining(),
                });
            }
        }
        let mut materials = alloc_vec::<Did>(&mut r, count, 4)?;
        for _ in 0..count {
            materials.push(r.did()?);
        }
        let tail = r.u16()?;
        r.expect_end(Self::NAME)?;
        Ok(Self {
            did,
            shader,
            binding_key,
            materials,
            tail,
        })
    }
}

/// A material (prefix 0x30, `material.md` §6): a partial parameter set listing only the
/// parameters it changes.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialRecord {
    /// The record's own DID (0x30…).
    pub did: Did,
    /// The parameter entries, in stored order. Not sorted by id; look up by id.
    pub entries: Vec<ParamEntry>,
}

impl MaterialRecord {
    /// The entry for `id` (the last one, should an id repeat).
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&ParamEntry> {
        self.entries.iter().rev().find(|e| e.id == id)
    }

    /// Whether the record differs from what every observed material looks like (MT2):
    /// an entry with bindings, a repeated id, or an unusual entry
    /// ([`ParamEntry::has_unusual_fields`]).
    #[must_use]
    pub fn has_unusual_fields(&self) -> bool {
        self.entries
            .iter()
            .any(|e| !e.bindings.is_empty() || e.has_unusual_fields())
            || self.has_repeated_id()
    }

    /// Whether some id occurs in more than one entry. Linear in the entry count.
    #[must_use]
    pub fn has_repeated_id(&self) -> bool {
        let mut seen = HashSet::with_capacity(self.entries.len());
        self.entries.iter().any(|e| !seen.insert(e.id))
    }

    /// Approximate heap bytes retained, for cache budgets.
    pub(crate) fn heap_bytes(&self) -> usize {
        self.entries.capacity() * size_of::<ParamEntry>()
            + self
                .entries
                .iter()
                .map(ParamEntry::heap_bytes)
                .sum::<usize>()
    }
}

/// Smallest encoding of one parameter entry: id, group, value type, a 4-byte value,
/// binding count and flag.
pub(crate) const MIN_ENTRY_SIZE: usize = 4 + 2 + 4 + 4 + 4 + 1;

impl Record<'_> for MaterialRecord {
    const NAME: &'static str = "material";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> tessera_formats::Result<Self> {
        let mut r = Reader::new(data);
        let did = read_did(&mut r, MATERIAL_PREFIX, Self::NAME)?;
        let count = r.u32()?;
        let mut entries = alloc_vec::<ParamEntry>(&mut r, count, MIN_ENTRY_SIZE)?;
        for _ in 0..count {
            entries.push(ParamEntry::parse(&mut r, Self::NAME)?);
        }
        r.expect_end(Self::NAME)?;
        Ok(Self { did, entries })
    }
}
