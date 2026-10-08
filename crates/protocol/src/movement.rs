// SPDX-License-Identifier: GPL-3.0-or-later
//! Movement and interest helpers (`tessera-protocol.md` §8, §9): serial-number comparison,
//! the client's send schedule, the server's movement validator, and interest blocks.

use std::time::Duration;

use crate::limits::{
    ALLOWANCE_WINDOW, DEFAULT_TICK_RATE, EXTENT_MAX, EXTENT_MIN, GRID_EXTENT, LANDBLOCK_SIZE,
    MAX_CORRECTIONS_PER_SECOND, MAX_DOWN_SPEED, MAX_HORIZONTAL_SPEED, MAX_LANDBLOCK, MAX_UP_SPEED,
    REST_REDUNDANCY,
};
use crate::message::{LocationCause, LocationReport, MoveInput};
use crate::rate::SlidingWindow;
use crate::types::{Anchor, Location, Motion, Pose, Vec3};

/// RFC 1982 serial comparison for u32: whether `a` is newer than `b`.
#[must_use]
pub fn serial_newer(a: u32, b: u32) -> bool {
    a != b && (a.wrapping_sub(b) as i32) > 0
}

/// The client's `MoveInput` schedule (§8.2), driven once per 30 Hz simulation tick.
///
/// While moving or turning it sends every second tick (15 Hz); it sends at once when the gait
/// changes, on starting or stopping, and when a teleport or correction is adopted; after
/// stopping it repeats the rest state in the next 3 ticks; at rest it sends a 1 Hz heartbeat.
/// It never sends more than once per tick.
#[derive(Debug, Clone, Default)]
pub struct MoveScheduler {
    last_send: Option<u64>,
    was_moving: bool,
    last_motion: Option<Motion>,
    redundancy: u32,
}

impl MoveScheduler {
    /// A scheduler that sends on its first tick.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether to send a `MoveInput` on simulation tick `tick`. `moving` is true while the
    /// avatar moves or turns; `forced` is true on the tick a teleport or correction was
    /// adopted.
    pub fn on_tick(&mut self, tick: u64, moving: bool, motion: Motion, forced: bool) -> bool {
        let gait_changed = self.last_motion.is_some_and(|m| m != motion);
        let started = moving && !self.was_moving;
        let stopped = !moving && self.was_moving;
        self.was_moving = moving;
        self.last_motion = Some(motion);
        if stopped {
            self.redundancy = REST_REDUNDANCY;
        }
        let since = self.last_send.map(|t| tick.saturating_sub(t));
        if since == Some(0) {
            return false;
        }
        let send = if forced || gait_changed || started || stopped || since.is_none() {
            true
        } else if moving {
            since.is_some_and(|s| s >= 2)
        } else if self.redundancy > 0 {
            self.redundancy -= 1;
            true
        } else {
            since.is_some_and(|s| s >= u64::from(DEFAULT_TICK_RATE))
        };
        if send {
            self.last_send = Some(tick);
        }
        send
    }
}

/// Tolerances of the server's movement check (§8.3 defaults; `test-server.md` may change
/// them).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovementLimits {
    /// *V*ₕ, m/s. Independent of `WALK_SPEED` and `RUN_SPEED` (§7.3).
    pub horizontal_speed: f64,
    /// *V*ᵤ, m/s.
    pub up_speed: f64,
    /// *V*_d, m/s.
    pub down_speed: f64,
    /// *W*, the burst window.
    pub window: Duration,
    /// Lowest x or y accepted, metres.
    pub extent_min: f64,
    /// Highest x or y accepted, metres.
    pub extent_max: f64,
    /// The descent credit of `test-server.md` §8.3 with its reserve *S* in metres, or
    /// `None` for the protocol's §8.3 procedure exactly (the default).
    ///
    /// With a reserve *S* > 0 the validator keeps a credit pool *C* (0 ≤ *C* ≤ *S*, 0 at
    /// every new epoch, never growing with time). A move's counted descent is
    /// *d*_c = min(*d*_d, *A*_d); step 5 rejects when *d*ₕ > *A*ₕ + *d*_c + *C*, and on
    /// acceptance *A*ₕ pays first, then *d*_c, then *C*, while descent the move does not
    /// need is kept in *C* up to *S*. A reserve of 0, a negative one or NaN turns the
    /// credit off (the protocol's horizontal test).
    pub descent_credit: Option<f64>,
}

impl Default for MovementLimits {
    fn default() -> Self {
        Self {
            horizontal_speed: MAX_HORIZONTAL_SPEED,
            up_speed: MAX_UP_SPEED,
            down_speed: MAX_DOWN_SPEED,
            window: ALLOWANCE_WINDOW,
            extent_min: EXTENT_MIN,
            extent_max: EXTENT_MAX,
            descent_credit: None,
        }
    }
}

impl MovementLimits {
    /// The descent-credit reserve *S* in metres (0 without a credit or for a negative or
    /// NaN value). The credit is on only while this is above 0.
    fn reserve(&self) -> f64 {
        self.descent_credit.map_or(0.0, |s| s.max(0.0))
    }
}

