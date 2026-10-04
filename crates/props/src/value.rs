// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed property values (`property-system.md` §3, §4).

use tessera_formats::{
    BitfieldAny, Did, Position, PropertyType, PropertyValue, StringInfo, Waveform,
};

use crate::error::{PropsError, Result};
use crate::set::PropertySet;

/// One (PID, value) pair: a top-level property, a struct field or an array element.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// Property ID (never 0).
    pub pid: u32,
    /// The value, of the type the registry defines for `pid`.
    pub value: Value,
}

impl Property {
    /// A property with the given PID and value.
    #[must_use]
    pub fn new(pid: u32, value: Value) -> Self {
        Self { pid, value }
    }
}

/// A property value (`property-system.md` §4), one variant per type code 1–22.
///
/// Structs are [`PropertySet`]s, so their fields can be looked up by PID or by name the
/// same way as top-level properties. Arrays keep their elements in stored order; each
/// element carries its own PID (§4.11).
///
/// The `as_*` accessors return `None` when the value has a different type; they never
/// convert between types, except the widening accessors [`Value::as_i64`] and
/// [`Value::as_f64`], which say which variants they accept.
///
/// Dropping, cloning, comparing and debug-printing a value recurse once per nesting
/// level. Decoded values are at most
/// [`tessera_formats::property::PROPERTY_MAX_DEPTH`] levels deep; a tree built by a
/// caller should stay within a few thousand levels, or the thread's stack overflows
/// (check with [`PropertySet::depth`]).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// 1: Latin-1 string (decoded to `String`).
    String(String),
    /// 2: string token.
    StringToken(u32),
    /// 3: waveform (animated scalar; subtype meanings OPEN).
    Waveform(Box<Waveform>),
    /// 4: timestamp (f64; epoch and unit OPEN).
    Timestamp(f64),
    /// 5: tri-state byte (meaning OPEN).
    TriState(u8),
    /// 6: x, y, z.
    Vector3([f32; 3]),
    /// 7: 64-bit instance ID.
    InstanceId(u64),
    /// 8: enum key; label it with [`crate::GameData::label`].
    Enum(u32),
    /// 9: float.
    Float(f32),
    /// 10: property ID; 0 means "none".
    PropertyId(u32),
    /// 11: struct.
    Struct(Box<PropertySet>),
    /// 12: array, elements in stored order.
    Array(Vec<Property>),
    /// 13: localizable text.
    StringInfo(Box<StringInfo>),
    /// 14: 64-bit bitfield.
    Bitfield64(u64),
    /// 15: signed 32-bit integer.
    Int(i32),
    /// 16: four colour bytes in stored order (fourth is alpha; RGB order OPEN).
    Colour([u8; 4]),
    /// 17: position.
    Position(Position),
    /// 18: 32-bit bitfield.
    Bitfield32(u32),
    /// 19: signed 64-bit integer.
    Int64(i64),
    /// 20: DID reference; the null DID means "none".
    DidRef(Did),
    /// 21: boolean.
    Bool(bool),
    /// 22: bitfield of any width.
    BitfieldAny(BitfieldAny),
}

impl Value {
    /// Converts a raw decoded value from `tessera-formats`, consuming it.
    ///
    /// The raw tree is at most [`tessera_formats::property::PROPERTY_MAX_DEPTH`] levels
    /// deep, so the recursion is bounded. Fails with [`PropsError::UnsupportedValue`] only
    /// for a raw variant this crate does not know.
    pub fn from_raw(raw: PropertyValue) -> Result<Self> {
        Self::from_raw_pid(raw, 0)
    }

