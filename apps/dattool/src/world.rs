// SPDX-License-Identifier: GPL-3.0-or-later
//! The record layer shared by `show` and `census`: one dispatcher over every record this
//! tool can decode, with the directory stamp and the property dictionary passed through.
//!
//! [`tessera_formats::AnyRecord`] covers the records that need neither or only the
//! dictionary; the landblock family (0x80), region descriptors (0x0F) and block-map
//! descriptors (0x18) are decoded through `tessera_formats::landblock` (`landblock.md`,
//! `environment.md` §3); surface records (0x31), materials (0x30) and shader records (0x2B)
//! through `tessera_material` (`material.md` §4, §6, §7), for counting only.

use anyhow::{Context, Result};
use tessera_formats::landblock::{
    BlockMap, BlockMapDescriptor, LandblockData, LandblockInfo, LandblockProperties,
    RegionDescriptor, SectorRecord,
};
use tessera_formats::{
    AnyRecord, Did, DidKind, LandblockKind, NoSchema, PropertyDictionary, PropertySchema, Record,
};

use tessera_material::{MaterialRecord, ShaderRecord, SurfaceRecord};

use crate::input::{Ctx, Input, open_archive};

/// DID of the master property dictionary (`property-system.md` §2).
pub const DICTIONARY_DID: u32 = 0x3400_0000;

/// Any record this tool decodes.
#[derive(Debug)]
pub enum Decoded<'a> {
    /// A record of [`AnyRecord`].
    Any(Box<AnyRecord<'a>>),
    /// 0x0F: region descriptor.
    Region(Box<RegionDescriptor>),
    /// 0x18: block-map property descriptor.
    BlockMapDescriptor(BlockMapDescriptor),
    /// 0x800RXXYY: landblock data.
    LandblockData(Box<LandblockData>),
    /// 0x801RXXYY: block map.
    BlockMap(BlockMap),
    /// 0x802RXXYY: landblock info.
    LandblockInfo(Box<LandblockInfo>),
    /// 0x804RXXYY: landblock properties.
    LandblockProperties(LandblockProperties),
    /// 0x805RXXYY: distant-landscape sector (`distant-landscape.md` §3).
    LandblockGroup(Box<SectorRecord>),
    /// 0x31, 0x30, 0x2B: a record of the material family, decoded by `tessera-material`;
    /// only its type name is kept.
    Material {
        /// The record's DID.
        did: Did,
        /// Record type name.
        name: &'static str,
    },
}

/// Whether [`decode`] has a decoder for `did`.
pub fn is_decodable(did: u32) -> bool {
    AnyRecord::is_decodable_with_schema(did)
        || matches!(
            Did(did).kind(),
            DidKind::RegionDescriptor
                | DidKind::BlockMapProperties
                | DidKind::SurfaceDescriptor
                | DidKind::Material
                | DidKind::ShaderProgram
                | DidKind::Landblock(
                    LandblockKind::Heightmap
                        | LandblockKind::BlockMap
                        | LandblockKind::Info
                        | LandblockKind::Properties
                        | LandblockKind::Other(5)
                )
        )
}

/// Whether decoding `did` needs the property dictionary (records with embedded property
/// sets or property values).
pub fn needs_schema(did: u32) -> bool {
    matches!(
        Did(did).kind(),
        DidKind::CollisionGeometry
            | DidKind::ScriptTable
            | DidKind::Scriptlet
            | DidKind::BlockMapProperties
            | DidKind::Landblock(
                LandblockKind::Info | LandblockKind::Properties | LandblockKind::Other(5)
            )
    )
}

/// Decodes `payload` as the record `did`, checking its stored DID where it has one.
/// `stamp` is the directory-entry stamp (`dat-container.md` §5.2). `Ok(None)` when no
/// decoder exists for the DID.
pub fn decode<'a>(
    did: u32,
    payload: &'a [u8],
    stamp: Option<u16>,
    schema: &dyn PropertySchema,
) -> tessera_formats::Result<Option<Decoded<'a>>> {
    Ok(Some(match Did(did).kind() {
        DidKind::RegionDescriptor => {
            Decoded::Region(Box::new(RegionDescriptor::parse_expecting(payload, did)?))
        }
        DidKind::BlockMapProperties => {
            Decoded::BlockMapDescriptor(BlockMapDescriptor::parse_expecting(payload, did, schema)?)
        }
        DidKind::Landblock(LandblockKind::Heightmap) => Decoded::LandblockData(Box::new(
            LandblockData::parse_expecting(payload, did, stamp)?,
        )),
        DidKind::Landblock(LandblockKind::BlockMap) => {
            Decoded::BlockMap(BlockMap::parse_expecting(payload, did)?)
        }
        DidKind::Landblock(LandblockKind::Info) => Decoded::LandblockInfo(Box::new(
            LandblockInfo::parse_expecting(payload, did, stamp, schema)?,
        )),
        DidKind::Landblock(LandblockKind::Properties) => Decoded::LandblockProperties(
            LandblockProperties::parse_expecting(payload, did, schema)?,
        ),
        DidKind::Landblock(LandblockKind::Other(5)) => Decoded::LandblockGroup(Box::new(
            SectorRecord::parse_expecting(payload, did, schema)?,
        )),
        DidKind::SurfaceDescriptor => {
            SurfaceRecord::parse_expecting(payload, did)?;
            Decoded::Material {
                did: Did(did),
                name: SurfaceRecord::NAME,
            }
        }
        DidKind::Material => {
            MaterialRecord::parse_expecting(payload, did)?;
            Decoded::Material {
                did: Did(did),
                name: MaterialRecord::NAME,
            }
        }
        DidKind::ShaderProgram => {
            ShaderRecord::parse_expecting(payload, did)?;
            Decoded::Material {
                did: Did(did),
                name: ShaderRecord::NAME,
            }
        }
        _ => match AnyRecord::decode_with_stamp(did, payload, stamp, schema)? {
            Some(r) => Decoded::Any(Box::new(r)),
            None => return Ok(None),
        },
    }))
}

