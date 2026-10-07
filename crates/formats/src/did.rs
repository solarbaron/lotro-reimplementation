// SPDX-License-Identifier: GPL-3.0-or-later
//! Data IDs and their classification by high byte.
//!
//! Spec: `docs/specs/records.md` §2.

use std::fmt;

/// A 32-bit data ID. The high byte is the record type, the low 24 bits a serial number.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Did(pub u32);

impl Did {
    /// The raw value.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// High byte (record type prefix).
    #[must_use]
    pub const fn prefix(self) -> u8 {
        (self.0 >> 24) as u8
    }

    /// Low 24 bits.
    #[must_use]
    pub const fn serial(self) -> u32 {
        self.0 & 0x00FF_FFFF
    }

    /// Whether this is the zero ("undefined") DID.
    #[must_use]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }

    /// Classification of this DID by its high byte.
    #[must_use]
    pub fn kind(self) -> DidKind {
        DidKind::of(self)
    }
}

impl From<u32> for Did {
    fn from(v: u32) -> Self {
        Self(v)
    }
}

impl From<Did> for u32 {
    fn from(d: Did) -> Self {
        d.0
    }
}

impl fmt::Debug for Did {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Did({:#010X})", self.0)
    }
}

impl fmt::Display for Did {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#010X}", self.0)
    }
}

impl fmt::LowerHex for Did {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl fmt::UpperHex for Did {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::UpperHex::fmt(&self.0, f)
    }
}

/// How well the meaning of a DID prefix is established (spec §2.1 status column: C, L, BIN,
/// R), and whether this crate decodes its records.
///
/// When a prefix has several statuses the strongest applies, in the order of the variants:
/// for example 0x0E (R, L, BIN) is [`Evidence::Labelled`] and 0x2B (C, L) is
/// [`Evidence::Specified`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Evidence {
    /// Record content is decoded by this crate: by [`crate::AnyRecord`] (including the
    /// script tables, scriptlets and cell geometry of
    /// [`crate::AnyRecord::decode_with_schema`]), or, for property sets and entity
    /// descriptors (which need the dictionary as a schema), by [`crate::PropertySetRecord`]
    /// and [`crate::EntityDescriptor`], or, for the landblock family and its block-map
    /// descriptors (which need the directory stamp or the dictionary), by the
    /// [`crate::landblock`] decoders.
    Decoded,
    /// The layout is specified (status C) and decoded by another Tessera crate, not by this
    /// one: animation clips (0x05), meshes (0x06), shader records, materials and surface
    /// records (0x2B, 0x30, 0x31), the script library (0x56) and script-state objects (0x70,
    /// 0x76).
    Specified,
    /// Meaning confirmed by a DID-map label; the layout is not specified (status L).
    Labelled,
    /// Only the DB object type the client registers for the prefix is known; the layout is
    /// not specified (status BIN).
    DbType,
    /// Meaning reported by third parties only (status R).
    Reported,
    /// Meaning unknown.
    Unknown,
}

/// Sub-type of a landblock-family DID (prefix 0x80), the K nibble (spec §2.2).
///
/// Non-exhaustive: K nibbles that are now carried by [`LandblockKind::Other`] may get a
/// named variant later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum LandblockKind {
    /// K = 0: landblock data (heights, terrain surface), decoded by
    /// [`crate::LandblockData`].
    Heightmap,
    /// K = 1: block-map indices, decoded by [`crate::BlockMap`].
    BlockMap,
    /// K = 2: landblock info (cells, entities, links), decoded by [`crate::LandblockInfo`].
    Info,
    /// K = 4: landblock properties (minimap), decoded by [`crate::LandblockProperties`].
    Properties,
    /// Any other K nibble. K = 5 is the distant-landscape sector (spec §2.2, DB type
    /// DISTANT_LANDSCAPE_SECTOR; framing C, most of the layout OPEN), whose framing is
    /// decoded by [`crate::LandblockGroupRecord`]; [`DidKind::name`] names it. K = 6 is
    /// registered (BIN) with no record in the install.
    Other(u8),
}

