// SPDX-License-Identifier: GPL-3.0-or-later
//! Cameras: a plain perspective camera and a third-person orbit controller.
//!
//! All numbers in [`OrbitConfig::default`] are Tessera's own tuning for a
//! comfortable over-the-shoulder MMO camera. They are *not* values taken from
//! any other client and are expected to be adjusted from user settings.
//!
//! Depth convention: right-handed view space, wgpu clip space (`z` in `0..=1`).

use glam::{Mat4, Vec3};

/// Which world axis points "up".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpAxis {
    /// `+Y` up, ground plane is XZ.
    #[default]
    Y,
    /// `+Z` up, ground plane is XY.
    Z,
}

impl UpAxis {
    /// Unit vector for this axis.
    #[must_use]
    pub fn vector(self) -> Vec3 {
        match self {
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
        }
    }
}

/// Perspective projection parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Perspective {
    /// Vertical field of view in radians.
    pub fov_y: f32,
    /// Near clip distance (> 0).
    pub near: f32,
    /// Far clip distance (> near).
    pub far: f32,
}

impl Default for Perspective {
    fn default() -> Self {
        Self {
            fov_y: 60f32.to_radians(),
            near: 0.1,
            far: 5000.0,
        }
    }
}

impl Perspective {
    /// Smallest near plane distance accepted.
    pub const MIN_NEAR: f32 = 1e-5;
    /// Largest near plane distance accepted.
    pub const MAX_NEAR: f32 = 1e6;
    /// Largest accepted `far / near` ratio (depth precision is meaningless
    /// beyond this anyway).
    pub const MAX_DEPTH_RATIO: f32 = 1e7;

    /// Projection matrix. Degenerate inputs (NaN, infinities, zero aspect,
    /// inverted planes, absurd magnitudes) are sanitised: `near` is clamped
    /// to [`Self::MIN_NEAR`]`..=`[`Self::MAX_NEAR`], `far` to
    /// `near * 1.001 ..= near * MAX_DEPTH_RATIO`. The result is always finite.
    #[must_use]
    pub fn matrix(&self, aspect: f32) -> Mat4 {
        let aspect = if aspect.is_finite() && aspect > 1e-6 {
            aspect.min(1e6)
        } else {
            1.0
        };
        let min_fov = 1f32.to_radians();
        let max_fov = 179f32.to_radians();
        let fov = if self.fov_y.is_finite() {
            self.fov_y.clamp(min_fov, max_fov)
        } else {
            Self::default().fov_y
        };
        let near = if self.near.is_finite() {
            self.near.clamp(Self::MIN_NEAR, Self::MAX_NEAR)
        } else {
            Self::MIN_NEAR
        };
        let far = if self.far.is_finite() && self.far > near * 1.001 {
            self.far.min(near * Self::MAX_DEPTH_RATIO)
        } else {
            near * 1000.0
        };
        let m = glam::camera::rh::proj::directx::perspective(fov, aspect, near, far);
        if m.is_finite() {
            m
        } else {
            let d = Self::default();
            glam::camera::rh::proj::directx::perspective(d.fov_y, 1.0, d.near, d.far)
        }
    }
}

/// Normalise a vector of any finite magnitude (pre-scaling avoids the
/// `length_squared` overflow of huge vectors). `None` for zero / non-finite.
fn robust_normalize(v: Vec3) -> Option<Vec3> {
    if !v.is_finite() {
        return None;
    }
    let m = v.abs().max_element();
    if m <= 0.0 || !m.is_normal() {
        return None;
    }
    (v / m).try_normalize()
}

/// A look-at camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Eye position.
    pub eye: Vec3,
    /// Point being looked at.
    pub target: Vec3,
    /// World up vector.
    pub up: Vec3,
    /// Projection.
    pub projection: Perspective,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            eye: Vec3::new(0.0, 0.0, 5.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            projection: Perspective::default(),
        }
    }
}

