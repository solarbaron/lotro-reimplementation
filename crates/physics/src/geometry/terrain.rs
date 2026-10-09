// SPDX-License-Identifier: GPL-3.0-or-later
//! Terrain and ceiling collision meshes (`collision.md` §6.1–§6.6).
//!
//! Every landblock gets a terrain mesh of all 2,048 grid triangles (§6.1), holes included
//! (§6.5: the client never leaves a triangle out, and Tessera collides with all of them as
//! the original does). A landblock with a ceiling also gets a ceiling mesh (§6.4) built
//! from all 1,089 ceiling samples, zeros included, with the quad diagonal from **bit 0** of
//! the vertex-flag byte and every triangle clockwise from above (facing down).
//!
//! Triangles are numbered as the client's trees number them (§6.2): quad (i, j) (i along X,
//! j along Y, 0–31) owns keys 2 · (32 · j + i) (the triangle **west** of the diagonal) and
//! that plus 1. [`GridMesh::client_triangles_near`] and [`GridMesh::client_plane_at`] are
//! the client's point queries that scenery placement must reproduce (§6.6, `scenery.md`
//! §7.4 steps 8 and 13).

use tessera_formats::LandblockData;
use tessera_formats::landblock::{
    GRID_SIDE, GRID_SPACING, GRID_TRIANGLE_COUNT, GridSurface, HeightGrid, LANDBLOCK_SIZE,
    QUAD_SIDE, VertexFlags, grid_triangle_quad, quad_triangles,
};

use super::primitives::{Aabb, Plane};

/// Grid triangles per landblock (2 per quad, 32 × 32 quads).
pub const GRID_TRIANGLES: usize = GRID_TRIANGLE_COUNT;

/// Grid samples per side (33).
const SIDE: usize = GRID_SIDE;

/// The largest x + r (or y + r) that still starts at quad 31 in the radius query is just
/// below 160: the client compares with 159.99998 (`collision.md` §6.6 step 2).
pub const RADIUS_QUERY_EDGE: f32 = 159.999_98;

/// The signed-distance tolerance of the radius query's diagonal side test (§6.6 step 5).
pub const RADIUS_QUERY_SIDE_TOLERANCE: f32 = 0.0002;

/// The terrain or ceiling collision mesh of one landblock: 1,089 vertices and the 2,048
/// grid triangles in key order, in the landblock frame (x, y from the south-west corner,
/// z the stored height × 2,000 / 65,535).
#[derive(Clone, PartialEq)]
pub struct GridMesh {
    surface: GridSurface,
    /// Vertex 33 · j + i at (5i, 5j, height(i, j)).
    vertices: Box<[[f32; 3]]>,
    /// Triangle `key` as three vertex numbers, in the surface's winding.
    triangles: Box<[[u16; 3]]>,
    bounds: Aabb,
}

impl std::fmt::Debug for GridMesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GridMesh")
            .field("surface", &self.surface)
            .field("bounds", &self.bounds)
            .finish_non_exhaustive()
    }
}

