// SPDX-License-Identifier: GPL-3.0-or-later
//! The property system: most gameplay data is stored as (property ID, value) pairs whose
//! types come from one global dictionary.
//!
//! This crate sits on top of `tessera-formats` (raw record decoding) and `tessera-dat`
//! (archives) and gives higher layers a typed, name-aware view:
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`PropertyRegistry`] | PID → name, type, definition, label source; the decoding schema | `property-system.md` §2, §3 |
//! | [`Value`], [`Property`] | Typed values with `as_*` accessors | §4 |
//! | [`PropertySet`] | Properties by PID or by name; struct values are sets too | §5 |
//! | [`GameData`] | Lazily loads the registry, enum mappers and DID maps from a [`RecordSource`]; resolves a DID to its properties (+0x09000000 rule) | §6, §8 |
//! | [`Label`], [`EnumLabel`], [`ResolvedProperty`] | Enum labels, bit labels, PID names, DID-reference types | §7, §9 |
//!
//! Inputs are treated as hostile. Decoding itself is done by `tessera-formats`, whose
//! decoders bound allocation and nesting; this crate adds bounded caches
//! ([`game_data::MAX_CACHED_RECORDS`]), bounded label paths
//! ([`game_data::MAX_PATH_LABELS`]), bounded enum base chains and a bounded resolve depth
//! ([`game_data::RESOLVE_MAX_DEPTH`]), and never panics.
//!
//! StringInfo display texts (enum display names, type-13 values) are returned unresolved;
//! turning them into text against the string tables belongs to `tessera-text`.
//!
//! ```no_run
//! use tessera_props::GameData;
//!
//! # fn main() -> Result<(), tessera_props::PropsError> {
//! let data = GameData::open("/path/to/install")?;
//! let registry = data.registry()?;
//! let object = 0x7000_0001.into();
//! if let Some(set) = data.properties(object)? {
//!     if let Some(v) = set.get_by_name(registry, "SomePropertyName") {
//!         println!("{:?} -> {}", v, data.label(registry.pid("SomePropertyName").unwrap_or(0), v));
//!     }
//! }
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

mod error;
pub mod game_data;
mod labels;
mod registry;
mod set;
mod source;
mod value;

#[cfg(test)]
mod testkit;
#[cfg(test)]
mod tests;

pub use error::{PropsError, Result, SourceError};
pub use game_data::{GameData, ROOT_DID_MAP};
pub use labels::{BitLabel, EnumLabel, Label, ResolvedProperty};
pub use registry::{EnumSource, PROPERTY_DICTIONARY_DID, PropertyRegistry};
pub use set::PropertySet;
pub use source::{MemorySource, RecordSource};
pub use value::{Property, SetBits, Value};

/// Re-exported value and record types from `tessera-formats` that appear in this crate's
/// API.
pub use tessera_formats::{
    BitfieldAny, DataIdEntry, DataIdMap, Did, EnumMapper, Position, PropertyDefinition,
    PropertyType, StringInfo, StringSource, Waveform, WaveformExtra,
};