impl Camera {
    /// World → view matrix. Never NaN: a degenerate eye/target, an up
    /// vector parallel to the view direction, or coordinates so large that
    /// the translation overflows are all sanitised (the latter falls back to
    /// the same orientation placed at the origin).
    #[must_use]
    pub fn view(&self) -> Mat4 {
        let dir = robust_normalize(self.target - self.eye).unwrap_or(Vec3::NEG_Z);
        let mut up = robust_normalize(self.up).unwrap_or(Vec3::Y);
        if dir.cross(up).length_squared() < 1e-8 {
            up = if dir.cross(Vec3::Y).length_squared() > 1e-8 {
                Vec3::Y
            } else {
                Vec3::Z
            };
        }
        let eye = if self.eye.is_finite() {
            self.eye
        } else {
            Vec3::ZERO
        };
        let m = glam::camera::rh::view::look_to_mat4(eye, dir, up);
        if m.is_finite() {
            m
        } else {
            glam::camera::rh::view::look_to_mat4(Vec3::ZERO, dir, up)
        }
    }

    /// Projection matrix for the given aspect ratio (width / height).
    #[must_use]
    pub fn projection(&self, aspect: f32) -> Mat4 {
        self.projection.matrix(aspect)
    }

    /// `projection * view`.
    #[must_use]
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.projection(aspect) * self.view()
    }
}

/// Tunables for [`OrbitCamera`]. Plain data so it can come from a settings file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitConfig {
    /// World up axis.
    pub up: UpAxis,
    /// Height of the orbit pivot above the followed position (e.g. head height).
    pub pivot_height: f32,
    /// Initial / reset distance from the pivot.
    pub distance: f32,
    /// Closest allowed zoom.
    pub min_distance: f32,
    /// Farthest allowed zoom.
    pub max_distance: f32,
    /// Initial pitch in radians (positive = camera above pivot, looking down).
    pub pitch: f32,
    /// Lowest pitch in radians (negative = below the pivot looking up).
    pub min_pitch: f32,
    /// Highest pitch in radians.
    pub max_pitch: f32,
    /// Radians of yaw per unit of horizontal input (e.g. per mouse pixel).
    pub yaw_sensitivity: f32,
    /// Radians of pitch per unit of vertical input.
    pub pitch_sensitivity: f32,
    /// Invert vertical mouse-look.
    pub invert_pitch: bool,
    /// Fractional distance change per zoom step (0.1 = 10 % per wheel notch).
    pub zoom_step: f32,
    /// Exponential smoothing rate (1/s) for distance changes; 0 = instant.
    pub zoom_smoothing: f32,
    /// Projection used by the produced camera.
    pub projection: Perspective,
}

impl Default for OrbitConfig {
    fn default() -> Self {
        Self {
            up: UpAxis::Y,
            pivot_height: 1.6,
            distance: 6.0,
            min_distance: 0.5,
            max_distance: 40.0,
            pitch: 15f32.to_radians(),
            min_pitch: (-60f32).to_radians(),
            max_pitch: 85f32.to_radians(),
            yaw_sensitivity: 0.005,
            pitch_sensitivity: 0.005,
            invert_pitch: false,
            zoom_step: 0.12,
            zoom_smoothing: 12.0,
            projection: Perspective::default(),
        }
    }
}

/// Third-person orbit camera following a target position.
///
/// Yaw 0 places the camera on the `+Z` side (Y-up) or `-Y` side (Z-up) of the
/// pivot; positive yaw rotates counter-clockwise seen from above.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitCamera {
    /// Tunables.
    pub config: OrbitConfig,
    /// Followed position (the character's feet).
    pub target: Vec3,
    /// Current yaw in radians (wrapped to `-PI..=PI`).
    pub yaw: f32,
    /// Current pitch in radians (clamped to the config range).
    pub pitch: f32,
    /// Current (smoothed) distance.
    pub distance: f32,
    /// Distance the zoom is easing toward.
    pub target_distance: f32,
}

impl OrbitCamera {
    /// New camera at the config's default pose.
    #[must_use]
    pub fn new(config: OrbitConfig) -> Self {
        let mut cam = Self {
            config,
            target: Vec3::ZERO,
            yaw: 0.0,
            pitch: config.pitch,
            distance: config.distance,
            target_distance: config.distance,
        };
        cam.sanitize();
        cam
    }