/// Decoded landblock-family DID: `0x80 | K<<20 | R<<16 | X<<8 | Y`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LandblockId {
    /// Record sub-type.
    pub kind: LandblockKind,
    /// Region number (1–5 or 14 in observed data).
    pub region: u8,
    /// Landblock X coordinate (0–254 in observed data).
    pub x: u8,
    /// Landblock Y coordinate (0–254 in observed data).
    pub y: u8,
}

impl LandblockId {
    /// Splits a landblock-family DID into its parts; `None` if the prefix is not 0x80.
    #[must_use]
    pub fn from_did(did: Did) -> Option<Self> {
        if did.prefix() != 0x80 {
            return None;
        }
        let k = ((did.0 >> 20) & 0xF) as u8;
        let kind = match k {
            0 => LandblockKind::Heightmap,
            1 => LandblockKind::BlockMap,
            2 => LandblockKind::Info,
            4 => LandblockKind::Properties,
            other => LandblockKind::Other(other),
        };
        Some(Self {
            kind,
            region: ((did.0 >> 16) & 0xF) as u8,
            x: (did.0 >> 8) as u8,
            y: did.0 as u8,
        })
    }

    /// Reassembles the DID.
    #[must_use]
    pub fn to_did(self) -> Did {
        let k = match self.kind {
            LandblockKind::Heightmap => 0,
            LandblockKind::BlockMap => 1,
            LandblockKind::Info => 2,
            LandblockKind::Properties => 4,
            LandblockKind::Other(k) => u32::from(k & 0xF),
        };
        Did(0x8000_0000
            | (k << 20)
            | (u32::from(self.region & 0xF) << 16)
            | (u32::from(self.x) << 8)
            | u32::from(self.y))
    }
}

