// SPDX-License-Identifier: GPL-3.0-or-later
//! Water a mover can be in: the landblock water plane and the liquid boxes of a collision
//! set (`water.md` §6.2–§6.3, §7.1–§7.4; decision 0013 "Water"; ARCHITECTURE §10.3).
//!
//! `tessera-world` decides a landblock's water (`water.md` §6.2) and places the monitor
//! boxes of its volume entities (§7.1, §7.3) when it builds the landblock's collision set on
//! the asset workers, and stores them in the set with
//! [`super::CollisionSetBuilder::set_water_plane`] and
//! [`super::CollisionSetBuilder::add_liquid_box`]. The controller's water step
//! (`collision.md` §12.3 step 7) reads them only from the resident sets.

use tessera_formats::PropertySet;
use tessera_formats::PropertyValue;

use super::cell::rotate_d;
use super::math::{self, d3};
use super::primitives::{Aabb, Transform};
use crate::error::{PhysicsError, Result};

/// A point is under the landblock water when z − h < −0.0002 (`water.md` §6.3; a client
/// constant, decision 0012).
pub const UNDER_WATER_TOLERANCE: f64 = 0.0002;

/// Most liquid boxes one collision set holds (a landblock has its volume entity's three and
/// a few per placed water volume; far more is hostile input).
pub const MAX_LIQUID_BOXES: usize = 4096;

/// A landblock's water plane (`water.md` §6.2 step 8, §6.3): z = `level` over the whole
/// 160 × 160 block, normal +Z.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterPlane {
    /// The water height *h* in the landblock frame.
    pub level: f32,
    /// The water type *t* (the block-map value that chose the water-table entry).
    pub water_type: u32,
}

impl WaterPlane {
    /// A plane; `InvalidInput` for a level that is not finite or beyond
    /// ±[`super::MAX_COORDINATE`].
    ///
    /// # Errors
    ///
    /// [`PhysicsError::InvalidInput`].
    pub fn new(level: f32, water_type: u32) -> Result<Self> {
        if !level.is_finite() || level.abs() > math::MAX_COORDINATE {
            return Err(PhysicsError::InvalidInput("water level is out of range"));
        }
        Ok(Self { level, water_type })
    }

    /// Whether height `z` is under the water: z − h < −[`UNDER_WATER_TOLERANCE`] (§6.3).
    #[must_use]
    pub fn is_under(&self, z: f32) -> bool {
        f64::from(z) - f64::from(self.level) < -UNDER_WATER_TOLERANCE
    }

    /// How far the surface lies above height `z` (negative above the water).
    #[must_use]
    pub fn depth_at(&self, z: f32) -> f32 {
        self.level - z
    }
}

/// The part of a vertical line inside one liquid box ([`LiquidBox::column_span`]), in the
/// frame of the query that reports it, with the box's flags.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidSpan {
    /// Lowest height inside the box.
    pub bottom: f64,
    /// Highest height inside the box.
    pub top: f64,
    /// The box's flags.
    pub flags: LiquidFlags,
}

/// The flags of a liquid box's property set (`water.md` §7.2). Water and lava share the
/// depth flags: `surface` is `Volume_WaterSurface` or `Volume_LavaSurface`, and so on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LiquidFlags {
    /// `Volume_IsLiquid`.
    pub liquid: bool,
    /// `Volume_IsLava`.
    pub lava: bool,
    /// `Volume_WaterSurface` / `Volume_LavaSurface`: the top 0.5 units.
    pub surface: bool,
    /// `Volume_WaterWading` / `Volume_LavaWading`.
    pub wading: bool,
    /// `Volume_WaterDeep` / `Volume_LavaDeep`.
    pub deep: bool,
}

impl LiquidFlags {
    /// Reads the flags from a monitor's property set (`Monitor_VolumePropertiesToApply`).
    /// The flags are boolean properties (`water.md` §7.2); a value of another type counts
    /// as not set (Tessera Decision, implementer OPEN in `water.md` §7.2).
    #[must_use]
    pub fn from_set(set: &PropertySet, pids: &LiquidPids) -> Self {
        let get = |pid: Option<u32>| {
            pid.and_then(|p| set.get(p))
                .is_some_and(|v| matches!(v, PropertyValue::Bool(true)))
        };
        Self {
            liquid: get(pids.is_liquid),
            lava: get(pids.is_lava),
            surface: get(pids.water_surface) || get(pids.lava_surface),
            wading: get(pids.water_wading) || get(pids.lava_wading),
            deep: get(pids.water_deep) || get(pids.lava_deep),
        }
    }

    /// Both sets of flags.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self {
            liquid: self.liquid || other.liquid,
            lava: self.lava || other.lava,
            surface: self.surface || other.surface,
            wading: self.wading || other.wading,
            deep: self.deep || other.deep,
        }
    }

    /// Whether no flag is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// The property IDs of the volume flags, looked up by name at run time (`water.md` §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LiquidPids {
    /// `Volume_IsLiquid`.
    pub is_liquid: Option<u32>,
    /// `Volume_IsLava`.
    pub is_lava: Option<u32>,
    /// `Volume_WaterSurface`.
    pub water_surface: Option<u32>,
    /// `Volume_WaterWading`.
    pub water_wading: Option<u32>,
    /// `Volume_WaterDeep`.
    pub water_deep: Option<u32>,
    /// `Volume_LavaSurface`.
    pub lava_surface: Option<u32>,
    /// `Volume_LavaWading`.
    pub lava_wading: Option<u32>,
    /// `Volume_LavaDeep`.
    pub lava_deep: Option<u32>,
}

