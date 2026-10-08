// SPDX-License-Identifier: GPL-3.0-or-later
//! Coordinate frames (`docs/specs/coordinates.md` §2–§7, §11, §12): landblock keys,
//! region-global and block-local positions, landcells, yaw and compass heading, and the
//! camera-anchored render frame.
//!
//! Conventions (§3): +X east, +Y north, +Z up, right-handed, one unit per metre. A
//! landblock is 160 × 160 units; landblock X and Y run 0–254, so region-global
//! coordinates run 0–40,800. Positions of different regions are never comparable (§4).

use std::fmt;

use tessera_formats::landblock::{INFO_DID_OFFSET, MAX_LANDBLOCK_COORD};
use tessera_formats::setup::Quaternion;
use tessera_formats::{Did, LandblockId, LandblockKind};

use crate::error::{Result, WorldError};

/// Side of a landblock in world units (§4).
pub const LANDBLOCK_SIZE: f64 = 160.0;
/// Largest landblock X or Y (§4, CO1).
pub const MAX_BLOCK: u8 = MAX_LANDBLOCK_COORD;
/// Landblocks per region side (0–254).
pub const BLOCKS_PER_SIDE: u32 = MAX_BLOCK as u32 + 1;
/// Extent of a region in world units on each axis (255 × 160).
pub const REGION_EXTENT: f64 = BLOCKS_PER_SIDE as f64 * LANDBLOCK_SIZE;
/// Side of a landcell in world units (§5.2).
pub const LANDCELL_SIZE: f64 = 20.0;
/// Landcells per landblock side (§5.2).
pub const LANDCELLS_PER_BLOCK: u32 = 8;
/// Largest region number the DID layout can carry (4 bits, `landblock.md` §2.1).
pub const MAX_REGION: u8 = 15;

/// A landblock of one region: the triple (R, X, Y) that addresses every landblock record
/// `0x80KRXXYY` (`landblock.md` §2.1, `coordinates.md` §4).
///
/// Constructed values are always valid: region 1–15, X and Y 0–254.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LandblockKey {
    region: u8,
    x: u8,
    y: u8,
}

impl LandblockKey {
    /// A key for landblock (X, Y) of region `region`.
    ///
    /// # Errors
    ///
    /// [`WorldError::InvalidLandblock`] for a region outside 1–15 or X, Y above 254.
    pub fn new(region: u8, x: u8, y: u8) -> Result<Self> {
        if region == 0 || region > MAX_REGION || x > MAX_BLOCK || y > MAX_BLOCK {
            return Err(WorldError::InvalidLandblock {
                region: region.into(),
                x: x.into(),
                y: y.into(),
            });
        }
        Ok(Self { region, x, y })
    }

    /// The landblock a landblock-family DID belongs to (any record kind). `None` for other
    /// prefixes and for out-of-range parts.
    #[must_use]
    pub fn from_did(did: impl Into<Did>) -> Option<Self> {
        let id = LandblockId::from_did(did.into())?;
        Self::new(id.region, id.x, id.y).ok()
    }

    /// Region number *R*.
    #[must_use]
    pub const fn region(self) -> u8 {
        self.region
    }

    /// Landblock X (east).
    #[must_use]
    pub const fn x(self) -> u8 {
        self.x
    }

    /// Landblock Y (north).
    #[must_use]
    pub const fn y(self) -> u8 {
        self.y
    }

    /// The DID of record kind `kind` of this landblock.
    #[must_use]
    pub fn did(self, kind: LandblockKind) -> Did {
        LandblockId {
            kind,
            region: self.region,
            x: self.x,
            y: self.y,
        }
        .to_did()
    }

    /// Landblock data (heights) `0x800RXXYY`.
    #[must_use]
    pub fn data_did(self) -> Did {
        self.did(LandblockKind::Heightmap)
    }

    /// Block map `0x801RXXYY`.
    #[must_use]
    pub fn block_map_did(self) -> Did {
        self.did(LandblockKind::BlockMap)
    }

    /// Landblock info `0x802RXXYY` (sparse; `landblock.md` §2.3).
    #[must_use]
    pub fn info_did(self) -> Did {
        Did(self.data_did().0 + INFO_DID_OFFSET)
    }

