// SPDX-License-Identifier: GPL-3.0-or-later
//! Effects at run time (`particle-runtime.md` §4–§7, §10, §12).
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`classify_system`], [`classify_fork`], [`ParticleNode::classification`] | The never-ending classification and the node's stored byte | §5.3, §12 rules 2 and 6 |
//! | [`ParticleNode`], [`start_path`] | The Particle node's fields, effect DID and start gates | §5.2, §5.4, `particles.md` §7.1.1 |
//! | [`GateOptions`], [`gate_allows`] | Start gating by owner kind, whitelist and options | `particles.md` §7.1.2 |
//! | [`attachment_matrix`], [`local_matrix`], [`dropped_frame`], [`owner_scale_vectors`] | Attachment frames and the owner's scale vectors | §5.6, §6.1–§6.4 |
//! | [`water_probe_segment`], [`water_height`] | The `OnWater` probe of dropped effects | §5.7 |
//! | [`EffectManager`] | Requests, attachments, sharing, stop, release, lingering, housekeeping, dropped effects | §4, §5.5, §5.6, §7 |
//!
//! The per-frame order is the Tessera decision of Q-RT1: sweep the lingering list
//! ([`EffectManager::begin_frame`]), place attached and lingering systems, run each owner's
//! housekeeping, then draw.

use std::collections::{HashMap, HashSet};

use glam::{Mat3, Mat4, Quat, Vec3};
use tessera_formats::gameplay::particles::{
    FORK_SYSTEM_PREFIX, ForkSystem, PARTICLE_SYSTEM_PREFIX, ParticleSystem, TOLERANCE,
};

use crate::fork::{ForkEffect, ForkFade};
use crate::rng::Ran2;
use crate::sim::{AttachmentSource, Environment, Placement, SystemSim};

/// Seconds after its stop at which a lingering system is removed (§7.3).
pub const LINGER_SECONDS: f64 = 6.0;
/// Interval of an effects component's housekeeping, seconds (§7.4).
pub const HOUSEKEEPING_SECONDS: f64 = 2.0;
/// Context flag that suppresses finite effects (§5.4; its source is Q-RT2).
pub const CONTEXT_FLAG_SUPPRESS: u32 = 0x40;

/// Whether an effect runs out by itself (`particle-runtime.md` §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Classification {
    /// Runs out by itself (stored byte 0).
    Finite,
    /// Runs until stopped (stored byte 1).
    NeverEnding,
}

impl Classification {
    /// The stored byte's value.
    #[must_use]
    pub fn stored_byte(self) -> bool {
        self == Self::NeverEnding
    }

    /// The classification of a stored byte.
    #[must_use]
    pub fn from_stored(byte: bool) -> Self {
        if byte {
            Self::NeverEnding
        } else {
            Self::Finite
        }
    }
}

/// The classification rule of a 0x39 effect (§5.3, rule §12.6): never-ending when
/// `physics_duration` > 0 or any emitter (active or not) has `emission_limit` = 0 and
/// `time_limit` = 0; otherwise never-ending only when the maximum of 0 and every emitter's
/// *v* is negative, which cannot happen. Returns the classification and that maximum (0
/// when the first two conditions decide).
#[must_use]
pub fn classify_system(ps: &ParticleSystem) -> (Classification, f64) {
    if ps.physics_duration > 0.0
        || ps
            .emitters
            .iter()
            .any(|e| e.emission_limit == 0 && e.time_limit == 0.0)
    {
        return (Classification::NeverEnding, 0.0);
    }
    let mut max = 0.0f64;
    for e in &ps.emitters {
        let time_limit = f64::from(e.time_limit);
        // Rule §12.6: with `burst_count` = 0 the emitter's term is `time_limit` alone.
        let span = match e.emission_limit.checked_div(e.burst_count) {
            None => time_limit,
            Some(bursts) => time_limit.max(f64::from(bursts) * f64::from(e.emit_interval)),
        };
        let mut l = f64::from(e.lifespan.base());
        if !e.lifespan.is_constant() {
            l += f64::from(e.lifespan.w[2]).abs();
        }
        let v = span + l + f64::from(e.start_time) - f64::from(ps.fast_forward);
        if v > max {
            max = v;
        }
    }
    let class = if max < 0.0 {
        Classification::NeverEnding
    } else {
        Classification::Finite
    };
    (class, max)
}

