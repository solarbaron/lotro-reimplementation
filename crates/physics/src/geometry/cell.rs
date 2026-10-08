// SPDX-License-Identifier: GPL-3.0-or-later
//! Cell volumes, cell placement and the point-in-cell lookup (`collision.md` §5.4, §5.8,
//! §5.8.1).
//!
//! A cell volume is the convex region where n · p + d ≥ 0 for every plane of the 0x01
//! record's plane list (§5.4). The client's point test accepts a point when every plane
//! gives at least −0.0002 ([`CELL_PLANE_TOLERANCE`]). A landblock-info cell places its
//! geometry with a position and an active (w, x, y, z) rotation in the landblock frame
//! (§5.8).
//!
//! The lookup of the cell that contains a landblock-frame point (§5.8.1): no cell beyond
//! |p|² > 7,593,750; candidates are the cells whose phantom (the convex hull of the hull
//! vertices, here its bounding box) overlaps the cube p ± 0.05; the point is in a candidate
//! when its volume test passes. When several cells contain the point the original's answer
//! depends on its broad phase; Tessera takes the **lowest cell number** (Tessera Decision
//! of §5.8.1). A point in no cell is outdoors.
//!
//! Point tests run in f64, so the 2 × 10⁻⁴ tolerance is honoured exactly for cells placed
//! far from the landblock origin.

use std::sync::Arc;

use tessera_formats::Did;
use tessera_formats::collision::{CellGeometry, CellPlane};
use tessera_formats::landblock::Cell;
use tessera_formats::setup::Quaternion;

use super::bvh::Bvh;
use super::math::{self, D3, d3};
use super::primitives::Aabb;
use crate::error::{PhysicsError, Result};

/// The client's point-in-volume tolerance: inside when every plane gives at least
/// −0.0002 (`collision.md` §5.4, §5.8.1; a client constant, decision 0012).
pub const CELL_PLANE_TOLERANCE: f64 = 0.0002;

/// Beyond this squared distance from the landblock origin no cell is looked up
/// (`collision.md` §5.8.1 step 1).
pub const CELL_LOOKUP_MAX_DISTANCE_SQ: f64 = 7_593_750.0;

/// Half the side of the broad-phase cube around the point (§5.8.1 step 2).
pub const CELL_BROAD_PHASE_HALF: f32 = 0.05;

/// Most planes a volume may have (the data has 5–28, `collision.md` §5.2; anything far
/// above is hostile).
pub const MAX_CELL_PLANES: usize = 256;

/// Most hull vertices kept per cell (the data has 8–56, `collision.md` §5.2).
pub const MAX_CELL_HULL: usize = 4096;

/// Most planes, summed over its cells, that one [`CellSet`] built by
/// [`super::CollisionSetBuilder`] may hold. Together with the hierarchy over the phantom
/// bounds this bounds the work of one point-in-cell lookup, which the controller runs every
/// step, whatever the data says.
pub const MAX_SET_CELL_PLANES: usize = 1 << 18;

/// Most hull vertices, summed over its cells, that [`super::CollisionSetBuilder`] transforms
/// for one set's phantom bounds.
pub const MAX_SET_CELL_HULL: usize = 1 << 20;

/// A convex cell volume in the cell frame (`collision.md` §5.4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CellVolume {
    planes: Vec<(D3, f64)>,
}

impl CellVolume {
    /// A volume from (unit normal, distance) planes. Planes with a non-finite value are
    /// refused (`InvalidInput`), as are more than [`MAX_CELL_PLANES`].
    ///
    /// # Errors
    ///
    /// [`PhysicsError::InvalidInput`] or [`PhysicsError::TooLarge`].
    pub fn new(planes: impl IntoIterator<Item = ([f32; 3], f32)>) -> Result<Self> {
        let mut out = Vec::new();
        for (n, d) in planes {
            if out.len() >= MAX_CELL_PLANES {
                return Err(PhysicsError::TooLarge {
                    what: "cell planes",
                    count: out.len() + 1,
                    limit: MAX_CELL_PLANES,
                });
            }
            if !math::bounded3(n) || !d.is_finite() || d.abs() > math::MAX_COORDINATE {
                return Err(PhysicsError::InvalidInput("cell plane is not finite"));
            }
            out.push((d3(n), f64::from(d)));
        }
        Ok(Self { planes: out })
    }

