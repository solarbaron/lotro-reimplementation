// SPDX-License-Identifier: GPL-3.0-or-later
//! The mounted player in the character layer (`walker.md` §16.7; `mounts.md` §14.5, §14.8):
//! the mount figure, built through the shared figure path from the MeshFX node's entity and
//! drawn at the rider's holding location, and the clips rider and mount play for the
//! current state key.
//!
//! [`MountedAnim`] plays one entity's clips of a key on an animator: one animator layer per
//! data layer, in data-layer order (`script-table.md` §6.2); a changed clip starts at phase
//! 0 with its `RampIn` (`mounts.md` §14.5 items 4–6), a data layer no longer listed fades
//! out over its `RampOut` (a one-shot clip, such as the mounting clip, first plays to its end
//! unless another clip takes its layer, §14.7), the rates are the nodes' (no speed blend, no
//! rate matching). The figure's held proportion clips (`script-table.md` §6.8) keep playing
//! on their layers.
//!
//! **Switching.** Entering ([`MountedAnim::entering`]) keeps a copy of the on-foot animator's
//! layers below the mounted ones and fades them out while the mounted clips ramp in; leaving
//! ([`MountedAnim::leave`]) puts a copy of the on-foot layers on top, fading in while the
//! mounted clips ramp out (`mounts.md` §14.7: the on-foot idle starts with its own ramp), and
//! the character layer returns to the on-foot animator once the fade is over. No pose jumps
//! between the seated and the standing figure.
//!
//! **Binding** ([`ClipBinder`]): clips are read, decoded, bound and baked on the asset
//! workers (`walker.md` §16.7); while any clip of a new key is still binding the previous
//! clips keep playing, and a clip that cannot be bound is remembered and left out.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use tessera_anim::{Animator, BoundClip, Playback, Skeleton, Source, Wrap};
use tessera_assets::{CancelToken, Priority, WorkerPool};
use tessera_formats::Did;
use tessera_formats::gameplay::mounts::MountMesh;

use tessera_render::glam::{Mat4, Quat, Vec3};

use super::figure::{ClipPlay, FigureContext, FigureRequest, PropAnchorRule};
use crate::walker::effects::data::AttachPoints;
use crate::walker::mount::eval::ShownClip;

/// The fade of a switch between the on-foot and the mounted animator when the clips give no
/// ramp, seconds (the longest `RampIn` / `RampOut` of the mounted nodes, `mounts.md` §14.5).
pub const SWITCH_FADE: f32 = 0.25;
/// Most (clip, skeleton) bindings the binder remembers.
pub const MAX_BINDINGS: usize = 256;

/// What the walker asks the character layer to show for a mounted player.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerMountView {
    /// The mount figure (the MeshFX node's entity); `None`: nothing drawn.
    pub figure: Option<FigureRequest>,
    /// The node's placement values.
    pub mesh: MountMesh,
    /// Where the node anchors the mount on the rider (`script-table.md` §5.2 "Where a MeshFX
    /// node attaches its entity"; [`crate::walker::mount::eval::MeshChoice::anchor`]): the
    /// placement follows it ([`mount_model`]), as the refusal check does
    /// ([`crate::walker::mount::eval::placeable_anchor`]).
    pub anchor: PropAnchorRule,
    /// The node's `AttachToParent` ([`crate::walker::mount::eval::MeshChoice::attached`]):
    /// `false` keeps the mount at the world frame of its placement ([`HeldPlacement`]).
    pub attached: bool,
    /// The rider's final model scale per axis (for [`MountMesh::draw_scale`]).
    pub rider_model_scale: [f32; 3],
    /// The mount's opacity, 0–1.
    pub alpha: f32,
    /// The mount is drawn (appearing, shown or fading out).
    pub drawn: bool,
    /// The rider's clips of the current key; `None`: the rider keeps its on-foot clips.
    pub rider_clips: Option<Vec<ShownClip>>,
    /// The mount's clips of the current key.
    pub mount_clips: Vec<ShownClip>,
    /// The rider's shown clip of the key (`walker.md` §16.10), for the phase rule.
    pub rider_shown: Option<ShownClip>,
    /// The mount's shown clip of the key.
    pub mount_shown: Option<ShownClip>,
    /// The key moves backwards (`Slider_Propel` < 0): the mount's phase is the rider's
    /// halved (`mounts.md` §14.5 item 6).
    pub back: bool,
}