/// The classification of a 0x44 effect: never-ending when its `flag` is 1 (§5.3).
#[must_use]
pub fn classify_fork(fs: &ForkSystem) -> Classification {
    Classification::from_stored(fs.flag)
}

/// The family of an effect DID (`particles.md` §7.1.1: by the DID's DB type, here its
/// prefix).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectFamily {
    /// Particle-system descriptor (0x39).
    Particles,
    /// Fork-system descriptor (0x44).
    Fork,
}

impl EffectFamily {
    /// The family of `did`, or `None` (nothing starts).
    #[must_use]
    pub fn of(did: u32) -> Option<Self> {
        match (did >> 24) as u8 {
            PARTICLE_SYSTEM_PREFIX => Some(Self::Particles),
            FORK_SYSTEM_PREFIX => Some(Self::Fork),
            _ => None,
        }
    }
}

/// The Particle node's fields at run time (`particle-runtime.md` §5.2; names after the
/// `ParticleScript_` labels).
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleNode {
    /// `ParticleID`.
    pub particle_id: u32,
    /// `ForkParticleID`.
    pub fork_particle_id: u32,
    /// `ShouldFade` (fork effects only).
    pub should_fade: bool,
    /// `FarFadeMinDistance`, `FarFadeMaxDistance`.
    pub far_fade: [f32; 2],
    /// `CloseFadeMinDistance`, `CloseFadeMaxDistance`.
    pub close_fade: [f32; 2],
    /// `HoldingLocation`.
    pub holding_location: u32,
    /// `BoneType`.
    pub bone_type: u32,
    /// `BoneOffset`, metres.
    pub bone_offset: Vec3,
    /// `DropEffect`.
    pub drop_effect: bool,
    /// `RemoveRotation`.
    pub remove_rotation: bool,
    /// `OnWater`.
    pub on_water: bool,
    /// `ImmediateDisposal`.
    pub immediate_disposal: bool,
    /// `RedirectProperty` (a PID, 0 for none).
    pub redirect_property: u32,
    /// The stored never-ending byte after the base block, when the node has one (§5.3).
    pub stored_never_ending: Option<bool>,
}

impl Default for ParticleNode {
    /// The defaults of a new node (§5.2).
    fn default() -> Self {
        Self {
            particle_id: 0,
            fork_particle_id: 0,
            should_fade: true,
            far_fade: [40.0, 60.0],
            close_fade: [0.0, 0.0],
            holding_location: 0,
            bone_type: 0,
            bone_offset: Vec3::ZERO,
            drop_effect: false,
            remove_rotation: false,
            on_water: false,
            immediate_disposal: false,
            redirect_property: 0,
            stored_never_ending: None,
        }
    }
}

impl ParticleNode {
    /// The effect DID (`particles.md` §7.1.1): `ParticleID` when not 0, else
    /// `ForkParticleID`; `None` when both are 0.
    #[must_use]
    pub fn effect_did(&self) -> Option<u32> {
        [self.particle_id, self.fork_particle_id]
            .into_iter()
            .find(|&d| d != 0)
    }

    /// The node's classification (rule §12.2): the stored byte when there is one,
    /// otherwise `compute()` (synthetic nodes, writers).
    pub fn classification(&self, compute: impl FnOnce() -> Classification) -> Classification {
        self.stored_never_ending
            .map_or_else(compute, Classification::from_stored)
    }

    /// The fade settings a fork effect takes from this node (`particles.md` §6.5).
    #[must_use]
    pub fn fork_fade(&self) -> ForkFade {
        ForkFade {
            enabled: self.should_fade,
            far: self.far_fade,
            near: self.close_fade,
        }
    }
}

/// What starting a Particle node does (§5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartPath {
    /// Nothing starts; the node completes at once.
    Skip,
    /// The dropped path (§5.6); the node completes at once.
    Dropped,
    /// The attached path (§5.5).
    Attached,
}

