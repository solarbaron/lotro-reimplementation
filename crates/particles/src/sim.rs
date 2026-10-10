// SPDX-License-Identifier: GPL-3.0-or-later
//! One running particle system (`particles.md` §8; `particle-runtime.md` §3, §8, §9.2).
//!
//! [`SystemSim`] runs a decoded 0x39 record: system start and restart (§8.17), the update
//! cadence (§8.1), the activity window (§8.2), timed and distance emission with their caps
//! (§8.3, §8.4, Tessera rules §10.8 and §10.9), spawning (§8.5), keyframe channels (§8.6),
//! the motion controllers (§8.7) and forces (§8.8). Particles of an emitter are stored in
//! creation order and a dying particle is replaced by the last one (`particle-runtime.md`
//! §9.2). Drawing is [`crate::draw`].
//!
//! Every random number comes from the [`Ran2`] the caller passes (`particle-runtime.md`
//! §3.3), so a system run twice with the same seed and the same inputs gives the same
//! particles.
//!
//! Keyframes (§8.6.1, BIN third and fourth passes): no key is current at creation; every
//! particle update, the one right after the spawn included, walks the later keys with the
//! age raised to 0 and makes a key current once that age is at least its `time`; a key
//! applies its controller, then its channels, then its physics flags. A controller key that
//! becomes current while another one is current first runs the **base fix** of the leaving
//! controller (the §8.6.1 table) and raises the rotation's starting value by the new key's
//! `time` × `rotation_velocity` (§8.7). Each particle has one point-index slot, shared by
//! controllers 1 and 3, filled once by the first evaluation that needs it (a particle
//! update under controller 1 or 3, or the base fix of a leaving controller-3 key) and never
//! emptied; controller 3 reduces it modulo the current key's point count (rule §10.12). A
//! channel without a current key uses scale (1, 1), opaque white and mass 1, and a particle
//! without a controller key does not move (rule §10.11).
//!
//! Frames (`particles.md` Q8, Tessera Decision of `particle-runtime.md` §9.2): a
//! world-space system simulates in world coordinates with the first draw instance of the
//! frame as its frame; a local-space system simulates in its own frame (identity at the
//! origin) and is drawn through each instance's transform. The anchors of distance
//! emission (§8.4) follow the first instance's world frame in both cases; for a
//! local-space system the distance-emission offsets are turned back into its own frame.

use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use tessera_formats::gameplay::particles::{
    Emitter, KEY_COLOUR, KEY_COLOUR_LERP, KEY_CONTROLLER, KEY_MASS, KEY_MASS_LERP, KEY_PHYSICS,
    KEY_SCALE, KEY_SCALE_LERP, Keyframe, MAX_EMISSION_CAP, MotionController, PHYSICS_GRAVITY,
    PHYSICS_RANDOM_POINT, PHYSICS_WIND, ParticleSystem, SpawnShape, TOLERANCE,
};

use crate::curve::{EvalCurve, EvalVec, finite};
use crate::rng::Ran2;

/// Gravity in the world frame, metres per second squared (`particles.md` §8.8).
pub const GRAVITY: Vec3 = Vec3::new(0.0, 0.0, -9.8);
/// A particle update longer than this (seconds) resets the interpolation history (§8.1).
pub const INTERPOLATION_RESET: f64 = 0.25;
/// Largest catch-up of controller 4 per update, seconds (§8.7).
pub const STEPPED_CATCH_UP: f64 = 0.25;
/// Velocity damping of controller 4 per step (§8.7).
pub const STEPPED_DAMPING: f32 = 0.95;
/// `fast_forward` above this restarts a system that was not placed for a while (§8.17).
pub const RESTART_FAST_FORWARD: f32 = 1.0;
/// Frames without a placement after which such a system restarts (§8.17).
pub const RESTART_FRAMES: u64 = 100;
/// Length below which a direction counts as zero (§8.5 step 5).
const ZERO_DIRECTION: f32 = 1e-6;

/// Engineering limits of the simulation for hostile records (`particle-runtime.md` §12
/// rule 9; the data never reaches them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimConfig {
    /// Most live particles of one system, all emitters together. Spawning stops at this
    /// count as if every emitter were full.
    pub max_live_particles: usize,
    /// Most controller-4 steps of one particle per update.
    pub max_stepped_steps: u32,
    /// Most legs of a controller-1 walk per particle and update.
    pub max_path_legs: u32,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            max_live_particles: 262_144,
            max_stepped_steps: 64,
            max_path_legs: 1_024,
        }
    }
}

/// One draw instance of a system for a frame (`particles.md` §8.16): the owner's frame,
/// its two scale vectors (§8.6.3, `particle-runtime.md` §6.4) and its opacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// World position of the system frame, metres.
    pub position: Vec3,
    /// Orientation of the system frame.
    pub rotation: Quat,
    /// Vector *A*: the owner's render scale.
    pub scale_a: Vec3,
    /// Vector *B*: (*p*, *p*, *p*) with *p* the owner's particle scale.
    pub scale_b: Vec3,
    /// Instance opacity (before the distance fade).
    pub opacity: f32,
}

impl Default for Placement {
    fn default() -> Self {
        Self::at(Vec3::ZERO)
    }
}

impl Placement {
    /// An unrotated, unscaled, opaque placement at `position`.
    #[must_use]
    pub fn at(position: Vec3) -> Self {
        Self {
            position,
            rotation: Quat::IDENTITY,
            scale_a: Vec3::ONE,
            scale_b: Vec3::ONE,
            opacity: 1.0,
        }
    }

    /// The placement with every value finite: a non-finite position is `None`, a rotation
    /// that cannot be normalised is the identity, non-finite scales are 1 and a non-finite
    /// opacity is 0.
    #[must_use]
    pub fn sanitized(self) -> Option<Self> {
        if !self.position.is_finite() {
            return None;
        }
        let rotation = if self.rotation.is_finite() && self.rotation.length_squared() > 1e-12 {
            self.rotation.normalize()
        } else {
            Quat::IDENTITY
        };
        let fix =
            |v: Vec3| Vec3::from_array(v.to_array().map(|c| if c.is_finite() { c } else { 1.0 }));
        Some(Self {
            position: self.position,
            rotation,
            scale_a: fix(self.scale_a),
            scale_b: fix(self.scale_b),
            opacity: if self.opacity.is_finite() {
                self.opacity
            } else {
                0.0
            },
        })
    }
}

/// What the environment hands every system (`particles.md` §8.8, §8.14).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Environment {
    /// The eased global wind vector, world frame (§8.8; [`crate::wind::Wind`]).
    pub wind: Vec3,
    /// Ambient colour multiplied into lit systems (§8.14; the light list is OPEN, Q8).
    pub ambient: [f32; 4],
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            wind: Vec3::ZERO,
            ambient: [1.0; 4],
        }
    }
}

