// SPDX-License-Identifier: GPL-3.0-or-later
//! Snapshot interpolation for remote entities (`docs/specs/tessera-protocol.md` §8.6).
//!
//! A server relays other entities' movement as timestamped snapshots (server time in
//! microseconds, §11.1) at a few tens of hertz. A client draws each remote entity at a
//! **render time** a little behind the estimated server time, so it nearly always has two
//! snapshots around that moment and can blend between them:
//!
//! - [`SnapshotBuffer`]: one entity's recent snapshots. Stale snapshots (not newer than the
//!   newest one) are refused, history is bounded in time and count, a jump larger than a
//!   plausible move is marked as a discontinuity and drawn as a snap, and sampling
//!   interpolates position and velocity linearly and heading along the shorter arc,
//!   extrapolates for a bounded time past the newest snapshot and then holds.
//! - [`InterpolationSet`]: one buffer per [`EntityId`], for a client that keeps all remote
//!   entities in one place.
//! - [`render_time`] and [`recommended_delay`]: the render-time rule of §8.6.
//!
//! Nothing here does I/O or reads a clock: the caller passes times in, so the module works
//! for any backend that supplies server timestamps (the test-server backend,
//! `tessera_net::remote`, keeps an [`InterpolationSet`] and exposes it).
//!
//! ```
//! use tessera_net::Vec3;
//! use tessera_net::interp::{InterpConfig, SampleKind, Snapshot, SnapshotBuffer};
//!
//! let cfg = InterpConfig::default();
//! let mut buf = SnapshotBuffer::new();
//! buf.push(Snapshot::at(1_000_000, Vec3::new(0.0, 0.0, 0.0)), &cfg);
//! buf.push(Snapshot::at(1_100_000, Vec3::new(1.0, 0.0, 0.0)), &cfg);
//! let s = buf.sample(1_050_000, &cfg).unwrap();
//! assert_eq!(s.kind, SampleKind::Interpolated);
//! assert!((s.position.x - 0.5).abs() < 1e-6);
//! ```

use std::collections::{BTreeMap, VecDeque};
use std::f64::consts::{PI, TAU};
use std::time::Duration;

use tessera_protocol::limits::MAX_HORIZONTAL_SPEED;
pub use tessera_protocol::types::{Gait, Motion};

use crate::NetError;
use crate::model::{EntityId, Vec3};

/// The recommended interpolation delay *D* of §8.6 (two 15 Hz snapshot intervals plus a
/// margin).
pub const DEFAULT_DELAY: Duration = Duration::from_millis(150);
/// How long a sample may run past the newest snapshot on its velocity before it holds
/// (§8.6).
pub const DEFAULT_MAX_EXTRAPOLATION: Duration = Duration::from_millis(250);
/// The horizontal speed bound *V*ₕ of §8.3 (8.5 m/s; independent of `RUN_SPEED`, §7.3), used
/// by the teleport test of §8.6.
pub const DEFAULT_SNAP_SPEED: f32 = MAX_HORIZONTAL_SPEED as f32;
/// The fixed margin of the teleport test of §8.6, in metres.
pub const DEFAULT_SNAP_MARGIN: f32 = 5.0;
/// How much history a buffer keeps behind its newest snapshot (§8.6: "the snapshots
/// received in the last second").
pub const DEFAULT_HISTORY: Duration = Duration::from_secs(1);
/// Hard cap on the snapshots one buffer keeps, whatever their timestamps (a hostile or
/// buggy source cannot grow a buffer beyond this).
pub const MAX_SNAPSHOTS: usize = 64;
/// Largest accepted [`InterpConfig::delay`] and [`InterpConfig::max_extrapolation`].
pub const MAX_CONFIG_DURATION: Duration = Duration::from_secs(10);