/// §5.4 steps 1–5 for a node whose effect is classified as `class`, in a script context
/// with `intensity` and `context_flags`.
#[must_use]
pub fn start_path(
    node: &ParticleNode,
    class: Classification,
    intensity: f32,
    context_flags: u32,
) -> StartPath {
    if node.effect_did().is_none() {
        return StartPath::Skip;
    }
    if class == Classification::Finite
        && (!(intensity > TOLERANCE) || context_flags & CONTEXT_FLAG_SUPPRESS != 0)
    {
        return StartPath::Skip;
    }
    if node.drop_effect {
        StartPath::Dropped
    } else {
        StartPath::Attached
    }
}

/// The owner of a requested effect (`particles.md` §7.1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerRole {
    /// A static-environment object ([`is_static_environment`]).
    StaticEnvironment,
    /// The local player's own character.
    LocalPlayer,
    /// The object the local player has selected.
    Selected,
    /// Any other dynamic object.
    Other,
}

/// Whether a 64-bit instance ID names a static-environment object: top byte 0x01, 0x05 or
/// 0x81 (`particles.md` §7.1.2).
#[must_use]
pub fn is_static_environment(instance_id: u64) -> bool {
    matches!(instance_id >> 56, 0x01 | 0x05 | 0x81)
}

/// The client options that gate effect starts (`particles.md` §7.1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateOptions {
    /// `DynamicParticles` (0–4, default 0).
    pub dynamic_particles: i32,
    /// `StaticEnvironmentalObjects` (default on).
    pub static_environmental_objects: bool,
}

impl Default for GateOptions {
    fn default() -> Self {
        Self {
            dynamic_particles: 0,
            static_environmental_objects: true,
        }
    }
}

/// The start gating of `particles.md` §7.1.2 for an effect `did` on `owner`.
#[must_use]
pub fn gate_allows(
    owner: OwnerRole,
    did: u32,
    whitelist: &HashSet<u32>,
    options: GateOptions,
) -> bool {
    if owner == OwnerRole::StaticEnvironment {
        return options.static_environmental_objects;
    }
    if whitelist.contains(&did) {
        return true;
    }
    match options.dynamic_particles {
        1 => owner == OwnerRole::LocalPlayer,
        2 => matches!(owner, OwnerRole::LocalPlayer | OwnerRole::Selected),
        3 => owner == OwnerRole::Selected,
        4 => false,
        _ => true,
    }
}

/// The attachment matrix *M*ₐ (§6.1, column vectors): identity rotation and translation
/// `BoneOffset` for a bone type or no attachment; for a holding location its frame with
/// `BoneOffset` added to the translation (not rotated).
#[must_use]
pub fn attachment_matrix(node: &ParticleNode, holding_frame: Option<Mat4>) -> Mat4 {
    let offset = if node.bone_offset.is_finite() {
        node.bone_offset
    } else {
        Vec3::ZERO
    };
    match holding_frame {
        Some(h) if node.bone_type == 0 && node.holding_location != 0 && h.is_finite() => {
            let mut m = h;
            m.w_axis += offset.extend(0.0);
            m
        }
        _ => Mat4::from_translation(offset),
    }
}

/// The attachment's local matrix *L* for a frame (§6.2): *M*ₐ followed by the bone's
/// current model-space matrix (when there is a bone), with the rotation part replaced by
/// the identity when `remove_rotation` is set. The system frame is then the owner's world
/// matrix times *L*.
#[must_use]
pub fn local_matrix(ma: Mat4, bone_model: Option<Mat4>, remove_rotation: bool) -> Mat4 {
    let l = match bone_model {
        Some(b) if b.is_finite() => b * ma,
        _ => ma,
    };
    if remove_rotation {
        Mat4::from_translation(l.w_axis.truncate())
    } else {
        l
    }
}

/// The frame of a dropped effect (§5.6 step 4): the attachment frame moved by `offset`
/// along its own axes.
#[must_use]
pub fn dropped_frame(attachment_world: Mat4, offset: Vec3) -> Mat4 {
    let shift = Mat3::from_mat4(attachment_world) * offset;
    let mut m = attachment_world;
    m.w_axis += shift.extend(0.0);
    m
}