/// Resolves the attachment points of emitters with a `holding_location` or `bone`
/// (`particles.md` §8.5.5): the point's frame in the owner's model space.
pub trait AttachmentSource {
    /// The frame of `bone` (when not 0, it wins) or `holding_location`, or `None`.
    fn frame(&self, holding_location: u32, bone: u32) -> Option<Mat4>;
}

/// An owner without a skeleton: no attachment resolves.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoAttachments;

impl AttachmentSource for NoAttachments {
    fn frame(&self, _: u32, _: u32) -> Option<Mat4> {
        None
    }
}

/// Every attachment point at the owner's origin (previews and tests).
#[derive(Debug, Clone, Copy, Default)]
pub struct OriginAttachments;

impl AttachmentSource for OriginAttachments {
    fn frame(&self, _: u32, _: u32) -> Option<Mat4> {
        Some(Mat4::IDENTITY)
    }
}

/// The frame a system simulates in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SimFrame {
    pub position: Vec3,
    pub rotation: Quat,
}

impl SimFrame {
    const IDENTITY: Self = Self {
        position: Vec3::ZERO,
        rotation: Quat::IDENTITY,
    };
}

/// One live particle (`particles.md` §8.5–§8.7).
#[derive(Debug, Clone, PartialEq)]
pub struct Particle {
    /// Creation number inside the emitter (0, 1, 2, …), for tests and tools.
    pub serial: u32,
    /// Spawn time on the client clock.
    pub spawn_time: f64,
    /// Lifespan *L*, seconds.
    pub lifespan: f32,
    /// Base position *p*₀ (spawn position, or the position fixed at the last controller
    /// key), simulation frame.
    pub base: Vec3,
    /// Velocity (the spawn velocity; controller 4 integrates it).
    pub velocity: Vec3,
    /// Position after the last particle update.
    pub position: Vec3,
    /// Position before the last particle update (§8.1).
    pub previous: Vec3,
    /// Streak anchor after the last update and before it (§8.12 mode 2).
    pub anchor: Vec3,
    /// Streak anchor before the last update.
    pub previous_anchor: Vec3,
    /// Keys visited so far (the next key to visit is this index).
    pub(crate) visited: u32,
    pub(crate) scale_key: Option<u32>,
    pub(crate) colour_key: Option<u32>,
    pub(crate) mass_key: Option<u32>,
    pub(crate) controller_key: Option<u32>,
    /// The particle's physics flags (§8.8).
    pub physics_flags: u32,
    /// The point-index slot shared by controllers 1 and 3 (`particles.md` §8.6.1 step 4):
    /// `None` until the first evaluation that needs it fills it, then never emptied.
    /// Controller 3: the chosen point; controller 1: the target point.
    pub(crate) point_slot: Option<u32>,
    /// Controller 1: speed (length of the spawn velocity).
    pub(crate) path_speed: f32,
    /// `particle_scale` drawn at spawn.
    pub scale: f32,
    /// `streak` drawn at spawn, seconds.
    pub streak: f32,
    /// `rotation` drawn at spawn, radians, raised at every base fix by the new controller
    /// key's `time` × `rotation_velocity` (§8.7): the value the growth with age starts from.
    pub rotation: [f32; 3],
    /// `world_rotation` drawn at spawn, radians, raised at every base fix like `rotation`.
    pub world_rotation: [f32; 3],
    /// `rotation_velocity` drawn at spawn.
    pub rotation_velocity: [f32; 3],
    /// Random number for the texture atlas (§8.10).
    pub random: f32,
    /// System position at spawn (the centre of the per-axis owner scale, §8.6.3).
    pub spawn_origin: Vec3,
    /// System orientation at spawn (`lock_world_rotation`, §8.12).
    pub spawn_rotation: Quat,
}

/// Per-emitter curves prepared for evaluation.
#[derive(Debug, Clone, Default)]
pub(crate) struct EmitterCurves {
    pub origin: EvalVec,
    pub origin_offset: EvalVec,
    pub pivot: EvalVec,
    pub speed: EvalCurve,
    pub direction: EvalVec,
    pub lifespan: EvalCurve,
    pub shape_size: EvalCurve,
    pub min_spread: EvalCurve,
    pub max_spread: EvalCurve,
    pub particle_scale: EvalCurve,
    pub rotation: EvalVec,
    pub streak: EvalCurve,
    pub world_rotation: EvalVec,
    pub rotation_velocity: EvalVec,
}

impl EmitterCurves {
    fn new(e: &Emitter) -> Self {
        Self {
            origin: EvalVec::new(&e.origin),
            origin_offset: EvalVec::new(&e.origin_offset),
            pivot: EvalVec::new(&e.pivot),
            speed: EvalCurve::new(&e.speed),
            direction: EvalVec::new(&e.direction),
            lifespan: EvalCurve::new(&e.lifespan),
            shape_size: EvalCurve::new(&e.shape_size),
            min_spread: EvalCurve::new(&e.min_spread),
            max_spread: EvalCurve::new(&e.max_spread),
            particle_scale: EvalCurve::new(&e.particle_scale),
            rotation: EvalVec::new(&e.rotation),
            streak: EvalCurve::new(&e.streak),
            world_rotation: EvalVec::new(&e.world_rotation),
            rotation_velocity: EvalVec::new(&e.rotation_velocity),
        }
    }
}

/// Per-emitter values the particle update reads and writes.
#[derive(Debug, Clone, Default)]
pub(crate) struct Control {
    /// Interpolation targets per key: scale, colour, mass (§8.6.1).
    pub targets: Vec<[u32; 3]>,
    /// Controller-3 point counters per key (§8.7).
    pub counters: Vec<u32>,
    /// Controller-5 path curves per key.
    pub paths: Vec<EvalVec>,
    /// Gravity and wind in the system frame, computed at start (§8.8).
    pub gravity: Vec3,
    pub wind: Vec3,
    pub snap: EvalCurve,
    /// Emitter's previous particle-update time before the current one (controller 1).
    pub previous_update: f64,
    /// The system's update interval under rule §10.5 (the controller-4 step).
    pub time_step: f64,
}

/// One emitter's running state.
#[derive(Debug, Clone, Default)]
pub(crate) struct EmitterSim {
    pub curves: EmitterCurves,
    pub control: Control,
    pub particles: Vec<Particle>,
    pub last_update: f64,
    pub last_burst: f64,
    pub last_distance: f64,
    /// Running total of emitted particles (saturating, §10.8).
    pub total: u32,
    pub next_serial: u32,
    pub prev_system_anchor: Vec3,
    pub prev_line_anchor: Vec3,
    /// The attachment frame resolved last (§8.5.5 "earlier result").
    pub attachment: Option<Mat4>,
}

