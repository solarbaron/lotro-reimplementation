// SPDX-License-Identifier: GPL-3.0-or-later
//! Shader records (prefix 0x2B, `material.md` §7): the parameter interface, the binding
//! tables, and per permutation the sampler table and render-state block. Program blobs
//! are skipped by their sizes and never kept.

use std::mem::size_of;

use tessera_formats::reader::latin1_to_string;
use tessera_formats::{Did, FormatError, Reader, Record};

use super::{GROUP_TEXTURE, MIN_ENTRY_SIZE, ParamEntry, SHADER_PREFIX, alloc_vec, read_did};

/// Words in a permutation key (§7.4).
pub const PERMUTATION_KEY_WORDS: usize = 4;
/// Size of a sampler state in bytes (§7.5).
pub const SAMPLER_STATE_SIZE: usize = 40;
/// Size of a render-state block in bytes (§7.6).
pub const RENDER_STATE_SIZE: usize = 39;

/// Smallest permutation: key, `has_programs`, empty sampler table, empty extra list and
/// the render-state block.
const MIN_PERMUTATION_SIZE: usize = 16 + 4 + 4 + 4 + RENDER_STATE_SIZE;
/// Smallest sampler entry: an empty name and the state.
const MIN_SAMPLER_SIZE: usize = 1 + SAMPLER_STATE_SIZE;
/// Smallest extra state item: type, stage and one empty array.
const MIN_EXTRA_ITEM_SIZE: usize = 1 + 4 + 4;

/// Texture address mode (sampler state bytes 8 and 9; names Inferred from D3D, §7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressMode {
    /// 1: repeat.
    Wrap,
    /// 3: clamp to edge.
    Clamp,
    /// 4: clamp to a border colour.
    Border,
    /// Any other code.
    Other(u8),
}

impl AddressMode {
    /// Maps the stored code.
    #[must_use]
    pub fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Wrap,
            3 => Self::Clamp,
            4 => Self::Border,
            c => Self::Other(c),
        }
    }
}

/// Texture filter (sampler state bytes 10–12; names Inferred from D3D, §7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterMode {
    /// 0 (mip filter only): no mipmapping.
    None,
    /// 1: nearest.
    Point,
    /// 2: linear.
    Linear,
    /// 3: anisotropic.
    Anisotropic,
    /// Any other code.
    Other(u8),
}

impl FilterMode {
    /// Maps the stored code.
    #[must_use]
    pub fn from_code(code: u8) -> Self {
        match code {
            0 => Self::None,
            1 => Self::Point,
            2 => Self::Linear,
            3 => Self::Anisotropic,
            c => Self::Other(c),
        }
    }
}

/// A sampler state block (§7.5). Only the address and filter bytes have a known meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SamplerState {
    /// The 40 stored bytes.
    pub raw: [u8; SAMPLER_STATE_SIZE],
}

impl SamplerState {
    /// Address mode along U (byte 8).
    #[must_use]
    pub fn address_u(&self) -> AddressMode {
        AddressMode::from_code(self.raw[8])
    }

    /// Address mode along V (byte 9).
    #[must_use]
    pub fn address_v(&self) -> AddressMode {
        AddressMode::from_code(self.raw[9])
    }

    /// Magnification filter (byte 10).
    #[must_use]
    pub fn mag_filter(&self) -> FilterMode {
        FilterMode::from_code(self.raw[10])
    }

    /// Minification filter (byte 11).
    #[must_use]
    pub fn min_filter(&self) -> FilterMode {
        FilterMode::from_code(self.raw[11])
    }

    /// Mip filter (byte 12).
    #[must_use]
    pub fn mip_filter(&self) -> FilterMode {
        FilterMode::from_code(self.raw[12])
    }
}

/// One sampler table entry (§7.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sampler {
    /// Sampler name (ASCII in observed data; decoded as Latin-1), may be empty.
    pub name: String,
    /// The sampler state.
    pub state: SamplerState,
}

