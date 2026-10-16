// SPDX-License-Identifier: GPL-3.0-or-later
//! Mounts in the walker (P5; `walker.md` §16, `mounts.md` §4, §5, §14).
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`spec`] | `--mount SPEC` and `/ride` arguments (std-only grammar) | `walker.md` §16.2, §16.3 |
//! | [`profile`] | One mount for one body: effect, category, rider properties, controller values, induction, camera | `mounts.md` §2, §4.1, §5, §14.6–§14.11 |
//! | [`state`] | On foot, summoning, mounting, mounted, dismounting | `walker.md` §16.5, `mounts.md` §14.7 |
//! | [`ride`] | Mounted movement inside the physics steps | `walker.md` §16.6, `mounts.md` §5, §14.6 |
//! | [`eval`] | The mount mesh, the clips of rider and mount per state key, script timings | `mounts.md` §4.4, §14.1–§14.5, §14.7 |
//! | [`MountEvaluator`], [`MountService`] | The evaluation of one (body, effect), synchronous or on its own thread | `walker.md` §16.7 |
//! | [`mount_transform`] | The mount's placement at the rider's holding location | `mounts.md` §4.4, §14.8 |
//! | [`walk`] | Headless mounted walks with the log rows | `walker.md` §16.8, §16.10, §16.11 |
//! | [`census`] | Every mount effect for one body: drawable or categorised | `mounts.md` §14.11 |
//!
//! # Using it
//!
//! The walker keeps the mount choice of its options ([`MountChoice`] in
//! [`crate::walker::WalkerConfig`]) and offers [`crate::walker::Walker::mount`] and
//! [`crate::walker::Walker::dismount`] at run time; both take a mount effect (or skill) DID
//! or a [`spec::MountSpec`]. Resolution reads the install on a worker thread
//! ([`MountService`]); a refusal ([`profile::MountRefusal`]) names its census category and
//! leaves the rider on foot.

pub mod census;
pub mod eval;
pub mod profile;
pub mod ride;
pub mod session;
pub mod spec;
pub mod state;
pub mod walk;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use tessera_formats::Did;
use tessera_formats::script_table::resolve::ScriptVocabulary;
use tessera_mesh::assembly::AssemblyOptions;
use tessera_render::glam::{Mat4, Vec3};

use self::eval::{
    ClipKey, EntityContext, KeyCache, KeyClips, MeshChoice, MountVocab, Resolver, Stage,
    choose_mesh, entity_context, evaluate_key, mount_context, resolver, rider_state,
    script_seconds,
};
use self::profile::{MountKind, MountProfile, MountRefusal};
use self::state::Timeline;
use super::characters::catalog::CharacterCatalog;
use super::characters::figure::FigureRequest;

pub use self::spec::{MountSource, MountSpec, MountTiming};

/// The mount choice of the walker's options (`walker.md` §16.2: from the command line the
/// default timing is `instant`).
pub type MountChoice = MountSpec;

/// What the evaluation of one (body, effect) found (`walker.md` §16.7).
#[derive(Debug, Clone, PartialEq)]
pub struct MountSetup {
    /// The MeshFX node and its fade times (§4.4).
    pub mesh: MeshChoice,
    /// The durations of the sequence (§14.7).
    pub timeline: Timeline,
    /// The rider's final model scale per axis.
    pub rider_model_scale: [f32; 3],
    /// The mounted branch of the jump (`walker.md` §16.6 "Jump", `mounts.md` §14.6);
    /// unused for a hobby-horse, whose rider keeps the on-foot jump.
    pub jump: ride::MountedJump,
    /// The mount entity evaluates (clips are found); a mount without is drawn in its
    /// reference pose.
    pub mount_animated: bool,
    /// Problems that do not stop the mount.
    pub notes: Vec<String>,
}