    pub(crate) fn from_raw_pid(raw: PropertyValue, pid: u32) -> Result<Self> {
        Ok(match raw {
            PropertyValue::String(s) => Self::String(s),
            PropertyValue::StringToken(t) => Self::StringToken(t),
            PropertyValue::Waveform(w) => Self::Waveform(w),
            PropertyValue::Timestamp(t) => Self::Timestamp(t),
            PropertyValue::TriState(t) => Self::TriState(t),
            PropertyValue::Vector3(v) => Self::Vector3(v),
            PropertyValue::InstanceId(i) => Self::InstanceId(i),
            PropertyValue::Enum(k) => Self::Enum(k),
            PropertyValue::Float(f) => Self::Float(f),
            PropertyValue::PropertyId(p) => Self::PropertyId(p),
            PropertyValue::Struct(s) => {
                Self::Struct(Box::new(PropertySet::from_raw_entries(s.fields)?))
            }
            PropertyValue::Array(a) => Self::Array(
                a.elements
                    .into_iter()
                    .map(|e| Ok(Property::new(e.pid, Self::from_raw_pid(e.value, e.pid)?)))
                    .collect::<Result<_>>()?,
            ),
            PropertyValue::StringInfo(s) => Self::StringInfo(s),
            PropertyValue::Bitfield64(b) => Self::Bitfield64(b),
            PropertyValue::Int(i) => Self::Int(i),
            PropertyValue::Colour(c) => Self::Colour(c),
            PropertyValue::Position(p) => Self::Position(p),
            PropertyValue::Bitfield32(b) => Self::Bitfield32(b),
            PropertyValue::Int64(i) => Self::Int64(i),
            PropertyValue::DidRef(d) => Self::DidRef(d),
            PropertyValue::Bool(b) => Self::Bool(b),
            PropertyValue::BitfieldAny(b) => Self::BitfieldAny(b),
            other => {
                return Err(PropsError::UnsupportedValue {
                    pid,
                    ty: other.property_type(),
                });
            }
        })
    }

    /// The type of this value.
    #[must_use]
    pub fn property_type(&self) -> PropertyType {
        match self {
            Self::String(_) => PropertyType::String,
            Self::StringToken(_) => PropertyType::StringToken,
            Self::Waveform(_) => PropertyType::Waveform,
            Self::Timestamp(_) => PropertyType::Timestamp,
            Self::TriState(_) => PropertyType::TriState,
            Self::Vector3(_) => PropertyType::Vector3,
            Self::InstanceId(_) => PropertyType::InstanceId,
            Self::Enum(_) => PropertyType::Enum,
            Self::Float(_) => PropertyType::Float,
            Self::PropertyId(_) => PropertyType::PropertyId,
            Self::Struct(_) => PropertyType::Struct,
            Self::Array(_) => PropertyType::Array,
            Self::StringInfo(_) => PropertyType::StringInfo,
            Self::Bitfield64(_) => PropertyType::Bitfield64,
            Self::Int(_) => PropertyType::Int,
            Self::Colour(_) => PropertyType::Colour,
            Self::Position(_) => PropertyType::Position,
            Self::Bitfield32(_) => PropertyType::Bitfield32,
            Self::Int64(_) => PropertyType::Int64,
            Self::DidRef(_) => PropertyType::DidRef,
            Self::Bool(_) => PropertyType::Bool,
            Self::BitfieldAny(_) => PropertyType::BitfieldAny,
        }
    }

    /// The text of a string value (type 1).
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// The token of a string-token value (type 2).
    #[must_use]
    pub fn as_string_token(&self) -> Option<u32> {
        match self {
            Self::StringToken(t) => Some(*t),
            _ => None,
        }
    }

    /// A waveform value (type 3).
    #[must_use]
    pub fn as_waveform(&self) -> Option<&Waveform> {
        match self {
            Self::Waveform(w) => Some(w),
            _ => None,
        }
    }

    /// A timestamp value (type 4), unit OPEN.
    #[must_use]
    pub fn as_timestamp(&self) -> Option<f64> {
        match self {
            Self::Timestamp(t) => Some(*t),
            _ => None,
        }
    }

    /// A tri-state value (type 5).
    #[must_use]
    pub fn as_tri_state(&self) -> Option<u8> {
        match self {
            Self::TriState(t) => Some(*t),
            _ => None,
        }
    }

    /// A vector value (type 6).
    #[must_use]
    pub fn as_vector3(&self) -> Option<[f32; 3]> {
        match self {
            Self::Vector3(v) => Some(*v),
            _ => None,
        }
    }

    /// An instance ID (type 7).
    #[must_use]
    pub fn as_instance_id(&self) -> Option<u64> {
        match self {
            Self::InstanceId(i) => Some(*i),
            _ => None,
        }
    }