/// What the server does with one `MoveInput` (§8.3).
#[derive(Debug, Clone, PartialEq)]
pub enum MoveVerdict {
    /// Step 2: the epoch is not current; dropped silently.
    StaleEpoch,
    /// Step 3: `seq` is not newer than the last accepted one; dropped.
    OldSeq,
    /// Step 6: accepted. The state now holds the new position; relay it.
    Accepted,
    /// Step 7: rejected; send this `LocationReport(CORRECTED)`. The new epoch has started.
    Corrected(LocationReport),
    /// Step 7 beyond 4 corrections per second: dropped without a report.
    Rejected,
}

/// The server's per-player movement state and check (§8.3).
#[derive(Debug, Clone)]
pub struct MovementValidator {
    limits: MovementLimits,
    epoch: u32,
    last_seq: Option<u32>,
    pose: Pose,
    velocity: Vec3,
    motion: Motion,
    allow_h: f64,
    allow_up: f64,
    allow_down: f64,
    /// *C*, the descent credit of `test-server.md` §8.3, metres.
    credit: f64,
    /// Whether the up and down tests of step 5 are skipped (`test-server.md` §8.3 settling
    /// grace).
    settling: bool,
    last_time: u64,
    corrections: SlidingWindow,
}

impl MovementValidator {
    /// State for a player who entered the world with `epoch` at `pose`, server time `now`
    /// (µs).
    #[must_use]
    pub fn new(limits: MovementLimits, epoch: u32, pose: Pose, now: u64) -> Self {
        let mut v = Self {
            limits,
            epoch,
            last_seq: None,
            pose,
            velocity: Vec3::ZERO,
            motion: Motion::STANDING,
            allow_h: 0.0,
            allow_up: 0.0,
            allow_down: 0.0,
            credit: 0.0,
            settling: false,
            last_time: now,
            corrections: SlidingWindow::new(MAX_CORRECTIONS_PER_SECOND, Duration::from_secs(1)),
        };
        v.start_epoch(epoch, pose, now);
        v
    }

    /// Starts a new epoch at `pose` (a teleport or correction): allowances start full, the
    /// descent credit is 0, the velocity is zero and the next `MoveInput` is accepted
    /// whatever its `seq`. The settling state is the caller's and stays as it is.
    pub fn start_epoch(&mut self, epoch: u32, pose: Pose, now: u64) {
        self.epoch = epoch;
        self.pose = pose;
        self.last_seq = None;
        self.velocity = Vec3::ZERO;
        self.motion = Motion::STANDING;
        let w = self.limits.window.as_secs_f64();
        self.allow_h = self.limits.horizontal_speed * w;
        self.allow_up = self.limits.up_speed * w;
        self.allow_down = self.limits.down_speed * w;
        self.credit = 0.0;
        self.last_time = now;
    }

    /// Changes the limits. The allowances and the credit are capped anew at the next
    /// `check`.
    pub fn set_limits(&mut self, limits: MovementLimits) {
        self.limits = limits;
    }

    /// Turns the settling grace of `test-server.md` §8.3 on or off: while it is on, step 5
    /// skips the up and down tests. The horizontal test (with the descent credit) and the
    /// extent check still apply, and step 6 still lowers *A*ᵤ and *A*_d, so a move counts at
    /// most *A*_d of its descent toward the credit.
    pub fn set_settling(&mut self, settling: bool) {
        self.settling = settling;
    }

    /// Whether the settling grace is on.
    #[must_use]
    pub fn settling(&self) -> bool {
        self.settling
    }

    /// *C*, the descent credit held, in metres (always 0 without a credit).
    #[must_use]
    pub fn descent_credit(&self) -> f64 {
        self.credit
    }

    /// The current epoch.
    #[must_use]
    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    /// The last accepted pose.
    #[must_use]
    pub fn pose(&self) -> &Pose {
        &self.pose
    }

    /// The last accepted (clamped) velocity.
    #[must_use]
    pub fn velocity(&self) -> Vec3 {
        self.velocity
    }

    /// The last accepted motion.
    #[must_use]
    pub fn motion(&self) -> Motion {
        self.motion
    }

    fn grow(&mut self, now: u64) {
        let dt = now.saturating_sub(self.last_time) as f64 / 1e6;
        self.last_time = self.last_time.max(now);
        let w = self.limits.window.as_secs_f64();
        let vh = self.limits.horizontal_speed;
        // A_h never holds more than V_h · W; the credit is a pool of its own and never grows
        // with time (test-server.md §8.3).
        self.allow_h = (self.allow_h + vh * dt).min(vh * w);
        self.credit = self.credit.min(self.limits.reserve());
        self.allow_up = (self.allow_up + self.limits.up_speed * dt).min(self.limits.up_speed * w);
        self.allow_down =
            (self.allow_down + self.limits.down_speed * dt).min(self.limits.down_speed * w);
    }