/// The rider's side of an evaluation, reusable across mount effects (the census).
pub struct RiderEvaluator<'s> {
    resolver: Resolver<'s>,
    script_vocab: ScriptVocabulary,
    vocab: MountVocab,
    rider: EntityContext,
    cat: &'s CharacterCatalog,
    options: AssemblyOptions,
}

impl std::fmt::Debug for RiderEvaluator<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RiderEvaluator").finish_non_exhaustive()
    }
}

impl<'s> RiderEvaluator<'s> {
    /// The rider whose figure `rider` builds.
    ///
    /// # Errors
    ///
    /// [`MountRefusal::Unavailable`] when the rider does not assemble or its scripts cannot
    /// be read.
    pub fn new(
        cat: &'s CharacterCatalog,
        options: AssemblyOptions,
        rider: &FigureRequest,
    ) -> Result<Self, MountRefusal> {
        let (ctx, script_vocab) =
            entity_context(cat, options, rider).map_err(MountRefusal::Unavailable)?;
        let resolver = resolver(cat, script_vocab.clone(), ctx.after_startup.clone())
            .map_err(MountRefusal::Unavailable)?;
        Ok(Self {
            resolver,
            script_vocab,
            vocab: MountVocab::from_catalog(cat),
            rider: ctx,
            cat,
            options,
        })
    }

    /// The rider's context.
    #[must_use]
    pub fn rider(&self) -> &EntityContext {
        &self.rider
    }

    /// The vocabulary.
    #[must_use]
    pub fn vocab(&self) -> &MountVocab {
        &self.vocab
    }

    /// The MeshFX choice for a rider with `entries` (the Decision of §4.1 applied), checked
    /// against the rider's holding locations; `category` names the refusal.
    ///
    /// # Errors
    ///
    /// [`MountRefusal::NoMesh`], [`MountRefusal::NoLocation`].
    pub fn mesh(
        &mut self,
        effect: Did,
        entries: &[tessera_formats::PropertyEntry],
        category: tessera_gamedata::mount::MountCategory,
    ) -> Result<MeshChoice, MountRefusal> {
        let m = choose_mesh(
            &mut self.resolver,
            &self.rider,
            &self.script_vocab,
            &self.vocab,
            entries,
        )
        .ok_or(MountRefusal::NoMesh { effect, category })?;
        let has = self
            .rider
            .points
            .as_ref()
            .is_some_and(|p| p.holding(m.mesh.holding_location).is_some());
        if !has {
            return Err(MountRefusal::NoLocation {
                effect,
                location: m.mesh.holding_location,
                category,
            });
        }
        Ok(m)
    }

    /// The mount entity's context for `mesh`.
    ///
    /// # Errors
    ///
    /// The text of the problem.
    pub fn mount_context(&self, mesh: &MeshChoice) -> Result<EntityContext, String> {
        mount_context(self.cat, self.options, &mesh.mesh, &self.vocab)
    }

    /// How long after the dismount script `key` starts (status 7, by the player) the
    /// channelling loop stops ([`eval::script_stops`]); the rider is mounted (status 5,
    /// `Mount_Is_Mounted` true) with the loop running first.
    #[must_use]
    pub fn loop_stops(
        &self,
        key: Option<u32>,
        entries: &[tessera_formats::PropertyEntry],
    ) -> Option<f32> {
        let key = key?;
        let watched = self.vocab.channelling_loop?;
        let mut props = self.rider.scripts.properties.clone();
        for e in entries {
            props.set(e.pid, e.value.clone());
        }
        if let Some(p) = self.vocab.is_mounted {
            props.set(p, tessera_formats::PropertyValue::Bool(true));
        }
        if let Some(p) = self.vocab.status {
            props.set(p, tessera_formats::PropertyValue::Enum(7));
        }
        eval::script_stops(self.cat, self.rider.scripts.tables(), props, watched, key)
    }