    /// The volume of a decoded 0x01 record's plane list.
    ///
    /// # Errors
    ///
    /// As [`CellVolume::new`].
    pub fn from_planes(planes: &[CellPlane]) -> Result<Self> {
        Self::new(planes.iter().map(|p| (p.normal, p.distance)))
    }

    /// Number of planes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.planes.len()
    }

    /// Whether the volume has no planes (it then contains every point).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.planes.is_empty()
    }

    /// The smallest n · p + d over all planes (+∞ without planes); `p` in the cell frame.
    #[must_use]
    pub fn min_signed_distance(&self, p: D3) -> f64 {
        self.planes
            .iter()
            .map(|(n, d)| math::dot(*n, p) + d)
            .fold(f64::INFINITY, f64::min)
    }

    /// The client's test: every plane gives at least −[`CELL_PLANE_TOLERANCE`], checked in
    /// list order and stopping at the first plane that fails (§5.4). `p` in the cell frame.
    #[must_use]
    pub fn contains(&self, p: D3) -> bool {
        self.planes
            .iter()
            .all(|(n, d)| math::dot(*n, p) + d >= -CELL_PLANE_TOLERANCE)
    }
}

/// How a cell is placed in its landblock: cell-frame v ↦ R · v + t (§5.8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellPlacement {
    /// t: the cell's `position`.
    pub position: [f32; 3],
    /// R: the cell's orientation, normalised (identity when absent).
    pub rotation: Quaternion,
}

impl Default for CellPlacement {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            rotation: Quaternion::IDENTITY,
        }
    }
}

impl CellPlacement {
    /// A placement; the rotation is normalised. `InvalidInput` for non-finite values or a
    /// zero rotation.
    ///
    /// # Errors
    ///
    /// [`PhysicsError::InvalidInput`].
    pub fn new(position: [f32; 3], rotation: Quaternion) -> Result<Self> {
        if !math::bounded3(position) {
            return Err(PhysicsError::InvalidInput("cell position is out of range"));
        }
        let rotation =
            rotation
                .normalized()
                .filter(|q| q.is_finite())
                .ok_or(PhysicsError::InvalidInput(
                    "cell rotation is not a rotation",
                ))?;
        Ok(Self { position, rotation })
    }

    /// The placement of a landblock-info cell. A cell whose position carries no x, y, z
    /// offset (two observed cells; `landblock.md` §4.4 OPEN) cannot be placed and is
    /// refused rather than put at the landblock origin.
    ///
    /// # Errors
    ///
    /// As [`CellPlacement::new`]; `InvalidInput` without an offset.
    pub fn of_cell(cell: &Cell) -> Result<Self> {
        let p = cell
            .position
            .offset()
            .ok_or(PhysicsError::InvalidInput("cell position has no offset"))?;
        Self::new(p, cell.rotation().unwrap_or(Quaternion::IDENTITY))
    }

    /// Cell frame → landblock frame, in f64.
    #[must_use]
    pub fn to_landblock(&self, v: D3) -> D3 {
        math::add(rotate_d(self.rotation, v, false), d3(self.position))
    }

    /// Landblock frame → cell frame: subtract t, rotate by the inverse of R (§5.8).
    #[must_use]
    pub fn to_cell(&self, p: D3) -> D3 {
        rotate_d(self.rotation, math::sub(p, d3(self.position)), true)
    }
}

/// Active rotation of `v` by unit `q` in f64 (`inverse`: by its conjugate).
pub(crate) fn rotate_d(q: Quaternion, v: D3, inverse: bool) -> D3 {
    let w = f64::from(q.w);
    let s = if inverse { -1.0 } else { 1.0 };
    let u = [s * f64::from(q.x), s * f64::from(q.y), s * f64::from(q.z)];
    // q v q* = (w² − u·u) v + 2 (u·v) u + 2 w (u × v)
    let (uu, uv, c) = (math::dot(u, u), math::dot(u, v), math::cross(u, v));
    std::array::from_fn(|i| (w * w - uu) * v[i] + 2.0 * uv * u[i] + 2.0 * w * c[i])
}