/// The owner's two scale vectors (§6.4): *A* = its render scale, *B* = (*p*, *p*, *p*) with
/// *p* its `Model_ParticleScale` (1 when absent or not a finite float).
#[must_use]
pub fn owner_scale_vectors(render_scale: Vec3, particle_scale: Option<f32>) -> (Vec3, Vec3) {
    let a = if render_scale.is_finite() {
        render_scale
    } else {
        Vec3::ONE
    };
    let p = particle_scale.filter(|p| p.is_finite()).unwrap_or(1.0);
    (a, Vec3::splat(p))
}

/// The vertical segment of the water probe (§5.7): from 40 m above to 1 m below the point
/// when it is under water, else from the point to 10 m below it. Returns (top, bottom).
#[must_use]
pub fn water_probe_segment(point: Vec3, under_water: bool) -> (Vec3, Vec3) {
    if under_water {
        (point + Vec3::Z * 40.0, point - Vec3::Z)
    } else {
        (point, point - Vec3::Z * 10.0)
    }
}

/// The dropped effect's position on water (§5.7): the point's X and Y at the highest hit
/// height, or `None` without a hit.
#[must_use]
pub fn water_height(point: Vec3, hits: impl IntoIterator<Item = f32>) -> Option<Vec3> {
    let z = hits
        .into_iter()
        .filter(|h| h.is_finite())
        .fold(None::<f32>, |m, h| Some(m.map_or(h, |m| m.max(h))))?;
    Some(Vec3::new(point.x, point.y, z))
}

/// A placement from a world matrix and the owner's scale vectors.
#[must_use]
pub fn placement_from(m: Mat4, scale_a: Vec3, scale_b: Vec3, opacity: f32) -> Placement {
    let (_, rotation, position) = m.to_scale_rotation_translation();
    Placement {
        position,
        rotation: if rotation.is_finite() {
            rotation
        } else {
            Quat::IDENTITY
        },
        scale_a,
        scale_b,
        opacity,
    }
}

// ---------------------------------------------------------------------------------------------
// The effect manager
// ---------------------------------------------------------------------------------------------

/// An owner (entity) ID.
pub type OwnerId = u64;

/// A request handle (an increasing counter, §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EffectHandle(pub u64);

/// A running system in an [`EffectManager`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SystemKey {
    index: usize,
    generation: u32,
}

/// A running effect.
#[derive(Debug, Clone)]
pub enum Effect {
    /// A 0x39 system.
    Particles(SystemSim),
    /// A 0x44 effect (not simulated, rule §10.2).
    Fork(ForkEffect),
}

impl Effect {
    /// Stops emission (0x39) or tells the fork effect to stop.
    pub fn stop(&mut self, now: f64) {
        match self {
            Self::Particles(s) => s.stop(now),
            Self::Fork(f) => f.stop(now),
        }
    }

    /// The end test: `particles.md` §8.2 for 0x39, §6.5 for 0x44.
    #[must_use]
    pub fn has_ended(&self, now: f64) -> bool {
        match self {
            Self::Particles(s) => s.has_ended(now),
            Self::Fork(f) => f.has_expired(now),
        }
    }

    fn stopped_at(&self) -> Option<f64> {
        match self {
            Self::Particles(s) => s.stopped_at(),
            Self::Fork(_) => None,
        }
    }

    /// Finished (§7.4): stopped more than 6 s ago, or the end test passes.
    #[must_use]
    pub fn has_finished(&self, now: f64) -> bool {
        self.stopped_at().is_some_and(|t| now - t > LINGER_SECONDS) || self.has_ended(now)
    }

    /// Whether the effect is of the shareable kind (`particles.md` §8.16).
    #[must_use]
    pub fn is_shareable_kind(&self) -> bool {
        match self {
            Self::Particles(s) => {
                let d = s.desc();
                d.shareable && !d.world_space && !d.use_lighting
            }
            Self::Fork(_) => false,
        }
    }

    fn world_space(&self) -> bool {
        match self {
            Self::Particles(s) => s.desc().world_space,
            Self::Fork(_) => false,
        }
    }

    /// The 0x39 system, if this is one.
    #[must_use]
    pub fn particles(&self) -> Option<&SystemSim> {
        match self {
            Self::Particles(s) => Some(s),
            Self::Fork(_) => None,
        }
    }