/// Counts of one update or a whole run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SimStats {
    /// Particles created.
    pub spawned: u64,
    /// Spawn attempts that created nothing (dead on arrival, attachment missing, full).
    pub skipped: u64,
    /// Particles that died.
    pub died: u64,
}

/// A running 0x39 particle system (see the module docs).
#[derive(Debug, Clone)]
pub struct SystemSim {
    pub(crate) desc: Arc<ParticleSystem>,
    pub(crate) config: SimConfig,
    pub(crate) created_at: f64,
    pub(crate) start: Option<f64>,
    pub(crate) stopped_at: Option<f64>,
    pub(crate) not_drawn: bool,
    pub(crate) last_placed_frame: Option<u64>,
    pub(crate) instance_frame: Option<u64>,
    pub(crate) instances: Vec<Placement>,
    pub(crate) frame: SimFrame,
    /// The first instance's world frame: the anchors of distance emission (§8.4).
    pub(crate) anchor: SimFrame,
    pub(crate) emitters: Vec<EmitterSim>,
    pub(crate) time_step: f64,
    pub(crate) env: Environment,
    pub(crate) stats: SimStats,
}

impl SystemSim {
    /// A system for `desc`, created at clock time `created_at` (§8.1: an update never runs
    /// earlier than the creation time). Nothing runs before the first placement.
    #[must_use]
    pub fn new(desc: Arc<ParticleSystem>, created_at: f64) -> Self {
        Self::with_config(desc, created_at, SimConfig::default())
    }

    /// [`SystemSim::new`] with explicit limits.
    #[must_use]
    pub fn with_config(desc: Arc<ParticleSystem>, created_at: f64, config: SimConfig) -> Self {
        let emitters = desc
            .emitters
            .iter()
            .map(|e| {
                let keys = &e.keyframes;
                let target = |k: usize, bit: u32| -> u32 {
                    u32::try_from(e.channel_target(k, bit)).unwrap_or(0)
                };
                EmitterSim {
                    curves: EmitterCurves::new(e),
                    control: Control {
                        targets: (0..keys.len())
                            .map(|k| {
                                [
                                    target(k, KEY_SCALE_LERP),
                                    target(k, KEY_COLOUR_LERP),
                                    target(k, KEY_MASS_LERP),
                                ]
                            })
                            .collect(),
                        counters: vec![0; keys.len()],
                        paths: keys.iter().map(|k| EvalVec::new(&k.position)).collect(),
                        snap: EvalCurve::new(&e.snap),
                        ..Control::default()
                    },
                    ..EmitterSim::default()
                }
            })
            .collect();
        let created_at = if created_at.is_finite() {
            created_at
        } else {
            0.0
        };
        Self {
            time_step: f64::from(desc.update_interval()),
            desc,
            config,
            created_at,
            start: None,
            stopped_at: None,
            not_drawn: false,
            last_placed_frame: None,
            instance_frame: None,
            instances: Vec::new(),
            frame: SimFrame::IDENTITY,
            anchor: SimFrame::IDENTITY,
            emitters,
            env: Environment::default(),
            stats: SimStats::default(),
        }
    }

    /// The record.
    #[must_use]
    pub fn desc(&self) -> &Arc<ParticleSystem> {
        &self.desc
    }

    /// The system start (first placement − `fast_forward`), once placed.
    #[must_use]
    pub fn start(&self) -> Option<f64> {
        self.start
    }

    /// The stop time, once stopped (`particle-runtime.md` §7.2).
    #[must_use]
    pub fn stopped_at(&self) -> Option<f64> {
        self.stopped_at
    }

    /// Whether the system is in the not-drawn state of `particles.md` §8.13.
    #[must_use]
    pub fn is_not_drawn(&self) -> bool {
        self.not_drawn
    }

    /// Counts since creation.
    #[must_use]
    pub fn stats(&self) -> SimStats {
        self.stats
    }

    /// Live particles of all emitters.
    #[must_use]
    pub fn live_particles(&self) -> usize {
        self.emitters.iter().map(|e| e.particles.len()).sum()
    }

    /// The live particles of emitter `index` in storage order (`particle-runtime.md` §9.2).
    #[must_use]
    pub fn particles(&self, index: usize) -> &[Particle] {
        self.emitters.get(index).map_or(&[], |e| &e.particles)
    }

    /// The running total of emitter `index` (§8.3).
    #[must_use]
    pub fn emitted(&self, index: usize) -> u32 {
        self.emitters.get(index).map_or(0, |e| e.total)
    }

    /// The emitter's last particle-update time (§8.1).
    #[must_use]
    pub fn last_update(&self, index: usize) -> Option<f64> {
        self.emitters.get(index).map(|e| e.last_update)
    }

    /// The draw instances placed in the current frame.
    #[must_use]
    pub fn instances(&self) -> &[Placement] {
        &self.instances
    }

    /// The frame the instances belong to.
    #[must_use]
    pub fn instance_frame(&self) -> Option<u64> {
        self.instance_frame
    }

    /// Stops emission at `now` (`particle-runtime.md` §7.2 step 3): particles go on
    /// moving, fading and dying. A second stop keeps the first time.
    pub fn stop(&mut self, now: f64) {
        if self.stopped_at.is_none() {
            self.stopped_at = Some(if now.is_finite() {
                now
            } else {
                self.created_at
            });
        }
    }

    /// The end test (`particles.md` §8.2) at `now`; `false` before the first placement.
    #[must_use]
    pub fn has_ended(&self, now: f64) -> bool {
        self.start.is_some_and(|s| self.desc.has_ended(now - s))
    }

    /// Whether the system has finished (`particle-runtime.md` §7.4): stopped more than 6 s
    /// ago, or its end test passes.
    #[must_use]
    pub fn has_finished(&self, now: f64) -> bool {
        self.stopped_at
            .is_some_and(|t| now - t > crate::runtime::LINGER_SECONDS)
            || self.has_ended(now)
    }

    /// Adds a draw instance for frame `frame` at clock `now` (`particles.md` §8.16). The
    /// first instance of a frame starts the system on its first placement (§8.17), restarts
    /// it after a long absence (§8.17), and runs the system update (§8.1) unless the system
    /// is in the not-drawn state. Instances with a non-finite position are ignored.
    pub fn place(
        &mut self,
        frame: u64,
        now: f64,
        placement: Placement,
        env: &Environment,
        rng: &mut Ran2,
        attach: &dyn AttachmentSource,
    ) {
        let Some(placement) = placement.sanitized() else {
            tracing::debug!("particle placement with a non-finite position ignored");
            return;
        };
        if !now.is_finite() {
            return;
        }
        if self.instance_frame == Some(frame) {
            self.instances.push(placement);
            return;
        }
        self.instance_frame = Some(frame);
        self.instances.clear();
        self.instances.push(placement);
        self.env = sanitized_env(env);
        let restart = self.desc.fast_forward > RESTART_FAST_FORWARD
            && self
                .last_placed_frame
                .is_some_and(|f| frame.saturating_sub(f) > RESTART_FRAMES);
        self.last_placed_frame = Some(frame);
        self.frame = self.sim_frame(&placement);
        self.anchor = SimFrame {
            position: placement.position,
            rotation: placement.rotation,
        };
        if self.start.is_none() || restart {
            self.begin(now, rng);
        }
        for (e, s) in self.desc.emitters.iter().zip(&mut self.emitters) {
            s.attachment = resolve_attachment(e, attach).or(s.attachment);
        }
        if !self.not_drawn {
            self.update(now, rng);
        }
    }

