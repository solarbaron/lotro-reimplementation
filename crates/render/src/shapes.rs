// SPDX-License-Identifier: GPL-3.0-or-later
//! Procedural meshes: placeholders for missing assets, debug geometry and
//! the synthetic golden-image scenes.
//!
//! Every shape has positions, normals, UVs and tangents, and follows the
//! [`MeshData`] conventions (counter-clockwise front faces, `v = 0` at the
//! top of a texture). Shapes are centred on the origin with `+Y` as their
//! "up"; rotate them for a `+Z`-up world. Segment counts are clamped to
//! `3..=`[`MAX_SEGMENTS`] so a bad argument cannot allocate without bound.

use std::f32::consts::{PI, TAU};

use glam::Vec3;

use crate::mesh::{Indices, MeshData};

/// Largest segment / ring / cell count accepted by the shape builders.
pub const MAX_SEGMENTS: u32 = 1024;

fn clamp_segments(n: u32, min: u32) -> u32 {
    n.clamp(min, MAX_SEGMENTS)
}

fn finite_or(v: f32, fallback: f32) -> f32 {
    if v.is_finite() { v } else { fallback }
}

/// Builder for grids of `(cols + 1) × (rows + 1)` vertices.
#[derive(Default)]
struct Grid {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl Grid {
    /// Add a grid. `f(u, v)` returns position and normal for `u, v` in
    /// `0..=1` (`v = 0` is the top row). Rows/columns are joined into
    /// counter-clockwise quads when `(u, v)` → position keeps the front on
    /// the side the normal points to with `u` rightwards and `v` downwards.
    fn add(
        &mut self,
        cols: u32,
        rows: u32,
        skip_degenerate: bool,
        f: impl Fn(f32, f32) -> (Vec3, Vec3),
    ) {
        let base = self.positions.len() as u32;
        for r in 0..=rows {
            for c in 0..=cols {
                let (u, v) = (c as f32 / cols as f32, r as f32 / rows as f32);
                let (p, n) = f(u, v);
                self.positions.push(p.to_array());
                self.normals.push(n.to_array());
                self.uvs.push([u, v]);
            }
        }
        let stride = cols + 1;
        for r in 0..rows {
            for c in 0..cols {
                let tl = base + r * stride + c;
                let (tr, bl, br) = (tl + 1, tl + stride, tl + stride + 1);
                // Pole rows of a sphere collapse one triangle of each quad.
                if !(skip_degenerate && r + 1 == rows) {
                    self.indices.extend_from_slice(&[tl, bl, br]);
                }
                if !(skip_degenerate && r == 0) {
                    self.indices.extend_from_slice(&[tl, br, tr]);
                }
            }
        }
    }

    fn finish(self, label: &str) -> MeshData {
        let mut m = MeshData {
            label: Some(label.to_owned()),
            positions: self.positions,
            normals: Some(self.normals),
            uvs: Some(self.uvs),
            indices: Indices::U32(self.indices),
            ..MeshData::default()
        };
        m.compute_tangents();
        m
    }
}

impl MeshData {
    /// Axis-aligned cube with half-extent `half`: 6 faces × 4 vertices,
    /// each face mapped to the full `0..1` UV square (upright on the side
    /// faces, `-Z` at the top of the `+Y` face).
    #[must_use]
    pub fn cube(half: f32) -> Self {
        let h = finite_or(half, 0.5);
        let mut g = Grid::default();
        // (normal, right, up): right × up = normal.
        for (n, right, up) in [
            (Vec3::Z, Vec3::X, Vec3::Y),
            (Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y),
            (Vec3::X, Vec3::NEG_Z, Vec3::Y),
            (Vec3::NEG_X, Vec3::Z, Vec3::Y),
            (Vec3::Y, Vec3::X, Vec3::NEG_Z),
            (Vec3::NEG_Y, Vec3::X, Vec3::Z),
        ] {
            g.add(1, 1, false, |u, v| {
                let p = (n + right * (u * 2.0 - 1.0) + up * (1.0 - v * 2.0)) * h;
                (p, n)
            });
        }
        g.finish("cube")
    }

    /// UV sphere of `radius` with poles on ±Y; `u` runs around the equator
    /// starting at `+Z` towards `+X`, `v` from the `+Y` pole (0) to the `-Y`
    /// pole (1).
    #[must_use]
    pub fn uv_sphere(radius: f32, segments: u32, rings: u32) -> Self {
        let r = finite_or(radius, 1.0);
        let mut g = Grid::default();
        g.add(
            clamp_segments(segments, 3),
            clamp_segments(rings, 2),
            true,
            |u, v| {
                let (st, ct) = (v * PI).sin_cos();
                let (sp, cp) = (u * TAU).sin_cos();
                let n = Vec3::new(st * sp, ct, st * cp);
                (n * r, n)
            },
        );
        g.finish("uv-sphere")
    }

