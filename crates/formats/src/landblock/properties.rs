// SPDX-License-Identifier: GPL-3.0-or-later
//! Landblock properties `0x804RXXYY` (minimap tile, light maps), the property IDs the
//! landblock family uses, and the framing of kind-5 records `0x805RXXYY`.
//!
//! Spec: `docs/specs/landblock.md` §6, §7; `docs/specs/coordinates.md` §10.

use super::{LANDBLOCK_SIZE, read_embedded_set, read_landblock_did};
use crate::did::{Did, LandblockId, LandblockKind};
use crate::error::{FormatError, Result};
use crate::property::{PropertyDictionary, PropertySchema, PropertyStruct, PropertyValue};
use crate::property_set::PropertySet;
use crate::reader::Reader;
use crate::record::Record;

/// Samples in a kind-5 record (99 × 99) in every observed record (§7).
pub const GROUP_SAMPLE_COUNT: u32 = 9_801;
/// Stored size of one kind-5 sample.
const GROUP_SAMPLE_SIZE: usize = 5;

/// Property IDs used by the landblock family, looked up by internal name in the property
/// dictionary at run time (PIDs are data, not constants). A name the dictionary lacks
/// leaves its field `None`, and the helpers that need it return nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LandblockPids {
    /// PID of `UI_Map_GameMap`.
    pub ui_map_game_map: Option<u32>,
    /// PID of `UI_Map_MapImage`.
    pub ui_map_map_image: Option<u32>,
    /// PID of `UI_Map_BlockOffsetX`.
    pub ui_map_block_offset_x: Option<u32>,
    /// PID of `UI_Map_BlockOffsetY`.
    pub ui_map_block_offset_y: Option<u32>,
    /// PID of `UI_Map_PixelOffsetX`.
    pub ui_map_pixel_offset_x: Option<u32>,
    /// PID of `UI_Map_PixelOffsetY`.
    pub ui_map_pixel_offset_y: Option<u32>,
    /// PID of `UI_Map_Scale`.
    pub ui_map_scale: Option<u32>,
    /// PID of `UI_Map_FogOfWar`.
    pub ui_map_fog_of_war: Option<u32>,
    /// PID of `UI_Map_FogOfWar_Color`.
    pub ui_map_fog_of_war_color: Option<u32>,
    /// PID of `UI_Map_MapImageArray`.
    pub ui_map_map_image_array: Option<u32>,
    /// PID of `UI_Map_ContentLayer`.
    pub ui_map_content_layer: Option<u32>,
    /// PID of `Landblock_LightMap`.
    pub landblock_light_map: Option<u32>,
    /// PID of `Landblock_CeilingLightMap`.
    pub landblock_ceiling_light_map: Option<u32>,
    /// PID of `Render_AdjustableScale`.
    pub render_adjustable_scale: Option<u32>,
    /// PID of `Physics_AdjustableScale`.
    pub physics_adjustable_scale: Option<u32>,
    /// PID of `Entity_ContentLayers`.
    pub entity_content_layers: Option<u32>,
    /// PID of `Render_Draw`.
    pub render_draw: Option<u32>,
    /// PID of `Render_DrawMeshes`.
    pub render_draw_meshes: Option<u32>,
    /// PID of `Render_NeverAddToScene`.
    pub render_never_add_to_scene: Option<u32>,
    /// PID of `Landblock_UseCeiling`.
    pub landblock_use_ceiling: Option<u32>,
    /// PID of `Area_DID`.
    pub area_did: Option<u32>,
    /// PID of `Environment_LocalSky`.
    pub environment_local_sky: Option<u32>,
    /// PID of `Ambient_MusicType`.
    pub ambient_music_type: Option<u32>,
    /// PID of `Ambient_MusicRegion`.
    pub ambient_music_region: Option<u32>,
    /// PID of `Ambient_SoundEnum_BlockMapOverride`.
    pub ambient_sound_override: Option<u32>,
}