/// Interpolation parameters. The defaults are the recommendations of §8.6.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InterpConfig {
    /// The interpolation delay *D*: render time = estimated server time − *D*. See
    /// [`recommended_delay`].
    pub delay: Duration,
    /// Extrapolation limit past the newest snapshot; afterwards the position holds.
    pub max_extrapolation: Duration,
    /// *V*ₕ in m/s: two consecutive snapshots further apart than
    /// `snap_speed · Δt + snap_margin` are a teleport or correction and are not blended.
    pub snap_speed: f32,
    /// The margin of the teleport test, metres.
    pub snap_margin: f32,
    /// History kept behind the newest snapshot.
    pub history: Duration,
}

impl Default for InterpConfig {
    fn default() -> Self {
        Self {
            delay: DEFAULT_DELAY,
            max_extrapolation: DEFAULT_MAX_EXTRAPOLATION,
            snap_speed: DEFAULT_SNAP_SPEED,
            snap_margin: DEFAULT_SNAP_MARGIN,
            history: DEFAULT_HISTORY,
        }
    }
}

impl InterpConfig {
    /// Checks the ranges: `delay` and `max_extrapolation` up to [`MAX_CONFIG_DURATION`],
    /// `history` at least the delay and at most twice [`MAX_CONFIG_DURATION`], and finite,
    /// non-negative snap parameters.
    ///
    /// History at least the delay is enough for a sample at render time to find its
    /// bracket: a buffer keeps the newest snapshot older than its history cut, so any render
    /// time up to `history` behind the newest snapshot lies inside the buffer. The
    /// extrapolation limit needs no history (it runs past the newest snapshot).
    /// [`for_server_rates`](Self::for_server_rates) adds one snapshot interval of margin
    /// for clock-estimate jitter.
    ///
    /// # Errors
    ///
    /// [`NetError::InvalidConfig`] naming the bad value.
    pub fn validate(&self) -> Result<(), NetError> {
        if self.delay > MAX_CONFIG_DURATION || self.max_extrapolation > MAX_CONFIG_DURATION {
            return Err(NetError::InvalidConfig(
                "interpolation delay or extrapolation above 10 s",
            ));
        }
        if self.history > MAX_CONFIG_DURATION * 2 || self.history < self.delay {
            return Err(NetError::InvalidConfig(
                "interpolation history shorter than the delay or above 20 s",
            ));
        }
        if !(self.snap_speed.is_finite() && self.snap_speed >= 0.0)
            || !(self.snap_margin.is_finite() && self.snap_margin >= 0.0)
        {
            return Err(NetError::InvalidConfig(
                "snap speed and margin must be finite and non-negative",
            ));
        }
        Ok(())
    }

    /// The configuration for a server's rates (§8.6): the delay raised to
    /// [`recommended_delay`] when that is larger, and the history raised, when needed, to
    /// the delay plus one snapshot interval (at most twice [`MAX_CONFIG_DURATION`]), so the
    /// snapshots a render time *D* behind the newest one needs are kept even at low
    /// snapshot rates. Nothing is lowered. Zero rates are treated as 1 Hz.
    #[must_use]
    pub fn for_server_rates(mut self, snapshot_rate: u8, tick_rate: u8) -> Self {
        self.delay = self.delay.max(recommended_delay(snapshot_rate, tick_rate));
        let interval =
            Duration::from_micros(1_000_000u64.div_ceil(u64::from(snapshot_rate.max(1))));
        let wanted = (self.delay + interval).min(MAX_CONFIG_DURATION * 2);
        self.history = self.history.max(wanted);
        self
    }
}

/// The interpolation delay for a server's rates (§8.6): the larger of
/// [`DEFAULT_DELAY`] and 2 / `snapshot_rate` + 1 / `tick_rate`, rounded up to the
/// millisecond. Zero rates are treated as 1 Hz.
#[must_use]
pub fn recommended_delay(snapshot_rate: u8, tick_rate: u8) -> Duration {
    let snap = u64::from(snapshot_rate.max(1));
    let tick = u64::from(tick_rate.max(1));
    // 2/snap + 1/tick seconds in µs, rounded up to whole milliseconds.
    let us = (2_000_000u64).div_ceil(snap) + 1_000_000u64.div_ceil(tick);
    Duration::from_millis(us.div_ceil(1000)).max(DEFAULT_DELAY)
}

