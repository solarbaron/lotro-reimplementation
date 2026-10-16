// SPDX-License-Identifier: GPL-3.0-or-later
//! Session figures in the effects runtime (`combat-visuals.md`): the figures of the session's
//! entities (scenario actors, test-server actors, the player) as script owners, so that the
//! combat layer ([`crate::walker::combat`]) can run behaviours, hit reactions, effect, state
//! and death scripts on them; the parts of the sink those scripts need beyond particles:
//!
//! | Part | Spec |
//! |---|---|
//! | Animation nodes play clips on the figure ([`ClipCommand`], applied by the character layer) | `script-table.md` §6.10 |
//! | StoryHook events are queued for the combat layer | `script-table.md` §9.1, `combat-visuals.md` §4.2 |
//! | Execute callbacks `GetUpcomingTargets` and `GetUpcomingHitResult` answer from the open stories | `combat-visuals.md` §4.6, §11 rule 8 |
//! | Beams: envelope, ends, head and arrival (the node's port), beam and head particle systems | `particle-runtime.md` §11.1, rule 14 |
//!
//! Session figures are spawned without their start-up scripts (the character layer already
//! animates them by the walker's own rule, `walker.md` §10), with their figure's tables and
//! stored values, and seeded from their network ID.
//!
//! A beam's ribbon (§11.1 step 6) is drawn by [`super::EffectLayer::draw`] with the surfaces
//! of its three materials, its head entity (step 9) by [`super::EffectLayer::draw_heads`]
//! through the object layer's instanced models.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tessera_formats::script_table::{AnimationNode, BeamNode};
use tessera_formats::{Did, PropertyArray, PropertyEntry, PropertyValue};
use tessera_particles::beam::{
    BEAM_EPSILON, Beam, BeamEnds, BeamSpan, beam_rotation, head_scale_vector,
};
use tessera_particles::glam::{Mat4, Quat, Vec3};
use tessera_particles::runtime::AttachSpec;
use tessera_particles::{EffectManager, Environment, NoAttachments, Ran2, SystemKey};
use tessera_scriptgraph::{
    AnimationHandle, AnimationOutcome, BeamHandle, BeamStart, EntityId, ExecuteReply,
    ExecuteRequest, StoryHookEvent,
};

use super::data::{EffectRecord, OwnerKind};
use super::{FxState, LiveOwner, OwnerFrame, make_effect};
use crate::walker::characters::FigureEffectView;
use crate::walker::characters::combat_anim::{ClipCommand, ClipRequest, ClipRun};
use crate::walker::characters::figure::FigureContext;
use crate::walker::combat::state::UpcomingView;

/// Runtime IDs of session figures: this plus a counter (the static and placed-figure owners'
/// IDs carry a landblock DID in their high word).
pub const SESSION_OWNER_BASE: u64 = 1 << 32;
/// Effect-manager owners of beams' particle systems: this plus the beam handle.
pub const BEAM_OWNER_BASE: u64 = 2 << 32;
/// Most session figures spawned at once.
pub const MAX_SESSION_OWNERS: usize = 1024;
/// Most beams alive at once (further Beam nodes make no beam).
pub const MAX_BEAMS: usize = 256;
/// Most story hooks queued between two drains.
pub const MAX_PENDING_HOOKS: usize = 4096;
/// Most clip Play commands queued between two drains.
pub const MAX_CLIP_COMMANDS: usize = 4096;
/// Most entries of an Execute target array.
pub const MAX_UPCOMING_TARGETS: usize = 256;
/// Height above an owner's origin used as its box centre when it has no skeleton point
/// (Tessera choice for `particle-runtime.md` §11.1 step 3).
pub const BOX_CENTRE_HEIGHT: f32 = 0.9;

/// The PIDs and callback keys the Execute callbacks use (`combat-visuals.md` §4.6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExecuteVocab {
    /// The `GetUpcomingTargets` key of the callback DID map.
    pub upcoming_targets: Option<u32>,
    /// The `GetUpcomingHitResult` key.
    pub upcoming_hit_result: Option<u32>,
    /// `SCP_GetUpcomingTargets_CombatResultType_Filter`.
    pub filter: Option<u32>,
    /// `SCP_Output_GetUpcomingTargets_TargetArray`.
    pub targets_out: Option<u32>,
    /// `SCP_GetUpcomingTargets_Result`.
    pub targets_result: Option<u32>,
    /// `SCP_GetUpcomingHitResult_Result`.
    pub hit_result_out: Option<u32>,
}

