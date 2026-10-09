// SPDX-License-Identifier: GPL-3.0-or-later
//! Value comparisons of Switch cases, transitions and When tests (`script-runtime.md` §6.3,
//! §6.9, §7) and typed reads of driven field values.

use tessera_formats::property::{PropertyArray, PropertyStruct, Waveform};
use tessera_formats::script_table::{SWITCH_ALL_BITS_SET, SWITCH_ANY_BIT_SET, when_op};
use tessera_formats::{Did, PropertyType, PropertyValue, StringInfo, StringSource, Utf16String};

/// The zero value of a property type (`script-runtime.md` §6.2, R27): 0, 0.0, false, enum 0,
/// DID 0, property ID 0; a tri-state is 255 ("undefined"); every other type its empty
/// value (an empty string, struct, array, bitfield or waveform, a zero vector, colour or
/// position, a StringInfo holding an empty literal).
///
/// `None` for a type code the format does not define ([`PropertyType::Unknown`]): such a
/// type has no value to start from (Tessera choice, `script-runtime.md` §29).
#[must_use]
pub(crate) fn zero_value(ty: PropertyType) -> Option<PropertyValue> {
    Some(match ty {
        PropertyType::String => PropertyValue::String(String::new()),
        PropertyType::StringToken => PropertyValue::StringToken(0),
        PropertyType::Waveform => PropertyValue::Waveform(Box::new(Waveform {
            subtype: 0,
            values: Vec::new(),
            extra: None,
        })),
        PropertyType::Timestamp => PropertyValue::Timestamp(0.0),
        PropertyType::TriState => PropertyValue::TriState(TRI_STATE_UNDEFINED),
        PropertyType::Vector3 => PropertyValue::Vector3([0.0; 3]),
        PropertyType::InstanceId => PropertyValue::InstanceId(0),
        PropertyType::Enum => PropertyValue::Enum(0),
        PropertyType::Float => PropertyValue::Float(0.0),
        PropertyType::PropertyId => PropertyValue::PropertyId(0),
        PropertyType::Struct => PropertyValue::Struct(PropertyStruct::default()),
        PropertyType::Array => PropertyValue::Array(PropertyArray::default()),
        PropertyType::StringInfo => PropertyValue::StringInfo(Box::new(StringInfo {
            source: StringSource::Literal(Utf16String::default()),
            args: None,
        })),
        PropertyType::Bitfield64 => PropertyValue::Bitfield64(0),
        PropertyType::Int => PropertyValue::Int(0),
        PropertyType::Colour => PropertyValue::Colour([0; 4]),
        PropertyType::Position => PropertyValue::Position(Default::default()),
        PropertyType::Bitfield32 => PropertyValue::Bitfield32(0),
        PropertyType::Int64 => PropertyValue::Int64(0),
        PropertyType::DidRef => PropertyValue::DidRef(Did(0)),
        PropertyType::Bool => PropertyValue::Bool(false),
        PropertyType::BitfieldAny => PropertyValue::BitfieldAny(Default::default()),
        PropertyType::Unknown(_) => return None,
    })
}

/// The tri-state value "undefined", a tri-state's zero value (`script-runtime.md` §6.2).
pub(crate) const TRI_STATE_UNDEFINED: u8 = 255;

/// Equality of two property values: numerically when both are scalars (integers, enum keys,
/// booleans, DIDs, bitfields, floats), structurally otherwise.
///
/// The spec traced comparisons only for integers, enums, booleans and DIDs (§6.3); a float
/// compares by value.
pub(crate) fn values_equal(a: &PropertyValue, b: &PropertyValue) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

