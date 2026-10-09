// SPDX-License-Identifier: GPL-3.0-or-later
//! CPU side of a UI entry: structural checks, sanitising, the transform chain
//! and the clip chain (render.md §6.5, §6.6, §6.8). Pure functions, so the
//! rotation convention and the clip rules are testable without a GPU.

use super::{
    MAX_UI_CLIPS, MAX_UI_COORD, MAX_UI_ROTATIONS, MAX_UI_TEXTURE_DIM, UI_ANGLE_EPSILON,
    UI_NODE_SCALE_RANGE, UiAddressing, UiEntry, UiRefusal,
};

/// A 3-D affine map `p' = m · p + t` (row-major 3 × 3 plus translation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Affine3 {
    m: [[f64; 3]; 3],
    t: [f64; 3],
}

impl Affine3 {
    pub(crate) const IDENTITY: Self = Self {
        m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        t: [0.0; 3],
    };

    fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        let mut out = self.t;
        for (r, o) in out.iter_mut().enumerate() {
            *o += self.m[r][0] * p[0] + self.m[r][1] * p[1] + self.m[r][2] * p[2];
        }
        out
    }

    /// `self ∘ first`: apply `first`, then `self`.
    fn after(&self, first: &Self) -> Self {
        let mut m = [[0.0; 3]; 3];
        for (r, row) in m.iter_mut().enumerate() {
            for (c, v) in row.iter_mut().enumerate() {
                *v = (0..3).map(|k| self.m[r][k] * first.m[k][c]).sum();
            }
        }
        Self {
            m,
            t: self.apply(first.t),
        }
    }

    /// The map restricted to the plane z = 0, keeping x and y:
    /// `[[a, b, c], [d, e, f]]` with `x' = a·x + b·y + c`.
    fn plane_xy(&self) -> [[f64; 3]; 2] {
        [
            [self.m[0][0], self.m[0][1], self.t[0]],
            [self.m[1][0], self.m[1][1], self.t[1]],
        ]
    }
}

/// The rotation matrix of three angles in degrees (render.md §6.6): reduce
/// modulo 360, convert to radians, apply *x*, then *y*, then *z*.
pub(crate) fn rotation_matrix(angles: [f64; 3]) -> [[f64; 3]; 3] {
    let rad = angles.map(|a| a.rem_euclid(360.0).to_radians());
    let (sa, ca) = rad[0].sin_cos();
    let (sb, cb) = rad[1].sin_cos();
    let (sc, cc) = rad[2].sin_cos();
    // x: (x, y ca − z sa, y sa + z ca)
    let rx = [[1.0, 0.0, 0.0], [0.0, ca, -sa], [0.0, sa, ca]];
    // y: (x cb + z sb, y, −x sb + z cb)
    let ry = [[cb, 0.0, sb], [0.0, 1.0, 0.0], [-sb, 0.0, cb]];
    // z: (x cc + y sc, −x sc + y cc, z): positive z is counter-clockwise on a
    // y-down screen.
    let rz = [[cc, sc, 0.0], [-sc, cc, 0.0], [0.0, 0.0, 1.0]];
    mul3(&rz, &mul3(&ry, &rx))
}

fn mul3(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut out = [[0.0; 3]; 3];
    for (r, row) in out.iter_mut().enumerate() {
        for (c, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| a[r][k] * b[k][c]).sum();
        }
    }
    out
}

/// A clip item after merging, as the shader tests it: the half-open
/// rectangle `rect` (`[x0, y0, x1, y1]`) applied to `map · (q, 1)`, where
/// *q* is the fragment's position in the root's unrotated space.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UiClipMap {
    /// `[x0, y0, x1, y1]`.
    pub rect: [i32; 4],
    /// `[[a, b, c], [d, e, f]]`: tested position = (a·qx + b·qy + c,
    /// d·qx + e·qy + f).
    pub map: [[f64; 3]; 2],
}

