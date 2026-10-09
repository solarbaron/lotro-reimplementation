// SPDX-License-Identifier: GPL-3.0-or-later
//! Distant-landscape sectors: kind-5 landblock records `0x805RXXYY` (DB type 0x41), decoded
//! to the last byte ([`SectorRecord`]).
//!
//! One record covers the 3 × 3 landblocks (X + x, Y + y), 0 ≤ x, y ≤ 2, whose south-west
//! corner (X, Y) has X and Y divisible by 3. It holds a terrain mesh block (nine 33 × 33
//! sample blocks with normals), an optional ceiling mesh block, the group's landscape water
//! pairs, distant billboard sprites, far-visible entity placements and six DIDs.
//!
//! # Orders
//!
//! - **Sample blocks** are in *vertex-block order*: block k = 3·x + y (X outer, Y inner);
//!   inside a block, sample (i, j) (i along X) is at 33·j + i (Y-major) (§4.1).
//! - **Chunk entries** are in *chunk order*: c = x + 3·y (X inner) (§3.4).
//! - The ceiling mesh packs the blocks of its ceiling landblocks by rank in vertex-block
//!   order (§4.4).
//!
//! # Hostile input (§3.11)
//!
//! Every count is checked against the remaining bytes before anything is allocated (samples
//! /5, indices /2, water pairs /5, sprites /23, entities /37), and every kept element is
//! charged against the [`Reader`] memory budget. The terrain sample count must be 9,801 and
//! the ceiling sample count 1,089 × the mask's bit count; every chunk's sample window must
//! be empty or one 1,089-sample block inside the sample array; a `wet` byte above 1, a
//! water index at or past the water list's end, a `mode` above 2 and a ceiling mask with bits
//! above bit 8 fail the record (the client ignores those bits; rejecting them is a Tessera
//! Decision, §3.11). Stored index bytes are skipped, never kept (§3.2). Only the newest
//! grammar is decoded (§3.10).
//!
//! Spec: `docs/specs/distant-landscape.md` §3, §4, §7.

use super::put;
use super::{
    GRID_SAMPLES, GRID_SIDE, InstanceId, LANDBLOCK_SIZE, SECTOR_SIDE, SectorWaterPair,
    StaticEntityType, height_from_raw, read_landblock_did,
};
use crate::did::{Did, LandblockId, LandblockKind};
use crate::error::{FormatError, Result};
use crate::property::{Position, PropertyEntry, PropertySchema, ValueDecoder};
use crate::reader::{Reader, write_vle};

/// Samples in the terrain mesh block: nine blocks of 1,089 (§3.2).
pub const SECTOR_TERRAIN_SAMPLES: u32 = 9_801;
/// Samples per landblock block (33 × 33).
pub const SECTOR_BLOCK_SAMPLES: u32 = GRID_SAMPLES as u32;
/// Chunk entries per mesh block (one per landblock of the sector).
pub const SECTOR_CHUNKS: usize = 9;
/// Levels of detail per chunk entry.
pub const SECTOR_LODS: usize = 4;
/// Effective ceiling mask of `mode` 1: all nine landblocks (§3.5).
pub const ALL_LANDBLOCKS_MASK: u32 = 0x1FF;
/// Edge length of the sector square in units (3 × 160).
pub const SECTOR_EXTENT: f32 = 3.0 * LANDBLOCK_SIZE;
/// Stored size of one sample (§3.3).
pub const SECTOR_SAMPLE_SIZE: usize = 5;
/// Stored size of one chunk entry (§3.4).
pub const SECTOR_CHUNK_SIZE: usize = 66;
/// Stored size of one water pair (§3.6).
pub const SECTOR_WATER_PAIR_SIZE: usize = 5;
/// Stored size of one sprite entry (§3.7).
pub const SECTOR_SPRITE_SIZE: usize = 23;
/// Smallest stored entity entry (§3.8): Position flag byte 0 and a null property.
pub const SECTOR_MIN_ENTITY_SIZE: usize = 37;
/// One chunk's share of the generated index list: 3 × (2,048 + 512 + 128 + 32) (§4.5).
pub const CHUNK_INDEX_SHARE: u32 = 8_160;
/// Triangles per chunk at LOD 0…3: (32 / 2^l)² × 2 (§4.5).
pub const LOD_TRIANGLES: [u32; SECTOR_LODS] = [2_048, 512, 128, 32];
/// First index of LOD 0…3 inside a chunk's share of the generated index list (§4.5).
pub const LOD_INDEX_OFFSETS: [u32; SECTOR_LODS] = [0, 6_144, 7_680, 8_064];
/// Smallest `view_distance_sq` observed on a far entity: 250² (DL-I12).
pub const MIN_FAR_VIEW_DISTANCE_SQ: f32 = 62_500.0;
/// The f32 the client multiplies a normal byte by before subtracting 1.0: 2 / 255 (§4.3).
pub const NORMAL_BYTE_SCALE: f32 = 2.0 / 255.0;
/// Side of the square of landblocks one imposter table covers (`misc-records.md` §5.1;
/// [`crate::imposter::IMPOSTER_TABLE_SQUARE`]).
pub const IMPOSTER_SQUARE: u8 = crate::imposter::IMPOSTER_TABLE_SQUARE;
/// DID prefix of imposter tables (`misc-records.md` §5.1;
/// [`crate::imposter::IMPOSTER_TABLE_PREFIX`]).
pub const IMPOSTER_TABLE_PREFIX: u8 = crate::imposter::IMPOSTER_TABLE_PREFIX;

const SIDE: usize = SECTOR_SIDE as usize;

/// Vertex-block index k = 3·x + y of landblock (x, y) of the sector (§4.1).
#[must_use]
pub fn vertex_block(x: usize, y: usize) -> usize {
    SIDE * x + y
}

