// SPDX-License-Identifier: GPL-3.0-or-later
//! A set of resident landblocks of one region, for region-global height queries (the walk
//! controller) and for finding a block's neighbours (normals, LOD stitching).
//!
//! Region-global coordinates follow `coordinates.md` §4: landblock (X, Y) covers
//! 160·X ≤ G_x ≤ 160·(X + 1) (likewise Y), X and Y in 0–254. A point on a shared edge may
//! be answered by either block; both give the same height there (HM7) up to rounding.

use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::landblock::{LANDBLOCK_SIZE, MAX_LANDBLOCK_COORD};

use crate::block::{GroundSample, TerrainBlock};
use crate::lod::Side;
use crate::normals::{Neighbours, VertexNormals};

/// Resident terrain blocks of one region, keyed by landblock (X, Y).
///
/// The map holds at most 255 × 255 entries (coordinates above 254 are refused), so it is
/// bounded whatever the caller inserts.
#[derive(Debug, Clone, Default)]
pub struct TerrainTiles {
    blocks: HashMap<(u8, u8), Arc<TerrainBlock>>,
}

impl TerrainTiles {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts (or replaces) block (x, y); returns the previous one. Coordinates above 254
    /// are refused and `block` is handed back as `Err`.
    pub fn insert(
        &mut self,
        x: u8,
        y: u8,
        block: Arc<TerrainBlock>,
    ) -> Result<Option<Arc<TerrainBlock>>, Arc<TerrainBlock>> {
        if x > MAX_LANDBLOCK_COORD || y > MAX_LANDBLOCK_COORD {
            return Err(block);
        }
        Ok(self.blocks.insert((x, y), block))
    }

    /// Removes block (x, y).
    pub fn remove(&mut self, x: u8, y: u8) -> Option<Arc<TerrainBlock>> {
        self.blocks.remove(&(x, y))
    }

    /// Block (x, y), if resident.
    #[must_use]
    pub fn get(&self, x: u8, y: u8) -> Option<&Arc<TerrainBlock>> {
        self.blocks.get(&(x, y))
    }