/// Record type implied by a DID's high byte (spec §2.1).
///
/// Names are descriptive. Every prefix of the table that has records in the install has a
/// named kind. Prefixes with no record in the install map to [`DidKind::Unknown`] carrying
/// the raw prefix, even where the table lists a DB type for them: the registered-only
/// prefixes 0x0D, 0x24, 0x46, 0x53 and 0x90–0x9F, and the unused members of the ranges
/// 0x70–0x77 (0x71–0x75, 0x77) and 0x78–0x7F (0x7A–0x7F). [`is_catalogued_prefix`] is
/// false for them. See [`crate::is_property_set_did`] for the property-set range.
///
/// The derived order follows the declaration order, which is ascending by prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum DidKind {
    /// 0x01: cell geometry (cell volume, portals, wall polygons, hull vertices, physics mesh;
    /// `collision.md` §5). Earlier notes called these setups; `setup.md` §2.2 shows they are
    /// not. Decoded by [`crate::CellGeometry`] (M4.5).
    CollisionGeometry,
    /// 0x02: scene records (DB type SCENE; `scenery.md` §5), decoded by [`crate::Scene`]
    /// (M4.5).
    Scene,
    /// 0x04: setups (part assemblies with a skeleton tagfile, collision volumes and the part
    /// list; `setup.md` §6), decoded by [`crate::Setup`].
    Setup,
    /// 0x05: animation clips (`havok-data.md` §2.3).
    Animation,
    /// 0x06: meshes (`mesh.md`; decoded by `tessera-mesh`).
    Mesh,
    /// 0x07: script tables (script-key → node-graph maps; `script-table.md` §2), decoded by
    /// [`crate::ScriptTable`].
    ScriptTable,
    /// 0x08: camera-state records (DB type CAMERASTATE; `walker.md` §11.5). Decoded in
    /// [`crate::environment`] (M4.5, optional).
    CameraState,
    /// 0x0A: audio records.
    Sound,
    /// 0x0C: scriptlets (reusable node graphs invoked by script tables; `script-table.md`
    /// §3), decoded by [`crate::Scriptlet`].
    Scriptlet,
    /// 0x0E: singleton tables.
    SingletonTable,
    /// 0x0F: region descriptors, decoded by [`crate::RegionDescriptor`].
    RegionDescriptor,
    /// 0x11: scene-choice tables (DB type SCENE_DESC; `scenery.md` §4.3). Decoded in
    /// [`crate::scenery`] (M4.5).
    SceneChoiceTable,
    /// 0x12: terrain-scene tables (DB type TERRAIN_DESC; `scenery.md` §4.2). Decoded in
    /// [`crate::scenery`] (M4.5).
    TerrainSceneTable,
    /// 0x14: encounter tables, region slot 3 (DB type ENCOUNTER_DESC; layout OPEN,
    /// `environment.md` §4).
    EncounterTable,
    /// 0x15: sky descriptions (`environment.md` §5.1). Decoded in [`crate::environment`]
    /// (M4.5).
    SkyDescription,
    /// 0x16: water tables (DB type WATER_DESC; `water.md` §4). Decoded in [`crate::water`]
    /// (M4.5).
    WaterTable,
    /// 0x17: fog-zone tables (DB type FOG_DESC; `water.md` §5). Decoded in [`crate::water`]
    /// (M4.5).
    FogZoneTable,
    /// 0x18: block-map property descriptors, decoded by [`crate::BlockMapDescriptor`].
    BlockMapProperties,
    /// 0x19: terrain type tables (DB type TERRAIN_TYPE_TABLE; `terrain-texturing.md` §3).
    /// Decoded in [`crate::terrain_type`] (M4.5).
    TerrainTypeTable,
    /// 0x1A: day descriptions (DB type DAY_DESC; `environment.md` §5.2). Decoded in
    /// [`crate::environment`] (M4.5).
    DayDescription,
    /// 0x1D: key maps.
    KeyMap,
    /// 0x1F: visual templates (`setup.md` §5), decoded by [`crate::VisualTemplate`].
    VisualTemplate,
    /// 0x20: appearance / wardrobe records (`setup.md` §8), decoded by
    /// [`crate::AppearanceRecord`].
    Appearance,
    /// 0x21: UI scenes (DB type UI_SCENE; layout OPEN).
    UiScene,
    /// 0x22: UI layouts.
    UiLayout,
    /// 0x23: enum mappers.
    EnumMapper,
    /// 0x25: string tables.
    StringTable,
    /// 0x26: part of the string-table range (DB type STRING_TABLE registered for
    /// 0x25000000–0x26FFFFFF; spec §2.1, status R, BIN). Not accepted by the string-table
    /// decoder.
    StringTableReported,
    /// 0x27: per-region imposter tables (DB type RENDER_IMPOSTER_TABLE; layout OPEN).
    ImposterTable,
    /// 0x28: DID maps.
    DataIdMap,
    /// 0x2A: sound-info records (DB type SOUNDINFO; layout OPEN).
    SoundInfo,
    /// 0x2B: compiled shader programs (shader records, `material.md` §7; decoded by
    /// `tessera-material`).
    ShaderProgram,
    /// 0x2D: compute shaders (DB type COMPUTESHADER; a DXBC container).
    ComputeShader,
    /// 0x30: materials (`material.md` §6; decoded by `tessera-material`).
    Material,
    /// 0x31: surface descriptors (`material.md` §4; decoded by `tessera-material`).
    SurfaceDescriptor,
    /// 0x32: the LOD-class table (DB type RENDER_LOD_CLASSES; `environment.md` §9.1).
    /// Decoded in [`crate::environment`] (M4.5).
    LodClassTable,
    /// 0x33: compositors.
    Compositor,
    /// 0x34: master property dictionary.
    PropertyDictionary,
    /// 0x35: game time (DB type GAME_TIME; `environment.md` §6). Decoded in
    /// [`crate::environment`] (M4.5).
    GameTime,
    /// 0x39: particle-system descriptors (DB type PSDESC; layout OPEN).
    ParticleSystemDescriptor,
    /// 0x40: texture lists / mip chains.
    TextureList,
    /// 0x41: render surfaces.
    RenderSurface,
    /// 0x42: fonts.
    Font,
    /// 0x43: script channels (DB type SCRIPTCHANNELS; layout OPEN).
    ScriptChannels,
    /// 0x44: particle systems: texture names, particle templates (DB type FORKPSDESC;
    /// layout OPEN).
    ParticleSystem,
    /// 0x47: entity descriptors (`setup.md` §4), decoded by [`crate::EntityDescriptor`].
    EntityDescriptor,
    /// 0x51: action maps.
    ActionMap,
    /// 0x56: compiled script library (`wlib.md`).
    ScriptLibrary,
    /// 0x58: string state (DB type STRING_STATE; layout OPEN).
    StringState,
    /// 0x66: stand-alone collision meshes (DB type PHYSICS_MESH; `collision.md` §4).
    /// Decoded in [`crate::collision`] (M4.5).
    CollisionMesh,
    /// 0x70: script-state objects (`wstate.md`).
    ScriptStateObject,
    /// 0x76: script-state objects (variant; `wstate.md`).
    ScriptStateVariant,
    /// 0x78: stand-alone property sets.
    PropertySet,
    /// 0x79: property set of the 0x70 object with the same serial.
    ObjectPropertySet,
    /// 0x80: landblock family.
    Landblock(LandblockKind),
    /// 0xFF: archive metadata records.
    ArchiveMetadata,
    /// Any prefix whose meaning is not established.
    Unknown(u8),
}