/// The mount's model transform relative to the rider's feet (`mounts.md` §14.8 item 1, the
/// Decision of §4.4): the rider's anchor of `view` ([`PlayerMountView::anchor`],
/// `script-table.md` §5.2, [`crate::walker::mount::eval::anchor_matrix`]) with the rider's
/// posed model matrices `model` (empty: the reference pose), composed by
/// [`crate::walker::mount::mount_transform`] with `rider` (the rider's model rotation and
/// final scale, no translation) and the node's `BoneOffset` and scale. `None` when the rider
/// lacks the anchor the node names (the node shows nothing).
///
/// For an attached node the walker calls this every frame with the rider's current pose (the
/// mount follows the anchor). For an unattached node (`script-table.md` §5.2 answer: the
/// entity takes the anchor's world frame once, at placement, and keeps it) the walker calls
/// it only at placement and holds the result ([`HeldPlacement`]). Every mount node of this
/// install is attached (`mounts.md` §4.4).
#[must_use]
pub fn mount_model(
    points: &AttachPoints,
    view: &PlayerMountView,
    model: &[Mat4],
    rider: Mat4,
) -> Option<Mat4> {
    let anchor = crate::walker::mount::eval::anchor_matrix(points, view.anchor, model)?;
    Some(crate::walker::mount::mount_transform(
        rider,
        anchor,
        &view.mesh,
        view.rider_model_scale,
    ))
}

/// The world placement of the mount of an unattached node, held from the frame it was placed
/// (`script-table.md` §5.2, the 2026-10-23 answer: an unattached node's entity takes the
/// anchor's world frame, times the placement frame, once at placement and keeps it; only the
/// props of attached nodes follow the anchor).
///
/// A placement is held while the same node ([`MountMesh::table`], [`MountMesh::node_id`] and
/// [`MountMesh::entity`]) stays drawn; a hidden mount, an attached node or another node
/// starts over, so the next unattached placement reads the anchor again.
#[derive(Debug, Clone)]
pub struct HeldPlacement<T> {
    held: Option<((Did, u32, Did), T)>,
}

impl<T> Default for HeldPlacement<T> {
    fn default() -> Self {
        Self { held: None }
    }
}

impl<T: Clone> HeldPlacement<T> {
    /// The placement of the mount of `view` this frame: `place()` (the placement from the
    /// rider's current anchor) for an attached node, every frame; for an unattached node the
    /// placement held from its first frame, `place()` only until one succeeds. `None` (and
    /// nothing held) when the mount is not drawn or `place()` finds no anchor.
    pub fn place(
        &mut self,
        view: &PlayerMountView,
        place: impl FnOnce() -> Option<T>,
    ) -> Option<T> {
        if !view.drawn {
            self.held = None;
            return None;
        }
        if view.attached {
            self.held = None;
            return place();
        }
        let key = (view.mesh.table, view.mesh.node_id, view.mesh.entity);
        if let Some((k, v)) = &self.held
            && *k == key
        {
            return Some(v.clone());
        }
        let v = place();
        self.held = v.clone().map(|v| (key, v));
        v
    }

    /// Forgets the held placement (the mount is hidden; its next placement reads the anchor
    /// again).
    pub fn clear(&mut self) {
        self.held = None;
    }

    /// Whether a placement is held.
    #[must_use]
    pub fn is_held(&self) -> bool {
        self.held.is_some()
    }
}

/// The rider's model matrix without translation (`mounts.md` §14.8 item 1): its final model
/// scale per axis and its rotation (a non-finite or zero rotation counts as none).
#[must_use]
pub fn rider_frame(scale: Vec3, rotation: [f32; 4]) -> Mat4 {
    let rot = Quat::from_array(rotation);
    let rot = if rot.is_finite() && rot.length_squared() > 0.0 {
        rot.normalize()
    } else {
        Quat::IDENTITY
    };
    Mat4::from_scale_rotation_translation(scale, rot, Vec3::ZERO)
}

/// The phase a mount clip starts at (`mounts.md` §14.5 item 6): the rider's phase `rider`
/// (0–1), halved for the back clips.
#[must_use]
pub fn matched_phase(rider: f32, back: bool) -> f32 {
    let r = if rider.is_finite() {
        rider.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if back { r * 0.5 } else { r }
}

/// The result of asking for a bound clip.
#[derive(Debug, Clone)]
pub enum Bind {
    /// Bound.
    Ready(Arc<BoundClip>),
    /// Being bound on a worker.
    Pending,
    /// It cannot be bound (missing, undecodable, or its tracks do not fit the skeleton).
    Failed,
}

enum BindState {
    Pending,
    Ready(Arc<BoundClip>),
    Failed,
}

type BindKey = (u32, usize);
type BindResult = (BindKey, Result<Arc<BoundClip>, String>);

/// Reports a binding job's result, or a failure when the job is dropped unrun.
struct BindGuard {
    key: BindKey,
    tx: Sender<BindResult>,
    sent: bool,
}

impl BindGuard {
    fn send(mut self, r: Result<Arc<BoundClip>, String>) {
        self.sent = true;
        let _ = self.tx.send((self.key, r));
    }
}

impl Drop for BindGuard {
    fn drop(&mut self) {
        if !self.sent {
            let _ = self.tx.send((self.key, Err("binding job dropped".into())));
        }
    }
}

/// Binds the mounted clips on the asset workers (module docs).
pub struct ClipBinder {
    ctx: Option<Arc<FigureContext>>,
    pool: WorkerPool,
    states: HashMap<BindKey, (BindState, Arc<Skeleton>)>,
    tx: Sender<BindResult>,
    rx: Receiver<BindResult>,
}

impl std::fmt::Debug for ClipBinder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClipBinder")
            .field("bindings", &self.states.len())
            .finish_non_exhaustive()
    }
}