    fn distance_bounds(&self) -> (f32, f32) {
        let lo = if self.config.min_distance.is_finite() {
            self.config.min_distance.max(0.01)
        } else {
            0.01
        };
        let hi = if self.config.max_distance.is_finite() {
            self.config.max_distance.max(lo)
        } else {
            lo.max(100.0)
        };
        (lo, hi)
    }

    fn pitch_bounds(&self) -> (f32, f32) {
        let limit = 89.5f32.to_radians();
        let lo = if self.config.min_pitch.is_finite() {
            self.config.min_pitch.clamp(-limit, limit)
        } else {
            -limit
        };
        let hi = if self.config.max_pitch.is_finite() {
            self.config.max_pitch.clamp(lo, limit)
        } else {
            limit
        };
        (lo, hi)
    }

    /// Replace non-finite tunables with their defaults so one bad settings
    /// value cannot poison the pose (bounds are handled by the `*_bounds`
    /// helpers; the projection by [`Perspective::matrix`]).
    fn sanitize_config(&mut self) {
        let d = OrbitConfig::default();
        let c = &mut self.config;
        let finite_or = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
        c.pivot_height = finite_or(c.pivot_height, d.pivot_height);
        c.yaw_sensitivity = finite_or(c.yaw_sensitivity, d.yaw_sensitivity);
        c.pitch_sensitivity = finite_or(c.pitch_sensitivity, d.pitch_sensitivity);
        c.zoom_step = finite_or(c.zoom_step, d.zoom_step);
        c.zoom_smoothing = finite_or(c.zoom_smoothing, d.zoom_smoothing);
    }

    fn sanitize(&mut self) {
        self.sanitize_config();
        let (dlo, dhi) = self.distance_bounds();
        // NaN falls back; infinities are handled by the clamps below.
        let fix = |v: f32, fallback: f32| if v.is_nan() { fallback } else { v };
        self.distance = fix(self.distance, dlo).clamp(dlo, dhi);
        self.target_distance = fix(self.target_distance, dlo).clamp(dlo, dhi);
        let (plo, phi) = self.pitch_bounds();
        self.pitch = fix(self.pitch, 0.0).clamp(plo, phi);
        let yaw = if self.yaw.is_finite() { self.yaw } else { 0.0 };
        self.yaw =
            (yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        if !self.target.is_finite() {
            self.target = Vec3::ZERO;
        }
    }

    /// Apply mouse-look style input (e.g. pixel deltas).
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.sanitize_config();
        let sign = if self.config.invert_pitch { -1.0 } else { 1.0 };
        self.yaw -= dx * self.config.yaw_sensitivity;
        self.pitch += sign * dy * self.config.pitch_sensitivity;
        self.sanitize();
    }

    /// Zoom by `steps` wheel notches (positive = closer).
    pub fn zoom(&mut self, steps: f32) {
        self.sanitize_config();
        let factor = (1.0 - self.config.zoom_step).clamp(0.01, 0.99);
        let scale = factor.powf(steps);
        // NaN steps: no-op. Infinite scales are fine (clamped by sanitize).
        if !scale.is_nan() {
            self.target_distance *= scale;
        }
        self.sanitize();
    }

