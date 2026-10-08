// SPDX-License-Identifier: GPL-3.0-or-later
//! Dispatch from a DID's type prefix to the matching decoder.

use crate::appearance::AppearanceRecord;
use crate::collision::{CellGeometry, CollisionMesh};
use crate::did::{Did, DidKind};
use crate::did_map::DataIdMap;
use crate::enum_mapper::EnumMapper;
use crate::environment::{CameraState, DayDescription, GameTime, LodClassTable, SkyDescription};
use crate::error::Result;
use crate::landblock::RegionDescriptor;
use crate::property::{PropertyDictionary, PropertySchema};
use crate::record::Record;
use crate::scenery::{Scene, SceneChoiceTable, TerrainSceneTable};
use crate::script_table::{ScriptTable, Scriptlet};
use crate::setup::{Setup, VisualTemplate};
use crate::sound::SoundRecord;
use crate::sound_info::SoundInfo;
use crate::string_table::StringTable;
use crate::surface::Surface;
use crate::terrain_type::TerrainTypeTable;
use crate::texture_list::TextureList;
use crate::water::{FogZoneTable, WaterTable};

/// Any record this crate can decode, selected by the DID's type prefix (spec §2.1).
///
/// Property sets (0x78–0x7F) and entity descriptors (0x47) are not included: they need the
/// property dictionary as a schema; see [`crate::PropertySetRecord`] and
/// [`crate::EntityDescriptor`]. Script tables (0x07), scriptlets (0x0C) and cell geometry
/// (0x01) need it too and are produced only by [`AnyRecord::decode_with_schema`] and
/// [`AnyRecord::decode_with_stamp`]. The landblock family (0x80) and block-map descriptors
/// (0x18) are decoded through [`crate::landblock`]; region descriptors (0x0F) need neither
/// a stamp nor a schema and are in both places.
///
/// Two record kinds depend on the directory-entry `stamp` (`collision.md` §3.1): collision
/// meshes (0x66) and cell geometry (0x01). [`AnyRecord::decode`] decodes 0x66 without a
/// stamp, and [`AnyRecord::decode_with_schema`] decodes both without one, by the spec's
/// fallback (the decoded record's `layout_rule` says so); [`AnyRecord::decode_with_stamp`]
/// passes the stamp through.
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
    /// 0x2A: sound info.
    SoundInfo(SoundInfo),
    /// 0x1F: visual template.
    VisualTemplate(VisualTemplate),
    /// 0x04: setup.
    Setup(Setup),
    /// 0x20: appearance record.
    Appearance(AppearanceRecord),
    /// 0x07: script table (only from [`AnyRecord::decode_with_schema`]).
    ScriptTable(ScriptTable),
    /// 0x0C: scriptlet (only from [`AnyRecord::decode_with_schema`]).
    Scriptlet(Scriptlet),
    /// 0x19: terrain type table (`terrain-texturing.md` §3).
    TerrainTypeTable(TerrainTypeTable),
    /// 0x12: terrain-scene table (`scenery.md` §4.2).
    TerrainSceneTable(TerrainSceneTable),
    /// 0x11: scene-choice table (`scenery.md` §4.3).
    SceneChoiceTable(SceneChoiceTable),
    /// 0x02: scene record (`scenery.md` §5).
    Scene(Scene),
    /// 0x16: water table (`water.md` §4).
    WaterTable(WaterTable),
    /// 0x17: fog-zone table (`water.md` §5).
    FogZoneTable(FogZoneTable),
    /// 0x15: sky description (`environment.md` §5.1).
    SkyDescription(SkyDescription),
    /// 0x1A: day description (`environment.md` §5.2).
    DayDescription(DayDescription),
    /// 0x35: game time (`environment.md` §6).
    GameTime(GameTime),
    /// 0x32: LOD-class table (`environment.md` §9.1).
    LodClassTable(LodClassTable),
    /// 0x08: camera state (`walker.md` §11.5). The record stores no DID of its own, so the
    /// DID it was decoded for is kept beside it.
    CameraState {
        /// The DID the record was looked up by.
        did: Did,
        /// The decoded record.
        state: Box<CameraState>,
    },
    /// 0x66: stand-alone collision mesh (`collision.md` §4). The record stores no DID of its
    /// own, so the DID it was decoded for is kept beside it.
    CollisionMesh {
        /// The DID the record was looked up by.
        did: Did,
        /// The decoded record.
        mesh: CollisionMesh,
    },
    /// 0x01: cell geometry (`collision.md` §5; only from [`AnyRecord::decode_with_schema`]
    /// and [`AnyRecord::decode_with_stamp`]).
    CellGeometry(Box<CellGeometry>),
    /// 0x0F: region descriptor (`environment.md` §3), also available as
    /// [`crate::landblock::RegionDescriptor::parse`].
    RegionDescriptor(Box<RegionDescriptor>),
}

