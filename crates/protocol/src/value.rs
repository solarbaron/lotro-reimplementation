// SPDX-License-Identifier: GPL-3.0-or-later
//! Property replication value model (`tessera-protocol.md` §7).
//!
//! Properties are Tessera's own 16-bit IDs, unrelated to the data files' property IDs. Values
//! are self-describing (a tag, then the payload), so receivers step over IDs they do not know.

use crate::error::{ProtocolError, Result};
use crate::limits::{
    MAX_ARRAY_LEN, MAX_MOVE_SPEED, MAX_PROPERTIES, MAX_STRING_VALUE_LEN, MAX_STRUCT_FIELDS,
    MAX_VALUE_BYTES, MAX_VALUE_DEPTH,
};
use crate::types::Vec3;
use crate::wire::{Reader, TextRule, Writer};

/// Property IDs defined in v1.0 (§7.3) and the ID ranges of §7.1.
pub mod pid {
    /// Invalid; a protocol violation on the wire.
    pub const INVALID: u16 = 0x0000;
    /// Enum: 0 Relaxed, 1 Ready, 2 Seated. All observers. Default 0.
    pub const STANCE: u16 = 0x0001;
    /// Struct {1 anim Enum, 2 started Time, 3 looping Bool}. All observers. Default: none.
    pub const EMOTE: u16 = 0x0002;
    /// Float32 m/s in (0, 100]. Owner only. Default 2.0.
    pub const WALK_SPEED: u16 = 0x0003;
    /// Float32 m/s in (0, 100]. Owner only. Default 6.0.
    pub const RUN_SPEED: u16 = 0x0004;
    /// Last ID owned by the Tessera protocol.
    pub const PROTOCOL_LAST: u16 = 0x3FFF;
    /// First reserved ID.
    pub const RESERVED_FIRST: u16 = 0x4000;
    /// First server-private ID (an operator's extensions).
    pub const PRIVATE_FIRST: u16 = 0x8000;

    /// Struct field IDs of [`EMOTE`].
    pub mod emote {
        /// Enum: opaque animation key, 0 = no emote.
        pub const ANIM: u16 = 1;
        /// Time: server time the emote started, µs.
        pub const STARTED: u16 = 2;
        /// Bool: play until stopped.
        pub const LOOPING: u16 = 3;
    }

    /// Whether this version defines `id`.
    #[must_use]
    pub fn is_defined(id: u16) -> bool {
        matches!(id, STANCE | EMOTE | WALK_SPEED | RUN_SPEED)
    }

    /// Whether `id` is sent only to the connection that controls the entity (§7.3).
    #[must_use]
    pub fn is_owner_only(id: u16) -> bool {
        matches!(id, WALK_SPEED | RUN_SPEED)
    }
}

/// The tag byte of a tagged value (§7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ValueTag {
    /// 0x00: removal (top level of `EntityUpdate` only).
    None = 0x00,
    /// 0x01.
    Bool = 0x01,
    /// 0x02.
    Int32 = 0x02,
    /// 0x03.
    Int64 = 0x03,
    /// 0x04.
    Float32 = 0x04,
    /// 0x05.
    Enum = 0x05,
    /// 0x06.
    DataId = 0x06,
    /// 0x07.
    String = 0x07,
    /// 0x08.
    Vector3 = 0x08,
    /// 0x09.
    Bitfield32 = 0x09,
    /// 0x0A.
    Bitfield64 = 0x0A,
    /// 0x0B.
    EntityRef = 0x0B,
    /// 0x0C.
    Time = 0x0C,
    /// 0x0D.
    Colour = 0x0D,
    /// 0x0E.
    Array = 0x0E,
    /// 0x0F.
    Struct = 0x0F,
}

impl ValueTag {
    /// The tag for a byte, if v1.0 defines it.
    #[must_use]
    pub fn from_byte(b: u8) -> Option<Self> {
        Some(match b {
            0x00 => Self::None,
            0x01 => Self::Bool,
            0x02 => Self::Int32,
            0x03 => Self::Int64,
            0x04 => Self::Float32,
            0x05 => Self::Enum,
            0x06 => Self::DataId,
            0x07 => Self::String,
            0x08 => Self::Vector3,
            0x09 => Self::Bitfield32,
            0x0A => Self::Bitfield64,
            0x0B => Self::EntityRef,
            0x0C => Self::Time,
            0x0D => Self::Colour,
            0x0E => Self::Array,
            0x0F => Self::Struct,
            _ => return None,
        })
    }
}

