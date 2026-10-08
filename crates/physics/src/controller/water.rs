// SPDX-License-Identifier: GPL-3.0-or-later
//! Water for the controller (`collision.md` §12.3 step 7, `water.md` §7.4).
//!
//! **Tessera rule** (`water.md` §7.4, Decision): a mover is in water when its feet are
//! inside a liquid box; it swims (state 2) when the water surface lies more than its float
//! height above its feet, otherwise it wades (state 1, or 3 on steep support); a mover in
//! water without any support swims, whatever the depth (the client's state too, W19). The
//! client's own state rule (`collision.md` §11.3.2, W19) gives state 1 on walkable support
//! however deep the water; Tessera keeps its float-height test because the client's buoyancy
//! is not known and the float line needs deep water to be a swimming state (`water.md` §7.4).
//!
//! Water reaches the controller through [`WaterQuery`]. [`LiquidWater`] implements it over
//! any [`CollisionQuery`] from the water plane and liquid boxes the sets carry (decision
//! 0013 "Water"), and a [`CollisionSet`] or [`CollisionView`] forwards to it. The surface
//! is the top of the contiguous liquid over the feet (the Tessera Decision confirmed in
//! `water.md` §7.4 "Water surface for the float line"), computed exactly from the boxes' spans over the mover's column: one
//! pass over the boxes (at most [`crate::geometry::MAX_LIQUID_BOXES`] per set) and a sort.
//! [`water_from_spans`] is the only implementation of that rule; `tessera-world`'s own
//! water query calls it too.

use crate::geometry::{
    CollisionQuery, CollisionSet, CollisionView, LiquidFlags, LiquidSpan, WaterPlane,
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

/// Liquid boxes whose spans over the mover's column are at most this far apart count as one
/// body of liquid, and a water plane whose level lies at most this far from the top of the
/// liquid is taken as its surface, m (Tessera Decision, `water.md` §7.4: the boxes are
/// placed in f32, so two boxes that meet in the data may miss each other by rounding).
pub const LIQUID_JOIN_TOLERANCE: f64 = 1e-3;

/// The water at `p` from the liquid spans over its column (`spans`, any order; they are
/// sorted in place) and the water plane over it (`water.md` §7.4, Tessera rule). The point
/// is in water when a liquid box contains it; the sample's flags are those of every box
/// containing it. The surface is the top of the **contiguous** liquid above the feet: the
/// spans of `Volume_IsLiquid` boxes are joined upward from the point, each span starting at
/// most [`LIQUID_JOIN_TOLERANCE`] above the top so far, so a dry gap between stacked boxes
/// ends the liquid. When a water plane lies over the point and its level is within the
/// tolerance of that top (the liquid ends at the plane, `water.md` §6.3), the plane's level
/// is the surface. The surface is never below the point: a level just below the feet (at
/// most the tolerance) gives the feet's height.
///
/// This is the one implementation of the rule: [`LiquidWater`] and `tessera-world`'s
/// column query both call it with the spans of their boxes.
#[must_use]
pub fn water_from_spans(
    p: [f32; 3],
    plane: Option<WaterPlane>,
    spans: &mut [LiquidSpan],
) -> Option<WaterSample> {
    if !p.iter().all(|c| c.is_finite()) {
        return None;
    }
    let z = f64::from(p[2]);
    let flags = spans
        .iter()
        .filter(|s| s.bottom <= z && z <= s.top)
        .fold(LiquidFlags::default(), |acc, s| acc.union(s.flags));
    if !flags.liquid {
        return None;
    }
    spans.sort_unstable_by(|a, b| a.bottom.total_cmp(&b.bottom));
    let mut top = z;
    for s in spans.iter().filter(|s| s.flags.liquid && s.top >= z) {
        if s.bottom > top + LIQUID_JOIN_TOLERANCE {
            break;
        }
        top = top.max(s.top);
    }
    if let Some(w) = plane
        && (f64::from(w.level) - top).abs() <= LIQUID_JOIN_TOLERANCE
    {
        top = f64::from(w.level);
    }
    #[allow(clippy::cast_possible_truncation)]
    let surface = (top as f32).max(p[2]);
    surface
        .is_finite()
        .then_some(WaterSample { surface, flags })
}

/// [`WaterQuery`] over the liquid boxes of any [`CollisionQuery`]: a point is in water when
/// a liquid box with `Volume_IsLiquid` contains it (`water.md` §7.4). The surface is the
/// top of the contiguous liquid above the point, or the water plane's level where that
/// liquid ends at the plane (`water.md` §6.3, §7.4), found exactly from the boxes' spans
/// over the point's column ([`CollisionQuery::liquid_spans`]): one pass over the boxes and
/// a sort of the spans. A [`CollisionSet`] and a [`CollisionView`] are [`WaterQuery`]s
/// themselves with the same answers.
#[derive(Debug, Clone, Copy)]
pub struct LiquidWater<'a, Q: ?Sized>(pub &'a Q);