    /// The 0x39 system, if this is one.
    pub fn particles_mut(&mut self) -> Option<&mut SystemSim> {
        match self {
            Self::Particles(s) => Some(s),
            Self::Fork(_) => None,
        }
    }
}

#[derive(Debug)]
struct Running {
    effect: Effect,
    did: u32,
    refs: u32,
    immediate: bool,
    seq: u64,
}

#[derive(Debug)]
struct Slot {
    generation: u32,
    running: Option<Running>,
}

/// An attached effect (§4 "Attachment").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Attachment {
    /// The request handle.
    pub handle: EffectHandle,
    /// The running system.
    pub system: SystemKey,
    /// *M*ₐ (§6.1).
    pub matrix: Mat4,
    /// The bone type or holding location's bone, resolved by the caller each frame.
    pub bone: Option<u32>,
    /// `RemoveRotation`.
    pub remove_rotation: bool,
}

/// How a served request attaches (§5.5 step 6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttachSpec {
    /// *M*ₐ.
    pub matrix: Mat4,
    /// The bone, if any.
    pub bone: Option<u32>,
    /// `RemoveRotation`.
    pub remove_rotation: bool,
}

impl Default for AttachSpec {
    fn default() -> Self {
        Self {
            matrix: Mat4::IDENTITY,
            bone: None,
            remove_rotation: false,
        }
    }
}

/// An entity's effects component (§4).
#[derive(Debug, Clone, Default)]
pub struct EffectsComponent {
    pending: Vec<(EffectHandle, u32)>,
    attachments: Vec<Attachment>,
    last_check: f64,
}

impl EffectsComponent {
    /// Pending requests (handle, DID).
    #[must_use]
    pub fn pending(&self) -> &[(EffectHandle, u32)] {
        &self.pending
    }

    /// Attached effects.
    #[must_use]
    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }
}

/// A lingering entry (§4, §7.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LingerEntry {
    /// The system.
    pub system: SystemKey,
    /// Its fixed placement.
    pub placement: Placement,
    /// A removal time of its own (rule §12.7: a never-ending dropped effect goes 6 s after
    /// it was placed), checked with the end test at each sweep.
    pub remove_at: Option<f64>,
}

/// The running effects of a scene (see the module docs).
#[derive(Debug, Default)]
pub struct EffectManager {
    slots: Vec<Slot>,
    free: Vec<usize>,
    components: HashMap<OwnerId, EffectsComponent>,
    lingering: Vec<LingerEntry>,
    next_handle: u64,
    next_seq: u64,
    frame: u64,
}

impl EffectManager {
    /// An empty manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Running systems (attached, shared and lingering).
    #[must_use]
    pub fn running_count(&self) -> usize {
        self.slots.iter().filter(|s| s.running.is_some()).count()
    }

    /// The lingering list.
    #[must_use]
    pub fn lingering(&self) -> &[LingerEntry] {
        &self.lingering
    }

    /// An owner's component.
    #[must_use]
    pub fn component(&self, owner: OwnerId) -> Option<&EffectsComponent> {
        self.components.get(&owner)
    }

    /// The effect of `key`.
    #[must_use]
    pub fn effect(&self, key: SystemKey) -> Option<&Effect> {
        self.running(key).map(|r| &r.effect)
    }

    /// The effect of `key`.
    pub fn effect_mut(&mut self, key: SystemKey) -> Option<&mut Effect> {
        self.running_mut(key).map(|r| &mut r.effect)
    }

    /// Every running system with its key, oldest first.
    pub fn effects_mut(&mut self) -> impl Iterator<Item = (SystemKey, &mut Effect)> {
        let mut v: Vec<(u64, SystemKey, &mut Effect)> = self
            .slots
            .iter_mut()
            .enumerate()
            .filter_map(|(index, s)| {
                let key = SystemKey {
                    index,
                    generation: s.generation,
                };
                s.running.as_mut().map(|r| (r.seq, key, &mut r.effect))
            })
            .collect();
        v.sort_by_key(|(seq, _, _)| *seq);
        v.into_iter().map(|(_, k, e)| (k, e))
    }