/// Chunk index c = x + 3·y of landblock (x, y) of the sector (§3.4).
#[must_use]
pub fn chunk_index(x: usize, y: usize) -> usize {
    x + SIDE * y
}

/// Landblock offsets (x, y) of chunk `c` (c = x + 3·y).
#[must_use]
pub fn chunk_landblock(c: usize) -> (usize, usize) {
    (c % SIDE, c / SIDE)
}

/// Offset of sample (i, j) (i along X) inside a 33 × 33 block: 33·j + i (Y-major, §4.1).
/// `None` outside 0–32.
#[must_use]
pub fn block_sample_offset(i: usize, j: usize) -> Option<usize> {
    (i < GRID_SIDE && j < GRID_SIDE).then_some(GRID_SIDE * j + i)
}

/// DID of the imposter table of the 27 × 27 square holding landblock (x, y) of region
/// `region` (`misc-records.md` §5.1; distant-landscape §3.9).
#[must_use]
pub fn imposter_table_did(region: u8, x: u8, y: u8) -> Did {
    crate::imposter::ImposterSquare::containing(region, x, y).did()
}

/// One stored sample (§3.3): a height on the landblock height scale and a biased normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SectorSample {
    /// Stored height (65,535 = 2,000 units, `landblock.md` §3.4).
    pub height: u16,
    /// Normal bytes as stored: world X, world Z (up), world Y.
    pub normal: [u8; 3],
}

impl SectorSample {
    /// Height in world units, converted bit-exactly as `landblock.md` §3.4 (§4.2).
    #[must_use]
    pub fn height_units(self) -> f32 {
        height_from_raw(self.height)
    }

    /// The normal in world (x, y, z) order, z up, each byte decoded as b × 2/255 − 1 in f32
    /// (§4.3). Not renormalised, as in the original.
    #[must_use]
    pub fn normal_xyz(self) -> [f32; 3] {
        let d = |b: u8| f32::from(b) * NORMAL_BYTE_SCALE - 1.0;
        let [nx, nz, ny] = self.normal;
        [d(nx), d(ny), d(nz)]
    }
}

/// One (first index, triangle count) range of a chunk entry (§3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LodRange {
    /// First index into the mesh's index list.
    pub first_index: u32,
    /// Number of triangles.
    pub triangles: u32,
}

/// One 66-byte chunk entry (§3.4): the part of a mesh block for one landblock.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SectorChunk {
    /// Index of the chunk's first sample in the mesh block's sample array.
    pub vertex_start: u32,
    /// 1,089, or 0 for an absent ceiling chunk.
    pub vertex_count: u32,
    /// Sector-local (x, y, z) of the south-west bottom corner.
    pub min: [f32; 3],
    /// Sector-local (x, y, z) of the north-east top corner.
    pub max: [f32; 3],
    /// The landblock has landscape water.
    pub wet: bool,
    /// Index into the water list when `wet`; 0 otherwise in observed data.
    pub water_index: u8,
    /// LOD 0…3 ranges into the mesh's (generated) index list.
    pub lods: [LodRange; SECTOR_LODS],
}

impl SectorChunk {
    /// Whether the chunk has samples (`vertex_count` ≠ 0).
    #[must_use]
    pub fn is_present(&self) -> bool {
        self.vertex_count != 0
    }