    /// The frame the system simulates in for a placement.
    pub(crate) fn sim_frame(&self, p: &Placement) -> SimFrame {
        if self.desc.world_space {
            SimFrame {
                position: p.position,
                rotation: p.rotation,
            }
        } else {
            SimFrame::IDENTITY
        }
    }

    /// Starts (or restarts) the system at `now` (§8.17): start = now − `fast_forward`,
    /// every emitter's update, burst and distance times at start + `start_time`, the
    /// forces computed (§8.8), live particles and totals cleared.
    fn begin(&mut self, now: f64, rng: &mut Ran2) {
        let now = now.max(self.created_at);
        let ff = f64::from(finite(self.desc.fast_forward));
        let start = now - ff;
        self.start = Some(start);
        let inv = self.frame.rotation.inverse();
        let gravity = inv * GRAVITY;
        let wind = if self.env.wind.length() <= TOLERANCE {
            self.env.wind
        } else {
            inv * self.env.wind
        };
        let anchor = self.anchor;
        let time_step = self.time_step;
        for (e, s) in self.desc.emitters.iter().zip(&mut self.emitters) {
            let t0 = start + f64::from(finite(e.start_time));
            s.last_update = t0;
            s.last_burst = t0;
            s.last_distance = t0;
            s.control.previous_update = t0;
            s.total = 0;
            s.particles.clear();
            s.control.counters.iter_mut().for_each(|c| *c = 0);
            s.control.gravity = gravity;
            s.control.wind = wind;
            s.control.time_step = time_step;
            let line = line_vector(e, &mut s.curves, now, rng);
            s.prev_system_anchor = anchor.position;
            s.prev_line_anchor = anchor.position + anchor.rotation * line;
        }
    }

    /// Recomputes every emitter's gravity and wind from `wind` (the eased global wind) in
    /// the system's current frame (§8.8 "Recomputation"; only the weather code calls it).
    pub fn recompute_forces(&mut self, wind: Vec3) {
        let inv = self.frame.rotation.inverse();
        let wind = if !wind.is_finite() {
            Vec3::ZERO
        } else if wind.length() <= TOLERANCE {
            wind
        } else {
            inv * wind
        };
        for s in &mut self.emitters {
            s.control.gravity = inv * GRAVITY;
            s.control.wind = wind;
        }
    }

    /// The system update (§8.1): emission then the particle update, emitter by emitter.
    pub(crate) fn update(&mut self, now: f64, rng: &mut Ran2) {
        let Some(start) = self.start else {
            return;
        };
        let now = now.max(self.created_at);
        let pd = self.desc.physics_duration;
        if pd != 0.0 && !(now - start <= f64::from(pd)) {
            return;
        }
        let desc = Arc::clone(&self.desc);
        let frame = self.frame;
        let anchor = self.anchor;
        let emitting = self.stopped_at.is_none();
        let budget = self.config.max_live_particles;
        let mut live_other: usize = self.live_particles();
        for (e, s) in desc.emitters.iter().zip(&mut self.emitters) {
            live_other -= s.particles.len();
            let et = now - (start + f64::from(finite(e.start_time)));
            let line = line_vector(e, &mut s.curves, now, rng);
            let line_anchor = anchor.position + anchor.rotation * line;
            let room_left = budget.saturating_sub(live_other);
            let mut ctx = SpawnCtx {
                desc: &desc,
                e,
                now,
                start,
                frame,
                anchor,
                room_left,
                config: self.config,
                stats: &mut self.stats,
            };
            if emitting && e.in_emission_window(et) {
                timed_emission(&mut ctx, s, rng);
                distance_emission(&mut ctx, s, line, line_anchor, rng);
            } else {
                s.prev_system_anchor = anchor.position;
                s.prev_line_anchor = line_anchor;
            }
            particle_update(&mut ctx, s, rng);
            live_other += s.particles.len();
        }
    }
}

fn sanitized_env(env: &Environment) -> Environment {
    Environment {
        wind: if env.wind.is_finite() {
            env.wind
        } else {
            Vec3::ZERO
        },
        ambient: env.ambient.map(|c| if c.is_finite() { c } else { 1.0 }),
    }
}

/// Whether the emitter has an attachment point (§8.5.5).
pub(crate) fn has_attachment(e: &Emitter) -> bool {
    e.bone != 0 || e.holding_location != 0
}

fn resolve_attachment(e: &Emitter, attach: &dyn AttachmentSource) -> Option<Mat4> {
    if !has_attachment(e) {
        return None;
    }
    attach
        .frame(e.holding_location, e.bone)
        .filter(|m| m.is_finite())
}

/// The line vector of a `Line` emitter (§8.5.1, §8.4): normalise(`direction`) ×
/// `shape_size`, both at the absolute clock; zero for other shapes.
fn line_vector(e: &Emitter, c: &mut EmitterCurves, now: f64, rng: &mut Ran2) -> Vec3 {
    if e.spawn_shape() != Some(SpawnShape::Line) {
        return Vec3::ZERO;
    }
    let d = Vec3::from_array(c.direction.eval(now, rng)).normalize_or_zero();
    let z = c.shape_size.eval(now, rng);
    d * z
}

/// What spawning and the particle update need besides the emitter state.
pub(crate) struct SpawnCtx<'a> {
    desc: &'a ParticleSystem,
    e: &'a Emitter,
    now: f64,
    start: f64,
    frame: SimFrame,
    /// The world frame of the distance-emission anchors (§8.4): the first instance's.
    anchor: SimFrame,
    /// Particles this emitter may still hold under the system budget.
    room_left: usize,
    config: SimConfig,
    stats: &'a mut SimStats,
}

impl SpawnCtx<'_> {
    /// The emitter's live-particle limit: `max_particles` clamped to §10.8's cap and to the
    /// system budget.
    fn max_live(&self) -> usize {
        let cap = self.e.max_particles.min(MAX_EMISSION_CAP) as usize;
        cap.min(self.room_left)
    }

    /// Cap of §8.3 step 1 (clamped, §10.8) and the system budget.
    fn cap(&self) -> usize {
        (self.e.emission_cap() as usize).min(self.room_left)
    }
}

