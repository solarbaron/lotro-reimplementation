// SPDX-License-Identifier: GPL-3.0-or-later
//! Walk controller: a kinematic character that follows a height field
//! (`docs/specs/keymap.md` §9.3).
//!
//! The ground is a callback ([`HeightQuery`]) answering "ground height at
//! `(x, y)`", so the controller needs nothing from the terrain or world
//! crates. Collision with objects comes later (it will plug in as another
//! query); until then only the height field constrains movement.

use glam::{Vec2, Vec3};

use super::fly::clamp_pos;
use super::{MoveIntent, approach, clean_dt, heading, lerp_angle, right_of, wrap_angle};

/// Most horizontal sub-steps per tick.
pub const MAX_SUBSTEPS: usize = 64;
/// Height above the ground at which a falling character counts as landed.
pub const LAND_TOLERANCE: f32 = 1e-4;
/// Largest `dt` one tick accepts (seconds).
pub const MAX_TICK_DT: f32 = 0.25;

/// Ground height source.
pub trait HeightQuery {
    /// Ground height at `(x, y)`, or `None` where no data is loaded.
    /// Non-finite answers are treated as `None`.
    fn height(&self, x: f32, y: f32) -> Option<f32>;

    /// Exact unit surface normal at `(x, y)` when the source has one;
    /// the default `None` makes the controller estimate it from heights
    /// ([`WalkConfig::normal_probe`]).
    fn normal(&self, x: f32, y: f32) -> Option<Vec3> {
        let _ = (x, y);
        None
    }
}

impl<F: Fn(f32, f32) -> Option<f32>> HeightQuery for F {
    fn height(&self, x: f32, y: f32) -> Option<f32> {
        self(x, y)
    }
}

/// Tunables of [`WalkController`]. Tessera's own tuning in metres (the
/// terrain height scale is still an OPEN item of the spec); plain data for
/// a settings file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WalkConfig {
    /// Running speed, units per second.
    pub run_speed: f32,
    /// Walking speed, units per second.
    pub walk_speed: f32,
    /// Speed factor when moving backwards.
    pub backward_multiplier: f32,
    /// Horizontal acceleration on the ground, units per second².
    pub ground_acceleration: f32,
    /// Horizontal acceleration in the air (air control), units per second².
    pub air_acceleration: f32,
    /// Gravity, units per second² (positive, pulls towards `-Z`).
    pub gravity: f32,
    /// Initial upward speed of a jump, units per second.
    pub jump_speed: f32,
    /// Terminal falling speed, units per second.
    pub max_fall_speed: f32,
    /// Highest ledge climbed without jumping.
    pub step_height: f32,
    /// Steepest walkable slope, radians from horizontal.
    pub max_slope: f32,
    /// Keyboard turn rate, radians per second.
    pub turn_rate: f32,
    /// How long a jump press is remembered while not on the ground, seconds.
    pub jump_buffer: f32,
    /// Half-distance of the finite differences that estimate normals.
    pub normal_probe: f32,
    /// How far past a ledge the step-up check looks for walkable ground.
    pub step_probe: f32,
    /// Longest horizontal sub-step.
    pub max_substep: f32,
}

impl Default for WalkConfig {
    fn default() -> Self {
        Self {
            run_speed: 6.0,
            walk_speed: 2.0,
            backward_multiplier: 0.6,
            ground_acceleration: 60.0,
            air_acceleration: 6.0,
            gravity: 20.0,
            jump_speed: 7.0,
            max_fall_speed: 60.0,
            step_height: 0.5,
            max_slope: 50f32.to_radians(),
            turn_rate: 2.5,
            jump_buffer: 0.15,
            normal_probe: 0.25,
            step_probe: 0.3,
            max_substep: 0.25,
        }
    }
}

