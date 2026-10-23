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
//! A looping animation node whose clip is still being decoded on the workers (the frame
//! thread reads no clip, ARCHITECTURE §21) runs until stopped and its clip starts once
//! decoded ([`DeferredClips`]), as on the character layer's live figures; a one-shot node,
//! or one whose clip failed, completes at once.
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
/// Most looping animation nodes of session figures waiting for their clip to be decoded
/// (further ones complete at once without a clip).
pub const MAX_DEFERRED_CLIPS: usize = 1024;
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
    /// Looping animation nodes whose clip was still being decoded when they started (their
    /// clip starts once decoded, [`FxState::retry_session_clips`]).
    pub deferred: DeferredClips,
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

/// What a deferred node's clip is now ([`retry_deferred`]).
#[derive(Debug, Clone)]
pub(super) enum ClipLookup<C> {
    /// Decoded and bound to the figure's skeleton.
    Ready(C),
    /// Still being decoded on the workers.
    Decoding,
    /// Failed for good, does not fit the skeleton, or the figure is gone.
    Gone,
}

/// Looping animation nodes of session figures whose clip was still being decoded when the
/// node started, by sink handle (the way the character layer's live figures wait for
/// theirs): the node runs until stopped, and its clip starts under the same handle once
/// decoded. Bounded by [`MAX_DEFERRED_CLIPS`].
#[derive(Debug, Default)]
pub(super) struct DeferredClips {
    waiting: HashMap<u64, (u64, AnimationNode, f32)>,
    /// Clip DIDs of nodes deferred since the last [`Self::take_prefetch`], each once (at most
    /// [`MAX_DEFERRED_CLIPS`]): the walker asks the character layer to decode them, so a
    /// waiting node does not depend on a frame-thread miss having queued its clip.
    prefetch: Vec<u32>,
    /// Nodes deferred so far.
    pub deferred: u64,
}

impl DeferredClips {
    /// Defers `node` (intensity `intensity`) of session figure `net` under `handle`; `false`
    /// when [`MAX_DEFERRED_CLIPS`] are waiting already (the node then completes at once).
    pub fn defer(&mut self, handle: u64, net: u64, node: &AnimationNode, intensity: f32) -> bool {
        if self.waiting.len() >= MAX_DEFERRED_CLIPS {
            return false;
        }
        self.waiting.insert(handle, (net, *node, intensity));
        self.deferred += 1;
        let did = node.asset.0;
        if self.prefetch.len() < MAX_DEFERRED_CLIPS && !self.prefetch.contains(&did) {
            self.prefetch.push(did);
        }
        true
    }

    /// The clip DIDs to decode for the nodes deferred since the last call (each once).
    pub fn take_prefetch(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.prefetch)
    }

    /// The node of `handle` stopped: `true` when it was still waiting (its clip never
    /// started, so there is nothing to stop).
    pub fn stop(&mut self, handle: u64) -> bool {
        self.waiting.remove(&handle).is_some()
    }

    /// A driven field of a waiting node changed (§6.10 step 7): its clip will start with
    /// the new values (the clip asset kept). `true` when `handle` was waiting.
    pub fn update(&mut self, handle: u64, node: &AnimationNode) -> bool {
        let Some(w) = self.waiting.get_mut(&handle) else {
            return false;
        };
        w.1 = AnimationNode {
            asset: w.1.asset,
            ..*node
        };
        true
    }

    /// Session figure `net` left: its waiting nodes are dropped.
    pub fn forget(&mut self, net: u64) {
        self.waiting.retain(|_, w| w.0 != net);
    }

    /// Nodes waiting.
    pub fn len(&self) -> usize {
        self.waiting.len()
    }

    /// Whether no node waits.
    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }
}