/// The resolved geometry of one entry ([`UiEntry::geometry`]).
#[derive(Debug, Clone, PartialEq)]
pub struct UiGeometry {
    /// Screen positions of the displayed box's corners after the transform
    /// chain: top-left, top-right, bottom-left, bottom-right of the box in
    /// the root's unrotated space.
    pub corners: [[f64; 2]; 4],
    /// Whether the transform chain holds a rotation (after dropping angles
    /// within [`super::UI_ANGLE_EPSILON`] of 0 and non-finite ones).
    pub rotated: bool,
    /// The exact 1 : 1 path of render.md §6.6.
    pub one_to_one: bool,
    /// `false` when the entry draws nothing: zero projected area, an empty
    /// clip intersection, or opacity 0.
    pub drawable: bool,
    /// Intersection of the clip items that are screen rectangles
    /// (`[x0, y0, x1, y1]`), replaced by one scissor.
    pub scissor: Option<[i32; 4]>,
    /// The other clip items, at most [`super::MAX_UI_CLIPS`].
    pub clips: Vec<UiClipMap>,
    /// Opacity after sanitising, 0–1.
    pub opacity: f32,
    /// Alpha threshold after sanitising, 0–1.
    pub alpha_threshold: f32,
    /// Whether a NaN opacity or threshold or a non-finite angle was replaced.
    pub sanitised: bool,
}

/// The structural checks of render.md §6.5 that need no renderer.
pub(crate) fn check_structure(e: &UiEntry<'_>) -> Result<(), UiRefusal> {
    if e.canvas.iter().any(|&c| c == 0 || c > MAX_UI_TEXTURE_DIM) {
        return Err(UiRefusal::Canvas);
    }
    let d = e.dest;
    let in_range = |v: i64| v.abs() <= MAX_UI_COORD;
    if d.w == 0
        || d.h == 0
        || !in_range(i64::from(d.w))
        || !in_range(i64::from(d.h))
        || !in_range(i64::from(d.x))
        || !in_range(i64::from(d.y))
    {
        return Err(UiRefusal::Dest);
    }
    if let UiAddressing::Tile { node_scale } = e.addressing {
        let (lo, hi) = UI_NODE_SCALE_RANGE;
        if node_scale
            .iter()
            .any(|s| !s.is_finite() || *s < lo || *s > hi)
        {
            return Err(UiRefusal::NodeScale);
        }
    }
    if e.rotations.len() > MAX_UI_ROTATIONS {
        return Err(UiRefusal::Rotations);
    }
    let lim = MAX_UI_COORD as f32;
    if e.rotations
        .iter()
        .any(|r| r.centre.iter().any(|c| !c.is_finite() || c.abs() > lim))
    {
        return Err(UiRefusal::Rotations);
    }
    if e.clips.len() > MAX_UI_CLIPS {
        return Err(UiRefusal::TooManyClips);
    }
    if e.clips.iter().any(|c| c.space as usize > e.rotations.len()) {
        return Err(UiRefusal::ClipSpace);
    }
    Ok(())
}