/// Former variant names, kept as deprecated aliases so that code written against them
/// fails with a pointer to the new name. The former `DidKind::Setup` meant 0x01; that name
/// now means 0x04 (`setup.md` §2.2).
#[allow(non_upper_case_globals)]
impl DidKind {
    /// Former name of [`DidKind::Setup`] (0x04).
    #[deprecated(note = "renamed to `DidKind::Setup` (0x04 setups)")]
    pub const Skeleton: Self = Self::Setup;
    /// Former name of [`DidKind::VisualTemplate`] (0x1F).
    #[deprecated(note = "renamed to `DidKind::VisualTemplate`")]
    pub const HeldItemTemplate: Self = Self::VisualTemplate;
    /// Former name of [`DidKind::EntityDescriptor`] (0x47).
    #[deprecated(note = "renamed to `DidKind::EntityDescriptor`")]
    pub const PhysicsDescriptor: Self = Self::EntityDescriptor;
    /// Former name of [`DidKind::Scriptlet`] (0x0C; `script-table.md` §3).
    #[deprecated(note = "renamed to `DidKind::Scriptlet` (0x0C scriptlets)")]
    pub const EntityData: Self = Self::Scriptlet;
}

impl DidKind {
    /// Classifies a DID by its high byte.
    #[must_use]
    pub fn of(did: Did) -> Self {
        match did.prefix() {
            0x01 => Self::CollisionGeometry,
            0x02 => Self::Scene,
            0x04 => Self::Setup,
            0x05 => Self::Animation,
            0x06 => Self::Mesh,
            0x07 => Self::ScriptTable,
            0x08 => Self::CameraState,
            0x0A => Self::Sound,
            0x0C => Self::Scriptlet,
            0x0E => Self::SingletonTable,
            0x0F => Self::RegionDescriptor,
            0x11 => Self::SceneChoiceTable,
            0x12 => Self::TerrainSceneTable,
            0x14 => Self::EncounterTable,
            0x15 => Self::SkyDescription,
            0x16 => Self::WaterTable,
            0x17 => Self::FogZoneTable,
            0x18 => Self::BlockMapProperties,
            0x19 => Self::TerrainTypeTable,
            0x1A => Self::DayDescription,
            0x1D => Self::KeyMap,
            0x1F => Self::VisualTemplate,
            0x20 => Self::Appearance,
            0x21 => Self::UiScene,
            0x22 => Self::UiLayout,
            0x23 => Self::EnumMapper,
            0x25 => Self::StringTable,
            0x26 => Self::StringTableReported,
            0x27 => Self::ImposterTable,
            0x28 => Self::DataIdMap,
            0x2A => Self::SoundInfo,
            0x2B => Self::ShaderProgram,
            0x2D => Self::ComputeShader,
            0x30 => Self::Material,
            0x31 => Self::SurfaceDescriptor,
            0x32 => Self::LodClassTable,
            0x33 => Self::Compositor,
            0x34 => Self::PropertyDictionary,
            0x35 => Self::GameTime,
            0x39 => Self::ParticleSystemDescriptor,
            0x40 => Self::TextureList,
            0x41 => Self::RenderSurface,
            0x42 => Self::Font,
            0x43 => Self::ScriptChannels,
            0x44 => Self::ParticleSystem,
            0x47 => Self::EntityDescriptor,
            0x51 => Self::ActionMap,
            0x56 => Self::ScriptLibrary,
            0x58 => Self::StringState,
            0x66 => Self::CollisionMesh,
            0x70 => Self::ScriptStateObject,
            0x76 => Self::ScriptStateVariant,
            0x78 => Self::PropertySet,
            0x79 => Self::ObjectPropertySet,
            0x80 => Self::Landblock(
                LandblockId::from_did(did).map_or(LandblockKind::Other(0), |l| l.kind),
            ),
            0xFF => Self::ArchiveMetadata,
            other => Self::Unknown(other),
        }
    }

