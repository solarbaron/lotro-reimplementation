// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed reads of property values (`ui-widgets.md` §2.2).
//!
//! A class reads a property as the element's **effective** value (run-time, then current
//! state, then own; `ui-layout.md` §9.4 step 5), otherwise the **dictionary default**,
//! otherwise zero, false or "none". [`Prop`] wraps the result of that lookup; the
//! accessors here convert values of the wrong type to the type's zero, so hostile data
//! never panics.

use tessera_formats::property::PropertyValue;

/// A property looked up through the effective value and the dictionary default.
#[derive(Debug, Clone, Copy, Default)]
pub struct Prop<'a> {
    /// The effective value, when the element sets one ("set on the element").
    pub set: Option<&'a PropertyValue>,
    /// The dictionary default, when the definition has one.
    pub default: Option<&'a PropertyValue>,
}

impl<'a> Prop<'a> {
    /// The value read (effective, then default).
    #[must_use]
    pub fn value(&self) -> Option<&'a PropertyValue> {
        self.set.or(self.default)
    }

    /// Whether the element itself sets the property (a dictionary default does not count).
    #[must_use]
    pub fn is_set(&self) -> bool {
        self.set.is_some()
    }

    /// As a boolean (false when absent or not a Bool; a non-zero Int or Enum counts true).
    #[must_use]
    pub fn bool(&self) -> bool {
        self.value().is_some_and(as_bool)
    }

    /// As a float (0 when absent; Int and Enum convert).
    #[must_use]
    pub fn float(&self) -> f32 {
        self.value().map_or(0.0, as_float)
    }

    /// As a signed integer (0 when absent; floats truncate).
    #[must_use]
    pub fn int(&self) -> i32 {
        self.value().map_or(0, as_int)
    }

    /// As an enum key, element ID, DID or state value (0 when absent).
    #[must_use]
    pub fn key(&self) -> u32 {
        self.value().map_or(0, as_key)
    }
}

/// A value as a boolean.
#[must_use]
pub fn as_bool(v: &PropertyValue) -> bool {
    match v {
        PropertyValue::Bool(b) => *b,
        PropertyValue::Int(i) => *i != 0,
        PropertyValue::Enum(e) => *e != 0,
        PropertyValue::TriState(t) => *t != 0,
        _ => false,
    }
}

/// A value as a 32-bit float.
#[must_use]
pub fn as_float(v: &PropertyValue) -> f32 {
    match v {
        PropertyValue::Float(f) => *f,
        PropertyValue::Int(i) => *i as f32,
        PropertyValue::Int64(i) => *i as f32,
        PropertyValue::Enum(e) => *e as f32,
        PropertyValue::Bool(b) => f32::from(u8::from(*b)),
        _ => 0.0,
    }
}

/// A value as a signed 32-bit integer.
#[must_use]
pub fn as_int(v: &PropertyValue) -> i32 {
    match v {
        PropertyValue::Int(i) => *i,
        PropertyValue::Int64(i) => (*i).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        PropertyValue::Float(f) => crate::geom::trunc_f32(*f),
        PropertyValue::Enum(e) => *e as i32,
        PropertyValue::Bool(b) => i32::from(*b),
        _ => 0,
    }
}

/// A value as a key (enum key, element ID, state, DID).
#[must_use]
pub fn as_key(v: &PropertyValue) -> u32 {
    match v {
        PropertyValue::Enum(e) => *e,
        PropertyValue::DidRef(d) => d.0,
        PropertyValue::Int(i) => *i as u32,
        PropertyValue::PropertyId(p) => *p,
        PropertyValue::Bitfield32(b) => *b,
        _ => 0,
    }
}

/// A colour value as `0xAARRGGBB`: property colours are stored as bytes B, G, R, A
/// (`property-system.md` §4.7, `ui-layout.md` §15).
#[must_use]
pub fn as_argb(v: &PropertyValue) -> Option<u32> {
    match v {
        PropertyValue::Colour(c) => Some(u32::from_le_bytes(*c)),
        _ => None,
    }
}

/// The elements of an array value (empty for any other type).
#[must_use]
pub fn array_elements(v: &PropertyValue) -> &[tessera_formats::property::PropertyEntry] {
    match v {
        PropertyValue::Array(a) => &a.elements,
        _ => &[],
    }
}

/// A field of a struct value.
#[must_use]
pub fn struct_field(v: &PropertyValue, pid: u32) -> Option<&PropertyValue> {
    match v {
        PropertyValue::Struct(s) => s.get(pid),
        _ => None,
    }
}