impl UiEntry<'_> {
    /// Check the entry and resolve its transform and clip chains (render.md
    /// §6.5, §6.6, §6.8). Texture checks are not made here.
    ///
    /// # Errors
    ///
    /// The [`UiRefusal`] of the first structural check that fails.
    pub fn geometry(&self) -> Result<UiGeometry, UiRefusal> {
        check_structure(self)?;
        let mut sanitised = false;
        let mut finite_or_zero = |v: f32| {
            if v.is_nan() {
                sanitised = true;
                0.0
            } else {
                v
            }
        };
        let opacity = finite_or_zero(self.opacity).clamp(0.0, 1.0);
        let alpha_threshold = finite_or_zero(self.alpha_threshold).clamp(0.0, 1.0);

        // Prefix maps: prefix[k] applies the first k rotations.
        let n = self.rotations.len();
        let mut prefix = [Affine3::IDENTITY; MAX_UI_ROTATIONS + 1];
        let mut is_rot = [false; MAX_UI_ROTATIONS];
        for (i, r) in self.rotations.iter().enumerate() {
            let angles = r.angles.map(|a| {
                if a.is_finite() {
                    f64::from(a)
                } else {
                    sanitised = true;
                    0.0
                }
            });
            let eps = f64::from(UI_ANGLE_EPSILON);
            let step = if angles.iter().all(|a| a.abs() <= eps) {
                Affine3::IDENTITY
            } else {
                is_rot[i] = true;
                let m = rotation_matrix(angles);
                let c = [f64::from(r.centre[0]), f64::from(r.centre[1]), 0.0];
                let lin = Affine3 { m, t: [0.0; 3] };
                let mc = lin.apply(c);
                Affine3 {
                    m,
                    t: [c[0] - mc[0], c[1] - mc[1], c[2] - mc[2]],
                }
            };
            prefix[i + 1] = step.after(&prefix[i]);
        }
        let rotated = is_rot[..n].iter().any(|&r| r);
        let full = prefix[n];

        let d = self.dest;
        let (x0, y0) = (f64::from(d.x), f64::from(d.y));
        let (x1, y1) = (x0 + f64::from(d.w), y0 + f64::from(d.h));
        let corners = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)].map(|(x, y)| {
            let p = full.apply([x, y, 0.0]);
            [p[0], p[1]]
        });
        let e1 = [corners[1][0] - corners[0][0], corners[1][1] - corners[0][1]];
        let e2 = [corners[2][0] - corners[0][0], corners[2][1] - corners[0][1]];
        let area = (e1[0] * e2[1] - e1[1] * e2[0]).abs();
        let mut drawable = area > 1.0e-9 * f64::from(d.w) * f64::from(d.h) && opacity > 0.0;

        // Merge consecutive items of one space, then fold those that test the
        // screen position (no rotation from their space on) into a scissor.
        let mut merged: Vec<(u32, [i32; 4])> = Vec::with_capacity(self.clips.len());
        for c in self.clips {
            match merged.last_mut() {
                Some((k, r)) if *k == c.space => *r = intersect(*r, c.rect),
                _ => merged.push((c.space, c.rect)),
            }
        }
        let mut scissor: Option<[i32; 4]> = None;
        let mut clips = Vec::with_capacity(merged.len());
        for (k, rect) in merged {
            if rect[0] >= rect[2] || rect[1] >= rect[3] {
                drawable = false;
            }
            let k = k as usize;
            if is_rot[k.min(n)..n].iter().all(|&r| !r) {
                scissor = Some(scissor.map_or(rect, |s| intersect(s, rect)));
            } else {
                clips.push(UiClipMap {
                    rect,
                    map: prefix[k].plane_xy(),
                });
            }
        }
        if let Some(s) = scissor
            && (s[0] >= s[2] || s[1] >= s[3])
        {
            drawable = false;
        }

        let one_to_one = !rotated
            && match self.addressing {
                UiAddressing::Fit => d.w == self.canvas[0] && d.h == self.canvas[1],
                UiAddressing::Tile { node_scale } => node_scale == [1.0, 1.0],
            };
        Ok(UiGeometry {
            corners,
            rotated,
            one_to_one,
            drawable,
            scissor,
            clips,
            opacity,
            alpha_threshold,
            sanitised,
        })
    }
}

