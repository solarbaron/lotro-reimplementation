// SPDX-License-Identifier: GPL-3.0-or-later
//! Dispatch from a DID's type prefix to the matching decoder.

use crate::did::{Did, DidKind};
use crate::did_map::DataIdMap;
use crate::enum_mapper::EnumMapper;
use crate::error::Result;
use crate::property::PropertyDictionary;
use crate::record::Record;
use crate::sound::SoundRecord;
use crate::string_table::StringTable;
use crate::surface::Surface;

/// Any record this crate can decode, selected by the DID's type prefix (spec §2.1).
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
    /// 0x0A: sound record.
    Sound(SoundRecord<'a>),
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
            DidKind::Sound => Self::Sound(SoundRecord::parse_expecting(payload, did)?),
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
                | DidKind::Sound
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
            Self::Sound(r) => r.did(),
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
            Self::Sound(_) => SoundRecord::NAME,
        }
    }
}