/// One pass over the waiting nodes of `deferred`, in start order (handles grow): `lookup`
/// tells what a node's clip is now (by figure and clip DID); a ready clip is handed to `play`
/// with the node's handle, figure, node and intensity, and the node stops waiting when
/// `play` queued it (`false`: the command list is full, tried again next pass); a clip that
/// is gone drops the node (it keeps running without a clip); a decoding one keeps it.
pub(super) fn retry_deferred<C>(
    deferred: &mut DeferredClips,
    mut lookup: impl FnMut(u64, u32) -> ClipLookup<C>,
    mut play: impl FnMut(u64, u64, &AnimationNode, f32, C) -> bool,
) {
    let mut waiting: Vec<(u64, (u64, AnimationNode, f32))> =
        deferred.waiting.iter().map(|(h, w)| (*h, *w)).collect();
    waiting.sort_unstable_by_key(|(h, _)| *h);
    for (handle, (net, node, intensity)) in waiting {
        match lookup(net, node.asset.0) {
            ClipLookup::Ready(clip) => {
                if play(handle, net, &node, intensity, clip) {
                    deferred.waiting.remove(&handle);
                }
            }
            ClipLookup::Gone => {
                deferred.waiting.remove(&handle);
            }
            ClipLookup::Decoding => {}
        }
    }
}

/// The decoded clips session figures bind their animation nodes' clips from (the character
/// layer's [`FigureContext`] clip store; a stand-in in tests).
pub(super) trait SessionClipSource {
    /// Whether clip `did` is in the store (decoded, or failed for good): binding it then
    /// reads nothing on the frame thread.
    fn decoded(&self, did: u32) -> bool;
    /// Clip `did` bound to `skeleton`. On the frame thread a clip not decoded yet is not
    /// read: the call fails, counts a frame-thread miss and queues the clip for the workers.
    fn bind(
        &self,
        did: u32,
        skeleton: &Arc<tessera_anim::Skeleton>,
    ) -> Result<Arc<tessera_anim::BoundClip>, String>;
}

impl SessionClipSource for FigureContext {
    fn decoded(&self, did: u32) -> bool {
        self.clips.has(did)
    }

    fn bind(
        &self,
        did: u32,
        skeleton: &Arc<tessera_anim::Skeleton>,
    ) -> Result<Arc<tessera_anim::BoundClip>, String> {
        self.clips.bind(&self.set, did, skeleton)
    }
}

/// Binds clip `did` to `skeleton`: [`ClipLookup::Ready`] when it binds,
/// [`ClipLookup::Gone`] when it is in the store and still does not bind (failed for good,
/// or does not fit the skeleton), [`ClipLookup::Decoding`] when it is not in the store. A
/// failed bind followed by a clip found in the store is bound once more: a worker may have
/// stored the clip between the two calls (it then binds without a read).
fn bind_clip<S: SessionClipSource + ?Sized>(
    src: &S,
    did: u32,
    skeleton: &Arc<tessera_anim::Skeleton>,
) -> ClipLookup<Arc<tessera_anim::BoundClip>> {
    match src.bind(did, skeleton) {
        Ok(c) => ClipLookup::Ready(c),
        Err(_) if src.decoded(did) => match src.bind(did, skeleton) {
            Ok(c) => ClipLookup::Ready(c),
            Err(_) => ClipLookup::Gone,
        },
        Err(_) => ClipLookup::Decoding,
    }
}