impl WalkConfig {
    /// A copy with every field finite and within a sane range.
    #[must_use]
    pub fn sanitized(&self) -> Self {
        let f = |v: f32, lo: f32, hi: f32, d: f32| {
            if v.is_finite() { v.clamp(lo, hi) } else { d }
        };
        let d = Self::default();
        Self {
            run_speed: f(self.run_speed, 0.0, 1000.0, d.run_speed),
            walk_speed: f(self.walk_speed, 0.0, 1000.0, d.walk_speed),
            backward_multiplier: f(self.backward_multiplier, 0.0, 1.0, d.backward_multiplier),
            ground_acceleration: f(self.ground_acceleration, 0.0, 1e5, d.ground_acceleration),
            air_acceleration: f(self.air_acceleration, 0.0, 1e5, d.air_acceleration),
            gravity: f(self.gravity, 0.0, 1000.0, d.gravity),
            jump_speed: f(self.jump_speed, 0.0, 1000.0, d.jump_speed),
            max_fall_speed: f(self.max_fall_speed, 0.1, 1e4, d.max_fall_speed),
            step_height: f(self.step_height, 0.0, 100.0, d.step_height),
            max_slope: f(self.max_slope, 0.0, 89f32.to_radians(), d.max_slope),
            turn_rate: f(self.turn_rate, 0.0, 100.0, d.turn_rate),
            jump_buffer: f(self.jump_buffer, 0.0, 5.0, d.jump_buffer),
            normal_probe: f(self.normal_probe, 0.01, 100.0, d.normal_probe),
            step_probe: f(self.step_probe, 0.0, 100.0, d.step_probe),
            max_substep: f(self.max_substep, 0.01, 100.0, d.max_substep),
        }
    }
}

/// Contact state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroundState {
    /// Standing or moving on walkable ground.
    #[default]
    Grounded,
    /// On ground steeper than the slope limit, sliding down.
    Sliding,
    /// Jumping or falling.
    Airborne,
    /// No height data under the character; it is held in place until data
    /// arrives.
    Unsupported,
}

/// What happened during one tick.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WalkEvent {
    /// A jump started.
    pub jumped: bool,
    /// Touched down, with the downward speed at impact.
    pub landed: Option<f32>,
    /// Horizontal movement was stopped (slope, ledge or missing data).
    pub blocked: bool,
}

/// Walk controller state. `position` is the point between the feet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WalkController {
    /// Tunables (sanitised at every tick).
    pub config: WalkConfig,
    position: Vec3,
    previous: Vec3,
    velocity: Vec3,
    yaw: f32,
    previous_yaw: f32,
    state: GroundState,
    jump_timer: f32,
}

impl WalkController {
    /// A controller at `position`, airborne until the first tick finds the
    /// ground (it then snaps down if within reach, or falls).
    #[must_use]
    pub fn new(position: Vec3, yaw: f32, config: WalkConfig) -> Self {
        let mut c = Self {
            config,
            position: Vec3::ZERO,
            previous: Vec3::ZERO,
            velocity: Vec3::ZERO,
            yaw: wrap_angle(yaw),
            previous_yaw: wrap_angle(yaw),
            state: GroundState::Airborne,
            jump_timer: 0.0,
        };
        c.teleport(position);
        c
    }

    /// Feet position after the last tick.
    #[must_use]
    pub fn position(&self) -> Vec3 {
        self.position
    }

    /// Velocity after the last tick.
    #[must_use]
    pub fn velocity(&self) -> Vec3 {
        self.velocity
    }

    /// Heading in radians.
    #[must_use]
    pub fn yaw(&self) -> f32 {
        self.yaw
    }

    /// Contact state.
    #[must_use]
    pub fn state(&self) -> GroundState {
        self.state
    }

    /// Whether standing on walkable ground.
    #[must_use]
    pub fn on_ground(&self) -> bool {
        self.state == GroundState::Grounded
    }