/// One beam with its particle systems.
#[derive(Debug)]
pub(super) struct BeamRun {
    beam: Beam,
    source: EntityId,
    target: Option<EntityId>,
    owner: u64,
    systems: [Option<SystemKey>; 2],
    last: f64,
    finished: bool,
    /// The beam's frame at its last span (rotation, tail point): the systems stay placed
    /// there while the beam has no span, so their particles keep ageing (§11.1 steps 8, 10).
    frame: Option<(Quat, Vec3)>,
    /// The span whose ribbon is drawn this frame (§11.1 step 6; `None`: not drawn).
    pub ribbon: Option<BeamSpan>,
    /// Where the head entity stands this frame (§11.1 step 9; `None`: hidden): the head point
    /// and the beam's frame.
    pub head: Option<(Vec3, Quat)>,
    /// The vector `HeadEntityScaleSource` selected from the source at creation (step 1).
    pub head_scale: Vec3,
}

impl BeamRun {
    /// The Beam node's fields.
    pub fn node(&self) -> &BeamNode {
        self.beam.node()
    }
}

/// The session part of the effects state.
#[derive(Debug, Default)]
pub(super) struct SessionState {
    pub views: HashMap<u64, FigureEffectView>,
    pub by_net: HashMap<u64, EntityId>,
    pub by_runtime: HashMap<EntityId, u64>,
    pub next: u64,
    pub clips: Vec<ClipCommand>,
    pub next_clip: u64,
    pub clip_ctx: Option<Arc<FigureContext>>,
    pub hooks: Vec<StoryHookEvent>,
    pub upcoming: UpcomingView,
    pub exec: ExecuteVocab,
    pub beams: HashMap<u64, BeamRun>,
    pub next_beam: u64,
    pub arrived: Vec<BeamHandle>,
    pub finished: Vec<BeamHandle>,
    pub requests: u64,
    pub beams_started: u64,
    pub beam_arrivals: u64,
    pub clips_started: u64,
    pub story_hooks: u64,
    pub story_hooks_with_story: u64,
}

impl SessionState {
    /// Whether `id` is a session figure.
    pub fn is_session(&self, id: EntityId) -> bool {
        self.by_runtime.contains_key(&id)
    }
}

/// The world point of an attachment on an owner (`particle-runtime.md` §11.1 step 3, built
/// as §6.1–§6.2 attach an effect): a bone type (the offset in the bone's space), else a
/// holding location (its frame with the offset added to its translation, on its bone), else
/// the owner's model origin (the offset in the model's space); followed by the owner's world
/// frame with its render scale. With `box_centre`, or a point the owner lacks, the model's
/// box centre (the Tessera stand-in [`BOX_CENTRE_HEIGHT`] above its origin) with the offset
/// in the model's space. In sim coordinates.
fn attach_point(
    f: &OwnerFrame<'_>,
    holding: u32,
    bone: u32,
    offset: [f32; 3],
    box_centre: bool,
) -> Vec3 {
    let offset = Vec3::from_array(offset);
    let offset = if offset.is_finite() {
        offset
    } else {
        Vec3::ZERO
    };
    let centre = Vec3::new(0.0, 0.0, BOX_CENTRE_HEIGHT) + offset;
    let local = if box_centre {
        None
    } else if bone != 0 {
        // §6.1: *M*ₐ = the offset; *L* = *M*ₐ on the bone's model-space matrix.
        f.points
            .and_then(|p| p.bone(bone))
            .and_then(|b| f.model.get(b))
            .filter(|m| m.is_finite())
            .map(|m| m.transform_point3(offset))
    } else if holding != 0 {
        // §6.1: *M*ₐ = the holding location's frame with the offset added to its
        // translation (not rotated by the location's rotation), on its bone.
        f.points.and_then(|p| p.holding(holding)).map(|(b, m)| {
            let mut ma = m;
            ma.w_axis += offset.extend(0.0);
            let l = match b.and_then(|b| f.model.get(b)).filter(|bm| bm.is_finite()) {
                Some(bm) => *bm * ma,
                None => ma,
            };
            l.w_axis.truncate()
        })
    } else {
        Some(offset)
    };
    let p = f.world.transform_point3(local.unwrap_or(centre));
    if p.is_finite() {
        p
    } else {
        f.world.w_axis.truncate()
    }
}