/// Render time for an estimated server time (both µs): `server_now − delay`, saturating
/// at zero.
#[must_use]
pub fn render_time(server_now: u64, delay: Duration) -> u64 {
    server_now.saturating_sub(u64::try_from(delay.as_micros()).unwrap_or(u64::MAX))
}

/// One received state of a remote entity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapshot {
    /// Server time of the state, µs.
    pub server_time: u64,
    /// Position in the receiver's current region frame, metres.
    pub position: Vec3,
    /// Facing in radians (0 = +X east, counter-clockwise from above).
    pub heading: f32,
    /// Velocity in m/s.
    pub velocity: Vec3,
    /// Gait and backward flag (§8.1).
    pub motion: Motion,
}

impl Snapshot {
    /// A resting snapshot at `position`, heading 0, no velocity.
    #[must_use]
    pub fn at(server_time: u64, position: Vec3) -> Self {
        Self {
            server_time,
            position,
            heading: 0.0,
            velocity: Vec3::ZERO,
            motion: Motion::STANDING,
        }
    }

    /// True when position, heading and velocity are finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.position.is_finite() && self.heading.is_finite() && self.velocity.is_finite()
    }
}

/// How a [`Sample`] was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleKind {
    /// Blended between the two snapshots around the render time.
    Interpolated,
    /// Render time lies between two snapshots that are too far apart for the move to be
    /// real (a teleport or correction, §8.6): the earlier state until the later one's time,
    /// then the later one. Never blended.
    Snapped,
    /// Render time lies before the oldest snapshot (a newly created entity): its state.
    Early,
    /// Past the newest snapshot, moved on along its velocity (at most
    /// [`InterpConfig::max_extrapolation`]).
    Extrapolated,
    /// Further past the newest snapshot than the extrapolation limit: position held where
    /// extrapolation stopped. The velocity is still the newest snapshot's, so a caller can
    /// decide how to animate an entity whose updates are late.
    Held,
}

/// The state of an entity at a render time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// Position, metres.
    pub position: Vec3,
    /// Facing in radians, reduced to (−π, π].
    pub heading: f32,
    /// Velocity for animation, m/s.
    pub velocity: Vec3,
    /// The later snapshot's motion (§8.6).
    pub motion: Motion,
    /// How the sample was made.
    pub kind: SampleKind,
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    snap: Snapshot,
    /// The move from the previous snapshot to this one is not plausible: do not blend.
    jump: bool,
}

/// The recent snapshots of one remote entity (§8.6).
#[derive(Debug, Clone, Default)]
pub struct SnapshotBuffer {
    entries: VecDeque<Entry>,
}

impl SnapshotBuffer {
    /// An empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A buffer that starts at `snapshot` (an entity's creation state).
    #[must_use]
    pub fn starting_at(snapshot: Snapshot) -> Self {
        let mut b = Self::new();
        b.reset(snapshot);
        b
    }

    /// Number of snapshots held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when no snapshot is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The newest snapshot.
    #[must_use]
    pub fn newest(&self) -> Option<&Snapshot> {
        self.entries.back().map(|e| &e.snap)
    }

    /// The server time of the newest snapshot (§8.6 "newest applied").
    #[must_use]
    pub fn newest_time(&self) -> Option<u64> {
        self.newest().map(|s| s.server_time)
    }

    /// Forgets everything and starts again at `snapshot` (a teleport of the entity, or a
    /// re-creation). Non-finite snapshots leave the buffer empty.
    pub fn reset(&mut self, snapshot: Snapshot) {
        self.entries.clear();
        if snapshot.is_finite() {
            self.entries.push_back(Entry {
                snap: snapshot,
                jump: false,
            });
        }
    }