impl SessionState {
    /// An animation node on a session figure (`script-table.md` §6.10): the clip bound to the
    /// figure's skeleton and its Play command queued for the character layer. A looping
    /// node whose clip is not in the store yet is deferred without binding (no read and no
    /// frame-thread miss): it runs until stopped, its clip is asked for once
    /// ([`DeferredClips::take_prefetch`]) and starts once decoded ([`Self::retry_clips`]).
    /// A one-shot node whose clip is not decoded, or a clip that failed, completes at once.
    pub(super) fn start_animation<S: SessionClipSource + ?Sized>(
        &mut self,
        rng: &mut Ran2,
        src: &S,
        entity: EntityId,
        node: &AnimationNode,
        intensity: f32,
    ) -> AnimationOutcome {
        let Some(&net) = self.by_runtime.get(&entity) else {
            return AnimationOutcome::CompleteAtOnce;
        };
        let Some(skel) = self.views.get(&net).and_then(|v| v.skeleton.clone()) else {
            return AnimationOutcome::CompleteAtOnce;
        };
        if node.asset.is_null() || self.clips.len() >= MAX_CLIP_COMMANDS {
            return AnimationOutcome::CompleteAtOnce;
        }
        let did = node.asset.0;
        if node.loop_forever && !src.decoded(did) && self.deferred.len() < MAX_DEFERRED_CLIPS {
            let handle = self.next_session_clip();
            if self.deferred.defer(handle, net, node, intensity) {
                return AnimationOutcome::RunUntilStopped(AnimationHandle(handle));
            }
        }
        let ClipLookup::Ready(clip) = bind_clip(src, did, &skel) else {
            return AnimationOutcome::CompleteAtOnce;
        };
        let handle = self.next_session_clip();
        match self.play_clip(rng, net, handle, node, intensity, clip) {
            Some(ClipRun::For(t)) => AnimationOutcome::RunFor(AnimationHandle(handle), t),
            Some(ClipRun::UntilStopped) => {
                AnimationOutcome::RunUntilStopped(AnimationHandle(handle))
            }
            Some(ClipRun::Refused) | None => AnimationOutcome::CompleteAtOnce,
        }
    }

    /// A new clip handle of the session sink.
    fn next_session_clip(&mut self) -> u64 {
        let handle = self.next_clip;
        self.next_clip = self.next_clip.wrapping_add(1);
        handle
    }

    /// Queues the Play command of `node`'s bound `clip` on session figure `net` under
    /// `handle` (`script-table.md` §6.10 step 1: the start fraction, drawn from `rng` when
    /// the node asks for a random one). `None` when the command list is full (nothing
    /// queued).
    fn play_clip(
        &mut self,
        rng: &mut Ran2,
        net: u64,
        handle: u64,
        node: &AnimationNode,
        intensity: f32,
        clip: Arc<tessera_anim::BoundClip>,
    ) -> Option<ClipRun> {
        if self.clips.len() >= MAX_CLIP_COMMANDS {
            return None;
        }
        let start = if node.randomize_start_time {
            rng.range(node.start_time, node.end_time)
        } else {
            node.start_time
        };
        let request = ClipRequest {
            handle,
            clip,
            node: *node,
            intensity,
            start: if start.is_finite() { start } else { 0.0 },
        };
        let run = request.run();
        if run == ClipRun::Refused {
            return Some(run);
        }
        self.clips_started += 1;
        self.clips.push(ClipCommand::Play {
            entity: tessera_net::EntityId(net),
            request,
        });
        Some(run)
    }

    /// Starts the deferred looping clips whose clip has entered the store since; drops
    /// those whose clip failed for good or whose figure left (their nodes keep running
    /// without a clip). A clip not in the store yet is not bound (no frame-thread miss per
    /// frame while it decodes).
    pub(super) fn retry_clips<S: SessionClipSource + ?Sized>(&mut self, rng: &mut Ran2, src: &S) {
        if self.deferred.is_empty() {
            return;
        }
        let mut deferred = std::mem::take(&mut self.deferred);
        // The skeletons of the figures with waiting nodes (the play step below changes
        // the state, so the lookup reads its own copy).
        let skeletons: HashMap<u64, _> = deferred
            .waiting
            .values()
            .filter(|w| self.by_net.contains_key(&w.0))
            .filter_map(|w| {
                let s = self.views.get(&w.0)?.skeleton.clone()?;
                Some((w.0, s))
            })
            .collect();
        retry_deferred(
            &mut deferred,
            |net, did| {
                // The figure left (or lost its skeleton): nothing to play on.
                let Some(skel) = skeletons.get(&net) else {
                    return ClipLookup::Gone;
                };
                if !src.decoded(did) {
                    return ClipLookup::Decoding;
                }
                bind_clip(src, did, skel)
            },
            |handle, net, node, intensity, clip| {
                self.play_clip(rng, net, handle, node, intensity, clip)
                    .is_some()
            },
        );
        self.deferred = deferred;
    }
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
    /// figure's skeleton, queued for the character layer ([`SessionState::start_animation`]).
    pub(super) fn session_animation(
        &mut self,
        entity: EntityId,
        node: &AnimationNode,
        intensity: f32,
    ) -> AnimationOutcome {
        let Some(ctx) = self.session.clip_ctx.clone() else {
            return AnimationOutcome::CompleteAtOnce;
        };
        self.session
            .start_animation(&mut self.rng, &*ctx, entity, node, intensity)
    }