/// What one beam did in a frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct BeamStep {
    /// The head arrived (fire the node's Default ports, §11.1 step 7).
    arrived_now: bool,
    /// The beam was removed (§11.1 step 10): end its node.
    removed: bool,
}

/// One frame of one beam (`particle-runtime.md` §11.1 steps 2–10) with its ends resolved.
fn step_beam(
    manager: &mut EffectManager,
    rng: &mut Ran2,
    b: &mut BeamRun,
    ends: BeamEnds,
    now: f64,
    frame: u64,
    env: &Environment,
) -> BeamStep {
    let mut out = BeamStep::default();
    if b.finished {
        return out;
    }
    let systems = b.systems;
    let alive = |k: Option<SystemKey>| {
        k.is_some_and(|k| manager.effect(k).and_then(|e| e.particles()).is_some())
    };
    let (beam_sys, head_sys) = (alive(systems[0]), alive(systems[1]));
    #[allow(clippy::cast_possible_truncation)]
    let dt = (now - b.last).max(0.0) as f32;
    b.last = now;
    let u = b.beam.update(now, dt, ends, beam_sys, head_sys, rng);
    out.arrived_now = u.arrived_now;
    let n = b.beam.node();
    let finished_env = b.beam.finished(now);
    // Step 8: both systems are placed every frame in the beam's frame with opacity alpha × e,
    // also once the envelope is done (e = 0) or the beam has no span this frame (its last
    // frame), so that their particles keep ageing and dying (step 10). Only spawning is
    // gated by the envelope.
    // The beam's frame: forward along the beam, up from world up (§11.1 step 9; the same
    // frame places the systems, step 8).
    if let Some(span) = u.span {
        let rot = beam_rotation(span.source, span.target).unwrap_or(Quat::IDENTITY);
        b.frame = Some((rot, span.tail));
    }
    let intensity = u.span.map_or(0.0, |s| s.intensity);
    // Step 6: the ribbon while the tail is before the head and the drawn segment is at least
    // 0.1 m long, its alpha × e (nothing to draw at e = 0 or once the envelope finished).
    b.ribbon = u
        .span
        .filter(|_| u.ribbon && !finished_env && intensity > 0.0);
    // Step 9: the head entity at the head point, turned along the beam, while shown.
    b.head = match (u.span, b.frame) {
        (Some(span), Some((rot, _))) if u.head_entity_shown && !finished_env => {
            Some((span.head, rot))
        }
        _ => None,
    };
    if let Some((rot, tail)) = b.frame {
        let e = if finished_env { 0.0 } else { intensity };
        let alpha = f32::from(n.color[3]) / 255.0 * e;
        let alpha = if alpha.is_finite() { alpha } else { 0.0 };
        let world = Mat4::from_rotation_translation(rot, tail);
        manager.place_owner(
            b.owner,
            frame,
            now,
            world,
            Vec3::ONE,
            Vec3::ONE,
            alpha.clamp(0.0, 1.0),
            &|_| None,
            env,
            rng,
            &NoAttachments,
        );
        if intensity > BEAM_EPSILON && u.span.is_some() {
            let mut spawn = |key: Option<SystemKey>, t: f64, p: Vec3| {
                let Some(sim) = key
                    .and_then(|k| manager.effect_mut(k))
                    .and_then(|e| e.particles_mut())
                else {
                    return;
                };
                let local = p - tail;
                let offset = if sim.desc().world_space {
                    local
                } else {
                    rot.inverse() * local
                };
                sim.spawn_at(now, t, offset, rng);
            };
            for (t, p) in &u.beam_spawns {
                spawn(systems[0], *t, *p);
            }
            if let Some((t, p)) = u.head_spawn {
                spawn(systems[1], t, p);
            }
        }
    }
    // Step 10: removed once finished and each system is absent, has a zero rate or holds no
    // live particle.
    if finished_env {
        // `particle-runtime.md` §11.1 Decision (RT46 answered in part): a finished beam's
        // systems stop emitting on their own, so that step 10 is reached.
        for k in systems.iter().flatten() {
            if let Some(e) = manager.effect_mut(*k) {
                e.stop(now);
            }
        }
        let rates = [n.beam_particles_per_second, n.head_particles_per_second];
        let empty = systems.iter().zip(rates).all(|(k, rate)| {
            rate == 0
                || k.is_none_or(|k| {
                    manager
                        .effect(k)
                        .and_then(|e| e.particles())
                        .is_none_or(|s| s.live_particles() == 0)
                })
        });
        if empty {
            manager.remove_owner(b.owner, now);
            b.finished = true;
            b.ribbon = None;
            b.head = None;
            out.removed = true;
        }
    }
    out
}

