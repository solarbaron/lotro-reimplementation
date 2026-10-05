// SPDX-License-Identifier: GPL-3.0-or-later
//! The parameter entry shared by materials and shader records (`material.md` §5).

use std::mem::size_of;

use tessera_formats::{Did, FormatError, Reader};

use super::alloc_vec;

/// Group of float parameters (ids 1000–1999), value type [`VALUE_TYPE_CURVE`].
pub const GROUP_FLOAT: u16 = 1000;
/// Group of colour parameters (ids 2000–2999), value type [`VALUE_TYPE_COLOR`].
pub const GROUP_COLOR: u16 = 2000;
/// Group of texture parameters (ids 3000–3999), value type [`VALUE_TYPE_WORD`].
pub const GROUP_TEXTURE: u16 = 3000;
/// Group of flag parameters (ids 4000–4999), value type [`VALUE_TYPE_WORD`].
pub const GROUP_FLAG: u16 = 4000;

/// `value_type` of a 4-byte word: a texture-list DID (group 3000) or a flag (group 4000).
pub const VALUE_TYPE_WORD: u32 = 4;
/// `value_type` of an RGBA colour (four f32).
pub const VALUE_TYPE_COLOR: u32 = 0x10;
/// `value_type` of a float curve (§5.3).
pub const VALUE_TYPE_CURVE: u32 = 0x40;

/// Highest float-curve `mode` with a known length (§5.3, MT3).
pub const MAX_CURVE_MODE: u32 = 10;

/// Size of one binding entry in bytes (§7.3).
pub const BINDING_SIZE: usize = 12;

/// The group an id belongs to: ⌊`id` / 1000⌋ × 1000 (§5.1), saturating at `u16::MAX`.
#[must_use]
pub fn group_of(id: u32) -> u16 {
    u16::try_from(id / 1000 * 1000).unwrap_or(u16::MAX)
}

/// A float curve (§5.3).
///
/// Mode 1 is a constant. Modes 2–9 carry ten waveform parameters `w[0..9]`; mode 10
/// carries `w[0..9]` (unused by that mode), its `period`, a `loop` byte and keyframe pairs.
/// [`FloatCurve::evaluate_at`] evaluates every mode over time (§5.3.1, Decision revised
/// 2026-10-05); [`FloatCurve::evaluate`] is the value at time 0.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatCurve {
    /// Curve mode, 1–10.
    pub mode: u32,
    /// The stored parameters: one for mode 1, ten for modes 2–9, eleven for mode 10.
    /// Unused trailing elements are 0.
    pub params: [f32; 11],
    /// Number of stored parameters in `params`.
    pub param_count: u8,
    /// The mode-10 `loop` byte: 0 holds the last value after one period, 1 restarts
    /// (§5.3.1). Always 0 or 1 (a reader rejects other values, §5.3); 0 for other modes.
    pub loop_flag: u8,
    /// The mode-10 keyframe pairs (value, position in percent of the period), in stored
    /// order (empty for other modes).
    pub keys: Vec<(f32, f32)>,
}

impl FloatCurve {
    /// A constant (mode 1) curve.
    #[must_use]
    pub fn constant(value: f32) -> Self {
        let mut params = [0.0; 11];
        params[0] = value;
        Self {
            mode: 1,
            params,
            param_count: 1,
            loop_flag: 0,
            keys: Vec::new(),
        }
    }

    /// The mode-10 `period` in seconds (`params[10]`); 0 for other modes.
    #[must_use]
    pub fn period(&self) -> f32 {
        if self.mode == 10 {
            self.params[10]
        } else {
            0.0
        }
    }

    /// Whether the curve is not a constant (`mode` ≠ 1), i.e. animated in the original.
    #[must_use]
    pub fn is_animated(&self) -> bool {
        self.mode != 1
    }

    /// The stored parameters (`w[0..]`, or the constant for mode 1).
    #[must_use]
    pub fn stored_params(&self) -> &[f32] {
        &self.params[..usize::from(self.param_count).min(self.params.len())]
    }

    fn parse(r: &mut Reader<'_>, what: &'static str) -> tessera_formats::Result<Self> {
        let at = r.position();
        let mode = r.u32()?;
        let param_count: u8 = match mode {
            1 => 1,
            2..=9 => 10,
            10 => 11,
            _ => {
                return Err(FormatError::Malformed {
                    what,
                    offset: at,
                    reason: format!("float curve mode {mode} has no known length").into(),
                });
            }
        };
        let mut params = [0.0f32; 11];
        for p in params.iter_mut().take(usize::from(param_count)) {
            *p = r.f32()?;
        }
        let mut loop_flag = 0;
        let mut keys = Vec::new();
        if mode == 10 {
            let loop_at = r.position();
            loop_flag = r.u8()?;
            // §5.3: `loop` is 0 or 1; a reader rejects ≥ 2.
            if loop_flag > 1 {
                return Err(FormatError::Malformed {
                    what,
                    offset: loop_at,
                    reason: format!("float curve loop byte {loop_flag} is not 0 or 1").into(),
                });
            }
            let k = r.u32()?;
            keys = alloc_vec::<(f32, f32)>(r, k, 8)?;
            for _ in 0..k {
                keys.push((r.f32()?, r.f32()?));
            }
        }
        Ok(Self {
            mode,
            params,
            param_count,
            loop_flag,
            keys,
        })
    }
}

/// A parameter value (§5.2).
#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    /// A float curve (value type 0x40).
    Float(FloatCurve),
    /// An RGBA colour (value type 0x10).
    Color([f32; 4]),
    /// A texture-list DID (value type 4 on a 3000-group id); zero means none.
    Texture(Did),
    /// A 4-byte word on any other id: a flag (0 or 1 observed) on 4000-group ids.
    Flag(u32),
}