impl ClipBinder {
    /// A binder over `ctx`'s clip store whose jobs run on `pool` (`None`: nothing binds).
    #[must_use]
    pub fn new(ctx: Option<Arc<FigureContext>>, pool: &WorkerPool) -> Self {
        let (tx, rx) = channel();
        Self {
            ctx,
            pool: pool.clone(),
            states: HashMap::new(),
            tx,
            rx,
        }
    }

    /// Collects finished bindings (never blocks).
    pub fn poll(&mut self) {
        while let Ok((key, r)) = self.rx.try_recv() {
            if let Some((state, _)) = self.states.get_mut(&key) {
                *state = match r {
                    Ok(b) => BindState::Ready(b),
                    Err(e) => {
                        tracing::debug!("mounted clip {:#010x} not bound: {e}", key.0);
                        BindState::Failed
                    }
                };
            }
        }
    }

    /// Clip `did` bound to `skeleton`: ready, or started on a worker (pending), or failed
    /// (remembered; never retried while remembered).
    pub fn get(&mut self, did: Did, skeleton: &Arc<Skeleton>) -> Bind {
        let key = (did.0, Arc::as_ptr(skeleton) as usize);
        if let Some((state, _)) = self.states.get(&key) {
            return match state {
                BindState::Pending => Bind::Pending,
                BindState::Ready(b) => Bind::Ready(Arc::clone(b)),
                BindState::Failed => Bind::Failed,
            };
        }
        let Some(ctx) = self.ctx.clone() else {
            return Bind::Failed;
        };
        if self.states.len() >= MAX_BINDINGS {
            self.states
                .retain(|_, (s, _)| matches!(s, BindState::Pending));
        }
        let guard = BindGuard {
            key,
            tx: self.tx.clone(),
            sent: false,
        };
        let skel = Arc::clone(skeleton);
        let spawned = self
            .pool
            .spawn(Priority::Visible, CancelToken::new(), move |_| {
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ctx.clips.bind(&ctx.set, did.0, &skel)
                }));
                guard.send(r.unwrap_or_else(|_| Err("the binding job panicked".into())));
            });
        match spawned {
            Ok(_) => {
                // The skeleton is kept so that its address is not reused by another one
                // while the entry lives.
                self.states
                    .insert(key, (BindState::Pending, Arc::clone(skeleton)));
                Bind::Pending
            }
            Err(e) => {
                // The guard reported a failure through the channel; ask again later.
                tracing::debug!("mounted clip binding not queued: {e}");
                let _ = self.rx.try_recv();
                Bind::Pending
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Current {
    clip: Did,
    ramp_out: f32,
    loops: bool,
}

/// One entity's mounted clips on an animator (module docs).
#[derive(Debug, Clone)]
pub struct MountedAnim {
    animator: Animator,
    /// Animator layers below the keyed ones: the on-foot animator's at entry, fading out.
    legacy: usize,
    /// The legacy layers' fade has not started yet (it starts with the first clips).
    legacy_pending: bool,
    /// Seconds until the legacy layers are faded out and dropped.
    legacy_left: f32,
    /// Data-layer key of each keyed animator layer (at `legacy + i`), ascending.
    keys: Vec<u32>,
    /// The clip each data layer plays now.
    current: HashMap<u32, Current>,
    /// Data layers whose one-shot clip plays to its end although no longer listed.
    lingering: Vec<u32>,
    /// The held proportion clips (never changed).
    held: Vec<ClipPlay>,
    /// Leaving: seconds of the fade to the on-foot layers left.
    leaving: Option<f32>,
}

fn source_playback(source: Option<&mut Source>) -> Option<&mut Playback> {
    match source? {
        Source::Clip(p) => Some(p),
        Source::CrossFade(f) => Some(&mut f.to),
        _ => None,
    }
}

impl MountedAnim {
    /// An animator for `skeleton` with the figure's held proportion clips `held` (nothing
    /// below the mounted clips).
    #[must_use]
    pub fn new(skeleton: &Arc<Skeleton>, held: &[ClipPlay]) -> Self {
        Self::from_animator(Animator::new(Arc::clone(skeleton)), 0, held)
    }

    /// The mounted clips over a copy of the animator the figure plays now (`from`, the
    /// on-foot one, or a mounted one being left): its layers fade out while the first clips
    /// ramp in (module docs).
    #[must_use]
    pub fn entering(from: &Animator, held: &[ClipPlay]) -> Self {
        let legacy = from.layers().len().min(Animator::MAX_LAYERS / 2);
        let mut a = Animator::new(Arc::clone(from.skeleton()));
        for (i, l) in from.layers().iter().take(legacy).enumerate() {
            if let Ok(slot) = a.ensure_layer(i) {
                *slot = l.clone();
            }
        }
        Self::from_animator(a, legacy, held)
    }

    fn from_animator(animator: Animator, legacy: usize, held: &[ClipPlay]) -> Self {
        let mut me = Self {
            animator,
            legacy,
            legacy_pending: legacy > 0,
            legacy_left: 0.0,
            keys: Vec::new(),
            current: HashMap::new(),
            lingering: Vec::new(),
            held: held.to_vec(),
            leaving: None,
        };
        let held = me.held.clone();
        for h in &held {
            if let Some(i) = me.index_of(h.layer) {
                let mut p = Playback::held(Arc::clone(&h.clip), h.held.unwrap_or(0.0));
                p.use_track_weights = h.track_weights;
                let _ = if me
                    .animator
                    .layers()
                    .get(i)
                    .is_some_and(|l| l.source.is_some())
                {
                    me.animator.add(i, p)
                } else {
                    me.animator.play(i, p, 0.0, false)
                };
                if let Some(l) = me.animator.layer_mut(i) {
                    l.fade_to(1.0, 0.0);
                }
            }
        }
        me
    }

    /// The animator (sampled by the scheduler).
    #[must_use]
    pub fn animator(&self) -> &Animator {
        &self.animator
    }

    /// Layers below the mounted ones that still fade out.
    #[must_use]
    pub fn legacy_layers(&self) -> usize {
        self.legacy
    }

    /// Rebuilds the animator with layers `[from, from + count)` moved to `to` (the others
    /// keep their order).
    fn rebuild(&mut self, f: impl Fn(usize) -> Option<usize>) {
        let old: Vec<_> = self.animator.layers().to_vec();
        let mut a = Animator::new(Arc::clone(self.animator.skeleton()));
        for (i, l) in old.into_iter().enumerate() {
            if let Some(j) = f(i)
                && let Ok(slot) = a.ensure_layer(j)
            {
                *slot = l;
            }
        }
        self.animator = a;
    }

    /// The animator layer of data layer `key`, inserting it in order (the layers above
    /// move up, keeping their state). `None` beyond [`Animator::MAX_LAYERS`]. A new layer
    /// starts at weight 0 over legacy layers (it ramps in over them), else at 1.
    fn index_of(&mut self, key: u32) -> Option<usize> {
        if let Some(i) = self.keys.iter().position(|k| *k == key) {
            return Some(self.legacy + i);
        }
        if self.legacy + self.keys.len() >= Animator::MAX_LAYERS {
            return None;
        }
        let at = self.legacy + self.keys.partition_point(|k| *k < key);
        if at < self.animator.layers().len() {
            self.rebuild(|i| Some(if i >= at { i + 1 } else { i }));
        }
        let start = if self.legacy > 0 { 0.0 } else { 1.0 };
        if let Ok(l) = self.animator.ensure_layer(at) {
            *l = tessera_anim::Layer::new(None);
            l.fade_to(start, 0.0);
        }
        self.keys.insert(at - self.legacy, key);
        Some(at)
    }

    fn keyed(&self, key: u32) -> Option<usize> {
        self.keys
            .iter()
            .position(|k| *k == key)
            .map(|i| self.legacy + i)
    }

    /// Plays `clips` (module docs); `bind` gives each clip bound to the skeleton. While any
    /// clip to start is still binding nothing changes (the previous clips keep playing); a
    /// clip that cannot be bound is left out. Returns the data layers whose clip started.
    pub fn apply(&mut self, clips: &[ShownClip], bind: &mut dyn FnMut(Did) -> Bind) -> Vec<u32> {
        let held_layers: Vec<u32> = self.held.iter().map(|h| h.layer).collect();
        let mut start: Vec<(ShownClip, Arc<BoundClip>)> = Vec::new();
        for c in clips {
            if held_layers.contains(&c.layer)
                || self.current.get(&c.layer).map(|cur| cur.clip) == Some(c.clip)
            {
                continue;
            }
            if self.keyed(c.layer).is_none()
                && self.legacy + self.keys.len() + start.len() >= Animator::MAX_LAYERS
            {
                continue;
            }
            match bind(c.clip) {
                Bind::Ready(b) => start.push((*c, b)),
                Bind::Pending => return Vec::new(),
                Bind::Failed => {}
            }
        }
        let mut started = Vec::new();
        for (c, bound) in start {
            let Some(i) = self.index_of(c.layer) else {
                continue;
            };
            let mut p = Playback::new(bound, if c.loops { Wrap::Loop } else { Wrap::Once });
            p.rate = c.rate;
            if self.animator.play(i, p, c.ramp_in, false).is_ok() {
                if let Some(l) = self.animator.layer_mut(i) {
                    l.use_track_weights = c.track_weights;
                    l.fade_to(1.0, c.ramp_in);
                }
                self.current.insert(
                    c.layer,
                    Current {
                        clip: c.clip,
                        ramp_out: c.ramp_out,
                        loops: c.loops,
                    },
                );
                self.lingering.retain(|k| *k != c.layer);
                started.push(c.layer);
            }
        }
        let gone: Vec<u32> = self
            .current
            .keys()
            .filter(|k| !clips.iter().any(|c| c.layer == **k) && !self.lingering.contains(k))
            .copied()
            .collect();
        for k in gone {
            let cur = self.current.get(&k).copied();
            if cur.is_some_and(|c| !c.loops) && !self.finished(k) {
                // §14.7: a one-shot clip (the mounting clip) plays on until its end.
                self.lingering.push(k);
                continue;
            }
            self.fade_out(k);
        }
        if self.legacy_pending && !started.is_empty() {
            let fade = clips
                .iter()
                .map(|c| c.ramp_in)
                .filter(|r| r.is_finite() && *r > 0.0)
                .fold(0.0_f32, f32::max);
            let fade = if fade > 0.0 { fade } else { SWITCH_FADE };
            for i in 0..self.legacy {
                if let Some(l) = self.animator.layer_mut(i) {
                    l.fade_to(0.0, fade);
                }
            }
            self.legacy_pending = false;
            self.legacy_left = fade;
        }
        started
    }

    fn finished(&mut self, key: u32) -> bool {
        let Some(i) = self.keyed(key) else {
            return true;
        };
        self.animator
            .layer_mut(i)
            .and_then(|l| source_playback(l.source.as_mut()))
            .is_none_or(|p| p.is_finished())
    }

    fn fade_out(&mut self, key: u32) {
        let ramp = self.current.get(&key).map_or(0.0, |c| c.ramp_out);
        if let Some(i) = self.keyed(key)
            && let Some(l) = self.animator.layer_mut(i)
        {
            l.fade_to(0.0, ramp);
        }
        self.current.remove(&key);
        self.lingering.retain(|k| *k != key);
    }

    /// Advances by `dt` seconds: one-shot clips that ended fade out, faded legacy layers
    /// are dropped, a leave counts down.
    pub fn advance(&mut self, dt: f32) {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        let _ = self.animator.advance(dt);
        let ended: Vec<u32> = self
            .lingering
            .clone()
            .into_iter()
            .filter(|k| self.finished(*k))
            .collect();
        for k in ended {
            self.fade_out(k);
        }
        if self.legacy > 0 && !self.legacy_pending && self.leaving.is_none() {
            self.legacy_left -= dt;
            if self.legacy_left <= 0.0 {
                let n = self.legacy;
                self.rebuild(|i| i.checked_sub(n));
                self.legacy = 0;
            }
        }
        if let Some(t) = &mut self.leaving {
            *t -= dt;
        }
    }

    /// Starts leaving towards the on-foot animator `on_foot` (module docs): a copy of its
    /// layers goes on top and fades in over `fade` seconds while every mounted clip ramps out.
    /// Nothing happens when already leaving.
    pub fn leave(&mut self, on_foot: &Animator, fade: f32) {
        if self.leaving.is_some() {
            return;
        }
        let fade = if fade.is_finite() && fade > 0.0 {
            fade
        } else {
            SWITCH_FADE
        };
        let keys: Vec<u32> = self.current.keys().copied().collect();
        for k in keys {
            self.fade_out(k);
        }
        let base = self.animator.layers().len();
        if base + on_foot.layers().len() > Animator::MAX_LAYERS {
            // No room for the copy: switch at once.
            self.leaving = Some(0.0);
            return;
        }
        for (j, l) in on_foot.layers().iter().enumerate() {
            if let Ok(slot) = self.animator.ensure_layer(base + j) {
                let target = l.weight;
                *slot = l.clone();
                slot.fade_to(0.0, 0.0);
                slot.fade_to(target, fade);
            }
        }
        self.leaving = Some(fade);
    }

    /// Whether a leave is in progress (or over).
    #[must_use]
    pub fn is_leaving(&self) -> bool {
        self.leaving.is_some()
    }

    /// Whether the leave is over (the on-foot animator takes over).
    #[must_use]
    pub fn left(&self) -> bool {
        self.leaving.is_some_and(|t| t <= 0.0)
    }

    /// The clip data layer `key` plays.
    #[must_use]
    pub fn playing(&self, key: u32) -> Option<Did> {
        self.current.get(&key).map(|c| c.clip)
    }

    /// The phase (0–1) of the clip data layer `key` plays.
    #[must_use]
    pub fn phase(&mut self, key: u32) -> Option<f32> {
        let i = self.keyed(key)?;
        let l = self.animator.layer_mut(i)?;
        source_playback(l.source.as_mut()).map(|p| p.phase())
    }

    /// Sets the phase (0–1) of the clip data layer `key` plays.
    pub fn set_phase(&mut self, key: u32, phase: f32) {
        if let Some(i) = self.keyed(key)
            && let Some(l) = self.animator.layer_mut(i)
            && let Some(p) = source_playback(l.source.as_mut())
        {
            p.set_phase(phase);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_havok::testkit::{
        AnimationKindSpec, AnimationSpec, BoneSpec, ClipRecordSpec, InterleavedClipSpec,
        SkeletonSpec,
    };
    use tessera_havok::{Clip, Transform};

    const BONES: usize = 2;
    const T: Transform = Transform {
        translation: [0.0, 0.1, 0.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0; 3],
    };

    fn skel() -> Arc<Skeleton> {
        let spec = SkeletonSpec {
            bones: (0..BONES)
                .map(|i| BoneSpec {
                    name: Some(format!("bone{i}")),
                    parent: i as i32 - 1,
                    reference: T,
                    lock_translation: false,
                })
                .collect(),
            overrides: Vec::new(),
        };
        Arc::new(Skeleton::from_setup_record(&spec.to_setup_prefix(0x0400_0001)).unwrap())
    }

    /// A clip of `seconds`.
    fn bound(s: &Arc<Skeleton>, seconds: f32) -> Arc<BoundClip> {
        let rec = ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Interleaved(
            InterleavedClipSpec {
                duration: seconds,
                tracks: BONES,
                transforms: vec![T; 2 * BONES],
            },
        )));
        let clip = Clip::parse(&rec.to_bytes()).unwrap();
        Arc::new(BoundClip::bind(Arc::new(clip), Arc::clone(s)).unwrap())
    }

    fn shown(clip: u32, layer: u32, loops: bool) -> ShownClip {
        ShownClip {
            clip: Did(clip),
            layer,
            rate: 1.0,
            ramp_in: 0.2,
            ramp_out: 0.2,
            loops,
            track_weights: false,
        }
    }

    fn mount_view(anchor: PropAnchorRule, location: u32) -> PlayerMountView {
        PlayerMountView {
            figure: None,
            mesh: MountMesh {
                entity: Did(0x4700_0001),
                holding_location: location,
                bone_offset: [0.0, 0.0, 0.5],
                scale: [1.0; 3],
                scale_driven: false,
                use_parent_scale: true,
                behavior: 0,
                table: Did(0x0700_0001),
                node_id: 1,
            },
            anchor,
            attached: true,
            rider_model_scale: [1.0; 3],
            alpha: 1.0,
            drawn: true,
            rider_clips: None,
            mount_clips: Vec::new(),
            rider_shown: None,
            mount_shown: None,
            back: false,
        }
    }

    /// `script-table.md` §5.2 in the draw path: the mount is placed at the anchor its rule
    /// names (a holding location at its bone and frame, a bone type at the bone, key 6 at
    /// its element or the model origin, the origin), not at the mesh's holding location, and
    /// a rider lacking the anchor draws nothing. The rider's pose and frame move it.
    #[test]
    fn mount_follows_the_anchor_rule() {
        use crate::walker::characters::figure::prop_anchor_rule;
        const LOC: u32 = 0x1000_0027;
        let t = |x: f32| Mat4::from_translation(Vec3::new(x, 0.0, 0.0));
        let pos = |m: Option<Mat4>| m.map(|m| m.to_scale_rotation_translation().2);
        let near = |a: Option<Vec3>, b: Vec3| a.is_some_and(|a| (a - b).length() < 1e-5);
        let points = AttachPoints {
            holding: vec![(LOC, Some(1), t(0.5)), (6, None, t(-3.0))],
            bones: vec![(3, 1)],
            reference: vec![Mat4::IDENTITY, t(2.0)],
        };
        let rider = Mat4::IDENTITY;
        let offset = Vec3::new(0.0, 0.0, 0.5);
        // An attached node with its holding location: bone 1 (x 2) × frame (x 0.5).
        let v = mount_view(prop_anchor_rule(true, LOC, 0), LOC);
        assert!(near(
            pos(mount_model(&points, &v, &[], rider)),
            Vec3::new(2.5, 0.0, 0.5)
        ));
        // The posed bone moves it.
        let posed = [Mat4::IDENTITY, t(10.0)];
        assert!(near(
            pos(mount_model(&points, &v, &posed, rider)),
            Vec3::new(10.5, 0.0, 0.5)
        ));
        // An attached node with bone type 3 and no location: the bone, identity frame, even
        // though the mesh still names a holding location of the rider.
        let v = mount_view(prop_anchor_rule(true, 0, 3), LOC);
        assert!(near(
            pos(mount_model(&points, &v, &[], rider)),
            Vec3::new(2.0, 0.0, 0.0) + offset
        ));
        // An unattached node setting both fields takes the bone (the reversed precedence).
        let v = mount_view(prop_anchor_rule(false, LOC, 3), LOC);
        assert_eq!(v.anchor, PropAnchorRule::Bone(3));
        assert!(near(
            pos(mount_model(&points, &v, &[], rider)),
            Vec3::new(2.0, 0.0, 0.0) + offset
        ));
        // Key 6: the rider's key-6 element; without one the model origin.
        let v = mount_view(prop_anchor_rule(true, 0, 0), 0);
        assert!(near(
            pos(mount_model(&points, &v, &[], rider)),
            Vec3::new(-3.0, 0.0, 0.0) + offset
        ));
        let bare = AttachPoints {
            holding: vec![(LOC, Some(1), t(0.5))],
            ..points.clone()
        };
        assert!(near(pos(mount_model(&bare, &v, &[], rider)), offset));
        // The origin (unattached, neither field).
        let v = mount_view(prop_anchor_rule(false, 0, 0), 0);
        assert!(near(pos(mount_model(&points, &v, &[], rider)), offset));
        // A rider without the named location or bone: nothing drawn.
        for rule in [PropAnchorRule::Location(77), PropAnchorRule::Bone(77)] {
            assert!(mount_model(&points, &mount_view(rule, LOC), &[], rider).is_none());
        }
        // The rider's frame turns and scales the placement.
        let turned = rider_frame(
            Vec3::splat(2.0),
            Quat::from_rotation_z(std::f32::consts::FRAC_PI_2).to_array(),
        );
        let v = mount_view(prop_anchor_rule(true, LOC, 0), LOC);
        assert!(near(
            pos(mount_model(&points, &v, &[], turned)),
            Vec3::new(0.0, 5.0, 1.0)
        ));
        // A degenerate rotation counts as none.
        assert_eq!(rider_frame(Vec3::ONE, [0.0; 4]), Mat4::IDENTITY);
        assert_eq!(rider_frame(Vec3::ONE, [f32::NAN; 4]), Mat4::IDENTITY);
    }

    /// `script-table.md` §5.2 answer: an unattached node's mount keeps the world frame of
    /// its placement while the rider moves; an attached node's follows the rider every
    /// frame; hiding the mount or changing the node places it again.
    #[test]
    fn unattached_mount_keeps_its_placement() {
        use crate::walker::characters::figure::prop_anchor_rule;
        const LOC: u32 = 0x1000_0027;
        let t = |x: f32| Mat4::from_translation(Vec3::new(x, 0.0, 0.0));
        let points = AttachPoints {
            holding: vec![(LOC, Some(1), t(0.5))],
            bones: vec![(3, 1)],
            reference: vec![Mat4::IDENTITY, t(2.0)],
        };
        // The world placement: the rider's position plus the model transform's translation.
        let world = |rider_x: f32, posed: &[Mat4], v: &PlayerMountView| {
            mount_model(&points, v, posed, Mat4::IDENTITY)
                .map(|m| m.to_scale_rotation_translation().2 + Vec3::new(rider_x, 0.0, 0.0))
        };
        let moved = [Mat4::IDENTITY, t(10.0)];

        // Unattached (bone type 3): placed once, then held while the rider walks and poses.
        let mut un = mount_view(prop_anchor_rule(false, 0, 3), LOC);
        un.attached = false;
        let mut hold = HeldPlacement::default();
        let first = hold.place(&un, || world(0.0, &[], &un)).unwrap();
        assert!((first - Vec3::new(2.0, 0.0, 0.5)).length() < 1e-5);
        assert!(hold.is_held());
        let later = hold.place(&un, || world(5.0, &moved, &un)).unwrap();
        assert_eq!(
            later, first,
            "the unattached mount does not follow the rider"
        );
        // Hidden, then shown again: a new placement from the current anchor.
        un.drawn = false;
        assert!(hold.place(&un, || world(5.0, &moved, &un)).is_none());
        assert!(!hold.is_held());
        un.drawn = true;
        let again = hold.place(&un, || world(5.0, &moved, &un)).unwrap();
        assert!((again - Vec3::new(15.0, 0.0, 0.5)).length() < 1e-5);
        // Another node: placed again.
        let mut other = un.clone();
        other.mesh.node_id = 2;
        let o = hold.place(&other, || world(0.0, &[], &other)).unwrap();
        assert!((o - first).length() < 1e-5);
        // A placement that finds no anchor holds nothing and is tried again.
        assert!(hold.place(&un, || None).is_none());
        assert!(!hold.is_held());

        // Attached: follows the rider every frame and holds nothing.
        let at = mount_view(prop_anchor_rule(true, LOC, 0), LOC);
        let mut hold = HeldPlacement::default();
        let a0 = hold.place(&at, || world(0.0, &[], &at)).unwrap();
        let a1 = hold.place(&at, || world(5.0, &moved, &at)).unwrap();
        assert!((a1 - a0 - Vec3::new(13.0, 0.0, 0.0)).length() < 1e-5);
        assert!(!hold.is_held());
    }

    /// `mounts.md` §14.7: entering keeps the on-foot layers (full weight at first, faded
    /// out over the ramp); leaving puts them back on top with a ramp; no frame jumps.
    #[test]
    fn switching_ramps_between_on_foot_and_mounted() {
        let s = skel();
        let mut foot = Animator::new(Arc::clone(&s));
        foot.play(0, Playback::new(bound(&s, 1.0), Wrap::Loop), 0.0, false)
            .unwrap();
        let mut m = MountedAnim::entering(&foot, &[]);
        assert_eq!(m.legacy_layers(), 1);
        let b = bound(&s, 2.0);
        let started = m.apply(&[shown(1, 13, true)], &mut |_| Bind::Ready(Arc::clone(&b)));
        assert_eq!(started, vec![13]);
        let w = |m: &MountedAnim| -> Vec<f32> {
            m.animator().layers().iter().map(|l| l.weight).collect()
        };
        assert_eq!(
            w(&m),
            vec![1.0, 0.0],
            "on-foot at full weight, mounted from 0"
        );
        m.advance(0.1);
        let mid = w(&m);
        assert!(
            (mid[0] - 0.5).abs() < 1e-4 && (mid[1] - 0.5).abs() < 1e-4,
            "{mid:?}"
        );
        m.advance(0.15);
        assert_eq!(m.legacy_layers(), 0, "legacy dropped after its fade");
        assert_eq!(w(&m), vec![1.0]);
        // Leaving: the on-foot copy on top fades in, the mounted clip out.
        m.leave(&foot, 0.2);
        assert_eq!(w(&m), vec![1.0, 0.0]);
        m.advance(0.1);
        let mid = w(&m);
        assert!(
            (mid[0] - 0.5).abs() < 1e-4 && (mid[1] - 0.5).abs() < 1e-4,
            "{mid:?}"
        );
        assert!(!m.left());
        m.advance(0.11);
        assert!(m.left());
    }

    /// `walker.md` §16.7: a key whose clips are still binding changes nothing; a clip that
    /// cannot be bound is left out.
    #[test]
    fn pending_bindings_keep_the_previous_clips() {
        let s = skel();
        let mut m = MountedAnim::new(&s, &[]);
        let b = bound(&s, 1.0);
        m.apply(&[shown(1, 13, true)], &mut |_| Bind::Ready(Arc::clone(&b)));
        assert_eq!(m.playing(13), Some(Did(1)));
        let started = m.apply(&[shown(2, 13, true), shown(3, 54, true)], &mut |d| {
            if d == Did(3) {
                Bind::Pending
            } else {
                Bind::Ready(Arc::clone(&b))
            }
        });
        assert!(started.is_empty());
        assert_eq!(m.playing(13), Some(Did(1)), "previous clip kept");
        let started = m.apply(&[shown(2, 13, true), shown(3, 54, true)], &mut |d| {
            if d == Did(3) {
                Bind::Failed
            } else {
                Bind::Ready(Arc::clone(&b))
            }
        });
        assert_eq!(started, vec![13]);
        assert_eq!(m.playing(54), None);
    }

    /// `mounts.md` §14.7: the mounting clip (one-shot, layer 54) plays to its end when the
    /// riding key no longer lists it; a looping clip that is no longer listed fades out.
    #[test]
    fn one_shot_clips_play_to_their_end() {
        let s = skel();
        let mut m = MountedAnim::new(&s, &[]);
        let short = bound(&s, 0.5);
        let b = bound(&s, 1.0);
        m.apply(&[shown(1, 13, true), shown(9, 54, false)], &mut |d| {
            Bind::Ready(Arc::clone(if d == Did(9) { &short } else { &b }))
        });
        m.advance(0.1);
        m.apply(&[shown(1, 13, true)], &mut |_| Bind::Ready(Arc::clone(&b)));
        assert_eq!(m.playing(54), Some(Did(9)), "still playing");
        m.advance(0.5);
        assert_eq!(m.playing(54), None, "faded once ended");
        // A locomotion clip on its layer replaces it at once.
        m.apply(&[shown(1, 13, true), shown(9, 54, false)], &mut |d| {
            Bind::Ready(Arc::clone(if d == Did(9) { &short } else { &b }))
        });
        m.apply(&[shown(1, 13, true), shown(4, 54, true)], &mut |_| {
            Bind::Ready(Arc::clone(&b))
        });
        assert_eq!(m.playing(54), Some(Did(4)));
    }

    /// `mounts.md` §14.5 item 6.
    #[test]
    fn phase_matching() {
        assert_eq!(matched_phase(0.6, false), 0.6);
        assert_eq!(matched_phase(0.6, true), 0.3);
        assert_eq!(matched_phase(f32::NAN, false), 0.0);
        let s = skel();
        let mut m = MountedAnim::new(&s, &[]);
        let b = bound(&s, 1.0);
        m.apply(&[shown(1, 54, true)], &mut |_| Bind::Ready(Arc::clone(&b)));
        m.set_phase(54, 0.25);
        assert!((m.phase(54).unwrap() - 0.25).abs() < 1e-3);
    }
}
