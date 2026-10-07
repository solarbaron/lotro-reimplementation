// SPDX-License-Identifier: GPL-3.0-or-later
//! Water for the controller (`collision.md` §12.3 step 7, `water.md` §7.4).
//!
//! **Tessera rule** (`water.md` §7.4, Decision): a mover is in water when its feet are
//! inside a liquid box; it swims (state 2) when the water surface lies more than its float
//! height above its feet, otherwise it wades (state 1, or 3 on steep support). The
//! controller adds (Decision, recorded as an OPEN note in `water.md` §7.4): a mover in water
//! without any support swims, whatever the depth.
//!
//! Water reaches the controller through [`WaterQuery`]. `tessera-world` may implement it
//! itself; [`LiquidWater`] implements it over any [`CollisionQuery`] from the water plane
//! and liquid boxes the sets carry (decision 0013 "Water"), and a [`CollisionSet`] or
//! [`CollisionView`] implements it directly with the same answers at a cost bounded by the
//! boxes over the mover's column.

use crate::geometry::{
    CollisionQuery, CollisionSet, CollisionView, LiquidBox, LiquidFlags, WaterPlane,
};

/// The water at a point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterSample {
    /// Height of the water surface above the point, in the query's frame (≥ the point's
    /// height).
    pub surface: f32,
    /// The flags of the liquid boxes containing the point.
    pub flags: LiquidFlags,
}

/// Where water is, in the frame of the collision query the controller steps against.
pub trait WaterQuery {
    /// The water containing `p`, or `None` when `p` is not in liquid.
    fn water_at(&self, p: [f32; 3]) -> Option<WaterSample>;
}

impl<T: WaterQuery + ?Sized> WaterQuery for &T {
    fn water_at(&self, p: [f32; 3]) -> Option<WaterSample> {
        (**self).water_at(p)
    }
}

/// No water anywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NoWater;

impl WaterQuery for NoWater {
    fn water_at(&self, _p: [f32; 3]) -> Option<WaterSample> {
        None
    }
}

/// Highest the surface search looks above a point, m (the deep zone of `water.md` §7.1 is
/// 1,600 units deep).
pub const SURFACE_SEARCH_RANGE: f32 = 2048.0;
/// Bisection rounds of the surface search (2,048 m / 2²⁴ ≈ 0.1 mm).
const SURFACE_BISECTIONS: usize = 24;
/// Offset used to test that a water plane's level is the top of the liquid, m.
const PLANE_PROBE: f32 = 0.001;
/// Most liquid boxes the column search of a [`CollisionSet`] or [`CollisionView`] keeps
/// (Tessera bound: a landblock's volume entity has three, `water.md` §7.1, and a column
/// lies in at most four landblocks). Boxes beyond it are not seen (logged at `debug`).
pub const MAX_COLUMN_BOXES: usize = 64;
/// Slack of the column filter, m: a box whose bounds miss the column by less still counts
/// (the bounds are f32, the containment test f64).
const COLUMN_SLACK: f32 = 1e-3;

/// The surface search shared by every implementation: `liquid(q)` gives the flags of the
/// liquid at `q`, `plane` the water plane over the point. At most 3 + 14 + 24 calls of
/// `liquid`: the test of the point, two plane probes, 14 doubling steps (0.25 m to
/// 2,048 m) and 24 bisections.
fn search(
    p: [f32; 3],
    plane: Option<WaterPlane>,
    liquid: impl Fn([f32; 3]) -> LiquidFlags,
) -> Option<WaterSample> {
    if !p.iter().all(|c| c.is_finite()) {
        return None;
    }
    let flags = liquid(p);
    if !flags.liquid {
        return None;
    }
    let at = |z: f32| [p[0], p[1], z];
    let wet = |z: f32| liquid(at(z)).liquid;
    // The landblock water plane, when the liquid ends at its level.
    if let Some(w) = plane
        && w.level >= p[2]
        && wet(w.level - PLANE_PROBE)
        && !wet(w.level + PLANE_PROBE)
    {
        return Some(WaterSample {
            surface: w.level,
            flags,
        });
    }
    // Upward search: double the step until the point is dry, then bisect.
    let mut wet_z = p[2];
    let mut step = 0.25_f32;
    let mut dry = None;
    while step <= SURFACE_SEARCH_RANGE {
        let z = p[2] + step;
        if !z.is_finite() {
            break;
        }
        if wet(z) {
            wet_z = z;
            step *= 2.0;
        } else {
            dry = Some(z);
            break;
        }
    }
    let Some(mut dry) = dry else {
        return Some(WaterSample {
            surface: wet_z.max(p[2]),
            flags,
        });
    };
    for _ in 0..SURFACE_BISECTIONS {
        let mid = 0.5 * wet_z + 0.5 * dry;
        if mid <= wet_z || mid >= dry {
            break;
        }
        if wet(mid) {
            wet_z = mid;
        } else {
            dry = mid;
        }
    }
    Some(WaterSample {
        surface: wet_z,
        flags,
    })
}