/// A cell placed in its landblock: number, geometry, placement, volume and the phantom's
/// bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedCell {
    /// The cell's number in its landblock-info record.
    pub index: u16,
    /// The 0x01 geometry it places.
    pub geometry: Did,
    /// The placement.
    pub placement: CellPlacement,
    /// The volume, cell frame.
    pub volume: Arc<CellVolume>,
    /// Bounds of the hull vertices in the landblock frame (the phantom's broad-phase box,
    /// §5.8.1). `None` when the geometry has no hull vertex: such a cell is never a
    /// candidate.
    pub phantom_bounds: Option<Aabb>,
    /// Number of hull vertices the phantom bounds were built from (§5.6); a
    /// [`super::CollisionSetBuilder`] charges it to the set's hull budget
    /// ([`MAX_SET_CELL_HULL`]).
    hull_count: usize,
}

impl PlacedCell {
    /// Places `geometry` as cell `index` with `placement`.
    ///
    /// # Errors
    ///
    /// [`PhysicsError::InvalidInput`] / [`PhysicsError::TooLarge`] for hostile planes or
    /// hull vertices.
    pub fn new(index: u16, geometry: &CellGeometry, placement: CellPlacement) -> Result<Self> {
        let volume = Arc::new(CellVolume::from_planes(&geometry.planes)?);
        Self::with_volume(index, geometry.did, volume, &geometry.hull, placement)
    }

    /// Places a cell from its parts (shared volume, hull vertices in the cell frame).
    ///
    /// # Errors
    ///
    /// As [`PlacedCell::new`].
    pub fn with_volume(
        index: u16,
        geometry: Did,
        volume: Arc<CellVolume>,
        hull: &[[f32; 3]],
        placement: CellPlacement,
    ) -> Result<Self> {
        if hull.len() > MAX_CELL_HULL {
            return Err(PhysicsError::TooLarge {
                what: "cell hull vertices",
                count: hull.len(),
                limit: MAX_CELL_HULL,
            });
        }
        let mut bounds = Aabb::EMPTY;
        let mut any = false;
        for &v in hull {
            if !math::bounded3(v) {
                return Err(PhysicsError::InvalidInput(
                    "cell hull vertex is out of range",
                ));
            }
            let p = placement.to_landblock(d3(v));
            bounds.grow([
                math::f32_down(p[0]),
                math::f32_down(p[1]),
                math::f32_down(p[2]),
            ]);
            bounds.grow([math::f32_up(p[0]), math::f32_up(p[1]), math::f32_up(p[2])]);
            any = true;
        }
        Ok(Self {
            index,
            geometry,
            placement,
            volume,
            phantom_bounds: any.then_some(bounds),
            hull_count: hull.len(),
        })
    }

    /// Number of hull vertices the cell was placed with (§5.6).
    #[must_use]
    pub fn hull_count(&self) -> usize {
        self.hull_count
    }

    /// Whether landblock-frame `p` is in this cell by the volume test alone (§5.4).
    #[must_use]
    pub fn contains(&self, p: [f32; 3]) -> bool {
        self.volume.contains(self.placement.to_cell(d3(p)))
    }
}

/// The cells of one landblock and the point-in-cell lookup (§5.8.1).
///
/// Cells are appended in any order; [`CellSet::finish`] sorts them by number (stably, so of
/// two cells with the same number the earlier one stays first) and builds a hierarchy over
/// their phantom bounds, so a lookup visits only the cells whose bounds overlap the query
/// cube. [`super::CollisionSetBuilder::build`] calls it. Every query gives the same answer
/// before `finish` (by scanning every cell).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CellSet {
    cells: Vec<PlacedCell>,
    /// Some cell was pushed after one with a higher number.
    unsorted: bool,
    /// The hierarchy over the phantom bounds (item *k* is `cells[k]`), built by `finish`.
    bvh: Option<Bvh>,
    /// Planes summed over the cells.
    planes: usize,
}