    /// The record DID a running system was made from.
    #[must_use]
    pub fn did_of(&self, key: SystemKey) -> Option<u32> {
        self.running(key).map(|r| r.did)
    }

    /// The creation rank of a running system (lower is older, `particles.md` §8.15).
    #[must_use]
    pub fn age_rank(&self, key: SystemKey) -> Option<u64> {
        self.running(key).map(|r| r.seq)
    }

    fn running(&self, key: SystemKey) -> Option<&Running> {
        self.slots
            .get(key.index)
            .filter(|s| s.generation == key.generation)
            .and_then(|s| s.running.as_ref())
    }

    fn running_mut(&mut self, key: SystemKey) -> Option<&mut Running> {
        self.slots
            .get_mut(key.index)
            .filter(|s| s.generation == key.generation)
            .and_then(|s| s.running.as_mut())
    }

    fn insert(&mut self, effect: Effect, did: u32) -> SystemKey {
        let seq = self.next_seq;
        self.next_seq += 1;
        let running = Running {
            effect,
            did,
            refs: 1,
            immediate: false,
            seq,
        };
        if let Some(index) = self.free.pop()
            && let Some(slot) = self.slots.get_mut(index)
        {
            slot.generation = slot.generation.wrapping_add(1);
            slot.running = Some(running);
            return SystemKey {
                index,
                generation: slot.generation,
            };
        }
        self.slots.push(Slot {
            generation: 0,
            running: Some(running),
        });
        SystemKey {
            index: self.slots.len() - 1,
            generation: 0,
        }
    }

    fn destroy(&mut self, key: SystemKey) {
        if let Some(slot) = self.slots.get_mut(key.index)
            && slot.generation == key.generation
            && slot.running.take().is_some()
        {
            self.free.push(key.index);
        }
        self.lingering.retain(|l| l.system != key);
    }

