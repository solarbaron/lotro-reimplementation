// SPDX-License-Identifier: GPL-3.0-or-later
//! Record decoders for subfile payloads.
//!
//! Every decoder works on a complete, already decompressed payload (`&[u8]`) and is
//! independent of the archive layer. Inputs are treated as hostile: decoders never panic
//! and report failures as [`FormatError`].
//!
//! Memory is bounded as follows:
//!
//! - Every [`Reader`] carries a memory budget of [`reader::MAX_DECODE_AMPLIFICATION`] (16)
//!   × input length + [`reader::DECODE_BUDGET_SLACK`] bytes. Record decoders charge the
//!   decoded data they keep against it before allocating: collection storage (at its
//!   capacity), strings, boxed values and lookup indexes. A record that would exceed the
//!   budget fails with [`FormatError::MemoryLimit`]; real records stay far below it.
//! - Counts read from the data are checked against the remaining input before use,
//!   initial capacities are clamped to [`reader::PREALLOC_CAP`], and collections then grow
//!   in charged steps whose capacity never exceeds the stored count, so a hostile header
//!   cannot reserve memory up front.
//! - Peak heap use while decoding is at most about 1.5× the charged amount: a growing
//!   collection briefly holds its old and new buffers. Allocator overhead is not counted.
//! - Property sets ([`PropertySetRecord`]) nest at most [`property::PROPERTY_MAX_DEPTH`]
//!   levels and cap arrays at [`property::PROPERTY_MAX_ARRAY_LEN`] elements. A tree entry
//!   takes 48 bytes; positions are stored inline, and waveforms and StringInfo values are
//!   boxed and charged at their full size.
//! - [`Surface::to_rgba8`] on raw and DXT formats allocates at most 8× the stored data.
//! - Embedded JPEG surfaces are decoded only when their frame size fits
//!   [`surface::jpeg_pixel_budget`]: at most [`surface::MAX_JPEG_PIXELS`] pixels (128 MiB of
//!   RGBA8) and at most [`surface::JPEG_PIXELS_PER_INPUT_BYTE`] pixels per input byte beyond a
//!   1 Mi-pixel allowance, and when their estimated decode memory
//!   ([`surface::jpeg_decode_bytes`], which includes progressive coefficient buffers) is at
//!   most [`surface::MAX_JPEG_DECODE_BYTES`]. The image is decoded straight into its RGBA8
//!   output buffer.
//!
//! [`AnyRecord::decode`] picks the decoder for a DID's type prefix. Property sets and entity
//! descriptors need the property dictionary as a schema, so they are decoded with
//! [`PropertySetRecord::parse`] and [`EntityDescriptor::parse`] instead. Script tables and
//! scriptlets need it too; [`AnyRecord::decode_with_schema`] routes them. Landblock records
//! (prefix 0x80) depend on their directory stamp, and some on the dictionary, so they are
//! decoded through the [`landblock`] module, not [`AnyRecord`]. The [`decode`] module has one `decode_*(&[u8])` function per decoder, for fuzz
//! targets and tools. The [`prelude`] brings the [`Record`] trait and the common types
//! into scope.
//!
//! Spec: `docs/specs/records.md`.
//!
//! | Module | Records | Spec |
//! |---|---|---|
//! | [`reader`] | primitives: fixed-width, VLE, TSize, strings | §1 |
//! | [`did`] | DID classification, landblock IDs, name hash | §1.5, §2 |
//! | [`string_table`] | string tables (0x25) | §3 |
//! | [`string_info`] | StringInfo values | §4 |
//! | [`surface`], [`bcn`] | render surfaces (0x41), DXT decoding | §5 |
//! | [`property`] | master property dictionary (0x34000000), property value tree | `property-system.md` §2–§4 |
//! | [`property_set`] | property sets (0x78–0x7F), embedded sets, +0x09000000 rule | `property-system.md` §5–§6 |
//! | [`texture_list`] | texture lists / mip chains (0x40), level selection | `texture-list.md` |
//! | [`enum_mapper`] | enum mappers (0x23) | §7 |
//! | [`did_map`] | DID maps (0x28) | §8 |
//! | [`sound`] | sound records (0x0A) and container sniffing | §9 |
//! | [`setup`] | entity descriptors (0x47), visual templates (0x1F), setups (0x04), chain resolution | `setup.md` §2–§7 |
//! | [`appearance`] | appearance records (0x20), block choice, part priorities, slot table, appearance lists | `setup.md` §8, `appearance.md` §3–§4 |
//! | [`avatar`] | race table, avatar bodies, bone-slider definitions | `appearance.md` §2–§3, §6 |
//! | [`item`] | worn and held items: wardrobe structs, dyes, holding locations | `appearance.md` §4–§5, `setup.md` §13 |
//! | [`script_table`] | script tables (0x07), scriptlets (0x0C), clip selection | `script-table.md` |
//! | [`landblock`] | landblock data, info (cells, static entities, links), block maps and their descriptors (0x18), landblock properties, region descriptors (0x0F) | `landblock.md`, `entity-descriptor.md`, `coordinates.md` |