    /// Feet position interpolated between the previous and the last tick.
    #[must_use]
    pub fn interpolated(&self, alpha: f32) -> Vec3 {
        let a = if alpha.is_finite() {
            alpha.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.previous.lerp(self.position, a)
    }

    /// Heading interpolated between the previous and the last tick along
    /// the shorter arc, so keyboard turning (applied per tick) renders
    /// smoothly. Per-frame turns ([`turn`](Self::turn)) shift both ends and
    /// show at once.
    #[must_use]
    pub fn interpolated_yaw(&self, alpha: f32) -> f32 {
        lerp_angle(self.previous_yaw, self.yaw, alpha)
    }

    /// Set the heading (no interpolation).
    pub fn set_yaw(&mut self, yaw: f32) {
        self.yaw = wrap_angle(yaw);
        self.previous_yaw = self.yaw;
    }

    /// Turn by `dyaw` radians (positive = left); for mouse turning every
    /// frame outside the fixed step. Both interpolation ends move, so the
    /// turn is visible immediately.
    pub fn turn(&mut self, dyaw: f32) {
        if dyaw.is_finite() {
            self.yaw = wrap_angle(self.yaw + dyaw);
            self.previous_yaw = wrap_angle(self.previous_yaw + dyaw);
        }
    }

    /// Jump to `position` (no interpolation, motion stopped, airborne until
    /// the next tick finds ground).
    pub fn teleport(&mut self, position: Vec3) {
        let p = clamp_pos(position);
        self.position = p;
        self.previous = p;
        self.previous_yaw = self.yaw;
        self.velocity = Vec3::ZERO;
        self.state = GroundState::Airborne;
        self.jump_timer = 0.0;
    }

    /// Advance one fixed step of `dt` seconds (capped at [`MAX_TICK_DT`]).
    pub fn tick(&mut self, intent: &MoveIntent, ground: &dyn HeightQuery, dt: f32) -> WalkEvent {
        let cfg = self.config.sanitized();
        let dt = clean_dt(dt, MAX_TICK_DT);
        let i = intent.sanitized();
        let g = Ground { q: ground, cfg };
        let mut ev = WalkEvent::default();
        self.previous = self.position;
        self.previous_yaw = self.yaw;
        self.yaw = wrap_angle(self.yaw + i.turn * cfg.turn_rate * dt);
        if i.jump {
            self.jump_timer = cfg.jump_buffer.max(dt);
        }

        let Some(h_here) = g.height(self.position.truncate()) else {
            self.state = GroundState::Unsupported;
            self.velocity = Vec3::ZERO;
            self.jump_timer = (self.jump_timer - dt).max(0.0);
            return ev;
        };

        // Coming back from missing data, or found below the surface (after a
        // teleport or a terrain change): stand on the surface. Standing on
        // ground that has dropped away (refined heights, a terrain swap):
        // follow it down within the step height, else fall.
        if self.state == GroundState::Unsupported || self.position.z < h_here {
            if self.position.z <= h_here + cfg.step_height {
                self.position.z = h_here;
                self.velocity.z = 0.0;
                self.state = g.contact_state(self.position.truncate());
            } else {
                self.state = GroundState::Airborne;
            }
        } else if self.state == GroundState::Airborne
            && self.velocity.z <= 0.0
            && self.position.z - h_here <= LAND_TOLERANCE
        {
            ev.landed = Some((-self.velocity.z).max(0.0));
            self.position.z = h_here;
            self.velocity.z = 0.0;
            self.state = g.contact_state(self.position.truncate());
        } else if matches!(self.state, GroundState::Grounded | GroundState::Sliding)
            && self.position.z - h_here > LAND_TOLERANCE
        {
            if self.position.z - h_here <= cfg.step_height {
                self.position.z = h_here;
                self.state = g.contact_state(self.position.truncate());
            } else {
                self.state = GroundState::Airborne;
            }
            self.velocity.z = 0.0;
        }

        // Desired horizontal velocity.
        let mut wish = heading(self.yaw) * i.forward + right_of(self.yaw) * i.strafe;
        if wish.length_squared() > 1.0 {
            wish = wish.normalize_or_zero();
        }
        let mut speed = if i.walk {
            cfg.walk_speed
        } else {
            cfg.run_speed
        };
        if i.forward < 0.0 {
            speed *= cfg.backward_multiplier;
        }
        let target = wish * speed;
        let horiz = Vec3::new(self.velocity.x, self.velocity.y, 0.0);

        match self.state {
            GroundState::Grounded => {
                let h = approach(horiz, target, cfg.ground_acceleration * dt);
                self.velocity = Vec3::new(h.x, h.y, 0.0);
                if self.jump_timer > 0.0 {
                    self.velocity.z = cfg.jump_speed;
                    self.state = GroundState::Airborne;
                    self.jump_timer = 0.0;
                    ev.jumped = true;
                }
            }
            GroundState::Sliding => {
                let n = g.normal(self.position.truncate());
                let downhill = Vec3::new(n.x, n.y, 0.0).normalize_or_zero();
                let sin = (1.0 - n.z * n.z).max(0.0).sqrt();
                let mut h = approach(horiz, target, cfg.air_acceleration * dt);
                h += downhill * cfg.gravity * sin * dt;
                let h = h.clamp_length_max(cfg.max_fall_speed);
                self.velocity = Vec3::new(h.x, h.y, 0.0);
            }
            GroundState::Airborne | GroundState::Unsupported => {
                let h = approach(horiz, target, cfg.air_acceleration * dt);
                self.velocity = Vec3::new(h.x, h.y, self.velocity.z);
                self.state = GroundState::Airborne;
            }
        }
        self.jump_timer = (self.jump_timer - dt).max(0.0);

        // Horizontal movement in sub-steps.
        let mut disp = Vec2::new(self.velocity.x, self.velocity.y) * dt;
        let max_len = cfg.max_substep * MAX_SUBSTEPS as f32;
        if disp.length() > max_len {
            disp = disp.clamp_length_max(max_len);
        }
        let n = ((disp.length() / cfg.max_substep).ceil() as usize).clamp(1, MAX_SUBSTEPS);
        let step = disp / n as f32;
        if step != Vec2::ZERO {
            for _ in 0..n {
                if self.try_move(step, &g, &mut ev) {
                    continue;
                }
                // Blocked: slide along whichever axis still works.
                let (a, b) = if step.x.abs() >= step.y.abs() {
                    (Vec2::new(step.x, 0.0), Vec2::new(0.0, step.y))
                } else {
                    (Vec2::new(0.0, step.y), Vec2::new(step.x, 0.0))
                };
                ev.blocked = true;
                if a != Vec2::ZERO && self.try_move(a, &g, &mut ev) {
                    self.zero_velocity_along(b);
                } else if b != Vec2::ZERO && self.try_move(b, &g, &mut ev) {
                    self.zero_velocity_along(a);
                } else {
                    self.velocity.x = 0.0;
                    self.velocity.y = 0.0;
                    break;
                }
            }
        }

        // Vertical movement: constant acceleration over the tick (exact for
        // a jump arc, so the apex is v²/2g whatever the tick rate).
        if self.state == GroundState::Airborne {
            let v0 = self.velocity.z;
            let v1 = (v0 - cfg.gravity * dt).max(-cfg.max_fall_speed);
            self.position.z += 0.5 * (v0 + v1) * dt;
            self.velocity.z = v1;
            if let Some(h) = g.height(self.position.truncate())
                && self.position.z <= h + LAND_TOLERANCE
            {
                ev.landed = Some((-self.velocity.z).max(0.0));
                self.position.z = h;
                self.velocity.z = 0.0;
                self.state = g.contact_state(self.position.truncate());
            }
        }
        self.position = clamp_pos(self.position);
        if !self.velocity.is_finite() {
            self.velocity = Vec3::ZERO;
        }
        ev
    }

    fn zero_velocity_along(&mut self, axis: Vec2) {
        if axis.x != 0.0 {
            self.velocity.x = 0.0;
        }
        if axis.y != 0.0 {
            self.velocity.y = 0.0;
        }
    }

    /// Try one horizontal sub-step; on success the position (and maybe the
    /// state) is updated.
    fn try_move(&mut self, d: Vec2, g: &Ground<'_>, ev: &mut WalkEvent) -> bool {
        let here = self.position.truncate();
        let cand = here + d;
        let Some(hc) = g.height(cand) else {
            return false;
        };
        let z = self.position.z;
        match self.state {
            GroundState::Grounded | GroundState::Sliding => {
                let rise = hc - z;
                if rise > 0.0 && !g.can_climb(cand, d, z, rise) {
                    return false;
                }
                self.position.x = cand.x;
                self.position.y = cand.y;
                let drop = z - hc;
                let snap = g.cfg.step_height + d.length() * g.cfg.max_slope.tan();
                if drop <= snap {
                    self.position.z = hc;
                    self.state = g.contact_state(cand);
                } else {
                    // Walked off a ledge.
                    self.state = GroundState::Airborne;
                    self.velocity.z = 0.0;
                }
                true
            }
            GroundState::Airborne | GroundState::Unsupported => {
                if hc > z + g.cfg.step_height {
                    return false;
                }
                self.position.x = cand.x;
                self.position.y = cand.y;
                if hc >= z && self.velocity.z <= 0.0 {
                    ev.landed = Some((-self.velocity.z).max(0.0));
                    self.position.z = hc;
                    self.velocity.z = 0.0;
                    self.state = g.contact_state(cand);
                } else if hc > z {
                    self.position.z = hc;
                }
                true
            }
        }
    }
}

/// Height queries with sanitising and normal estimation.
struct Ground<'a> {
    q: &'a dyn HeightQuery,
    cfg: WalkConfig,
}