impl LandblockPids {
    /// Looks every PID up with `pid_of(name)`.
    #[must_use]
    pub fn from_names(pid_of: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            ui_map_game_map: pid_of("UI_Map_GameMap"),
            ui_map_map_image: pid_of("UI_Map_MapImage"),
            ui_map_block_offset_x: pid_of("UI_Map_BlockOffsetX"),
            ui_map_block_offset_y: pid_of("UI_Map_BlockOffsetY"),
            ui_map_pixel_offset_x: pid_of("UI_Map_PixelOffsetX"),
            ui_map_pixel_offset_y: pid_of("UI_Map_PixelOffsetY"),
            ui_map_scale: pid_of("UI_Map_Scale"),
            ui_map_fog_of_war: pid_of("UI_Map_FogOfWar"),
            ui_map_fog_of_war_color: pid_of("UI_Map_FogOfWar_Color"),
            ui_map_map_image_array: pid_of("UI_Map_MapImageArray"),
            ui_map_content_layer: pid_of("UI_Map_ContentLayer"),
            landblock_light_map: pid_of("Landblock_LightMap"),
            landblock_ceiling_light_map: pid_of("Landblock_CeilingLightMap"),
            render_adjustable_scale: pid_of("Render_AdjustableScale"),
            physics_adjustable_scale: pid_of("Physics_AdjustableScale"),
            entity_content_layers: pid_of("Entity_ContentLayers"),
            render_draw: pid_of("Render_Draw"),
            render_draw_meshes: pid_of("Render_DrawMeshes"),
            render_never_add_to_scene: pid_of("Render_NeverAddToScene"),
            landblock_use_ceiling: pid_of("Landblock_UseCeiling"),
            area_did: pid_of("Area_DID"),
            environment_local_sky: pid_of("Environment_LocalSky"),
            ambient_music_type: pid_of("Ambient_MusicType"),
            ambient_music_region: pid_of("Ambient_MusicRegion"),
            ambient_sound_override: pid_of("Ambient_SoundEnum_BlockMapOverride"),
        }
    }

    /// Looks every PID up in the master property dictionary.
    #[must_use]
    pub fn from_dictionary(dict: &PropertyDictionary) -> Self {
        Self::from_names(|n| dict.pid(n))
    }
}

/// An alternative minimap tile for a content layer (§6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MinimapVariant {
    /// The content layer that selects this tile (`UI_Map_ContentLayer`).
    pub content_layer: u32,
    /// The tile image, a 0x41 render surface.
    pub image: Did,
}

/// A landblock's minimap tile and its georeference (§6.2–§6.3, `coordinates.md` §10).
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapTile {
    /// The base tile image (0x41 render surface; a 200 × 200 JPEG in every checked case).
    pub image: Did,
    /// `UI_Map_BlockOffsetX`, `UI_Map_BlockOffsetY` (bx, by): always this block's X, Y.
    pub block_offset: [i32; 2],
    /// `UI_Map_PixelOffsetX`, `UI_Map_PixelOffsetY` (px0, py0): always (0, 200).
    pub pixel_offset: [i32; 2],
    /// `UI_Map_Scale`: pixels per world unit (1.25).
    pub scale: f32,
    /// `UI_Map_FogOfWar`, if present.
    pub fog_of_war: Option<i64>,
    /// `UI_Map_FogOfWar_Color` in stored byte order, if present.
    pub fog_colour: Option<[u8; 4]>,
    /// Alternative tiles per content layer (`UI_Map_MapImageArray`). The base tile applies
    /// when none of the listed layers is active.
    pub variants: Vec<MinimapVariant>,
}

impl MinimapTile {
    /// Region-global position (G_x, G_y) of pixel (px, py) (origin top-left, py down;
    /// `coordinates.md` §10). `None` when the scale is not a positive finite number.
    #[must_use]
    pub fn pixel_to_region(&self, px: f64, py: f64) -> Option<[f64; 2]> {
        let s = f64::from(self.scale);
        if !(s.is_finite() && s > 0.0) {
            return None;
        }
        let size = f64::from(LANDBLOCK_SIZE);
        Some([
            size * f64::from(self.block_offset[0]) + (px - f64::from(self.pixel_offset[0])) / s,
            size * f64::from(self.block_offset[1]) + (f64::from(self.pixel_offset[1]) - py) / s,
        ])
    }