    /// Landblock properties `0x804RXXYY` (sparse).
    #[must_use]
    pub fn properties_did(self) -> Did {
        self.did(LandblockKind::Properties)
    }

    /// Region-global (G_x, G_y) of the south-west corner.
    #[must_use]
    pub fn origin(self) -> [f64; 2] {
        [
            LANDBLOCK_SIZE * f64::from(self.x),
            LANDBLOCK_SIZE * f64::from(self.y),
        ]
    }

    /// Region-global (G_x, G_y) of the centre.
    #[must_use]
    pub fn center(self) -> [f64; 2] {
        let [x, y] = self.origin();
        [x + LANDBLOCK_SIZE / 2.0, y + LANDBLOCK_SIZE / 2.0]
    }

    /// The landblock `dx`, `dy` blocks away in the same region, if it exists.
    #[must_use]
    pub fn offset(self, dx: i32, dy: i32) -> Option<Self> {
        let x = u8::try_from(i32::from(self.x).checked_add(dx)?).ok()?;
        let y = u8::try_from(i32::from(self.y).checked_add(dy)?).ok()?;
        Self::new(self.region, x, y).ok()
    }

    /// Chessboard distance in landblocks; `None` across regions.
    #[must_use]
    pub fn chebyshev(self, other: Self) -> Option<u32> {
        (self.region == other.region)
            .then(|| u32::from(self.x.abs_diff(other.x)).max(u32::from(self.y.abs_diff(other.y))))
    }

    /// Distance in world units from region-global point `g` to the nearest point of this
    /// landblock's square (0 inside it). Non-finite input gives infinity.
    #[must_use]
    pub fn distance_to(self, g: [f64; 2]) -> f64 {
        if !(g[0].is_finite() && g[1].is_finite()) {
            return f64::INFINITY;
        }
        let [x0, y0] = self.origin();
        let dx = (x0 - g[0]).max(g[0] - (x0 + LANDBLOCK_SIZE)).max(0.0);
        let dy = (y0 - g[1]).max(g[1] - (y0 + LANDBLOCK_SIZE)).max(0.0);
        dx.hypot(dy)
    }

    /// The landblock's eight neighbours that exist, in a fixed order (west to east, south to
    /// north).
    pub fn neighbours(self) -> impl Iterator<Item = Self> {
        (-1..=1)
            .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| (dx, dy) != (0, 0))
            .filter_map(move |(dx, dy)| self.offset(dx, dy))
    }

    /// The east, north and north-east neighbours (in that order) whose heights a terrain
    /// mesh of this block needs for seamless edge normals (`landblock.md` §3.3, §8 step 5);
    /// `None` past the grid edge.
    #[must_use]
    pub fn seam_neighbours(self) -> [Option<Self>; 3] {
        [self.offset(1, 0), self.offset(0, 1), self.offset(1, 1)]
    }

    /// Packs the key as `R << 16 | X << 8 | Y` (the low 20 bits of its DIDs).
    #[must_use]
    pub const fn packed(self) -> u32 {
        (self.region as u32) << 16 | (self.x as u32) << 8 | self.y as u32
    }
}

impl fmt::Debug for LandblockKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LandblockKey(r{} {}, {})", self.region, self.x, self.y)
    }
}

impl fmt::Display for LandblockKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "r{} ({}, {})", self.region, self.x, self.y)
    }
}

/// Splits one region-global coordinate into (block, block-local) with the §4 edge rule:
/// X = ⌊G / 160⌋, except that G = 40,800 (the far edge of block 254) stays in block 254.
fn split_axis(g: f64) -> Option<(u8, f64)> {
    if !(0.0..=REGION_EXTENT).contains(&g) {
        return None;
    }
    let b = (g / LANDBLOCK_SIZE).floor().min(f64::from(MAX_BLOCK));
    // In range by the checks above.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let block = b as u8;
    Some((block, g - LANDBLOCK_SIZE * b))
}

/// A region-global position (`coordinates.md` §2): (G_x, G_y) = (160 · X + x, 160 · Y + y)
/// and absolute height z, in f64 because G reaches 40,800 where f32 keeps only millimetres
/// (§12).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WorldPos {
    /// Region number.
    pub region: u8,
    /// G_x, east.
    pub x: f64,
    /// G_y, north.
    pub y: f64,
    /// Absolute height.
    pub z: f64,
}