impl Ground<'_> {
    fn height(&self, p: Vec2) -> Option<f32> {
        if !p.is_finite() {
            return None;
        }
        self.q
            .height(p.x, p.y)
            .filter(|h| h.is_finite())
            .map(|h| h.clamp(-super::fly::MAX_COORD, super::fly::MAX_COORD))
    }

    /// Unit normal; `+Z` when nothing better is known.
    fn normal(&self, p: Vec2) -> Vec3 {
        if let Some(n) = self.q.normal(p.x, p.y)
            && n.is_finite()
            && let Some(n) = n.try_normalize()
        {
            return if n.z < 0.0 { -n } else { n };
        }
        let e = self.cfg.normal_probe;
        let center = self.height(p);
        let slope = |a: Vec2, b: Vec2| -> f32 {
            match (self.height(p + a), self.height(p + b), center) {
                (Some(h1), Some(h0), _) => (h1 - h0) / (2.0 * e),
                (Some(h1), None, Some(c)) => (h1 - c) / e,
                (None, Some(h0), Some(c)) => (c - h0) / e,
                _ => 0.0,
            }
        };
        let dx = slope(Vec2::new(e, 0.0), Vec2::new(-e, 0.0));
        let dy = slope(Vec2::new(0.0, e), Vec2::new(0.0, -e));
        Vec3::new(-dx, -dy, 1.0).try_normalize().unwrap_or(Vec3::Z)
    }

    fn walkable(&self, n: Vec3) -> bool {
        n.z >= self.cfg.max_slope.cos() - 1e-6
    }

    fn contact_state(&self, p: Vec2) -> GroundState {
        if self.walkable(self.normal(p)) {
            GroundState::Grounded
        } else {
            GroundState::Sliding
        }
    }

    /// May a grounded character at height `z` move to `cand` (`rise > 0`
    /// above it) along `d`?
    fn can_climb(&self, cand: Vec2, d: Vec2, z: f32, rise: f32) -> bool {
        if rise > self.cfg.step_height {
            return false;
        }
        if self.walkable(self.normal(cand)) {
            return true;
        }
        // Step-up: a short steep face is fine if walkable ground within the
        // step height lies just beyond it.
        let dir = d.normalize_or_zero();
        let ahead = cand + dir * self.cfg.step_probe;
        match self.height(ahead) {
            Some(ha) => ha - z <= self.cfg.step_height && self.walkable(self.normal(ahead)),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FixedTimestep;

    const DT: f32 = 1.0 / 30.0;

    fn run(forward: f32) -> MoveIntent {
        MoveIntent {
            forward,
            ..MoveIntent::default()
        }
    }

    fn flat(_: f32, _: f32) -> Option<f32> {
        Some(0.0)
    }

    /// Settle on the ground, then run `ticks` ticks.
    fn simulate(
        c: &mut WalkController,
        ground: &dyn HeightQuery,
        intent: MoveIntent,
        ticks: usize,
    ) -> Vec<(Vec3, GroundState, WalkEvent)> {
        (0..ticks)
            .map(|_| {
                let e = c.tick(&intent, ground, DT);
                (c.position(), c.state(), e)
            })
            .collect()
    }

    #[test]
    fn runs_on_flat_ground() {
        let mut c = WalkController::new(Vec3::ZERO, 0.0, WalkConfig::default());
        simulate(&mut c, &flat, run(0.0), 1);
        assert!(c.on_ground());
        simulate(&mut c, &flat, run(1.0), 60);
        // 6 u/s with a 0.1 s ramp-up at 60 u/s².
        let x = c.position().x;
        assert!((x - (12.0 - 0.3)).abs() < 0.1, "{x}");
        assert_eq!(c.position().z, 0.0);
        assert!((c.velocity().x - 6.0).abs() < 1e-4);
        // Walking and backwards.
        let mut c = WalkController::new(Vec3::ZERO, 0.0, WalkConfig::default());
        let i = MoveIntent {
            forward: -1.0,
            walk: true,
            ..MoveIntent::default()
        };
        simulate(&mut c, &flat, i, 30);
        assert!((c.velocity().x + 1.2).abs() < 1e-4);
    }

    #[test]
    fn turning() {
        let mut c = WalkController::new(Vec3::ZERO, 0.0, WalkConfig::default());
        let i = MoveIntent {
            turn: 1.0,
            ..MoveIntent::default()
        };
        simulate(&mut c, &flat, i, 30);
        assert!((c.yaw() - 2.5).abs() < 1e-4);
    }

    #[test]
    fn follows_walkable_slope_both_ways() {
        let tan30 = 30f32.to_radians().tan();
        let ramp = move |x: f32, _: f32| Some(x.max(0.0) * tan30);
        let mut c = WalkController::new(Vec3::new(-1.0, 0.0, 0.0), 0.0, WalkConfig::default());
        for (p, s, _) in simulate(&mut c, &ramp, run(1.0), 90) {
            assert_eq!(s, GroundState::Grounded, "{p}");
            assert!((p.z - p.x.max(0.0) * tan30).abs() < 1e-4);
        }
        assert!(c.position().x > 15.0);
        // And back down without leaving the ground.
        c.set_yaw(std::f32::consts::PI);
        for (p, s, _) in simulate(&mut c, &ramp, run(1.0), 60) {
            assert_eq!(s, GroundState::Grounded, "{p}");
        }
    }

    #[test]
    fn steep_slope_blocks() {
        let tan60 = 60f32.to_radians().tan();
        let wall = move |x: f32, _: f32| Some((x - 2.0).max(0.0) * tan60);
        let mut c = WalkController::new(Vec3::ZERO, 0.0, WalkConfig::default());
        let out = simulate(&mut c, &wall, run(1.0), 120);
        assert!(out.iter().any(|(_, _, e)| e.blocked));
        let p = c.position();
        assert!(p.x < 2.6 && p.z < 0.6, "{p}");
        // Diagonal approach slides along the wall (y keeps growing).
        let mut c = WalkController::new(Vec3::ZERO, 45f32.to_radians(), WalkConfig::default());
        simulate(&mut c, &wall, run(1.0), 120);
        assert!(
            c.position().x < 2.6 && c.position().y > 6.0,
            "{}",
            c.position()
        );
    }

    #[test]
    fn step_up_small_ledges_only() {
        let ledge = |h: f32| move |x: f32, _: f32| Some(if x >= 3.0 { h } else { 0.0 });
        for (h, climbs) in [(0.3, true), (0.5, true), (0.8, false), (2.0, false)] {
            let mut c = WalkController::new(Vec3::ZERO, 0.0, WalkConfig::default());
            simulate(&mut c, &ledge(h), run(1.0), 60);
            let p = c.position();
            if climbs {
                assert!(p.x > 5.0 && (p.z - h).abs() < 1e-5, "h={h}: {p}");
            } else {
                assert!(p.x < 3.0 && p.z == 0.0, "h={h}: {p}");
            }
        }
    }

    #[test]
    fn jump_kinematics() {
        let cfg = WalkConfig::default();
        let mut c = WalkController::new(Vec3::ZERO, 0.0, cfg);
        simulate(&mut c, &flat, run(0.0), 2);
        let jump = MoveIntent {
            jump: true,
            ..MoveIntent::default()
        };
        let first = c.tick(&jump, &flat, DT);
        assert!(first.jumped);
        let mut apex = c.position().z;
        let mut ticks = 1;
        let mut landed = None;
        while landed.is_none() && ticks < 200 {
            let e = c.tick(&MoveIntent::default(), &flat, DT);
            apex = apex.max(c.position().z);
            ticks += 1;
            landed = e.landed;
        }
        let expect_apex = cfg.jump_speed * cfg.jump_speed / (2.0 * cfg.gravity);
        assert!(
            (apex - expect_apex).abs() < 0.15,
            "apex {apex} vs {expect_apex}"
        );
        let air_time = ticks as f32 * DT;
        let expect_time = 2.0 * cfg.jump_speed / cfg.gravity;
        assert!(
            (air_time - expect_time).abs() <= 2.0 * DT,
            "{air_time} vs {expect_time}"
        );
        assert!(landed.unwrap() > 5.0);
        assert!(c.on_ground() && c.position().z == 0.0);
    }

    #[test]
    fn jump_is_buffered_until_landing() {
        let mut c = WalkController::new(Vec3::new(0.0, 0.0, 0.1), 0.0, WalkConfig::default());
        // Still falling the 0.1 units (0.1 s) when jump is pressed.
        let jump = MoveIntent {
            jump: true,
            ..MoveIntent::default()
        };
        let e0 = c.tick(&jump, &flat, DT);
        assert!(!e0.jumped);
        let mut jumped = false;
        for _ in 0..4 {
            jumped |= c.tick(&MoveIntent::default(), &flat, DT).jumped;
        }
        assert!(jumped, "the buffered press fires on touchdown");
        // An old press expires.
        let mut c = WalkController::new(Vec3::new(0.0, 0.0, 20.0), 0.0, WalkConfig::default());
        c.tick(&jump, &flat, DT);
        let jumped = (0..100).any(|_| c.tick(&MoveIntent::default(), &flat, DT).jumped);
        assert!(!jumped);
    }

    #[test]
    fn walks_off_cliff_and_falls() {
        let cliff = |x: f32, _: f32| Some(if x < 2.0 { 10.0 } else { 0.0 });
        let mut c = WalkController::new(Vec3::new(0.0, 0.0, 10.0), 0.0, WalkConfig::default());
        let out = simulate(&mut c, &cliff, run(1.0), 90);
        assert!(out.iter().any(|(_, s, _)| *s == GroundState::Airborne));
        // The spawn touchdown reports 0; the fall reports the impact speed.
        let land = out
            .iter()
            .filter_map(|(_, _, e)| e.landed)
            .fold(0.0f32, f32::max);
        // v = sqrt(2 g h) = 20 for a 10 unit drop at g = 20.
        assert!((land - 20.0).abs() < 1.5, "{land}");
        assert!(c.on_ground() && c.position().z == 0.0);
    }

    #[test]
    fn slides_off_steep_ground() {
        let tan70 = 70f32.to_radians().tan();
        let steep = move |x: f32, _: f32| Some((10.0 - x).max(0.0) * tan70);
        let mut c =
            WalkController::new(Vec3::new(5.0, 0.0, 5.0 * tan70), 0.0, WalkConfig::default());
        let out = simulate(&mut c, &steep, run(0.0), 120);
        assert!(out.iter().any(|(_, s, _)| *s == GroundState::Sliding));
        assert!(
            c.position().x > 9.9,
            "slid down to the foot: {}",
            c.position()
        );
        assert!(c.on_ground());
    }

    #[test]
    fn missing_data_holds_position() {
        let island = |x: f32, _: f32| (x < 3.0).then_some(0.0);
        let mut c = WalkController::new(Vec3::ZERO, 0.0, WalkConfig::default());
        let out = simulate(&mut c, &island, run(1.0), 60);
        assert!(out.iter().any(|(_, _, e)| e.blocked));
        assert!(c.position().x < 3.0);
        let none = |_: f32, _: f32| None;
        let mut c = WalkController::new(Vec3::new(1.0, 2.0, 50.0), 0.0, WalkConfig::default());
        simulate(&mut c, &none, run(1.0), 30);
        assert_eq!(c.state(), GroundState::Unsupported);
        assert_eq!(c.position(), Vec3::new(1.0, 2.0, 50.0));
        // Data arrives: it falls and lands.
        simulate(&mut c, &flat, run(0.0), 120);
        assert!(c.on_ground() && c.position().z == 0.0);
    }

    #[test]
    fn idle_character_follows_dropping_ground() {
        use std::cell::Cell;
        let level = Cell::new(0.0f32);
        let ground = |_: f32, _: f32| Some(level.get());
        let mut c = WalkController::new(Vec3::ZERO, 0.0, WalkConfig::default());
        simulate(&mut c, &ground, run(0.0), 2);
        assert!(c.on_ground());
        // A small drop: snapped down at once.
        level.set(-0.3);
        simulate(&mut c, &ground, run(0.0), 1);
        assert!(c.on_ground() && c.position().z == -0.3, "{}", c.position());
        // A big drop: falls and lands.
        level.set(-5.0);
        let out = simulate(&mut c, &ground, run(0.0), 60);
        assert_eq!(out[0].1, GroundState::Airborne);
        assert!(c.on_ground() && c.position().z == -5.0, "{}", c.position());
    }

    #[test]
    fn yaw_interpolates_between_ticks() {
        use std::f32::consts::PI;
        let mut c = WalkController::new(Vec3::ZERO, PI - 0.02, WalkConfig::default());
        let left = MoveIntent {
            turn: 1.0,
            ..MoveIntent::default()
        };
        c.tick(&left, &flat, DT);
        let (a, b) = (c.interpolated_yaw(0.0), c.interpolated_yaw(1.0));
        assert!(
            (a - (PI - 0.02)).abs() < 1e-5 && b < 0.0,
            "wrapped: {a} {b}"
        );
        let mid = c.interpolated_yaw(0.5);
        let step = 2.5 * DT;
        assert!(
            (super::super::wrap_angle(mid - a) - step / 2.0).abs() < 1e-4,
            "{mid}"
        );
        // A per-frame turn shifts both ends: shows at once, no lag.
        c.turn(0.5);
        assert!((super::super::wrap_angle(c.interpolated_yaw(0.0) - a) - 0.5).abs() < 1e-4);
        c.set_yaw(1.0);
        assert_eq!(c.interpolated_yaw(0.0), 1.0);
    }

    #[test]
    fn below_surface_is_lifted() {
        let mut c = WalkController::new(Vec3::new(0.0, 0.0, -0.3), 0.0, WalkConfig::default());
        simulate(&mut c, &flat, run(0.0), 1);
        assert!(c.on_ground() && c.position().z == 0.0);
    }

    #[test]
    fn hostile_ground_and_config() {
        let nan = |x: f32, _: f32| Some(if x > 1.0 { f32::NAN } else { f32::INFINITY });
        let mut c = WalkController::new(Vec3::ZERO, 0.0, WalkConfig::default());
        simulate(&mut c, &nan, run(1.0), 10);
        assert!(c.position().is_finite());
        let noise =
            |x: f32, y: f32| Some(((x * 12.9898 + y * 78.233).sin() * 43758.547).fract() * 1e6);
        c.config = WalkConfig {
            run_speed: f32::INFINITY,
            gravity: f32::NAN,
            max_substep: 0.0,
            ..WalkConfig::default()
        };
        for _ in 0..50 {
            c.tick(
                &MoveIntent {
                    forward: 1.0,
                    strafe: f32::NAN,
                    jump: true,
                    ..MoveIntent::default()
                },
                &noise,
                f32::INFINITY,
            );
            assert!(c.position().is_finite() && c.velocity().is_finite());
        }
    }

    #[test]
    fn exact_normal_from_source() {
        struct Tilted;
        impl HeightQuery for Tilted {
            fn height(&self, _: f32, _: f32) -> Option<f32> {
                Some(0.0)
            }
            fn normal(&self, _: f32, _: f32) -> Option<Vec3> {
                // Claims a 80° slope although heights are flat.
                Some(Vec3::new(
                    80f32.to_radians().sin(),
                    0.0,
                    80f32.to_radians().cos(),
                ))
            }
        }
        let mut c = WalkController::new(Vec3::ZERO, 0.0, WalkConfig::default());
        c.tick(&MoveIntent::default(), &Tilted, DT);
        assert_eq!(c.state(), GroundState::Sliding);
    }

    #[test]
    fn frame_rate_independent_with_fixed_step() {
        let tan20 = 20f32.to_radians().tan();
        let hills = move |x: f32, y: f32| Some((x * 0.3).sin() * 2.0 + y * tan20 * 0.1);
        let mut results = Vec::new();
        for fps in [24.0f64, 60.0, 144.0, 33.3] {
            let mut c = WalkController::new(Vec3::new(0.0, 0.0, 5.0), 0.3, WalkConfig::default());
            let mut step = FixedTimestep::new(30.0);
            let mut ticks = 0;
            let frame = 1.0 / fps;
            let mut t = 0.0;
            while ticks < 150 {
                t += frame;
                for _ in 0..step.advance(frame) {
                    if ticks < 150 {
                        c.tick(&run(1.0), &hills, step.dt() as f32);
                        ticks += 1;
                    }
                }
            }
            let _ = t;
            results.push(c.position());
        }
        for r in &results[1..] {
            assert_eq!(*r, results[0], "identical after the same number of ticks");
        }
    }

    #[test]
    fn interpolation_between_ticks() {
        let mut c = WalkController::new(Vec3::ZERO, 0.0, WalkConfig::default());
        simulate(&mut c, &flat, run(1.0), 30);
        let a = c.interpolated(0.0);
        let b = c.interpolated(1.0);
        let mid = c.interpolated(0.5);
        assert!((mid - (a + b) * 0.5).length() < 1e-5);
        c.teleport(Vec3::new(100.0, 0.0, 0.0));
        assert_eq!(c.interpolated(0.0), c.interpolated(1.0));
    }
}