mod any;
pub mod appearance;
pub mod avatar;
pub mod bcn;
pub mod decode;
pub mod did;
pub mod did_map;
pub mod enum_mapper;
mod error;
pub mod item;
pub mod landblock;
pub mod property;
pub mod property_set;
pub mod reader;
mod record;
pub mod script_table;
pub mod setup;
pub mod sound;
pub mod string_info;
pub mod string_table;
pub mod surface;
pub mod texture_list;

pub use any::AnyRecord;
pub use appearance::{
    AppearanceBlock, AppearanceEntry, AppearanceExtra, AppearanceKeyInfo, AppearanceListPids,
    AppearancePart, AppearanceRecord, MaterialGroup, SlotTable,
};
pub use bcn::{BcFormat, decode_bcn};
pub use decode::{
    decode_appearance, decode_block_map, decode_block_map_descriptor, decode_did_map,
    decode_embedded_property_set, decode_entity_descriptor, decode_entity_header,
    decode_enum_mapper, decode_landblock_data, decode_landblock_group, decode_landblock_info,
    decode_landblock_properties, decode_property_dictionary, decode_property_set,
    decode_property_value, decode_region_descriptor, decode_script_table, decode_scriptlet,
    decode_setup, decode_sound, decode_string_info, decode_string_table, decode_surface,
    decode_texture_list, decode_visual_template,
};
pub use did::{
    Did, DidKind, Evidence, LandblockId, LandblockKind, is_catalogued_prefix, name_hash,
    name_hash_utf16,
};
pub use did_map::{DataIdEntry, DataIdGroup, DataIdMap};
pub use enum_mapper::EnumMapper;
pub use error::{FormatError, Result};
pub use landblock::{
    BlockMap, BlockMapDescriptor, LandblockData, LandblockGroupRecord, LandblockInfo,
    LandblockProperties, RegionDescriptor, StaticEntity,
};
pub use property::{
    BitfieldAny, NoSchema, Position, PropertyArray, PropertyDefinition, PropertyDictionary,
    PropertyEntry, PropertyNames, PropertySchema, PropertyStruct, PropertyType, PropertyValue,
    Waveform, WaveformExtra,
};
pub use property_set::{
    EntryContext, EntryVisit, PropertySet, PropertySetRecord, is_property_set_did,
    object_property_set_did,
};
pub use reader::{Reader, TSize, Utf16String};
pub use record::Record;
pub use script_table::{Node, NodeData, NodeKind, Script, ScriptTable, Scriptlet};
// Record types and resolvers only; the setup family's field types (frames, collision
// volumes, `Aabb`, ...) stay under `setup::`.
pub use setup::{
    EntityDescriptor, EntityHeader, ResolveError, Setup, SetupChain, VisualTemplate, resolve_setup,
    resolve_setup_did,
};
pub use sound::{AudioContainer, SoundRecord, WaveInfo};
pub use string_info::{StringArg, StringArgValue, StringArgs, StringInfo, StringSource};
pub use string_table::{
    MessagePart, MessageParts, StringTable, StringTableEntry, StringTableIssue,
};
pub use surface::{RgbaImage, Surface, SurfaceFormat};
pub use texture_list::{ChainLevel, TextureList, TextureListKind, cap_chain_resolution};

/// The [`Record`] trait plus the record types, for `use tessera_formats::prelude::*`.
pub mod prelude {
    pub use crate::{
        AnyRecord, AppearanceRecord, BlockMap, BlockMapDescriptor, DataIdMap, Did, DidKind,
        EntityDescriptor, EntityHeader, EnumMapper, FormatError, LandblockData, LandblockInfo,
        LandblockProperties, PropertyDictionary, PropertySchema, PropertySet, PropertySetRecord,
        PropertyValue, Record, RegionDescriptor, ScriptTable, Scriptlet, Setup, SoundRecord,
        StringInfo, StringTable, Surface, SurfaceFormat, TextureList, VisualTemplate, WaveInfo,
    };
}