impl LiquidPids {
    /// Looks every PID up with `pid_of(name)`.
    #[must_use]
    pub fn from_names(pid_of: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            is_liquid: pid_of("Volume_IsLiquid"),
            is_lava: pid_of("Volume_IsLava"),
            water_surface: pid_of("Volume_WaterSurface"),
            water_wading: pid_of("Volume_WaterWading"),
            water_deep: pid_of("Volume_WaterDeep"),
            lava_surface: pid_of("Volume_LavaSurface"),
            lava_wading: pid_of("Volume_LavaWading"),
            lava_deep: pid_of("Volume_LavaDeep"),
        }
    }

    /// Looks every PID up in the master property dictionary.
    #[must_use]
    pub fn from_dictionary(dict: &tessera_formats::PropertyDictionary) -> Self {
        Self::from_names(|n| dict.pid(n))
    }
}

/// A liquid box (`water.md` §7.1–§7.3): a monitor's axis-aligned box in its entity's frame
/// (`min`–`max`), placed by the entity's `transform` into the landblock frame (with scale 1
/// when `Monitor_ScaleWithEntity` is false), and its flags.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidBox {
    /// Box corner, entity frame.
    pub min: [f32; 3],
    /// Opposite corner, entity frame.
    pub max: [f32; 3],
    /// Entity frame → landblock frame (rotation normalised).
    pub transform: Transform,
    /// The flags of its property set.
    pub flags: LiquidFlags,
}

impl LiquidBox {
    /// A box; `InvalidInput` for non-finite or inverted corners or an invalid transform.
    ///
    /// # Errors
    ///
    /// [`PhysicsError::InvalidInput`].
    pub fn new(
        min: [f32; 3],
        max: [f32; 3],
        transform: &Transform,
        flags: LiquidFlags,
    ) -> Result<Self> {
        if !math::bounded3(min) || !math::bounded3(max) || (0..3).any(|a| min[a] > max[a]) {
            return Err(PhysicsError::InvalidInput("liquid box corners"));
        }
        let transform = transform
            .validated()
            .ok_or(PhysicsError::InvalidInput("liquid box transform"))?;
        let b = Self {
            min,
            max,
            transform,
            flags,
        };
        if b.aabb().is_none() {
            return Err(PhysicsError::InvalidInput("liquid box is out of range"));
        }
        Ok(b)
    }

    /// Whether landblock-frame `p` is inside (boundary included), in f64.
    #[must_use]
    pub fn contains(&self, p: [f32; 3]) -> bool {
        if !math::finite3(p) {
            return false;
        }
        let t = &self.transform;
        let local = rotate_d(t.rotation, math::sub(d3(p), d3(t.position)), true);
        (0..3).all(|a| {
            let s = f64::from(t.scale[a]);
            let v = local[a];
            v >= f64::from(self.min[a]) * s && v <= f64::from(self.max[a]) * s
        })
    }

    /// The heights z, in the landblock frame, at which (x, y, z) is inside the box
    /// (boundary included, as [`LiquidBox::contains`]): the box's span over the vertical
    /// line through (x, y), lowest first, in f64. `None` when the line misses the box.
    #[must_use]
    pub fn column_span(&self, x: f32, y: f32) -> Option<(f64, f64)> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        let t = &self.transform;
        // In the box frame the line is a + z · d.
        let a = rotate_d(
            t.rotation,
            math::sub([f64::from(x), f64::from(y), 0.0], d3(t.position)),
            true,
        );
        let d = rotate_d(t.rotation, [0.0, 0.0, 1.0], true);
        let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
        for k in 0..3 {
            let s = f64::from(t.scale[k]);
            let (min, max) = (f64::from(self.min[k]) * s, f64::from(self.max[k]) * s);
            if min > max {
                return None;
            }
            if d[k].abs() < 1e-12 {
                if a[k] < min || a[k] > max {
                    return None;
                }
            } else {
                let (t1, t2) = ((min - a[k]) / d[k], (max - a[k]) / d[k]);
                lo = lo.max(t1.min(t2));
                hi = hi.min(t1.max(t2));
            }
        }
        (lo.is_finite() && hi.is_finite() && lo <= hi).then_some((lo, hi))
    }

    /// Bounds in the landblock frame, or `None` when a placed corner is out of range.
    #[must_use]
    pub fn aabb(&self) -> Option<Aabb> {
        let corners = (0..8).map(|k: usize| {
            let c: [f32; 3] = std::array::from_fn(|a| {
                if k & (1 << a) == 0 {
                    self.min[a]
                } else {
                    self.max[a]
                }
            });
            self.transform.apply(c)
        });
        let b = Aabb::from_points(corners)?;
        (math::bounded3(b.min) && math::bounded3(b.max)).then_some(b)
    }
}