impl WorldPos {
    /// A position.
    #[must_use]
    pub const fn new(region: u8, x: f64, y: f64, z: f64) -> Self {
        Self { region, x, y, z }
    }

    /// (G_x, G_y).
    #[must_use]
    pub const fn xy(&self) -> [f64; 2] {
        [self.x, self.y]
    }

    /// Whether every component is finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    /// The containing landblock (§4): X = ⌊G_x / 160⌋, with a point on the far edge of the
    /// grid (G = 40,800) kept in block 254. `None` outside the grid, for a non-finite
    /// position or an invalid region.
    #[must_use]
    pub fn landblock(&self) -> Option<LandblockKey> {
        let (x, _) = split_axis(self.x)?;
        let (y, _) = split_axis(self.y)?;
        LandblockKey::new(self.region, x, y).ok()
    }

    /// The containing landblock and the block-local position (§4, §5.1). `None` also for
    /// a non-finite z or one beyond ±40,800 (the bound of [`crate::LocReport::to_local`]).
    #[must_use]
    pub fn to_local(&self) -> Option<LocalPos> {
        let (bx, lx) = split_axis(self.x)?;
        let (by, ly) = split_axis(self.y)?;
        let block = LandblockKey::new(self.region, bx, by).ok()?;
        // z is bounded like `/loc` offsets (±40,800), so narrowing cannot overflow to ±inf.
        if !(self.z.is_finite() && self.z.abs() <= REGION_EXTENT) {
            return None;
        }
        // Block-local values are within [0, 160]; z is narrowed as stored data is (f32).
        #[allow(clippy::cast_possible_truncation)]
        let local = [lx as f32, ly as f32, self.z as f32];
        Some(LocalPos { block, local })
    }

    /// The outdoor landcell containing the position (§5.2).
    #[must_use]
    pub fn landcell(&self) -> Option<Landcell> {
        let local = self.to_local()?;
        Some(local.landcell())
    }

    /// Horizontal distance to `other`; infinity across regions.
    #[must_use]
    pub fn distance_xy(&self, other: &Self) -> f64 {
        if self.region != other.region {
            return f64::INFINITY;
        }
        (self.x - other.x).hypot(self.y - other.y)
    }
}

/// A position in a landblock's own frame (§5.1): x, y from the south-west corner, z
/// absolute. Outdoors x and y are in [0, 160]; interior positions may lie outside (§7) and
/// are never re-homed to a neighbouring block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalPos {
    /// The owning landblock.
    pub block: LandblockKey,
    /// Block-local (x, y, z).
    pub local: [f32; 3],
}

impl LocalPos {
    /// A block-local position.
    #[must_use]
    pub const fn new(block: LandblockKey, local: [f32; 3]) -> Self {
        Self { block, local }
    }

    /// The region-global position (160 · X + x, 160 · Y + y, z).
    #[must_use]
    pub fn to_world(&self) -> WorldPos {
        let [ox, oy] = self.block.origin();
        WorldPos {
            region: self.block.region(),
            x: ox + f64::from(self.local[0]),
            y: oy + f64::from(self.local[1]),
            z: f64::from(self.local[2]),
        }
    }

    /// The landcell (§5.2): lx = 8 · X + ⌊x / 20⌋, clamped to the block's own eight cells
    /// so that x = 160 (and interior positions outside the square) keep the owning block,
    /// as the original does for interiors (`/loc` keeps lx fixed indoors; §9).
    #[must_use]
    pub fn landcell(&self) -> Landcell {
        let cell = |c: f32, block: u8| {
            let i = (f64::from(c) / LANDCELL_SIZE).floor();
            let i = if i.is_finite() {
                i.clamp(0.0, 7.0)
            } else {
                0.0
            };
            // 0..=7 by the clamp.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let i = i as u16;
            u16::from(block) * 8 + i
        };
        Landcell {
            region: self.block.region(),
            lx: cell(self.local[0], self.block.x()),
            ly: cell(self.local[1], self.block.y()),
        }
    }
}

/// An outdoor landcell (§5.2): a 20 × 20 unit cell addressed region-wide, 0–2,039.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Landcell {
    /// Region number.
    pub region: u8,
    /// lx = ⌊G_x / 20⌋.
    pub lx: u16,
    /// ly = ⌊G_y / 20⌋.
    pub ly: u16,
}