impl<Q: CollisionQuery + ?Sized> WaterQuery for LiquidWater<'_, Q> {
    fn water_at(&self, p: [f32; 3]) -> Option<WaterSample> {
        if !p.iter().all(|c| c.is_finite()) {
            return None;
        }
        let mut spans = Vec::new();
        self.0.liquid_spans(p[0], p[1], &mut spans);
        water_from_spans(p, self.0.water_plane_at(p[0], p[1]), &mut spans)
    }
}

impl WaterQuery for CollisionSet {
    /// As [`LiquidWater`] over the set.
    fn water_at(&self, p: [f32; 3]) -> Option<WaterSample> {
        LiquidWater(self).water_at(p)
    }
}

impl WaterQuery for CollisionView<'_> {
    /// As [`LiquidWater`] over the view.
    fn water_at(&self, p: [f32; 3]) -> Option<WaterSample> {
        LiquidWater(self).water_at(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(bottom: f64, top: f64) -> LiquidSpan {
        LiquidSpan {
            bottom,
            top,
            flags: LiquidFlags {
                liquid: true,
                ..LiquidFlags::default()
            },
        }
    }

    /// Finding (`water.md` §7.4 OPEN): the doubling search jumped dry gaps between stacked
    /// boxes depending on its probe spacing. The surface is now the top of the contiguous
    /// liquid: boxes [−1, 0.6] and [0.8, 5] give 0.6, and so do [−1, 0.4] and [0.6, 5]
    /// (0.4).
    #[test]
    fn a_dry_gap_ends_the_liquid() {
        let at = |spans: &[LiquidSpan]| {
            water_from_spans([0.0; 3], None, &mut spans.to_vec()).map(|w| w.surface)
        };
        assert_eq!(at(&[span(-1.0, 0.6), span(0.8, 5.0)]), Some(0.6));
        assert_eq!(at(&[span(0.6, 5.0), span(-1.0, 0.4)]), Some(0.4));
        // Touching or overlapping boxes join, in any order.
        assert_eq!(at(&[span(0.6, 5.0), span(-1.0, 0.6)]), Some(5.0));
        assert_eq!(
            at(&[span(2.0, 7.0), span(-1.0, 3.0), span(-5.0, -2.0)]),
            Some(7.0)
        );
        // A gap within the tolerance joins.
        assert_eq!(at(&[span(-1.0, 0.6), span(0.6005, 5.0)]), Some(5.0));
        // Not in liquid.
        assert_eq!(at(&[span(0.5, 5.0)]), None);
        // A box without Volume_IsLiquid neither wets nor joins.
        let mut dry = span(0.6, 9.0);
        dry.flags.liquid = false;
        assert_eq!(at(&[span(-1.0, 0.6), dry]), Some(0.6));
    }

    /// The water plane is the surface where the liquid ends at its level (`water.md` §6.3).
    #[test]
    fn the_plane_is_the_surface_where_the_liquid_ends_there() {
        let plane = |level: f32| Some(WaterPlane::new(level, 1).expect("plane"));
        let at = |plane: Option<WaterPlane>, spans: &[LiquidSpan]| {
            water_from_spans([0.0, 0.0, -3.0], plane, &mut spans.to_vec()).map(|w| w.surface)
        };
        assert_eq!(at(plane(2.0), &[span(-10.0, 2.0004)]), Some(2.0));
        // Liquid above the plane (a placed volume on the lake) or ending below it.
        assert_eq!(at(plane(2.0), &[span(-10.0, 3.0)]), Some(3.0));
        assert_eq!(at(plane(2.0), &[span(-10.0, 1.0)]), Some(1.0));
    }

    /// Finding (consistency with `tessera-world`, `water.md` §7.4): the plane's level is the
    /// surface whenever it lies within 1 mm of the top of the liquid, also when it is just
    /// below the feet; the surface is then the feet's height (never below the point).
    #[test]
    fn a_plane_just_below_the_feet_still_snaps() {
        let plane = |level: f32| Some(WaterPlane::new(level, 1).expect("plane"));
        let at = |plane: Option<WaterPlane>, spans: &[LiquidSpan]| {
            water_from_spans([0.0; 3], plane, &mut spans.to_vec()).map(|w| w.surface)
        };
        // Top 0.0005, level 0.4 mm below the feet: the level, raised to the feet.
        assert_eq!(at(plane(-0.0004), &[span(-1.0, 0.0005)]), Some(0.0));
        // Level above the top, within 1 mm: the level.
        assert_eq!(at(plane(0.0012), &[span(-1.0, 0.0005)]), Some(0.0012));
        // Level more than 1 mm from the top: the top.
        assert_eq!(at(plane(-0.002), &[span(-1.0, 0.0005)]), Some(0.0005));
    }

    /// The review's worked case on a real set and view: boxes [−1, 0.6] and [0.8, 5] over
    /// feet at 0 give the surface 0.6 (the probes once found about 5 m).
    #[test]
    fn stacked_boxes_in_a_set_and_a_view() {
        use crate::geometry::{CollisionLimits, CollisionSetBuilder, LiquidBox, Transform};
        use tessera_formats::{LandblockId, LandblockKind};
        let liquid = LiquidFlags {
            liquid: true,
            deep: true,
            ..LiquidFlags::default()
        };
        let build = |boxes: &[(f32, f32)]| {
            let mut b = CollisionSetBuilder::new(CollisionLimits::default());
            b.set_landblock(LandblockId {
                kind: LandblockKind::Heightmap,
                region: 1,
                x: 10,
                y: 20,
            });
            for &(lo, hi) in boxes {
                b.add_liquid_box(
                    LiquidBox::new(
                        [0.0, 0.0, lo],
                        [160.0, 160.0, hi],
                        &Transform::IDENTITY,
                        liquid,
                    )
                    .expect("box"),
                )
                .expect("fits");
            }
            b.build()
        };
        let set = build(&[(-1.0, 0.6), (0.8, 5.0)]);
        let w = set.water_at([50.0, 50.0, 0.0]).expect("in water");
        assert_eq!(w.surface, 0.6);
        assert!(w.flags.deep);
        let set2 = build(&[(-1.0, 0.4), (0.6, 5.0)]);
        assert_eq!(
            set2.water_at([50.0, 50.0, 0.0]).map(|w| w.surface),
            Some(0.4)
        );
        // A view at another origin gives the same surface, moved into its frame.
        let mut view = CollisionView::with_origin([1600.0, 3200.0, 10.0]);
        view.push_landblock(&set).expect("keyed");
        let v = view.water_at([50.0, 50.0, -10.0]).expect("in water");
        assert!((v.surface - (0.6 - 10.0)).abs() < 1e-5, "{v:?}");
        assert!(v.flags.deep);
    }
}