/// The property dictionary used as the schema, and where it came from.
#[derive(Debug, Default)]
pub struct Schema {
    /// The dictionary, if one was found.
    pub dict: Option<PropertyDictionary>,
    /// Where it was read from (an archive label), or why there is none.
    pub source: String,
}

impl Schema {
    /// The schema to decode with: the dictionary, or [`NoSchema`] (every PID unknown).
    pub fn get(&self) -> &dyn PropertySchema {
        match &self.dict {
            Some(d) => d,
            None => &NoSchema,
        }
    }

    /// No dictionary (for records that do not need one).
    pub fn none(why: &str) -> Self {
        Self {
            dict: None,
            source: why.to_owned(),
        }
    }

    /// Loads the dictionary (DID 0x34000000): from the input itself, else from the
    /// `gamelogic` archive next to a single-archive input, else from the game directory
    /// (`--game-dir`, `$TESSERA_GAME_DIR`, auto-detection). Without one, records whose
    /// property sets are not empty fail to decode; that is reported, not an error here.
    pub fn load(input: &Input, ctx: &Ctx) -> Result<Self> {
        if let Some(f) = input.locate(DICTIONARY_DID)? {
            let payload = f.read()?;
            let dict = PropertyDictionary::parse_expecting(&payload, DICTIONARY_DID)
                .with_context(|| format!("property dictionary in {}", f.origin))?;
            return Ok(Self {
                dict: Some(dict),
                source: f.origin,
            });
        }
        let mut candidates = Vec::new();
        if let Input::Archive { .. } = input
            && let Some(dir) = input.home_dir()
        {
            candidates.push(Ctx {
                game_dir: Some(dir),
            });
        }
        candidates.push(ctx.clone());
        for c in &candidates {
            let Ok((label, archive)) = open_archive("gamelogic", c) else {
                continue;
            };
            let Some(entry) = archive.lookup(DICTIONARY_DID).ok().flatten() else {
                continue;
            };
            let payload = archive
                .read_entry(&entry)
                .with_context(|| format!("cannot read the property dictionary from {label}"))?;
            let dict = PropertyDictionary::parse_expecting(&payload, DICTIONARY_DID)
                .with_context(|| format!("property dictionary in {label}"))?;
            return Ok(Self {
                dict: Some(dict),
                source: label,
            });
        }
        tracing::warn!(
            "no property dictionary (0x34000000) found; records with property sets may fail"
        );
        Ok(Self::none("none found"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodable_kinds() {
        for did in [
            0x0100_0001u32,
            0x0200_0001,
            0x0800_0001,
            0x0F00_0001,
            0x1100_0001,
            0x1200_0001,
            0x1500_0007,
            0x1600_0001,
            0x1700_0001,
            0x1800_0001,
            0x1900_0001,
            0x1A00_0001,
            0x3200_0000,
            0x3500_0000,
            0x6600_0001,
            0x2B00_0001,
            0x3000_0001,
            0x3100_0001,
            0x8001_0203,
            0x8011_0203,
            0x8021_0203,
            0x8041_0203,
            0x8051_0203,
        ] {
            assert!(is_decodable(did), "{did:#x}");
        }
        for did in [0x1400_0001u32, 0x8031_0203, 0x0600_0001] {
            assert!(!is_decodable(did), "{did:#x}");
        }
        assert!(needs_schema(0x0100_0001));
        assert!(needs_schema(0x8041_0203));
        assert!(needs_schema(0x8051_0203));
        assert!(!needs_schema(0x6600_0001));
        assert!(!needs_schema(0x8001_0203));
    }

    #[test]
    fn unknown_prefix_is_none() {
        assert!(matches!(
            decode(0x1400_0001, b"xxxx", None, &NoSchema),
            Ok(None)
        ));
        assert!(decode(0x1900_0001, b"\x01\x00\x00\x19", None, &NoSchema).is_err());
    }
}