    /// How long script `key` runs on the rider with `entries` and status `status`
    /// ([`script_seconds`]).
    #[must_use]
    pub fn script_seconds(
        &self,
        key: Option<u32>,
        entries: &[tessera_formats::PropertyEntry],
        status: u32,
    ) -> Option<f32> {
        let key = key?;
        let mut props = self.rider.scripts.properties.clone();
        for e in entries {
            props.set(e.pid, e.value.clone());
        }
        if let Some(p) = self.vocab.status {
            props.set(p, tessera_formats::PropertyValue::Enum(status));
        }
        script_seconds(self.cat, self.rider.scripts.tables(), props, key)
    }
}

/// The evaluation of one (body, effect): the setup and the clips per state key, cached.
pub struct MountEvaluator<'s> {
    rider: RiderEvaluator<'s>,
    mount: Option<EntityContext>,
    profile: Arc<MountProfile>,
    setup: MountSetup,
    cache: KeyCache,
}

impl std::fmt::Debug for MountEvaluator<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountEvaluator")
            .field("setup", &self.setup)
            .finish_non_exhaustive()
    }
}

impl<'s> MountEvaluator<'s> {
    /// Evaluates `profile` for the rider `rider` builds: the mesh (§4.4), the mount's
    /// scripts and the timeline (§14.7).
    ///
    /// # Errors
    ///
    /// [`MountRefusal`] when the mount cannot be drawn for this body (with its category).
    pub fn new(
        cat: &'s CharacterCatalog,
        options: AssemblyOptions,
        rider: &FigureRequest,
        profile: Arc<MountProfile>,
    ) -> Result<Self, MountRefusal> {
        Self::new_cancellable(cat, options, rider, profile, &|| false)
    }