/// A D3D-style blend factor (render-state bytes 0 and 1; names Inferred, §7.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendFactor {
    /// 1.
    Zero,
    /// 2.
    One,
    /// 3: source colour.
    SrcColor,
    /// 4: one minus source colour.
    InvSrcColor,
    /// 5: source alpha (the opacity source, §11.3).
    SrcAlpha,
    /// 6: one minus source alpha.
    InvSrcAlpha,
    /// 7: destination alpha.
    DestAlpha,
    /// 9: destination colour.
    DestColor,
    /// Any code the spec does not name.
    Other(u8),
}

impl BlendFactor {
    /// Maps the stored code.
    #[must_use]
    pub fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Zero,
            2 => Self::One,
            3 => Self::SrcColor,
            4 => Self::InvSrcColor,
            5 => Self::SrcAlpha,
            6 => Self::InvSrcAlpha,
            7 => Self::DestAlpha,
            9 => Self::DestColor,
            c => Self::Other(c),
        }
    }
}

/// A D3D-style blend operation (render-state byte 2; names Inferred, §7.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendOp {
    /// 1: source + destination.
    Add,
    /// 3: destination − source.
    RevSubtract,
    /// Any code the spec does not name.
    Other(u8),
}

impl BlendOp {
    /// Maps the stored code.
    #[must_use]
    pub fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Add,
            3 => Self::RevSubtract,
            c => Self::Other(c),
        }
    }
}

/// Face culling (render-state byte 5; names Inferred, §7.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CullMode {
    /// 1: no culling (two-sided).
    None,
    /// 2: back faces culled.
    Back,
    /// Any other code.
    Other(u8),
}

impl CullMode {
    /// Maps the stored code.
    #[must_use]
    pub fn from_code(code: u8) -> Self {
        match code {
            1 => Self::None,
            2 => Self::Back,
            c => Self::Other(c),
        }
    }
}

/// A render-state block (§7.6; layout Confirmed, meanings Inferred).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderState {
    /// The 39 stored bytes.
    pub raw: [u8; RENDER_STATE_SIZE],
}

impl RenderState {
    /// Source blend factor code (byte 0).
    #[must_use]
    pub fn src_blend(&self) -> u8 {
        self.raw[0]
    }

    /// Destination blend factor code (byte 1).
    #[must_use]
    pub fn dst_blend(&self) -> u8 {
        self.raw[1]
    }

    /// Blend operation code (byte 2).
    #[must_use]
    pub fn blend_op(&self) -> u8 {
        self.raw[2]
    }

    /// Alpha compare function code (byte 3; 7 GREATEREQUAL, 8 ALWAYS).
    #[must_use]
    pub fn alpha_cmp(&self) -> u8 {
        self.raw[3]
    }

    /// Depth write enable (byte 4 ≠ 0; Inferred, §7.6).
    #[must_use]
    pub fn depth_write(&self) -> bool {
        self.raw[4] != 0
    }

    /// Cull mode (byte 5).
    #[must_use]
    pub fn cull(&self) -> CullMode {
        CullMode::from_code(self.raw[5])
    }

    /// Default alpha-test reference (bytes 11–14, f32).
    #[must_use]
    pub fn alpha_ref(&self) -> f32 {
        f32::from_le_bytes([self.raw[11], self.raw[12], self.raw[13], self.raw[14]])
    }

    /// The (src, dst, op) blend codes.
    #[must_use]
    pub fn blend_triple(&self) -> (u8, u8, u8) {
        (self.src_blend(), self.dst_blend(), self.blend_op())
    }
}

/// The compiled programs of a permutation: only their kind and sizes are kept (§7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Programs {
    /// Program kind (0–4 observed; Inferred to be the back end).
    pub kind: u32,
    /// Size of the first opaque blob in bytes.
    pub a_len: u32,
    /// Size of the second opaque blob in bytes.
    pub b_len: u32,
}

/// One item of a permutation's extra state list (§7.7; meaning OPEN).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraStateItem {
    /// Item type: 1, 2, 3 or 5.
    pub item_type: u8,
    /// A small integer.
    pub stage: u32,
    /// The item's arrays of 4-byte values (one value each in observed data).
    pub arrays: Vec<Vec<u32>>,
}