    /// Runs steps 2–7 of §8.3 for `input` received at server time `now` (µs), steps 5 and 6
    /// as `test-server.md` §8.3 changes them when [`MovementLimits::descent_credit`] holds a
    /// reserve above 0 or [`Self::set_settling`] is on. Step 1 (not InWorld) is the caller's.
    /// The caller MAY run further checks of its own after `Accepted`.
    pub fn check(&mut self, input: &MoveInput, now: u64) -> MoveVerdict {
        if input.epoch != self.epoch {
            return MoveVerdict::StaleEpoch;
        }
        if let Some(last) = self.last_seq
            && !serial_newer(input.seq, last)
        {
            return MoveVerdict::OldSeq;
        }
        self.grow(now);
        let new = input.location;
        let p = self.pose.location;
        let in_extent = |c: f64| c >= self.limits.extent_min && c <= self.limits.extent_max;
        if new.region != p.region || !in_extent(new.x) || !in_extent(new.y) || !new.is_valid() {
            return self.reject(now);
        }
        let dh = p.distance_xy(&new);
        let dz = new.z - p.z;
        let (du, dd) = (dz.max(0.0), (-dz).max(0.0));
        // Counted descent d_c: the part of the descent that the down allowance covers (all
        // of it outside the settling grace, where a larger descent is rejected).
        let counted = dd.min(self.allow_down);
        let reserve = self.limits.reserve();
        let credit_on = reserve > 0.0;
        let h_budget = if credit_on {
            self.allow_h + counted + self.credit
        } else {
            self.allow_h
        };
        let vertical_ok = self.settling || (du <= self.allow_up && dd <= self.allow_down);
        if !(dh <= h_budget && vertical_ok) {
            return self.reject(now);
        }
        // Step 6: A_h pays first, the counted descent next and the credit last; descent the
        // move does not need is kept in C, up to S.
        let beyond = (dh - self.allow_h).max(0.0);
        self.allow_h = (self.allow_h - dh).max(0.0);
        self.allow_up = (self.allow_up - du).max(0.0);
        self.allow_down = (self.allow_down - counted).max(0.0);
        self.credit = if credit_on {
            (self.credit + counted - beyond).clamp(0.0, reserve)
        } else {
            0.0
        };
        self.last_seq = Some(input.seq);
        let anchor = self.pose.anchor.filter(|a| a.owns(&new));
        self.pose = Pose {
            location: new,
            heading: input.heading,
            anchor,
        };
        let vh = self.limits.horizontal_speed as f32;
        let mut v = input.velocity;
        let h = v.horizontal_length();
        if h > vh && h > 0.0 {
            let k = vh / h;
            v.x *= k;
            v.y *= k;
        }
        v.z = v.z.clamp(
            -(self.limits.down_speed as f32),
            self.limits.up_speed as f32,
        );
        self.velocity = v;
        self.motion = input.motion;
        MoveVerdict::Accepted
    }

    fn reject(&mut self, now: u64) -> MoveVerdict {
        if !self.corrections.try_record(now) {
            return MoveVerdict::Rejected;
        }
        let epoch = self.epoch.wrapping_add(1);
        let pose = self.pose;
        self.start_epoch(epoch, pose, now);
        MoveVerdict::Corrected(LocationReport {
            cause: LocationCause::Corrected,
            request_id: 0,
            epoch,
            server_time: now,
            pose,
        })
    }
}

/// The interest block of an entity (§9.1): its anchor's block if the anchor owns the
/// position, else the landblock containing the position with x and y clamped into
/// [0, 40,800].
#[must_use]
pub fn interest_block(location: &Location, anchor: Option<&Anchor>) -> (u8, u8) {
    if let Some(a) = anchor
        && a.owns(location)
        && a.block_x <= MAX_LANDBLOCK
        && a.block_y <= MAX_LANDBLOCK
    {
        return (a.block_x, a.block_y);
    }
    let index = |c: f64| -> u8 {
        let c = if c.is_finite() {
            c.clamp(0.0, GRID_EXTENT)
        } else {
            0.0
        };
        let i = (c / LANDBLOCK_SIZE).floor().min(f64::from(MAX_LANDBLOCK));
        // 0..=254 after the clamp.
        i as u8
    };
    (index(location.x), index(location.y))
}

/// Chebyshev distance between two blocks.
#[must_use]
pub fn block_distance(a: (u8, u8), b: (u8, u8)) -> u32 {
    u32::from(a.0.abs_diff(b.0)).max(u32::from(a.1.abs_diff(b.1)))
}