impl Landcell {
    /// The landblock (⌊lx / 8⌋, ⌊ly / 8⌋), if valid.
    #[must_use]
    pub fn landblock(&self) -> Option<LandblockKey> {
        let x = u8::try_from(self.lx / 8).ok()?;
        let y = u8::try_from(self.ly / 8).ok()?;
        LandblockKey::new(self.region, x, y).ok()
    }

    /// Region-global (G_x, G_y) of the south-west corner (20 · lx, 20 · ly).
    #[must_use]
    pub fn origin(&self) -> [f64; 2] {
        [
            LANDCELL_SIZE * f64::from(self.lx),
            LANDCELL_SIZE * f64::from(self.ly),
        ]
    }
}

/// The yaw ψ in radians of rotation `q` (`coordinates.md` §11.3): counter-clockwise seen
/// from above, 0 when model +Y points north. It is the yaw of the z-y-x decomposition
/// R_Z(ψ) · R_Y(θ) · R_X(φ), ψ = atan2(2(xy + wz), w² + x² − y² − z²), evaluated in f32
/// in the client's order (§11.3 table, as [`Quaternion::yaw`]) and widened to f64. For a
/// pure turn about +Z this is 2 · atan2(z, w); for a tilted rotation it is **not** the yaw
/// of the projected model +Y axis (§11.4: the bearing of the rotated model +X axis projected
/// onto the ground, minus 90°). Result in [−π, π]; 0 for a non-finite rotation.
#[must_use]
pub fn yaw(q: Quaternion) -> f64 {
    if !q.is_finite() {
        return 0.0;
    }
    let psi = f64::from(q.yaw());
    if psi.is_finite() { psi } else { 0.0 }
}

/// The f32 radians-to-degrees factor of `coordinates.md` §9.4 step 2 (bit pattern
/// 0x42652EE1, 57.295780181884766; client constant, decision 0012).
pub const DEGREES_PER_RADIAN_F32: f32 = f32::from_bits(0x4265_2EE1);

/// The compass heading in degrees, 0 = north, clockwise, of an object whose forward axis
/// is model +Y and whose rotation is `q`, exactly as the client's `/loc` computes `h`
/// (`coordinates.md` §9.4 step 2, C19): ψ of [`Quaternion::yaw`] in f32, then
/// [`heading_from_yaw_f32`], widened to f64 for printing. In [0, 360). For a pure turn it
/// is the bearing of model +Y; for a tilted rotation it follows the z-y-x decomposition
/// (§11.4). 0 for a non-finite rotation.
#[must_use]
pub fn heading_degrees(q: Quaternion) -> f64 {
    if !q.is_finite() {
        return 0.0;
    }
    f64::from(heading_from_yaw_f32(q.yaw()))
}

/// The `/loc` heading `h` in degrees for an f32 yaw ψ in radians (`coordinates.md` §9.4
/// step 2, all in single precision): −ψ × [`DEGREES_PER_RADIAN_F32`], reduced with
/// [`reduce_degrees_f32`]. 0 for a non-finite ψ.
#[must_use]
pub fn heading_from_yaw_f32(psi: f32) -> f32 {
    if !psi.is_finite() {
        return 0.0;
    }
    reduce_degrees_f32((-psi) * DEGREES_PER_RADIAN_F32)
}

/// The angle reduction of `coordinates.md` §9.4 step 2 in single precision: the truncating
/// remainder by 360 (sign of the dividend); a negative remainder gets 360 added in f32, and
/// a sum of 360 or more (a remainder of magnitude at most about 1.53 · 10⁻⁵) becomes 0.
/// In [0, 360) for finite input; NaN stays NaN.
#[must_use]
pub fn reduce_degrees_f32(d: f32) -> f32 {
    let mut r = d % 360.0;
    if r < 0.0 {
        r += 360.0;
        if r >= 360.0 {
            r = 0.0;
        }
    }
    r
}