impl ExtraStateItem {
    /// Number of arrays an item of `item_type` carries, or `None` for an unknown type
    /// (whose length is unknown).
    #[must_use]
    pub fn array_count(item_type: u8) -> Option<usize> {
        match item_type {
            1 | 3 => Some(2),
            2 => Some(1),
            5 => Some(7),
            _ => None,
        }
    }
}

/// One permutation (§7.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permutation {
    /// Permutation key, an opaque bit pattern.
    pub key: [u32; PERMUTATION_KEY_WORDS],
    /// The programs' kind and sizes, if the permutation has programs.
    pub programs: Option<Programs>,
    /// The sampler table.
    pub samplers: Vec<Sampler>,
    /// The extra state list.
    pub extra_items: Vec<ExtraStateItem>,
    /// The render-state block.
    pub render_state: RenderState,
}

/// A shader record (prefix 0x2B, `material.md` §7).
#[derive(Debug, Clone, PartialEq)]
pub struct ShaderRecord {
    /// The record's own DID (0x2B…).
    pub did: Did,
    /// Reserved word, 0 in every observed record.
    pub reserved: u32,
    /// Parameter declarations: defaults plus binding tables (§7.2).
    pub params: Vec<ParamEntry>,
    /// The permutations (§7.4).
    pub permutations: Vec<Permutation>,
}

impl ShaderRecord {
    /// The declaration of `id` (the last one, should an id repeat).
    #[must_use]
    pub fn declaration(&self, id: u32) -> Option<&ParamEntry> {
        self.params.iter().rev().find(|e| e.id == id)
    }

    /// Whether the record declares `id`.
    #[must_use]
    pub fn declares(&self, id: u32) -> bool {
        self.params.iter().any(|e| e.id == id)
    }

    /// Whether a field differs from what every observed record has (still decodes): a
    /// non-zero reserved word, or an unusual declaration.
    #[must_use]
    pub fn has_unusual_fields(&self) -> bool {
        self.reserved != 0 || self.params.iter().any(ParamEntry::has_unusual_fields)
    }

    fn parse_permutation(r: &mut Reader<'_>) -> tessera_formats::Result<Permutation> {
        let mut key = [0u32; PERMUTATION_KEY_WORDS];
        for k in &mut key {
            *k = r.u32()?;
        }
        let at = r.position();
        let programs = match r.u32()? {
            0 => None,
            1 => {
                let kind = r.u32()?;
                let a_len = r.u32()?;
                r.skip(usize::try_from(a_len).unwrap_or(usize::MAX))?;
                let b_len = r.u32()?;
                r.skip(usize::try_from(b_len).unwrap_or(usize::MAX))?;
                Some(Programs { kind, a_len, b_len })
            }
            other => {
                return Err(FormatError::Malformed {
                    what: Self::NAME,
                    offset: at,
                    reason: format!("has_programs is {other}, not 0 or 1").into(),
                });
            }
        };
        let sampler_count = r.u32()?;
        let mut samplers = alloc_vec::<Sampler>(r, sampler_count, MIN_SAMPLER_SIZE)?;
        for _ in 0..sampler_count {
            let len = r.u8()?;
            let name = r.bytes(usize::from(len))?;
            // Latin-1 takes at most two UTF-8 bytes per input byte.
            r.charge(name.len().saturating_mul(2))?;
            let name = latin1_to_string(name);
            let state = SamplerState { raw: r.array()? };
            samplers.push(Sampler { name, state });
        }
        let item_count = r.u32()?;
        let mut extra_items = alloc_vec::<ExtraStateItem>(r, item_count, MIN_EXTRA_ITEM_SIZE)?;
        for _ in 0..item_count {
            let type_at = r.position();
            let item_type = r.u8()?;
            let Some(arrays_n) = ExtraStateItem::array_count(item_type) else {
                return Err(FormatError::Malformed {
                    what: Self::NAME,
                    offset: type_at,
                    reason: format!("unknown extra state item type {item_type}").into(),
                });
            };
            let stage = r.u32()?;
            let arrays_n = u32::try_from(arrays_n).unwrap_or(u32::MAX);
            let mut arrays = alloc_vec::<Vec<u32>>(r, arrays_n, 4)?;
            for _ in 0..arrays_n {
                let n = r.u32()?;
                let mut values = alloc_vec::<u32>(r, n, 4)?;
                for _ in 0..n {
                    values.push(r.u32()?);
                }
                arrays.push(values);
            }
            extra_items.push(ExtraStateItem {
                item_type,
                stage,
                arrays,
            });
        }
        let render_state = RenderState { raw: r.array()? };
        Ok(Permutation {
            key,
            programs,
            samplers,
            extra_items,
            render_state,
        })
    }