/// [`values_equal`] over optional values (two absent values are equal).
pub(crate) fn opt_equal(a: Option<&PropertyValue>, b: Option<&PropertyValue>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => values_equal(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// A switch-case comparison of the switch value `v` with a case value `c` under `operator`
/// (enum 0x230000A2: 1 AllBitsSet, 2 AnyBitSet, 3 Equal). An unknown operator compares
/// for equality.
pub(crate) fn case_matches(v: &PropertyValue, c: &PropertyValue, operator: i32) -> bool {
    match operator {
        SWITCH_ALL_BITS_SET => match (v.as_bits(), c.as_bits()) {
            (Some(v), Some(c)) => v & c == c,
            _ => false,
        },
        SWITCH_ANY_BIT_SET => match (v.as_bits(), c.as_bits()) {
            (Some(v), Some(c)) => v & c != 0,
            _ => false,
        },
        _ => values_equal(v, c),
    }
}

/// A number for When comparisons: the scalar's value, or an array's element count
/// (`script-runtime.md` §7).
fn when_number(v: &PropertyValue) -> Option<f64> {
    match v.as_array() {
        Some(a) => Some(f64::from(a.count)),
        None => v.as_f64(),
    }
}

/// A When test: property value `p` against `c` under `operator` (enum 0x230000A3). Values
/// that are not numbers compare only for (in)equality; an unknown operator never holds.
pub(crate) fn when_holds(p: &PropertyValue, c: &PropertyValue, operator: i32) -> bool {
    let (x, y) = match (when_number(p), when_number(c)) {
        (Some(x), Some(y)) => (x, y),
        _ => {
            return match operator {
                when_op::EQUAL => p == c,
                when_op::NOT_EQUAL => p != c,
                _ => false,
            };
        }
    };
    match operator {
        when_op::LESS_THAN_OR_EQUAL => x <= y,
        when_op::GREATER_THAN_OR_EQUAL => x >= y,
        when_op::NOT_EQUAL => x != y,
        when_op::LESS_THAN => x < y,
        when_op::EQUAL => x == y,
        when_op::GREATER_THAN => x > y,
        _ => false,
    }
}

/// A value read as `f32` (numbers only).
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn as_f32(v: &PropertyValue) -> Option<f32> {
    v.as_f64().map(|x| x as f32)
}

/// A value read as a boolean (non-zero numbers are true).
pub(crate) fn as_bool(v: &PropertyValue) -> Option<bool> {
    v.as_f64().map(|x| x != 0.0)
}

/// A value read as a `u32` (its low 32 bits).
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn as_u32(v: &PropertyValue) -> Option<u32> {
    v.as_bits().map(|b| b as u32)
}

/// A value read as an `i32` (its low 32 bits, two's complement).
#[allow(clippy::cast_possible_truncation)]
pub(crate) fn as_i32(v: &PropertyValue) -> Option<i32> {
    match v {
        PropertyValue::Float(f) => f.is_finite().then_some(*f as i32),
        _ => v.as_bits().map(|b| b as u32 as i32),
    }
}

/// A value read as a vector (vectors only).
pub(crate) fn as_vec3(v: &PropertyValue) -> Option<[f32; 3]> {
    match v {
        PropertyValue::Vector3(x) => Some(*x),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_values_follow_the_type() {
        assert_eq!(zero_value(PropertyType::Enum), Some(PropertyValue::Enum(0)));
        assert_eq!(
            zero_value(PropertyType::Bool),
            Some(PropertyValue::Bool(false))
        );
        assert_eq!(
            zero_value(PropertyType::TriState),
            Some(PropertyValue::TriState(255))
        );
        assert_eq!(
            zero_value(PropertyType::DidRef),
            Some(PropertyValue::DidRef(Did(0)))
        );
        assert_eq!(zero_value(PropertyType::Unknown(99)), None);
        for code in 1..=22 {
            let ty = PropertyType::from_code(code);
            let v = zero_value(ty).expect("every defined type has a zero value");
            assert_eq!(v.property_type(), ty);
        }
    }

    #[test]
    fn equality_is_numeric_across_scalar_types() {
        assert!(values_equal(
            &PropertyValue::Enum(3),
            &PropertyValue::Int(3)
        ));
        assert!(values_equal(
            &PropertyValue::Bool(true),
            &PropertyValue::Int(1)
        ));
        assert!(!values_equal(
            &PropertyValue::Enum(3),
            &PropertyValue::Enum(4)
        ));
        assert!(values_equal(
            &PropertyValue::String("a".into()),
            &PropertyValue::String("a".into())
        ));
    }

    #[test]
    fn bit_operators() {
        let v = PropertyValue::Bitfield32(0b1010);
        assert!(case_matches(
            &v,
            &PropertyValue::Bitfield32(0b1000),
            SWITCH_ALL_BITS_SET
        ));
        assert!(!case_matches(
            &v,
            &PropertyValue::Bitfield32(0b1100),
            SWITCH_ALL_BITS_SET
        ));
        assert!(case_matches(
            &v,
            &PropertyValue::Bitfield32(0b0110),
            SWITCH_ANY_BIT_SET
        ));
        assert!(!case_matches(
            &v,
            &PropertyValue::Bitfield32(0b0101),
            SWITCH_ANY_BIT_SET
        ));
    }

    #[test]
    fn when_operators() {
        let p = PropertyValue::Int(5);
        let c = PropertyValue::Int(5);
        assert!(when_holds(&p, &c, when_op::LESS_THAN_OR_EQUAL));
        assert!(when_holds(&p, &c, when_op::GREATER_THAN_OR_EQUAL));
        assert!(!when_holds(&p, &c, when_op::NOT_EQUAL));
        assert!(!when_holds(&p, &c, when_op::LESS_THAN));
        assert!(when_holds(&p, &c, when_op::EQUAL));
        assert!(!when_holds(&p, &c, when_op::GREATER_THAN));
        assert!(when_holds(
            &PropertyValue::Float(4.5),
            &c,
            when_op::LESS_THAN
        ));
        assert!(!when_holds(&p, &c, 99));
    }
}
