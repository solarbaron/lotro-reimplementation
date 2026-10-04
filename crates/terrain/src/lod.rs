// SPDX-License-Identifier: GPL-3.0-or-later
//! Levels of detail (geomipmapping), block sides and LOD selection.
//!
//! A landblock's 32 × 32 quads are meshed at one of six levels: level *l* uses every
//! 2^*l*-th grid sample, so level 0 is the full 5-unit grid (exactly the stored
//! triangulation, `landblock.md` §3.6) and level 5 is one 160-unit quad. Every level's
//! vertices are grid samples, so a vertex has the same position and normal at every level.
//!
//! Neighbouring blocks may sit at different levels. The edge they share is meshed by both
//! at the **coarser** of the two steps (see [`EdgeLods`] and [`crate::mesh::triangulate`]),
//! so their edge vertices and edge segments are identical and no crack can open.

use crate::block::TerrainBlock;
use crate::error::{Result, TerrainError};
use tessera_formats::landblock::{GRID_SPACING, QUAD_SIDE};

/// A terrain level of detail, 0 (full grid) to 5 (one quad per block).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Lod(u8);

impl Lod {
    /// The full 33 × 33 grid (5-unit quads).
    pub const FINEST: Self = Self(0);
    /// One quad per block (160-unit quads).
    pub const COARSEST: Self = Self(5);
    /// Number of levels.
    pub const COUNT: usize = 6;

    /// Level `level`, or [`TerrainError::LodOutOfRange`] above 5.
    pub fn new(level: u8) -> Result<Self> {
        if level <= Self::COARSEST.0 {
            Ok(Self(level))
        } else {
            Err(TerrainError::LodOutOfRange(level))
        }
    }

    /// Level `level`, clamped to 5.
    #[must_use]
    pub fn clamped(level: u8) -> Self {
        Self(level.min(Self::COARSEST.0))
    }

    /// The level number (0–5).
    #[must_use]
    pub fn level(self) -> u8 {
        self.0
    }

    /// Distance between the level's vertices in grid samples: 1, 2, 4, 8, 16 or 32.
    #[must_use]
    pub fn step(self) -> usize {
        1 << self.0
    }

    /// Quads per block side at this level: 32, 16, 8, 4, 2 or 1.
    #[must_use]
    pub fn quads_per_side(self) -> usize {
        QUAD_SIDE >> self.0
    }

    /// Edge length of one of this level's quads in world units.
    #[must_use]
    pub fn quad_size(self) -> f32 {
        GRID_SPACING * self.step() as f32
    }

    /// The next coarser level, saturating at 5.
    #[must_use]
    pub fn coarser(self) -> Self {
        Self::clamped(self.0 + 1)
    }

    /// The next finer level, saturating at 0.
    #[must_use]
    pub fn finer(self) -> Self {
        Self(self.0.saturating_sub(1))
    }

    /// Every level, finest first.
    pub fn all() -> impl Iterator<Item = Self> {
        (0..=Self::COARSEST.0).map(Self)
    }
}

impl std::fmt::Display for Lod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LOD{}", self.0)
    }
}

/// One side of a landblock. Compass directions follow `coordinates.md` §3: +X east, +Y
/// north.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Side {
    /// y = 0 (towards landblock Y − 1).
    South,
    /// x = 160 (towards landblock X + 1).
    East,
    /// y = 160 (towards landblock Y + 1).
    North,
    /// x = 0 (towards landblock X − 1).
    West,
}

impl Side {
    /// All sides, counter-clockwise from south (the order of every per-side array here).
    pub const ALL: [Self; 4] = [Self::South, Self::East, Self::North, Self::West];

    /// Position in [`Side::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    /// The side facing this one across the shared edge.
    #[must_use]
    pub fn opposite(self) -> Self {
        Self::ALL[(self.index() + 2) % 4]
    }

    /// Landblock offset (dX, dY) of the neighbour across this side.
    #[must_use]
    pub fn offset(self) -> (i32, i32) {
        match self {
            Self::South => (0, -1),
            Self::East => (1, 0),
            Self::North => (0, 1),
            Self::West => (-1, 0),
        }
    }