    /// [`MountEvaluator::new`] that gives up between its stages (assembly, mesh, mount
    /// entity, each script run) once `cancelled` returns true.
    ///
    /// # Errors
    ///
    /// As [`MountEvaluator::new`]; [`MountRefusal::Unavailable`] when cancelled.
    pub fn new_cancellable(
        cat: &'s CharacterCatalog,
        options: AssemblyOptions,
        rider: &FigureRequest,
        profile: Arc<MountProfile>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, MountRefusal> {
        let stop = || {
            if cancelled() {
                Err(MountRefusal::Unavailable("cancelled".into()))
            } else {
                Ok(())
            }
        };
        stop()?;
        let mut r = RiderEvaluator::new(cat, options, rider)?;
        stop()?;
        let category = profile.info.category;
        let mesh = r.mesh(profile.effect, &profile.entries, category)?;
        stop()?;
        let mut notes = Vec::new();
        let mount = match r.mount_context(&mesh) {
            Ok(m) => Some(m),
            Err(e) => {
                notes.push(format!("mount entity {}: {e}", mesh.mesh.entity));
                None
            }
        };
        if mount.is_none() && profile.kind != MountKind::HobbyHorse {
            return Err(MountRefusal::NoMesh {
                effect: profile.effect,
                category,
            });
        }
        stop()?;
        let mounting = r
            .script_seconds(profile.start_script, &profile.entries, 0)
            .unwrap_or_else(|| {
                notes.push("the start script's length is unknown; mounting takes no time".into());
                0.0
            });
        stop()?;
        // The wait ends when the dismount script stops the channelling loop (§14.7); else
        // when the dismount script itself completes.
        let dismount_wait = r
            .loop_stops(profile.dismount_by_player, &profile.entries)
            .or_else(|| r.script_seconds(profile.dismount_by_player, &profile.entries, 7))
            .unwrap_or_else(|| {
                notes.push("the dismount script's length is unknown; no wait".into());
                0.0
            });
        let timeline = Timeline {
            summon: profile.induction.map(|i| i.duration),
            mounting,
            dismount_wait,
            fade_in: mesh.fade_in,
            fade_out: mesh.fade_out,
        }
        .sanitized();
        stop()?;
        let rider_model_scale = r.rider().model_scale;
        let jump = if profile.rides() {
            eval::mounted_jump(
                cat,
                &r.rider,
                &r.script_vocab,
                &r.vocab,
                &profile.entries,
                profile.mount_type,
                &mut notes,
            )
        } else {
            ride::MountedJump::default()
        };
        let mut me = Self {
            rider: r,
            mount,
            profile,
            setup: MountSetup {
                mesh,
                timeline,
                rider_model_scale,
                jump,
                mount_animated: false,
                notes,
            },
            cache: KeyCache::default(),
        };
        let idle = me.clips(&ClipKey::default());
        me.setup.mount_animated = !idle.mount.layers.is_empty();
        Ok(me)
    }

    /// The setup.
    #[must_use]
    pub fn setup(&self) -> &MountSetup {
        &self.setup
    }

    /// The profile.
    #[must_use]
    pub fn profile(&self) -> &Arc<MountProfile> {
        &self.profile
    }

    /// The clips of `key` (cached per key, `walker.md` §16.7).
    pub fn clips(&mut self, key: &ClipKey) -> Arc<KeyClips> {
        if let Some(c) = self.cache.get(key) {
            return c;
        }
        let stage_script = match key.stage {
            Stage::Summoning => self.profile.induction.and_then(|i| i.script),
            Stage::Mounting => self.profile.start_script,
            Stage::Riding | Stage::Dismounting => None,
        };
        // A hobby-horse rider stays on foot: it is evaluated on foot (no rider fields),
        // the prop shows its own idle.
        let entries: &[tessera_formats::PropertyEntry] = &self.profile.entries;
        let r = &mut self.rider;
        let clips = Arc::new(evaluate_key(
            &mut r.resolver,
            &r.rider,
            self.mount.as_ref(),
            &r.script_vocab,
            &r.vocab,
            entries,
            key,
            stage_script,
        ));
        self.cache.put(*key, Arc::clone(&clips));
        clips
    }

    /// The rider's evaluated property state for `key` (diagnostics and tests).
    #[must_use]
    pub fn rider_state(
        &self,
        key: &ClipKey,
    ) -> tessera_formats::script_table::resolve::PropertyState {
        rider_state(
            &self.rider.rider,
            &self.rider.script_vocab,
            &self.rider.vocab,
            &self.profile.entries,
            key,
            None,
        )
        .1
    }
}

enum Request {
    Key(ClipKey),
}

enum Reply {
    Setup(Box<Result<MountSetup, MountRefusal>>),
    Clips(ClipKey, Arc<KeyClips>),
}

/// [`MountEvaluator`] on its own thread (`walker.md` §16.7: evaluations off the frame
/// loop). Dropping the service cancels the thread: an evaluation in progress gives up at its
/// next stage, a finished one ends with its request channel.
pub struct MountService {
    cancel: Arc<AtomicBool>,
    tx: Sender<Request>,
    rx: Receiver<Reply>,
    profile: Arc<MountProfile>,
    setup: Option<Result<MountSetup, MountRefusal>>,
    cache: KeyCache,
    pending: Vec<ClipKey>,
}

impl std::fmt::Debug for MountService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountService")
            .field("effect", &self.profile.effect)
            .field("pending", &self.pending.len())
            .finish_non_exhaustive()
    }
}

/// Most key evaluations queued at once.
pub const MAX_PENDING_KEYS: usize = 4;