/// Timed emission (`particles.md` §8.3 with rule §10.8).
fn timed_emission(ctx: &mut SpawnCtx<'_>, s: &mut EmitterSim, rng: &mut Ran2) {
    let e = ctx.e;
    let Some(interval) = e.timed_interval() else {
        return;
    };
    let interval = f64::from(interval);
    let now = ctx.now;
    if !(s.last_burst <= now) {
        return;
    }
    // Step 1.
    let mut room = ctx.cap().saturating_sub(s.particles.len());
    // Step 2: catch-up, with the lifespan at the last-burst time on the client clock.
    let l = f64::from(s.curves.lifespan.eval(s.last_burst, rng));
    if now - s.last_burst > l {
        let skipped = (now - l) - s.last_burst;
        s.last_burst = now - l;
        let bursts = (skipped / interval).trunc();
        let add = bursts * f64::from(e.burst_count);
        if add.is_finite() && add > 0.0 {
            s.total = s.total.saturating_add(add.min(f64::from(u32::MAX)) as u32);
        }
    }
    // Step 3.
    if e.emission_limit != 0 && s.total > e.emission_limit {
        return;
    }
    // Step 4.
    while s.last_burst < now {
        if room == 0 {
            s.last_burst = now;
            break;
        }
        let mut burst = e.burst_count;
        if e.emission_limit != 0 {
            burst = burst.min(e.emission_limit.saturating_sub(s.total));
        }
        if burst < 1 {
            s.last_burst = now;
            break;
        }
        let n = (burst as usize).min(room);
        let t = s.last_burst;
        for _ in 0..n {
            spawn(ctx, s, t, Vec3::ZERO, None, rng);
        }
        s.total = s.total.saturating_add(u32::try_from(n).unwrap_or(u32::MAX));
        room -= n;
        let next = s.last_burst + interval;
        if next == s.last_burst {
            // §10.8: the interval no longer moves the time.
            s.last_burst = now;
            break;
        }
        s.last_burst = next;
    }
}

/// Distance emission (`particles.md` §8.4 with rule §10.9).
fn distance_emission(
    ctx: &mut SpawnCtx<'_>,
    s: &mut EmitterSim,
    line: Vec3,
    line_anchor: Vec3,
    rng: &mut Ran2,
) {
    let e = ctx.e;
    let dist = e.emission_distance;
    if !(dist.is_finite() && dist > 0.0) {
        return;
    }
    let now = ctx.now;
    let system_anchor = ctx.anchor.position;
    // Step 1.
    let tp = s.last_distance;
    s.last_distance = now;
    // Step 2.
    if !(e.emission_limit == 0 || s.total <= e.emission_limit) {
        return;
    }
    // Step 3.
    let d1 = (system_anchor - s.prev_system_anchor).length();
    let d2 = (line_anchor - s.prev_line_anchor).length();
    if !(d1 >= TOLERANCE || d2 >= TOLERANCE) {
        return;
    }
    // Step 4.
    let steps = (d1.max(d2) / dist).trunc();
    if !(steps >= 1.0) {
        return;
    }
    let step1 = (system_anchor - s.prev_system_anchor) / steps;
    let step2 = (line_anchor - s.prev_line_anchor) / steps;
    // Step 5.
    let mut room = ctx.cap().saturating_sub(s.particles.len());
    // World offsets into the simulation frame: unchanged for a world-space system (its
    // frame is the anchor frame), turned back by the anchor's rotation for a local-space
    // one (`particle-runtime.md` §9.2, Tessera Decision until `particles.md` Q8 (a)).
    let anchor_inv = ctx.anchor.rotation.inverse();
    let frame_rot = ctx.frame.rotation;
    let to_sim = move |v: Vec3| frame_rot * (anchor_inv * v);
    let n = f64::from(steps);
    let mut k = 1.0f64;
    let _ = line;
    while room > 0 {
        let t = (tp + k * (tp - now) / n).min(now);
        if (system_anchor - s.prev_system_anchor).length() < dist
            && (line_anchor - s.prev_line_anchor).length() < dist
        {
            break;
        }
        let mut burst = e.burst_count;
        if e.emission_limit != 0 {
            burst = burst.min(e.emission_limit.saturating_sub(s.total));
        }
        s.prev_system_anchor += step1;
        s.prev_line_anchor += step2;
        if burst < 1 {
            s.last_distance = now;
            break;
        }
        let count = (burst as usize).min(room);
        let offset = to_sim(s.prev_system_anchor - system_anchor);
        let line_override = (e.spawn_shape() == Some(SpawnShape::Line))
            .then(|| anchor_inv * (s.prev_line_anchor - s.prev_system_anchor));
        for _ in 0..count {
            spawn(ctx, s, t, offset, line_override, rng);
        }
        s.total = s
            .total
            .saturating_add(u32::try_from(count).unwrap_or(u32::MAX));
        room -= count;
        k += 1.0;
        // At most n + 1 steps (§8.4); guards rounding of the anchor steps.
        if k > n + 1.0 {
            break;
        }
    }
}

/// The shape point *s* (`particles.md` §8.5.1), emitter frame.
fn shape_point(e: &Emitter, c: &mut EmitterCurves, et: f64, line: Vec3, rng: &mut Ran2) -> Vec3 {
    let z = c.shape_size.eval(et, rng);
    let half = z / 2.0;
    match e.spawn_shape() {
        Some(SpawnShape::Line) => {
            if line.length_squared() >= TOLERANCE {
                line.normalize_or_zero() * rng.range(0.0, z)
            } else {
                Vec3::ZERO
            }
        }
        Some(SpawnShape::Disc) => {
            let mut u =
                Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), 0.0).normalize_or_zero();
            if e.inclusive {
                u *= rng.range(0.0, 1.0);
            }
            u * half
        }
        Some(SpawnShape::Sphere) => {
            let mut u = Vec3::new(
                rng.range(-1.0, 1.0),
                rng.range(-1.0, 1.0),
                rng.range(-1.0, 1.0),
            )
            .normalize_or_zero();
            if e.inclusive {
                u *= rng.range(0.0, 1.0);
            }
            u * half
        }
        Some(SpawnShape::Quad) => {
            let mut s = Vec3::new(rng.range(-half, half), rng.range(-half, half), 0.0);
            if !e.inclusive {
                match rng.index(4) {
                    0 => s.x = -half,
                    1 => s.y = -half,
                    2 => s.x = half,
                    _ => s.y = half,
                }
            }
            s
        }
        Some(SpawnShape::Point) | None => Vec3::ZERO,
    }
}

