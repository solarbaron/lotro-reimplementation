// SPDX-License-Identifier: GPL-3.0-or-later
//! Slot values, the value map and the variable-name dictionary.
//!
//! Implements the substitution interface of `docs/specs/string-table.md` §8.3: values are
//! keyed by argument ID; a convenience form keyed by name hashes the name with the
//! `records.md` §1.5 name hash first.

use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::name_hash;

use crate::message::Message;
use crate::render::Diagnostic;

/// What fills a slot at render time (spec §7).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Raw text: may carry a trailing tag group (§4.9), escapes and markup.
    Text(String),
    /// An integer, printed as plain decimal digits.
    Integer(i64),
    /// A float, printed in the shortest round-trip form (spec §14 Q6).
    Float(f64),
    /// A nested message with its own value map (e.g. a kind-1 StringInfo argument).
    Message(Box<MessageValue>),
    /// A value that could not be built, for example a nested StringInfo whose lookup
    /// missed. It renders as an absent value and reports the diagnostic in every render
    /// that uses it.
    Unresolved(Diagnostic),
}

/// A nested message value.
#[derive(Debug, Clone, PartialEq)]
pub struct MessageValue {
    /// The message (shared: one parsed entry can fill many slots and values).
    pub message: Arc<Message>,
    /// Its own values.
    pub values: Values,
}

impl Value {
    /// A nested message value.
    #[must_use]
    pub fn message(message: Message, values: Values) -> Self {
        Self::shared_message(Arc::new(message), values)
    }

    /// A nested message value over an already shared message.
    #[must_use]
    pub fn shared_message(message: Arc<Message>, values: Values) -> Self {
        Self::Message(Box::new(MessageValue { message, values }))
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::Text(s.to_owned())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Self::Integer(n)
    }
}

impl From<i32> for Value {
    fn from(n: i32) -> Self {
        Self::Integer(i64::from(n))
    }
}

impl From<u32> for Value {
    fn from(n: u32) -> Self {
        Self::Integer(i64::from(n))
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Self::Float(f)
    }
}

impl From<f32> for Value {
    /// Converts through the f32's shortest decimal form, so `0.1f32` prints as `0.1`.
    fn from(f: f32) -> Self {
        Self::Float(f32_to_f64_shortest(f))
    }
}

/// Widens an f32 to the f64 with the same shortest decimal representation.
#[must_use]
pub fn f32_to_f64_shortest(f: f32) -> f64 {
    if !f.is_finite() {
        return f64::from(f);
    }
    f.to_string().parse::<f64>().unwrap_or(f64::from(f))
}

/// Formats a float (spec §14 Q6, Tessera policy): the shortest form that round-trips,
/// with no exponent for magnitudes from 10⁻⁴ to 10⁹; outside that range an exponent is
/// used. Zero prints as `0`.
#[must_use]
pub fn format_float(f: f64) -> String {
    if !f.is_finite() {
        return f.to_string();
    }
    if f == 0.0 {
        return "0".to_owned();
    }
    let a = f.abs();
    if (1e-4..1e9).contains(&a) {
        format!("{f}")
    } else {
        format!("{f:e}")
    }
}

/// Values keyed by argument ID (spec §8.3). Values for IDs a message does not use are
/// ignored.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Values {
    map: HashMap<u32, Value>,
}

impl Values {
    /// An empty value map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the value for an argument ID.
    pub fn insert(&mut self, id: u32, value: impl Into<Value>) -> &mut Self {
        self.map.insert(id, value.into());
        self
    }

    /// Sets the value for a variable name, hashed as given (ASCII bytes, no case change).
    /// Returns the argument ID.
    pub fn insert_named(&mut self, name: &str, value: impl Into<Value>) -> u32 {
        let id = name_hash(name.as_bytes());
        self.map.insert(id, value.into());
        id
    }

    /// Builder form of [`Values::insert_named`].
    #[must_use]
    pub fn with(mut self, name: &str, value: impl Into<Value>) -> Self {
        self.insert_named(name, value);
        self
    }

    /// The value for an argument ID.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&Value> {
        self.map.get(&id)
    }

    /// Removes a value.
    pub fn remove(&mut self, id: u32) -> Option<Value> {
        self.map.remove(&id)
    }

    /// Number of values.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the map is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Iterates over (ID, value) pairs in unspecified order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &Value)> {
        self.map.iter().map(|(&k, v)| (k, v))
    }
}

impl FromIterator<(u32, Value)> for Values {
    fn from_iter<I: IntoIterator<Item = (u32, Value)>>(iter: I) -> Self {
        Self {
            map: iter.into_iter().collect(),
        }
    }
}

impl Extend<(u32, Value)> for Values {
    fn extend<I: IntoIterator<Item = (u32, Value)>>(&mut self, iter: I) {
        self.map.extend(iter);
    }
}

/// A run-time hash → name dictionary for readable `${NAME}` placeholders (spec §8.2).
///
/// It may be seeded only from the user's own data or from names Tessera itself defines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NameDictionary {
    map: HashMap<u32, String>,
}

impl NameDictionary {
    /// An empty dictionary.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a name; returns its hash. The first name for a hash is kept.
    pub fn insert(&mut self, name: &str) -> u32 {
        let id = name_hash(name.as_bytes());
        self.map.entry(id).or_insert_with(|| name.to_owned());
        id
    }

    /// The name for an argument ID.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&str> {
        self.map.get(&id).map(String::as_str)
    }

    /// Number of names.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the dictionary is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl<'a> FromIterator<&'a str> for NameDictionary {
    fn from_iter<I: IntoIterator<Item = &'a str>>(iter: I) -> Self {
        let mut d = Self::new();
        for name in iter {
            d.insert(name);
        }
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats() {
        assert_eq!(format_float(1.0), "1");
        assert_eq!(format_float(1.5), "1.5");
        assert_eq!(format_float(-0.0), "0");
        assert_eq!(format_float(0.0001), "0.0001");
        assert_eq!(format_float(123_456_789.0), "123456789");
        assert_eq!(format_float(1e9), "1e9");
        assert_eq!(format_float(0.00001), "1e-5");
        assert_eq!(format_float(f64::NAN), "NaN");
        assert_eq!(f32_to_f64_shortest(0.1), 0.1);
        assert_eq!(Value::from(0.1f32), Value::Float(0.1));
    }

    #[test]
    fn named_values() {
        let mut v = Values::new();
        let id = v.insert_named("NAME", "x");
        assert_eq!(id, name_hash(b"NAME"));
        assert!(id < 0x1000_0000);
        assert_eq!(v.get(id), Some(&Value::Text("x".into())));
        let d: NameDictionary = ["NAME", "COUNT"].into_iter().collect();
        assert_eq!(d.get(id), Some("NAME"));
        assert_eq!(d.len(), 2);
    }
}