    /// Starts the deferred looping clips of session figures whose clip has been decoded
    /// since ([`SessionState::retry_clips`]). Once per frame, before the runtime steps.
    pub(super) fn retry_session_clips(&mut self) {
        if self.session.deferred.is_empty() {
            return;
        }
        let Some(ctx) = self.session.clip_ctx.clone() else {
            return;
        };
        self.session.retry_clips(&mut self.rng, &*ctx);
    }

    /// A stopped animation node (§6.10 step 8). Stops are never dropped (a lost stop would
    /// leave its clip playing); there is at most one per started clip, and Play commands are
    /// bounded by [`MAX_CLIP_COMMANDS`].
    pub(super) fn session_animation_stop(&mut self, handle: AnimationHandle) {
        if self.session.deferred.stop(handle.0) {
            // Its clip never started: nothing to stop.
            return;
        }
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
        if self.session.deferred.update(handle.0, node) {
            // Not playing yet: the clip starts with the current driven values.
            return;
        }
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
    /// `lights` gives the light list of a point (simulation frame, rule 13): each beam's
    /// systems get the list at its source.
    pub(super) fn update_beams(
        &mut self,
        frame: u64,
        env: &Environment,
        lights: &dyn Fn(Vec3) -> tessera_particles::LightList,
    ) {
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
            if let Some(at) = ends.source.or(b.frame.map(|f| f.1)) {
                manager.set_owner_lights(b.owner, &lights(at));
            }
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
                placed: None,
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
        self.session.deferred.forget(net);
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
            opacity: 1.0,
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

    fn looping(clip: u32) -> AnimationNode {
        AnimationNode {
            asset: Did(clip),
            loop_forever: true,
            speed: 1.0,
            speed_multiplier: 1.0,
            end_time: 1.0,
            ..AnimationNode::default()
        }
    }

    /// A looping node whose clip is still decoding waits under its handle and starts, in
    /// start order and with the node's latest driven values, once the clip is ready; a full
    /// command list keeps it waiting; a stop, a failed clip or a departed figure drops it.
    #[test]
    fn deferred_session_clips_start_once_decoded() {
        let mut d = DeferredClips::default();
        assert!(d.defer(7, 100, &looping(0x0500_0001), 1.0));
        assert!(d.defer(3, 100, &looping(0x0500_0002), 0.5));
        assert!(d.defer(9, 200, &looping(0x0500_0003), 1.0));
        assert!(d.defer(11, 200, &looping(0x0500_0004), 1.0));
        assert_eq!((d.len(), d.deferred), (4, 4));
        // A driven change while waiting: the clip asset is kept.
        let mut driven = looping(0x0500_9999);
        driven.speed = 2.0;
        assert!(d.update(7, &driven));
        assert!(!d.update(42, &driven));

        // Nothing decoded yet: all keep waiting.
        let mut played = Vec::new();
        retry_deferred(
            &mut d,
            |_, _| ClipLookup::<()>::Decoding,
            |h, _, _, _, ()| {
                played.push(h);
                true
            },
        );
        assert!(played.is_empty());
        assert_eq!(d.len(), 4);

        // Clips 1 and 2 decode; the command list takes only one this pass.
        let ready = |_: u64, did: u32| match did {
            0x0500_0001 | 0x0500_0002 => ClipLookup::Ready(did),
            0x0500_0003 => ClipLookup::Gone,
            _ => ClipLookup::Decoding,
        };
        let mut room = 1;
        let mut got = Vec::new();
        retry_deferred(&mut d, ready, |h, net, node, _, clip| {
            if room == 0 {
                return false;
            }
            room -= 1;
            got.push((h, net, node.asset.0, node.speed, clip));
            true
        });
        // Handle 3 first (start order); 9's clip failed: dropped; 7 waits for room.
        assert_eq!(got, vec![(3, 100, 0x0500_0002, 1.0, 0x0500_0002)]);
        assert_eq!(d.len(), 2);
        got.clear();
        retry_deferred(&mut d, ready, |h, net, node, _, clip| {
            got.push((h, net, node.asset.0, node.speed, clip));
            true
        });
        assert_eq!(got, vec![(7, 100, 0x0500_0001, 2.0, 0x0500_0001)]);
        // Handle 11 still decoding; its node stops: nothing to stop on the figure.
        assert!(d.stop(11));
        assert!(!d.stop(7));
        assert!(d.is_empty());

        // A departed figure's waiting nodes are dropped.
        assert!(d.defer(20, 300, &looping(0x0500_0005), 1.0));
        assert!(d.defer(21, 301, &looping(0x0500_0005), 1.0));
        d.forget(300);
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn deferred_session_clips_are_bounded() {
        let mut d = DeferredClips::default();
        for h in 0..MAX_DEFERRED_CLIPS as u64 {
            assert!(d.defer(h, 1, &looping(0x0500_0001), 1.0));
        }
        assert!(!d.defer(u64::MAX, 1, &looping(0x0500_0001), 1.0));
        assert_eq!(d.len(), MAX_DEFERRED_CLIPS);
    }

    /// A stand-in clip store: clips in `decoded` bind; a bind of any other counts a
    /// frame-thread miss, and `arrive_on_miss` stores that clip right after the miss (a
    /// worker finishing between the miss and the store check).
    struct FakeClips {
        decoded: std::cell::RefCell<HashMap<u32, Option<Arc<tessera_anim::BoundClip>>>>,
        misses: std::cell::Cell<u32>,
        arrive_on_miss: Option<(u32, Arc<tessera_anim::BoundClip>)>,
    }

    impl SessionClipSource for FakeClips {
        fn decoded(&self, did: u32) -> bool {
            self.decoded.borrow().contains_key(&did)
        }

        fn bind(
            &self,
            did: u32,
            _: &Arc<tessera_anim::Skeleton>,
        ) -> Result<Arc<tessera_anim::BoundClip>, String> {
            if let Some(c) = self.decoded.borrow().get(&did) {
                return c.clone().ok_or_else(|| "failed".to_string());
            }
            self.misses.set(self.misses.get() + 1);
            if let Some((d, c)) = &self.arrive_on_miss
                && *d == did
            {
                self.decoded.borrow_mut().insert(did, Some(Arc::clone(c)));
            }
            Err("not decoded yet".into())
        }
    }

    fn test_skeleton() -> Arc<tessera_anim::Skeleton> {
        Arc::new(
            tessera_anim::Skeleton::from_parts(&[-1], &[tessera_havok::Transform::IDENTITY])
                .unwrap(),
        )
    }

    fn test_clip(s: &Arc<tessera_anim::Skeleton>) -> Arc<tessera_anim::BoundClip> {
        use tessera_havok::testkit::{
            AnimationKindSpec, AnimationSpec, ClipRecordSpec, InterleavedClipSpec,
        };
        let spec = InterleavedClipSpec {
            duration: 1.0,
            tracks: 1,
            transforms: vec![tessera_havok::Transform::IDENTITY; 2],
        };
        let rec = ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Interleaved(spec)))
            .to_bytes();
        let c = tessera_havok::Clip::parse(&rec).unwrap();
        Arc::new(tessera_anim::BoundClip::bind(Arc::new(c), Arc::clone(s)).unwrap())
    }

    /// A session state with one session figure (network ID 100) with a skeleton.
    fn session_with_figure(skel: &Arc<tessera_anim::Skeleton>) -> (SessionState, EntityId) {
        let mut st = SessionState::default();
        let id = EntityId(SESSION_OWNER_BASE);
        st.by_net.insert(100, id);
        st.by_runtime.insert(id, 100);
        st.views.insert(
            100,
            FigureEffectView {
                region: 1,
                position: [0.0; 3],
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                drawn: true,
                model: vec![Mat4::IDENTITY],
                points: None,
                skeleton: Some(Arc::clone(skel)),
                scripts: None,
                opacity: 1.0,
            },
        );
        (st, id)
    }

    fn plays(st: &SessionState) -> Vec<u64> {
        st.clips
            .iter()
            .filter_map(|c| match c {
                ClipCommand::Play { request, .. } => Some(request.handle),
                _ => None,
            })
            .collect()
    }

    /// A looping node on a session figure whose clip is not decoded yet runs until stopped
    /// without binding (no frame-thread miss, however many frames it waits), asks for its
    /// clip once, and its Play command is queued under the node's handle once the clip
    /// enters the store.
    #[test]
    fn a_waiting_looping_node_plays_once_its_clip_is_stored() {
        let skel = test_skeleton();
        let clip = test_clip(&skel);
        let (mut st, id) = session_with_figure(&skel);
        let src = FakeClips {
            decoded: std::cell::RefCell::new(HashMap::new()),
            misses: std::cell::Cell::new(0),
            arrive_on_miss: None,
        };
        let mut rng = Ran2::new(1);
        let did = 0x0500_0001;
        let AnimationOutcome::RunUntilStopped(handle) =
            st.start_animation(&mut rng, &src, id, &looping(did), 1.0)
        else {
            panic!("a waiting looping node runs until stopped");
        };
        assert_eq!(st.deferred.len(), 1);
        assert_eq!(
            st.deferred.take_prefetch(),
            vec![did],
            "its clip is asked for once"
        );
        assert!(st.deferred.take_prefetch().is_empty());
        for _ in 0..30 {
            st.retry_clips(&mut rng, &src);
        }
        assert_eq!(src.misses.get(), 0, "no frame-thread miss while it waits");
        assert!(st.clips.is_empty());
        // The workers store the clip: the next pass queues its Play command.
        src.decoded
            .borrow_mut()
            .insert(did, Some(Arc::clone(&clip)));
        st.retry_clips(&mut rng, &src);
        assert_eq!(plays(&st), vec![handle.0]);
        assert!(st.deferred.is_empty());
        assert_eq!(st.clips_started, 1);
        assert_eq!(src.misses.get(), 0);

        // A clip that failed for good drops the waiting node (it plays nothing).
        let bad = 0x0500_0002;
        assert!(matches!(
            st.start_animation(&mut rng, &src, id, &looping(bad), 1.0),
            AnimationOutcome::RunUntilStopped(_)
        ));
        src.decoded.borrow_mut().insert(bad, None);
        st.retry_clips(&mut rng, &src);
        assert!(st.deferred.is_empty());
        assert_eq!(plays(&st), vec![handle.0]);
    }

    /// A clip a worker stores between the miss and the store check binds on the second
    /// try: a one-shot node plays instead of completing at once.
    #[test]
    fn a_clip_stored_during_the_miss_still_plays() {
        let skel = test_skeleton();
        let clip = test_clip(&skel);
        let (mut st, id) = session_with_figure(&skel);
        let did = 0x0500_0003;
        let src = FakeClips {
            decoded: std::cell::RefCell::new(HashMap::new()),
            misses: std::cell::Cell::new(0),
            arrive_on_miss: Some((did, Arc::clone(&clip))),
        };
        let one_shot = AnimationNode {
            loop_forever: false,
            loop_count: 1,
            ..looping(did)
        };
        let mut rng = Ran2::new(1);
        let out = st.start_animation(&mut rng, &src, id, &one_shot, 1.0);
        assert!(matches!(out, AnimationOutcome::RunFor(..)), "{out:?}");
        assert_eq!(plays(&st).len(), 1);
        assert_eq!(src.misses.get(), 1);
        // A one-shot whose clip is not decoded completes at once (and was asked for).
        let other = AnimationNode {
            asset: Did(0x0500_0004),
            ..one_shot
        };
        assert!(matches!(
            st.start_animation(&mut rng, &src, id, &other, 1.0),
            AnimationOutcome::CompleteAtOnce
        ));
        assert!(st.deferred.is_empty());
    }
}