    /// Whether every stored field is zero (an absent ceiling chunk, §4.4). Floats compare by
    /// bit pattern, so −0.0 is not zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        *self == Self::default() && self.min.iter().chain(&self.max).all(|v| v.to_bits() == 0)
    }

    /// The entry the spec's rules (§4.5) give a present chunk of landblock (x, y) with
    /// packed rank `rank` (3·x + y in the terrain mesh) and converted height range
    /// `min_z`…`max_z`; dry.
    #[must_use]
    pub fn expected(rank: u32, x: usize, y: usize, min_z: f32, max_z: f32) -> Self {
        let fx = LANDBLOCK_SIZE * x as f32;
        let fy = LANDBLOCK_SIZE * y as f32;
        let mut lods = [LodRange::default(); SECTOR_LODS];
        for (l, lod) in lods.iter_mut().enumerate() {
            *lod = LodRange {
                first_index: CHUNK_INDEX_SHARE * rank + LOD_INDEX_OFFSETS[l],
                triangles: LOD_TRIANGLES[l],
            };
        }
        Self {
            vertex_start: SECTOR_BLOCK_SAMPLES * rank,
            vertex_count: SECTOR_BLOCK_SAMPLES,
            min: [fx, fy, min_z],
            max: [fx + LANDBLOCK_SIZE, fy + LANDBLOCK_SIZE, max_z],
            wet: false,
            water_index: 0,
            lods,
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        let vertex_start = r.u32()?;
        let vertex_count = r.u32()?;
        let min = [r.f32()?, r.f32()?, r.f32()?];
        let max = [r.f32()?, r.f32()?, r.f32()?];
        let wet = r.bool8()?;
        let water_index = r.u8()?;
        let mut lods = [LodRange::default(); SECTOR_LODS];
        for lod in &mut lods {
            *lod = LodRange {
                first_index: r.u32()?,
                triangles: r.u32()?,
            };
        }
        Ok(Self {
            vertex_start,
            vertex_count,
            min,
            max,
            wet,
            water_index,
            lods,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        put::u32(out, self.vertex_start);
        put::u32(out, self.vertex_count);
        put::f32s(out, &self.min);
        put::f32s(out, &self.max);
        out.push(u8::from(self.wet));
        out.push(self.water_index);
        for lod in &self.lods {
            put::u32(out, lod.first_index);
            put::u32(out, lod.triangles);
        }
    }
}

/// A mesh block (§3.2): samples, the stored index count, nine chunk entries and the bounds.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SectorMesh {
    /// The sample array (terrain: nine blocks in vertex-block order; ceiling: the ceiling
    /// landblocks' blocks packed by rank).
    pub samples: Vec<SectorSample>,
    /// The stored index count *m* (0 in every observed record). The *m* × 2 index bytes
    /// are skipped and never kept: a reader always uses the generated index list of §5.3
    /// (Tessera rule of §3.2, the client's ordinary loading path).
    pub stored_index_count: u32,
    /// Chunk entries in chunk order c = x + 3·y.
    pub chunks: [SectorChunk; SECTOR_CHUNKS],
    /// Minimum (x, y, z) over the present chunks.
    pub bounds_min: [f32; 3],
    /// Maximum (x, y, z) over the present chunks.
    pub bounds_max: [f32; 3],
}

impl SectorMesh {
    /// The chunk entry of landblock (x, y) of the sector (0 ≤ x, y ≤ 2).
    #[must_use]
    pub fn chunk(&self, x: usize, y: usize) -> Option<&SectorChunk> {
        if x >= SIDE || y >= SIDE {
            return None;
        }
        self.chunks.get(chunk_index(x, y))
    }

    /// The samples a chunk entry names, or `None` for an absent chunk. A decoded record's
    /// windows always lie inside the sample array.
    #[must_use]
    pub fn chunk_samples(&self, chunk: &SectorChunk) -> Option<&[SectorSample]> {
        if !chunk.is_present() {
            return None;
        }
        let start = usize::try_from(chunk.vertex_start).ok()?;
        let end = start.checked_add(usize::try_from(chunk.vertex_count).ok()?)?;
        self.samples.get(start..end)
    }

    /// The 1,089 samples of landblock (x, y), Y-major ([`block_sample_offset`]), through its
    /// chunk entry; `None` for an absent chunk.
    #[must_use]
    pub fn block(&self, x: usize, y: usize) -> Option<&[SectorSample]> {
        self.chunk_samples(self.chunk(x, y)?)
    }

    /// Sample (i, j) (i along X) of landblock (x, y).
    #[must_use]
    pub fn sample(&self, x: usize, y: usize, i: usize, j: usize) -> Option<SectorSample> {
        self.block(x, y)?.get(block_sample_offset(i, j)?).copied()
    }

    /// Lowest and highest stored height of a sample slice, or `None` when it is empty.
    #[must_use]
    pub fn raw_height_range(samples: &[SectorSample]) -> Option<(u16, u16)> {
        let mut it = samples.iter().map(|s| s.height);
        let first = it.next()?;
        Some(it.fold((first, first), |(lo, hi), h| (lo.min(h), hi.max(h))))
    }

    fn read(r: &mut Reader<'_>, expected_samples: u32, what: &'static str) -> Result<Self> {
        let count_at = r.position();
        let n = r.u32()?;
        r.ensure_elements(n, SECTOR_SAMPLE_SIZE)?;
        if n != expected_samples {
            return Err(FormatError::CountMismatch {
                what,
                offset: count_at,
                expected: expected_samples,
                found: n,
            });
        }
        let mut samples = r.alloc_vec(n, SECTOR_SAMPLE_SIZE, usize::MAX)?;
        for _ in 0..n {
            let height = r.u16()?;
            let normal = r.array::<3>()?;
            r.push(&mut samples, SectorSample { height, normal }, n)?;
        }
        let m = r.u32()?;
        r.ensure_elements(m, 2)?;
        r.skip(usize::try_from(m).unwrap_or(usize::MAX).saturating_mul(2))?;
        let mut chunks = [SectorChunk::default(); SECTOR_CHUNKS];
        for chunk in &mut chunks {
            let at = r.position();
            *chunk = SectorChunk::read(r)?;
            check_window(chunk, n, at)?;
        }
        let bounds_min = [r.f32()?, r.f32()?, r.f32()?];
        let bounds_max = [r.f32()?, r.f32()?, r.f32()?];
        Ok(Self {
            samples,
            stored_index_count: m,
            chunks,
            bounds_min,
            bounds_max,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        put::u32(out, put::len_u32(self.samples.len()));
        for s in &self.samples {
            put::u16(out, s.height);
            out.extend_from_slice(&s.normal);
        }
        // The stored index bytes are not kept (§3.2); the encoder writes zeros for them.
        put::u32(out, self.stored_index_count);
        for _ in 0..self.stored_index_count {
            put::u16(out, 0);
        }
        for c in &self.chunks {
            c.write(out);
        }
        put::f32s(out, &self.bounds_min);
        put::f32s(out, &self.bounds_max);
    }
}

/// §3.11: a chunk is usable only when its window is empty or one block inside the samples.
fn check_window(chunk: &SectorChunk, samples: u32, at: usize) -> Result<()> {
    if chunk.vertex_count != 0 && chunk.vertex_count != SECTOR_BLOCK_SAMPLES {
        return Err(FormatError::malformed(
            "sector chunk entry",
            at + 4,
            format!("vertex count {} is neither 0 nor 1,089", chunk.vertex_count),
        ));
    }
    let end = u64::from(chunk.vertex_start) + u64::from(chunk.vertex_count);
    if end > u64::from(samples) {
        return Err(FormatError::IndexOutOfRange {
            what: "sector chunk sample window end",
            offset: at,
            index: end,
            limit: u64::from(samples),
        });
    }
    Ok(())
}

/// One 23-byte sprite entry (§3.7): a distant billboard drawn from the imposter atlases.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SectorSprite {
    /// Imposter name key, looked up in the sector's imposter table.
    pub name_key: u16,
    /// Sector-local x (east), units.
    pub east: f32,
    /// Absolute height, units.
    pub height: f32,
    /// Sector-local y (north), units.
    pub north: f32,
    /// Size factor.
    pub scale: f32,
    /// Fade distance, units (§4.8): the sprite's fade parameter when it lies near the
    /// viewer, replaced by a computed value otherwise ([`sprite_fade_parameter`], §5.6).
    pub distance: f32,
    /// Heading, `byte` × 360/256 degrees ([`SectorSprite::heading_degrees`], §4.8); used
    /// only for the mirror decision of §5.6 ([`sprite_is_mirrored`]).
    pub byte: u8,
}

impl SectorSprite {
    /// Sector-frame position (x east, y north, z absolute height).
    #[must_use]
    pub fn position(&self) -> [f32; 3] {
        [self.east, self.north, self.height]
    }

    /// The heading θ in degrees, 0 ≤ θ < 360 (§4.8): (`byte` × 1/256 × 360) modulo 360,
    /// in 32-bit floats as the client computes it.
    #[must_use]
    pub fn heading_degrees(&self) -> f32 {
        (f32::from(self.byte) * (1.0 / 256.0) * 360.0) % 360.0
    }

    /// The billboard's size *S* = `value_a` × `scale` (width and height of the quad) and
    /// lower-edge height *B* = `value_b` × `scale` above the sprite position, from the
    /// imposter entry's two stored floats (§4.8, `misc-records.md` §5.2).
    #[must_use]
    pub fn billboard_size(&self, value_a: f32, value_b: f32) -> (f32, f32) {
        (value_a * self.scale, value_b * self.scale)
    }

    /// The sprite's horizontal position (*x*, *y*) relative to the south-west corner of the
    /// viewer's landblock (§5.6 step 2): sector-local `east`, `north` plus 160 × (sector
    /// corner landblock − viewer landblock) on each axis. `sector_origin` and `viewer` are
    /// landblock (X, Y) of the same region.
    #[must_use]
    pub fn viewer_frame(&self, sector_origin: (u8, u8), viewer: (u8, u8)) -> (f32, f32) {
        let off = |s: u8, v: u8| LANDBLOCK_SIZE * (f32::from(s) - f32::from(v));
        (
            self.east + off(sector_origin.0, viewer.0),
            self.north + off(sector_origin.1, viewer.1),
        )
    }

    fn read(r: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            name_key: r.u16()?,
            east: r.f32()?,
            height: r.f32()?,
            north: r.f32()?,
            scale: r.f32()?,
            distance: r.f32()?,
            byte: r.u8()?,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        put::u16(out, self.name_key);
        put::f32s(
            out,
            &[
                self.east,
                self.height,
                self.north,
                self.scale,
                self.distance,
            ],
        );
        out.push(self.byte);
    }
}

/// Whether a sprite at viewer-frame position (*x*, *y*) (§5.6 step 2) lies on the viewer's
/// landblock or one of its eight neighbours: −160 ≤ *x* ≤ 320 and −160 ≤ *y* ≤ 320.
#[must_use]
pub fn sprite_is_near(x: f32, y: f32) -> bool {
    let r = -LANDBLOCK_SIZE..=2.0 * LANDBLOCK_SIZE;
    r.contains(&x) && r.contains(&y)
}

/// The sprite's fade parameter *P* (§5.6 step 3, DL25): the stored `distance` for a sprite
/// near the viewer ([`sprite_is_near`]), otherwise *S* × *K* / *M_d* with *S* the billboard
/// size ([`SectorSprite::billboard_size`]), *K* = (*H* / *φ*) / 248 for a render target
/// *H* pixels high and a field of view of *φ* radians, and *M_d* the object draw-distance
/// multiplier. Identifying *H* and *φ* with those quantities is Inferred in the spec.
#[must_use]
pub fn sprite_fade_parameter(
    sprite: &SectorSprite,
    (x, y): (f32, f32),
    size: f32,
    target_height_px: f32,
    fov_radians: f32,
    draw_distance_multiplier: f32,
) -> f32 {
    if sprite_is_near(x, y) {
        return sprite.distance;
    }
    let k = (target_height_px / fov_radians) / 248.0;
    size * k / draw_distance_multiplier
}

/// Whether the billboard is drawn mirrored (§5.6 step 4, DL26): exactly when
/// (*x* − 160) × sin θ + (*y* − 160) × cos θ > 0, with (*x*, *y*) the viewer-frame position
/// and θ the sprite's heading ([`SectorSprite::heading_degrees`]) in radians.
#[must_use]
pub fn sprite_is_mirrored(sprite: &SectorSprite, (x, y): (f32, f32)) -> bool {
    let theta = sprite.heading_degrees().to_radians();
    (x - LANDBLOCK_SIZE) * theta.sin() + (y - LANDBLOCK_SIZE) * theta.cos() > 0.0
}

/// The near radius *N* of §5.2 in landblocks: the viewer's landblock and its eight
/// neighbours (DL29).
pub const SECTOR_NEAR_RADIUS: u32 = 1;

/// The sector radius *F* of §5.1 for a landscape far radius of `landscape_radius`
/// landblocks (the landscape draw-distance option; 2, 4, 6, 9 or 12 for levels 0–4):
/// ⌊*R* / 3⌋ + 1 (DL29). *R* is a user setting and comes from Tessera's own settings.
#[must_use]
pub fn sector_radius(landscape_radius: u32) -> u32 {
    landscape_radius / 3 + 1
}

/// The far placements registered from sector entity lists (§4.9, DL28): one per instance
/// ID; an entry whose instance ID is already known is skipped, so a sector loaded again,
/// or an entry repeated in two records, adds nothing. When the placements are turned into
/// entities, drawn and removed is OPEN (§10).
#[derive(Debug, Clone, Default)]
pub struct FarPlacements {
    by_iid: std::collections::HashMap<u64, SectorEntity>,
}

impl FarPlacements {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers every entity of `record` whose instance ID is not yet known; returns how
    /// many were added.
    pub fn register(&mut self, record: &SectorRecord) -> usize {
        let mut added = 0;
        for e in &record.entities {
            if let std::collections::hash_map::Entry::Vacant(v) = self.by_iid.entry(e.iid.0) {
                v.insert(e.clone());
                added += 1;
            }
        }
        added
    }

    /// The placement registered under `iid`.
    #[must_use]
    pub fn get(&self, iid: u64) -> Option<&SectorEntity> {
        self.by_iid.get(&iid)
    }

    /// Number of registered placements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_iid.len()
    }

    /// Whether nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_iid.is_empty()
    }
}

