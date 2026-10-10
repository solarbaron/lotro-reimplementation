// SPDX-License-Identifier: GPL-3.0-or-later
//! The turntable and zoom of the creation viewport (`character-screens.md` §2.1.3, §2.3,
//! BIN): the controls do not turn the figure; they change the turn angle *θ* and the zoom
//! fraction *t* by which the camera is put between the stage's base and zoom frames and
//! turned about the figure ([`super::scene::CameraRig`]).
//!
//! - Each hot click of a turn button sets the target of *θ* to the current *θ* ∓ 20°
//!   ([`Turn::Left`] −, [`Turn::Right`] +); *θ* eases towards its target, the remaining
//!   difference halving every 0.15 s, and counts as there within 0.0002.
//! - A drag across the viewport sets *θ* at once to its value at the start of the drag
//!   + (*x* − *x*₀) × (−0.5)°.
//! - The zoom buttons set *t* to 1 or 0 and move the camera from where it is to the frame
//!   for the new *t* in a timed move of *T* = ½ × (*d* / 3 + ½ × (2 − *c*)) seconds
//!   (*d* the distance between the start and end positions, *c* the cosine of the angle
//!   between their view directions), placed at the eased fraction
//!   ½ × sin((*u* − ½) × π) + ½ of the progress *u*, its field of view at *u*.
//! - The mouse wheel moves the target of *t* by 0.15 per notch (clamped to [0, 1]); *t*
//!   eases with a half-life of 0.1 s. Which notch direction zooms in was not traced
//!   (§2.3 OPEN): a notch away from the player zooms in (Tessera rule).
//! - The turn buttons are ghosted while a zoom move runs; zoom in while the target of *t*
//!   is at least 1 − 0.0002, zoom out while it is at most 0.0002.
//! - Setting the stage camera (a new stage, race, gender or selected character) puts *θ*
//!   and *t* back to 0 at once ([`Turntable::reset`]).
//!
//! During a zoom move the turn buttons are ghosted, and drags and wheel notches are
//! ignored (Tessera rule: the move's end frame is fixed when it starts).

use std::time::Instant;

use super::scene::{CameraFrame, CameraRig};

/// Turn per hot click, degrees.
pub const TURN_STEP: f32 = 20.0;
/// Half-life of the turn's easing, seconds.
pub const TURN_HALF_LIFE: f32 = 0.15;
/// Turn per pixel of drag, degrees.
pub const DRAG_DEGREES_PER_PIXEL: f32 = -0.5;
/// Zoom per wheel notch.
pub const WHEEL_STEP: f32 = 0.15;
/// Half-life of the wheel zoom's easing, seconds.
pub const ZOOM_HALF_LIFE: f32 = 0.1;
/// A target counts as reached within this difference; the zoom buttons ghost within it of
/// the ends.
pub const REACHED: f32 = 0.0002;

/// A turn button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turn {
    /// `CharGen_Turntable_Left_Button`: *θ* − 20°.
    Left,
    /// `CharGen_Turntable_Right_Button`: *θ* + 20°.
    Right,
}

impl Turn {
    fn step(self) -> f32 {
        match self {
            Self::Left => -TURN_STEP,
            Self::Right => TURN_STEP,
        }
    }
}

/// A value that eases towards a target, the remaining difference halving every
/// `half_life` seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Eased {
    value: f32,
    target: f32,
    from: f32,
    since: Option<Instant>,
    half_life: f32,
}

impl Eased {
    fn new(half_life: f32) -> Self {
        Self {
            value: 0.0,
            target: 0.0,
            from: 0.0,
            since: None,
            half_life,
        }
    }

    fn set_target(&mut self, target: f32, now: Instant) {
        if !target.is_finite() {
            return;
        }
        self.target = target;
        self.from = self.value;
        self.since = Some(now);
    }

    fn set_now(&mut self, v: f32) {
        if v.is_finite() {
            self.value = v;
            self.target = v;
            self.from = v;
            self.since = None;
        }
    }