impl FxState {
    /// The world frame of an owner as `(frame, forward)`, when the owner is placed.
    fn owner_point(
        &self,
        id: EntityId,
        holding: u32,
        bone: u32,
        offset: [f32; 3],
        box_centre: bool,
    ) -> Option<(Vec3, Vec3)> {
        let owner = self.owners.get(&id)?;
        let f = self.owner_frame(owner)?;
        let p = attach_point(&f, holding, bone, offset, box_centre);
        let fwd = f.world.transform_vector3(Vec3::Y);
        Some((p, fwd))
    }

    // ----- sink parts -----

    /// An animation node on a session figure (`script-table.md` §6.10): the clip bound to the
    /// figure's skeleton, queued for the character layer.
    pub(super) fn session_animation(
        &mut self,
        entity: EntityId,
        node: &AnimationNode,
        intensity: f32,
    ) -> AnimationOutcome {
        let Some(&net) = self.session.by_runtime.get(&entity) else {
            return AnimationOutcome::CompleteAtOnce;
        };
        let (Some(view), Some(ctx)) = (self.session.views.get(&net), &self.session.clip_ctx) else {
            return AnimationOutcome::CompleteAtOnce;
        };
        let Some(skel) = view.skeleton.as_ref() else {
            return AnimationOutcome::CompleteAtOnce;
        };
        if node.asset.is_null() || self.session.clips.len() >= MAX_CLIP_COMMANDS {
            return AnimationOutcome::CompleteAtOnce;
        }
        let Ok(clip) = ctx.clips.bind(&ctx.set, node.asset.0, skel) else {
            return AnimationOutcome::CompleteAtOnce;
        };
        let start = if node.randomize_start_time {
            self.rng.range(node.start_time, node.end_time)
        } else {
            node.start_time
        };
        let handle = self.session.next_clip;
        self.session.next_clip = self.session.next_clip.wrapping_add(1);
        let request = ClipRequest {
            handle,
            clip,
            node: *node,
            intensity,
            start: if start.is_finite() { start } else { 0.0 },
        };
        let outcome = match request.run() {
            ClipRun::Refused => return AnimationOutcome::CompleteAtOnce,
            ClipRun::For(t) => AnimationOutcome::RunFor(AnimationHandle(handle), t),
            ClipRun::UntilStopped => AnimationOutcome::RunUntilStopped(AnimationHandle(handle)),
        };
        self.session.clips_started += 1;
        self.session.clips.push(ClipCommand::Play {
            entity: tessera_net::EntityId(net),
            request,
        });
        outcome
    }

    /// A stopped animation node (§6.10 step 8). Stops are never dropped (a lost stop would
    /// leave its clip playing); there is at most one per started clip, and Play commands are
    /// bounded by [`MAX_CLIP_COMMANDS`].
    pub(super) fn session_animation_stop(&mut self, handle: AnimationHandle) {
        self.session
            .clips
            .push(ClipCommand::Stop { handle: handle.0 });
    }

    /// A driven field of a playing clip's node changed (§6.10 step 7): the new rate, held
    /// time and `UseBoneOpacity` go to the character layer. Like stops, updates are never
    /// dropped (at most one per driven change of a live clip).
    pub(super) fn session_animation_update(
        &mut self,
        handle: AnimationHandle,
        node: &AnimationNode,
    ) {
        self.session.clips.push(ClipCommand::Update {
            handle: handle.0,
            node: *node,
        });
    }

    /// A StoryHook event, queued for the combat layer.
    pub(super) fn session_hook(&mut self, event: &StoryHookEvent) {
        self.session.story_hooks += 1;
        self.session.story_hooks_with_story += u64::from(event.story.is_some());
        if self.session.hooks.len() < MAX_PENDING_HOOKS {
            self.session.hooks.push(*event);
        }
    }