impl<'a> AnyRecord<'a> {
    /// Decodes `payload` with the decoder for `did`'s type prefix and checks that the record
    /// prologue matches `did` (records without a DID of their own, 0x08 and 0x66, take
    /// `did`). A collision mesh (0x66) is decoded without a directory stamp, by the
    /// fallback of `collision.md` §3.1; use [`AnyRecord::decode_with_stamp`] when the stamp
    /// is known.
    ///
    /// Returns `Ok(None)` when no decoder exists for the prefix; that is not an error.
    ///
    /// # Errors
    ///
    /// Whatever the selected decoder reports, and [`crate::FormatError::DidMismatch`] when
    /// the stored DID is not `did`.
    pub fn decode(did: impl Into<Did>, payload: &'a [u8]) -> Result<Option<Self>> {
        Self::decode_stamped(did.into(), payload, None)
    }

    /// [`AnyRecord::decode`] with the directory stamp for 0x66 records.
    fn decode_stamped(did: Did, payload: &'a [u8], stamp: Option<u16>) -> Result<Option<Self>> {
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
            DidKind::SoundInfo => Self::SoundInfo(SoundInfo::parse_expecting(payload, did)?),
            DidKind::VisualTemplate => {
                Self::VisualTemplate(VisualTemplate::parse_expecting(payload, did)?)
            }
            DidKind::Setup => Self::Setup(Setup::parse_expecting(payload, did)?),
            DidKind::Appearance => {
                Self::Appearance(AppearanceRecord::parse_expecting(payload, did)?)
            }
            DidKind::TerrainTypeTable => {
                Self::TerrainTypeTable(TerrainTypeTable::parse_expecting(payload, did)?)
            }
            DidKind::TerrainSceneTable => {
                Self::TerrainSceneTable(TerrainSceneTable::parse_expecting(payload, did)?)
            }
            DidKind::SceneChoiceTable => {
                Self::SceneChoiceTable(SceneChoiceTable::parse_expecting(payload, did)?)
            }
            DidKind::Scene => Self::Scene(Scene::parse_expecting(payload, did)?),
            DidKind::WaterTable => Self::WaterTable(WaterTable::parse_expecting(payload, did)?),
            DidKind::FogZoneTable => {
                Self::FogZoneTable(FogZoneTable::parse_expecting(payload, did)?)
            }
            DidKind::SkyDescription => {
                Self::SkyDescription(SkyDescription::parse_expecting(payload, did)?)
            }
            DidKind::DayDescription => {
                Self::DayDescription(DayDescription::parse_expecting(payload, did)?)
            }
            DidKind::GameTime => Self::GameTime(GameTime::parse_expecting(payload, did)?),
            DidKind::LodClassTable => {
                Self::LodClassTable(LodClassTable::parse_expecting(payload, did)?)
            }
            DidKind::CameraState => Self::CameraState {
                did,
                state: Box::new(CameraState::parse(payload)?),
            },
            DidKind::CollisionMesh => Self::CollisionMesh {
                did,
                mesh: CollisionMesh::parse(payload, stamp)?,
            },
            DidKind::RegionDescriptor => {
                Self::RegionDescriptor(Box::new(RegionDescriptor::parse_expecting(payload, did)?))
            }
            _ => return Ok(None),
        }))
    }

    /// [`AnyRecord::decode`], plus the record types that need the property dictionary as a
    /// schema: script tables (0x07) and scriptlets (0x0C), which carry no DID of their own
    /// (their [`ScriptTable::did`] / [`Scriptlet::did`] is set to `did`), and cell geometry
    /// (0x01), decoded without a directory stamp by the fallback of `collision.md` §3.1.
    ///
    /// Returns `Ok(None)` when no decoder exists for the prefix.
    ///
    /// # Errors
    ///
    /// As [`AnyRecord::decode`].
    pub fn decode_with_schema<S: PropertySchema + ?Sized>(
        did: impl Into<Did>,
        payload: &'a [u8],
        schema: &S,
    ) -> Result<Option<Self>> {
        Self::decode_with_stamp(did, payload, None, schema)
    }

    /// The most complete dispatcher: [`AnyRecord::decode_with_schema`] with the record's
    /// directory-entry `stamp` (`dat-container.md` §5.2), which decides the collision-tree
    /// `build_byte` of collision meshes (0x66) and cell geometry (0x01) (`collision.md`
    /// §3.1). `None` uses the spec's fallback, as the other dispatchers do.
    ///
    /// Returns `Ok(None)` when no decoder exists for the prefix.
    ///
    /// # Errors
    ///
    /// As [`AnyRecord::decode`].
    pub fn decode_with_stamp<S: PropertySchema + ?Sized>(
        did: impl Into<Did>,
        payload: &'a [u8],
        stamp: Option<u16>,
        schema: &S,
    ) -> Result<Option<Self>> {
        let did = did.into();
        Ok(Some(match did.kind() {
            DidKind::ScriptTable => {
                Self::ScriptTable(ScriptTable::parse_expecting(payload, did, schema)?)
            }
            DidKind::Scriptlet => {
                Self::Scriptlet(Scriptlet::parse_expecting(payload, did, schema)?)
            }
            DidKind::CollisionGeometry => Self::CellGeometry(Box::new(
                CellGeometry::parse_expecting(payload, did, stamp, schema)?,
            )),
            _ => return Self::decode_stamped(did, payload, stamp),
        }))
    }

    /// Whether [`AnyRecord::decode_with_schema`] and [`AnyRecord::decode_with_stamp`] have a
    /// decoder for this DID's prefix.
    #[must_use]
    pub fn is_decodable_with_schema(did: impl Into<Did>) -> bool {
        let did = did.into();
        Self::is_decodable(did)
            || matches!(
                did.kind(),
                DidKind::ScriptTable | DidKind::Scriptlet | DidKind::CollisionGeometry
            )
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
                | DidKind::SoundInfo
                | DidKind::VisualTemplate
                | DidKind::Setup
                | DidKind::Appearance
                | DidKind::TerrainTypeTable
                | DidKind::TerrainSceneTable
                | DidKind::SceneChoiceTable
                | DidKind::Scene
                | DidKind::WaterTable
                | DidKind::FogZoneTable
                | DidKind::SkyDescription
                | DidKind::DayDescription
                | DidKind::GameTime
                | DidKind::LodClassTable
                | DidKind::CameraState
                | DidKind::CollisionMesh
                | DidKind::RegionDescriptor
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
            Self::SoundInfo(r) => r.did(),
            Self::VisualTemplate(r) => r.did(),
            Self::Setup(r) => r.did(),
            Self::Appearance(r) => r.did(),
            Self::ScriptTable(r) => r.did,
            Self::Scriptlet(r) => r.did,
            Self::TerrainTypeTable(r) => r.did(),
            Self::TerrainSceneTable(r) => r.did(),
            Self::SceneChoiceTable(r) => r.did(),
            Self::Scene(r) => r.did(),
            Self::WaterTable(r) => r.did(),
            Self::FogZoneTable(r) => r.did(),
            Self::SkyDescription(r) => r.did(),
            Self::DayDescription(r) => r.did(),
            Self::GameTime(r) => r.did(),
            Self::LodClassTable(r) => r.did(),
            Self::CameraState { did, .. } | Self::CollisionMesh { did, .. } => *did,
            Self::CellGeometry(r) => r.did,
            Self::RegionDescriptor(r) => r.did,
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
            Self::SoundInfo(_) => SoundInfo::NAME,
            Self::VisualTemplate(_) => VisualTemplate::NAME,
            Self::Setup(_) => Setup::NAME,
            Self::Appearance(_) => AppearanceRecord::NAME,
            Self::ScriptTable(_) => ScriptTable::NAME,
            Self::Scriptlet(_) => Scriptlet::NAME,
            Self::TerrainTypeTable(_) => TerrainTypeTable::NAME,
            Self::TerrainSceneTable(_) => TerrainSceneTable::NAME,
            Self::SceneChoiceTable(_) => SceneChoiceTable::NAME,
            Self::Scene(_) => Scene::NAME,
            Self::WaterTable(_) => WaterTable::NAME,
            Self::FogZoneTable(_) => FogZoneTable::NAME,
            Self::SkyDescription(_) => SkyDescription::NAME,
            Self::DayDescription(_) => DayDescription::NAME,
            Self::GameTime(_) => GameTime::NAME,
            Self::LodClassTable(_) => LodClassTable::NAME,
            Self::CameraState { .. } => CameraState::NAME,
            Self::CollisionMesh { .. } => CollisionMesh::NAME,
            Self::CellGeometry(_) => CellGeometry::NAME,
            Self::RegionDescriptor(_) => RegionDescriptor::NAME,
        }
    }
}