/// One entity entry (§3.8): a far-visible placement of one of the sector's landblocks.
#[derive(Debug, Clone, PartialEq)]
pub struct SectorEntity {
    /// Entity type (`entity-descriptor.md` §4.1), as stored.
    pub entity_type: u32,
    /// A 0x47 entity descriptor or a 0x70 object.
    pub object: Did,
    /// Instance ID (`entity-descriptor.md` §4.2).
    pub iid: InstanceId,
    /// Scale per axis.
    pub scale: [f32; 3],
    /// Stored Position (`property-system.md` §4.6).
    pub position: Position,
    /// The list-form property, `None` for a null (PID 0) entry.
    pub property: Option<PropertyEntry>,
    /// Square of the view distance in units.
    pub view_distance_sq: f32,
}

impl SectorEntity {
    /// The entity type as an enumeration.
    #[must_use]
    pub fn kind(&self) -> StaticEntityType {
        StaticEntityType::from_code(self.entity_type)
    }

    /// The view distance, √`view_distance_sq` (NaN for a negative value).
    #[must_use]
    pub fn view_distance(&self) -> f32 {
        self.view_distance_sq.sqrt()
    }

    fn read<S: PropertySchema + ?Sized>(r: &mut Reader<'_>, schema: &S) -> Result<Self> {
        let entity_type = r.u32()?;
        let object = r.did()?;
        let iid = InstanceId(r.u64()?);
        let scale = [r.f32()?, r.f32()?, r.f32()?];
        let position = Position::read(r)?;
        let property = ValueDecoder::new(schema).entry(r, false, 0)?;
        let view_distance_sq = r.f32()?;
        Ok(Self {
            entity_type,
            object,
            iid,
            scale,
            position,
            property,
            view_distance_sq,
        })
    }

    fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        put::u32(out, self.entity_type);
        put::u32(out, self.object.0);
        out.extend_from_slice(&self.iid.0.to_le_bytes());
        put::f32s(out, &self.scale);
        write_position(out, &self.position);
        if let Some(p) = &self.property {
            return Err(FormatError::Unsupported {
                what: "sector entity property (the encoder writes null properties only)",
                offset: out.len(),
                value: p.pid,
            });
        }
        put::u32(out, 0);
        put::f32s(out, &[self.view_distance_sq]);
        Ok(())
    }
}

/// Writes a Position in its stored form (flag byte, then the fields its payload bits select).
fn write_position(out: &mut Vec<u8>, p: &Position) {
    out.push(p.flags());
    if let Some(region) = p.region() {
        out.push(region);
    }
    if let Some((x, y)) = p.landblock() {
        out.extend_from_slice(&[x, y]);
    }
    if let Some(i) = p.instance() {
        put::u16(out, i);
    }
    if let Some(c) = p.cell() {
        put::u16(out, c);
    }
    if let Some(o) = p.offset() {
        put::f32s(out, &o);
    }
    if let Some(q) = p.orientation() {
        put::f32s(out, &q);
    }
}

/// A distant-landscape sector record `0x805RXXYY` decoded to the last byte (§3.1).
#[derive(Debug, Clone, PartialEq)]
pub struct SectorRecord {
    /// Own DID.
    pub did: Did,
    /// Terrain mesh block (9,801 samples).
    pub terrain: SectorMesh,
    /// 0 = no ceiling, 1 = ceiling in all nine landblocks, 2 = ceiling where
    /// [`SectorRecord::ceiling_mask`] says.
    pub mode: u8,
    /// Effective ceiling mask: bit x + 3·y = landblock (X + x, Y + y) has a ceiling (0 for
    /// `mode` 0, 0x1FF for `mode` 1, the stored mask for `mode` 2).
    pub ceiling_mask: u32,
    /// Ceiling mesh block, present exactly when `mode` ≠ 0.
    pub ceiling: Option<SectorMesh>,
    /// Landblock edge length (160.0 in every observed record).
    pub block_size: f32,
    /// Distinct (water type, height) pairs, in first-use order over vertex-block order.
    pub water: Vec<SectorWaterPair>,
    /// Distant billboard sprites.
    pub sprites: Vec<SectorSprite>,
    /// Far-visible entity placements.
    pub entities: Vec<SectorEntity>,
    /// Terrain colour texture list (0x40).
    pub colour_list: Did,
    /// Ceiling colour texture list (0 without ceiling).
    pub ceiling_colour_list: Did,
    /// Terrain tile surface (0x41, 96 × 96).
    pub tile: Did,
    /// Ceiling tile surface (0 without ceiling).
    pub ceiling_tile: Did,
    /// Distant-water flow map texture list, or 0.
    pub flow_map: Did,
    /// The imposter table (0x27) of the 27 × 27 square holding the sector.
    pub imposter_table: Did,
}

impl SectorRecord {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "distant-landscape sector";

    /// Decodes a complete payload. `schema` supplies the types of entity properties
    /// (`property-system.md` §5.3); an unknown PID fails the record.
    pub fn parse<S: PropertySchema + ?Sized>(data: &[u8], schema: &S) -> Result<Self> {
        let mut r = Reader::new(data);
        let rec = Self::read(&mut r, schema)?;
        r.expect_end(Self::NAME)?;
        tracing::trace!(
            did = %rec.did,
            mode = rec.mode,
            water = rec.water.len(),
            sprites = rec.sprites.len(),
            entities = rec.entities.len(),
            "distant-landscape sector decoded"
        );
        Ok(rec)
    }

