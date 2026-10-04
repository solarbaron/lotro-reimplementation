// SPDX-License-Identifier: GPL-3.0-or-later
//! Record decoders for subfile payloads.
//!
//! Every decoder works on a complete, already decompressed payload (`&[u8]`) and is
//! independent of the archive layer. Inputs are treated as hostile: decoders never panic
//! and report failures as [`FormatError`].
//!
//! Memory is bounded as follows:
//!
//! - Record decoders allocate at most a small constant multiple of the input (roughly 10×
//!   for the densest synthetic inputs). Counts read from the data are checked against the
//!   remaining input before use, and initial capacities are clamped to
//!   [`reader::PREALLOC_CAP`], so a hostile header cannot reserve memory up front.
//! - [`Surface::to_rgba8`] on raw and DXT formats allocates at most 8× the stored data.
//! - Embedded JPEG surfaces are decoded only when their frame size fits
//!   [`surface::jpeg_pixel_budget`]: at most [`surface::MAX_JPEG_PIXELS`] pixels (128 MiB of
//!   RGBA8) and at most [`surface::JPEG_PIXELS_PER_INPUT_BYTE`] pixels per input byte beyond a
//!   1 Mi-pixel allowance. The image is decoded straight into its RGBA8 output buffer.
//!
//! [`AnyRecord::decode`] picks the decoder for a DID's type prefix. The [`prelude`] brings
//! the [`Record`] trait and the common types into scope.
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
//! | [`property`] | master property dictionary (0x34000000) | §6 |
//! | [`enum_mapper`] | enum mappers (0x23) | §7 |
//! | [`did_map`] | DID maps (0x28) | §8 |
//! | [`sound`] | sound records (0x0A) and container sniffing | §9 |

mod any;
pub mod bcn;
pub mod did;
pub mod did_map;
pub mod enum_mapper;
mod error;
pub mod property;
pub mod reader;
mod record;
pub mod sound;
pub mod string_info;
pub mod string_table;
pub mod surface;

pub use any::AnyRecord;
pub use bcn::{BcFormat, decode_bcn};
pub use did::{
    Did, DidKind, Evidence, LandblockId, LandblockKind, is_catalogued_prefix, name_hash,
    name_hash_utf16,
};
pub use did_map::{DataIdEntry, DataIdGroup, DataIdMap};
pub use enum_mapper::EnumMapper;
pub use error::{FormatError, Result};
pub use property::{
    PropertyDefinition, PropertyDictionary, PropertyType, PropertyValue, Waveform, WaveformExtra,
};
pub use reader::{Reader, TSize, Utf16String};
pub use record::Record;
pub use sound::{AudioContainer, SoundRecord, WaveInfo};
pub use string_info::{StringArg, StringArgValue, StringArgs, StringInfo, StringSource};
pub use string_table::{StringTable, StringTableEntry};
pub use surface::{RgbaImage, Surface, SurfaceFormat};

/// The [`Record`] trait plus the record types, for `use tessera_formats::prelude::*`.
pub mod prelude {
    pub use crate::{
        AnyRecord, DataIdMap, Did, DidKind, EnumMapper, FormatError, PropertyDictionary, Record,
        SoundRecord, StringInfo, StringTable, Surface, SurfaceFormat, WaveInfo,
    };
}