    /// The previous side counter-clockwise (the side at this side's start corner).
    #[must_use]
    pub(crate) fn prev(self) -> Self {
        Self::ALL[(self.index() + 3) % 4]
    }

    /// The next side counter-clockwise (the side at this side's end corner).
    #[must_use]
    pub(crate) fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % 4]
    }
}

/// The LOD of each neighbour, or `None` where there is no neighbour (edge of the world,
/// not loaded). The edge shared with a neighbour is meshed at the coarser of the two
/// levels; a side without a neighbour uses the block's own level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct EdgeLods(pub [Option<Lod>; 4]);

impl EdgeLods {
    /// No neighbour information: every edge at the block's own level.
    pub const NONE: Self = Self([None; 4]);

    /// Every neighbour at `lod`.
    #[must_use]
    pub fn uniform(lod: Lod) -> Self {
        Self([Some(lod); 4])
    }

    /// The neighbour LOD across `side`.
    #[must_use]
    pub fn get(&self, side: Side) -> Option<Lod> {
        self.0[side.index()]
    }

    /// This value with the neighbour across `side` at `lod`.
    #[must_use]
    pub fn with(mut self, side: Side, lod: Option<Lod>) -> Self {
        self.0[side.index()] = lod;
        self
    }

    /// The level each side of a block at `own` is meshed at: the coarser of `own` and the
    /// neighbour's level.
    #[must_use]
    pub fn effective(&self, own: Lod) -> [Lod; 4] {
        Side::ALL.map(|s| self.get(s).map_or(own, |n| n.max(own)))
    }
}

/// Screen-space-error LOD selection (Tessera's own policy, not a measurement of any other
/// client).
///
/// A level is acceptable when its geometric error ([`TerrainBlock::lod_error`], the largest
/// vertical distance between the level's regular surface and the level-0 surface) projects
/// to at most `pixel_tolerance` pixels at the block's distance. The selector picks the
/// coarsest acceptable level; within `full_detail_radius` it always picks level 0, so the
/// walker's collision surface and the rendered surface agree near the camera.
///
/// Both thresholds have hysteresis (see [`Self::hysteresis`]), so a block whose distance
/// sits on a threshold keeps its level instead of flipping (and re-stitching its
/// neighbours) on every small camera move.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LodSelector {
    /// Largest projected error in pixels.
    pub pixel_tolerance: f32,
    /// Pixels per world unit at distance 1: `viewport_height / (2 · tan(fov_y / 2))`.
    pub projection_scale: f32,
    /// Blocks closer than this (world units) are always level 0.
    pub full_detail_radius: f32,
    /// Relative hysteresis margin (clamped to 0–10; non-finite counts as 0). A block only
    /// moves to a coarser level once that level's projected error is below
    /// `pixel_tolerance / (1 + hysteresis)`, and a block at level 0 only leaves it once
    /// its distance reaches `full_detail_radius · (1 + hysteresis)`.
    pub hysteresis: f32,
}

impl Default for LodSelector {
    fn default() -> Self {
        Self::from_fov(1080.0, 60f32.to_radians(), 1.5)
    }
}

impl LodSelector {
    /// A selector for a viewport `viewport_height` pixels tall with vertical field of view
    /// `fov_y` (radians). Non-finite or non-positive inputs fall back to 1080 px and 60°.
    #[must_use]
    pub fn from_fov(viewport_height: f32, fov_y: f32, pixel_tolerance: f32) -> Self {
        let h = if viewport_height.is_finite() && viewport_height > 0.0 {
            viewport_height
        } else {
            1080.0
        };
        let fov = if fov_y.is_finite() && fov_y > 0.0 && fov_y < std::f32::consts::PI {
            fov_y
        } else {
            60f32.to_radians()
        };
        Self {
            pixel_tolerance,
            projection_scale: h / (2.0 * (fov / 2.0).tan()),
            full_detail_radius: 240.0,
            hysteresis: 0.15,
        }
    }