/// Interest membership after a re-evaluation (§9.2): an entity enters at distance ≤ *R*,
/// leaves at distance > *R* + 1, and otherwise keeps its membership (hysteresis).
#[must_use]
pub fn in_interest(currently_in: bool, distance: u32, radius: u8) -> bool {
    let r = u32::from(radius);
    if distance <= r {
        true
    } else if distance > r + 1 {
        false
    } else {
        currently_in
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Gait;

    fn input(seq: u32, epoch: u32, x: f64, y: f64, z: f64) -> MoveInput {
        MoveInput {
            seq,
            epoch,
            location: Location::new(1, x, y, z),
            heading: 0.0,
            velocity: Vec3::new(20.0, 0.0, -100.0),
            motion: Motion::new(Gait::Running, false),
        }
    }

    #[test]
    fn serial() {
        assert!(serial_newer(1, 0));
        assert!(serial_newer(0, u32::MAX));
        assert!(!serial_newer(5, 5));
        assert!(!serial_newer(4, 5));
    }

    #[test]
    fn validator() {
        let pose = Pose {
            location: Location::new(1, 1000.0, 1000.0, 10.0),
            heading: 0.0,
            anchor: Some(Anchor::new(6, 6, 3)),
        };
        let mut v = MovementValidator::new(MovementLimits::default(), 1, pose, 0);
        // 8.5 m allowance at start.
        assert_eq!(
            v.check(&input(10, 1, 1007.0, 1000.0, 10.0), 0),
            MoveVerdict::Accepted
        );
        assert_eq!(v.velocity().x, 8.5);
        assert_eq!(v.velocity().z, -100.0);
        assert!(v.pose().anchor.is_some());
        assert_eq!(
            v.check(&input(10, 1, 1007.0, 1000.0, 10.0), 0),
            MoveVerdict::OldSeq
        );
        assert_eq!(
            v.check(&input(11, 0, 1007.0, 1000.0, 10.0), 0),
            MoveVerdict::StaleEpoch
        );
        // Half a second later: 1.5 + 4.25 m available.
        assert_eq!(
            v.check(&input(11, 1, 1011.0, 1000.0, 10.0), 500_000),
            MoveVerdict::Accepted
        );
        match v.check(&input(12, 1, 1020.0, 1000.0, 10.0), 500_000) {
            MoveVerdict::Corrected(r) => {
                assert_eq!(r.epoch, 2);
                assert_eq!(r.pose.location.x, 1011.0);
                assert_eq!(r.request_id, 0);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(v.epoch(), 2);
        // Wrong region and leaving the extent are rejected.
        let mut bad = input(1, 2, 1011.0, 1000.0, 10.0);
        bad.location.region = 2;
        assert!(matches!(v.check(&bad, 600_000), MoveVerdict::Corrected(_)));
        assert!(matches!(
            v.check(&input(1, 3, -1281.0, 1000.0, 10.0), 600_000),
            MoveVerdict::Corrected(_)
        ));
        // Four corrections per second at most.
        assert!(matches!(
            v.check(&input(1, 4, 5000.0, 1000.0, 10.0), 600_000),
            MoveVerdict::Corrected(_)
        ));
        assert_eq!(
            v.check(&input(1, 5, 5000.0, 1000.0, 10.0), 600_000),
            MoveVerdict::Rejected
        );
        assert_eq!(v.epoch(), 5);
        // Leaving the anchor's square drops the anchor.
        assert!(matches!(
            v.check(&input(1, 5, 1011.0, 1000.0, 3.0), 2_000_000),
            MoveVerdict::Accepted
        ));
    }

    const SEND_US: u64 = 66_667;

    fn flat_pose() -> Pose {
        Pose {
            location: Location::new(1, 10_000.0, 10_000.0, 1_000.0),
            heading: 0.0,
            anchor: None,
        }
    }

    /// The server's default reserve *S* (`test-server.md` §4.5 `slide_reserve`).
    const RESERVE: f64 = 56.0;

    fn server_limits() -> MovementLimits {
        MovementLimits {
            descent_credit: Some(RESERVE),
            ..MovementLimits::default()
        }
    }

    /// Feeds 15 Hz `MoveInput`s east from `v`'s pose, one per (horizontal, vertical)
    /// speed pair in m/s, from server time `now`. Returns the index of the first input
    /// not accepted, if any, and the time after the last one.
    fn drive(
        v: &mut MovementValidator,
        seq: &mut u32,
        now: u64,
        speeds: impl IntoIterator<Item = (f64, f64)>,
    ) -> (Option<usize>, u64) {
        drive_with(v, seq, now, speeds, |_, t| t)
    }

    /// [`drive`] with the server time at which input `i` (sent at `t`) arrives given by
    /// `arrival(i, t)`; arrival times must not decrease.
    fn drive_with(
        v: &mut MovementValidator,
        seq: &mut u32,
        mut now: u64,
        speeds: impl IntoIterator<Item = (f64, f64)>,
        arrival: impl Fn(usize, u64) -> u64,
    ) -> (Option<usize>, u64) {
        let dt = SEND_US as f64 / 1e6;
        for (i, (h, vz)) in speeds.into_iter().enumerate() {
            now += SEND_US;
            *seq += 1;
            let p = v.pose().location;
            let m = MoveInput {
                seq: *seq,
                epoch: v.epoch(),
                location: Location::new(1, p.x + h * dt, p.y, p.z + vz * dt),
                heading: 0.0,
                velocity: Vec3::new(h as f32, 0.0, vz as f32),
                motion: Motion::new(Gait::Running, false),
            };
            if v.check(&m, arrival(i, now)) != MoveVerdict::Accepted {
                return (Some(i), now);
            }
        }
        (None, now)
    }

    /// `n` sends at constant speeds.
    fn steady(n: usize, h: f64, vz: f64) -> impl Iterator<Item = (f64, f64)> {
        std::iter::repeat_n((h, vz), n)
    }

    /// The controller step of `collision.md` §12.3, 1/60 s.
    const STEP: f64 = 1.0 / 60.0;

    /// Collects 1/60 s steps into 15 Hz sends (four steps each) as average speeds.
    #[derive(Default)]
    struct Stepper {
        out: Vec<(f64, f64)>,
        dx: f64,
        dz: f64,
        steps: u32,
    }

    impl Stepper {
        /// One step at average horizontal speed `h` and vertical speed `vz`.
        fn step(&mut self, h: f64, vz: f64) {
            self.dx += h * STEP;
            self.dz += vz * STEP;
            self.steps += 1;
            if self.steps == 4 {
                let t = 4.0 * STEP;
                self.out.push((self.dx / t, self.dz / t));
                (self.dx, self.dz, self.steps) = (0.0, 0.0, 0);
            }
        }

        /// A slide from rest down a face `angle` degrees steep (Slope layer: gravity
        /// 10 m/s², no control) up to the terminal velocity of 50 m/s, then `extra` steps at
        /// it (`test-server.md` §8.3).
        fn face(&mut self, angle: f64, extra: usize) {
            let a = angle.to_radians();
            let mut s: f64 = 0.0;
            let mut left = extra;
            while s < 50.0 || left > 0 {
                if s >= 50.0 {
                    left -= 1;
                }
                let next = (s + 10.0 * a.sin() * STEP).min(50.0);
                let avg = (s + next) / 2.0;
                self.step(avg * a.cos(), -avg * a.sin());
                s = next;
            }
        }

        /// The run-out on level walkable ground from horizontal speed `h0` down to 6.5 m/s.
        /// The ground brakes at 38.5 m/s² (the damper of 0.2 left out, so it brakes less than
        /// the controller); with `jump`, the walker jumps (5.2 m/s, Falling gravity 15 m/s²)
        /// after every ground step and the air brakes at only 10 m/s² toward 5 m/s.
        fn run_out(&mut self, h0: f64, jump: bool) {
            let (mut v, mut z, mut vz, mut air) = (h0, 0.0f64, 0.0f64, false);
            while v > 6.5 || air {
                let (v0, z0) = (v, z);
                if air {
                    v = (v - 10.0 * STEP).max(5.0);
                    vz -= 15.0 * STEP;
                    z += vz * STEP;
                    if z <= 0.0 {
                        (z, vz, air) = (0.0, 0.0, false);
                    }
                } else {
                    v = (v - 38.5 * STEP).max(6.5);
                    if jump && v > 6.5 {
                        (vz, air) = (5.2, true);
                    }
                }
                self.step((v0 + v) / 2.0, (z - z0) / STEP);
            }
        }

        /// Pads the last send and returns all sends.
        fn finish(mut self) -> Vec<(f64, f64)> {
            while self.steps != 0 {
                self.step(0.0, 0.0);
            }
            self.out
        }
    }

    /// The fastest landing speed on walkable ground after a face (`test-server.md` §8.3):
    /// 50 × (1 + cos 45°) / 2.
    const LANDING: f64 = 42.7;

    /// A slide from rest down a face `angle` degrees steep to the terminal velocity and
    /// 0.5 s beyond, a landing at [`LANDING`] (more than such a face gives onto level
    /// ground), the run-out, then 3 s at 6.5 m/s. Returns the sends and the index of the
    /// first send after the face.
    fn slide_and_run_out(angle: f64, jump: bool) -> (Vec<(f64, f64)>, usize) {
        let mut st = Stepper::default();
        st.face(angle, 30);
        while st.steps != 0 {
            st.step(0.0, 0.0);
        }
        let foot = st.out.len();
        st.run_out(LANDING, jump);
        for _ in 0..180 {
            st.step(6.5, 0.0);
        }
        (st.finish(), foot)
    }

    #[test]
    fn run_out_after_a_46_degree_face_is_accepted() {
        for (jump, stall) in [(false, false), (true, false), (true, true)] {
            let (speeds, foot) = slide_and_run_out(46.0, jump);
            // With `stall`, the first second of the run-out arrives at once, at its end.
            let held = foot + 15;
            let arrival = |i: usize, t: u64| {
                if stall && (foot..held).contains(&i) {
                    t + (held - i) as u64 * SEND_US
                } else {
                    t
                }
            };
            let mut v = MovementValidator::new(server_limits(), 1, flat_pose(), 0);
            let mut seq = 0;
            let (fail, now) = drive(&mut v, &mut seq, 0, speeds[..foot].iter().copied());
            assert_eq!(fail, None);
            // The face filled the credit (test-server.md §8.3 "the credit fills in time").
            assert!(v.descent_credit() > RESERVE - 0.5, "{}", v.descent_credit());
            let (fail, _) = drive_with(
                &mut v,
                &mut seq,
                now,
                speeds[foot..].iter().copied(),
                |i, t| arrival(i + foot, t),
            );
            assert_eq!(fail, None, "jump {jump}, stall {stall}");
            assert_eq!(v.epoch(), 1, "no correction");
            assert!(v.velocity().horizontal_length() <= 8.5);
            assert!(v.descent_credit() <= RESERVE);
            // Without the credit (the protocol's procedure, or S = 0) the face itself is
            // corrected.
            for credit in [None, Some(0.0)] {
                let limits = MovementLimits {
                    descent_credit: credit,
                    ..MovementLimits::default()
                };
                let mut v = MovementValidator::new(limits, 1, flat_pose(), 0);
                let mut seq = 0;
                let (fail, _) = drive(&mut v, &mut seq, 0, speeds.iter().copied());
                assert!(fail.is_some_and(|i| i < foot), "{fail:?}");
            }
        }
    }

    #[test]
    fn slide_right_after_spawn_is_accepted() {
        // The client sees its ground `drop` metres below the spawn and settles onto it in the
        // first move, then slides from rest down a 46° face, runs out jumping and runs on.
        // The settling grace (3 s) ends during the face.
        for drop in [2.0, 200.0] {
            let (speeds, _) = slide_and_run_out(46.0, true);
            let mut v = MovementValidator::new(server_limits(), 1, flat_pose(), 0);
            v.set_settling(true);
            let mut seq = 0;
            let (fail, now) = drive(&mut v, &mut seq, 0, steady(1, 0.0, -drop / 0.066_667));
            assert_eq!(fail, None, "settling move of {drop} m");
            let (fail, now) = drive(&mut v, &mut seq, now, speeds[..44].iter().copied());
            assert_eq!(fail, None, "drop {drop}");
            v.set_settling(false);
            let (fail, _) = drive(&mut v, &mut seq, now, speeds[44..].iter().copied());
            assert_eq!(fail.map(|i| i + 44), None, "drop {drop}");
            assert_eq!(v.epoch(), 1, "no correction");
        }
        // Outside the grace the 200 m settling move is corrected.
        let mut v = MovementValidator::new(server_limits(), 1, flat_pose(), 0);
        let mut seq = 0;
        let (fail, _) = drive(&mut v, &mut seq, 0, steady(1, 0.0, -200.0 / 0.066_667));
        assert_eq!(fail, Some(0));
    }

    fn burst(v: &MovementValidator, seq: u32, dx: f64, dz: f64) -> MoveInput {
        let p = v.pose().location;
        MoveInput {
            seq,
            epoch: v.epoch(),
            location: Location::new(1, p.x + dx, p.y, p.z + dz),
            heading: 0.0,
            velocity: Vec3::ZERO,
            motion: Motion::new(Gait::Running, false),
        }
    }

    #[test]
    fn claimed_drop_during_settling_earns_at_most_the_reserve() {
        let mut v = MovementValidator::new(server_limits(), 1, flat_pose(), 0);
        v.set_settling(true);
        // A 5,000 m drop passes the grace but counts only A_d = V_d · W = 120 m, and the
        // credit keeps at most S of it.
        assert_eq!(
            v.check(&burst(&v, 1, 0.0, -5_000.0), 0),
            MoveVerdict::Accepted
        );
        assert_eq!(v.descent_credit(), RESERVE);
        let mut w = v.clone();
        assert!(matches!(
            w.check(&burst(&w, 2, 8.5 + RESERVE + 0.1, 0.0), 0),
            MoveVerdict::Corrected(_)
        ));
        let mut w = v.clone();
        assert_eq!(
            w.check(&burst(&w, 2, 8.5 + RESERVE - 0.1, 0.0), 0),
            MoveVerdict::Accepted
        );
        // A_d is still tracked: right after the grace a further metre down is too much.
        let mut w = v.clone();
        w.set_settling(false);
        assert!(matches!(
            w.check(&burst(&w, 2, 0.0, -1.0), 0),
            MoveVerdict::Corrected(_)
        ));
        // So is A_u: a 5,000 m rise passes the grace, a further metre after it does not.
        assert_eq!(
            v.check(&burst(&v, 2, 0.0, 5_000.0), 0),
            MoveVerdict::Accepted
        );
        v.set_settling(false);
        assert!(matches!(
            v.check(&burst(&v, 3, 0.0, 1.0), 0),
            MoveVerdict::Corrected(_)
        ));
    }

    #[test]
    fn descent_credit_is_capped_and_cleared() {
        // A long fall fills the credit up to S; A_h stays at V_h · W.
        let mut v = MovementValidator::new(server_limits(), 1, flat_pose(), 0);
        let mut seq = 0;
        let (fail, now) = drive(&mut v, &mut seq, 0, steady(30, 0.0, -50.0));
        assert_eq!(fail, None);
        assert_eq!(v.descent_credit(), RESERVE);
        let mut w = v.clone();
        assert!(matches!(
            w.check(&burst(&w, seq + 1, 64.6, 0.0), now),
            MoveVerdict::Corrected(_)
        ));
        // The correction cleared it.
        assert_eq!(w.descent_credit(), 0.0);
        let mut w = v.clone();
        assert_eq!(
            w.check(&burst(&w, seq + 1, 64.4, 0.0), now),
            MoveVerdict::Accepted
        );
        // Spent: the next metre right away is too much.
        assert!(matches!(
            w.check(&burst(&w, seq + 2, 1.0, 0.0), now),
            MoveVerdict::Corrected(_)
        ));
        // It does not grow with time: a second later only A_h has refilled.
        let mut w = v.clone();
        assert_eq!(
            w.check(&burst(&w, seq + 1, 60.0, 0.0), now),
            MoveVerdict::Accepted
        );
        let later = now + 1_000_000;
        let mut x = w.clone();
        assert!(matches!(
            x.check(&burst(&x, seq + 2, 8.5 + 4.6, 0.0), later),
            MoveVerdict::Corrected(_)
        ));
        assert_eq!(
            w.check(&burst(&w, seq + 2, 8.5 + 4.4, 0.0), later),
            MoveVerdict::Accepted
        );
        // A new epoch clears it.
        let pose = *v.pose();
        v.start_epoch(2, pose, now);
        assert_eq!(v.descent_credit(), 0.0);
        assert!(matches!(
            v.check(&burst(&v, 1, 8.6, 0.0), now),
            MoveVerdict::Corrected(_)
        ));
        // Lowering S caps it.
        let mut v = MovementValidator::new(server_limits(), 1, flat_pose(), 0);
        let mut seq = 0;
        let (_, now) = drive(&mut v, &mut seq, 0, steady(30, 0.0, -50.0));
        v.set_limits(MovementLimits {
            descent_credit: Some(10.0),
            ..MovementLimits::default()
        });
        assert!(matches!(
            v.clone().check(&burst(&v, seq + 1, 18.6, 0.0), now),
            MoveVerdict::Corrected(_)
        ));
        assert_eq!(
            v.check(&burst(&v, seq + 1, 18.4, 0.0), now),
            MoveVerdict::Accepted
        );
        // A reserve of 0, a negative one or NaN turns the credit off, also for the move's
        // own descent.
        for s in [0.0, -5.0, f64::NAN] {
            let limits = MovementLimits {
                descent_credit: Some(s),
                ..MovementLimits::default()
            };
            let mut v = MovementValidator::new(limits, 1, flat_pose(), 0);
            let mut seq = 0;
            let (_, now) = drive(&mut v, &mut seq, 0, steady(30, 0.0, -50.0));
            assert_eq!(v.descent_credit(), 0.0);
            assert!(matches!(
                v.clone().check(&burst(&v, seq + 1, 8.6, 0.0), now),
                MoveVerdict::Corrected(_)
            ));
            assert!(matches!(
                v.check(&burst(&v, seq + 1, 8.6, -20.0), now),
                MoveVerdict::Corrected(_)
            ));
        }
    }

    /// `test-server.md` §14 invariant 8 on pseudo-random input outside the settling grace,
    /// where the counted descent is all of each accepted move's descent.
    #[test]
    fn accepted_distance_stays_within_invariant_8() {
        let mut rng: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng >> 11) as f64 / (1u64 << 53) as f64
        };
        for reserve in [None, Some(0.0), Some(RESERVE)] {
            let limits = MovementLimits {
                descent_credit: reserve,
                ..MovementLimits::default()
            };
            let s = limits.reserve();
            let mut v = MovementValidator::new(limits, 1, flat_pose(), 0);
            // (time s, d_h, d_d) of the accepted moves of the current epoch.
            let mut moves: Vec<(f64, f64, f64)> = Vec::new();
            let mut now = 0u64;
            let mut seq = 0u32;
            let mut accepted = 0;
            for _ in 0..3_000 {
                now += (next() * 150_000.0) as u64;
                seq += 1;
                let dt = 0.066_667;
                let (h, vz) = (next() * 40.0, next() * 70.0 - 60.0);
                let m = burst(&v, seq, h * dt, vz * dt);
                let before = v.epoch();
                match v.check(&m, now) {
                    MoveVerdict::Accepted => {
                        accepted += 1;
                        let dd = (-vz * dt).max(0.0);
                        moves.push((now as f64 / 1e6, h * dt, dd));
                        assert!(v.descent_credit() <= s);
                    }
                    _ => {
                        if v.epoch() != before {
                            moves.clear();
                        }
                    }
                }
                // Every stretch ending at the newest move.
                if let Some(&(t_end, _, _)) = moves.last() {
                    let (mut dh, mut dd) = (0.0, 0.0);
                    for &(t, h, d) in moves.iter().rev() {
                        dh += h;
                        dd += d;
                        let bound = 8.5 * (t_end - t + 1.0) + s + dd;
                        assert!(dh <= bound + 1e-6, "{dh} > {bound}");
                    }
                }
            }
            assert!(accepted > 300, "{accepted}");
        }
    }

    /// A slide at the default terminal velocity of 50 m/s down a 47° face for 3 s, then
    /// the run-out on walkable ground braking at 38.5 m/s² down to 6.5 m/s, then 3 s of
    /// running at 6.5 m/s (`test-server.md` §4.5, §8.3).
    fn slide() -> Vec<(f64, f64)> {
        let a = 47f64.to_radians();
        let (h, vz) = (50.0 * a.cos(), -50.0 * a.sin());
        let mut out: Vec<(f64, f64)> = steady(45, h, vz).collect();
        let dt = SEND_US as f64 / 1e6;
        let mut speed = h;
        while speed > 6.5 {
            let next = (speed - 38.5 * dt).max(6.5);
            out.push(((speed + next) / 2.0, 0.0));
            speed = next;
        }
        out.extend(steady(45, 6.5, 0.0));
        out
    }

    #[test]
    fn fast_legitimate_slide_is_accepted_with_the_descent_credit() {
        let mut v = MovementValidator::new(server_limits(), 1, flat_pose(), 0);
        let mut seq = 0;
        let (fail, _) = drive(&mut v, &mut seq, 0, slide());
        assert_eq!(fail, None);
        assert_eq!(v.epoch(), 1, "no correction");
        // The relayed velocity is still clamped to V_h.
        assert!(v.velocity().horizontal_length() <= 8.5);
        // The protocol's procedure without the credit corrects the same slide at once.
        let mut v = MovementValidator::new(MovementLimits::default(), 1, flat_pose(), 0);
        let mut seq = 0;
        let (fail, _) = drive(&mut v, &mut seq, 0, slide());
        assert!(fail.is_some_and(|i| i < 10), "{fail:?}");
    }

    #[test]
    fn fastest_controlled_run_is_accepted() {
        for limits in [MovementLimits::default(), server_limits()] {
            let mut v = MovementValidator::new(limits, 1, flat_pose(), 0);
            let mut seq = 0;
            // 6.5 m/s on the flat and up a 45° slope (6.5 m/s along it), for 10 s each.
            let up = 6.5 * 45f64.to_radians().sin();
            let (fail, now) = drive(&mut v, &mut seq, 0, steady(150, 6.5, 0.0));
            assert_eq!(fail, None);
            let (fail, _) = drive(&mut v, &mut seq, now, steady(150, up, up));
            assert_eq!(fail, None);
        }
    }

    #[test]
    fn too_fast_move_is_corrected() {
        for limits in [MovementLimits::default(), server_limits()] {
            // Sustained 10 m/s on the flat: the 8.5 m burst drains at 1.5 m/s (about 5.3 s).
            let mut v = MovementValidator::new(limits, 1, flat_pose(), 0);
            let mut seq = 0;
            let (fail, _) = drive(&mut v, &mut seq, 0, steady(150, 10.0, 0.0));
            assert!(fail.is_some_and(|i| (75..=90).contains(&i)), "{fail:?}");
            assert_eq!(v.epoch(), 2);
            // One 9 m jump right after the epoch starts.
            let mut v = MovementValidator::new(limits, 1, flat_pose(), 0);
            let mut m = input(1, 1, 10_009.0, 10_000.0, 1_000.0);
            m.velocity = Vec3::ZERO;
            assert!(matches!(v.check(&m, 0), MoveVerdict::Corrected(_)));
            // Rising faster than V_u.
            let mut v = MovementValidator::new(limits, 1, flat_pose(), 0);
            let mut seq = 0;
            let (fail, _) = drive(&mut v, &mut seq, 0, steady(60, 0.0, 15.0));
            assert!(fail.is_some(), "rise at 15 m/s");
        }
    }

    #[test]
    fn interest() {
        let l = Location::new(1, 20_250.5, 22_500.25, 0.0);
        assert_eq!(interest_block(&l, None), (126, 140));
        assert_eq!(
            interest_block(&l, Some(&Anchor::new(127, 140, 1))),
            (127, 140)
        );
        assert_eq!(
            interest_block(&Location::new(1, -50.0, 99_999.0, 0.0), None),
            (0, 254)
        );
        assert_eq!(block_distance((1, 1), (3, 0)), 2);
        assert!(in_interest(false, 2, 2));
        assert!(!in_interest(false, 3, 2));
        assert!(in_interest(true, 3, 2));
        assert!(!in_interest(true, 4, 2));
    }

    #[test]
    fn schedule() {
        let mut s = MoveScheduler::new();
        let run = Motion::new(Gait::Running, false);
        let stand = Motion::STANDING;
        let sends: Vec<bool> = (0..8).map(|t| s.on_tick(t, true, run, false)).collect();
        assert_eq!(sends, [true, false, true, false, true, false, true, false]);
        // Stop: send now and the next three ticks, then a 1 Hz heartbeat.
        assert!(s.on_tick(8, false, stand, false));
        assert!(s.on_tick(9, false, stand, false));
        assert!(s.on_tick(10, false, stand, false));
        assert!(s.on_tick(11, false, stand, false));
        assert!(!s.on_tick(12, false, stand, false));
        assert!(!s.on_tick(40, false, stand, false));
        assert!(s.on_tick(41, false, stand, false));
        assert!(s.on_tick(42, false, stand, true));
    }
}