/// Tilts `d` (unit) by a spread angle between `min_spread` and `max_spread` degrees, then
/// turns it about the original `d` by a random angle (§8.5.3).
fn spread(d: Vec3, a0: f32, a1: f32, rng: &mut Ran2) -> Vec3 {
    if !(a1 - a0 > TOLERANCE) {
        return d;
    }
    let a = rng.range(a0, a1);
    if a == 0.0 || !a.is_finite() {
        return d;
    }
    let perp = |axis: Vec3| (axis - d * axis.dot(d)).normalize_or_zero();
    let mut u = perp(Vec3::Z);
    if u == Vec3::ZERO {
        u = perp(Vec3::Y);
    }
    let axis = d.cross(u).normalize_or_zero();
    if axis == Vec3::ZERO {
        return d;
    }
    let tilted = Quat::from_axis_angle(axis, a.to_radians()) * d;
    let turn = rng.range(0.0, std::f32::consts::TAU);
    Quat::from_axis_angle(d, turn) * tilted
}

/// Spawns one particle at `t` (`particles.md` §8.5), offset by `offset` (distance
/// emission), with `line_override` replacing the line vector of a `Line` emitter.
fn spawn(
    ctx: &mut SpawnCtx<'_>,
    s: &mut EmitterSim,
    t: f64,
    offset: Vec3,
    line_override: Option<Vec3>,
    rng: &mut Ran2,
) {
    let e = ctx.e;
    let et = t - (ctx.start + f64::from(finite(e.start_time)));
    // Step 1.
    let l = s.curves.lifespan.eval(et, rng);
    if !(t + f64::from(l) > ctx.now) {
        ctx.stats.skipped += 1;
        return;
    }
    // Step 2.
    let line = match line_override {
        Some(v) => v,
        None => line_vector(e, &mut s.curves, t, rng),
    };
    let mut point = shape_point(e, &mut s.curves, et, line, rng);
    // Step 3.
    if has_attachment(e) && !e.stick_to_bones {
        if !ctx.desc.world_space {
            ctx.stats.skipped += 1;
            return;
        }
        match s.attachment {
            Some(m) => point = m.transform_point3(point),
            None => {
                ctx.stats.skipped += 1;
                return;
            }
        }
    }
    // Step 4.
    if s.particles.len() >= ctx.max_live() {
        ctx.stats.skipped += 1;
        return;
    }
    // Step 5. The radial direction of an exploding emitter uses the shape point after the
    // attachment of step 3 and before `origin` (§8.5.2, BIN third pass).
    let rot = ctx.frame.rotation;
    let exploding_radial = e.exploding && point.to_array().iter().any(|c| c.abs() > TOLERANCE);
    let mut d = if !e.exploding {
        rot * Vec3::from_array(s.curves.direction.eval(et, rng))
    } else if exploding_radial {
        rot * point
    } else {
        Vec3::new(
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
            rng.range(-1.0, 1.0),
        )
    };
    d = if d.is_finite() && d.length() > ZERO_DIRECTION {
        d.normalize()
    } else {
        Vec3::Y
    };
    // Step 6.
    point += Vec3::from_array(s.curves.origin.eval(et, rng));
    // Step 7.
    let a0 = s.curves.min_spread.eval(et, rng);
    let a1 = s.curves.max_spread.eval(et, rng);
    d = spread(d, a0, a1, rng);
    // Step 8.
    let position = ctx.frame.position + rot * point + offset;
    let speed = s.curves.speed.eval(et, rng);
    let velocity = d * speed;
    // Step 9.
    let scale = s.curves.particle_scale.eval(et, rng);
    let streak = s.curves.streak.eval(et, rng);
    let rotation = s.curves.rotation.eval(et, rng);
    let world_rotation = s.curves.world_rotation.eval(et, rng);
    let rotation_velocity = s.curves.rotation_velocity.eval(et, rng);
    let random = if e.random_start_frame {
        rng.range(0.0, 1.0)
    } else {
        0.0
    };
    if !(position.is_finite() && velocity.is_finite()) {
        ctx.stats.skipped += 1;
        return;
    }
    let serial = s.next_serial;
    s.next_serial = s.next_serial.wrapping_add(1);
    let mut p = Particle {
        serial,
        spawn_time: t,
        lifespan: l,
        base: position,
        velocity,
        position,
        previous: position,
        anchor: position,
        previous_anchor: position,
        visited: 0,
        scale_key: None,
        colour_key: None,
        mass_key: None,
        controller_key: None,
        physics_flags: 0,
        point_slot: None,
        path_speed: velocity.length(),
        scale,
        streak,
        rotation,
        world_rotation,
        rotation_velocity,
        random,
        spawn_origin: ctx.frame.position,
        spawn_rotation: rot,
    };
    // No key is current at creation (§8.6.1 step 1); the update right after the spawn
    // walks the keys (step 2), so a first key with `time` 0 is current from it on, also at
    // a negative age (`particle-runtime.md` §14.3).
    let at = s.last_update;
    if advance(
        &mut p,
        e,
        &mut s.control,
        ctx.frame,
        at,
        0.0,
        ctx.config,
        rng,
    ) {
        p.previous = p.position;
        p.previous_anchor = p.anchor;
        s.particles.push(p);
        ctx.stats.spawned += 1;
    } else {
        ctx.stats.skipped += 1;
    }
}

/// The particle update of one emitter (§8.1): when at least `time_step` has passed since
/// its last particle update, every particle advances to now; dead particles are replaced
/// by the last one (`particle-runtime.md` §9.2).
fn particle_update(ctx: &mut SpawnCtx<'_>, s: &mut EmitterSim, rng: &mut Ran2) {
    let now = ctx.now;
    let step = now - s.last_update;
    if !(step >= s.control.time_step) {
        return;
    }
    s.control.previous_update = s.last_update;
    let e = ctx.e;
    let reset = step > INTERPOLATION_RESET;
    let mut i = 0;
    while let Some(p) = s.particles.get_mut(i) {
        let before = (p.position, p.anchor);
        if advance(p, e, &mut s.control, ctx.frame, now, step, ctx.config, rng) {
            if reset {
                p.previous = p.position;
                p.previous_anchor = p.anchor;
            } else {
                p.previous = before.0;
                p.previous_anchor = before.1;
            }
            i += 1;
        } else {
            s.particles.swap_remove(i);
            ctx.stats.died += 1;
        }
    }
    s.last_update = now;
}

/// Makes key `k` current for what its flags name (§8.6.1 step 3): the controller switch
/// (0x100) first, then the scale, colour and mass bits, then the physics flags (0x200).
/// The point index of controllers 1 and 3 is not taken here (step 4: the shared slot is
/// filled by the first evaluation that needs it); the base fix runs before this call.
fn visit_key(p: &mut Particle, e: &Emitter, k: usize) {
    let Some(key) = e.keyframes.get(k) else {
        return;
    };
    let k32 = u32::try_from(k).unwrap_or(u32::MAX);
    if key.flags & KEY_CONTROLLER != 0 {
        p.controller_key = Some(k32);
    }
    if key.flags & (KEY_SCALE | KEY_SCALE_LERP) != 0 {
        p.scale_key = Some(k32);
    }
    if key.flags & (KEY_COLOUR | KEY_COLOUR_LERP) != 0 {
        p.colour_key = Some(k32);
    }
    if key.flags & (KEY_MASS | KEY_MASS_LERP) != 0 {
        p.mass_key = Some(k32);
    }
    if key.flags & KEY_PHYSICS != 0 {
        p.physics_flags = key.physics_flags;
    }
}