/// [`WaterQuery`] over the liquid boxes of any [`CollisionQuery`]: a point is in water when
/// a liquid box with `Volume_IsLiquid` contains it (`water.md` §7.4). The surface is the
/// top of the liquid above the point: the water plane's level when the liquid ends there
/// (`water.md` §6.3), otherwise found by a bounded upward search of the boxes (doubling
/// steps, then bisection): at most 14 + 24 point tests plus the test of the point and two
/// plane probes, each a [`CollisionQuery::liquid_at`] over every box of the query. A
/// [`CollisionSet`] and a [`CollisionView`] are [`WaterQuery`]s themselves with the same
/// answers, testing only the boxes over the point's column.
#[derive(Debug, Clone, Copy)]
pub struct LiquidWater<'a, Q: ?Sized>(pub &'a Q);

impl<Q: CollisionQuery + ?Sized> WaterQuery for LiquidWater<'_, Q> {
    fn water_at(&self, p: [f32; 3]) -> Option<WaterSample> {
        if !p.iter().all(|c| c.is_finite()) {
            return None;
        }
        search(p, self.0.water_plane_at(p[0], p[1]), |q| {
            self.0.liquid_at(q)
        })
    }
}

/// The liquid boxes over one column (x, y) from a height up, with their sets' offsets.
struct Column<'a> {
    boxes: [Option<(&'a LiquidBox, [f32; 3])>; MAX_COLUMN_BOXES],
    len: usize,
    full: bool,
}

impl<'a> Column<'a> {
    fn new() -> Self {
        Self {
            boxes: [None; MAX_COLUMN_BOXES],
            len: 0,
            full: false,
        }
    }

    /// Adds the boxes of `set` (at `offset`) that may contain a point (p.x, p.y, z ≥ p.z).
    fn gather(&mut self, set: &'a CollisionSet, offset: [f32; 3], p: [f32; 3]) {
        let local = [p[0] - offset[0], p[1] - offset[1], p[2] - offset[2]];
        for b in set.liquid_boxes() {
            let Some(bb) = b.aabb() else { continue };
            let over = bb.min[0] - COLUMN_SLACK <= local[0]
                && local[0] <= bb.max[0] + COLUMN_SLACK
                && bb.min[1] - COLUMN_SLACK <= local[1]
                && local[1] <= bb.max[1] + COLUMN_SLACK
                && bb.max[2] + COLUMN_SLACK >= local[2];
            if !over {
                continue;
            }
            if let Some(slot) = self.boxes.get_mut(self.len) {
                *slot = Some((b, offset));
                self.len += 1;
            } else {
                self.full = true;
                break;
            }
        }
    }

    fn liquid_at(&self, q: [f32; 3]) -> LiquidFlags {
        self.boxes[..self.len]
            .iter()
            .flatten()
            .filter(|(b, o)| b.contains([q[0] - o[0], q[1] - o[1], q[2] - o[2]]))
            .fold(LiquidFlags::default(), |acc, (b, _)| acc.union(b.flags))
    }

    fn report(&self) {
        if self.full {
            tracing::debug!(
                limit = MAX_COLUMN_BOXES,
                "liquid boxes over one column beyond the limit are not seen"
            );
        }
    }
}

impl WaterQuery for CollisionSet {
    /// As [`LiquidWater`] over the set, testing only the boxes over the point's column (at
    /// most [`MAX_COLUMN_BOXES`]).
    fn water_at(&self, p: [f32; 3]) -> Option<WaterSample> {
        if !p.iter().all(|c| c.is_finite()) {
            return None;
        }
        let mut column = Column::new();
        column.gather(self, [0.0; 3], p);
        column.report();
        search(p, self.water_plane_at(p[0], p[1]), |q| column.liquid_at(q))
    }
}

impl WaterQuery for CollisionView<'_> {
    /// As [`LiquidWater`] over the view, testing only the boxes over the point's column (at
    /// most [`MAX_COLUMN_BOXES`], in set order).
    fn water_at(&self, p: [f32; 3]) -> Option<WaterSample> {
        if !p.iter().all(|c| c.is_finite()) {
            return None;
        }
        let mut column = Column::new();
        for (set, offset) in self.members() {
            column.gather(set, *offset, p);
        }
        column.report();
        search(p, CollisionQuery::water_plane_at(self, p[0], p[1]), |q| {
            column.liquid_at(q)
        })
    }
}