/// Intersection of two half-open rectangles `[x0, y0, x1, y1]` (may be
/// empty, with x0 ≥ x1 or y0 ≥ y1).
pub(crate) fn intersect(a: [i32; 4], b: [i32; 4]) -> [i32; 4] {
    [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture::TextureId;
    use crate::ui::{UiClip, UiRect, UiRotation};

    fn entry(dest: UiRect) -> UiEntry<'static> {
        UiEntry::new(TextureId::dangling(), [dest.w, dest.h], dest)
    }

    fn close(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
    }

    #[test]
    fn positive_z_turns_counter_clockwise_on_screen() {
        // A point right of the centre moves up (y down on screen).
        let m = rotation_matrix([0.0, 0.0, 90.0]);
        let p = Affine3 { m, t: [0.0; 3] }.apply([1.0, 0.0, 0.0]);
        assert!(close([p[0], p[1]], [0.0, -1.0]), "{p:?}");
    }

    #[test]
    fn x_tilt_of_60_halves_height_and_90_draws_nothing() {
        let rot = [UiRotation {
            centre: [50.0, 50.0],
            angles: [60.0, 0.0, 0.0],
        }];
        let e = entry(UiRect::new(0, 0, 100, 100)).with_rotations(&rot);
        let g = e.geometry().expect("valid");
        assert!(g.rotated && g.drawable && !g.one_to_one);
        let h = g.corners[2][1] - g.corners[0][1];
        assert!((h - 50.0).abs() < 1e-9, "{h}");
        assert!(close(g.corners[0], [0.0, 25.0]));
        let rot = [UiRotation {
            centre: [50.0, 50.0],
            angles: [90.0, 0.0, 0.0],
        }];
        let g = entry(UiRect::new(0, 0, 100, 100))
            .with_rotations(&rot)
            .geometry()
            .expect("valid");
        assert!(!g.drawable, "a tilt of 90 degrees has no area");
    }

    #[test]
    fn order_is_x_then_y_then_z() {
        // x by 90 then z by 90: (0, 1, 0) → x: (0, 0, 1) → z leaves z: (0, 0, 1).
        let m = rotation_matrix([90.0, 0.0, 90.0]);
        let p = Affine3 { m, t: [0.0; 3] }.apply([0.0, 1.0, 0.0]);
        assert!((p[0]).abs() < 1e-9 && p[1].abs() < 1e-9 && (p[2] - 1.0).abs() < 1e-9);
        // y by 90 maps x to −z.
        let m = rotation_matrix([0.0, 90.0, 0.0]);
        let p = Affine3 { m, t: [0.0; 3] }.apply([1.0, 0.0, 0.0]);
        assert!((p[2] + 1.0).abs() < 1e-9);
        // Angles reduce modulo 360.
        assert_eq!(
            rotation_matrix([0.0, 0.0, 450.0]).map(|r| r.map(|v| (v * 1e9).round())),
            rotation_matrix([0.0, 0.0, 90.0]).map(|r| r.map(|v| (v * 1e9).round()))
        );
    }

    #[test]
    fn chain_applies_own_rotation_first_and_projects_once() {
        // Own rotation z = 90 about (10, 0), then the root's z = 90 about the
        // origin: (11, 0) → (10, −1) → (−1, −10).
        let rot = [
            UiRotation::z([10.0, 0.0], 90.0),
            UiRotation::z([0.0, 0.0], 90.0),
        ];
        let g = entry(UiRect::new(11, 0, 1, 1))
            .with_rotations(&rot)
            .geometry()
            .expect("valid");
        assert!(close(g.corners[0], [-1.0, -10.0]), "{:?}", g.corners);
    }

    #[test]
    fn tiny_and_nan_angles_are_no_rotation() {
        let rot = [UiRotation {
            centre: [0.0, 0.0],
            angles: [0.0001, f32::NAN, -0.0002],
        }];
        let g = entry(UiRect::new(0, 0, 8, 8))
            .with_rotations(&rot)
            .geometry()
            .expect("valid");
        assert!(!g.rotated && g.one_to_one && g.sanitised);
        assert!(close(g.corners[3], [8.0, 8.0]));
    }

    #[test]
    fn opacity_and_threshold_are_sanitised() {
        let g = entry(UiRect::new(0, 0, 4, 4))
            .with_opacity(f32::NAN)
            .with_alpha_threshold(f32::NAN)
            .geometry()
            .expect("valid");
        assert_eq!(
            (g.opacity, g.alpha_threshold, g.drawable),
            (0.0, 0.0, false)
        );
        assert!(g.sanitised);
        let g = entry(UiRect::new(0, 0, 4, 4))
            .with_opacity(7.0)
            .with_alpha_threshold(-3.0)
            .geometry()
            .expect("valid");
        assert_eq!(
            (g.opacity, g.alpha_threshold, g.sanitised),
            (1.0, 0.0, false)
        );
    }

    #[test]
    fn unrotated_clip_chain_is_one_scissor() {
        let clips = [
            UiClip::new(UiRect::new(0, 0, 50, 50), 0),
            UiClip::new(UiRect::new(10, 10, 100, 100), 0),
        ];
        let g = entry(UiRect::new(0, 0, 64, 64))
            .with_clips(&clips)
            .geometry()
            .expect("valid");
        assert_eq!(g.scissor, Some([10, 10, 50, 50]));
        assert!(g.clips.is_empty() && g.drawable);
        // Disjoint items: nothing is drawn.
        let clips = [
            UiClip::new(UiRect::new(0, 0, 5, 5), 0),
            UiClip::new(UiRect::new(10, 10, 5, 5), 0),
        ];
        let g = entry(UiRect::new(0, 0, 64, 64))
            .with_clips(&clips)
            .geometry()
            .expect("valid");
        assert!(!g.drawable);
    }

    #[test]
    fn rotated_clip_items_keep_their_space() {
        // The root turns: a clip in space 0 is tested before the rotation, so
        // it is not a scissor; one in space 1 (screen) is.
        let rot = [UiRotation::z([50.0, 50.0], 30.0)];
        let clips = [
            UiClip::new(UiRect::new(0, 0, 100, 100), 0),
            UiClip::new(UiRect::new(0, 0, 200, 200), 1),
        ];
        let g = entry(UiRect::new(80, 80, 40, 40))
            .with_rotations(&rot)
            .with_clips(&clips)
            .geometry()
            .expect("valid");
        assert_eq!(g.clips.len(), 1);
        assert_eq!(g.clips[0].map, Affine3::IDENTITY.plane_xy());
        assert_eq!(g.scissor, Some([0, 0, 200, 200]));
        // An own rotation (index 0) before the window's (index 1): space 1
        // maps through the own rotation.
        let rot = [UiRotation::z([5.0, 5.0], 90.0), UiRotation::default()];
        let clips = [UiClip::new(UiRect::new(0, 0, 10, 10), 1)];
        let g = entry(UiRect::new(0, 0, 10, 10))
            .with_rotations(&rot)
            .with_clips(&clips)
            .geometry()
            .expect("valid");
        // The second rotation is the identity, so space 1 equals the screen.
        assert_eq!(g.scissor, Some([0, 0, 10, 10]));
        assert!(g.clips.is_empty());
    }

    #[test]
    fn one_to_one_rules() {
        let fit = entry(UiRect::new(3, 4, 32, 16));
        assert!(fit.geometry().expect("valid").one_to_one);
        let scaled = UiEntry {
            canvas: [16, 16],
            ..fit
        };
        assert!(!scaled.geometry().expect("valid").one_to_one);
        let tile = scaled.with_addressing(UiAddressing::Tile {
            node_scale: [1.0, 1.0],
        });
        assert!(tile.geometry().expect("valid").one_to_one);
        let tile = scaled.with_addressing(UiAddressing::Tile {
            node_scale: [1.5, 1.5],
        });
        assert!(!tile.geometry().expect("valid").one_to_one);
    }

    #[test]
    fn hostile_entries_are_refused() {
        let ok = entry(UiRect::new(0, 0, 4, 4));
        let bad = |e: UiEntry<'_>| e.geometry().err();
        assert_eq!(
            bad(UiEntry {
                canvas: [0, 4],
                ..ok
            }),
            Some(UiRefusal::Canvas)
        );
        assert_eq!(
            bad(UiEntry {
                canvas: [MAX_UI_TEXTURE_DIM + 1, 4],
                ..ok
            }),
            Some(UiRefusal::Canvas)
        );
        assert_eq!(
            bad(UiEntry {
                dest: UiRect::new(0, 0, 0, 4),
                ..ok
            }),
            Some(UiRefusal::Dest)
        );
        assert_eq!(
            bad(UiEntry {
                dest: UiRect::new(i32::MIN, 0, 4, 4),
                ..ok
            }),
            Some(UiRefusal::Dest)
        );
        for s in [0.0, -1.0, f32::NAN, f32::INFINITY, 1.0e-9] {
            assert_eq!(
                bad(ok.with_addressing(UiAddressing::Tile {
                    node_scale: [1.0, s]
                })),
                Some(UiRefusal::NodeScale)
            );
        }
        let rot = [UiRotation::default(); MAX_UI_ROTATIONS + 1];
        assert_eq!(bad(ok.with_rotations(&rot)), Some(UiRefusal::Rotations));
        let rot = [UiRotation {
            centre: [f32::NAN, 0.0],
            angles: [0.0; 3],
        }];
        assert_eq!(bad(ok.with_rotations(&rot)), Some(UiRefusal::Rotations));
        let clips = [UiClip::default(); MAX_UI_CLIPS + 1];
        assert_eq!(bad(ok.with_clips(&clips)), Some(UiRefusal::TooManyClips));
        let clips = [UiClip {
            rect: [0, 0, 1, 1],
            space: 1,
        }];
        assert_eq!(bad(ok.with_clips(&clips)), Some(UiRefusal::ClipSpace));
    }

    #[test]
    fn extreme_values_stay_finite() {
        let rot = [UiRotation {
            centre: [MAX_UI_COORD as f32; 2],
            angles: [f32::MAX, -f32::MAX, 1.0e30],
        }; MAX_UI_ROTATIONS];
        let clips = [UiClip {
            rect: [i32::MIN, i32::MIN, i32::MAX, i32::MAX],
            space: 3,
        }; MAX_UI_CLIPS];
        let e = UiEntry {
            dest: UiRect::new(
                -(MAX_UI_COORD as i32),
                MAX_UI_COORD as i32,
                MAX_UI_COORD as u32,
                1,
            ),
            canvas: [1, MAX_UI_TEXTURE_DIM],
            ..entry(UiRect::new(0, 0, 1, 1))
        }
        .with_rotations(&rot)
        .with_clips(&clips);
        let g = e.geometry().expect("valid");
        assert!(g.corners.iter().flatten().all(|v| v.is_finite()));
        assert!(g.clips.len() <= MAX_UI_CLIPS);
        assert!(
            g.clips
                .iter()
                .all(|c| c.map.iter().flatten().all(|v| v.is_finite()))
        );
    }
}