/// Fills the empty point-index slot at a particle update under controller 3 (§8.7): the
/// particle takes key `k`'s counter, which then advances by one (wrapping) or, with
/// physics flag 0x8, is replaced by a uniform index. Because it runs at the update, it sees
/// the physics flags of the key that set the controller (§8.6.1 step 4). A key without
/// points leaves the slot empty.
fn fill_slot_by_update(
    p: &mut Particle,
    key: &Keyframe,
    c: &mut Control,
    k: usize,
    rng: &mut Ran2,
) {
    let count = key.points.len();
    if count == 0 || p.point_slot.is_some() {
        return;
    }
    if let Some(counter) = c.counters.get_mut(k) {
        p.point_slot = Some(*counter);
        *counter = if p.physics_flags & PHYSICS_RANDOM_POINT != 0 {
            u32::try_from(rng.index(count)).unwrap_or(0)
        } else {
            let next = (*counter as usize + 1) % count;
            u32::try_from(next).unwrap_or(0)
        };
    }
}

/// The point of `points` at the particle's slot, rotated by `rot`; the slot is reduced
/// modulo the point count (rule §10.12; the slot itself keeps its value). `None` with an
/// empty slot or no points.
fn slot_point(slot: Option<u32>, points: &[[f32; 3]], rot: Quat) -> Option<Vec3> {
    let n = points.len();
    if n == 0 {
        return None;
    }
    let i = slot? as usize % n;
    points.get(i).map(|&q| rot * Vec3::from_array(q))
}

/// The point-path walk of controller 1 (§8.7) with travel budget `b`: towards the point at
/// the slot (reduced modulo the count), landing on points and moving the slot on. The
/// budget is compared with, and reduced by, squared distances, as in the original.
fn walk_path(p: &mut Particle, points: &[[f32; 3]], rot: Quat, mut b: f32, max_legs: u32) {
    let n = points.len();
    if n == 0 {
        return;
    }
    let mut legs = 0;
    while b > TOLERANCE && legs < max_legs {
        legs += 1;
        let idx = p.point_slot.unwrap_or(0) as usize % n;
        let target = p.base
            + points
                .get(idx)
                .map_or(Vec3::ZERO, |&q| rot * Vec3::from_array(q));
        let q2 = (target - p.position).length_squared();
        if b <= q2 {
            let dir = (target - p.position).normalize_or_zero();
            p.position += dir * b.sqrt();
            b = 0.0;
        } else {
            p.position = target;
            b -= q2;
            p.point_slot = Some(u32::try_from((idx + 1) % n).unwrap_or(0));
        }
    }
}

/// The base fix (§8.6.1, BIN fourth pass): key `new` sets a controller while another
/// controller key is current, so the base *p*₀ first moves by the motion of the
/// **leaving** key over *τ* = `new.time` − its `time`:
///
/// - 2: *p*₀ + *v* *τ* + ½ *a* *τ*², *a* scaled by the stored `mass` of the current mass
///   key (not interpolated; 1 without one, rule §10.11);
/// - 5: *p*₀ + `position`(*τ*);
/// - 3: an empty slot takes the leaving key's counter, which advances by one (wrapping),
///   whatever physics flag 0x8 says; then *p*₀ + that point (rule §10.12); no points: *p*₀;
/// - 1: the point-path walk run once to the new key's time, the new base the result (see
///   the implementer OPEN of `particles.md` §8.6.1 for the reading used);
/// - 6, 4 and any other value: *p*₀.
///
/// Then the starting value of the rotation growth rises by `new.time` × the particle's
/// `rotation_velocity` (§8.7), for `rotation` and `world_rotation` alike (only one of the
/// two is drawn, §8.12 item 3).
#[allow(clippy::too_many_arguments)]
fn base_fix(
    p: &mut Particle,
    e: &Emitter,
    c: &mut Control,
    rot: Quat,
    new: &Keyframe,
    config: SimConfig,
    rng: &mut Ran2,
) {
    let Some(k) = p.controller_key else {
        return;
    };
    let Some(old) = e.keyframes.get(k as usize) else {
        return;
    };
    let tau = new.time - old.time;
    match old.motion() {
        MotionController::Ballistic => {
            let mass = p
                .mass_key
                .and_then(|i| e.keyframes.get(i as usize))
                .map_or(1.0, |kf| finite(kf.mass));
            let a = force(p, c, mass);
            p.base += p.velocity * tau + 0.5 * a * tau * tau;
        }
        MotionController::PathCurves => {
            let off = c
                .paths
                .get_mut(k as usize)
                .map_or([0.0; 3], |v| v.eval(f64::from(tau), rng));
            p.base += Vec3::from_array(off);
        }
        MotionController::PointPlacement => {
            let n = old.points.len();
            if n > 0 {
                if p.point_slot.is_none()
                    && let Some(counter) = c.counters.get_mut(k as usize)
                {
                    p.point_slot = Some(*counter);
                    *counter = u32::try_from((*counter as usize + 1) % n).unwrap_or(0);
                }
                if let Some(pt) = slot_point(p.point_slot, &old.points, rot) {
                    p.base += pt;
                }
            }
        }
        MotionController::PointPath => {
            // Tessera reading (implementer OPEN, `particles.md` §8.6.1): with a filled slot
            // the walk runs from the current position with the budget *s* × (clock time of
            // the new key − the emitter's previous particle-update time); an empty slot
            // (controller 1 never ran) leaves the base and the slot as they are.
            if p.point_slot.is_some() && !old.points.is_empty() {
                let span = (p.spawn_time + f64::from(new.time)) - c.previous_update;
                let b = p.path_speed * span as f32;
                let b = if b.is_finite() { b.max(0.0) } else { 0.0 };
                walk_path(p, &old.points, rot, b, config.max_path_legs);
                p.base = p.position;
            }
        }
        MotionController::Static | MotionController::Stepped => {}
    }
    if !p.base.is_finite() {
        p.base = p.position;
    }
    let rv = p.rotation_velocity;
    for r in [&mut p.rotation, &mut p.world_rotation] {
        for (a, v) in r.iter_mut().zip(rv) {
            let x = *a + new.time * v;
            if x.is_finite() {
                *a = x;
            }
        }
    }
}