    /// Flat `width × height` rectangle in the XY plane facing `+Z`,
    /// subdivided into `cols × rows` cells; `u` along `+X`, `v` along `-Y`.
    #[must_use]
    pub fn plane(width: f32, height: f32, cols: u32, rows: u32) -> Self {
        let (w, h) = (finite_or(width, 1.0), finite_or(height, 1.0));
        let mut g = Grid::default();
        g.add(
            clamp_segments(cols, 1),
            clamp_segments(rows, 1),
            false,
            |u, v| (Vec3::new((u - 0.5) * w, (0.5 - v) * h, 0.0), Vec3::Z),
        );
        g.finish("plane")
    }

    /// Open tube (no caps) of `radius` and `height` around the Y axis with
    /// outward normals; `u` runs around from `+Z` towards `+X`, `v` from
    /// the top (0) to the bottom (1). Its inside is made of back faces, so
    /// it shows the difference between one- and two-sided materials.
    #[must_use]
    pub fn tube(radius: f32, height: f32, segments: u32) -> Self {
        let (r, h) = (finite_or(radius, 0.5), finite_or(height, 1.0));
        let mut g = Grid::default();
        g.add(clamp_segments(segments, 3), 1, false, |u, v| {
            let (sp, cp) = (u * TAU).sin_cos();
            let n = Vec3::new(sp, 0.0, cp);
            (n * r + Vec3::Y * ((0.5 - v) * h), n)
        });
        g.finish("tube")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::MeshConfig;

    /// Every triangle's geometric normal agrees with its vertex normals
    /// (counter-clockwise front faces), and the tangent frame matches the
    /// UV directions.
    fn check(m: &MeshData) {
        let info = m.validate(&MeshConfig::default()).expect("valid");
        assert!(info.has_tangents && info.has_normals && info.has_uvs);
        let p = |i: u32| Vec3::from(m.positions[i as usize]);
        let normals = m.normals.as_ref().expect("normals");
        let n = |i: u32| Vec3::from(normals[i as usize]);
        let tris = m.indices.len() / 3;
        assert!(tris > 0);
        for t in 0..tris {
            let [a, b, c] = [0, 1, 2].map(|k| m.indices.get(t * 3 + k).expect("index"));
            let face = (p(b) - p(a)).cross(p(c) - p(a));
            if face.length() < 1e-9 {
                continue;
            }
            let avg = n(a) + n(b) + n(c);
            assert!(
                face.dot(avg) > 0.0,
                "{:?} triangle {t} faces inwards",
                m.label
            );
        }
        let tangents = m.tangents.as_ref().expect("tangents");
        for (i, t) in tangents.iter().enumerate() {
            let tv = Vec3::new(t[0], t[1], t[2]);
            assert!((tv.length() - 1.0).abs() < 1e-3, "{:?} {i}", m.label);
            assert!(tv.dot(Vec3::from(normals[i])).abs() < 1e-3);
        }
    }

    #[test]
    fn shapes_are_consistent() {
        check(&MeshData::cube(0.5));
        check(&MeshData::uv_sphere(1.0, 16, 8));
        check(&MeshData::plane(2.0, 1.0, 3, 2));
        check(&MeshData::tube(0.5, 1.0, 12));
        assert_eq!(MeshData::cube(1.0).positions.len(), 24);
        assert_eq!(MeshData::plane(1.0, 1.0, 3, 2).positions.len(), 12);
    }

    #[test]
    fn plane_tangent_frame_follows_uv() {
        let m = MeshData::plane(1.0, 1.0, 1, 1);
        for t in m.tangents.expect("tangents") {
            assert_eq!([t[0], t[1], t[2]], [1.0, 0.0, 0.0]);
            assert_eq!(t[3], -1.0, "v grows towards -Y");
        }
    }

    #[test]
    fn hostile_arguments_are_clamped() {
        let s = MeshData::uv_sphere(f32::NAN, u32::MAX, 0);
        assert!(s.positions.len() <= ((MAX_SEGMENTS + 1) * 3) as usize);
        assert!(s.validate(&MeshConfig::default()).is_ok());
        assert!(
            MeshData::plane(f32::INFINITY, 1.0, 0, 0)
                .validate(&MeshConfig::default())
                .is_ok()
        );
        assert!(
            MeshData::cube(f32::NAN)
                .validate(&MeshConfig::default())
                .is_ok()
        );
    }
}