impl MountService {
    /// Starts the evaluation of `profile` for the rider `rider` builds.
    ///
    /// # Errors
    ///
    /// [`MountRefusal::Unavailable`] when the thread cannot be started.
    pub fn start(
        cat: Arc<CharacterCatalog>,
        options: AssemblyOptions,
        rider: FigureRequest,
        profile: Arc<MountProfile>,
    ) -> Result<Self, MountRefusal> {
        let (tx, req_rx) = channel::<Request>();
        let (reply_tx, rx) = channel::<Reply>();
        let p = Arc::clone(&profile);
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        std::thread::Builder::new()
            .name("tessera-mount".into())
            .spawn(move || {
                let cancelled = || flag.load(Ordering::Relaxed);
                let ev = MountEvaluator::new_cancellable(&cat, options, &rider, p, &cancelled);
                let mut ev = match ev {
                    Ok(ev) => {
                        let _ = reply_tx.send(Reply::Setup(Box::new(Ok(ev.setup().clone()))));
                        ev
                    }
                    Err(e) => {
                        let _ = reply_tx.send(Reply::Setup(Box::new(Err(e))));
                        return;
                    }
                };
                while let Ok(Request::Key(k)) = req_rx.recv() {
                    let c = ev.clips(&k);
                    if reply_tx.send(Reply::Clips(k, c)).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| MountRefusal::Unavailable(format!("mount thread: {e}")))?;
        Ok(Self {
            cancel,
            tx,
            rx,
            profile,
            setup: None,
            cache: KeyCache::default(),
            pending: Vec::new(),
        })
    }

    /// The profile.
    #[must_use]
    pub fn profile(&self) -> &Arc<MountProfile> {
        &self.profile
    }

    /// Collects finished work (never blocks).
    pub fn poll(&mut self) {
        loop {
            match self.rx.try_recv() {
                Ok(Reply::Setup(s)) => self.setup = Some(*s),
                Ok(Reply::Clips(k, c)) => {
                    self.pending.retain(|p| *p != k);
                    self.cache.put(k, c);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if self.setup.is_none() {
                        self.setup = Some(Err(MountRefusal::Unavailable(
                            "mount evaluation stopped".into(),
                        )));
                    }
                    self.pending.clear();
                    break;
                }
            }
        }
    }

    /// The setup once evaluated.
    #[must_use]
    pub fn setup(&self) -> Option<&Result<MountSetup, MountRefusal>> {
        self.setup.as_ref()
    }

    /// The clips of `key` when evaluated; otherwise starts its evaluation (at most
    /// [`MAX_PENDING_KEYS`] at once) and returns `None` (the caller keeps its clips).
    pub fn clips(&mut self, key: &ClipKey) -> Option<Arc<KeyClips>> {
        if let Some(c) = self.cache.get(key) {
            return Some(c);
        }
        if !self.pending.contains(key)
            && self.pending.len() < MAX_PENDING_KEYS
            && self.tx.send(Request::Key(*key)).is_ok()
        {
            self.pending.push(*key);
        }
        None
    }
}

impl Drop for MountService {
    /// Asks the thread to stop: an evaluation in progress gives up at its next stage.
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// The mount's model transform (`mounts.md` §14.8 item 1, the Decision of §4.4): with
/// `rider_world` the rider's model transform (feet, heading, final model scale) and
/// `location` the rider's posed bone × location frame ([`eval::location_matrix`]), the
/// attachment point is *P* = `rider_world` × `location` × T(`BoneOffset`); the mount is drawn
/// at *P*'s position and rotation with the scale [`tessera_formats::gameplay::mounts::MountMesh::draw_scale`].
#[must_use]
pub fn mount_transform(
    rider_world: Mat4,
    location: Mat4,
    mesh: &tessera_formats::gameplay::mounts::MountMesh,
    rider_model_scale: [f32; 3],
) -> Mat4 {
    let offset = Vec3::from_array(
        mesh.bone_offset
            .map(|v| if v.is_finite() { v } else { 0.0 }),
    );
    let p = rider_world * location * Mat4::from_translation(offset);
    let (_, rot, pos) = p.to_scale_rotation_translation();
    let scale = Vec3::from_array(mesh.draw_scale(rider_model_scale));
    let m = Mat4::from_scale_rotation_translation(scale, rot.normalize(), pos);
    if m.is_finite() { m } else { Mat4::IDENTITY }
}

/// The mount columns of a log row (`walker.md` §16.10) for the walker's state: the
/// mover (if any), its clips, the controller's layer and the ground speed.
#[must_use]
pub fn mount_row(
    walker: &super::movement::PhysicsWalker,
    clips: Option<&KeyClips>,
    ground_speed: f32,
) -> super::framelog::MountRow {
    let ctl = walker.controller();
    let walking =
        walker.player_input().style() == tessera_physics::controller::MovementStyle::Walking;
    let style = if walking { 1 } else { 2 };
    let mut row = super::framelog::MountRow {
        style,
        move_layer: ride::top_layer_name(ctl),
        ground_speed: if ground_speed.is_finite() {
            ground_speed
        } else {
            0.0
        },
        ..super::framelog::MountRow::default()
    };
    let Some(m) = walker.mount() else {
        return row;
    };
    row.effect = m.profile().effect.0;
    row.state = m.state().phase().number();
    row.mount_type = m.profile().mount_type;
    row.gait = m.gait();
    row.locked_gear = m.locked_gear();
    row.style = if m.walking() { 1 } else { 2 };
    row.control_speed = m.control_speed();
    row.mount_alpha = m.state().mount_alpha();
    if let Some(c) = clips {
        if let Some(r) = c.rider.shown {
            row.rider_clip = r.clip.0;
            row.rider_layer = r.layer;
            row.rider_rate = r.rate;
        }
        if m.state().mount_drawn()
            && let Some(r) = c.mount.shown
        {
            row.mount_clip = r.clip.0;
            row.mount_layer = r.layer;
            row.mount_rate = r.rate;
        }
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::gameplay::mounts::MountMesh;

    fn mesh(offset: [f32; 3], scale: f32, parent: bool) -> MountMesh {
        MountMesh {
            entity: Did(0x4700_0001),
            holding_location: 1,
            bone_offset: offset,
            scale: [scale; 3],
            scale_driven: false,
            use_parent_scale: parent,
            behavior: 0,
            table: Did(0x0700_0001),
            node_id: 1,
        }
    }

    /// `mounts.md` §14.13 "Placement": a River-hobbit (model scale 0.7), bone 0 at its
    /// reference height, `Mount_Connection` cancelling it.
    #[test]
    fn worked_example_placement() {
        let s = 0.7;
        let rider = Mat4::from_scale(Vec3::splat(s));
        let h = Mat4::from_translation(Vec3::new(0.0, -0.0075, -0.9953));
        let bone0 = Mat4::from_translation(Vec3::new(0.0, 0.0075, 0.9953));
        let m = mount_transform(rider, bone0 * h, &mesh([0.0; 3], 1.0, true), [s; 3]);
        let (scale, _, pos) = m.to_scale_rotation_translation();
        assert!(pos.length() < 1e-5, "at the feet: {pos}");
        assert!((scale - Vec3::splat(0.7)).abs().max_element() < 1e-5);
        // The medium war-steed: offset clip raises bone 0 by 0.1347 m and 0.065 m forward;
        // BoneOffset (0, 0.05, −0.135).
        let posed = Mat4::from_translation(Vec3::new(0.0, 0.0075 + 0.065, 0.9953 + 0.1347));
        let m = mount_transform(
            rider,
            posed * h,
            &mesh([0.0, 0.05, -0.135], 1.0, true),
            [s; 3],
        );
        let (_, _, pos) = m.to_scale_rotation_translation();
        assert!((pos.y - 0.115 * 0.7).abs() < 1e-4, "{pos}");
        assert!((pos.z + 0.0003 * 0.7).abs() < 1e-4, "{pos}");
        // Without UseParentScale the rider's scale does not reach the mount's size.
        let m = mount_transform(rider, bone0 * h, &mesh([0.0; 3], 1.03, false), [s; 3]);
        let (scale, _, _) = m.to_scale_rotation_translation();
        assert!((scale - Vec3::splat(1.03)).abs().max_element() < 1e-5);
        // Hostile values stay finite.
        let m = mount_transform(
            rider,
            Mat4::from_translation(Vec3::splat(f32::NAN)),
            &mesh([f32::NAN; 3], f32::NAN, true),
            [f32::INFINITY; 3],
        );
        assert!(m.is_finite());
    }
}
