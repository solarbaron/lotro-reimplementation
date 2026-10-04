// SPDX-License-Identifier: GPL-3.0-or-later
//! Free-function entry points, one per decoder: `decode_*(&[u8])`.
//!
//! Each takes a complete payload and returns the decoded record or a [`FormatError`],
//! never panicking. They are thin wrappers over the [`Record`] implementations and the
//! schema-driven property decoders, meant for fuzz targets, tools and quick scripts.
//! Payloads must be consumed exactly unless the function says otherwise.
//!
//! [`FormatError`]: crate::FormatError

use crate::did_map::DataIdMap;
use crate::enum_mapper::EnumMapper;
use crate::error::Result;
use crate::property::{PropertyDictionary, PropertySchema, PropertyType, PropertyValue};
use crate::property_set::{PropertySet, PropertySetRecord};
use crate::record::Record;
use crate::sound::SoundRecord;
use crate::string_info::StringInfo;
use crate::string_table::StringTable;
use crate::surface::Surface;
use crate::texture_list::TextureList;

/// Decodes a string-table record (prefix 0x25; `records.md` §3).
pub fn decode_string_table(data: &[u8]) -> Result<StringTable> {
    StringTable::parse(data)
}

/// Decodes a buffer holding exactly one StringInfo (`records.md` §4).
pub fn decode_string_info(data: &[u8]) -> Result<StringInfo> {
    StringInfo::parse(data)
}

/// Decodes a render-surface record header and borrows its pixel data (prefix 0x41;
/// `records.md` §5). Call [`Surface::to_rgba8`] to decode the pixels.
pub fn decode_surface(data: &[u8]) -> Result<Surface<'_>> {
    Surface::parse(data)
}

/// Decodes a texture-list record (prefix 0x40; `texture-list.md` §2).
pub fn decode_texture_list(data: &[u8]) -> Result<TextureList> {
    TextureList::parse(data)
}

/// Decodes an enum-mapper record (prefix 0x23; `records.md` §7).
pub fn decode_enum_mapper(data: &[u8]) -> Result<EnumMapper> {
    EnumMapper::parse(data)
}

/// Decodes a DID-map record (prefix 0x28; `records.md` §8).
pub fn decode_did_map(data: &[u8]) -> Result<DataIdMap> {
    DataIdMap::parse(data)
}

/// Decodes the master property dictionary (DID 0x34000000; `property-system.md` §2).
pub fn decode_property_dictionary(data: &[u8]) -> Result<PropertyDictionary> {
    PropertyDictionary::parse(data)
}

/// Decodes a sound record envelope (prefix 0x0A; `records.md` §9).
pub fn decode_sound(data: &[u8]) -> Result<SoundRecord<'_>> {
    SoundRecord::parse(data)
}

/// Decodes a property-set record (prefixes 0x78–0x7F; `property-system.md` §5.1) with
/// `schema` supplying each PID's type.
pub fn decode_property_set<S: PropertySchema + ?Sized>(
    data: &[u8],
    schema: &S,
) -> Result<PropertySetRecord> {
    PropertySetRecord::parse(data, schema)
}

/// Decodes an embedded property set: a TSize count and entries, no leading DID
/// (`property-system.md` §5.1).
pub fn decode_embedded_property_set<S: PropertySchema + ?Sized>(
    data: &[u8],
    schema: &S,
) -> Result<PropertySet> {
    PropertySet::parse(data, schema)
}

/// Decodes a buffer holding exactly one property value of type `ty`
/// (`property-system.md` §4).
pub fn decode_property_value<S: PropertySchema + ?Sized>(
    data: &[u8],
    ty: PropertyType,
    schema: &S,
) -> Result<PropertyValue> {
    PropertyValue::parse(data, ty, schema)
}