    /// MT4: every binding names an existing permutation, and every texture binding an
    /// existing sampler of it.
    fn check_bindings(&self) -> tessera_formats::Result<()> {
        for decl in &self.params {
            for b in &decl.bindings {
                let perm = usize::try_from(b.permutation)
                    .ok()
                    .and_then(|i| self.permutations.get(i));
                let Some(perm) = perm else {
                    return Err(FormatError::Malformed {
                        what: Self::NAME,
                        offset: 0,
                        reason: format!(
                            "parameter {} binds permutation {} of {}",
                            decl.id,
                            b.permutation,
                            self.permutations.len()
                        )
                        .into(),
                    });
                };
                let texture = super::group_of(decl.id) == GROUP_TEXTURE;
                if texture && usize::try_from(b.slot).map_or(true, |s| s >= perm.samplers.len()) {
                    return Err(FormatError::Malformed {
                        what: Self::NAME,
                        offset: 0,
                        reason: format!(
                            "texture parameter {} binds sampler {} of {} in permutation {}",
                            decl.id,
                            b.slot,
                            perm.samplers.len(),
                            b.permutation
                        )
                        .into(),
                    });
                }
            }
        }
        Ok(())
    }

    /// The sampler a binding points to, if it is in range.
    #[must_use]
    pub fn sampler(&self, permutation: u32, slot: u32) -> Option<&Sampler> {
        self.permutations
            .get(usize::try_from(permutation).ok()?)?
            .samplers
            .get(usize::try_from(slot).ok()?)
    }

    /// Approximate heap bytes retained, for cache budgets.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        let params = self.params.capacity() * size_of::<ParamEntry>()
            + self
                .params
                .iter()
                .map(ParamEntry::heap_bytes)
                .sum::<usize>();
        let perms = self.permutations.capacity() * size_of::<Permutation>()
            + self
                .permutations
                .iter()
                .map(|p| {
                    p.samplers.capacity() * size_of::<Sampler>()
                        + p.samplers.iter().map(|s| s.name.capacity()).sum::<usize>()
                        + p.extra_items.capacity() * size_of::<ExtraStateItem>()
                        + p.extra_items
                            .iter()
                            .map(|i| {
                                i.arrays.capacity() * size_of::<Vec<u32>>()
                                    + i.arrays.iter().map(|a| a.capacity() * 4).sum::<usize>()
                            })
                            .sum::<usize>()
                })
                .sum::<usize>();
        params + perms
    }
}

impl Record<'_> for ShaderRecord {
    const NAME: &'static str = "shader record";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> tessera_formats::Result<Self> {
        let mut r = Reader::new(data);
        let did = read_did(&mut r, SHADER_PREFIX, Self::NAME)?;
        let reserved = r.u32()?;
        let param_count = r.u32()?;
        let mut params = alloc_vec::<ParamEntry>(&mut r, param_count, MIN_ENTRY_SIZE)?;
        for _ in 0..param_count {
            params.push(ParamEntry::parse(&mut r, Self::NAME)?);
        }
        let permutation_count = r.u32()?;
        let mut permutations =
            alloc_vec::<Permutation>(&mut r, permutation_count, MIN_PERMUTATION_SIZE)?;
        for _ in 0..permutation_count {
            permutations.push(Self::parse_permutation(&mut r)?);
        }
        r.expect_end(Self::NAME)?;
        let record = Self {
            did,
            reserved,
            params,
            permutations,
        };
        record.check_bindings()?;
        Ok(record)
    }
}
