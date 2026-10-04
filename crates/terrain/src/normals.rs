// SPDX-License-Identifier: GPL-3.0-or-later
//! Seamless per-vertex normals.
//!
//! The normal at grid sample (i, j) comes from central differences of the stored heights,
//! `dz/dx ≈ (h(i + 1, j) − h(i − 1, j)) / 10` (likewise for y). On a block edge the
//! sample beyond the edge is read from the neighbour across that side (its second row or
//! column, `landblock.md` §3.3/§8 step 5). Because neighbouring blocks share their edge
//! samples (HM7), both blocks then compute **bit-identical** normals for every shared
//! vertex: the differences are taken on stored integers before any floating-point step.
//!
//! Only the four edge neighbours are needed (a corner sample uses the east/west neighbour
//! for x and the north/south neighbour for y). Normals are computed on the full grid and
//! shared by every LOD, so a vertex keeps its normal across LOD transitions too.
//!
//! Without a neighbour the difference becomes one-sided (scaled to the same span); the
//! result is marked **provisional** on that side so a streamer can recompute it once the
//! neighbour arrives.

use tessera_formats::landblock::{GRID_SAMPLES, GRID_SIDE, GRID_SPACING, QUAD_SIDE};

use crate::block::{STEPS_PER_UNIT, TerrainBlock, unit_normal, vertex_index};
use crate::lod::Side;

/// The four edge neighbours of a block (any may be missing).
#[derive(Debug, Clone, Copy, Default)]
pub struct Neighbours<'a>(pub [Option<&'a TerrainBlock>; 4]);

impl<'a> Neighbours<'a> {
    /// No neighbours.
    pub const NONE: Self = Self([None; 4]);

    /// The neighbour across `side`.
    #[must_use]
    pub fn get(&self, side: Side) -> Option<&'a TerrainBlock> {
        self.0[side.index()]
    }

    /// This value with `block` across `side`.
    #[must_use]
    pub fn with(mut self, side: Side, block: Option<&'a TerrainBlock>) -> Self {
        self.0[side.index()] = block;
        self
    }
}

/// Unit normals of all 33 × 33 grid samples, in vertex order (33·j + i).
#[derive(Clone, PartialEq)]
pub struct VertexNormals {
    normals: Box<[[f32; 3]; GRID_SAMPLES]>,
    provisional: [bool; 4],
}

impl std::fmt::Debug for VertexNormals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VertexNormals")
            .field("provisional", &self.provisional)
            .finish_non_exhaustive()
    }
}

impl VertexNormals {
    /// Normals of `block` using `neighbours` for the samples beyond its edges.
    #[must_use]
    pub fn compute(block: &TerrainBlock, neighbours: &Neighbours<'_>) -> Self {
        let raw =
            |b: &TerrainBlock, i: usize, j: usize| i32::from(b.raw_heights()[vertex_index(i, j)]);
        let last = QUAD_SIDE;
        // Twice the stored-height difference over two cells along x at (i, j).
        let dx = |i: usize, j: usize| -> i32 {
            let h = |a| raw(block, a, j);
            match i {
                0 => match neighbours.get(Side::West) {
                    Some(w) => h(1) - raw(w, last - 1, j),
                    None => 2 * (h(1) - h(0)),
                },
                i if i == last => match neighbours.get(Side::East) {
                    Some(e) => raw(e, 1, j) - h(last - 1),
                    None => 2 * (h(last) - h(last - 1)),
                },
                i => h(i + 1) - h(i - 1),
            }
        };
        let dy = |i: usize, j: usize| -> i32 {
            let h = |b| raw(block, i, b);
            match j {
                0 => match neighbours.get(Side::South) {
                    Some(s) => h(1) - raw(s, i, last - 1),
                    None => 2 * (h(1) - h(0)),
                },
                j if j == last => match neighbours.get(Side::North) {
                    Some(n) => raw(n, i, 1) - h(last - 1),
                    None => 2 * (h(last) - h(last - 1)),
                },
                j => h(j + 1) - h(j - 1),
            }
        };
        // Stored steps over two cells → world-unit slope.
        let scale = 1.0 / (STEPS_PER_UNIT * 2.0 * f64::from(GRID_SPACING));
        let mut normals = Box::new([[0.0f32, 0.0, 1.0]; GRID_SAMPLES]);
        for j in 0..GRID_SIDE {
            for i in 0..GRID_SIDE {
                let gx = f64::from(dx(i, j)) * scale;
                let gy = f64::from(dy(i, j)) * scale;
                normals[vertex_index(i, j)] = unit_normal(gx, gy);
            }
        }
        Self {
            normals,
            provisional: Side::ALL.map(|s| neighbours.get(s).is_none()),
        }
    }

    /// Normal of sample (i, j); `None` outside 0–32.
    #[must_use]
    pub fn get(&self, i: usize, j: usize) -> Option<[f32; 3]> {
        (i < GRID_SIDE && j < GRID_SIDE).then(|| self.normals[vertex_index(i, j)])
    }