    /// Advance smoothing by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        self.sanitize_config();
        let rate = self.config.zoom_smoothing;
        if rate <= 0.0 || !rate.is_finite() || !dt.is_finite() {
            self.distance = self.target_distance;
        } else {
            let t = 1.0 - (-rate * dt.max(0.0)).exp();
            self.distance += (self.target_distance - self.distance) * t;
        }
        self.sanitize();
    }

    /// The point the camera orbits around and looks at.
    #[must_use]
    pub fn pivot(&self) -> Vec3 {
        // `config` is public and may have been edited since the last
        // sanitising call, so guard here too.
        let h = if self.config.pivot_height.is_finite() {
            self.config.pivot_height
        } else {
            OrbitConfig::default().pivot_height
        };
        let target = if self.target.is_finite() {
            self.target
        } else {
            Vec3::ZERO
        };
        let p = target + self.config.up.vector() * h;
        if p.is_finite() { p } else { target }
    }

    /// Unit vector from pivot to eye.
    #[must_use]
    pub fn offset_dir(&self) -> Vec3 {
        let (sp, cp) = self.pitch.sin_cos();
        let (sy, cy) = self.yaw.sin_cos();
        match self.config.up {
            UpAxis::Y => Vec3::new(cp * sy, sp, cp * cy),
            // The Y-up branch rotated +90 degrees about X (Y -> Z, Z -> -Y):
            // a proper rotation, so handedness (CCW yaw) is preserved.
            UpAxis::Z => Vec3::new(cp * sy, -cp * cy, sp),
        }
    }

    /// Eye position.
    #[must_use]
    pub fn eye(&self) -> Vec3 {
        self.pivot() + self.offset_dir() * self.distance
    }

    /// Horizontal forward direction of the camera (useful for movement input).
    #[must_use]
    pub fn forward_flat(&self) -> Vec3 {
        let up = self.config.up.vector();
        let d = -self.offset_dir();
        let flat = d - up * d.dot(up);
        flat.try_normalize().unwrap_or(match self.config.up {
            UpAxis::Y => Vec3::NEG_Z,
            UpAxis::Z => Vec3::Y,
        })
    }

    /// Snapshot as a plain [`Camera`].
    #[must_use]
    pub fn camera(&self) -> Camera {
        Camera {
            eye: self.eye(),
            target: self.pivot(),
            up: self.config.up.vector(),
            projection: self.config.projection,
        }
    }
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self::new(OrbitConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec4;

    #[test]
    fn perspective_is_finite_for_garbage() {
        let p = Perspective {
            fov_y: f32::NAN,
            near: -1.0,
            far: f32::INFINITY,
        };
        assert!(p.matrix(0.0).is_finite());
        assert!(p.matrix(f32::NAN).is_finite());
        // Huge finite magnitudes used to overflow to inf / NaN.
        for (near, far) in [
            (1e38, 5000.0),
            (1e36, 5000.0),
            (3.4e35, 5000.0),
            (1e30, 5000.0),
            (f32::MAX, f32::MAX),
            (0.1, f32::MAX),
            (1e-30, 1e30),
        ] {
            let p = Perspective {
                fov_y: 1.0,
                near,
                far,
            };
            for aspect in [1.0, f32::MAX, f32::MIN_POSITIVE] {
                assert!(
                    p.matrix(aspect).is_finite(),
                    "near={near} far={far} aspect={aspect}"
                );
            }
        }
    }

    #[test]
    fn camera_projects_target_to_center() {
        let cam = Camera {
            eye: Vec3::new(3.0, 2.0, 5.0),
            target: Vec3::new(1.0, 0.5, -1.0),
            ..Camera::default()
        };
        let clip = cam.view_proj(16.0 / 9.0) * cam.target.extend(1.0);
        let ndc = clip / clip.w;
        assert!(ndc.x.abs() < 1e-5 && ndc.y.abs() < 1e-5, "{ndc:?}");
        assert!((0.0..=1.0).contains(&ndc.z));
    }

    #[test]
    fn degenerate_camera_is_finite() {
        let cam = Camera {
            eye: Vec3::ONE,
            target: Vec3::ONE,
            up: Vec3::ZERO,
            ..Camera::default()
        };
        assert!(cam.view().is_finite());
        let straight_down = Camera {
            eye: Vec3::new(0.0, 10.0, 0.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            ..Camera::default()
        };
        assert!(straight_down.view().is_finite());
        // Huge finite coordinates used to overflow inside look_to.
        let huge = Camera {
            eye: Vec3::splat(3e38),
            target: Vec3::new(3e38, 3e38, 2e38),
            ..Camera::default()
        };
        assert!(huge.view().is_finite());
        let opposite = Camera {
            eye: Vec3::splat(-f32::MAX),
            target: Vec3::splat(f32::MAX),
            up: Vec3::splat(f32::MAX),
            ..Camera::default()
        };
        assert!(opposite.view().is_finite());
        assert!(opposite.view_proj(1.0).is_finite());
    }

    #[test]
    fn orbit_clamps_and_wraps() {
        let mut cam = OrbitCamera::default();
        cam.orbit(0.0, 1e9);
        assert!(cam.pitch <= cam.config.max_pitch + 1e-6);
        cam.orbit(0.0, -1e9);
        assert!(cam.pitch >= cam.config.min_pitch - 1e-6);
        cam.orbit(1e5, 0.0);
        assert!((-std::f32::consts::PI..=std::f32::consts::PI).contains(&cam.yaw));
        cam.zoom(1e6);
        cam.update(10.0);
        assert!((cam.distance - cam.config.min_distance).abs() < 1e-4);
        cam.zoom(-1e6);
        cam.update(10.0);
        assert!((cam.distance - cam.config.max_distance).abs() < 1e-3);
        cam.orbit(f32::NAN, f32::NAN);
        assert!(cam.eye().is_finite());
    }

    #[test]
    fn orbit_tolerates_non_finite_config() {
        let mut cam = OrbitCamera::new(OrbitConfig {
            zoom_step: f32::NAN,
            pivot_height: f32::NAN,
            yaw_sensitivity: f32::INFINITY,
            pitch_sensitivity: f32::NAN,
            zoom_smoothing: f32::NAN,
            ..OrbitConfig::default()
        });
        let before = cam.target_distance;
        // A NaN zoom step falls back to the default step: one notch closer,
        // not a snap to min_distance.
        cam.zoom(1.0);
        let expected = before * (1.0 - OrbitConfig::default().zoom_step);
        assert!(
            (cam.target_distance - expected).abs() < 1e-4,
            "{}",
            cam.target_distance
        );
        cam.orbit(10.0, 10.0);
        cam.update(0.016);
        assert!(cam.pivot().is_finite() && cam.eye().is_finite());
        assert!(cam.forward_flat().is_finite());

        // Editing the public config after construction is also tolerated.
        cam.config.pivot_height = f32::NAN;
        assert!(cam.pivot().is_finite() && cam.eye().is_finite());
        cam.zoom(f32::NAN);
        assert!(cam.target_distance.is_finite());
    }

    #[test]
    fn positive_yaw_is_counter_clockwise_for_both_up_axes() {
        for up in [UpAxis::Y, UpAxis::Z] {
            let dir = |yaw: f32| {
                let mut cam = OrbitCamera::new(OrbitConfig {
                    up,
                    pitch: 0.0,
                    ..OrbitConfig::default()
                });
                cam.yaw = yaw;
                cam.offset_dir()
            };
            let d0 = dir(0.0);
            let d1 = dir(std::f32::consts::FRAC_PI_2);
            let turn = d0.cross(d1).dot(up.vector());
            assert!(turn > 0.99, "{up:?}: cross·up = {turn}");
            // The same mouse input orbits the same way on both axes.
            let mut cam = OrbitCamera::new(OrbitConfig {
                up,
                ..OrbitConfig::default()
            });
            let before = cam.offset_dir();
            cam.orbit(-50.0, 0.0);
            assert!(
                before.cross(cam.offset_dir()).dot(up.vector()) > 0.0,
                "{up:?}"
            );
        }
        // Yaw 0 sits on +Z (Y-up) / -Y (Z-up).
        let z = OrbitCamera::new(OrbitConfig {
            up: UpAxis::Z,
            pitch: 0.0,
            ..OrbitConfig::default()
        });
        assert!((z.offset_dir() - Vec3::NEG_Y).length() < 1e-6);
    }

    #[test]
    fn orbit_default_pose_is_behind_and_above() {
        for up in [UpAxis::Y, UpAxis::Z] {
            let cam = OrbitCamera::new(OrbitConfig {
                up,
                ..OrbitConfig::default()
            });
            let eye = cam.eye();
            let pivot = cam.pivot();
            assert!(((eye - pivot).length() - cam.distance).abs() < 1e-4);
            assert!((eye - pivot).dot(up.vector()) > 0.0, "camera above pivot");
            let c = cam.camera();
            let clip = c.view_proj(1.0) * Vec4::from((pivot, 1.0));
            assert!((clip.x / clip.w).abs() < 1e-5 && (clip.y / clip.w).abs() < 1e-5);
            assert!(cam.forward_flat().dot(up.vector()).abs() < 1e-6);
        }
    }
}
