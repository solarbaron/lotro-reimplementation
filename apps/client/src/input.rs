// SPDX-License-Identifier: GPL-3.0-or-later
//! Window-library-independent camera controls: held keys, mouse drags and the wheel are
//! turned into [`OrbitCamera`] input.

use tessera_render::OrbitCamera;

/// A camera action a key can be bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CameraAction {
    /// Rotate the view to the left.
    YawLeft,
    /// Rotate the view to the right.
    YawRight,
    /// Tilt the camera up (raise the eye).
    PitchUp,
    /// Tilt the camera down.
    PitchDown,
    /// Move closer.
    ZoomIn,
    /// Move away.
    ZoomOut,
}

impl CameraAction {
    const ALL: [Self; 6] = [
        Self::YawLeft,
        Self::YawRight,
        Self::PitchUp,
        Self::PitchDown,
        Self::ZoomIn,
        Self::ZoomOut,
    ];

    fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

/// Keyboard orbit speed in radians per second.
pub const KEY_ORBIT_SPEED: f32 = 1.6;
/// Keyboard zoom speed in wheel notches per second.
pub const KEY_ZOOM_SPEED: f32 = 6.0;
/// Largest time step applied in one update, so a stalled frame cannot spin the camera.
pub const MAX_DT: f32 = 0.25;

/// Accumulated controller state between frames.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CameraInput {
    held: u8,
    dragging: bool,
    last_cursor: Option<(f64, f64)>,
    drag: (f32, f32),
    wheel: f32,
}

impl CameraInput {
    /// A key bound to `action` was pressed or released.
    pub fn set_action(&mut self, action: CameraAction, pressed: bool) {
        if pressed {
            self.held |= action.bit();
        } else {
            self.held &= !action.bit();
        }
    }

    /// Whether `action` is held.
    #[must_use]
    pub fn is_held(&self, action: CameraAction) -> bool {
        self.held & action.bit() != 0
    }

    /// Release everything (e.g. on focus loss).
    pub fn release_all(&mut self) {
        self.held = 0;
        self.dragging = false;
        self.last_cursor = None;
    }

    /// The orbit mouse button changed state.
    pub fn set_dragging(&mut self, dragging: bool) {
        self.dragging = dragging;
    }

    /// The cursor moved to `(x, y)` (physical pixels).
    pub fn cursor_moved(&mut self, x: f64, y: f64) {
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        if let Some((lx, ly)) = self.last_cursor
            && self.dragging
        {
            self.drag.0 += (x - lx) as f32;
            self.drag.1 += (y - ly) as f32;
        }
        self.last_cursor = Some((x, y));
    }

    /// The cursor left the window.
    pub fn cursor_left(&mut self) {
        self.last_cursor = None;
    }

    /// Wheel notches (positive = away from the user = zoom in).
    pub fn wheel(&mut self, notches: f32) {
        if notches.is_finite() {
            self.wheel += notches.clamp(-100.0, 100.0);
        }
    }

    /// Apply everything accumulated since the last call and advance smoothing by `dt`.
    pub fn apply(&mut self, camera: &mut OrbitCamera, dt: f32) {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, MAX_DT)
        } else {
            0.0
        };
        let axis = |neg: CameraAction, pos: CameraAction| -> f32 {
            f32::from(u8::from(self.is_held(pos))) - f32::from(u8::from(self.is_held(neg)))
        };
        // Convert keyboard rates into the camera's "pixel" units.
        let yaw_rate = axis(CameraAction::YawRight, CameraAction::YawLeft) * KEY_ORBIT_SPEED * dt;
        let pitch_rate =
            axis(CameraAction::PitchDown, CameraAction::PitchUp) * KEY_ORBIT_SPEED * dt;
        let ys = camera.config.yaw_sensitivity;
        let ps = camera.config.pitch_sensitivity;
        let key_dx = if ys.abs() > f32::EPSILON {
            -yaw_rate / ys
        } else {
            0.0
        };
        let key_dy = if ps.abs() > f32::EPSILON {
            pitch_rate / ps
        } else {
            0.0
        };
        camera.orbit(self.drag.0 + key_dx, self.drag.1 + key_dy);
        let zoom =
            self.wheel + axis(CameraAction::ZoomOut, CameraAction::ZoomIn) * KEY_ZOOM_SPEED * dt;
        if zoom != 0.0 {
            camera.zoom(zoom);
        }
        camera.update(dt);
        self.drag = (0.0, 0.0);
        self.wheel = 0.0;
    }

    /// Every action, for key-binding tables.
    #[must_use]
    pub fn actions() -> &'static [CameraAction] {
        &CameraAction::ALL
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_render::{OrbitConfig, UpAxis};

    fn cam() -> OrbitCamera {
        OrbitCamera::new(OrbitConfig {
            up: UpAxis::Z,
            zoom_smoothing: 0.0,
            ..OrbitConfig::default()
        })
    }

    #[test]
    fn held_keys_rotate_and_release() {
        let mut c = cam();
        let mut i = CameraInput::default();
        i.set_action(CameraAction::YawLeft, true);
        i.apply(&mut c, 0.1);
        assert!(c.yaw > 0.1, "yaw {}", c.yaw);
        i.set_action(CameraAction::YawLeft, false);
        let yaw = c.yaw;
        i.apply(&mut c, 0.1);
        assert!((c.yaw - yaw).abs() < 1e-6);
    }

    #[test]
    fn drag_only_while_button_held() {
        let mut c = cam();
        let mut i = CameraInput::default();
        i.cursor_moved(10.0, 10.0);
        i.cursor_moved(50.0, 10.0);
        i.apply(&mut c, 0.0);
        assert!(c.yaw.abs() < 1e-6);
        i.set_dragging(true);
        i.cursor_moved(90.0, 10.0);
        i.apply(&mut c, 0.0);
        assert!(c.yaw < 0.0);
    }

    #[test]
    fn wheel_zooms_in_and_bad_input_is_ignored() {
        let mut c = cam();
        let d = c.distance;
        let mut i = CameraInput::default();
        i.wheel(2.0);
        i.wheel(f32::NAN);
        i.cursor_moved(f64::NAN, 0.0);
        i.apply(&mut c, f32::INFINITY);
        assert!(c.distance < d);
        assert!(c.eye().is_finite());
    }

    #[test]
    fn action_bits_are_distinct() {
        let mut seen = 0u8;
        for a in CameraInput::actions() {
            assert_eq!(seen & a.bit(), 0);
            seen |= a.bit();
        }
    }
}
