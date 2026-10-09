// SPDX-License-Identifier: GPL-3.0-or-later
//! Placement of the horizon fill mesh (`far-view.md` §7.3): a render mesh from UNIQUEDB key
//! 0x95, centred on the eye and stretched down to where the landscape floor would appear at
//! 1.25 × *E*.

/// Units added to the lowest floor (§7.3 step 1, BIN).
pub const FLOOR_LIFT: f32 = 15.0;

/// Factor of *E* at which the floor is matched (§7.3 step 2, BIN).
pub const EXTENT_FACTOR: f32 = 1.25;

/// The mesh's bounds in its own frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshBounds {
    /// Minimum (*x*, *y*, *z*).
    pub min: [f32; 3],
    /// Maximum (*x*, *y*, *z*).
    pub max: [f32; 3],
}

/// Where the horizon mesh goes this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HorizonPlacement {
    /// Shift down by *k* (the mesh is placed at the eye's horizontal position, its own
    /// origin height minus *k*).
    pub drop: f32,
    /// Vertical scale (*H* + *k*) / *H*.
    pub scale_z: f32,
}

/// The floor *z*_f = min(*z*_ring, *z*_near) + 15 (§7.3 step 1). `None` when neither is
/// known or a value is not finite.
#[must_use]
pub fn floor(z_ring: Option<f32>, z_near: Option<f32>) -> Option<f32> {
    let z = match (
        z_ring.filter(|v| v.is_finite()),
        z_near.filter(|v| v.is_finite()),
    ) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => return None,
    };
    Some(z + FLOOR_LIFT)
}

/// The placement for eye height `eye_z`, floor `floor`, extent `extent` and mesh `bounds`
/// (§7.3 steps 2–5): *r* = max(max *x*, max *y*), *H* = max *z* − min *z*,
/// *δ* = max(0, eye *z* − *z*_f), *k* = *r* × *δ* / (1.25 *E*). `None` for a degenerate mesh
/// (*H* ≤ 0 or *r* ≤ 0) or non-finite inputs.
#[must_use]
pub fn placement(
    eye_z: f32,
    floor: f32,
    extent: f32,
    bounds: &MeshBounds,
) -> Option<HorizonPlacement> {
    let e = EXTENT_FACTOR * extent;
    let r = bounds.max[0].max(bounds.max[1]);
    let h = bounds.max[2] - bounds.min[2];
    let ok = [eye_z, floor, e, r, h].iter().all(|v| v.is_finite());
    if !ok || h <= 0.0 || r <= 0.0 || e <= 0.0 {
        return None;
    }
    let delta = (eye_z - floor).max(0.0);
    let k = r * delta / e;
    Some(HorizonPlacement {
        drop: k,
        scale_z: (h + k) / h,
    })
}