impl GridMesh {
    /// The mesh of `surface` from a height grid and the vertex flags (bit 7 picks the
    /// terrain diagonal, bit 0 the ceiling diagonal; §6.1, §6.4). Never fails: every grid
    /// value gives a finite height.
    #[must_use]
    pub fn from_grid(surface: GridSurface, heights: &HeightGrid, flags: &VertexFlags) -> Self {
        let mut vertices = vec![[0.0f32; 3]; SIDE * SIDE].into_boxed_slice();
        let mut bounds = Aabb::EMPTY;
        for j in 0..SIDE {
            for i in 0..SIDE {
                #[allow(clippy::cast_precision_loss)]
                let v = [
                    i as f32 * GRID_SPACING,
                    j as f32 * GRID_SPACING,
                    heights.height(i, j).unwrap_or(0.0),
                ];
                vertices[SIDE * j + i] = v;
                bounds.grow(v);
            }
        }
        let mut triangles = vec![[0u16; 3]; GRID_TRIANGLES].into_boxed_slice();
        for (key, slot) in triangles.iter_mut().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let Some((i, j, west)) = grid_triangle_quad(key as u16) else {
                continue;
            };
            let split = flags.surface_split_sw_ne(surface, i, j);
            // `quad_triangles` lists the west triangle second for the SW-NE diagonal and
            // first for the SE-NW one (`collision.md` §6.2, §14.4).
            let mut tri = quad_triangles(i, j, split)[usize::from(split == west)];
            // §6.1 stored corner order (BIN, C43): the same winding as `quad_triangles`, from
            // the table's first corner, which is `quad_triangles`' last for every triangle
            // but the north-east one of a SE-NW quad (key base + 1, diagonal bit clear).
            if split || west {
                let [a, b, c] = tri;
                tri = [c, a, b];
            }
            // §6.4: the ceiling has the same split with each triangle's corners reversed.
            if surface == GridSurface::Ceiling {
                tri.reverse();
            }
            #[allow(clippy::cast_possible_truncation)]
            {
                *slot = tri.map(|(a, b)| (SIDE * b + a) as u16);
            }
        }
        Self {
            surface,
            vertices,
            triangles,
            bounds,
        }
    }

    /// The terrain mesh of a landblock (§6.1). Every landblock has one.
    #[must_use]
    pub fn terrain(data: &LandblockData) -> Self {
        Self::from_grid(GridSurface::Terrain, &data.heights, &data.vertex_flags)
    }

    /// The ceiling mesh of a landblock with a ceiling (§6.4), else `None`.
    #[must_use]
    pub fn ceiling(data: &LandblockData) -> Option<Self> {
        let c = data.ceiling.as_ref()?;
        Some(Self::from_grid(
            GridSurface::Ceiling,
            &c.heights,
            &data.vertex_flags,
        ))
    }

    /// Terrain or ceiling.
    #[must_use]
    pub fn surface(&self) -> GridSurface {
        self.surface
    }

    /// Bounds of all 1,089 vertices.
    #[must_use]
    pub fn bounds(&self) -> Aabb {
        self.bounds
    }

    /// Vertex (i, j), 0 ≤ i, j ≤ 32.
    #[must_use]
    pub fn vertex(&self, i: usize, j: usize) -> Option<[f32; 3]> {
        (i < SIDE && j < SIDE).then(|| self.vertices[SIDE * j + i])
    }

    /// Triangle `key` as three vertex numbers 33 · j + i, in the surface's winding
    /// (counter-clockwise from above for the terrain, clockwise for the ceiling). `None`
    /// for a key ≥ 2,048.
    #[must_use]
    pub fn triangle_vertices(&self, key: u16) -> Option<[u16; 3]> {
        self.triangles.get(usize::from(key)).copied()
    }

    /// Triangle `key` as three positions.
    #[must_use]
    pub fn triangle(&self, key: u16) -> Option<[[f32; 3]; 3]> {
        let t = self.triangle_vertices(key)?;
        Some(t.map(|v| self.vertices[usize::from(v)]))
    }

    /// The plane of triangle `key` by its winding: n.z > 0 for the terrain (n.z = 1 on
    /// flat ground), n.z < 0 for the ceiling. `None` for a key ≥ 2,048 (grid triangles are
    /// never degenerate: their horizontal projection has area 12.5).
    #[must_use]
    pub fn plane(&self, key: u16) -> Option<Plane> {
        let [a, b, c] = self.triangle(key)?;
        Plane::from_triangle(a, b, c)
    }

    /// All triangles as (key, positions), in key order.
    pub fn triangles(&self) -> impl Iterator<Item = (u16, [[f32; 3]; 3])> + '_ {
        self.triangles.iter().enumerate().map(|(k, t)| {
            #[allow(clippy::cast_possible_truncation)]
            (k as u16, t.map(|v| self.vertices[usize::from(v)]))
        })
    }

    /// The client's radius query (`collision.md` §6.6, used by scenery placement's
    /// clearance step, `scenery.md` §7.4 step 13): the keys of the grid triangles it
    /// collects near block-local (x, y) within horizontal radius `r`, in collection order
    /// (i outer, j inner; base key before base + 1). Reproduces the client's quirk: steps
    /// 4–5 always test the south-west/north-east diagonal, whatever bit 7 says, so away
    /// from the diagonal it collects the triangle with key *base* + 1 on the north-west
    /// side and *base* on the south-east side.
    ///
    /// Computed in f32 like the client's constants (159.99998, 0.0002). A NaN input or a
    /// negative or NaN radius collects nothing. At most 2,048 keys.
    #[must_use]
    pub fn client_keys_near(x: f32, y: f32, r: f32) -> Vec<u16> {
        let mut out = Vec::new();
        if !(x.is_finite() && y.is_finite() && r.is_finite()) || r < 0.0 {
            return out;
        }
        // Step 1.
        if x - r >= LANDBLOCK_SIZE || y - r >= LANDBLOCK_SIZE || x + r < 0.0 || y + r < 0.0 {
            return out;
        }
        // Step 2.
        let range = |c: f32| {
            let lo = if c - r <= 0.0 {
                0
            } else {
                quad_index((c - r) / GRID_SPACING)
            };
            let hi = if c + r >= RADIUS_QUERY_EDGE {
                QUAD_SIDE - 1
            } else {
                quad_index((c + r) / GRID_SPACING)
            };
            (lo, hi)
        };
        let (i0, i1) = range(x);
        let (j0, j1) = range(y);
        let r2 = r * r;
        let inv_sqrt2 = std::f32::consts::FRAC_1_SQRT_2;
        for i in i0..=i1 {
            for j in j0..=j1 {
                #[allow(clippy::cast_precision_loss)]
                let (x0, y0) = (i as f32 * GRID_SPACING, j as f32 * GRID_SPACING);
                // Step 3: distance to the square [x0, x0 + 5] × [y0, y0 + 5].
                let dx = axis_gap(x, x0, x0 + GRID_SPACING);
                let dy = axis_gap(y, y0, y0 + GRID_SPACING);
                if dx * dx + dy * dy > r2 {
                    continue;
                }
                #[allow(clippy::cast_possible_truncation)]
                let base = (2 * (QUAD_SIDE * j + i)) as u16;
                // Step 4: distance to the segment (x0, y0)–(x0 + 5, y0 + 5).
                let (px, py) = (x - x0, y - y0);
                let t = ((px + py) / (2.0 * GRID_SPACING)).clamp(0.0, 1.0);
                let (ex, ey) = (px - t * GRID_SPACING, py - t * GRID_SPACING);
                if ex * ex + ey * ey <= r2 {
                    out.push(base);
                    out.push(base + 1);
                    continue;
                }
                // Step 5.
                let s = (py - px) * inv_sqrt2;
                out.push(if s < -RADIUS_QUERY_SIDE_TOLERANCE {
                    base
                } else {
                    base + 1
                });
            }
        }
        out
    }

    /// [`GridMesh::client_keys_near`] with each collected triangle's plane (unit normal,
    /// n.z > 0 on the terrain; §6.6).
    #[must_use]
    pub fn client_triangles_near(&self, x: f32, y: f32, r: f32) -> Vec<(u16, Plane)> {
        Self::client_keys_near(x, y, r)
            .into_iter()
            .filter_map(|k| Some((k, self.plane(k)?)))
            .collect()
    }

    /// The triangles whose horizontal projection actually meets the disc of radius `r`
    /// around (x, y) (Tessera Decision of §6.6 for every use other than reproducing the
    /// original's scenery placement), in key order. Nothing for NaN input or a negative
    /// radius.
    #[must_use]
    pub fn keys_in_disc(&self, x: f32, y: f32, r: f32) -> Vec<u16> {
        let mut out = Vec::new();
        if !(x.is_finite() && y.is_finite() && r.is_finite()) || r < 0.0 {
            return out;
        }
        let range = |c: f32| {
            let lo = ((c - r) / GRID_SPACING).floor();
            let hi = ((c + r) / GRID_SPACING).floor();
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let clamp = |v: f32| v.clamp(0.0, (QUAD_SIDE - 1) as f32) as usize;
            // A point on the east or north edge (c = 160) lies on quad 31's triangles: let
            // lo reach 32 and clamp it, as the point queries do (32 becomes 31).
            (lo <= QUAD_SIDE as f32 && hi >= 0.0).then(|| (clamp(lo), clamp(hi)))
        };
        let (Some((i0, i1)), Some((j0, j1))) = (range(x), range(y)) else {
            return out;
        };
        let p = [f64::from(x), f64::from(y)];
        let r2 = f64::from(r) * f64::from(r);
        for j in j0..=j1 {
            for i in i0..=i1 {
                #[allow(clippy::cast_possible_truncation)]
                let base = (2 * (QUAD_SIDE * j + i)) as u16;
                for key in [base, base + 1] {
                    let Some(t) = self.triangle(key) else {
                        continue;
                    };
                    let t2 = t.map(|v| [f64::from(v[0]), f64::from(v[1])]);
                    if disc_meets_triangle_2d(p, r2, &t2) {
                        out.push(key);
                    }
                }
            }
        }
        out
    }

    /// The key of the triangle the client's terrain plane and height queries choose at
    /// block-local (x, y) (`scenery.md` §7.4 step 8, S25), or `None` outside the grid.
    ///
    /// i = ⌊x / 5⌋, j = ⌊y / 5⌋ (32 becomes 31), fx = x − 5i, fy = y − 5j. With bit 7 of
    /// sample (i, j) set the side is 1 when fx > fy (so a point on the diagonal gets the
    /// north-west triangle); with bit 7 clear it is 1 when fy > 4 − fx (the band
    /// 4 < fx + fy ≤ 5 gets the north-east triangle). Meaningful for the terrain; on a
    /// ceiling mesh the same rule is applied with bit 0.
    #[must_use]
    pub fn client_key_at(&self, x: f32, y: f32, flags: &VertexFlags) -> Option<u16> {
        if !(x.is_finite() && y.is_finite()) {
            return None;
        }
        let fi = (x / GRID_SPACING).floor();
        let fj = (y / GRID_SPACING).floor();
        #[allow(clippy::cast_precision_loss)]
        let last = QUAD_SIDE as f32;
        if !(0.0..=last).contains(&fi) || !(0.0..=last).contains(&fj) {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (i, j) = (
            (fi as usize).min(QUAD_SIDE - 1),
            (fj as usize).min(QUAD_SIDE - 1),
        );
        #[allow(clippy::cast_precision_loss)]
        let (fx, fy) = (x - i as f32 * GRID_SPACING, y - j as f32 * GRID_SPACING);
        let side = if flags.surface_split_sw_ne(self.surface, i, j) {
            fx > fy
        } else {
            fy > 4.0 - fx
        };
        #[allow(clippy::cast_possible_truncation)]
        Some((2 * (QUAD_SIDE * j + i) + usize::from(side)) as u16)
    }

    /// The client's terrain plane at (x, y) (`scenery.md` §7.4 step 8): the plane of the
    /// triangle [`GridMesh::client_key_at`] chooses, or [`Plane::NONE`] outside the grid.
    #[must_use]
    pub fn client_plane_at(&self, x: f32, y: f32, flags: &VertexFlags) -> Plane {
        self.client_key_at(x, y, flags)
            .and_then(|k| self.plane(k))
            .unwrap_or(Plane::NONE)
    }

    /// The client's terrain height at (x, y): [`GridMesh::client_plane_at`]'s height, or
    /// `None` outside the grid.
    #[must_use]
    pub fn client_height_at(&self, x: f32, y: f32, flags: &VertexFlags) -> Option<f32> {
        self.client_plane_at(x, y, flags).height_at(x, y)
    }

    /// The geometric triangle under (x, y) (`landblock.md` §3.11, Tessera's height rule
    /// for walking and the camera): with the SW-NE diagonal the south-east triangle when
    /// fx ≥ fy, with the SE-NW diagonal the north-east one when fx + fy > 5. x and y are
    /// clamped to the block; `None` for NaN.
    #[must_use]
    pub fn key_under(&self, x: f32, y: f32, flags: &VertexFlags) -> Option<u16> {
        if x.is_nan() || y.is_nan() {
            return None;
        }
        let gx = x.clamp(0.0, LANDBLOCK_SIZE) / GRID_SPACING;
        let gy = y.clamp(0.0, LANDBLOCK_SIZE) / GRID_SPACING;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (i, j) = (
            (gx.floor() as usize).min(QUAD_SIDE - 1),
            (gy.floor() as usize).min(QUAD_SIDE - 1),
        );
        #[allow(clippy::cast_precision_loss)]
        let (fx, fy) = (gx - i as f32, gy - j as f32);
        let split = flags.surface_split_sw_ne(self.surface, i, j);
        // West of the diagonal: SW-NE → fx < fy; SE-NW → fx + fy ≤ 1 (south-west half).
        let west = if split { fx < fy } else { fx + fy <= 1.0 };
        #[allow(clippy::cast_possible_truncation)]
        Some((2 * (QUAD_SIDE * j + i) + usize::from(!west)) as u16)
    }
}

