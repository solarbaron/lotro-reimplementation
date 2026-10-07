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

/// How well the meaning of a DID prefix is established (spec §2 status column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Evidence {
    /// Record content is decoded by this crate: by [`crate::AnyRecord`] (including the
    /// script tables and scriptlets of [`crate::AnyRecord::decode_with_schema`]), or, for property
    /// sets and entity descriptors (which need the dictionary as a schema), by
    /// [`crate::PropertySetRecord`] and [`crate::EntityDescriptor`], or, for the landblock
    /// family and its per-region tables (which need the directory stamp or the dictionary),
    /// by the [`crate::landblock`] decoders.
    Decoded,
    /// Meaning confirmed by a DID-map label.
    Labelled,
    /// Meaning reported by third parties only.
    Reported,
    /// Meaning unknown.
    Unknown,
}

/// Sub-type of a landblock-family DID (prefix 0x80), the K nibble (spec §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// Any other K nibble (5 is observed, meaning unknown; its framing is decoded by
    /// [`crate::LandblockGroupRecord`]).
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
/// Names are descriptive. Prefixes whose meaning is unknown map to [`DidKind::Unknown`]
/// carrying the raw prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DidKind {
    /// 0x01: stand-alone collision geometry. Earlier notes called these setups; `setup.md`
    /// §2.2 shows they are not. Not decoded.
    CollisionGeometry,
    /// 0x04: setups (part assemblies with a skeleton tagfile, collision volumes and the part
    /// list; `setup.md` §6), decoded by [`crate::Setup`].
    Setup,
    /// 0x05: animation clips.
    Animation,
    /// 0x06: meshes.
    Mesh,
    /// 0x07: script tables (script-key → node-graph maps; `script-table.md` §2), decoded by
    /// [`crate::ScriptTable`].
    ScriptTable,
    /// 0x0A: audio records.
    Sound,
    /// 0x0C: scriptlets (reusable node graphs invoked by script tables; `script-table.md`
    /// §3), decoded by [`crate::Scriptlet`].
    Scriptlet,
    /// 0x0E: singleton tables.
    SingletonTable,
    /// 0x0F: region descriptors, decoded by [`crate::RegionDescriptor`].
    RegionDescriptor,
    /// 0x18: block-map property descriptors, decoded by [`crate::BlockMapDescriptor`].
    BlockMapProperties,
    /// 0x1D: key maps.
    KeyMap,
    /// 0x1F: visual templates (`setup.md` §5), decoded by [`crate::VisualTemplate`].
    VisualTemplate,
    /// 0x20: appearance / wardrobe records (`setup.md` §8), decoded by
    /// [`crate::AppearanceRecord`].
    Appearance,
    /// 0x22: UI layouts.
    UiLayout,
    /// 0x23: enum mappers.
    EnumMapper,
    /// 0x25: string tables.
    StringTable,
    /// 0x26: reported as part of the string-table range, but no record has been observed
    /// (spec §2.1, status R). Not accepted by the string-table decoder.
    StringTableReported,
    /// 0x28: DID maps.
    DataIdMap,
    /// 0x2A: sound infos (playback parameters naming a 0x0A payload; `sound.md` §4),
    /// decoded by [`crate::SoundInfo`].
    SoundInfo,
    /// 0x2B: compiled shader programs.
    ShaderProgram,
    /// 0x30: materials.
    Material,
    /// 0x31: surface descriptors.
    SurfaceDescriptor,
    /// 0x33: compositors.
    Compositor,
    /// 0x34: master property dictionary.
    PropertyDictionary,
    /// 0x40: texture lists / mip chains.
    TextureList,
    /// 0x41: render surfaces.
    RenderSurface,
    /// 0x42: fonts.
    Font,
    /// 0x47: entity descriptors (`setup.md` §4), decoded by [`crate::EntityDescriptor`].
    EntityDescriptor,
    /// 0x51: action maps.
    ActionMap,
    /// 0x56: compiled script library.
    ScriptLibrary,
    /// 0x70: script-state objects.
    ScriptStateObject,
    /// 0x76: script-state objects (variant).
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
            0x04 => Self::Setup,
            0x05 => Self::Animation,
            0x06 => Self::Mesh,
            0x07 => Self::ScriptTable,
            0x0A => Self::Sound,
            0x0C => Self::Scriptlet,
            0x0E => Self::SingletonTable,
            0x0F => Self::RegionDescriptor,
            0x18 => Self::BlockMapProperties,
            0x1D => Self::KeyMap,
            0x1F => Self::VisualTemplate,
            0x20 => Self::Appearance,
            0x22 => Self::UiLayout,
            0x23 => Self::EnumMapper,
            0x25 => Self::StringTable,
            0x26 => Self::StringTableReported,
            0x28 => Self::DataIdMap,
            0x2A => Self::SoundInfo,
            0x2B => Self::ShaderProgram,
            0x30 => Self::Material,
            0x31 => Self::SurfaceDescriptor,
            0x33 => Self::Compositor,
            0x34 => Self::PropertyDictionary,
            0x40 => Self::TextureList,
            0x41 => Self::RenderSurface,
            0x42 => Self::Font,
            0x47 => Self::EntityDescriptor,
            0x51 => Self::ActionMap,
            0x56 => Self::ScriptLibrary,
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

    /// How well this classification is established.
    #[must_use]
    pub fn evidence(self) -> Evidence {
        match self {
            Self::Sound
            | Self::SoundInfo
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
            | Self::Landblock(
                LandblockKind::Heightmap
                | LandblockKind::BlockMap
                | LandblockKind::Info
                | LandblockKind::Properties,
            ) => Evidence::Decoded,
            Self::SingletonTable
            | Self::KeyMap
            | Self::ShaderProgram
            | Self::Compositor
            | Self::Font
            | Self::ActionMap
            | Self::ScriptLibrary
            | Self::ScriptStateObject => Evidence::Labelled,
            Self::Unknown(_) | Self::Landblock(LandblockKind::Other(_)) | Self::ArchiveMetadata => {
                Evidence::Unknown
            }
            _ => Evidence::Reported,
        }
    }

    /// Short descriptive name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::CollisionGeometry => "collision geometry",
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
            Self::SoundInfo => "sound info",
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

/// Whether a prefix appears in the spec §2.1 table: either it has a named [`DidKind`] or it
/// is listed there with an OPEN meaning. A DID whose prefix is not catalogued has never been
/// observed in data.
#[must_use]
pub fn is_catalogued_prefix(prefix: u8) -> bool {
    /// Prefixes listed in §2.1 with an unknown (OPEN) meaning.
    const OPEN_PREFIXES: [u8; 21] = [
        0x02, 0x08, 0x11, 0x12, 0x14, 0x15, 0x16, 0x17, 0x19, 0x1A, 0x21, 0x27, 0x2A, 0x2D, 0x32,
        0x35, 0x39, 0x43, 0x44, 0x58, 0x66,
    ];
    !matches!(
        DidKind::of(Did(u32::from(prefix) << 24)),
        DidKind::Unknown(_)
    ) || OPEN_PREFIXES.contains(&prefix)
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
