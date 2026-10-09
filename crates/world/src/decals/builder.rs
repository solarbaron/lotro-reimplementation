// SPDX-License-Identifier: GPL-3.0-or-later
//! The projection volume and the decal mesh (`decals.md` §4).
//!
//! All positions are in the landblock frame of the decal (x, y in metres from the
//! landblock's south-west corner, z up). The caller collects the receiving triangles
//! (§4.3: placed objects and cell geometry first, then terrain triangles, each with
//! positions and normals in the decal's frame) and hands them to [`build_mesh`], which
//! applies the water rule, the back-face test, the clipping, the triangulation, the texture
//! coordinates and the lift.

use tessera_formats::setup::Quaternion;

use super::DecalParams;

/// Lift of every output vertex along its normal, metres (§4.5 step 4; a client constant).
pub const LIFT: f32 = 0.008;
/// Above this many triangles the client logs a performance warning (§4.7).
pub const TRIANGLE_WARNING: usize = 8_000;
/// An `OrthoScale` component strictly below this gives an extent of 1.0 (§4.2).
pub const MIN_EXTENT: f32 = 0.1;
/// Weight of the projector direction in the mean normal (§4.6: minus 4 × Y′).
pub const MEAN_NORMAL_PULL: f32 = 4.0;
/// Lengths at or below this count as zero (§4.4, §4.6).
pub const ZERO_LENGTH: f32 = 1.0e-6;

type V3 = [f32; 3];

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn length(a: V3) -> f32 {
    dot(a, a).sqrt()
}
fn lerp(a: V3, b: V3, t: f32) -> V3 {
    add(a, scale(sub(b, a), t))
}
fn finite(a: V3) -> bool {
    a.iter().all(|c| c.is_finite())
}

/// The projection volume of a decal (§4.1, §4.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecalBox {
    /// The SED's position *p*.
    pub position: V3,
    /// The local axes X′, Y′, Z′ (the rotation applied to the unit axes).
    pub axes: [V3; 3],
    /// The centre *c* = *p* + (raw *s_y* / 2) × Y′.
    pub center: V3,
    /// The extents *e*: `OrthoScale` with every component below 0.1 replaced by 1.0.
    pub extents: V3,
    /// The raw `OrthoScale` *s* (scene bounds).
    pub raw_scale: V3,
}

impl DecalBox {
    /// The box of a decal at `position` with rotation `rotation` (stored (w, x, y, z),
    /// applied as an active rotation) and the effective `OrthoScale` `ortho_scale`.
    #[must_use]
    pub fn new(position: V3, rotation: Quaternion, ortho_scale: V3) -> Self {
        let q = rotation.normalized().unwrap_or(Quaternion::IDENTITY);
        let axes = [
            q.rotate([1.0, 0.0, 0.0]),
            q.rotate([0.0, 1.0, 0.0]),
            q.rotate([0.0, 0.0, 1.0]),
        ];
        let extents = ortho_scale.map(|c| {
            if c < MIN_EXTENT || !c.is_finite() {
                1.0
            } else {
                c
            }
        });
        let sy = if ortho_scale[1].is_finite() {
            ortho_scale[1]
        } else {
            0.0
        };
        Self {
            position,
            axes,
            center: add(position, scale(axes[1], sy / 2.0)),
            extents,
            raw_scale: ortho_scale,
        }
    }

    /// The projection direction +Y′ (surfaces face back along −Y′).
    #[must_use]
    pub fn projector(&self) -> V3 {
        self.axes[1]
    }

    /// Local coordinates of `p` relative to the centre along X′, Y′, Z′.
    #[must_use]
    pub fn local(&self, p: V3) -> V3 {
        let d = sub(p, self.center);
        [
            dot(d, self.axes[0]),
            dot(d, self.axes[1]),
            dot(d, self.axes[2]),
        ]
    }

    /// Whether `p` lies in the box within `tolerance` metres (§4.2 step 3).
    #[must_use]
    pub fn contains(&self, p: V3, tolerance: f32) -> bool {
        let l = self.local(p);
        (0..3).all(|i| l[i].abs() <= self.extents[i] / 2.0 + tolerance)
    }

    /// The eight corners of the box.
    #[must_use]
    pub fn corners(&self) -> [V3; 8] {
        std::array::from_fn(|i| {
            let s = |bit: usize| if i & (1 << bit) != 0 { 0.5 } else { -0.5 };
            let mut p = self.center;
            for (axis, k) in self.axes.iter().zip(0..3) {
                p = add(p, scale(*axis, s(k) * self.extents[k]));
            }
            p
        })
    }