/// The value of a keyframe channel for a particle whose current key is `key`, at `age`
/// (§8.6.1): interpolated towards the key's target, or the key's own value. `None` without
/// a current key.
pub(crate) fn channel<T>(
    keys: &[Keyframe],
    targets: &[[u32; 3]],
    key: Option<u32>,
    channel: usize,
    age: f32,
    value: impl Fn(&Keyframe) -> T,
    lerp: impl Fn(&T, &T, f32) -> T,
) -> Option<T> {
    let k = key? as usize;
    let kf = keys.get(k)?;
    let ti = targets.get(k)?.get(channel).copied()? as usize;
    let tf = keys.get(ti)?;
    if ti == k {
        return Some(value(kf));
    }
    let mass = channel == 2;
    let dt = tf.time - kf.time;
    let small = if mass {
        dt.abs() <= TOLERANCE
    } else {
        dt <= TOLERANCE
    };
    let f = if small {
        if mass { 0.0 } else { 1.0 }
    } else {
        ((age - kf.time) / dt).clamp(0.0, 1.0)
    };
    let f = if f.is_finite() { f } else { 0.0 };
    Some(lerp(&value(kf), &value(tf), f))
}

pub(crate) fn lerp1(a: &f32, b: &f32, f: f32) -> f32 {
    (1.0 - f) * a + f * b
}

/// The mass channel (1 without a mass key, §8.6.1).
fn mass_of(p: &Particle, e: &Emitter, c: &Control, age: f32) -> f32 {
    finite(
        channel(
            &e.keyframes,
            &c.targets,
            p.mass_key,
            2,
            age,
            |k| k.mass,
            lerp1,
        )
        .unwrap_or(1.0),
    )
}

/// The force *a* of §8.8 for the particle's physics flags and mass.
fn force(p: &Particle, c: &Control, mass: f32) -> Vec3 {
    let mut a = Vec3::ZERO;
    if p.physics_flags & PHYSICS_GRAVITY != 0 {
        a += c.gravity * mass;
    }
    if p.physics_flags & PHYSICS_WIND != 0 {
        a += c.wind * mass;
    }
    a
}

/// The position on the current controller path at `tau` for the closed-form controllers
/// (2, 3, 5, 6 and unknown values); `None` for controllers 1 and 4.
fn path_position(
    p: &Particle,
    key: Option<&Keyframe>,
    c: &mut Control,
    rot: Quat,
    a: Vec3,
    tau: f32,
    rng: &mut Ran2,
) -> Option<Vec3> {
    let Some(key) = key else {
        return Some(p.base);
    };
    match key.motion() {
        MotionController::Ballistic => Some(p.base + p.velocity * tau + 0.5 * a * tau * tau),
        MotionController::PathCurves => {
            let k = p.controller_key.unwrap_or(0) as usize;
            let off = c
                .paths
                .get_mut(k)
                .map_or([0.0; 3], |v| v.eval(f64::from(tau), rng));
            Some(p.base + Vec3::from_array(off))
        }
        MotionController::PointPlacement => {
            Some(p.base + slot_point(p.point_slot, &key.points, rot).unwrap_or(Vec3::ZERO))
        }
        MotionController::Static => Some(p.base),
        MotionController::PointPath | MotionController::Stepped => None,
    }
}

/// Advances `p` to clock time `at` (a particle update or the spawn update, §8.5 step 9).
/// `step` is the time since the emitter's previous particle update (0 at spawn). Returns
/// `false` when the particle is dead.
#[allow(clippy::too_many_arguments)]
fn advance(
    p: &mut Particle,
    e: &Emitter,
    c: &mut Control,
    frame: SimFrame,
    at: f64,
    step: f64,
    config: SimConfig,
    rng: &mut Ran2,
) -> bool {
    let age64 = at - p.spawn_time;
    if !(age64 <= f64::from(p.lifespan)) {
        return false;
    }
    let age = age64 as f32;
    // §8.6.1 step 2: walk the keys after the current one with the age raised to 0; a key
    // is current once that age is at least its `time`.
    let walk_age = age.max(0.0);
    while let Some(key) = e.keyframes.get(p.visited as usize) {
        if !(key.time <= walk_age) {
            break;
        }
        let k = p.visited as usize;
        if key.flags & KEY_CONTROLLER != 0 && p.controller_key.is_some() {
            base_fix(p, e, c, frame.rotation, key, config, rng);
        }
        visit_key(p, e, k);
        p.visited = p.visited.saturating_add(1);
    }
    let key = p.controller_key.and_then(|i| e.keyframes.get(i as usize));
    if let (Some(k), Some(kf)) = (p.controller_key, key)
        && kf.motion() == MotionController::PointPlacement
    {
        fill_slot_by_update(p, kf, c, k as usize, rng);
    }
    let tau = age - key.map_or(0.0, |k| k.time);
    let mass = mass_of(p, e, c, age);
    let a = force(p, c, mass);
    let rot = frame.rotation;
    match key.map(Keyframe::motion) {
        Some(MotionController::PointPath) => {
            let points = key.map_or(&[][..], |k| &k.points[..]);
            if points.is_empty() {
                p.position = p.base;
            } else if p.point_slot.is_none() {
                // The first evaluation fills the shared slot with the start point (§8.7).
                let idx = if p.physics_flags & PHYSICS_RANDOM_POINT != 0 {
                    rng.index(points.len())
                } else {
                    0
                };
                p.point_slot = Some(u32::try_from(idx).unwrap_or(0));
                p.position = p.base + slot_point(p.point_slot, points, rot).unwrap_or(Vec3::ZERO);
            } else {
                // A filled slot (this key's or an earlier controller-1 or controller-3
                // key's): walk on from it.
                let b = p.path_speed * (at - c.previous_update) as f32;
                walk_path(p, points, rot, b, config.max_path_legs);
            }
        }
        Some(MotionController::Stepped) => {
            let dt = step.min(STEPPED_CATCH_UP);
            let snap = c.snap.eval(f64::from(age), rng);
            let ts = c.time_step;
            let mut remaining = dt;
            let mut n = 0;
            while remaining >= ts && n < config.max_stepped_steps {
                n += 1;
                let h = ts as f32;
                let f = a;
                p.position += h * (p.velocity + f) + (p.base - p.position) * snap * h;
                p.velocity = (p.velocity + f) * STEPPED_DAMPING;
                remaining -= ts;
            }
        }
        _ => {
            if let Some(pos) = path_position(p, key, c, rot, a, tau, rng) {
                p.position = pos;
            }
        }
    }
    // Streak anchor (§8.12 mode 2).
    match key.map(Keyframe::motion) {
        Some(MotionController::Ballistic | MotionController::PathCurves) => {
            let ts = (tau - p.streak).max(0.0);
            if let Some(q) = path_position(p, key, c, rot, a, ts, rng) {
                p.anchor = q;
            }
        }
        Some(MotionController::PointPlacement) => p.anchor = p.position,
        _ => {}
    }
    if !(p.position.is_finite() && p.velocity.is_finite() && p.anchor.is_finite()) {
        return false;
    }
    true
}
