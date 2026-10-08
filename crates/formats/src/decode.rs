// SPDX-License-Identifier: GPL-3.0-or-later
//! Free-function entry points, one per decoder: `decode_*(payload[, stamp][, schema])`.
//!
//! Each takes a complete payload, plus the directory-entry stamp for the records whose
//! layout depends on it and the property schema for records with property sets, and
//! returns the decoded record or a [`FormatError`], never panicking. They are thin wrappers over the [`Record`] implementations and the
//! schema-driven property decoders, meant for fuzz targets, tools and quick scripts.
//! Payloads must be consumed exactly unless the function says otherwise.
//!
//! [`FormatError`]: crate::FormatError

use crate::appearance::AppearanceRecord;
use crate::collision::{CellGeometry, CollisionMesh};
use crate::did_map::DataIdMap;
use crate::enum_mapper::EnumMapper;
use crate::error::Result;
use crate::landblock::{
    BlockMap, BlockMapDescriptor, LandblockData, LandblockGroupRecord, LandblockInfo,
    LandblockProperties, RegionDescriptor,
};
use crate::property::{PropertyDictionary, PropertySchema, PropertyType, PropertyValue};
use crate::property_set::{PropertySet, PropertySetRecord};
use crate::record::Record;
use crate::script_table::{ScriptTable, Scriptlet};
use crate::setup::{EntityDescriptor, EntityHeader, Setup, VisualTemplate};
use crate::sound::SoundRecord;
use crate::sound_info::SoundInfo;
use crate::string_info::StringInfo;
use crate::string_table::StringTable;
use crate::surface::Surface;
use crate::texture_list::TextureList;

pub use crate::environment::{
    decode_camera_state, decode_day_description, decode_game_time, decode_lod_class_table,
    decode_sky_description,
};
pub use crate::landblock::decode_distant_sector;
pub use crate::scenery::{decode_scene, decode_scene_choice_table, decode_terrain_scene_table};
pub use crate::terrain_type::decode_terrain_type_table;
pub use crate::water::{decode_fog_zone_table, decode_water_table};

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

/// Decodes a sound-info record (prefix 0x2A; `sound.md` §4, §12.1). Unknown flag bits are
/// kept ([`SoundInfo::unknown_flags`]).
pub fn decode_sound_info(data: &[u8]) -> Result<SoundInfo> {
    SoundInfo::parse(data)
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

/// Decodes an entity descriptor (prefix 0x47; `setup.md` §4) with `schema` supplying the
/// embedded property set's types.
pub fn decode_entity_descriptor<S: PropertySchema + ?Sized>(
    data: &[u8],
    schema: &S,
) -> Result<EntityDescriptor> {
    EntityDescriptor::parse(data, schema)
}

/// Decodes the schema-independent header of an entity descriptor (prefix 0x47;
/// `setup.md` §4.1 fields 1–5). The rest of the payload is not examined.
pub fn decode_entity_header(data: &[u8]) -> Result<EntityHeader> {
    EntityHeader::parse(data)
}

/// Decodes a visual template (prefix 0x1F; `setup.md` §5).
pub fn decode_visual_template(data: &[u8]) -> Result<VisualTemplate> {
    VisualTemplate::parse(data)
}

/// Decodes a setup (prefix 0x04; `setup.md` §6).
pub fn decode_setup(data: &[u8]) -> Result<Setup> {
    Setup::parse(data)
}

/// Decodes an appearance record (prefix 0x20; `setup.md` §8).
pub fn decode_appearance(data: &[u8]) -> Result<AppearanceRecord> {
    AppearanceRecord::parse(data)
}

/// Decodes a script table (prefix 0x07; `script-table.md` §2) with `schema` typing the
/// PVs and property sets inside its nodes. The record stores no DID, so the result's
/// [`ScriptTable::did`] is null.
pub fn decode_script_table<S: PropertySchema + ?Sized>(
    data: &[u8],
    schema: &S,
) -> Result<ScriptTable> {
    ScriptTable::parse(data, schema)
}

/// Decodes a scriptlet (prefix 0x0C; `script-table.md` §3), like [`decode_script_table`].
pub fn decode_scriptlet<S: PropertySchema + ?Sized>(data: &[u8], schema: &S) -> Result<Scriptlet> {
    Scriptlet::parse(data, schema)
}

/// Decodes a landblock-data record (`0x800RXXYY`; `landblock.md` §3). `stamp` is the
/// record's directory stamp; `None` uses the §3.9 fallback.
pub fn decode_landblock_data(data: &[u8], stamp: Option<u16>) -> Result<LandblockData> {
    LandblockData::parse(data, stamp)
}

/// Decodes a landblock-info record (`0x802RXXYY`; `landblock.md` §4) with `schema`
/// supplying the embedded property sets' types. `stamp` as for [`decode_landblock_data`].
pub fn decode_landblock_info<S: PropertySchema + ?Sized>(
    data: &[u8],
    stamp: Option<u16>,
    schema: &S,
) -> Result<LandblockInfo> {
    LandblockInfo::parse(data, stamp, schema)
}

/// Decodes a block map (`0x801RXXYY`; `landblock.md` §5.3).
pub fn decode_block_map(data: &[u8]) -> Result<BlockMap> {
    BlockMap::parse(data)
}

/// Decodes a block-map descriptor (prefix 0x18; `landblock.md` §5.2) with `schema`
/// supplying the palette values' types.
pub fn decode_block_map_descriptor<S: PropertySchema + ?Sized>(
    data: &[u8],
    schema: &S,
) -> Result<BlockMapDescriptor> {
    BlockMapDescriptor::parse(data, schema)
}

/// Decodes a landblock-properties record (`0x804RXXYY`; `landblock.md` §6).
pub fn decode_landblock_properties<S: PropertySchema + ?Sized>(
    data: &[u8],
    schema: &S,
) -> Result<LandblockProperties> {
    LandblockProperties::parse(data, schema)
}

/// Decodes the framing of a kind-5 landblock record (`0x805RXXYY`; `landblock.md` §7).
pub fn decode_landblock_group(data: &[u8]) -> Result<LandblockGroupRecord> {
    LandblockGroupRecord::parse(data)
}

/// Decodes a region descriptor (prefix 0x0F; `environment.md` §3.1).
pub fn decode_region_descriptor(data: &[u8]) -> Result<RegionDescriptor> {
    RegionDescriptor::parse(data)
}

/// Decodes a stand-alone collision mesh (prefix 0x66; `collision.md` §4). `stamp` is the
/// record's directory stamp, which decides the tree's `build_byte` (§3.1); `None` uses the
/// fallback described at [`CollisionMesh::parse`].
pub fn decode_collision_mesh(data: &[u8], stamp: Option<u16>) -> Result<CollisionMesh> {
    CollisionMesh::parse(data, stamp)
}

/// Decodes cell geometry (prefix 0x01; `collision.md` §5) with `schema` supplying the
/// embedded property set's types. `stamp` as for [`decode_collision_mesh`].
pub fn decode_cell_geometry<S: PropertySchema + ?Sized>(
    data: &[u8],
    stamp: Option<u16>,
    schema: &S,
) -> Result<CellGeometry> {
    CellGeometry::parse(data, stamp, schema)
}