    /// Number of resident blocks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Whether no block is resident.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Resident landblock coordinates (unordered).
    pub fn keys(&self) -> impl Iterator<Item = (u8, u8)> + '_ {
        self.blocks.keys().copied()
    }

    /// The landblock across `side` of (x, y), or `None` past the edge of the region.
    #[must_use]
    pub fn neighbour_coord(x: u8, y: u8, side: Side) -> Option<(u8, u8)> {
        let (dx, dy) = side.offset();
        let nx = u8::try_from(i32::from(x) + dx).ok()?;
        let ny = u8::try_from(i32::from(y) + dy).ok()?;
        (nx <= MAX_LANDBLOCK_COORD && ny <= MAX_LANDBLOCK_COORD).then_some((nx, ny))
    }

    /// The resident edge neighbours of (x, y).
    #[must_use]
    pub fn neighbours(&self, x: u8, y: u8) -> Neighbours<'_> {
        Neighbours(Side::ALL.map(|side| {
            Self::neighbour_coord(x, y, side)
                .and_then(|(nx, ny)| self.get(nx, ny))
                .map(Arc::as_ref)
        }))
    }

    /// Vertex normals of block (x, y) using its resident neighbours, or `None` if the block
    /// is not resident.
    #[must_use]
    pub fn normals(&self, x: u8, y: u8) -> Option<VertexNormals> {
        let block = self.get(x, y)?;
        Some(VertexNormals::compute(block, &self.neighbours(x, y)))
    }

    /// The landblock and block-local (x, y) of region-global (G_x, G_y), or `None` outside
    /// 0–40,800 or for NaN. A point on a shared edge goes to the block that contains it in
    /// its half-open lower square, except at the region's far edge (X = 254, x = 160).
    #[must_use]
    pub fn locate(gx: f64, gy: f64) -> Option<(u8, u8, f32, f32)> {
        let size = f64::from(LANDBLOCK_SIZE);
        let max = size * (f64::from(MAX_LANDBLOCK_COORD) + 1.0);
        let axis = |g: f64| -> Option<(u8, f32)> {
            if !(0.0..=max).contains(&g) {
                return None;
            }
            let b = (g / size).floor().min(f64::from(MAX_LANDBLOCK_COORD));
            let local = (g - b * size).clamp(0.0, size);
            Some((b as u8, local as f32))
        };
        let (bx, lx) = axis(gx)?;
        let (by, ly) = axis(gy)?;
        Some((bx, by, lx, ly))
    }

    /// The resident block holding region-global (G_x, G_y) and the local position in it.
    /// When the point lies on an edge or corner shared with a resident block and its own
    /// block is not resident, the neighbour answers instead.
    fn resolve(&self, gx: f64, gy: f64) -> Option<(&TerrainBlock, f32, f32)> {
        let (bx, by, lx, ly) = Self::locate(gx, gy)?;
        if let Some(b) = self.get(bx, by) {
            return Some((b, lx, ly));
        }
        // On a lower edge, the block below/left also contains the point (at local 160).
        let alt_x = (lx == 0.0 && bx > 0).then(|| (bx - 1, LANDBLOCK_SIZE));
        let alt_y = (ly == 0.0 && by > 0).then(|| (by - 1, LANDBLOCK_SIZE));
        let candidates = [
            alt_x.map(|(x, l)| (x, by, l, ly)),
            alt_y.map(|(y, l)| (bx, y, lx, l)),
            alt_x.zip(alt_y).map(|((x, l), (y, m))| (x, y, l, m)),
        ];
        candidates
            .into_iter()
            .flatten()
            .find_map(|(x, y, l, m)| self.get(x, y).map(|b| (b.as_ref(), l, m)))
    }

    /// Terrain height at region-global (G_x, G_y) (`landblock.md` §3.11), or `None` when
    /// no resident block contains the point.
    #[must_use]
    pub fn height_at(&self, gx: f64, gy: f64) -> Option<f32> {
        let (b, x, y) = self.resolve(gx, gy)?;
        b.height_at(x, y)
    }

    /// Height and facet normal at region-global (G_x, G_y).
    #[must_use]
    pub fn ground_at(&self, gx: f64, gy: f64) -> Option<GroundSample> {
        let (b, x, y) = self.resolve(gx, gy)?;
        b.sample(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::landblock::{HeightGrid, VertexFlags};

    fn block(base: u16) -> Arc<TerrainBlock> {
        Arc::new(TerrainBlock::new(
            &HeightGrid::from_fn(|i, j| base + (i + 2 * j) as u16),
            &VertexFlags::default(),
        ))
    }

    #[test]
    fn locate_follows_the_coordinate_spec() {
        // coordinates.md §13: G (29,360, 24,880) → X 183, Y 155, x 80, y 80.
        assert_eq!(
            TerrainTiles::locate(29_360.0, 24_880.0),
            Some((183, 155, 80.0, 80.0))
        );
        assert_eq!(TerrainTiles::locate(160.0, 0.0), Some((1, 0, 0.0, 0.0)));
        assert_eq!(
            TerrainTiles::locate(40_800.0, 40_800.0),
            Some((254, 254, 160.0, 160.0))
        );
        assert_eq!(TerrainTiles::locate(-0.1, 5.0), None);
        assert_eq!(TerrainTiles::locate(40_800.1, 5.0), None);
        assert_eq!(TerrainTiles::locate(f64::NAN, 5.0), None);
        assert_eq!(TerrainTiles::locate(f64::INFINITY, 5.0), None);
    }

    #[test]
    fn queries_and_neighbours() {
        let mut t = TerrainTiles::new();
        assert!(t.insert(10, 20, block(1000)).unwrap().is_none());
        assert!(t.insert(255, 0, block(0)).is_err());
        assert_eq!(t.len(), 1);
        let gx = 10.0 * 160.0 + 52.5;
        let gy = 20.0 * 160.0 + 7.5;
        let z = t.height_at(gx, gy).unwrap();
        assert_eq!(Some(z), t.get(10, 20).unwrap().height_at(52.5, 7.5));
        assert!(t.ground_at(gx, gy).unwrap().normal[2] > 0.0);
        assert_eq!(t.height_at(0.0, 0.0), None);
        // The right edge of (10, 20) is answered by (10, 20) although it locates to (11, 20).
        let edge = t.height_at(11.0 * 160.0, gy).unwrap();
        assert_eq!(Some(edge), t.get(10, 20).unwrap().height_at(160.0, 7.5));
        // And the corner.
        assert!(t.height_at(11.0 * 160.0, 21.0 * 160.0).is_some());
        assert!(t.neighbours(10, 20).0.iter().all(Option::is_none));
        t.insert(11, 20, block(1032)).unwrap();
        let n = t.neighbours(10, 20);
        assert!(n.get(Side::East).is_some());
        assert!(n.get(Side::West).is_none());
        assert!(
            t.normals(10, 20)
                .is_some_and(|n| !n.is_provisional(Side::East))
        );
        assert!(t.normals(0, 0).is_none());
        assert_eq!(TerrainTiles::neighbour_coord(0, 5, Side::West), None);
        assert_eq!(TerrainTiles::neighbour_coord(254, 5, Side::East), None);
        assert_eq!(
            TerrainTiles::neighbour_coord(3, 5, Side::North),
            Some((3, 6))
        );
        assert!(t.remove(10, 20).is_some());
        assert_eq!(t.keys().collect::<Vec<_>>(), [(11, 20)]);
    }
}