impl CellSet {
    /// An empty set (an outdoor-only landblock).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a cell (a duplicate number is kept as well; the lookup then prefers the
    /// earlier one). Constant time; the order is restored by [`CellSet::finish`].
    pub fn push(&mut self, cell: PlacedCell) {
        if self.cells.last().is_some_and(|c| c.index > cell.index) {
            self.unsorted = true;
        }
        self.planes = self.planes.saturating_add(cell.volume.len());
        self.bvh = None;
        self.cells.push(cell);
    }

    /// Sorts the cells by number (stable) and builds the lookup hierarchy. O(n log n).
    pub fn finish(&mut self) {
        if self.unsorted {
            self.cells.sort_by_key(|c| c.index);
            self.unsorted = false;
        }
        let boxes: Vec<Aabb> = self
            .cells
            .iter()
            .map(|c| c.phantom_bounds.unwrap_or(Aabb::EMPTY))
            .collect();
        self.bvh = Some(Bvh::build(&boxes));
    }

    /// The cells: by number after [`CellSet::finish`] (or when pushed in order), else in
    /// push order.
    #[must_use]
    pub fn cells(&self) -> &[PlacedCell] {
        &self.cells
    }

    /// The (first) cell with number `index`.
    #[must_use]
    pub fn get(&self, index: u16) -> Option<&PlacedCell> {
        if self.unsorted {
            return self.cells.iter().find(|c| c.index == index);
        }
        let at = self.cells.partition_point(|c| c.index < index);
        self.cells.get(at).filter(|c| c.index == index)
    }

    /// Number of cells.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Planes summed over the cells (the cost of the worst lookup).
    #[must_use]
    pub fn plane_count(&self) -> usize {
        self.planes
    }

    /// The cell that contains landblock-frame `p` (§5.8.1), or `None` (outdoors): the
    /// lowest-numbered cell whose phantom bounds overlap p ± 0.05 and whose volume test
    /// passes (of two with the same number, the one pushed first). `None` for a non-finite
    /// point or |p|² > 7,593,750.
    #[must_use]
    pub fn cell_at(&self, p: [f32; 3]) -> Option<&PlacedCell> {
        if !math::finite3(p) || math::len_sq(d3(p)) > CELL_LOOKUP_MAX_DISTANCE_SQ {
            return None;
        }
        let cube = Aabb {
            min: p.map(|c| c - CELL_BROAD_PHASE_HALF),
            max: p.map(|c| c + CELL_BROAD_PHASE_HALF),
        };
        let q = d3(p);
        let passes = |c: &PlacedCell| {
            c.phantom_bounds.is_some_and(|b| b.overlaps(&cube))
                && c.volume.contains(c.placement.to_cell(q))
        };
        match (&self.bvh, self.unsorted) {
            (Some(bvh), false) => {
                // Sorted: the lowest position among the passing candidates.
                let mut best: Option<usize> = None;
                bvh.query(&cube, |item| {
                    let k = item as usize;
                    if best.is_none_or(|b| k < b) && self.cells.get(k).is_some_and(passes) {
                        best = Some(k);
                    }
                });
                best.and_then(|k| self.cells.get(k))
            }
            (_, false) => self.cells.iter().find(|c| passes(c)),
            (_, true) => self
                .cells
                .iter()
                .enumerate()
                .filter(|(_, c)| passes(c))
                .min_by_key(|(k, c)| (c.index, *k))
                .map(|(_, c)| c),
        }
    }

    /// Approximate heap bytes (volumes shared between cells are counted once per cell).
    #[must_use]
    pub fn heap_size(&self) -> usize {
        self.cells.capacity() * std::mem::size_of::<PlacedCell>()
            + self
                .cells
                .iter()
                .map(|c| 32 * c.volume.len())
                .sum::<usize>()
            + self.bvh.as_ref().map_or(0, Bvh::heap_size)
    }
}