    fn update(&mut self, now: Instant) {
        let Some(since) = self.since else {
            return;
        };
        let dt = now.saturating_duration_since(since).as_secs_f32();
        let left = (self.target - self.from) * 0.5f32.powf(dt / self.half_life);
        if left.abs() <= REACHED || !left.is_finite() {
            self.set_now(self.target);
        } else {
            self.value = self.target - left;
        }
    }
}

/// A zoom button's timed camera move.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ZoomMove {
    start: CameraFrame,
    start_fov: f32,
    end_t: f32,
    started: Instant,
    duration: f32,
    u: f32,
}

/// The duration of a timed zoom move between two camera frames: ½ × (*d* / 3 + ½ × (2 −
/// *c*)) seconds (§2.3).
#[must_use]
pub fn move_duration(from: &CameraFrame, to: &CameraFrame) -> f32 {
    let d = from.position.distance(to.position);
    let c = from.forward().dot(to.forward()).clamp(-1.0, 1.0);
    let t = 0.5 * (d / 3.0 + 0.5 * (2.0 - c));
    if t.is_finite() { t.max(0.0) } else { 0.0 }
}

/// The eased fraction of a timed move at progress `u`: ½ × sin((*u* − ½) × π) + ½.
#[must_use]
pub fn eased(u: f32) -> f32 {
    let u = if u.is_finite() {
        u.clamp(0.0, 1.0)
    } else {
        1.0
    };
    0.5 * ((u - 0.5) * std::f32::consts::PI).sin() + 0.5
}

/// The turntable state (module docs).
#[derive(Debug, Clone, PartialEq)]
pub struct Turntable {
    theta: Eased,
    zoom: Eased,
    moving: Option<ZoomMove>,
    drag: Option<(f32, f32)>,
}

impl Default for Turntable {
    fn default() -> Self {
        Self {
            theta: Eased::new(TURN_HALF_LIFE),
            zoom: Eased::new(ZOOM_HALF_LIFE),
            moving: None,
            drag: None,
        }
    }
}

impl Turntable {
    /// The turn angle *θ*, degrees.
    #[must_use]
    pub fn angle(&self) -> f32 {
        self.theta.value
    }

    /// The zoom fraction *t* in [0, 1].
    #[must_use]
    pub fn zoom(&self) -> f32 {
        self.zoom.value
    }

    /// The target of *t*.
    #[must_use]
    pub fn zoom_target(&self) -> f32 {
        self.zoom.target
    }

    /// Whether a zoom button's camera move runs.
    #[must_use]
    pub fn moving(&self) -> bool {
        self.moving.is_some()
    }

    /// Whether the turn buttons are ghosted (a zoom move runs).
    #[must_use]
    pub fn turn_ghosted(&self) -> bool {
        self.moving()
    }

    /// Whether the zoom-in button is ghosted.
    #[must_use]
    pub fn zoom_in_ghosted(&self) -> bool {
        self.zoom.target >= 1.0 - REACHED
    }

    /// Whether the zoom-out button is ghosted.
    #[must_use]
    pub fn zoom_out_ghosted(&self) -> bool {
        self.zoom.target <= REACHED
    }

    /// One hot click of a turn button (ignored while ghosted).
    pub fn turn_click(&mut self, turn: Turn, now: Instant) {
        if self.turn_ghosted() {
            return;
        }
        self.theta.update(now);
        let target = self.theta.value + turn.step();
        self.theta.set_target(target, now);
    }

    /// A drag across the viewport starts at pixel column `x`.
    pub fn drag_start(&mut self, x: f32) {
        if self.moving() || !x.is_finite() {
            return;
        }
        self.drag = Some((self.theta.value, x));
    }

    /// The drag is at pixel column `x`: *θ* follows at once.
    pub fn drag_to(&mut self, x: f32) {
        let Some((theta0, x0)) = self.drag else {
            return;
        };
        if self.moving() || !x.is_finite() {
            return;
        }
        self.theta
            .set_now(theta0 + (x - x0) * DRAG_DEGREES_PER_PIXEL);
    }