    /// Adds a snapshot. Returns false (and changes nothing) when it is not finite or not
    /// newer than the newest snapshot held (§8.6 staleness). A snapshot that lies further
    /// from the previous one than `snap_speed · Δt + snap_margin` is kept but marked as a
    /// jump, so [`sample`](Self::sample) snaps across it.
    pub fn push(&mut self, snapshot: Snapshot, cfg: &InterpConfig) -> bool {
        if !snapshot.is_finite() {
            return false;
        }
        let jump = match self.entries.back() {
            Some(last) if snapshot.server_time <= last.snap.server_time => return false,
            Some(last) => {
                let dt = (snapshot.server_time - last.snap.server_time) as f64 / 1e6;
                let allowed = f64::from(cfg.snap_speed) * dt + f64::from(cfg.snap_margin);
                distance(last.snap.position, snapshot.position) > allowed
            }
            None => false,
        };
        self.entries.push_back(Entry {
            snap: snapshot,
            jump,
        });
        self.prune(cfg);
        true
    }

    /// Drops snapshots older than the history behind the newest one, keeping the newest one
    /// older than the cut (so a render time just after the cut still has its bracket), and
    /// never more than [`MAX_SNAPSHOTS`]. The history used is never shorter than the delay,
    /// even for a configuration that fails [`InterpConfig::validate`].
    fn prune(&mut self, cfg: &InterpConfig) {
        while self.entries.len() > MAX_SNAPSHOTS {
            self.entries.pop_front();
        }
        let Some(newest) = self.newest_time() else {
            return;
        };
        let keep = cfg.history.max(cfg.delay);
        let cut = newest.saturating_sub(u64::try_from(keep.as_micros()).unwrap_or(u64::MAX));
        while self.entries.len() > 2
            && self
                .entries
                .get(1)
                .is_some_and(|e| e.snap.server_time <= cut)
        {
            self.entries.pop_front();
        }
    }

    /// The entity's state at `render_time` (server µs), or `None` when the buffer is empty.
    #[must_use]
    pub fn sample(&self, render_time: u64, cfg: &InterpConfig) -> Option<Sample> {
        let first = self.entries.front()?;
        let last = self.entries.back()?;
        if render_time <= first.snap.server_time {
            return Some(state(&first.snap, SampleKind::Early));
        }
        if render_time >= last.snap.server_time {
            let late = render_time - last.snap.server_time;
            let max = u64::try_from(cfg.max_extrapolation.as_micros()).unwrap_or(u64::MAX);
            let (dt, kind) = if late <= max {
                (late, SampleKind::Extrapolated)
            } else {
                (max, SampleKind::Held)
            };
            let t = dt as f32 / 1e6;
            let s = &last.snap;
            return Some(Sample {
                position: s.position + s.velocity * t,
                heading: reduce(f64::from(s.heading)),
                velocity: s.velocity,
                motion: s.motion,
                kind,
            });
        }
        // first.time < render_time < last.time: find b, the first entry later than it.
        let i = self
            .entries
            .partition_point(|e| e.snap.server_time <= render_time);
        let (a, b) = (self.entries.get(i.checked_sub(1)?)?, self.entries.get(i)?);
        if b.jump {
            return Some(state(&a.snap, SampleKind::Snapped));
        }
        let span = (b.snap.server_time - a.snap.server_time) as f64;
        let t = ((render_time - a.snap.server_time) as f64 / span).clamp(0.0, 1.0);
        Some(Sample {
            position: lerp(a.snap.position, b.snap.position, t),
            heading: lerp_heading(f64::from(a.snap.heading), f64::from(b.snap.heading), t),
            velocity: lerp(a.snap.velocity, b.snap.velocity, t),
            motion: b.snap.motion,
            kind: SampleKind::Interpolated,
        })
    }
}

fn state(s: &Snapshot, kind: SampleKind) -> Sample {
    Sample {
        position: s.position,
        heading: reduce(f64::from(s.heading)),
        velocity: s.velocity,
        motion: s.motion,
        kind,
    }
}