/// A tagged property value (§7.2).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Removes the property (only as a top-level value in `EntityUpdate`).
    None,
    /// A boolean.
    Bool(bool),
    /// A signed 32-bit integer.
    Int32(i32),
    /// A signed 64-bit integer.
    Int64(i64),
    /// A finite f32.
    Float32(f32),
    /// An enum key.
    Enum(u32),
    /// A DID (0 = none), opaque.
    DataId(u32),
    /// 0–1,024 bytes of text; control characters are sanitised on receipt.
    String(String),
    /// Three finite f32.
    Vector3(Vec3),
    /// 32 bits.
    Bitfield32(u32),
    /// 64 bits.
    Bitfield64(u64),
    /// An entity ID (0 = none).
    EntityRef(u64),
    /// Server time in µs (§11.1).
    Time(u64),
    /// R, G, B, A.
    Colour([u8; 4]),
    /// 0–256 values, all with the same tag, none of them None.
    Array(Vec<Value>),
    /// 0–64 fields: (field ID, non-zero and strictly ascending; value, not None).
    Struct(Vec<(u16, Value)>),
}

impl Value {
    /// The value's tag.
    #[must_use]
    pub fn tag(&self) -> ValueTag {
        match self {
            Self::None => ValueTag::None,
            Self::Bool(_) => ValueTag::Bool,
            Self::Int32(_) => ValueTag::Int32,
            Self::Int64(_) => ValueTag::Int64,
            Self::Float32(_) => ValueTag::Float32,
            Self::Enum(_) => ValueTag::Enum,
            Self::DataId(_) => ValueTag::DataId,
            Self::String(_) => ValueTag::String,
            Self::Vector3(_) => ValueTag::Vector3,
            Self::Bitfield32(_) => ValueTag::Bitfield32,
            Self::Bitfield64(_) => ValueTag::Bitfield64,
            Self::EntityRef(_) => ValueTag::EntityRef,
            Self::Time(_) => ValueTag::Time,
            Self::Colour(_) => ValueTag::Colour,
            Self::Array(_) => ValueTag::Array,
            Self::Struct(_) => ValueTag::Struct,
        }
    }