    /// Pixel (px, py) of region-global position (G_x, G_y), the inverse of
    /// [`MinimapTile::pixel_to_region`].
    #[must_use]
    pub fn region_to_pixel(&self, g: [f64; 2]) -> [f64; 2] {
        let s = f64::from(self.scale);
        let size = f64::from(LANDBLOCK_SIZE);
        [
            f64::from(self.pixel_offset[0]) + (g[0] - size * f64::from(self.block_offset[0])) * s,
            f64::from(self.pixel_offset[1]) - (g[1] - size * f64::from(self.block_offset[1])) * s,
        ]
    }

    /// The tile image for the active content layers: the first variant whose layer is
    /// active, else the base image.
    #[must_use]
    pub fn image_for(&self, active: impl Fn(u32) -> bool) -> Did {
        self.variants
            .iter()
            .find(|v| active(v.content_layer))
            .map_or(self.image, |v| v.image)
    }
}

/// A landblock-properties record `0x804RXXYY` (§6.1): own DID and one property set.
#[derive(Debug, Clone, PartialEq)]
pub struct LandblockProperties {
    /// Own DID.
    pub did: Did,
    /// The property set.
    pub properties: PropertySet,
}

impl LandblockProperties {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "landblock properties";

    /// Decodes a complete payload with `schema` (normally the property dictionary). The
    /// payload must be consumed exactly.
    pub fn parse<S: PropertySchema + ?Sized>(data: &[u8], schema: &S) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_landblock_did(&mut r, LandblockKind::Properties, Self::NAME)?;
        let properties = read_embedded_set(&mut r, schema)?;
        r.expect_end(Self::NAME)?;
        r.log_unknown_hints(Self::NAME);
        Ok(Self { did, properties })
    }

    /// [`LandblockProperties::parse`], also checking the DID against the one looked up.
    pub fn parse_expecting<S: PropertySchema + ?Sized>(
        data: &[u8],
        expected: impl Into<Did>,
        schema: &S,
    ) -> Result<Self> {
        let expected = expected.into();
        let rec = Self::parse(data, schema)?;
        if rec.did != expected {
            return Err(FormatError::DidMismatch {
                expected: expected.0,
                found: rec.did.0,
            });
        }
        Ok(rec)
    }

    /// The landblock this record describes.
    #[must_use]
    pub fn landblock(&self) -> Option<LandblockId> {
        LandblockId::from_did(self.did)
    }

    /// The minimap tile (`UI_Map_GameMap`), or `None` when the record has none (light-map
    /// only records) or a required field is missing.
    #[must_use]
    pub fn minimap(&self, pids: &LandblockPids) -> Option<MinimapTile> {
        let map = self.properties.get(pids.ui_map_game_map?)?.as_struct()?;
        let field = |pid: Option<u32>| pid.and_then(|p| map.get(p));
        let int = |pid: Option<u32>| field(pid).and_then(as_i64);
        let i32_of = |pid: Option<u32>| int(pid).and_then(|v| i32::try_from(v).ok());
        let image = field(pids.ui_map_map_image).and_then(as_did)?;
        let scale = match field(pids.ui_map_scale)? {
            PropertyValue::Float(f) => *f,
            _ => return None,
        };
        let fog_colour = match field(pids.ui_map_fog_of_war_color) {
            Some(PropertyValue::Colour(c)) => Some(*c),
            _ => None,
        };
        // The image array is a field of the struct; accept it at top level too.
        let array = field(pids.ui_map_map_image_array).or_else(|| {
            pids.ui_map_map_image_array
                .and_then(|p| self.properties.get(p))
        });
        let variants = array
            .and_then(PropertyValue::as_array)
            .map(|a| {
                a.elements
                    .iter()
                    .filter_map(|e| variant(e.value.as_struct()?, pids))
                    .collect()
            })
            .unwrap_or_default();
        Some(MinimapTile {
            image,
            block_offset: [
                i32_of(pids.ui_map_block_offset_x)?,
                i32_of(pids.ui_map_block_offset_y)?,
            ],
            pixel_offset: [
                i32_of(pids.ui_map_pixel_offset_x)?,
                i32_of(pids.ui_map_pixel_offset_y)?,
            ],
            scale,
            fog_of_war: int(pids.ui_map_fog_of_war),
            fog_colour,
            variants,
        })
    }

    /// The baked terrain light map (`Landblock_LightMap`, a 0x41 surface), if any.
    #[must_use]
    pub fn light_map(&self, pids: &LandblockPids) -> Option<Did> {
        self.properties
            .get(pids.landblock_light_map?)
            .and_then(as_did)
    }

    /// The baked ceiling light map (`Landblock_CeilingLightMap`), if any.
    #[must_use]
    pub fn ceiling_light_map(&self, pids: &LandblockPids) -> Option<Did> {
        self.properties
            .get(pids.landblock_ceiling_light_map?)
            .and_then(as_did)
    }
}

