// SPDX-License-Identifier: GPL-3.0-or-later
//! Dispatch from a DID's type prefix to the matching decoder.

use crate::appearance::AppearanceRecord;
use crate::did::{Did, DidKind};
use crate::did_map::DataIdMap;
use crate::enum_mapper::EnumMapper;
use crate::error::Result;
use crate::property::PropertyDictionary;
use crate::record::Record;
use crate::setup::{Setup, VisualTemplate};
use crate::sound::SoundRecord;
use crate::string_table::StringTable;
use crate::surface::Surface;
use crate::texture_list::TextureList;

/// Any record this crate can decode, selected by the DID's type prefix (spec §2.1).
///
/// Property sets (0x78–0x7F) and entity descriptors (0x47) are not included: they need the
/// property dictionary as a schema; see [`crate::PropertySetRecord`] and
/// [`crate::EntityDescriptor`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum AnyRecord<'a> {
    /// 0x25: string table.
    StringTable(StringTable),
    /// 0x23: enum mapper.
    EnumMapper(EnumMapper),
    /// 0x28: DID map.
    DataIdMap(DataIdMap),
    /// 0x34: master property dictionary.
    PropertyDictionary(PropertyDictionary),
    /// 0x41: render surface.
    Surface(Surface<'a>),
    /// 0x40: texture list.
    TextureList(TextureList),
    /// 0x0A: sound record.
    Sound(SoundRecord<'a>),
    /// 0x1F: visual template.
    VisualTemplate(VisualTemplate),
    /// 0x04: setup.
    Setup(Setup),
    /// 0x20: appearance record.
    Appearance(AppearanceRecord),
}

impl<'a> AnyRecord<'a> {
    /// Decodes `payload` with the decoder for `did`'s type prefix and checks that the record
    /// prologue matches `did`.
    ///
    /// Returns `Ok(None)` when no decoder exists for the prefix; that is not an error.
    pub fn decode(did: impl Into<Did>, payload: &'a [u8]) -> Result<Option<Self>> {
        let did = did.into();
        Ok(Some(match did.kind() {
            DidKind::StringTable => Self::StringTable(StringTable::parse_expecting(payload, did)?),
            DidKind::EnumMapper => Self::EnumMapper(EnumMapper::parse_expecting(payload, did)?),
            DidKind::DataIdMap => Self::DataIdMap(DataIdMap::parse_expecting(payload, did)?),
            DidKind::PropertyDictionary => {
                Self::PropertyDictionary(PropertyDictionary::parse_expecting(payload, did)?)
            }
            DidKind::RenderSurface => Self::Surface(Surface::parse_expecting(payload, did)?),
            DidKind::TextureList => Self::TextureList(TextureList::parse_expecting(payload, did)?),
            DidKind::Sound => Self::Sound(SoundRecord::parse_expecting(payload, did)?),
            DidKind::VisualTemplate => {
                Self::VisualTemplate(VisualTemplate::parse_expecting(payload, did)?)
            }
            DidKind::Setup => Self::Setup(Setup::parse_expecting(payload, did)?),
            DidKind::Appearance => {
                Self::Appearance(AppearanceRecord::parse_expecting(payload, did)?)
            }
            _ => return Ok(None),
        }))
    }

    /// Whether [`AnyRecord::decode`] has a decoder for this DID's prefix.
    #[must_use]
    pub fn is_decodable(did: impl Into<Did>) -> bool {
        matches!(
            did.into().kind(),
            DidKind::StringTable
                | DidKind::EnumMapper
                | DidKind::DataIdMap
                | DidKind::PropertyDictionary
                | DidKind::RenderSurface
                | DidKind::TextureList
                | DidKind::Sound
                | DidKind::VisualTemplate
                | DidKind::Setup
                | DidKind::Appearance
        )
    }

    /// The record's DID.
    #[must_use]
    pub fn did(&self) -> Did {
        match self {
            Self::StringTable(r) => r.did(),
            Self::EnumMapper(r) => r.did(),
            Self::DataIdMap(r) => r.did(),
            Self::PropertyDictionary(r) => r.did(),
            Self::Surface(r) => r.did(),
            Self::TextureList(r) => r.did(),
            Self::Sound(r) => r.did(),
            Self::VisualTemplate(r) => r.did(),
            Self::Setup(r) => r.did(),
            Self::Appearance(r) => r.did(),
        }
    }

    /// Human-readable record type name.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::StringTable(_) => StringTable::NAME,
            Self::EnumMapper(_) => EnumMapper::NAME,
            Self::DataIdMap(_) => DataIdMap::NAME,
            Self::PropertyDictionary(_) => PropertyDictionary::NAME,
            Self::Surface(_) => Surface::NAME,
            Self::TextureList(_) => TextureList::NAME,
            Self::Sound(_) => SoundRecord::NAME,
            Self::VisualTemplate(_) => VisualTemplate::NAME,
            Self::Setup(_) => Setup::NAME,
            Self::Appearance(_) => AppearanceRecord::NAME,
        }
    }
}
