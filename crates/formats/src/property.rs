// SPDX-License-Identifier: GPL-3.0-or-later
//! Master property dictionary, DID 0x34000000 (spec §6).

use std::collections::HashMap;
use std::fmt;

use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::reader::{Reader, TSize};
use crate::record::{Record, read_prologue};
use crate::string_info::StringInfo;

/// Marker that follows the optional values of every definition (f32 1.5).
pub const DEFINITION_MARKER: u32 = 0x3FC0_0000;

/// Flag bit: a default value is stored.
pub const FLAG_HAS_DEFAULT: u32 = 0x800;
/// Flag bit: a minimum value is stored.
pub const FLAG_HAS_MIN: u32 = 0x1000;
/// Flag bit: a maximum value is stored.
pub const FLAG_HAS_MAX: u32 = 0x2000;

/// Property value type (spec §6.3). Names are descriptive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyType {
    /// 1
    String,
    /// 2
    StringToken,
    /// 3
    Waveform,
    /// 4
    Timestamp,
    /// 5
    TriState,
    /// 6
    Vector3,
    /// 7
    InstanceId,
    /// 8
    Enum,
    /// 9
    Float,
    /// 10
    PropertyId,
    /// 11
    Struct,
    /// 12
    Array,
    /// 13
    StringInfo,
    /// 14
    Bitfield64,
    /// 15
    Int,
    /// 16
    Colour,
    /// 17
    Position,
    /// 18
    Bitfield32,
    /// 19
    Int64,
    /// 20
    DidRef,
    /// 21
    Bool,
    /// 22
    BitfieldAny,
    /// Any other code.
    Unknown(u32),
}

impl PropertyType {
    /// Maps a stored type code.
    #[must_use]
    pub fn from_code(code: u32) -> Self {
        match code {
            1 => Self::String,
            2 => Self::StringToken,
            3 => Self::Waveform,
            4 => Self::Timestamp,
            5 => Self::TriState,
            6 => Self::Vector3,
            7 => Self::InstanceId,
            8 => Self::Enum,
            9 => Self::Float,
            10 => Self::PropertyId,
            11 => Self::Struct,
            12 => Self::Array,
            13 => Self::StringInfo,
            14 => Self::Bitfield64,
            15 => Self::Int,
            16 => Self::Colour,
            17 => Self::Position,
            18 => Self::Bitfield32,
            19 => Self::Int64,
            20 => Self::DidRef,
            21 => Self::Bool,
            22 => Self::BitfieldAny,
            other => Self::Unknown(other),
        }
    }

    /// The stored type code.
    #[must_use]
    pub fn code(self) -> u32 {
        match self {
            Self::String => 1,
            Self::StringToken => 2,
            Self::Waveform => 3,
            Self::Timestamp => 4,
            Self::TriState => 5,
            Self::Vector3 => 6,
            Self::InstanceId => 7,
            Self::Enum => 8,
            Self::Float => 9,
            Self::PropertyId => 10,
            Self::Struct => 11,
            Self::Array => 12,
            Self::StringInfo => 13,
            Self::Bitfield64 => 14,
            Self::Int => 15,
            Self::Colour => 16,
            Self::Position => 17,
            Self::Bitfield32 => 18,
            Self::Int64 => 19,
            Self::DidRef => 20,
            Self::Bool => 21,
            Self::BitfieldAny => 22,
            Self::Unknown(c) => c,
        }
    }

    /// Whether definitions of this type never store default/min/max values
    /// (types 10, 11, 12; spec §6.2).
    #[must_use]
    pub fn skips_definition_values(self) -> bool {
        matches!(self, Self::PropertyId | Self::Struct | Self::Array)
    }
}

/// Extra data of a subtype-10 waveform.
#[derive(Debug, Clone, PartialEq)]
pub struct WaveformExtra {
    /// The f32 after the ten base values.
    pub scalar: f32,
    /// The bool8 after it.
    pub flag: bool,
    /// The `k` pairs of f32.
    pub pairs: Vec<(f32, f32)>,
}

/// An animated scalar (type 3).
#[derive(Debug, Clone, PartialEq)]
pub struct Waveform {
    /// Subtype selecting the layout.
    pub subtype: u32,
    /// Base values: 0, 1 or 10 floats depending on the subtype.
    pub values: Vec<f32>,
    /// Extra data, present only for subtype 10.
    pub extra: Option<WaveformExtra>,
}