    /// [`SectorRecord::parse`] plus a check that the stored DID is `expected`.
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

    fn read<S: PropertySchema + ?Sized>(r: &mut Reader<'_>, schema: &S) -> Result<Self> {
        let did = read_landblock_did(r, LandblockKind::Other(5), Self::NAME)?;
        let terrain = SectorMesh::read(r, SECTOR_TERRAIN_SAMPLES, "sector terrain sample count")?;
        let mode_at = r.position();
        let mode = r.u8()?;
        let ceiling_mask = match mode {
            0 => 0,
            1 => ALL_LANDBLOCKS_MASK,
            2 => {
                let at = r.position();
                let mask = r.u32()?;
                // Bits 9–31: the client ignores them (§3.5, DL24); Tessera rejects them
                // (Tessera Decision of §3.11, DL-I15).
                if mask & !ALL_LANDBLOCKS_MASK != 0 {
                    return Err(FormatError::malformed(
                        "sector ceiling mask",
                        at,
                        format!("mask {mask:#x} has bits above bit 8"),
                    ));
                }
                mask
            }
            m => {
                return Err(FormatError::malformed(
                    "sector mode",
                    mode_at,
                    format!("mode {m} is not 0, 1 or 2"),
                ));
            }
        };
        let ceiling = if mode == 0 {
            None
        } else {
            Some(SectorMesh::read(
                r,
                SECTOR_BLOCK_SAMPLES * ceiling_mask.count_ones(),
                "sector ceiling sample count",
            )?)
        };
        let block_size = r.f32()?;
        let water_at = r.position();
        let w = r.vle()?;
        let mut water = r.alloc_vec(w, SECTOR_WATER_PAIR_SIZE, usize::MAX)?;
        for _ in 0..w {
            let pair = SectorWaterPair {
                water_type: r.u8()?,
                height: r.f32()?,
            };
            r.push(&mut water, pair, w)?;
        }
        for mesh in std::iter::once(&terrain).chain(&ceiling) {
            check_water_indices(mesh, water.len(), water_at)?;
        }
        let s = r.u32()?;
        let mut sprites = r.alloc_vec(s, SECTOR_SPRITE_SIZE, usize::MAX)?;
        for _ in 0..s {
            let sprite = SectorSprite::read(r)?;
            r.push(&mut sprites, sprite, s)?;
        }
        let e = r.u32()?;
        let mut entities = r.alloc_vec(e, SECTOR_MIN_ENTITY_SIZE, usize::MAX)?;
        for _ in 0..e {
            let entity = SectorEntity::read(r, schema)?;
            r.push(&mut entities, entity, e)?;
        }
        Ok(Self {
            did,
            terrain,
            mode,
            ceiling_mask,
            ceiling,
            block_size,
            water,
            sprites,
            entities,
            colour_list: r.did()?,
            ceiling_colour_list: r.did()?,
            tile: r.did()?,
            ceiling_tile: r.did()?,
            flow_map: r.did()?,
            imposter_table: r.did()?,
        })
    }

