// SPDX-License-Identifier: GPL-3.0-or-later
//! Scripted objects: the script class library ("WLib", DID 0x56000000) and object-state
//! records ("WState", DIDs 0x70000000–0x77FFFFFF), their object graphs, and the
//! area / territory / region / dungeon resolution the world walker needs.
//!
//! Specs: `docs/specs/wlib.md`, `docs/specs/wstate.md`.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`ClassLibrary`] | Class table, attribute lists (serialization order), parent map, singletons; invariants L1–L9 | `wlib.md` §3–§11 |
//! | [`ObjectState`], [`ObjectHeader`] | Record header, imports, bundle (labels, class table, items), links, available properties; partial decode on an unknown native class | `wstate.md` §3–§7 |
//! | [`Native`] | The built-in codecs of native classes (maps, arrays, sets, positions, property sets, strings, ...) | §5.3, §5.4 |
//! | [`ItemView`], [`ScriptView`], [`Resolved`] | Typed attribute access by name hash, reference resolution, reachability | §8 |
//! | [`ObjectStore`], [`GameObject`] | Loading through a record source: library lookup, LRU cache bounded by count and bytes, DID items, import lists, property sets | §2, §8.2, §12 |
//! | [`Geo`], [`GeoIndex`] | Area → territory → region names, dungeons and their map positions, the region-number table, lookup by name | §11 |
//! | [`testkit`] | Synthetic record builders (tests, fuzzing) | §14 / §15 |
//!
//! # Hostile input
//!
//! Every decoder works on a complete payload, never panics, and bounds its memory: counts
//! are checked against the remaining bytes before anything is allocated, and retained
//! heap is charged against the reader budget of `tessera-formats` (a fixed multiple of the
//! input length). Bundles are parsed from their own `bundle_size` slice. Graph walks keep
//! visited sets; nothing expands references recursively. Parent chains are bounded by the
//! class count and cycles are rejected.
//!
//! # Example
//!
//! ```no_run
//! use tessera_wstate::{Geo, ObjectStore};
//!
//! # fn main() -> Result<(), tessera_wstate::WStateError> {
//! let store = ObjectStore::open("/path/to/install")?;
//! let geo = Geo::new(&store)?;
//! let chain = geo.area_chain(0x7000_0001.into())?;
//! println!("{:?} in {:?}", chain.area.name, chain.region.map(|r| r.region_id));
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

mod error;
pub mod geo;
mod graph;
pub mod library;
pub mod native;
pub mod state;
mod store;
pub mod testkit;
mod util;

pub use error::{Result, WStateError};
pub use geo::{
    AreaChain, AreaInfo, DungeonInfo, Geo, GeoBuildReport, GeoEntry, GeoIndex, GeoIndexBuilder,
    GeoKind, GeoSchema, RegionInfo, RegionName, TerritoryInfo, normalize_name, render_name,
};
pub use graph::{AttributeValue, ItemView, Resolved, ScriptView};
pub use library::{
    Attribute, ClassDef, ClassLibrary, LIBRARY_DID, LibraryWarning, Slot, ValueType,
};
pub use native::{Native, Ref, codec_name, has_codec};
pub use state::{
    AvailableProperties, Body, Bundle, DecodeContext, Import, Item, Link, Object, ObjectHeader,
    ObjectState, Partial, RecordClass, ScriptObject, StateWarning, Value, is_object_did,
};
pub use store::{
    CacheLimits, DEFAULT_CACHE_BYTES, GameObject, MAX_CACHED_PAYLOAD, MAX_CACHED_STATES,
    ObjectStore, WEENIE_MAP_LABEL, WLIB_CLIENT_LABEL, WLIB_MAP_LABEL, object_dids,
};

/// Types of other crates that appear in this crate's API.
pub use tessera_formats::{Did, Position, PropertySchema, StringInfo};

#[cfg(test)]
mod tests;