    /// How well this classification is established (spec §2.1 and §2.2 status columns).
    #[must_use]
    pub fn evidence(self) -> Evidence {
        match self {
            Self::Sound
            | Self::EnumMapper
            | Self::StringTable
            | Self::DataIdMap
            | Self::PropertyDictionary
            | Self::RenderSurface
            | Self::TextureList
            | Self::PropertySet
            | Self::ObjectPropertySet
            | Self::Setup
            | Self::VisualTemplate
            | Self::Appearance
            | Self::EntityDescriptor
            | Self::RegionDescriptor
            | Self::BlockMapProperties
            | Self::ScriptTable
            | Self::Scriptlet
            | Self::CollisionGeometry
            | Self::Scene
            | Self::CameraState
            | Self::SceneChoiceTable
            | Self::TerrainSceneTable
            | Self::SkyDescription
            | Self::WaterTable
            | Self::FogZoneTable
            | Self::TerrainTypeTable
            | Self::DayDescription
            | Self::LodClassTable
            | Self::GameTime
            | Self::CollisionMesh
            | Self::Landblock(
                LandblockKind::Heightmap
                | LandblockKind::BlockMap
                | LandblockKind::Info
                | LandblockKind::Properties,
            ) => Evidence::Decoded,
            Self::Animation
            | Self::Mesh
            | Self::ShaderProgram
            | Self::Material
            | Self::SurfaceDescriptor
            | Self::ScriptLibrary
            | Self::ScriptStateObject
            | Self::ScriptStateVariant => Evidence::Specified,
            Self::SingletonTable
            | Self::KeyMap
            | Self::Compositor
            | Self::Font
            | Self::ActionMap => Evidence::Labelled,
            Self::EncounterTable
            | Self::UiScene
            | Self::UiLayout
            | Self::StringTableReported
            | Self::ImposterTable
            | Self::SoundInfo
            | Self::ComputeShader
            | Self::ParticleSystemDescriptor
            | Self::ScriptChannels
            | Self::ParticleSystem
            | Self::StringState => Evidence::DbType,
            // K = 5 (distant-landscape sector, framing C) is still reported as unknown here.
            Self::Unknown(_) | Self::Landblock(LandblockKind::Other(_)) | Self::ArchiveMetadata => {
                Evidence::Unknown
            }
        }
    }