    /// The field `id` of a Struct value.
    #[must_use]
    pub fn field(&self, id: u16) -> Option<&Value> {
        match self {
            Self::Struct(fields) => fields.iter().find(|(f, _)| *f == id).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Encodes one top-level value (depth 1), checking every rule of §7.2. On error `out` is
    /// left as it was.
    pub fn encode(&self, out: &mut Vec<u8>, allow_none: bool) -> Result<()> {
        let start = out.len();
        let r = encode_top(&mut Writer::new(out), self, allow_none);
        if r.is_err() {
            out.truncate(start);
        }
        r
    }

    /// Decodes one top-level value from the start of `bytes`; returns it and the bytes used.
    pub fn decode(bytes: &[u8], allow_none: bool) -> Result<(Self, usize)> {
        let mut r = Reader::new(bytes);
        let v = decode_top(&mut r, allow_none)?;
        Ok((v, bytes.len() - r.remaining()))
    }
}

const STRING_VALUE: TextRule = TextRule::sanitised("string value", 0, MAX_STRING_VALUE_LEN);

fn encode_top(w: &mut Writer<'_>, v: &Value, allow_none: bool) -> Result<()> {
    if matches!(v, Value::None) && !allow_none {
        return Err(ProtocolError::NoneNotAllowed);
    }
    let start = w.len();
    encode_value(w, v, 1)?;
    if w.len() - start > MAX_VALUE_BYTES {
        return Err(ProtocolError::ValueTooLarge);
    }
    Ok(())
}

fn encode_value(w: &mut Writer<'_>, v: &Value, depth: usize) -> Result<()> {
    if depth > MAX_VALUE_DEPTH {
        return Err(ProtocolError::ValueTooDeep);
    }
    w.u8(v.tag() as u8);
    match v {
        Value::None => {}
        Value::Bool(b) => w.bool(*b),
        Value::Int32(i) => w.i32(*i),
        Value::Int64(i) => w.i64(*i),
        Value::Float32(f) => w.f32("float32 value", *f)?,
        Value::Enum(u) | Value::DataId(u) | Value::Bitfield32(u) => w.u32(*u),
        Value::String(s) => w.str(STRING_VALUE, s)?,
        Value::Vector3(v) => v.encode(w, "vector3 value")?,
        Value::Bitfield64(u) | Value::EntityRef(u) | Value::Time(u) => w.u64(*u),
        Value::Colour(c) => {
            for b in c {
                w.u8(*b);
            }
        }
        Value::Array(items) => {
            let n = items.len();
            if n > MAX_ARRAY_LEN {
                return Err(ProtocolError::Count {
                    field: "array",
                    count: n,
                    min: 0,
                    max: MAX_ARRAY_LEN,
                });
            }
            w.u16(u16::try_from(n).unwrap_or(u16::MAX));
            let tag = items.first().map(Value::tag);
            for item in items {
                if matches!(item, Value::None) {
                    return Err(ProtocolError::NoneNotAllowed);
                }
                if Some(item.tag()) != tag {
                    return Err(ProtocolError::MixedArray);
                }
                encode_value(w, item, depth + 1)?;
            }
        }
        Value::Struct(fields) => {
            let n = fields.len();
            if n > MAX_STRUCT_FIELDS {
                return Err(ProtocolError::Count {
                    field: "struct",
                    count: n,
                    min: 0,
                    max: MAX_STRUCT_FIELDS,
                });
            }
            w.u16(u16::try_from(n).unwrap_or(u16::MAX));
            let mut last = 0u16;
            for (id, item) in fields {
                if *id <= last {
                    return Err(ProtocolError::IdOrder("struct fields"));
                }
                last = *id;
                if matches!(item, Value::None) {
                    return Err(ProtocolError::NoneNotAllowed);
                }
                w.u16(*id);
                encode_value(w, item, depth + 1)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn decode_top(r: &mut Reader<'_>, allow_none: bool) -> Result<Value> {
    let rest = r.rest();
    let cap = rest.len().min(MAX_VALUE_BYTES);
    let mut sub = Reader::new(rest.get(..cap).unwrap_or(rest));
    let v = match decode_value(&mut sub, 1) {
        Ok(v) => v,
        Err(ProtocolError::Truncated) if rest.len() > cap => {
            return Err(ProtocolError::ValueTooLarge);
        }
        Err(e) => return Err(e),
    };
    if matches!(v, Value::None) && !allow_none {
        return Err(ProtocolError::NoneNotAllowed);
    }
    let used = cap - sub.remaining();
    r.bytes(used)?;
    Ok(v)
}

fn decode_value(r: &mut Reader<'_>, depth: usize) -> Result<Value> {
    if depth > MAX_VALUE_DEPTH {
        return Err(ProtocolError::ValueTooDeep);
    }
    let tag_byte = r.u8()?;
    let tag = ValueTag::from_byte(tag_byte).ok_or(ProtocolError::UnknownValueTag(tag_byte))?;
    Ok(match tag {
        ValueTag::None => Value::None,
        ValueTag::Bool => Value::Bool(r.bool("bool value")?),
        ValueTag::Int32 => Value::Int32(r.i32()?),
        ValueTag::Int64 => Value::Int64(r.i64()?),
        ValueTag::Float32 => Value::Float32(r.f32("float32 value")?),
        ValueTag::Enum => Value::Enum(r.u32()?),
        ValueTag::DataId => Value::DataId(r.u32()?),
        ValueTag::String => Value::String(r.str(STRING_VALUE)?),
        ValueTag::Vector3 => Value::Vector3(Vec3::decode(r, "vector3 value")?),
        ValueTag::Bitfield32 => Value::Bitfield32(r.u32()?),
        ValueTag::Bitfield64 => Value::Bitfield64(r.u64()?),
        ValueTag::EntityRef => Value::EntityRef(r.u64()?),
        ValueTag::Time => Value::Time(r.u64()?),
        ValueTag::Colour => {
            let b = r.bytes(4)?;
            let mut c = [0u8; 4];
            c.copy_from_slice(b);
            Value::Colour(c)
        }
        ValueTag::Array => {
            let n = usize::from(r.u16()?);
            // Every element takes at least one byte.
            if n > MAX_ARRAY_LEN || n > r.remaining() {
                if n > MAX_ARRAY_LEN {
                    return Err(ProtocolError::Count {
                        field: "array",
                        count: n,
                        min: 0,
                        max: MAX_ARRAY_LEN,
                    });
                }
                return Err(ProtocolError::Truncated);
            }
            let mut items = Vec::with_capacity(n);
            let mut first: Option<ValueTag> = None;
            for _ in 0..n {
                let item = decode_value(r, depth + 1)?;
                if matches!(item, Value::None) {
                    return Err(ProtocolError::NoneNotAllowed);
                }
                match first {
                    None => first = Some(item.tag()),
                    Some(t) if t != item.tag() => return Err(ProtocolError::MixedArray),
                    Some(_) => {}
                }
                items.push(item);
            }
            Value::Array(items)
        }
        ValueTag::Struct => {
            let n = usize::from(r.u16()?);
            if n > MAX_STRUCT_FIELDS {
                return Err(ProtocolError::Count {
                    field: "struct",
                    count: n,
                    min: 0,
                    max: MAX_STRUCT_FIELDS,
                });
            }
            // Every field takes at least three bytes.
            if n.saturating_mul(3) > r.remaining() {
                return Err(ProtocolError::Truncated);
            }
            let mut fields = Vec::with_capacity(n);
            let mut last = 0u16;
            for _ in 0..n {
                let id = r.u16()?;
                if id <= last {
                    return Err(ProtocolError::IdOrder("struct fields"));
                }
                last = id;
                let item = decode_value(r, depth + 1)?;
                if matches!(item, Value::None) {
                    return Err(ProtocolError::NoneNotAllowed);
                }
                fields.push((id, item));
            }
            Value::Struct(fields)
        }
    })
}

/// One PropList entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// The property ID (non-zero).
    pub id: u16,
    /// Its value.
    pub value: Value,
}

impl Property {
    /// A property.
    #[must_use]
    pub fn new(id: u16, value: Value) -> Self {
        Self { id, value }
    }
}

/// Checks a value against the kind of a property ID this version defines (§7.2, §7.3).
/// None (removal) is accepted for every ID; unknown IDs are accepted.
pub fn check_property_kind(id: u16, value: &Value) -> Result<()> {
    let ok = match (id, value) {
        (_, Value::None) => true,
        (pid::STANCE, v) => matches!(v, Value::Enum(_)),
        (pid::WALK_SPEED | pid::RUN_SPEED, v) => matches!(v, Value::Float32(_)),
        (pid::EMOTE, Value::Struct(fields)) => fields.iter().all(|(f, v)| match *f {
            pid::emote::ANIM => matches!(v, Value::Enum(_)),
            pid::emote::STARTED => matches!(v, Value::Time(_)),
            pid::emote::LOOPING => matches!(v, Value::Bool(_)),
            _ => true,
        }),
        (pid::EMOTE, _) => false,
        _ => true,
    };
    if ok {
        Ok(())
    } else {
        Err(ProtocolError::WrongPropertyKind { pid: id })
    }
}

/// How a PropList is checked: `EntityCreate` (no removals, any count) or `EntityUpdate`
/// (removals allowed, 1–256 entries).
#[derive(Debug, Clone, Copy)]
pub(crate) struct PropListRule {
    pub allow_none: bool,
    pub min: usize,
}

pub(crate) fn encode_prop_list(
    w: &mut Writer<'_>,
    props: &[Property],
    rule: PropListRule,
) -> Result<()> {
    let n = props.len();
    if n > MAX_PROPERTIES || n < rule.min {
        return Err(ProtocolError::Count {
            field: "properties",
            count: n,
            min: rule.min,
            max: MAX_PROPERTIES,
        });
    }
    w.u16(u16::try_from(n).unwrap_or(u16::MAX));
    let mut last = 0u16;
    for p in props {
        if p.id <= last {
            return Err(ProtocolError::IdOrder("properties"));
        }
        last = p.id;
        check_property_kind(p.id, &p.value)?;
        w.u16(p.id);
        encode_top(w, &p.value, rule.allow_none)?;
    }
    Ok(())
}

pub(crate) fn decode_prop_list(r: &mut Reader<'_>, rule: PropListRule) -> Result<Vec<Property>> {
    let n = usize::from(r.u16()?);
    if n > MAX_PROPERTIES || n < rule.min {
        return Err(ProtocolError::Count {
            field: "properties",
            count: n,
            min: rule.min,
            max: MAX_PROPERTIES,
        });
    }
    // Every entry takes at least three bytes.
    if n.saturating_mul(3) > r.remaining() {
        return Err(ProtocolError::Truncated);
    }
    let mut props = Vec::with_capacity(n);
    let mut last = 0u16;
    for _ in 0..n {
        let id = r.u16()?;
        if id <= last {
            return Err(ProtocolError::IdOrder("properties"));
        }
        last = id;
        let value = decode_top(r, rule.allow_none)?;
        check_property_kind(id, &value)?;
        props.push(Property { id, value });
    }
    Ok(props)
}

/// The player's stance (`STANCE`, §7.3; `StanceSet`, §6.2.16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Stance {
    /// 0.
    #[default]
    Relaxed,
    /// 1: held items drawn.
    Ready,
    /// 2.
    Seated,
}

impl Stance {
    /// The wire value.
    #[must_use]
    pub fn to_u8(self) -> u8 {
        match self {
            Self::Relaxed => 0,
            Self::Ready => 1,
            Self::Seated => 2,
        }
    }

    /// The stance for a wire value, if defined.
    #[must_use]
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Relaxed),
            1 => Some(Self::Ready),
            2 => Some(Self::Seated),
            _ => None,
        }
    }

    /// The stance a `STANCE` property value means: unknown values and an absent or
    /// wrongly-kinded value are Relaxed (§7.3).
    #[must_use]
    pub fn from_property(value: Option<&Value>) -> Self {
        match value {
            Some(Value::Enum(v)) => u8::try_from(*v)
                .ok()
                .and_then(Self::from_u8)
                .unwrap_or_default(),
            _ => Self::Relaxed,
        }
    }

    /// The `STANCE` property value.
    #[must_use]
    pub fn to_property(self) -> Value {
        Value::Enum(u32::from(self.to_u8()))
    }
}

/// The `EMOTE` property (§7.3) as a typed value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Emote {
    /// Opaque animation key from the client's data; 0 = no emote.
    pub anim: u32,
    /// Server time the emote started, µs.
    pub started: u64,
    /// Play until stopped.
    pub looping: bool,
}