/// A decoded property value (spec §6.4).
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    /// 1: Latin-1 Pascal string.
    String(String),
    /// 2: string token.
    StringToken(u32),
    /// 3: waveform.
    Waveform(Waveform),
    /// 4: timestamp (f64).
    Timestamp(f64),
    /// 5: tri-state byte.
    TriState(u8),
    /// 6: x, y, z.
    Vector3([f32; 3]),
    /// 7: 64-bit instance ID.
    InstanceId(u64),
    /// 8: enum value.
    Enum(u32),
    /// 9: float.
    Float(f32),
    /// 10: property ID.
    PropertyId(u32),
    /// 11: struct header (ignored byte, field count).
    Struct {
        /// The ignored leading byte.
        ignored: u8,
        /// Field count.
        field_count: u8,
    },
    /// 12: array (u32).
    Array(u32),
    /// 13: StringInfo.
    StringInfo(Box<StringInfo>),
    /// 14: 64-bit bitfield.
    Bitfield64(u64),
    /// 15: signed 32-bit integer.
    Int(i32),
    /// 16: four colour bytes in stored order (channel order OPEN).
    Colour([u8; 4]),
    /// 18: 32-bit bitfield.
    Bitfield32(u32),
    /// 19: signed 64-bit integer.
    Int64(i64),
    /// 20: DID reference.
    DidRef(Did),
    /// 21: boolean.
    Bool(bool),
    /// 22: bitfield of any width; bit `i` is in `bytes[i / 8]`, LSB first.
    BitfieldAny {
        /// Number of bits.
        bits: u32,
        /// `ceil(bits / 8)` bytes.
        bytes: Vec<u8>,
    },
}

impl PropertyValue {
    /// Decodes a value of type `ty` at the reader's position.
    pub fn read(r: &mut Reader<'_>, ty: PropertyType) -> Result<Self> {
        Ok(match ty {
            PropertyType::String => Self::String(r.pascal_string()?),
            PropertyType::StringToken => Self::StringToken(r.u32()?),
            PropertyType::Waveform => Self::Waveform(read_waveform(r)?),
            PropertyType::Timestamp => Self::Timestamp(r.f64()?),
            PropertyType::TriState => Self::TriState(r.u8()?),
            PropertyType::Vector3 => Self::Vector3([r.f32()?, r.f32()?, r.f32()?]),
            PropertyType::InstanceId => Self::InstanceId(r.u64()?),
            PropertyType::Enum => Self::Enum(r.u32()?),
            PropertyType::Float => Self::Float(r.f32()?),
            PropertyType::PropertyId => Self::PropertyId(r.u32()?),
            PropertyType::Struct => Self::Struct {
                ignored: r.u8()?,
                field_count: r.u8()?,
            },
            PropertyType::Array => Self::Array(r.u32()?),
            PropertyType::StringInfo => Self::StringInfo(Box::new(StringInfo::read(r)?)),
            PropertyType::Bitfield64 => Self::Bitfield64(r.u64()?),
            PropertyType::Int => Self::Int(r.i32()?),
            PropertyType::Colour => Self::Colour(r.array()?),
            PropertyType::Bitfield32 => Self::Bitfield32(r.u32()?),
            PropertyType::Int64 => Self::Int64(r.i64()?),
            PropertyType::DidRef => Self::DidRef(r.did()?),
            PropertyType::Bool => Self::Bool(r.bool8()?),
            PropertyType::BitfieldAny => {
                let bits = r.vle()?;
                let n = bits.div_ceil(8);
                let n = usize::try_from(n).unwrap_or(usize::MAX);
                Self::BitfieldAny {
                    bits,
                    bytes: r.bytes(n)?.to_vec(),
                }
            }
            PropertyType::Position | PropertyType::Unknown(_) => {
                return Err(FormatError::UnsupportedPropertyType { code: ty.code() });
            }
        })
    }
}

fn read_floats(r: &mut Reader<'_>, n: u32) -> Result<Vec<f32>> {
    let cap = r.ensure_elements(n, 4)?;
    let mut v = Vec::with_capacity(cap);
    for _ in 0..n {
        v.push(r.f32()?);
    }
    Ok(v)
}