    /// The Execute callbacks of `combat-visuals.md` §11 rule 8.
    pub(super) fn session_execute(&mut self, req: &ExecuteRequest<'_>) -> Option<ExecuteReply> {
        let x = self.session.exec;
        let v = &self.session.upcoming;
        if Some(req.callback) == x.upcoming_targets && req.callback != 0 {
            let filter: Vec<u32> = x
                .filter
                .and_then(|p| req.call_properties.get(p))
                .and_then(PropertyValue::as_array)
                .map(|a| {
                    a.elements
                        .iter()
                        .filter_map(|e| e.value.as_bits())
                        .filter_map(|b| u32::try_from(b).ok())
                        .collect()
                })
                .unwrap_or_default();
            let mut targets = v.targets(req.story, &filter);
            targets.truncate(MAX_UPCOMING_TARGETS);
            let mut outputs = Vec::new();
            if let Some(p) = x.targets_out {
                outputs.push((
                    p,
                    PropertyValue::Array(PropertyArray {
                        count: u32::try_from(targets.len()).unwrap_or(u32::MAX),
                        elements: targets
                            .iter()
                            .map(|t| PropertyEntry {
                                pid: p,
                                value: PropertyValue::InstanceId(t.0),
                            })
                            .collect(),
                    }),
                ));
            }
            if let Some(p) = x.targets_result {
                outputs.push((p, PropertyValue::Bool(!targets.is_empty())));
            }
            return Some(ExecuteReply { outputs });
        }
        if Some(req.callback) == x.upcoming_hit_result && req.callback != 0 {
            let r = v.hit_result(req.story);
            return Some(ExecuteReply {
                outputs: x
                    .hit_result_out
                    .map(|p| vec![(p, PropertyValue::Enum(r))])
                    .unwrap_or_default(),
            });
        }
        None
    }

    /// A Beam node asks for a beam (`particle-runtime.md` §11.1 step 1): its systems are
    /// created at once, without the start gating.
    pub(super) fn session_beam(&mut self, req: &BeamStart) -> Option<BeamHandle> {
        if self.session.beams.len() >= MAX_BEAMS || !self.owners.contains_key(&req.entity) {
            return None;
        }
        let handle = self.session.next_beam;
        self.session.next_beam = self.session.next_beam.wrapping_add(1);
        let owner = BEAM_OWNER_BASE | (handle & 0xFFFF_FFFF);
        let beam = Beam::new(&req.node, self.now, &mut self.rng);
        let mut systems = [None, None];
        let mut pending = Vec::new();
        for (slot, did) in [req.node.beam_particle_system, req.node.head_particle_system]
            .into_iter()
            .enumerate()
        {
            if did.is_null() {
                continue;
            }
            // The record is read on a worker (ARCHITECTURE §21): a system whose record has
            // not loaded yet is created when it lands (`FxState::beam_system_loaded`).
            match self.data.cached_record(did.0) {
                Some(Some(record)) => systems[slot] = self.beam_system(owner, did.0, &record),
                Some(None) => {}
                None => pending.push((slot, did.0)),
            }
        }
        for (slot, did) in pending {
            self.wait(did, super::Waiting::Beam { beam: handle, slot });
            self.load(did);
        }
        // The three materials are loaded now (step 1), read on a worker like the systems.
        self.load_ribbon_materials([
            req.node.tail_material,
            req.node.body_material,
            req.node.head_material,
        ]);
        // Step 1: the head entity's scale vector, from the source's render scale.
        let source_scale = self
            .owners
            .get(&req.entity)
            .and_then(|o| self.owner_frame(o))
            .map_or(Vec3::ONE, |f| f.scale);
        let head_scale = head_scale_vector(req.node.head_entity_scale_source, source_scale);
        self.session.beams.insert(
            handle,
            BeamRun {
                beam,
                source: req.entity,
                target: req.target,
                owner,
                systems,
                last: self.now,
                finished: false,
                frame: None,
                ribbon: None,
                head: None,
                head_scale,
            },
        );
        self.session.beams_started += 1;
        Some(BeamHandle(handle))
    }

    /// Creates a beam's particle system from its record (§11.1 step 1, without the start
    /// gating), with the camera-distance fade switched off (step 8, RT46).
    fn beam_system(
        &mut self,
        owner: u64,
        did: u32,
        record: &Arc<EffectRecord>,
    ) -> Option<SystemKey> {
        if !matches!(&**record, EffectRecord::Particles { .. }) {
            return None;
        }
        self.note_texture(record);
        let now = self.now;
        let node = tessera_formats::script_table::ParticleScriptNode::default();
        let key = self
            .manager
            .start_attached_own(
                owner,
                did,
                now,
                || make_effect(record, now, &node),
                AttachSpec::default(),
            )
            .map(|(_, key)| key)?;
        // The beam's systems skip the camera-distance fade and cut of `particles.md` §8.13
        // (§11.1 step 8, RT46).
        if let Some(sim) = self.manager.effect_mut(key).and_then(|e| e.particles_mut()) {
            sim.set_distance_fade(false);
        }
        Some(key)
    }