    /// The key of an enum value (type 8).
    #[must_use]
    pub fn as_enum(&self) -> Option<u32> {
        match self {
            Self::Enum(k) => Some(*k),
            _ => None,
        }
    }

    /// A float value (type 9).
    #[must_use]
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Self::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// A float (type 9) or timestamp (type 4), widened to f64.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(f64::from(*f)),
            Self::Timestamp(t) => Some(*t),
            _ => None,
        }
    }

    /// A property-ID value (type 10), including 0 ("none").
    #[must_use]
    pub fn as_pid(&self) -> Option<u32> {
        match self {
            Self::PropertyId(p) => Some(*p),
            _ => None,
        }
    }

    /// The fields of a struct value (type 11).
    #[must_use]
    pub fn as_struct(&self) -> Option<&PropertySet> {
        match self {
            Self::Struct(s) => Some(s),
            _ => None,
        }
    }

    /// The elements of an array value (type 12).
    #[must_use]
    pub fn as_array(&self) -> Option<&[Property]> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// A StringInfo value (type 13).
    #[must_use]
    pub fn as_string_info(&self) -> Option<&StringInfo> {
        match self {
            Self::StringInfo(s) => Some(s),
            _ => None,
        }
    }

    /// A 32-bit integer (type 15).
    #[must_use]
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Self::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// A 32-bit (type 15) or 64-bit (type 19) integer, widened to i64.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(i64::from(*i)),
            Self::Int64(i) => Some(*i),
            _ => None,
        }
    }

    /// A colour value (type 16), bytes in stored order.
    #[must_use]
    pub fn as_colour(&self) -> Option<[u8; 4]> {
        match self {
            Self::Colour(c) => Some(*c),
            _ => None,
        }
    }

    /// A position value (type 17).
    #[must_use]
    pub fn as_position(&self) -> Option<&Position> {
        match self {
            Self::Position(p) => Some(p),
            _ => None,
        }
    }

    /// A DID reference (type 20), including the null DID.
    #[must_use]
    pub fn as_did(&self) -> Option<Did> {
        match self {
            Self::DidRef(d) => Some(*d),
            _ => None,
        }
    }

    /// A DID reference (type 20) that is not null (`property-system.md` §4.9).
    #[must_use]
    pub fn did_ref(&self) -> Option<Did> {
        self.as_did().filter(|d| !d.is_null())
    }

    /// A boolean value (type 21).
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The mask of a fixed-width bitfield (type 14 or 18), widened to u64.
    #[must_use]
    pub fn as_bits(&self) -> Option<u64> {
        match self {
            Self::Bitfield64(b) => Some(*b),
            Self::Bitfield32(b) => Some(u64::from(*b)),
            _ => None,
        }
    }

    /// A variable-width bitfield (type 22).
    #[must_use]
    pub fn as_bitfield_any(&self) -> Option<&BitfieldAny> {
        match self {
            Self::BitfieldAny(b) => Some(b),
            _ => None,
        }
    }

    /// Whether this is a bitfield of any width (types 14, 18, 22).
    #[must_use]
    pub fn is_bitfield(&self) -> bool {
        matches!(
            self,
            Self::Bitfield64(_) | Self::Bitfield32(_) | Self::BitfieldAny(_)
        )
    }

    /// Indices of the set bits of a bitfield (types 14, 18, 22), ascending; empty for any
    /// other type. Bit *i* is labelled by enum key *i* + 1 (`property-system.md` §7.3).
    ///
    /// Allocates one `u32` per set bit, so a wide type-22 value yields up to 32 bytes per
    /// stored byte; use [`Value::iter_set_bits`] to stop early.
    #[must_use]
    pub fn set_bits(&self) -> Vec<u32> {
        self.iter_set_bits().collect()
    }

    /// Iterates the indices of the set bits of a bitfield (types 14, 18, 22), ascending;
    /// empty for any other type.
    ///
    /// For type 22 only bits below both `bits` and 8 × the stored byte count are visited,
    /// and zero bytes are skipped whole, so the cost follows the stored bytes, not the
    /// declared width.
    pub fn iter_set_bits(&self) -> SetBits<'_> {
        let (bytes, limit) = match self {
            Self::Bitfield64(b) => (BitBytes::Fixed(b.to_le_bytes()), 64),
            Self::Bitfield32(b) => (BitBytes::Fixed(u64::from(*b).to_le_bytes()), 32),
            Self::BitfieldAny(b) => (BitBytes::Slice(&b.bytes), any_limit(b)),
            _ => (BitBytes::Slice(&[]), 0),
        };
        SetBits {
            bytes,
            limit,
            byte: 0,
            pending: 0,
        }
    }

    /// Number of set bits of a bitfield (types 14, 18, 22), counted without listing them;
    /// 0 for any other type. Same bit range as [`Value::iter_set_bits`].
    #[must_use]
    pub fn set_bit_count(&self) -> u64 {
        match self {
            Self::Bitfield64(b) => u64::from(b.count_ones()),
            Self::Bitfield32(b) => u64::from(b.count_ones()),
            Self::BitfieldAny(b) => {
                let limit = any_limit(b);
                let full = (limit / 8) as usize;
                let mut n: u64 = b
                    .bytes
                    .iter()
                    .take(full)
                    .map(|x| u64::from(x.count_ones()))
                    .sum();
                let rest = limit % 8;
                if let Some(x) = b.bytes.get(full).filter(|_| rest != 0) {
                    n += u64::from((x & ((1u8 << rest) - 1)).count_ones());
                }
                n
            }
            _ => 0,
        }
    }

    /// Nested properties of a struct (fields, stored order) or array (elements); empty
    /// for scalars.
    #[must_use]
    pub fn children(&self) -> &[Property] {
        match self {
            Self::Struct(s) => s.as_slice(),
            Self::Array(a) => a,
            _ => &[],
        }
    }
}