    /// The axis-aligned box around the corners (the scene query of §4.3 step 1).
    #[must_use]
    pub fn query_aabb(&self) -> (V3, V3) {
        aabb(&self.corners())
    }

    /// The half-width of the horizontal square of the terrain search (§4.3 step 2): the
    /// length of the extents vector.
    #[must_use]
    pub fn terrain_half_width(&self) -> f32 {
        length(self.extents)
    }

    /// The scene bounds (§4.2): the box around *p* with the **raw** scale, local x ∈
    /// [−*s_x*/2, *s_x*/2], y ∈ [0, *s_y*], z ∈ [−*s_z*/2, *s_z*/2], as a world-space
    /// axis-aligned box.
    #[must_use]
    pub fn scene_bounds(&self) -> (V3, V3) {
        let s = self.raw_scale.map(|c| if c.is_finite() { c } else { 0.0 });
        let corners: [V3; 8] = std::array::from_fn(|i| {
            let x = if i & 1 != 0 { s[0] / 2.0 } else { -s[0] / 2.0 };
            let y = if i & 2 != 0 { s[1] } else { 0.0 };
            let z = if i & 4 != 0 { s[2] / 2.0 } else { -s[2] / 2.0 };
            add(
                self.position,
                add(
                    scale(self.axes[0], x),
                    add(scale(self.axes[1], y), scale(self.axes[2], z)),
                ),
            )
        });
        aabb(&corners)
    }

    /// Texture coordinates of `p` (§4.5 step 3): u = ((*P* − *c*) · X′) / *e_x* + 0.5,
    /// v = 0.5 − ((*P* − *c*) · Z′) / *e_z*.
    #[must_use]
    pub fn uv(&self, p: V3) -> [f32; 2] {
        let l = self.local(p);
        [l[0] / self.extents[0] + 0.5, 0.5 - l[2] / self.extents[2]]
    }
}

fn aabb(points: &[V3]) -> (V3, V3) {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for p in points {
        for ((l, h), v) in lo.iter_mut().zip(hi.iter_mut()).zip(p) {
            *l = l.min(*v);
            *h = h.max(*v);
        }
    }
    (lo, hi)
}

/// Where a receiving triangle comes from (§4.3, §4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReceiverKind {
    /// A placed object's or an interior cell's mesh (§4.3 step 1).
    Object,
    /// The full-resolution terrain mesh (§4.3 step 2).
    Terrain,
}

/// A vertex of a receiving triangle: position and normal in the decal's frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReceiverVertex {
    /// Position.
    pub position: V3,
    /// Normal (as collected, not renormalised).
    pub normal: V3,
}

/// A receiving triangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReceiverTriangle {
    /// Where it comes from.
    pub kind: ReceiverKind,
    /// The vertices in collection order (winding kept).
    pub vertices: [ReceiverVertex; 3],
}

/// One output vertex (§4.5): position after the lift, normal, texture coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecalVertex {
    /// Position (lifted by [`LIFT`] along the normal).
    pub position: V3,
    /// Interpolated normal (not renormalised).
    pub normal: V3,
    /// (u, v).
    pub uv: [f32; 2],
}

/// Counts of one build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BuildReport {
    /// Triangles handed in.
    pub candidates: usize,
    /// Triangles dropped by the back-face test.
    pub culled: usize,
    /// Triangles dropped as non-finite.
    pub invalid: usize,
    /// Source triangles that left at least one output triangle.
    pub kept: usize,
}

/// A built decal mesh (§4.5–§4.6).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DecalMesh {
    /// The output triangles as a list (three vertices each).
    pub vertices: Vec<DecalVertex>,
    /// The mesh centre *m* (centre of the vertices' axis-aligned box), when lit and
    /// non-empty.
    pub center: Option<V3>,
    /// The mean normal n̄ (§4.6), when lit and non-empty.
    pub mean_normal: Option<V3>,
    /// Counts.
    pub report: BuildReport,
}

impl DecalMesh {
    /// Number of output triangles.
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.vertices.len() / 3
    }
}