    /// The level for `block` at `distance` world units from the camera (see
    /// [`TerrainBlock::distance_to`]). `current` is the block's level last frame, for
    /// hysteresis.
    ///
    /// Degenerate inputs never force a re-mesh storm:
    /// - `distance` = +∞ (a camera infinitely far away) selects level 5, the cheapest;
    /// - `distance` = NaN (a corrupt camera) keeps `current`, or level 5 without one;
    /// - an invalid selector (NaN or negative `pixel_tolerance`, `projection_scale` not
    ///   finite and positive) selects level 0, the same for every frame.
    #[must_use]
    pub fn select(&self, block: &TerrainBlock, distance: f32, current: Option<Lod>) -> Lod {
        if distance.is_nan() {
            return current.unwrap_or(Lod::COARSEST);
        }
        let tol = self.pixel_tolerance;
        let scale = self.projection_scale;
        if tol.is_nan() || tol < 0.0 || !(scale.is_finite() && scale > 0.0) {
            return Lod::FINEST;
        }
        if distance == f32::INFINITY {
            return Lod::COARSEST;
        }
        let hyst = if self.hysteresis.is_finite() {
            self.hysteresis.clamp(0.0, 10.0)
        } else {
            0.0
        };
        // A block at level 0 keeps it until the radius plus the margin.
        let radius = if current == Some(Lod::FINEST) {
            self.full_detail_radius * (1.0 + hyst)
        } else {
            self.full_detail_radius
        };
        if distance < radius {
            return Lod::FINEST;
        }
        let d = distance.max(1.0);
        let mut chosen = Lod::FINEST;
        for lod in Lod::all().skip(1) {
            let pixels = block.lod_error(lod) * scale / d;
            // Moving coarser than the current level needs the hysteresis margin.
            let limit = if current.is_some_and(|c| lod > c) {
                tol / (1.0 + hyst)
            } else {
                tol
            };
            if pixels <= limit {
                chosen = lod;
            } else {
                break;
            }
        }
        chosen
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::landblock::{HeightGrid, VertexFlags};

    #[test]
    fn levels() {
        assert_eq!(Lod::new(6), Err(TerrainError::LodOutOfRange(6)));
        assert_eq!(Lod::clamped(200), Lod::COARSEST);
        let steps: Vec<_> = Lod::all().map(Lod::step).collect();
        assert_eq!(steps, [1, 2, 4, 8, 16, 32]);
        let quads: Vec<_> = Lod::all().map(Lod::quads_per_side).collect();
        assert_eq!(quads, [32, 16, 8, 4, 2, 1]);
        assert_eq!(Lod::COARSEST.coarser(), Lod::COARSEST);
        assert_eq!(Lod::FINEST.finer(), Lod::FINEST);
        assert_eq!(Lod::new(3).unwrap().quad_size(), 40.0);
        assert_eq!(Lod::new(2).unwrap().to_string(), "LOD2");
    }

    #[test]
    fn sides() {
        for s in Side::ALL {
            assert_eq!(s.opposite().opposite(), s);
            assert_eq!(s.next().prev(), s);
            let (dx, dy) = s.offset();
            let (ox, oy) = s.opposite().offset();
            assert_eq!((dx + ox, dy + oy), (0, 0));
        }
    }

    #[test]
    fn effective_edges_take_the_coarser_level() {
        let own = Lod::new(2).unwrap();
        let e = EdgeLods::NONE
            .with(Side::East, Some(Lod::new(4).unwrap()))
            .with(Side::West, Some(Lod::FINEST));
        assert_eq!(
            e.effective(own),
            [own, Lod::new(4).unwrap(), own, own],
            "south none, east coarser, north none, west finer"
        );
        assert_eq!(EdgeLods::uniform(own).effective(Lod::FINEST), [own; 4]);
    }

    #[test]
    fn selector_prefers_coarse_far_away_and_full_detail_near() {
        let block = TerrainBlock::new(
            &HeightGrid::from_fn(|i, j| ((i * 37 + j * 91) % 13 * 400) as u16),
            &VertexFlags::default(),
        );
        let sel = LodSelector::default();
        assert_eq!(sel.select(&block, 10.0, None), Lod::FINEST);
        // A corrupt camera keeps the current level; an infinitely far one is cheapest.
        assert_eq!(sel.select(&block, f32::NAN, None), Lod::COARSEST);
        assert_eq!(sel.select(&block, f32::NAN, Some(Lod::FINEST)), Lod::FINEST);
        let l2 = Lod::new(2).unwrap();
        assert_eq!(sel.select(&block, f32::NAN, Some(l2)), l2);
        assert_eq!(sel.select(&block, f32::INFINITY, None), Lod::COARSEST);
        assert_eq!(
            sel.select(&block, f32::INFINITY, Some(Lod::FINEST)),
            Lod::COARSEST
        );
        assert_eq!(
            sel.select(
                &block,
                block.distance_to([1e30, 0.0, 0.0]),
                Some(Lod::FINEST)
            ),
            Lod::COARSEST
        );
        // An invalid selector is level 0, consistently.
        for bad in [
            LodSelector {
                pixel_tolerance: f32::NAN,
                ..sel
            },
            LodSelector {
                pixel_tolerance: -1.0,
                ..sel
            },
            LodSelector {
                projection_scale: f32::INFINITY,
                ..sel
            },
            LodSelector {
                projection_scale: 0.0,
                ..sel
            },
        ] {
            assert_eq!(bad.select(&block, 1.0e6, None), Lod::FINEST);
            assert_eq!(bad.select(&block, f32::INFINITY, None), Lod::FINEST);
        }
        let far = sel.select(&block, 1.0e7, None);
        assert_eq!(far, Lod::COARSEST);
        // Monotone in distance.
        let mut last = Lod::FINEST;
        for d in [300.0, 600.0, 1200.0, 2400.0, 4800.0, 9600.0, 19200.0, 1.0e6] {
            let l = sel.select(&block, d, None);
            assert!(l >= last, "{d}: {l} < {last}");
            last = l;
        }
        // A flat block is coarsest as soon as it leaves the full-detail radius.
        let flat = TerrainBlock::flat(1000);
        assert_eq!(sel.select(&flat, 300.0, None), Lod::COARSEST);
    }

    #[test]
    fn selector_hysteresis_delays_coarsening() {
        let block = TerrainBlock::new(
            &HeightGrid::from_fn(|i, j| ((i * 37 + j * 91) % 13 * 400) as u16),
            &VertexFlags::default(),
        );
        let sel = LodSelector {
            hysteresis: 0.5,
            ..LodSelector::default()
        };
        let l1 = Lod::new(1).unwrap();
        // Distance at which LOD1 projects to exactly the tolerance.
        let d = block.lod_error(l1) * sel.projection_scale / sel.pixel_tolerance;
        let d = d * 1.01;
        assert!(sel.select(&block, d, None) >= l1);
        assert_eq!(sel.select(&block, d, Some(Lod::FINEST)), Lod::FINEST);
        // Already at LOD1: stays.
        assert!(sel.select(&block, d, Some(l1)) >= l1);
    }

    #[test]
    fn selector_full_detail_radius_has_hysteresis() {
        // Regression: the radius was a hard threshold, so a block at ~240 units flipped
        // between level 0 and level 5 on every 0.2-unit camera move.
        let rough = TerrainBlock::new(
            &HeightGrid::from_fn(|i, j| (20_000 + (i * 7 + j * 3) % 5) as u16),
            &VertexFlags::default(),
        );
        let sel = LodSelector::default();
        let r = sel.full_detail_radius;
        for block in [TerrainBlock::flat(10_000), rough] {
            let mut current = None;
            let mut levels = Vec::new();
            for d in [r - 0.1, r + 0.1, r - 0.1, r + 0.1, r - 0.1, r + 0.1] {
                let l = sel.select(&block, d, current);
                levels.push(l);
                current = Some(l);
            }
            assert!(levels.iter().all(|&l| l == Lod::FINEST), "{levels:?}");
            // Leaves level 0 only past the margin, then stays coarse just outside the
            // radius.
            let out = r * (1.0 + sel.hysteresis) + 1.0;
            let coarse = sel.select(&block, out, Some(Lod::FINEST));
            assert!(coarse > Lod::FINEST);
            assert_eq!(sel.select(&block, r + 0.1, Some(coarse)), coarse);
            // Entering the radius always gives level 0.
            assert_eq!(sel.select(&block, r - 0.1, Some(coarse)), Lod::FINEST);
        }
    }
}