/// Bits of a type-22 value that are both declared and stored.
fn any_limit(b: &BitfieldAny) -> u32 {
    let stored = u32::try_from(b.bytes.len())
        .unwrap_or(u32::MAX)
        .saturating_mul(8);
    b.bits.min(stored)
}

#[derive(Debug, Clone)]
enum BitBytes<'a> {
    Fixed([u8; 8]),
    Slice(&'a [u8]),
}

impl BitBytes<'_> {
    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Fixed(b) => b,
            Self::Slice(s) => s,
        }
    }
}

/// Iterator over the set bits of a bitfield value, from [`Value::iter_set_bits`].
#[derive(Debug, Clone)]
pub struct SetBits<'a> {
    bytes: BitBytes<'a>,
    /// Bits at or past this index are not visited.
    limit: u32,
    /// Index of the next byte to load.
    byte: usize,
    /// Unvisited set bits of the byte before `byte`.
    pending: u8,
}

impl Iterator for SetBits<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        loop {
            if self.pending != 0 {
                let bit = self.pending.trailing_zeros();
                self.pending &= self.pending - 1;
                // `byte` is at least 1 here and at most `limit / 8 + 1`, so this fits.
                let i = u32::try_from(self.byte - 1).ok()?.checked_mul(8)? + bit;
                if i < self.limit {
                    return Some(i);
                }
                // Bits only grow from here on.
                self.pending = 0;
                self.byte = usize::MAX;
                return None;
            }
            let end = (self.limit as usize).div_ceil(8);
            let bytes = self.bytes.as_slice();
            // Skip zero bytes in one pass.
            let rest = bytes.get(self.byte..end.min(bytes.len()))?;
            let skip = rest.iter().position(|&b| b != 0)?;
            self.byte += skip;
            self.pending = *bytes.get(self.byte)?;
            self.byte += 1;
        }
    }
}

macro_rules! impl_from {
    ($($t:ty => $v:ident),* $(,)?) => {
        $(impl From<$t> for Value {
            fn from(x: $t) -> Self {
                Self::$v(x)
            }
        })*
    };
}

impl_from! {
    String => String,
    f32 => Float,
    i32 => Int,
    i64 => Int64,
    bool => Bool,
    Did => DidRef,
    [f32; 3] => Vector3,
    Position => Position,
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::String(s.to_owned())
    }
}