    /// All normals in vertex order.
    #[must_use]
    pub fn as_slice(&self) -> &[[f32; 3]; GRID_SAMPLES] {
        &self.normals
    }

    /// Whether the normals along `side` were computed without the neighbour there (and will
    /// change once it is known).
    #[must_use]
    pub fn is_provisional(&self, side: Side) -> bool {
        self.provisional[side.index()]
    }

    /// Whether every side had its neighbour.
    #[must_use]
    pub fn is_final(&self) -> bool {
        !self.provisional.contains(&true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::landblock::{HeightGrid, VertexFlags};

    fn field(gi: i64, gj: i64) -> u16 {
        // Smooth-ish integer field with variation in both axes.
        let v = 20_000 + (gi * 37) % 1_500 + (gj * gj) % 977 + (gi * gj) % 211;
        v as u16
    }

    fn block_at(bx: i64, by: i64) -> TerrainBlock {
        TerrainBlock::new(
            &HeightGrid::from_fn(|i, j| field(bx * 32 + i as i64, by * 32 + j as i64)),
            &VertexFlags::default(),
        )
    }

    #[test]
    fn flat_and_plane_normals() {
        let n = VertexNormals::compute(&TerrainBlock::flat(500), &Neighbours::NONE);
        assert!(n.as_slice().iter().all(|v| *v == [0.0, 0.0, 1.0]));
        assert!(!n.is_final());
        // A plane rising 2 steps per sample east: one-sided differences at the edges agree
        // with central ones.
        let g = HeightGrid::from_fn(|i, _| (1000 + 2 * i) as u16);
        let b = TerrainBlock::new(&g, &VertexFlags::default());
        let n = VertexNormals::compute(&b, &Neighbours::NONE);
        let first = n.get(0, 0).unwrap();
        assert!(n.as_slice().iter().all(|v| *v == first));
        assert!(first[0] < 0.0 && first[1] == 0.0 && first[2] > 0.0);
        let len: f32 = first.iter().map(|c| c * c).sum();
        assert!((len - 1.0).abs() < 1e-6);
    }

    /// Normals of shared vertices are bit-identical in both blocks of every pair.
    #[test]
    fn seams_are_bit_identical() {
        let blocks: Vec<Vec<TerrainBlock>> = (0..3)
            .map(|y| (0..3).map(|x| block_at(x, y)).collect())
            .collect();
        let nb = |x: usize, y: usize| {
            let get = |dx: i64, dy: i64| {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                (0..3)
                    .contains(&nx)
                    .then_some(())
                    .and((0..3).contains(&ny).then_some(()))
                    .map(|()| &blocks[ny as usize][nx as usize])
            };
            Neighbours(Side::ALL.map(|s| {
                let (dx, dy) = s.offset();
                get(i64::from(dx), i64::from(dy))
            }))
        };
        let normals: Vec<Vec<VertexNormals>> = (0..3)
            .map(|y| {
                (0..3)
                    .map(|x| VertexNormals::compute(&blocks[y][x], &nb(x, y)))
                    .collect()
            })
            .collect();
        assert!(normals[1][1].is_final());
        for (y, row) in normals.iter().enumerate() {
            for (x, pair) in row.windows(2).enumerate() {
                for j in 0..GRID_SIDE {
                    let (a, b) = (pair[0].get(32, j), pair[1].get(0, j));
                    assert_eq!(a, b, "east seam ({x},{y}) j={j}");
                }
            }
        }
        for (y, rows) in normals.windows(2).enumerate() {
            for (x, (a, b)) in rows[0].iter().zip(&rows[1]).enumerate() {
                for i in 0..GRID_SIDE {
                    assert_eq!(a.get(i, 32), b.get(i, 0), "north seam ({x},{y}) i={i}");
                }
            }
        }
        // The shared corner of four blocks.
        let c = normals[0][0].get(32, 32);
        assert_eq!(c, normals[0][1].get(0, 32));
        assert_eq!(c, normals[1][0].get(32, 0));
        assert_eq!(c, normals[1][1].get(0, 0));
        // And they equal a normal computed in the interior of one big block over the same field.
        let big = TerrainBlock::new(
            &HeightGrid::from_fn(|i, j| field(16 + i as i64, 16 + j as i64)),
            &VertexFlags::default(),
        );
        let bn = VertexNormals::compute(&big, &Neighbours::NONE);
        assert_eq!(bn.get(16, 16), c);
    }

    #[test]
    fn provisional_sides_are_reported() {
        let a = block_at(0, 0);
        let e = block_at(1, 0);
        let n = VertexNormals::compute(&a, &Neighbours::NONE.with(Side::East, Some(&e)));
        assert!(!n.is_provisional(Side::East));
        assert!(n.is_provisional(Side::West));
        assert!(n.is_provisional(Side::North));
        assert!(n.is_provisional(Side::South));
    }
}