    /// Re-encodes the record (§3.1), for synthetic fixtures and fuzzing (the format needs
    /// no encoder; Tessera Decision of §3.11). Byte-exact for a decoded record whose
    /// entities all have a null property and whose stored index lists are empty (skipped
    /// index bytes come back as zeros); an entity with a property fails with
    /// [`FormatError::Unsupported`] (this crate has no property-value encoder; records with
    /// null properties are valid). The water count is written as a shortest VLE and the
    /// mask only for `mode` 2.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        put::u32(&mut out, self.did.0);
        self.terrain.write(&mut out);
        out.push(self.mode);
        if self.mode == 2 {
            put::u32(&mut out, self.ceiling_mask);
        }
        if let Some(c) = &self.ceiling {
            c.write(&mut out);
        }
        put::f32s(&mut out, &[self.block_size]);
        write_vle(&mut out, put::len_u32(self.water.len()));
        for p in &self.water {
            out.extend_from_slice(&p.to_bytes());
        }
        put::u32(&mut out, put::len_u32(self.sprites.len()));
        for s in &self.sprites {
            s.write(&mut out);
        }
        put::u32(&mut out, put::len_u32(self.entities.len()));
        for e in &self.entities {
            e.write(&mut out)?;
        }
        for d in [
            self.colour_list,
            self.ceiling_colour_list,
            self.tile,
            self.ceiling_tile,
            self.flow_map,
            self.imposter_table,
        ] {
            put::u32(&mut out, d.0);
        }
        Ok(out)
    }

    /// The sector's landblock-family ID (kind 5, region, X, Y of the south-west corner).
    #[must_use]
    pub fn id(&self) -> Option<LandblockId> {
        LandblockId::from_did(self.did)
    }

    /// Region of the sector.
    #[must_use]
    pub fn region(&self) -> Option<u8> {
        self.id().map(|l| l.region)
    }

    /// (X, Y) of the south-west corner landblock.
    #[must_use]
    pub fn origin(&self) -> Option<(u8, u8)> {
        self.id().map(|l| (l.x, l.y))
    }

    /// Sector key (X / 3, Y / 3) (§2.1).
    #[must_use]
    pub fn key(&self) -> Option<(u8, u8)> {
        self.origin()
            .map(|(x, y)| (x / SECTOR_SIDE, y / SECTOR_SIDE))
    }

    /// Whether landblock (x, y) of the sector has a ceiling (its mask bit, §3.5).
    #[must_use]
    pub fn has_ceiling(&self, x: usize, y: usize) -> bool {
        x < SIDE && y < SIDE && self.ceiling_mask & (1 << chunk_index(x, y)) != 0
    }

    /// The water pair of landblock (x, y): from its terrain chunk's `wet` and `water_index`
    /// and the water list (§4.6). `None` when dry.
    #[must_use]
    pub fn water_at(&self, x: usize, y: usize) -> Option<SectorWaterPair> {
        let c = self.terrain.chunk(x, y)?;
        if !c.wet {
            return None;
        }
        self.water.get(usize::from(c.water_index)).copied()
    }

    /// Every record-local invariant of §7 (DL-I2–I5, the local parts of DL-I7, I8, I10, I12,
    /// I13, and DL-I9, I11) that this record breaks, as short descriptions. Empty for a
    /// record that follows all observed rules. The cross-record parts (heights against the
    /// landblock data, mask against `has_ceiling`, water against the block maps, named DIDs
    /// existing) need other records; see the real-data census.
    #[must_use]
    pub fn deviations(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        let mut flag = |cond: bool, why: &'static str| {
            if cond {
                out.push(why);
            }
        };
        let id = self.id();
        flag(
            !id.is_some_and(|l| l.x % 3 == 0 && l.y % 3 == 0 && l.x < 255 && l.y < 255),
            "DL-I2: X or Y not a multiple of 3 below 255",
        );
        flag(
            self.terrain.stored_index_count != 0,
            "DL-I3: terrain index count not 0",
        );
        flag(
            self.ceiling
                .as_ref()
                .is_some_and(|c| c.stored_index_count != 0),
            "DL-I3: ceiling index count not 0",
        );
        let (chunks_ok, heights_ok, bounds_ok) = mesh_rules(&self.terrain, ALL_LANDBLOCKS_MASK);
        flag(!chunks_ok, "DL-I4: terrain chunk fields differ from §4.5");
        flag(
            !heights_ok,
            "DL-I5: terrain chunk z range differs from its samples",
        );
        flag(
            !bounds_ok,
            "DL-I5: terrain bounds not the union of its chunks",
        );
        flag(
            (self.mode == 1) != (self.ceiling_mask == ALL_LANDBLOCKS_MASK),
            "DL-I7: mode 1 not exactly when the mask is 0x1FF",
        );
        flag(
            self.mode == 2 && self.ceiling_mask == 0,
            "DL-I7: mode 2 with an empty mask",
        );
        if let Some(c) = &self.ceiling {
            let (chunks_ok, heights_ok, bounds_ok) = mesh_rules(c, self.ceiling_mask);
            flag(
                !chunks_ok,
                "DL-I8: ceiling chunks not packed by rank, or absent chunks not zero",
            );
            flag(
                !heights_ok,
                "DL-I5: ceiling chunk z range differs from its samples",
            );
            flag(
                !bounds_ok,
                "DL-I5: ceiling bounds not the union of its chunks",
            );
            flag(
                c.chunks.iter().any(|ch| ch.wet || ch.water_index != 0),
                "DL-I10: a ceiling chunk is wet",
            );
        }
        flag(
            self.block_size.to_bits() != LANDBLOCK_SIZE.to_bits(),
            "DL-I9: block_size not 160.0",
        );
        flag(
            !self.water_list_follows_rules(),
            "DL-I10: water pairs not distinct, unused or not in first-use order",
        );
        flag(
            self.terrain
                .chunks
                .iter()
                .any(|c| !c.wet && c.water_index != 0),
            "DL-I10: a dry chunk has a water index",
        );
        flag(
            self.sprites.iter().any(|s| {
                !((0.0..=SECTOR_EXTENT).contains(&s.east)
                    && (0.0..=SECTOR_EXTENT).contains(&s.north)
                    && s.scale > 0.0)
            }),
            "DL-I11: a sprite outside the sector square or with scale ≤ 0",
        );
        flag(
            self.entities.iter().any(|e| !self.entity_follows_rules(e)),
            "DL-I12: an entity outside the sector's landblocks, not outdoors, with unequal \
             scales or view distance below 250",
        );
        flag(
            id.is_none_or(|l| self.imposter_table != imposter_table_did(l.region, l.x, l.y)),
            "DL-I13: imposter table is not the one of the sector's 27 × 27 square",
        );
        flag(
            self.colour_list.prefix() != 0x40 || self.tile.prefix() != 0x41,
            "DL-I13: colour list or tile missing or of the wrong type",
        );
        let ceiling_dids = !self.ceiling_colour_list.is_null() || !self.ceiling_tile.is_null();
        let ceiling_dids_all = !self.ceiling_colour_list.is_null() && !self.ceiling_tile.is_null();
        flag(
            if self.mode == 0 {
                ceiling_dids
            } else {
                !ceiling_dids_all
            },
            "DL-I13: ceiling colour list and tile not non-zero exactly when mode ≠ 0",
        );
        flag(
            !self.flow_map.is_null() && self.flow_map.prefix() != 0x40,
            "§3.9: flow map neither 0 nor a texture list",
        );
        out
    }

    /// The first entry of [`SectorRecord::deviations`].
    #[must_use]
    pub fn first_unusual_field(&self) -> Option<&'static str> {
        self.deviations().first().copied()
    }

    /// DL-I14: compares every stored terrain and ceiling normal with the rule of §4.3
    /// ([`super::sector_normals`]). `landblock(x, y)` returns the region's landblock-data
    /// record at absolute landblock (x, y) (any landblock of the region, also outside the
    /// sector; `None` beyond the region border or 0–254).
    pub fn check_normals<'a>(
        &self,
        landblock: impl Fn(i32, i32) -> Option<&'a super::LandblockData>,
    ) -> super::sector_normals::SectorNormalCheck {
        use super::sector_normals::{
            NeighbourWindow, NormalSurface, SectorNormalCheck, rule_normal_bytes,
        };
        let mut out = SectorNormalCheck::default();
        let Some((x0, y0)) = self.origin() else {
            return out;
        };
        for x in 0..SIDE {
            for y in 0..SIDE {
                let (lx, ly) = (i32::from(x0) + x as i32, i32::from(y0) + y as i32);
                let mut w = NeighbourWindow::default();
                for (dy, row) in (-1..=1).zip(w.0.iter_mut()) {
                    for (dx, cell) in (-1..=1).zip(row.iter_mut()) {
                        *cell = landblock(lx + dx, ly + dy);
                    }
                }
                let meshes = [
                    (Some(&self.terrain), NormalSurface::Terrain),
                    (
                        self.ceiling.as_ref().filter(|_| self.has_ceiling(x, y)),
                        NormalSurface::Ceiling,
                    ),
                ];
                for (mesh, surface) in meshes {
                    let Some(block) = mesh.and_then(|m| m.block(x, y)) else {
                        continue;
                    };
                    for i in 0..GRID_SIDE {
                        for j in 0..GRID_SIDE {
                            let Some(stored) = block_sample_offset(i, j).and_then(|k| block.get(k))
                            else {
                                continue;
                            };
                            let rule = rule_normal_bytes(&w, surface, i, j);
                            match surface {
                                NormalSurface::Terrain => {
                                    out.terrain.add(stored.normal, rule, surface);
                                }
                                NormalSurface::Ceiling => {
                                    out.ceiling.add(stored.normal, rule, surface);
                                }
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// DL-I10 (local part): pairs distinct, each used by a wet terrain chunk, and listed in
    /// first-use order when the landblocks are visited in vertex-block order.
    #[must_use]
    pub fn water_list_follows_rules(&self) -> bool {
        let n = self.water.len();
        for (a, p) in self.water.iter().enumerate() {
            if self
                .water
                .iter()
                .skip(a + 1)
                .any(|q| q.water_type == p.water_type && q.height.to_bits() == p.height.to_bits())
            {
                return false;
            }
        }
        let mut next = 0usize;
        for x in 0..SIDE {
            for y in 0..SIDE {
                let Some(c) = self.terrain.chunk(x, y) else {
                    return false;
                };
                if !c.wet {
                    continue;
                }
                let i = usize::from(c.water_index);
                if i == next {
                    next += 1;
                } else if i > next {
                    return false;
                }
            }
        }
        next == n
    }

    /// DL-I12 (local part) for one entity.
    fn entity_follows_rules(&self, e: &SectorEntity) -> bool {
        let Some(id) = self.id() else { return false };
        let inside = |(x, y): (u8, u8)| {
            x.checked_sub(id.x).is_some_and(|d| d < SECTOR_SIDE)
                && y.checked_sub(id.y).is_some_and(|d| d < SECTOR_SIDE)
        };
        e.iid.region() == id.region
            && inside(e.iid.landblock())
            && e.position.region() == Some(id.region)
            && e.position.landblock().is_some_and(inside)
            && e.position.cell() == Some(0)
            && e.scale[0].to_bits() == e.scale[1].to_bits()
            && e.scale[1].to_bits() == e.scale[2].to_bits()
            && e.view_distance_sq >= MIN_FAR_VIEW_DISTANCE_SQ
    }
}

/// §3.11: a wet chunk's water index must name a pair of the list.
fn check_water_indices(mesh: &SectorMesh, pairs: usize, at: usize) -> Result<()> {
    for c in &mesh.chunks {
        if c.wet && usize::from(c.water_index) >= pairs {
            return Err(FormatError::IndexOutOfRange {
                what: "sector chunk water index",
                offset: at,
                index: u64::from(c.water_index),
                limit: pairs as u64,
            });
        }
    }
    Ok(())
}

/// Checks a mesh against §4.4/§4.5: (chunk fields by packed rank and absent chunks zero,
/// chunk z ranges equal to their samples, mesh bounds the union of the present chunks).
fn mesh_rules(mesh: &SectorMesh, mask: u32) -> (bool, bool, bool) {
    let (mut chunks_ok, mut heights_ok) = (true, true);
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    let mut rank = 0u32;
    for x in 0..SIDE {
        for y in 0..SIDE {
            let c = chunk_index(x, y);
            let Some(chunk) = mesh.chunks.get(c) else {
                return (false, false, false);
            };
            if mask & (1 << c) == 0 {
                chunks_ok &= chunk.is_zero();
                continue;
            }
            let range = mesh
                .chunk_samples(chunk)
                .and_then(SectorMesh::raw_height_range)
                .map(|(a, b)| (height_from_raw(a), height_from_raw(b)));
            let (min_z, max_z) = range.unwrap_or((f32::NAN, f32::NAN));
            heights_ok &= range.is_some()
                && chunk.min[2].to_bits() == min_z.to_bits()
                && chunk.max[2].to_bits() == max_z.to_bits();
            let mut want = SectorChunk::expected(rank, x, y, chunk.min[2], chunk.max[2]);
            want.wet = chunk.wet;
            want.water_index = chunk.water_index;
            chunks_ok &= *chunk == want;
            for a in 0..3 {
                lo[a] = lo[a].min(chunk.min[a]);
                hi[a] = hi[a].max(chunk.max[a]);
            }
            rank += 1;
        }
    }
    let bounds_ok = rank == 0
        || (lo
            .iter()
            .zip(&mesh.bounds_min)
            .all(|(a, b)| a.to_bits() == b.to_bits())
            && hi
                .iter()
                .zip(&mesh.bounds_max)
                .all(|(a, b)| a.to_bits() == b.to_bits()));
    (chunks_ok, heights_ok, bounds_ok)
}

/// Decodes a distant-landscape sector (`0x805RXXYY`; `distant-landscape.md` §3) with
/// `schema` supplying the types of entity properties.
pub fn decode_distant_sector<S: PropertySchema + ?Sized>(
    data: &[u8],
    schema: &S,
) -> Result<SectorRecord> {
    SectorRecord::parse(data, schema)
}