    /// The drag ends.
    pub fn drag_end(&mut self) {
        self.drag = None;
    }

    /// Whether a drag runs.
    #[must_use]
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// `notches` wheel notches (positive: away from the player, zooming in).
    pub fn wheel(&mut self, notches: f32, now: Instant) {
        if self.moving() || !notches.is_finite() || notches == 0.0 {
            return;
        }
        self.zoom.update(now);
        let target = (self.zoom.target + notches * WHEEL_STEP).clamp(0.0, 1.0);
        self.zoom.set_target(target, now);
    }

    /// A zoom button: *t* to `to` (1 in, 0 out) in a timed move from the camera as it is
    /// now; `rig` gives the frames (`None`: *t* is set at once, nothing to move).
    pub fn zoom_to(&mut self, to: f32, rig: Option<&CameraRig>, now: Instant) {
        let to = if to.is_finite() {
            to.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if self.zoom.target == to && self.moving.is_none_or(|m| m.end_t == to) {
            return;
        }
        let Some(rig) = rig else {
            self.zoom.set_now(to);
            self.moving = None;
            return;
        };
        let (start, start_fov) = self.camera(rig);
        self.drag = None;
        self.zoom.set_now(to);
        let (end, _) = rig.frame(to, self.theta.value);
        self.moving = Some(ZoomMove {
            start,
            start_fov,
            end_t: to,
            started: now,
            duration: move_duration(&start, &end),
            u: 0.0,
        });
    }

    /// Advances the easings and a zoom move to `now`.
    pub fn update(&mut self, now: Instant) {
        self.theta.update(now);
        self.zoom.update(now);
        if let Some(m) = &mut self.moving {
            let dt = now.saturating_duration_since(m.started).as_secs_f32();
            m.u = if m.duration > 0.0 {
                (dt / m.duration).min(1.0)
            } else {
                1.0
            };
            if m.u >= 1.0 {
                self.moving = None;
            }
        }
        // Keep the angle small once it rests (a full turn changes nothing).
        if self.theta.since.is_none() && self.drag.is_none() && self.theta.value.abs() >= 360.0 {
            self.theta.set_now(self.theta.value.rem_euclid(360.0));
        }
    }

    /// The camera and its field of view (degrees) for `rig` as the turntable stands
    /// (§2.1.3; during a zoom move between its start and end frames).
    #[must_use]
    pub fn camera(&self, rig: &CameraRig) -> (CameraFrame, f32) {
        let (frame, fov) = rig.frame(self.zoom.value, self.theta.value);
        match &self.moving {
            Some(m) => {
                let (end, end_fov) = rig.frame(m.end_t, self.theta.value);
                (
                    m.start.lerp(&end, eased(m.u)),
                    m.start_fov + (end_fov - m.start_fov) * m.u,
                )
            }
            None => (frame, fov),
        }
    }

    /// *θ* and *t* back to 0 at once (the stage camera was set); a move or drag ends.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tessera_render::glam::{Quat, Vec3};

    fn rig() -> CameraRig {
        let back = Quat::from_rotation_z(std::f32::consts::PI);
        CameraRig {
            base: CameraFrame {
                position: Vec3::new(0.0, 3.0, 1.5),
                rotation: back,
            },
            zoom: CameraFrame {
                position: Vec3::new(0.0, 1.0, 1.7),
                rotation: back,
            },
            pivot: Vec3::ZERO,
        }
    }

    #[test]
    fn hot_clicks_ease_twenty_degrees_with_a_half_life() {
        let t0 = Instant::now();
        let mut t = Turntable::default();
        t.turn_click(Turn::Right, t0);
        t.update(t0 + Duration::from_millis(150));
        assert!((t.angle() - 10.0).abs() < 1e-3, "{}", t.angle());
        // The next click starts from the current θ, not from the target.
        t.turn_click(Turn::Right, t0 + Duration::from_millis(150));
        t.update(t0 + Duration::from_millis(300));
        assert!((t.angle() - 20.0).abs() < 1e-3, "{}", t.angle());
        t.update(t0 + Duration::from_secs(10));
        assert_eq!(t.angle(), 30.0, "reached within 0.0002 snaps to the target");
        t.turn_click(Turn::Left, t0 + Duration::from_secs(10));
        t.update(t0 + Duration::from_secs(20));
        assert_eq!(t.angle(), 10.0);
    }

    #[test]
    fn drags_set_the_angle_at_once() {
        let mut t = Turntable::default();
        t.drag_start(100.0);
        t.drag_to(140.0);
        assert_eq!(t.angle(), -20.0);
        t.drag_to(60.0);
        assert_eq!(t.angle(), 20.0);
        t.drag_end();
        t.drag_to(0.0);
        assert_eq!(t.angle(), 20.0, "no drag: nothing");
        t.drag_start(f32::NAN);
        assert!(!t.dragging());
    }

    #[test]
    fn the_wheel_eases_the_zoom_and_clamps() {
        let t0 = Instant::now();
        let mut t = Turntable::default();
        assert!(t.zoom_out_ghosted() && !t.zoom_in_ghosted());
        t.wheel(2.0, t0);
        assert!((t.zoom_target() - 0.3).abs() < 1e-6);
        t.update(t0 + Duration::from_millis(100));
        assert!((t.zoom() - 0.15).abs() < 1e-4);
        t.wheel(100.0, t0 + Duration::from_millis(100));
        assert_eq!(t.zoom_target(), 1.0);
        assert!(t.zoom_in_ghosted());
        t.wheel(-100.0, t0 + Duration::from_millis(100));
        assert_eq!(t.zoom_target(), 0.0);
        t.wheel(f32::NAN, t0);
        assert_eq!(t.zoom_target(), 0.0);
    }

    #[test]
    fn zoom_buttons_move_the_camera_in_a_timed_eased_move() {
        let t0 = Instant::now();
        let r = rig();
        let mut t = Turntable::default();
        t.zoom_to(1.0, Some(&r), t0);
        assert!(t.moving() && t.turn_ghosted() && t.zoom_in_ghosted());
        // d = |(0,3,1.5) − (0,1,1.7)| ≈ 2.00998, c = 1: T = ½ (d/3 + ½) ≈ 0.585 s.
        let d = Vec3::new(0.0, 2.0, -0.2).length();
        let want = 0.5 * (d / 3.0 + 0.5);
        assert!((move_duration(&r.base, &r.zoom) - want).abs() < 1e-5);
        let (c0, f0) = t.camera(&r);
        assert!((c0.position - r.base.position).length() < 1e-5);
        assert!((f0 - 65.0).abs() < 1e-4);
        t.turn_click(Turn::Right, t0);
        t.update(t0 + Duration::from_secs_f32(want / 2.0));
        assert_eq!(t.angle(), 0.0, "turning is ghosted during the move");
        let (mid, fov) = t.camera(&r);
        assert!(
            (mid.position - Vec3::new(0.0, 2.0, 1.6)).length() < 1e-3,
            "eased ½"
        );
        assert!((fov - 52.5).abs() < 0.2, "the field of view at u");
        t.update(t0 + Duration::from_secs(5));
        assert!(!t.moving());
        let (end, fov) = t.camera(&r);
        assert!((end.position - r.zoom.position).length() < 1e-5);
        assert!((fov - 40.0).abs() < 1e-4);
        assert!(t.zoom_in_ghosted() && !t.zoom_out_ghosted());
        // Without frames the fraction is set at once.
        let mut u = Turntable::default();
        u.zoom_to(1.0, None, t0);
        assert!(!u.moving() && u.zoom() == 1.0);
        u.reset();
        assert_eq!((u.zoom(), u.angle()), (0.0, 0.0));
        assert_eq!(eased(0.0), 0.0);
        assert!((eased(1.0) - 1.0).abs() < 1e-6);
        assert!((eased(0.5) - 0.5).abs() < 1e-6);
    }
}