impl Emote {
    /// Reads an `EMOTE` Struct value. Missing fields take their defaults (anim 0, started =
    /// `message_time`, looping false). Returns `None` for no emote (absent value, anim 0, or
    /// not a Struct).
    #[must_use]
    pub fn from_property(value: Option<&Value>, message_time: u64) -> Option<Self> {
        let v = value?;
        if !matches!(v, Value::Struct(_)) {
            return None;
        }
        let anim = match v.field(pid::emote::ANIM) {
            Some(Value::Enum(a)) => *a,
            _ => 0,
        };
        if anim == 0 {
            return None;
        }
        let started = match v.field(pid::emote::STARTED) {
            Some(Value::Time(t)) => *t,
            _ => message_time,
        };
        let looping = matches!(v.field(pid::emote::LOOPING), Some(Value::Bool(true)));
        Some(Self {
            anim,
            started,
            looping,
        })
    }

    /// The `EMOTE` Struct value with all three fields.
    #[must_use]
    pub fn to_property(self) -> Value {
        Value::Struct(vec![
            (pid::emote::ANIM, Value::Enum(self.anim)),
            (pid::emote::STARTED, Value::Time(self.started)),
            (pid::emote::LOOPING, Value::Bool(self.looping)),
        ])
    }
}

/// Reads a `WALK_SPEED` / `RUN_SPEED` value: a Float32 in (0, 100], else `default`.
#[must_use]
pub fn speed_from_property(value: Option<&Value>, default: f32) -> f32 {
    match value {
        Some(Value::Float32(v)) if *v > 0.0 && *v <= MAX_MOVE_SPEED => *v,
        _ => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(v: &Value) -> Vec<u8> {
        let mut out = Vec::new();
        v.encode(&mut out, true).unwrap();
        out
    }

    #[test]
    fn depth_limit() {
        // depth 4: Array(Array(Array(Bool))) has Bool at depth 4.
        let ok = Value::Array(vec![Value::Array(vec![Value::Array(vec![Value::Bool(
            true,
        )])])]);
        let bytes = enc(&ok);
        assert_eq!(Value::decode(&bytes, false).unwrap().0, ok);
        let deep = Value::Array(vec![ok.clone()]);
        let mut out = Vec::new();
        assert_eq!(
            deep.encode(&mut out, false),
            Err(ProtocolError::ValueTooDeep)
        );
        let mut bytes = vec![0x0e, 1, 0];
        bytes.extend_from_slice(&enc(&ok));
        assert_eq!(
            Value::decode(&bytes, false),
            Err(ProtocolError::ValueTooDeep)
        );
    }

    #[test]
    fn size_limit() {
        let s = Value::String("x".repeat(1024));
        let big = Value::Array(vec![s.clone(); 8]); // 3 + 8 * 1027 > 8192
        let mut out = Vec::new();
        assert_eq!(
            big.encode(&mut out, false),
            Err(ProtocolError::ValueTooLarge)
        );
        let mut bytes = vec![0x0e, 8, 0];
        for _ in 0..8 {
            bytes.extend_from_slice(&enc(&s));
        }
        assert_eq!(
            Value::decode(&bytes, false),
            Err(ProtocolError::ValueTooLarge)
        );
        let fits = Value::Array(vec![s; 7]);
        let bytes = enc(&fits);
        assert_eq!(Value::decode(&bytes, false).unwrap().0, fits);
    }

    #[test]
    fn arrays_and_structs() {
        assert_eq!(
            Value::decode(&[0x0e, 2, 0, 1, 1, 2, 0, 0, 0, 0], false),
            Err(ProtocolError::MixedArray)
        );
        assert_eq!(
            Value::decode(&[0x0e, 1, 0, 0], false),
            Err(ProtocolError::NoneNotAllowed)
        );
        assert_eq!(
            Value::decode(&[0x0f, 2, 0, 2, 0, 1, 1, 1, 0, 1, 1], false),
            Err(ProtocolError::IdOrder("struct fields"))
        );
        assert_eq!(
            Value::decode(&[0x10], false),
            Err(ProtocolError::UnknownValueTag(0x10))
        );
        assert_eq!(
            Value::decode(&[0x00], false),
            Err(ProtocolError::NoneNotAllowed)
        );
        assert_eq!(Value::decode(&[0x00], true).unwrap(), (Value::None, 1));
        // Claimed count far above what remains: rejected before allocating.
        assert_eq!(
            Value::decode(&[0x0e, 0, 1], false),
            Err(ProtocolError::Truncated)
        );
    }

    #[test]
    fn typed_helpers() {
        assert_eq!(
            Stance::from_property(Some(&Value::Enum(7))),
            Stance::Relaxed
        );
        assert_eq!(Stance::from_property(Some(&Value::Enum(2))), Stance::Seated);
        let e = Emote {
            anim: 5,
            started: 9,
            looping: true,
        };
        assert_eq!(Emote::from_property(Some(&e.to_property()), 1), Some(e));
        let partial = Value::Struct(vec![(1, Value::Enum(5))]);
        assert_eq!(
            Emote::from_property(Some(&partial), 77),
            Some(Emote {
                anim: 5,
                started: 77,
                looping: false
            })
        );
        assert!(check_property_kind(pid::STANCE, &Value::String("x".into())).is_err());
        assert!(check_property_kind(0x9000, &Value::String("x".into())).is_ok());
        assert_eq!(speed_from_property(Some(&Value::Float32(0.0)), 6.0), 6.0);
    }
}