    /// Asks for the surface records of a beam's three materials (§11.1 steps 1, 6); once
    /// loaded, their textures are acquired.
    fn load_ribbon_materials(&mut self, materials: [Did; 3]) {
        for did in materials {
            if did.is_null() {
                continue;
            }
            match self.data.cached_record(did.0) {
                None => self.load(did.0),
                Some(Some(record)) => self.note_texture(&record),
                Some(None) => {}
            }
        }
    }

    /// A beam's system's record landed: the system is created when its beam still runs.
    pub(super) fn beam_system_loaded(
        &mut self,
        beam: u64,
        slot: usize,
        did: u32,
        record: &Arc<EffectRecord>,
    ) {
        let Some(owner) = self
            .session
            .beams
            .get(&beam)
            .filter(|b| !b.finished && b.systems.get(slot).is_some_and(Option::is_none))
            .map(|b| b.owner)
        else {
            return;
        };
        let key = self.beam_system(owner, did, record);
        if let Some(b) = self.session.beams.get_mut(&beam)
            && let Some(s) = b.systems.get_mut(slot)
        {
            *s = key;
        }
    }

    /// A Beam node stopped: its beam ramps out (§11.1 step 2).
    pub(super) fn session_beam_stop(&mut self, handle: BeamHandle) {
        let now = self.now;
        if let Some(b) = self.session.beams.get_mut(&handle.0) {
            b.beam.stop(now);
        }
    }

    /// One frame of every beam (§11.1 steps 2–10): ends, envelope, head, arrival, systems.
    pub(super) fn update_beams(&mut self, frame: u64, env: &Environment) {
        let now = self.now;
        let handles: Vec<u64> = self.session.beams.keys().copied().collect();
        for h in handles {
            let Some(b) = self.session.beams.get(&h) else {
                continue;
            };
            if b.finished {
                continue;
            }
            let n: &BeamNode = b.beam.node();
            let materials = [n.tail_material, n.body_material, n.head_material];
            let src = self.owner_point(
                b.source,
                n.source_holding_location,
                n.source_bone_type,
                n.source_bone_offset,
                n.source_use_box_center,
            );
            let tgt = b.target.and_then(|t| {
                self.owner_point(
                    t,
                    n.target_holding_location,
                    n.target_bone_type,
                    n.target_bone_offset,
                    n.target_use_box_center,
                )
            });
            let ends = BeamEnds {
                source: src.map(|s| s.0),
                source_forward: src.map_or(Vec3::Y, |s| s.1),
                target: tgt.map(|t| t.0),
            };
            let FxState {
                manager,
                rng,
                session,
                ..
            } = self;
            let Some(b) = session.beams.get_mut(&h) else {
                continue;
            };
            let step = step_beam(manager, rng, b, ends, now, frame, env);
            let drawn = b.ribbon.is_some();
            if step.arrived_now {
                session.arrived.push(BeamHandle(h));
                session.beam_arrivals += 1;
            }
            if step.removed {
                session.finished.push(BeamHandle(h));
            }
            // Materials still loading (a load beyond the in-flight cap is asked again) or
            // landed since the last frame: their textures are acquired before the draw.
            if drawn {
                self.load_ribbon_materials(materials);
            }
        }
    }

    /// The effect-manager owners of the session figures and beams (for draw counts).
    pub(super) fn session_systems(&self) -> HashSet<SystemKey> {
        let mut out = HashSet::new();
        let owners = self
            .session
            .by_runtime
            .keys()
            .map(|e| e.0)
            .chain(self.session.beams.values().map(|b| b.owner));
        for o in owners {
            if let Some(c) = self.manager.component(o) {
                out.extend(c.attachments().iter().map(|a| a.system));
            }
        }
        out
    }

    /// Inserts a session owner.
    pub(super) fn add_session_owner(&mut self, id: EntityId, net: u64, view: &FigureEffectView) {
        self.owners.insert(
            id,
            LiveOwner {
                kind: OwnerKind::Session { net },
                iid: None,
                points: view.points.clone(),
                particle_scale: 1.0,
            },
        );
        self.session.views.insert(net, view.clone());
        self.session.by_net.insert(net, id);
        self.session.by_runtime.insert(id, net);
    }