fn read_waveform(r: &mut Reader<'_>) -> Result<Waveform> {
    let subtype = r.u32()?;
    let (values, extra) = match subtype {
        0 => (Vec::new(), None),
        1 => (read_floats(r, 1)?, None),
        10 => {
            let values = read_floats(r, 10)?;
            let scalar = r.f32()?;
            let flag = r.bool8()?;
            let k = r.u32()?;
            let cap = r.ensure_elements(k, 8)?;
            let mut pairs = Vec::with_capacity(cap);
            for _ in 0..k {
                pairs.push((r.f32()?, r.f32()?));
            }
            (
                values,
                Some(WaveformExtra {
                    scalar,
                    flag,
                    pairs,
                }),
            )
        }
        _ => (read_floats(r, 10)?, None),
    };
    Ok(Waveform {
        subtype,
        values,
        extra,
    })
}

/// One property definition (spec §6.2).
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyDefinition {
    /// Property ID.
    pub pid: u32,
    /// Value type.
    pub ty: PropertyType,
    /// Unknown field (OPEN).
    pub field_a: u32,
    /// Unknown field (OPEN).
    pub field_b: u32,
    /// Type-dependent reference: enum DID, DID map or the dictionary itself; else 0.
    pub data: Did,
    /// Unknown field (OPEN).
    pub field_c: u32,
    /// Flag bits; see [`FLAG_HAS_DEFAULT`], [`FLAG_HAS_MIN`], [`FLAG_HAS_MAX`].
    pub flags: u32,
    /// Default value.
    pub default: Option<PropertyValue>,
    /// Minimum value.
    pub minimum: Option<PropertyValue>,
    /// Maximum value.
    pub maximum: Option<PropertyValue>,
    /// Five unknown bytes after the marker (OPEN).
    pub field_d: [u8; 5],
    /// Child PIDs (array element or struct fields).
    pub children: Vec<u32>,
    /// Subset list of PIDs (meaning OPEN).
    pub subset: Vec<u32>,
}

impl PropertyDefinition {
    fn read(r: &mut Reader<'_>, pid: u32) -> Result<Self> {
        const WHAT: &str = "property definition";
        let echo_at = r.position();
        let echo = r.u32()?;
        if echo != pid {
            return Err(FormatError::malformed(
                WHAT,
                echo_at,
                format!("pid echo {echo:#x} differs from key {pid:#x}"),
            ));
        }
        let ty = PropertyType::from_code(r.u32()?);
        let field_a = r.u32()?;
        let field_b = r.u32()?;
        let data = r.did()?;
        let field_c = r.u32()?;
        let flags = r.u32()?;
        let read_opt = |bit: u32, r: &mut Reader<'_>| -> Result<Option<PropertyValue>> {
            if flags & bit != 0 && !ty.skips_definition_values() {
                PropertyValue::read(r, ty).map(Some)
            } else {
                Ok(None)
            }
        };
        let default = read_opt(FLAG_HAS_DEFAULT, r)?;
        let minimum = read_opt(FLAG_HAS_MIN, r)?;
        let maximum = read_opt(FLAG_HAS_MAX, r)?;
        let marker_at = r.position();
        let marker = r.u32()?;
        if marker != DEFINITION_MARKER {
            return Err(FormatError::malformed(
                WHAT,
                marker_at,
                format!("marker {marker:#010x}, expected {DEFINITION_MARKER:#010x}"),
            ));
        }
        let field_d = r.array()?;
        let child_count = r.vle()?;
        let cap = r.ensure_elements(child_count, 8)?;
        let mut children = Vec::with_capacity(cap);
        let mut mismatched = 0usize;
        for _ in 0..child_count {
            let a = r.u32()?;
            let b = r.u32()?;
            if a != b {
                mismatched += 1;
            }
            children.push(a);
        }
        if mismatched != 0 {
            // One event per definition, not per child (spec §13: the first copy is kept).
            tracing::debug!(pid, count = mismatched, "property child PID copies differ");
        }
        let subset_count = r.u32()?;
        let cap = r.ensure_elements(subset_count, 4)?;
        let mut subset = Vec::with_capacity(cap);
        for _ in 0..subset_count {
            subset.push(r.u32()?);
        }
        let term_at = r.position();
        let term = r.u32()?;
        if term != 0 {
            return Err(FormatError::malformed(
                WHAT,
                term_at,
                format!("terminator {term:#x}, expected 0"),
            ));
        }
        Ok(Self {
            pid,
            ty,
            field_a,
            field_b,
            data,
            field_c,
            flags,
            default,
            minimum,
            maximum,
            field_d,
            children,
            subset,
        })
    }
}