/// The back-face test of §4.4 for one triangle (already known to be tested).
fn faces_projector(params: &DecalParams, b: &DecalBox, v: &[ReceiverVertex; 3]) -> bool {
    let n = if params.use_precise_backface_culling {
        let c = cross(
            sub(v[1].position, v[0].position),
            sub(v[2].position, v[0].position),
        );
        let l = length(c);
        if l <= ZERO_LENGTH {
            [0.0; 3]
        } else {
            scale(c, 1.0 / l)
        }
    } else {
        v[0].normal
    };
    let limit = (params.backface_culling_angle * std::f32::consts::PI / 180.0).cos();
    dot(n, scale(b.projector(), -1.0)) >= limit
}

/// Clips a polygon against one plane: `dist` > 0 inside.
fn clip(poly: &[ReceiverVertex], dist: impl Fn(V3) -> f32) -> Vec<ReceiverVertex> {
    let mut out = Vec::with_capacity(poly.len() + 1);
    for (i, a) in poly.iter().enumerate() {
        let b = &poly[(i + 1) % poly.len()];
        let da = dist(a.position);
        let db = dist(b.position);
        if da >= 0.0 {
            out.push(*a);
        }
        if (da > 0.0 && db < 0.0) || (da < 0.0 && db > 0.0) {
            let t = da / (da - db);
            out.push(ReceiverVertex {
                position: lerp(a.position, b.position, t),
                normal: lerp(a.normal, b.normal, t),
            });
        }
    }
    out
}

/// Builds the decal mesh (§4.3 step 3, §4.4–§4.6) from the collected triangles, in
/// collection order. `water_height` is the height *h* of the water surface found at the
/// decal (only used when `StickToWater` is true, §4.3 step 3).
#[must_use]
pub fn build_mesh(
    params: &DecalParams,
    b: &DecalBox,
    triangles: impl IntoIterator<Item = ReceiverTriangle>,
    water_height: Option<f32>,
) -> DecalMesh {
    let mut mesh = DecalMesh::default();
    let water = water_height
        .filter(|h| params.stick_to_water && h.is_finite())
        .map(|h| h.min(b.center[2] + b.extents[2]));
    for tri in triangles {
        mesh.report.candidates += 1;
        let mut v = tri.vertices;
        if !v.iter().all(|x| finite(x.position) && finite(x.normal)) {
            mesh.report.invalid += 1;
            continue;
        }
        if let Some(w) = water {
            for x in &mut v {
                if x.position[2] < w {
                    x.position[2] = w;
                }
            }
        }
        let tested = params.use_backface_culling
            && (tri.kind == ReceiverKind::Object || params.use_landscape_backface_culling);
        if tested && !faces_projector(params, b, &v) {
            mesh.report.culled += 1;
            continue;
        }
        let mut poly: Vec<ReceiverVertex> = v.to_vec();
        for axis in 0..3 {
            let a = b.axes[axis];
            let half = b.extents[axis] / 2.0;
            let c = b.center;
            poly = clip(&poly, |p| half - dot(sub(p, c), a));
            if poly.len() < 3 {
                break;
            }
            poly = clip(&poly, |p| dot(sub(p, c), a) + half);
            if poly.len() < 3 {
                break;
            }
        }
        if poly.len() < 3 {
            continue;
        }
        mesh.report.kept += 1;
        let out: Vec<DecalVertex> = poly
            .iter()
            .map(|x| DecalVertex {
                position: add(x.position, scale(x.normal, LIFT)),
                normal: x.normal,
                uv: b.uv(x.position),
            })
            .collect();
        for i in 1..out.len() - 1 {
            mesh.vertices.extend([out[0], out[i], out[i + 1]]);
        }
    }
    if mesh.triangle_count() > TRIANGLE_WARNING {
        tracing::warn!(
            triangles = mesh.triangle_count(),
            "decal mesh above {TRIANGLE_WARNING} triangles"
        );
    }
    if params.is_lit() && !mesh.vertices.is_empty() {
        let positions: Vec<V3> = mesh.vertices.iter().map(|v| v.position).collect();
        let (lo, hi) = aabb(&positions);
        mesh.center = Some(scale(add(lo, hi), 0.5));
        let mut sum = [0.0; 3];
        for v in &mesh.vertices {
            sum = add(sum, v.normal);
        }
        #[allow(clippy::cast_precision_loss)]
        let mean = scale(sum, 1.0 / mesh.vertices.len() as f32);
        let n = sub(mean, scale(b.projector(), MEAN_NORMAL_PULL));
        let l = length(n);
        mesh.mean_normal = Some(if l <= ZERO_LENGTH {
            [0.0; 3]
        } else {
            scale(n, 1.0 / l)
        });
    }
    mesh
}