    /// Removes a session owner from the state (the runtime side is the caller's).
    pub(super) fn remove_session_owner(&mut self, net: u64) -> Option<EntityId> {
        let id = self.session.by_net.remove(&net)?;
        self.session.by_runtime.remove(&id);
        self.session.views.remove(&net);
        self.owners.remove(&id);
        let now = self.now;
        self.manager.remove_owner(id.0, now);
        self.handles.retain(|_, o| *o != id);
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::gameplay::particles::{Curve, CurveVec, Emitter, ParticleSystem};
    use tessera_particles::{Effect, SystemSim};

    fn system(lifespan: f32) -> ParticleSystem {
        ParticleSystem {
            time_step: 0.1,
            fast_forward: 0.0,
            physics_duration: 0.0,
            world_space: true,
            force_draw: false,
            uses_video_post: false,
            draw_reflection: false,
            use_lighting: false,
            inherit_opacity: false,
            far_fade_start: 40.0,
            far_fade_end: 60.0,
            near_fade_start: 0.0,
            near_fade_end: 0.0,
            scale_mode: 1,
            emitters: vec![Emitter {
                emit_interval: 0.5,
                speed: Curve::constant(0.0),
                direction: CurveVec::constant([0.0, 0.0, 1.0]),
                lifespan: Curve::constant(lifespan),
                active: true,
                ..Emitter::default()
            }],
            has_bounds: false,
            bounds_min: [0.0; 3],
            bounds_max: [0.0; 3],
            sorting_priority: 0,
            shareable: false,
            surface: tessera_formats::Did(0x3100_0001),
            extra_surface: tessera_formats::Did(0),
        }
    }

    /// `particle-runtime.md` §11.1 steps 8 and 10: a (0, 1, 0) beam's particles keep ageing
    /// after its envelope is done; the beam is removed once they have died.
    #[test]
    fn a_finished_beam_is_removed_once_its_particles_die() {
        let node = BeamNode {
            color: [255; 4],
            sustain: 1.0,
            beam_particles_per_second: 10,
            segment_size: 1.0,
            head_start: 1.0,
            beam_particle_system: tessera_formats::Did(0x3900_0001),
            ..BeamNode::default()
        };
        let mut rng = Ran2::new(3);
        let mut manager = EffectManager::new();
        let owner = BEAM_OWNER_BASE | 1;
        let desc = Arc::new(system(1.5));
        let key = manager
            .start_attached(
                owner,
                0x3900_0001,
                0.0,
                || Some(Effect::Particles(SystemSim::new(Arc::clone(&desc), 0.0))),
                AttachSpec::default(),
            )
            .map(|(_, k)| k);
        assert!(key.is_some());
        let mut b = BeamRun {
            beam: Beam::new(&node, 0.0, &mut rng),
            source: EntityId(1),
            target: Some(EntityId(2)),
            owner,
            systems: [key, None],
            last: 0.0,
            finished: false,
            frame: None,
            ribbon: None,
            head: None,
            head_scale: Vec3::ONE,
        };
        let ends = BeamEnds {
            source: Some(Vec3::ZERO),
            source_forward: Vec3::Y,
            target: Some(Vec3::new(0.0, 10.0, 0.0)),
        };
        let env = Environment::default();
        let live = |m: &EffectManager| {
            key.and_then(|k| m.effect(k))
                .and_then(|e| e.particles())
                .map_or(0, SystemSim::live_particles)
        };
        let mut removed_at = None;
        let mut live_after_envelope = 0;
        for f in 1..=300u64 {
            #[allow(clippy::cast_precision_loss)]
            let now = f as f64 / 50.0;
            let step = step_beam(&mut manager, &mut rng, &mut b, ends, now, f, &env);
            if (1.05..1.1).contains(&now) {
                live_after_envelope = live(&manager);
            }
            if step.removed {
                removed_at = Some(now);
                break;
            }
        }
        assert!(live_after_envelope > 0, "particles outlive the envelope");
        let t = removed_at.expect("the beam is removed");
        assert!(
            (2.4..3.0).contains(&t),
            "removed after the last lifespan: {t}"
        );
        assert!(b.finished);
        assert!(
            manager
                .component(owner)
                .is_none_or(|c| c.attachments().is_empty())
        );
    }

    /// `particle-runtime.md` §11.1 step 3 with §6.1: a bone type's offset is in the bone's
    /// space, a holding location's is added to its frame's translation (not rotated by it),
    /// no point's is in the model's space; all then go through the owner's world frame.
    #[test]
    fn beam_end_offsets_follow_the_attachment_rules() {
        use super::super::data::AttachPoints;
        let turn = Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let points = AttachPoints {
            holding: vec![(
                7,
                Some(0),
                Mat4::from_rotation_translation(
                    Quat::from_rotation_z(std::f32::consts::PI),
                    Vec3::new(0.0, 0.0, 1.0),
                ),
            )],
            bones: vec![(3, 0)],
            reference: Vec::new(),
        };
        // Bone 0 one metre up, turned by a quarter turn about Z.
        let model = [Mat4::from_translation(Vec3::Z) * turn];
        let f = OwnerFrame {
            world: Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)),
            scale: Vec3::ONE,
            model: &model,
            points: Some(&points),
            drawn: true,
        };
        let off = [1.0, 0.0, 0.0];
        // Bone type: the offset turns with the bone.
        let p = attach_point(&f, 0, 3, off, false);
        assert!((p - Vec3::new(10.0, 1.0, 1.0)).length() < 1e-5, "{p:?}");
        // Holding location: frame translation + offset, then the bone.
        let p = attach_point(&f, 7, 0, off, false);
        assert!((p - Vec3::new(10.0, 1.0, 2.0)).length() < 1e-5, "{p:?}");
        // Neither: the offset in the model's space, through the owner's turn.
        let turned = OwnerFrame {
            world: Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)) * turn,
            ..f
        };
        let p = attach_point(&turned, 0, 0, off, false);
        assert!((p - Vec3::new(10.0, 1.0, 0.0)).length() < 1e-5, "{p:?}");
        // A point the owner lacks, or the box centre, is the box centre with the offset.
        let p = attach_point(&f, 0, 99, off, false);
        assert!(
            (p - Vec3::new(11.0, 0.0, BOX_CENTRE_HEIGHT)).length() < 1e-5,
            "{p:?}"
        );
        let p = attach_point(&f, 7, 0, [f32::NAN; 3], true);
        assert!(
            (p - Vec3::new(10.0, 0.0, BOX_CENTRE_HEIGHT)).length() < 1e-5,
            "{p:?}"
        );
    }

    /// §11.1 steps 6 and 9: while the beam runs, its ribbon span and the head entity's
    /// place are kept for the draw; once removed, neither.
    #[test]
    fn a_running_beam_keeps_its_ribbon_and_head_for_the_draw() {
        let node = BeamNode {
            color: [255; 4],
            width: 0.2,
            sustain: 0.5,
            segment_size: 1.0,
            head_speed: 20.0,
            body_material: tessera_formats::Did(0x3100_0001),
            head_entity: tessera_formats::Did(0x4700_0001),
            ..BeamNode::default()
        };
        let mut rng = Ran2::new(5);
        let mut manager = EffectManager::new();
        let mut b = BeamRun {
            beam: Beam::new(&node, 0.0, &mut rng),
            source: EntityId(1),
            target: Some(EntityId(2)),
            owner: BEAM_OWNER_BASE | 2,
            systems: [None, None],
            last: 0.0,
            finished: false,
            frame: None,
            ribbon: None,
            head: None,
            head_scale: Vec3::ONE,
        };
        let ends = BeamEnds {
            source: Some(Vec3::ZERO),
            source_forward: Vec3::Y,
            target: Some(Vec3::new(10.0, 0.0, 0.0)),
        };
        let env = Environment::default();
        let step = step_beam(&mut manager, &mut rng, &mut b, ends, 0.25, 1, &env);
        assert!(!step.removed);
        let span = b.ribbon.expect("the ribbon is drawn");
        assert!((span.head.x - 5.0).abs() < 1e-3, "{span:?}");
        let (at, rot) = b.head.expect("the head entity is shown");
        assert!((at - span.head).length() < 1e-5);
        assert!(
            (rot * Vec3::Y - Vec3::X).length() < 1e-5,
            "it looks along the beam"
        );
        let mut removed = false;
        for f in 2..200u64 {
            #[allow(clippy::cast_precision_loss)]
            let now = 0.25 + f as f64 / 50.0;
            if step_beam(&mut manager, &mut rng, &mut b, ends, now, f, &env).removed {
                removed = true;
                break;
            }
        }
        assert!(removed, "the beam ends");
        assert!(b.ribbon.is_none() && b.head.is_none());
    }
}