impl From<PropertySet> for Value {
    fn from(s: PropertySet) -> Self {
        Self::Struct(Box::new(s))
    }
}

impl From<StringInfo> for Value {
    fn from(s: StringInfo) -> Self {
        Self::StringInfo(Box::new(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_stays_small() {
        assert!(std::mem::size_of::<Value>() <= 40);
        assert!(std::mem::size_of::<Property>() <= 48);
    }

    #[test]
    fn accessors_match_only_their_type() {
        let v = Value::Int(-5);
        assert_eq!(v.as_i32(), Some(-5));
        assert_eq!(v.as_i64(), Some(-5));
        assert_eq!(v.as_f32(), None);
        assert_eq!(v.as_bool(), None);
        assert_eq!(v.property_type(), PropertyType::Int);

        assert_eq!(Value::Int64(1 << 40).as_i64(), Some(1 << 40));
        assert_eq!(Value::Int64(1).as_i32(), None);
        assert_eq!(Value::Float(1.5).as_f64(), Some(1.5));
        assert_eq!(Value::Timestamp(2.0).as_f64(), Some(2.0));
        assert_eq!(Value::from("x").as_str(), Some("x"));
        assert_eq!(Value::DidRef(Did(0)).as_did(), Some(Did(0)));
        assert_eq!(Value::DidRef(Did(0)).did_ref(), None);
        assert_eq!(
            Value::DidRef(Did(0x7000_0001)).did_ref(),
            Some(Did(0x7000_0001))
        );
        assert_eq!(Value::Bool(true).as_bool(), Some(true));
        assert_eq!(Value::Enum(3).as_enum(), Some(3));
        assert_eq!(Value::PropertyId(0).as_pid(), Some(0));
        assert!(Value::Int(1).children().is_empty());
    }

    #[test]
    fn bitfield_bits() {
        assert_eq!(Value::Bitfield32(0b1010).set_bits(), vec![1, 3]);
        assert_eq!(Value::Bitfield64(1 << 63).set_bits(), vec![63]);
        assert_eq!(Value::Bitfield32(0b1010).as_bits(), Some(10));
        let any = BitfieldAny {
            bits: 10,
            bytes: vec![0x01, 0xFE],
        };
        // Bits past `bits` (10..16 of byte 1) are ignored.
        assert_eq!(Value::BitfieldAny(any).set_bits(), vec![0, 9]);
        assert!(Value::Int(3).set_bits().is_empty());
        assert!(!Value::Int(3).is_bitfield());
    }

    #[test]
    fn bit_iteration_follows_stored_bytes() {
        // Declared width far past the stored bytes: no time spent on the missing ones.
        let huge = Value::BitfieldAny(BitfieldAny {
            bits: u32::MAX,
            bytes: vec![],
        });
        let t = std::time::Instant::now();
        assert!(huge.set_bits().is_empty());
        assert_eq!(huge.set_bit_count(), 0);
        assert!(t.elapsed() < std::time::Duration::from_secs(1));

        let sparse = Value::BitfieldAny(BitfieldAny {
            bits: u32::MAX,
            bytes: {
                let mut b = vec![0u8; 1 << 20];
                b[0] = 0x81;
                b[(1 << 20) - 1] = 0x80;
                b
            },
        });
        assert_eq!(sparse.set_bits(), vec![0, 7, (1 << 23) - 1]);
        assert_eq!(sparse.set_bit_count(), 3);

        // Bits past `bits` are ignored, also by the count.
        let partial = Value::BitfieldAny(BitfieldAny {
            bits: 12,
            bytes: vec![0xFF, 0xFF],
        });
        assert_eq!(partial.set_bits(), (0..12).collect::<Vec<_>>());
        assert_eq!(partial.set_bit_count(), 12);
        assert_eq!(partial.iter_set_bits().take(3).count(), 3);

        assert_eq!(Value::Bitfield64(u64::MAX).set_bit_count(), 64);
        assert_eq!(Value::Bitfield64(u64::MAX).set_bits().len(), 64);
        assert_eq!(Value::Bitfield32(u32::MAX).set_bits().last(), Some(&31));
        assert_eq!(Value::Int(-1).set_bit_count(), 0);
    }
}