/// ⌊v⌋ clamped to a quad index 0–31.
fn quad_index(v: f32) -> usize {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let i = v.floor().max(0.0) as usize;
    i.min(QUAD_SIDE - 1)
}

/// Distance from `c` to the interval [lo, hi] (0 inside).
fn axis_gap(c: f32, lo: f32, hi: f32) -> f32 {
    if c < lo {
        lo - c
    } else if c > hi {
        c - hi
    } else {
        0.0
    }
}

/// Whether the disc (centre `p`, squared radius `r2`) meets triangle `t` in 2D.
pub(crate) fn disc_meets_triangle_2d(p: [f64; 2], r2: f64, t: &[[f64; 2]; 3]) -> bool {
    let cross = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let s0 = cross(t[0], t[1], p);
    let s1 = cross(t[1], t[2], p);
    let s2 = cross(t[2], t[0], p);
    if (s0 >= 0.0 && s1 >= 0.0 && s2 >= 0.0) || (s0 <= 0.0 && s1 <= 0.0 && s2 <= 0.0) {
        return true;
    }
    (0..3).any(|k| {
        let (a, b) = (t[k], t[(k + 1) % 3]);
        let ab = [b[0] - a[0], b[1] - a[1]];
        let ap = [p[0] - a[0], p[1] - a[1]];
        let l2 = ab[0] * ab[0] + ab[1] * ab[1];
        let s = if l2 > 0.0 {
            ((ap[0] * ab[0] + ap[1] * ab[1]) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let e = [ap[0] - s * ab[0], ap[1] - s * ab[1]];
        e[0] * e[0] + e[1] * e[1] <= r2
    })
}