impl ParamValue {
    /// The stored `value_type` code of this value.
    #[must_use]
    pub fn value_type(&self) -> u32 {
        match self {
            Self::Float(_) => VALUE_TYPE_CURVE,
            Self::Color(_) => VALUE_TYPE_COLOR,
            Self::Texture(_) | Self::Flag(_) => VALUE_TYPE_WORD,
        }
    }

    /// Whether `other` has the same value type (MT7 compares these).
    #[must_use]
    pub fn same_type(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    /// Heap bytes retained by the value (a mode-10 curve's keyframes), for cache budgets.
    pub(crate) fn heap_bytes(&self) -> usize {
        match self {
            Self::Float(c) => c.keys.capacity() * size_of::<(f32, f32)>(),
            _ => 0,
        }
    }

    /// The value type a well-formed entry of `group` has (§5.2), if the group is known.
    #[must_use]
    pub fn expected_type(group: u16) -> Option<u32> {
        match group {
            GROUP_FLOAT => Some(VALUE_TYPE_CURVE),
            GROUP_COLOR => Some(VALUE_TYPE_COLOR),
            GROUP_TEXTURE | GROUP_FLAG => Some(VALUE_TYPE_WORD),
            _ => None,
        }
    }

    /// The float value at time 0 ([`FloatCurve::evaluate`]), for float values. Animated
    /// curves need [`FloatCurve::evaluate_at`] with a clock.
    #[must_use]
    pub fn as_float(&self) -> Option<f32> {
        match self {
            Self::Float(c) => Some(c.evaluate()),
            _ => None,
        }
    }

    /// The colour, for colour values.
    #[must_use]
    pub fn as_color(&self) -> Option<[f32; 4]> {
        match self {
            Self::Color(c) => Some(*c),
            _ => None,
        }
    }

    /// The texture-list DID, for non-zero texture values.
    #[must_use]
    pub fn as_texture(&self) -> Option<Did> {
        match self {
            Self::Texture(d) if !d.is_null() => Some(*d),
            _ => None,
        }
    }

    /// Whether a flag word is non-zero, for flag values.
    #[must_use]
    pub fn as_flag(&self) -> Option<bool> {
        match self {
            Self::Flag(v) => Some(*v != 0),
            _ => None,
        }
    }
}

/// One binding entry of a shader declaration (§7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Binding {
    /// Small integer, fixed per id (10 for every texture binding). Not needed by Tessera.
    pub semantic: u16,
    /// Equals the declaration's group in every observed record.
    pub group: u16,
    /// Index into the shader record's permutation list.
    pub permutation: u32,
    /// For texture parameters: index into that permutation's sampler table. For other
    /// groups the meaning is OPEN.
    pub slot: u32,
}

/// One parameter entry (§5.1): a material override or a shader declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamEntry {
    /// Parameter id, for example 3000 (§8).
    pub id: u32,
    /// Stored group; ⌊`id` / 1000⌋ × 1000 in every observed entry.
    pub group: u16,
    /// The value (a material's override, or a declaration's default).
    pub value: ParamValue,
    /// The per-entry flag byte, fixed per id (meaning OPEN, §8.6).
    pub flag: u8,
    /// Binding entries (always empty in materials).
    pub bindings: Vec<Binding>,
}

impl ParamEntry {
    /// Decodes one entry at the reader's position (§5.1). Fails on an unknown
    /// `value_type` or curve mode, because the entry length is then unknown.
    pub fn parse(r: &mut Reader<'_>, what: &'static str) -> tessera_formats::Result<Self> {
        let id = r.u32()?;
        let group = r.u16()?;
        let type_at = r.position();
        let value_type = r.u32()?;
        let value = match value_type {
            VALUE_TYPE_WORD => {
                let word = r.u32()?;
                if group_of(id) == GROUP_TEXTURE {
                    ParamValue::Texture(Did(word))
                } else {
                    ParamValue::Flag(word)
                }
            }
            VALUE_TYPE_COLOR => ParamValue::Color([r.f32()?, r.f32()?, r.f32()?, r.f32()?]),
            VALUE_TYPE_CURVE => ParamValue::Float(FloatCurve::parse(r, what)?),
            other => {
                return Err(FormatError::Malformed {
                    what,
                    offset: type_at,
                    reason: format!("unknown parameter value type {other:#x} (id {id})").into(),
                });
            }
        };
        let binding_count = r.u32()?;
        let flag = r.u8()?;
        let mut bindings = alloc_vec::<Binding>(r, binding_count, BINDING_SIZE)?;
        for _ in 0..binding_count {
            bindings.push(Binding {
                semantic: r.u16()?,
                group: r.u16()?,
                permutation: r.u32()?,
                slot: r.u32()?,
            });
        }
        Ok(Self {
            id,
            group,
            value,
            flag,
            bindings,
        })
    }

    /// Heap bytes this entry retains beyond its own size.
    pub(crate) fn heap_bytes(&self) -> usize {
        self.value.heap_bytes() + self.bindings.capacity() * size_of::<Binding>()
    }

    /// Whether the entry differs from what every observed entry looks like: a stored
    /// group other than ⌊`id` / 1000⌋ × 1000, a value type other than the group's (§5.2),
    /// a flag byte above 1, or a flag word above 1. Such entries still decode.
    #[must_use]
    pub fn has_unusual_fields(&self) -> bool {
        let group = group_of(self.id);
        self.group != group
            || ParamValue::expected_type(group) != Some(self.value.value_type())
            || self.flag > 1
            || matches!(self.value, ParamValue::Flag(v) if v > 1)
    }
}