fn variant(s: &PropertyStruct, pids: &LandblockPids) -> Option<MinimapVariant> {
    let layer = s.get(pids.ui_map_content_layer?).and_then(as_i64)?;
    Some(MinimapVariant {
        // Content-layer keys are u32 values (1, or 0x100… keys); keep the bit pattern.
        content_layer: u32::try_from(layer).unwrap_or(layer as u32),
        image: s.get(pids.ui_map_map_image?).and_then(as_did)?,
    })
}

/// A non-null DID value.
fn as_did(v: &PropertyValue) -> Option<Did> {
    match v {
        PropertyValue::DidRef(d) if !d.is_null() => Some(*d),
        _ => None,
    }
}

/// An integer-like value.
fn as_i64(v: &PropertyValue) -> Option<i64> {
    Some(match v {
        PropertyValue::Int(i) => i64::from(*i),
        PropertyValue::Int64(i) => *i,
        PropertyValue::Enum(e) | PropertyValue::Bitfield32(e) => i64::from(*e),
        PropertyValue::DidRef(d) => i64::from(d.0),
        _ => return None,
    })
}

/// One kind-5 sample (§7; meaning OPEN): bytes 0–1 read as a u16 on the terrain height
/// scale, bytes 2–4 as three bytes near (127, 255, 127).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GroupSample {
    /// Bytes 0–1 as a little-endian u16.
    pub value: u16,
    /// Bytes 2–4.
    pub vector: [u8; 3],
}

/// The framing of a kind-5 record `0x805RXXYY` (§7): one per 3 × 3-landblock group. Only
/// the sample array is framed; the trailing section is kept raw (layout OPEN).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandblockGroupRecord {
    /// Own DID.
    pub did: Did,
    /// The samples, 5 bytes each, as stored.
    pub samples: Vec<u8>,
    /// The undecoded trailing section (starts with u32 0 in every observed record).
    pub tail: Vec<u8>,
}

impl LandblockGroupRecord {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "landblock group record";

    /// Number of samples.
    #[must_use]
    pub fn sample_count(&self) -> usize {
        self.samples.len() / GROUP_SAMPLE_SIZE
    }

    /// Sample `i`.
    #[must_use]
    pub fn sample(&self, i: usize) -> Option<GroupSample> {
        let at = i.checked_mul(GROUP_SAMPLE_SIZE)?;
        let end = at.checked_add(GROUP_SAMPLE_SIZE)?;
        let b = self.samples.get(at..end)?;
        Some(GroupSample {
            value: u16::from_le_bytes([b[0], b[1]]),
            vector: [b[2], b[3], b[4]],
        })
    }

    /// The first deviation from the observed framing (§7), or `None`.
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        if self.sample_count() != GROUP_SAMPLE_COUNT as usize {
            return Some("sample count is not 9,801");
        }
        if self.tail.get(..4) != Some(&[0, 0, 0, 0][..]) {
            return Some("trailing section does not start with u32 0");
        }
        if LandblockId::from_did(self.did).is_some_and(|l| l.x % 3 != 0 || l.y % 3 != 0) {
            return Some("landblock X or Y not a multiple of 3");
        }
        None
    }
}

impl Record<'_> for LandblockGroupRecord {
    const NAME: &'static str = LandblockGroupRecord::NAME;

    fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let did = read_landblock_did(&mut r, LandblockKind::Other(5), Self::NAME)?;
        let count = r.u32()?;
        r.ensure_elements(count, GROUP_SAMPLE_SIZE)?;
        let n = usize::try_from(count).unwrap_or(usize::MAX);
        let samples = r.bytes(n.saturating_mul(GROUP_SAMPLE_SIZE))?;
        let tail = r.rest();
        r.charge(samples.len().saturating_add(tail.len()))?;
        Ok(Self {
            did,
            samples: samples.to_vec(),
            tail: tail.to_vec(),
        })
    }

    fn did(&self) -> Did {
        self.did
    }
}