/// The master property dictionary (spec §6.1).
///
/// The decoded lists are read-only so that the lookup indexes always agree with them.
#[derive(Clone, PartialEq)]
pub struct PropertyDictionary {
    /// Record DID (0x34000000).
    pub did: Did,
    /// Eight reserved bytes after the DID (observed zero; meaning OPEN).
    pub reserved: [u8; 8],
    /// TSize header of the name list.
    pub names_size: TSize,
    /// Two-byte separator (observed `00 00`; meaning OPEN).
    pub separator: [u8; 2],
    /// TSize header of the definition list.
    pub definitions_size: TSize,
    names: Vec<(u32, String)>,
    definitions: Vec<PropertyDefinition>,
    name_index: HashMap<u32, usize>,
    /// Name → position in `names` of the first PID with that name.
    pid_by_name: HashMap<String, usize>,
    def_index: HashMap<u32, usize>,
}

impl fmt::Debug for PropertyDictionary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PropertyDictionary")
            .field("did", &self.did)
            .field("reserved", &self.reserved)
            .field("names_size", &self.names_size)
            .field("names", &self.names)
            .field("separator", &self.separator)
            .field("definitions_size", &self.definitions_size)
            .field("definitions", &self.definitions)
            .finish_non_exhaustive()
    }
}

impl PropertyDictionary {
    /// PID → name, in stored (bucket) order.
    #[must_use]
    pub fn names(&self) -> &[(u32, String)] {
        &self.names
    }

    /// Definitions in stored (bucket) order.
    #[must_use]
    pub fn definitions(&self) -> &[PropertyDefinition] {
        &self.definitions
    }

    /// Name of a PID.
    #[must_use]
    pub fn name(&self, pid: u32) -> Option<&str> {
        self.name_index
            .get(&pid)
            .and_then(|&i| self.names.get(i))
            .map(|(_, n)| n.as_str())
    }

    /// PID with the given name.
    #[must_use]
    pub fn pid(&self, name: &str) -> Option<u32> {
        self.pid_by_name
            .get(name)
            .and_then(|&i| self.names.get(i))
            .map(|(pid, _)| *pid)
    }

    /// Definition of a PID.
    #[must_use]
    pub fn definition(&self, pid: u32) -> Option<&PropertyDefinition> {
        self.def_index
            .get(&pid)
            .and_then(|&i| self.definitions.get(i))
    }
}

impl Record<'_> for PropertyDictionary {
    const NAME: &'static str = "property dictionary";

    fn did(&self) -> Did {
        self.did
    }

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_prologue(&mut r, Self::NAME, |k| k == DidKind::PropertyDictionary)?;
        let reserved = r.array()?;
        let names_size = r.tsize()?;
        let cap = r.ensure_elements(names_size.count, 5)?;
        let mut names = Vec::with_capacity(cap);
        let mut name_index = HashMap::with_capacity(cap);
        let mut pid_by_name = HashMap::with_capacity(cap);
        for i in 0..names_size.count as usize {
            let pid = r.u32()?;
            let name = r.pascal_string()?;
            name_index.entry(pid).or_insert(i);
            if !pid_by_name.contains_key(&name) {
                pid_by_name.insert(name.clone(), i);
            }
            names.push((pid, name));
        }
        let separator = r.array()?;
        let definitions_size = r.tsize()?;
        // Smallest definition: key + 7 u32 + marker + 5 + 1-byte VLE + subset count + term.
        let cap = r.ensure_elements(definitions_size.count, 4 + 28 + 4 + 5 + 1 + 4 + 4)?;
        let mut definitions = Vec::with_capacity(cap);
        let mut def_index = HashMap::with_capacity(cap);
        for i in 0..definitions_size.count as usize {
            let pid = r.u32()?;
            let def = PropertyDefinition::read(&mut r, pid)?;
            def_index.entry(pid).or_insert(i);
            definitions.push(def);
        }
        r.expect_end(Self::NAME)?;
        Ok(Self {
            did,
            reserved,
            names_size,
            names,
            separator,
            definitions_size,
            definitions,
            name_index,
            pid_by_name,
            def_index,
        })
    }
}