    /// Short descriptive name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::CollisionGeometry => "collision geometry",
            Self::Scene => "scene",
            Self::CameraState => "camera state",
            Self::SceneChoiceTable => "scene-choice table",
            Self::TerrainSceneTable => "terrain-scene table",
            Self::EncounterTable => "encounter table",
            Self::UiScene => "UI scene",
            Self::ImposterTable => "imposter table",
            Self::SoundInfo => "sound info",
            Self::ComputeShader => "compute shader",
            Self::ParticleSystemDescriptor => "particle-system descriptor",
            Self::ScriptChannels => "script channels",
            Self::ParticleSystem => "particle system",
            Self::StringState => "string state",
            Self::SkyDescription => "sky description",
            Self::WaterTable => "water table",
            Self::FogZoneTable => "fog-zone table",
            Self::TerrainTypeTable => "terrain type table",
            Self::DayDescription => "day description",
            Self::LodClassTable => "LOD-class table",
            Self::GameTime => "game time",
            Self::CollisionMesh => "collision mesh",
            Self::Setup => "setup",
            Self::Animation => "animation",
            Self::Mesh => "mesh",
            Self::ScriptTable => "script table",
            Self::Sound => "sound",
            Self::Scriptlet => "scriptlet",
            Self::SingletonTable => "singleton table",
            Self::RegionDescriptor => "region descriptor",
            Self::BlockMapProperties => "block-map properties",
            Self::KeyMap => "key map",
            Self::VisualTemplate => "visual template",
            Self::Appearance => "appearance",
            Self::UiLayout => "UI layout",
            Self::EnumMapper => "enum mapper",
            Self::StringTable => "string table",
            Self::StringTableReported => "string table (reported)",
            Self::DataIdMap => "DID map",
            Self::ShaderProgram => "shader program",
            Self::Material => "material",
            Self::SurfaceDescriptor => "surface descriptor",
            Self::Compositor => "compositor",
            Self::PropertyDictionary => "property dictionary",
            Self::TextureList => "texture list",
            Self::RenderSurface => "render surface",
            Self::Font => "font",
            Self::EntityDescriptor => "entity descriptor",
            Self::ActionMap => "action map",
            Self::ScriptLibrary => "script library",
            Self::ScriptStateObject => "script-state object",
            Self::ScriptStateVariant => "script-state variant",
            Self::PropertySet => "property set",
            Self::ObjectPropertySet => "object property set",
            Self::Landblock(LandblockKind::Heightmap) => "landblock heightmap",
            Self::Landblock(LandblockKind::BlockMap) => "landblock block map",
            Self::Landblock(LandblockKind::Info) => "landblock info",
            Self::Landblock(LandblockKind::Properties) => "landblock properties",
            Self::Landblock(LandblockKind::Other(5)) => "distant-landscape sector",
            Self::Landblock(LandblockKind::Other(_)) => "landblock (other)",
            Self::ArchiveMetadata => "archive metadata",
            Self::Unknown(_) => "unknown",
        }
    }
}

impl fmt::Display for DidKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(p) => write!(f, "unknown (prefix {p:#04X})"),
            _ => f.write_str(self.name()),
        }
    }
}

/// Whether a prefix appears in the spec §2.1 table with records in the install, which is
/// exactly when it has a named [`DidKind`]. A DID whose prefix is not catalogued has never
/// been observed in data (the registered-only prefixes and range members listed at
/// [`DidKind`] included).
#[must_use]
pub fn is_catalogued_prefix(prefix: u8) -> bool {
    !matches!(
        DidKind::of(Did(u32::from(prefix) << 24)),
        DidKind::Unknown(_)
    )
}

/// Name hash used for argument IDs and variable IDs (spec §1.5). Result is < 0x10000000.
#[must_use]
pub fn name_hash(name: &[u8]) -> u32 {
    let mut h: u32 = 0;
    for &c in name {
        h = (h << 4).wrapping_add(u32::from(c));
        let g = h & 0xF000_0000;
        h ^= g >> 24;
    }
    h & 0x0FFF_FFFF
}

/// [`name_hash`] over a UTF-16 name; `None` if the name contains non-ASCII units.
#[must_use]
pub fn name_hash_utf16(name: &[u16]) -> Option<u32> {
    let mut bytes = Vec::with_capacity(name.len());
    for &u in name {
        bytes.push(u8::try_from(u).ok().filter(u8::is_ascii)?);
    }
    Some(name_hash(&bytes))
}