/// The compass heading (degrees, [0, 360)) for a yaw ψ in radians held in f64 (§11.4):
/// (360° − ψ) mod 360° in double precision. A convenience for f64 callers; it can differ
/// from the client's single-precision `/loc` value in the last printed decimal, which
/// [`heading_degrees`] and [`heading_from_yaw_f32`] reproduce.
#[must_use]
pub fn heading_from_yaw(psi: f64) -> f64 {
    if !psi.is_finite() {
        return 0.0;
    }
    let h = (360.0 - psi.to_degrees()).rem_euclid(360.0);
    // rem_euclid can return 360.0 itself for tiny negative inputs.
    if h >= 360.0 { 0.0 } else { h }
}

/// The yaw ψ (radians) for compass heading `h` (degrees), the inverse of
/// [`heading_from_yaw`], in (−π, π].
#[must_use]
pub fn yaw_from_heading(h: f64) -> f64 {
    if !h.is_finite() {
        return 0.0;
    }
    let psi = (360.0 - h).rem_euclid(360.0);
    let psi = if psi > 180.0 { psi - 360.0 } else { psi };
    psi.to_radians()
}

/// The pure turn about +Z by ψ radians, (cos(ψ / 2), 0, 0, sin(ψ / 2)) (§11.3).
#[must_use]
pub fn yaw_rotation(psi: f64) -> Quaternion {
    let half = if psi.is_finite() { psi / 2.0 } else { 0.0 };
    // Narrowed to the stored precision.
    #[allow(clippy::cast_possible_truncation)]
    Quaternion {
        w: half.cos() as f32,
        x: 0.0,
        y: 0.0,
        z: half.sin() as f32,
    }
}

/// The camera-anchored render frame (`coordinates.md` §12, ARCH §9.1).
///
/// Vertex data of a landblock is built relative to its south-west corner and translated by
/// (160 · (X − X_anchor), 160 · (Y − Y_anchor), 0) at draw time, so every f32 the GPU sees
/// is small. The anchor moves when the camera crosses a landblock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderAnchor {
    /// The anchor landblock.
    pub block: LandblockKey,
}

impl RenderAnchor {
    /// An anchor at `block`.
    #[must_use]
    pub const fn new(block: LandblockKey) -> Self {
        Self { block }
    }

    /// The translation of landblock `key`'s local frame into the render frame; `None`
    /// across regions.
    #[must_use]
    pub fn offset_of(&self, key: LandblockKey) -> Option<[f32; 3]> {
        if key.region() != self.block.region() {
            return None;
        }
        let d = |a: u8, b: u8| (f32::from(a) - f32::from(b)) * 160.0;
        Some([d(key.x(), self.block.x()), d(key.y(), self.block.y()), 0.0])
    }

    /// `pos` in the render frame; `None` across regions or for a non-finite position.
    #[must_use]
    pub fn to_render(&self, pos: &WorldPos) -> Option<[f32; 3]> {
        if pos.region != self.block.region() || !pos.is_finite() {
            return None;
        }
        let [ox, oy] = self.block.origin();
        // Camera-relative values are small within the streamed area.
        #[allow(clippy::cast_possible_truncation)]
        Some([(pos.x - ox) as f32, (pos.y - oy) as f32, pos.z as f32])
    }