fn distance(a: Vec3, b: Vec3) -> f64 {
    let (dx, dy, dz) = (
        f64::from(a.x) - f64::from(b.x),
        f64::from(a.y) - f64::from(b.y),
        f64::from(a.z) - f64::from(b.z),
    );
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn lerp(a: Vec3, b: Vec3, t: f64) -> Vec3 {
    let l = |x: f32, y: f32| (f64::from(x) + (f64::from(y) - f64::from(x)) * t) as f32;
    Vec3::new(l(a.x, b.x), l(a.y, b.y), l(a.z, b.z))
}

/// Reduces an angle to (−π, π].
fn reduce(h: f64) -> f32 {
    let mut r = h.rem_euclid(TAU);
    if r > PI {
        r -= TAU;
    }
    r as f32
}

/// Blends two headings along the shorter arc.
fn lerp_heading(a: f64, b: f64, t: f64) -> f32 {
    let mut d = (b - a).rem_euclid(TAU);
    if d > PI {
        d -= TAU;
    }
    reduce(a + d * t)
}

/// One [`SnapshotBuffer`] per remote entity, with one configuration.
///
/// The set does not bound the number of entities itself: the caller adds an entity when the
/// source creates it and removes it when the source destroys it, so its size follows the
/// source's own entity limit (for the test-server backend, `Hello.max_entities`).
#[derive(Debug, Clone, Default)]
pub struct InterpolationSet {
    config: InterpConfig,
    buffers: BTreeMap<EntityId, SnapshotBuffer>,
}

impl InterpolationSet {
    /// An empty set with `config` (not validated; see [`InterpConfig::validate`]).
    #[must_use]
    pub fn new(config: InterpConfig) -> Self {
        Self {
            config,
            buffers: BTreeMap::new(),
        }
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &InterpConfig {
        &self.config
    }

    /// Replaces the configuration (for example the delay once the server's rates are
    /// known). Buffers are kept.
    pub fn set_config(&mut self, config: InterpConfig) {
        self.config = config;
    }

    /// Starts (or restarts) `id` at `snapshot`: a creation, or a teleport of the entity.
    pub fn reset(&mut self, id: EntityId, snapshot: Snapshot) {
        self.buffers.entry(id).or_default().reset(snapshot);
    }

    /// Adds a snapshot for `id`. Returns false when `id` is unknown or the snapshot is
    /// stale or not finite.
    pub fn push(&mut self, id: EntityId, snapshot: Snapshot) -> bool {
        let cfg = self.config;
        self.buffers
            .get_mut(&id)
            .is_some_and(|b| b.push(snapshot, &cfg))
    }

    /// Forgets `id`.
    pub fn remove(&mut self, id: EntityId) {
        self.buffers.remove(&id);
    }

    /// Forgets every entity.
    pub fn clear(&mut self) {
        self.buffers.clear();
    }

    /// Number of entities tracked.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    /// True when no entity is tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }

    /// The buffer of `id`.
    #[must_use]
    pub fn get(&self, id: EntityId) -> Option<&SnapshotBuffer> {
        self.buffers.get(&id)
    }

    /// The state of `id` at `render_time` (server µs).
    #[must_use]
    pub fn sample(&self, id: EntityId, render_time: u64) -> Option<Sample> {
        self.buffers.get(&id)?.sample(render_time, &self.config)
    }

    /// The tracked entities.
    pub fn ids(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.buffers.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(t_ms: u64, x: f32, vx: f32, heading: f32) -> Snapshot {
        Snapshot {
            server_time: t_ms * 1000,
            position: Vec3::new(x, 0.0, 0.0),
            heading,
            velocity: Vec3::new(vx, 0.0, 0.0),
            motion: Motion::new(Gait::Walking, false),
        }
    }

    #[test]
    fn delay_rule() {
        assert_eq!(recommended_delay(15, 30), Duration::from_millis(167));
        assert_eq!(recommended_delay(30, 30), DEFAULT_DELAY);
        assert_eq!(recommended_delay(0, 0), Duration::from_secs(3));
        assert_eq!(render_time(100, Duration::from_millis(1)), 0);
        assert_eq!(render_time(5_000_000, DEFAULT_DELAY), 4_850_000);
    }

    #[test]
    fn interpolates_and_extrapolates() {
        let cfg = InterpConfig::default();
        let mut b = SnapshotBuffer::new();
        assert!(b.sample(0, &cfg).is_none());
        assert!(b.push(snap(1000, 0.0, 2.0, 0.0), &cfg));
        assert!(b.push(snap(1100, 0.2, 2.0, 0.0), &cfg));
        assert!(!b.push(snap(1100, 9.0, 2.0, 0.0), &cfg), "stale");
        assert!(!b.push(snap(1050, 9.0, 2.0, 0.0), &cfg), "reordered");
        let s = b.sample(1_050_000, &cfg).unwrap();
        assert_eq!(s.kind, SampleKind::Interpolated);
        assert!((s.position.x - 0.1).abs() < 1e-5);
        let e = b.sample(1_200_000, &cfg).unwrap();
        assert_eq!(e.kind, SampleKind::Extrapolated);
        assert!((e.position.x - 0.4).abs() < 1e-5);
        let h = b.sample(5_000_000, &cfg).unwrap();
        assert_eq!(h.kind, SampleKind::Held);
        assert!((h.position.x - (0.2 + 2.0 * 0.25)).abs() < 1e-5);
        let early = b.sample(10, &cfg).unwrap();
        assert_eq!(early.kind, SampleKind::Early);
        assert_eq!(early.position.x, 0.0);
    }

    #[test]
    fn heading_takes_shorter_arc() {
        let cfg = InterpConfig::default();
        let mut b = SnapshotBuffer::new();
        b.push(snap(0, 0.0, 0.0, 3.0), &cfg);
        b.push(snap(100, 0.0, 0.0, -3.0), &cfg);
        let s = b.sample(50_000, &cfg).unwrap();
        // Halfway between 3.0 and -3.0 through π, not through 0.
        assert!(
            (s.heading.abs() - std::f32::consts::PI).abs() < 1e-3,
            "{}",
            s.heading
        );
    }

    #[test]
    fn snaps_across_teleports() {
        let cfg = InterpConfig::default();
        let mut b = SnapshotBuffer::new();
        b.push(snap(0, 0.0, 0.0, 0.0), &cfg);
        // 100 ms later 500 m away: far above 8.5 · 0.1 + 5.
        b.push(snap(100, 500.0, 0.0, 0.0), &cfg);
        let s = b.sample(50_000, &cfg).unwrap();
        assert_eq!(s.kind, SampleKind::Snapped);
        assert_eq!(s.position.x, 0.0);
        let after = b.sample(100_000, &cfg).unwrap();
        assert_eq!(after.position.x, 500.0);
        // A plausible move is blended.
        b.push(snap(200, 500.5, 0.0, 0.0), &cfg);
        assert_eq!(
            b.sample(150_000, &cfg).unwrap().kind,
            SampleKind::Interpolated
        );
    }

    #[test]
    fn history_is_bounded() {
        let cfg = InterpConfig::default();
        let mut b = SnapshotBuffer::new();
        for i in 0..10_000u64 {
            b.push(snap(i * 10, i as f32 * 0.01, 1.0, 0.0), &cfg);
            assert!(b.len() <= MAX_SNAPSHOTS);
        }
        // 1 s of history at 100 Hz is capped at MAX_SNAPSHOTS; with 15 Hz it is ~16.
        let mut c = SnapshotBuffer::new();
        for i in 0..1000u64 {
            c.push(snap(i * 66, i as f32 * 0.1, 1.5, 0.0), &cfg);
        }
        assert!(c.len() <= 17, "{}", c.len());
        let newest = c.newest_time().unwrap();
        // The render time D behind the newest snapshot still has its bracket.
        let s = c.sample(newest - 150_000, &cfg).unwrap();
        assert_eq!(s.kind, SampleKind::Interpolated);
    }

    #[test]
    fn non_finite_refused() {
        let cfg = InterpConfig::default();
        let mut b = SnapshotBuffer::new();
        assert!(!b.push(snap(0, f32::NAN, 0.0, 0.0), &cfg));
        b.reset(snap(0, f32::INFINITY, 0.0, 0.0));
        assert!(b.is_empty());
    }

    #[test]
    fn set_tracks_entities() {
        let mut set = InterpolationSet::new(InterpConfig::default());
        let id = EntityId(7);
        assert!(!set.push(id, snap(0, 0.0, 0.0, 0.0)));
        set.reset(id, snap(0, 0.0, 0.0, 0.0));
        assert!(set.push(id, snap(100, 1.0, 0.0, 0.0)));
        assert!(set.sample(id, 50_000).is_some());
        assert_eq!(set.ids().collect::<Vec<_>>(), vec![id]);
        set.remove(id);
        assert!(set.is_empty());
    }

    #[test]
    fn config_ranges() {
        assert!(InterpConfig::default().validate().is_ok());
        let bad = InterpConfig {
            delay: Duration::from_secs(11),
            ..InterpConfig::default()
        };
        assert!(bad.validate().is_err());
        let bad = InterpConfig {
            snap_speed: f32::NAN,
            ..InterpConfig::default()
        };
        assert!(bad.validate().is_err());
        assert_eq!(InterpConfig::default().snap_speed, 8.5);
        // The documented rule is the checked one: history equal to the delay is enough.
        let edge = InterpConfig {
            delay: Duration::from_millis(500),
            history: Duration::from_millis(500),
            ..InterpConfig::default()
        };
        assert!(edge.validate().is_ok());
        let short = InterpConfig {
            history: Duration::from_millis(499),
            ..edge
        };
        assert!(short.validate().is_err());
    }

    #[test]
    fn server_rates_raise_delay_and_history_together() {
        // Finding: at 1-2 Hz snapshot rates the recommended delay exceeded the 1 s history,
        // so the bracket a render time needed was pruned and every sample was Early.
        for (snap_rate, tick_rate) in [(1u8, 30u8), (2, 30), (15, 30), (30, 30), (0, 0), (1, 1)] {
            let cfg = InterpConfig::default().for_server_rates(snap_rate, tick_rate);
            assert!(cfg.validate().is_ok(), "{snap_rate}/{tick_rate}: {cfg:?}");
            assert_eq!(
                cfg.delay,
                recommended_delay(snap_rate, tick_rate).max(DEFAULT_DELAY)
            );
            assert!(cfg.history >= cfg.delay);
        }
        // Defaults at default rates keep the 1 s history.
        let cfg = InterpConfig::default().for_server_rates(15, 30);
        assert_eq!(cfg.history, DEFAULT_HISTORY);
        // Nothing is lowered.
        let big = InterpConfig {
            delay: Duration::from_secs(5),
            history: Duration::from_secs(8),
            ..InterpConfig::default()
        };
        assert_eq!(big.for_server_rates(15, 30), big);
    }

    fn early_share(cfg: &InterpConfig, rate_hz: u64) -> (usize, usize) {
        let mut b = SnapshotBuffer::new();
        let step = 1_000_000 / rate_hz;
        let mut early = 0;
        let mut total = 0;
        for i in 0..40u64 {
            let t = 1_000_000 + i * step;
            b.push(snap(t / 1000, (t as f32) / 1e6, 1.0, 0.0), cfg);
            // Sample ten times per snapshot interval, the server "now" running from this
            // snapshot to the next.
            for k in 0..10u64 {
                let now = t + k * step / 10;
                if i < 4 {
                    continue; // warm-up
                }
                let s = b.sample(render_time(now, cfg.delay), cfg).unwrap();
                total += 1;
                if s.kind == SampleKind::Early {
                    early += 1;
                }
            }
        }
        (early, total)
    }

    #[test]
    fn low_snapshot_rates_interpolate() {
        for rate in [1u8, 2] {
            let cfg = InterpConfig::default().for_server_rates(rate, 30);
            let (early, total) = early_share(&cfg, u64::from(rate));
            assert_eq!(early, 0, "{rate} Hz: {early}/{total} Early");
        }
        // Even a config that breaks the history rule keeps what the delay needs.
        let broken = InterpConfig {
            delay: recommended_delay(1, 30),
            ..InterpConfig::default()
        };
        assert!(broken.validate().is_err());
        let (early, _) = early_share(&broken, 1);
        assert_eq!(early, 0);
    }
}