    /// The newest running shareable system made from `did` (`particles.md` §8.16).
    #[must_use]
    pub fn shared(&self, did: u32) -> Option<SystemKey> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(index, s)| {
                let r = s.running.as_ref()?;
                // A system without the distance fade is a beam's own (§11.1, RT46).
                let beams_own =
                    matches!(&r.effect, Effect::Particles(p) if !p.distance_fade_enabled());
                (r.did == did && r.refs > 0 && r.effect.is_shareable_kind() && !beams_own)
                    .then_some((
                        r.seq,
                        SystemKey {
                            index,
                            generation: s.generation,
                        },
                    ))
            })
            .max_by_key(|(seq, _)| *seq)
            .map(|(_, k)| k)
    }

    /// Starts a frame (§3.2 step 1): records the frame number and sweeps the lingering list
    /// (§7.3): an entry goes when its system was stopped more than 6 s ago or its end test
    /// passes.
    pub fn begin_frame(&mut self, frame: u64, now: f64) {
        self.frame = frame;
        let done: Vec<SystemKey> = self
            .lingering
            .iter()
            .filter(|l| {
                l.remove_at.is_some_and(|t| now > t)
                    || self
                        .running(l.system)
                        .is_none_or(|r| r.effect.has_finished(now))
            })
            .map(|l| l.system)
            .collect();
        for key in done {
            self.destroy(key);
        }
    }

    /// Asks for an effect on `owner` (§5.5 step 4): a new handle on the owner's pending
    /// list. The component is created at `now` when the owner had none.
    pub fn request(&mut self, owner: OwnerId, did: u32, now: f64) -> EffectHandle {
        let handle = EffectHandle(self.next_handle);
        self.next_handle += 1;
        self.components
            .entry(owner)
            .or_insert_with(|| EffectsComponent {
                last_check: now,
                ..EffectsComponent::default()
            })
            .pending
            .push((handle, did));
        handle
    }

    /// Serves a request whose record has loaded (§5.5 step 6): a shareable running copy of
    /// the DID when there is one, otherwise `make()`. Returns `None` when the handle is no
    /// longer pending (it was stopped meanwhile, §7.5: the new system is discarded) or
    /// `make` gives nothing.
    pub fn serve(
        &mut self,
        owner: OwnerId,
        handle: EffectHandle,
        make: impl FnOnce() -> Option<Effect>,
        spec: AttachSpec,
    ) -> Option<SystemKey> {
        self.serve_with(owner, handle, make, spec, true)
    }

    fn serve_with(
        &mut self,
        owner: OwnerId,
        handle: EffectHandle,
        make: impl FnOnce() -> Option<Effect>,
        spec: AttachSpec,
        share: bool,
    ) -> Option<SystemKey> {
        let comp = self.components.get_mut(&owner)?;
        let at = comp.pending.iter().position(|(h, _)| *h == handle)?;
        let (_, did) = comp.pending.remove(at);
        let key = match self.shared(did).filter(|_| share) {
            Some(k) => {
                if let Some(r) = self.running_mut(k) {
                    r.refs += 1;
                }
                k
            }
            None => {
                let effect = make()?;
                self.insert(effect, did)
            }
        };
        if let Some(comp) = self.components.get_mut(&owner) {
            comp.attachments.push(Attachment {
                handle,
                system: key,
                matrix: spec.matrix,
                bone: spec.bone,
                remove_rotation: spec.remove_rotation,
            });
        }
        Some(key)
    }

    /// [`EffectManager::request`] and [`EffectManager::serve`] at once (a record that is
    /// already loaded).
    pub fn start_attached(
        &mut self,
        owner: OwnerId,
        did: u32,
        now: f64,
        make: impl FnOnce() -> Option<Effect>,
        spec: AttachSpec,
    ) -> Option<(EffectHandle, SystemKey)> {
        let handle = self.request(owner, did, now);
        let key = self.serve(owner, handle, make, spec)?;
        Some((handle, key))
    }

    /// [`EffectManager::start_attached`] that always makes its own system, never a shared
    /// running copy: a beam creates its systems itself (`particle-runtime.md` §11.1 step 1)
    /// and drives them (step 8).
    pub fn start_attached_own(
        &mut self,
        owner: OwnerId,
        did: u32,
        now: f64,
        make: impl FnOnce() -> Option<Effect>,
        spec: AttachSpec,
    ) -> Option<(EffectHandle, SystemKey)> {
        let handle = self.request(owner, did, now);
        let key = self.serve_with(owner, handle, make, spec, false)?;
        Some((handle, key))
    }

    /// Stops the effect of `handle` on `owner` (§7.1): removes it from the pending list,
    /// removes its attachment, marks the system for immediate disposal when asked, then
    /// releases it (§7.2).
    pub fn stop(&mut self, owner: OwnerId, handle: EffectHandle, immediate: bool, now: f64) {
        let Some(comp) = self.components.get_mut(&owner) else {
            return;
        };
        comp.pending.retain(|(h, _)| *h != handle);
        let Some(at) = comp.attachments.iter().position(|a| a.handle == handle) else {
            return;
        };
        let att = comp.attachments.remove(at);
        if immediate && let Some(r) = self.running_mut(att.system) {
            r.immediate = true;
        }
        self.release(att.system, now);
    }

    /// Releases one reference to `key` (§7.2). When it was the last, the system lingers or
    /// is destroyed.
    pub fn release(&mut self, key: SystemKey, now: f64) {
        let frame = self.frame;
        let Some(r) = self.running_mut(key) else {
            return;
        };
        r.refs = r.refs.saturating_sub(1);
        if r.refs > 0 {
            return;
        }
        let world_space = r.effect.world_space();
        let instance = match &r.effect {
            Effect::Particles(s) if s.instance_frame() == Some(frame) => {
                s.instances().first().copied()
            }
            _ => None,
        };
        let opaque_enough = instance.is_none_or(|i| i.opacity >= TOLERANCE);
        let lingers = opaque_enough && !r.immediate && !r.effect.is_shareable_kind();
        let placement = if world_space {
            Some(instance.unwrap_or_else(|| match &r.effect {
                Effect::Particles(s) => s.instances().first().copied().unwrap_or_default(),
                Effect::Fork(_) => Placement::default(),
            }))
        } else {
            instance
        };
        match (lingers, placement) {
            (true, Some(placement)) => {
                r.effect.stop(now);
                self.lingering.push(LingerEntry {
                    system: key,
                    placement,
                    remove_at: None,
                });
            }
            _ => self.destroy(key),
        }
    }

    /// Places a dropped effect once in the world (§5.6 step 5): a lingering entry that was
    /// never stopped, removed when its end test passes. A never-ending dropped effect (never
    /// in the data) runs normally and is removed at the first lingering sweep more than 6 s
    /// after it was placed (rule §12.7): it is not stopped first, so it shows its first 6 s.
    pub fn drop_effect(
        &mut self,
        did: u32,
        effect: Effect,
        placement: Placement,
        class: Classification,
        now: f64,
    ) -> SystemKey {
        let remove_at = (class == Classification::NeverEnding).then(|| {
            if now.is_finite() {
                now + LINGER_SECONDS
            } else {
                0.0
            }
        });
        let key = self.insert(effect, did);
        if let Some(r) = self.running_mut(key) {
            r.refs = 0;
        }
        self.lingering.push(LingerEntry {
            system: key,
            placement,
            remove_at,
        });
        key
    }

    /// The housekeeping of `owner`'s component (§7.4): when more than 2 s have passed since
    /// its last check, it records `now` and removes (releases) every attachment whose
    /// system has finished.
    pub fn housekeep(&mut self, owner: OwnerId, now: f64) {
        let Some(comp) = self.components.get_mut(&owner) else {
            return;
        };
        if !(now - comp.last_check > HOUSEKEEPING_SECONDS) {
            return;
        }
        comp.last_check = now;
        let attachments = std::mem::take(&mut comp.attachments);
        let mut keep = Vec::with_capacity(attachments.len());
        let mut finished = Vec::new();
        for a in attachments {
            let done = self
                .running(a.system)
                .is_none_or(|r| r.effect.has_finished(now));
            if done {
                finished.push(a.system);
            } else {
                keep.push(a);
            }
        }
        if let Some(comp) = self.components.get_mut(&owner) {
            comp.attachments = keep;
        }
        for key in finished {
            self.release(key, now);
        }
    }

    /// The owner's component goes away (entity removal): every attachment is released and
    /// pending requests are dropped.
    pub fn remove_owner(&mut self, owner: OwnerId, now: f64) {
        if let Some(comp) = self.components.remove(&owner) {
            for a in comp.attachments {
                self.release(a.system, now);
            }
        }
    }

    /// Places `owner`'s attached 0x39 systems for frame `frame` (§3.2 step 2, §6.2):
    /// `bone_model(bone)` gives a bone's current model-space matrix, `owner_world` the
    /// owner's frame; `scale_a`, `scale_b` and `opacity` are the owner's (§6.4).
    #[allow(clippy::too_many_arguments)]
    pub fn place_owner(
        &mut self,
        owner: OwnerId,
        frame: u64,
        now: f64,
        owner_world: Mat4,
        scale_a: Vec3,
        scale_b: Vec3,
        opacity: f32,
        bone_model: &dyn Fn(u32) -> Option<Mat4>,
        env: &Environment,
        rng: &mut Ran2,
        attach: &dyn AttachmentSource,
    ) {
        let Some(comp) = self.components.get(&owner) else {
            return;
        };
        let atts = comp.attachments.clone();
        for a in atts {
            let l = local_matrix(a.matrix, a.bone.and_then(bone_model), a.remove_rotation);
            let p = placement_from(owner_world * l, scale_a, scale_b, opacity);
            if let Some(s) = self
                .running_mut(a.system)
                .and_then(|r| r.effect.particles_mut())
            {
                s.place(frame, now, p, env, rng, attach);
            }
        }
    }

    /// Places every lingering 0x39 system with its fixed placement (§3.2 step 2).
    pub fn place_lingering(
        &mut self,
        frame: u64,
        now: f64,
        env: &Environment,
        rng: &mut Ran2,
        attach: &dyn AttachmentSource,
    ) {
        let entries = self.lingering.clone();
        for l in entries {
            if let Some(s) = self
                .running_mut(l.system)
                .and_then(|r| r.effect.particles_mut())
            {
                s.place(frame, now, l.placement, env, rng, attach);
            }
        }
    }
}