    /// Moves the anchor to the landblock containing `pos` if it left the current one.
    /// Returns whether the anchor changed (callers then rebuild per-block translations).
    pub fn rebase(&mut self, pos: &WorldPos) -> bool {
        match pos.landblock() {
            Some(b) if b != self.block => {
                self.block = b;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(r: u8, x: u8, y: u8) -> LandblockKey {
        LandblockKey::new(r, x, y).unwrap()
    }

    #[test]
    fn seam_neighbours_east_north_northeast() {
        let k = LandblockKey::new(1, 10, 20).unwrap();
        let n = |x, y| Some(LandblockKey::new(1, x, y).unwrap());
        assert_eq!(k.seam_neighbours(), [n(11, 20), n(10, 21), n(11, 21)]);
        let edge = LandblockKey::new(1, MAX_BLOCK, 3).unwrap();
        assert_eq!(edge.seam_neighbours(), [None, n(MAX_BLOCK, 4), None]);
    }

    #[test]
    fn to_local_rejects_z_beyond_f32() {
        assert!(WorldPos::new(1, 100.0, 100.0, 1e300).to_local().is_none());
        assert!(WorldPos::new(1, 100.0, 100.0, -1e39).to_local().is_none());
        assert!(
            WorldPos::new(1, 100.0, 100.0, REGION_EXTENT + 1.0)
                .to_local()
                .is_none()
        );
        let l = WorldPos::new(1, 100.0, 100.0, -REGION_EXTENT)
            .to_local()
            .unwrap();
        assert!(l.local[2].is_finite());
    }

    #[test]
    fn key_validation_and_dids() {
        assert!(LandblockKey::new(0, 1, 1).is_err());
        assert!(LandblockKey::new(16, 1, 1).is_err());
        assert!(LandblockKey::new(1, 255, 1).is_err());
        assert!(LandblockKey::new(1, 1, 255).is_err());
        let k = key(1, 126, 140);
        assert_eq!(k.data_did(), Did(0x8001_7E8C)); // coordinates.md §8.4 B
        assert_eq!(k.block_map_did(), Did(0x8011_7E8C));
        assert_eq!(k.info_did(), Did(0x8021_7E8C));
        assert_eq!(k.properties_did(), Did(0x8041_7E8C));
        assert_eq!(LandblockKey::from_did(0x8041_7E8Cu32), Some(k));
        assert_eq!(LandblockKey::from_did(0x80E1_FE01u32), Some(key(1, 254, 1)));
        assert_eq!(LandblockKey::from_did(0x8001_FF00u32), None);
        assert_eq!(LandblockKey::from_did(0x4100_0000u32), None);
        assert_eq!(key(14, 1, 2).data_did(), Did(0x800E_0102));
        assert_eq!(k.packed(), 0x01_7E8C);
        assert_eq!(k.to_string(), "r1 (126, 140)");
    }

    #[test]
    fn neighbours_and_distances() {
        assert_eq!(key(1, 0, 0).neighbours().count(), 3);
        assert_eq!(key(1, 5, 5).neighbours().count(), 8);
        assert_eq!(key(1, 254, 254).neighbours().count(), 3);
        assert_eq!(key(1, 0, 0).offset(-1, 0), None);
        assert_eq!(key(1, 3, 4).chebyshev(key(1, 6, 2)), Some(3));
        assert_eq!(key(1, 3, 4).chebyshev(key(2, 3, 4)), None);
        let k = key(1, 1, 1);
        assert_eq!(k.distance_to([200.0, 200.0]), 0.0);
        assert_eq!(k.distance_to([100.0, 200.0]), 60.0);
        assert!((k.distance_to([0.0, 0.0]) - 160f64.hypot(160.0)).abs() < 1e-9);
        assert_eq!(k.distance_to([f64::NAN, 0.0]), f64::INFINITY);
    }

    #[test]
    fn world_local_round_trip() {
        // coordinates.md §8.4 A and D.
        let p = LocalPos::new(key(1, 100, 150), [40.0, 80.0, 10.0]).to_world();
        assert_eq!((p.x, p.y), (16_040.0, 24_080.0));
        let l = WorldPos::new(1, 29_360.0, 24_880.0, 0.0)
            .to_local()
            .unwrap();
        assert_eq!(l.block, key(1, 183, 155));
        assert_eq!(l.local, [80.0, 80.0, 0.0]);
        // Edges: the far edge of the grid stays in block 254, a shared edge goes up.
        let e = WorldPos::new(1, REGION_EXTENT, 160.0, 0.0)
            .to_local()
            .unwrap();
        assert_eq!(e.block, key(1, 254, 1));
        assert_eq!(e.local[0], 160.0);
        assert_eq!(e.local[1], 0.0);
        assert!(WorldPos::new(1, -0.1, 0.0, 0.0).landblock().is_none());
        assert!(WorldPos::new(1, 40_800.1, 0.0, 0.0).landblock().is_none());
        assert!(WorldPos::new(1, f64::NAN, 0.0, 0.0).landblock().is_none());
        assert!(WorldPos::new(0, 1.0, 1.0, 0.0).landblock().is_none());
        assert!(
            WorldPos::new(1, 1.0, 1.0, f64::INFINITY)
                .to_local()
                .is_none()
        );
    }

    #[test]
    fn landcells() {
        // coordinates.md §13: (X 100, Y 150, x 40, y 80) → lx 802, ly 1204.
        let c = LocalPos::new(key(1, 100, 150), [40.0, 80.0, 0.0]).landcell();
        assert_eq!((c.lx, c.ly), (802, 1204));
        assert_eq!(c.landblock(), Some(key(1, 100, 150)));
        assert_eq!(c.origin(), [16_040.0, 24_080.0]);
        let edge = LocalPos::new(key(1, 3, 3), [160.0, -500.0, 0.0]).landcell();
        assert_eq!((edge.lx, edge.ly), (31, 24));
        let c = WorldPos::new(1, 40_799.0, 0.0, 0.0).landcell().unwrap();
        assert_eq!(c.lx, 2_039);
    }

    #[test]
    fn yaw_and_heading() {
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let cases = [
            (
                Quaternion {
                    w: 1.0,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                0.0,
            ),
            (
                Quaternion {
                    w: s,
                    x: 0.0,
                    y: 0.0,
                    z: s,
                },
                270.0,
            ),
            (
                Quaternion {
                    w: 0.0,
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                180.0,
            ),
            (
                Quaternion {
                    w: s,
                    x: 0.0,
                    y: 0.0,
                    z: -s,
                },
                90.0,
            ),
        ];
        for (q, h) in cases {
            let got = heading_degrees(q);
            let diff = (got - h).abs().min(360.0 - (got - h).abs());
            assert!(diff < 1e-3, "{q:?}: {got} != {h}");
        }
        // Model +Y under a 90° yaw goes west (§13).
        let v = Quaternion {
            w: s,
            x: 0.0,
            y: 0.0,
            z: s,
        }
        .rotate([0.0, 1.0, 0.0]);
        assert!((v[0] + 1.0).abs() < 1e-5 && v[1].abs() < 1e-5);
        // Tilt about +X keeps the heading at north.
        let tilt = Quaternion {
            w: 0.99619,
            x: 0.08716,
            y: 0.0,
            z: 0.0,
        };
        assert!(heading_degrees(tilt).min(360.0 - heading_degrees(tilt)) < 1e-3);
        for h in [0.0, 45.0, 90.0, 180.0, 270.0, 359.0] {
            let back = heading_from_yaw(yaw_from_heading(h));
            assert!(
                (back - h).abs() < 1e-9 || (back - h).abs() > 359.999,
                "{h} {back}"
            );
            let q = yaw_rotation(yaw_from_heading(h));
            let got = heading_degrees(q);
            assert!(
                (got - h).abs() < 1e-3 || (got - h).abs() > 359.99,
                "{h} {got}"
            );
        }
        assert_eq!(heading_from_yaw(f64::NAN), 0.0);
        assert_eq!(
            yaw(Quaternion {
                w: f32::NAN,
                x: 0.0,
                y: 0.0,
                z: 0.0
            }),
            0.0
        );
    }

    /// `coordinates.md` §11.4–§11.5: the heading of a tilted rotation follows the z-y-x
    /// decomposition, not the projected model +Y axis.
    #[test]
    fn heading_of_tilted_rotations_uses_zyx_decomposition() {
        let near = |got: f64, want: f64, tol: f64| {
            let d = (got - want).rem_euclid(360.0);
            d.min(360.0 - d) < tol
        };
        // 45° about +X, then 30° about +Y: ψ = 0, so `/loc` omits h.
        let tilted = Quaternion {
            w: 0.8924,
            x: 0.36964,
            y: 0.23912,
            z: -0.09905,
        };
        assert!(yaw(tilted).abs() < 1e-3, "{}", yaw(tilted));
        assert!(near(heading_degrees(tilted), 0.0, 0.05));
        // Its model +Y axis projects to a bearing of 26.6°: the withdrawn rule's answer.
        let f = tilted.rotate([0.0, 1.0, 0.0]);
        let projected =
            (360.0 - f64::from(-f[0]).atan2(f64::from(f[1])).to_degrees()).rem_euclid(360.0);
        assert!(near(projected, 26.6, 0.1), "{projected}");
        assert!(!near(heading_degrees(tilted), projected, 1.0));
        // The row above, then yaw 90°: h270.0 (§11.5).
        let turned = Quaternion {
            w: 0.70106,
            x: 0.0923,
            y: 0.43046,
            z: 0.56099,
        };
        assert!(
            near(heading_degrees(turned), 270.0, 0.05),
            "{}",
            heading_degrees(turned)
        );
        // Every rotation agrees with tessera-formats (same f32 evaluation order).
        let mut seed = 0x1234_5678_u32;
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            f32::from(u16::try_from(seed >> 16).unwrap_or(0)) / 32_768.0 - 1.0
        };
        for _ in 0..1_000 {
            let raw = Quaternion {
                w: next(),
                x: next(),
                y: next(),
                z: next(),
            };
            let len = raw.length();
            if len < 0.1 {
                continue;
            }
            let q = Quaternion {
                w: raw.w / len,
                x: raw.x / len,
                y: raw.y / len,
                z: raw.z / len,
            };
            let psi = q.yaw();
            assert_eq!(yaw(q), f64::from(psi));
            let a = heading_degrees(q);
            assert_eq!(a, f64::from(heading_from_yaw_f32(psi)));
            let b = f64::from(q.heading_degrees());
            assert!(near(a, b, 1e-3), "{q:?}: {a} vs {b}");
        }
        // Non-finite input gives 0.
        let nan = Quaternion {
            w: 1.0,
            x: f32::INFINITY,
            y: 0.0,
            z: 0.0,
        };
        assert_eq!(yaw(nan), 0.0);
        assert_eq!(heading_degrees(nan), 0.0);
    }

    /// `coordinates.md` §9.4 step 2 and C19: `h` is computed and reduced in f32.
    #[test]
    fn heading_follows_the_single_precision_loc_rule() {
        assert_eq!(DEGREES_PER_RADIAN_F32.to_bits(), 0x4265_2EE1);
        assert_eq!(f64::from(DEGREES_PER_RADIAN_F32), 57.295_780_181_884_766);
        // §9.4 examples: −ψ in degrees −0.00001 → 360 in f32 → 0; −0.00002 → the f32
        // below 360; −0.02 → 359.98.
        assert_eq!(reduce_degrees_f32(-0.000_01), 0.0);
        let below = reduce_degrees_f32(-0.000_02);
        assert_eq!(below, f32::from_bits(360.0_f32.to_bits() - 1));
        assert_eq!(reduce_degrees_f32(-0.02), 360.0_f32 - 0.02);
        assert_eq!(reduce_degrees_f32(725.5), 5.5);
        assert_eq!(reduce_degrees_f32(-725.5), 360.0 - 5.5);
        assert!(reduce_degrees_f32(f32::NAN).is_nan());
        assert_eq!(heading_from_yaw_f32(f32::INFINITY), 0.0);
        // A pure turn where the f64 path rounds differently: the f32 rule gives exactly
        // 173.25 (printed 173.2 with round-half-even), the f64 one 173.2500023 (173.3).
        let q = Quaternion {
            w: 0.058_870_804,
            x: 0.0,
            y: 0.0,
            z: -0.998_265_6,
        };
        assert_eq!(heading_degrees(q), 173.25);
        assert!(heading_from_yaw(yaw(q)) > 173.25);
        // ψ so small that −ψ in degrees is −0.00001: the f32 sum rounds to 360 → 0.
        let psi = 0.000_01_f32 / DEGREES_PER_RADIAN_F32;
        assert_eq!(heading_from_yaw_f32(psi), 0.0);
        let turn = Quaternion {
            w: (psi / 2.0).cos(),
            x: 0.0,
            y: 0.0,
            z: (psi / 2.0).sin(),
        };
        assert_eq!(heading_degrees(turn), 0.0);
    }

    #[test]
    fn render_anchor() {
        let mut a = RenderAnchor::new(key(1, 100, 100));
        assert_eq!(a.offset_of(key(1, 101, 99)), Some([160.0, -160.0, 0.0]));
        assert_eq!(a.offset_of(key(2, 101, 99)), None);
        let p = WorldPos::new(1, 16_010.0, 16_020.0, 5.0);
        assert_eq!(a.to_render(&p), Some([10.0, 20.0, 5.0]));
        assert!(!a.rebase(&p));
        assert!(a.rebase(&WorldPos::new(1, 16_170.0, 16_020.0, 5.0)));
        assert_eq!(a.block, key(1, 101, 100));
    }
}
