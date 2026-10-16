// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's mount requests (`walker.md` §16.1–§16.9): checks, resolution against the
//! install on a worker thread, the evaluation service, the mover on the physics walker, the
//! character layer's view, the camera offsets, the HUD line, the chat lines and the log row.
//!
//! **One request at a time** (Tessera rule): while a request is being resolved or evaluated
//! another is refused ("A mount request is pending."); `/ride off`, the toggle, fly mode and
//! a teleport cancel it. Every worker thread stops at its next stage once its request is
//! dropped, and at most [`MAX_WORKERS`] threads run at once. Worker threads of their own (not
//! the asset workers): a resolution reads the gameplay store for hundreds of milliseconds,
//! which would hold up the figure jobs.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tessera_net::Body;
use tessera_physics::controller::MovementState;

use super::eval::KeyClips;
use super::profile::{
    AreaAt, MountData, MountProfile, MountRefusal, WalkerMountPids, area_allows, resolve_mount,
    rider_body,
};
use super::ride::MountMover;
use super::spec::{MountSource, MountSpec, MountTiming, gait_label};
use super::state::{MountEvent, MountPhase};
use super::{MountService, MountSetup, mount_row};
use crate::commands::{LineKind, LocalLine};
use crate::walker::characters::CharacterLayer;
use crate::walker::characters::figure::{FigureContext, FigureRequest};
use crate::walker::characters::mounted::PlayerMountView;
use crate::walker::framelog::MountRow;
use crate::walker::movement::PhysicsWalker;

/// How long a refusal or interruption stays on the HUD (`walker.md` §16.9).
pub const NOTICE_TIME: Duration = Duration::from_secs(5);
/// Most messages kept for the chat between frames.
pub const MAX_MESSAGES: usize = 32;
/// Most mount worker threads alive at once (resolution, evaluation, listing).
pub const MAX_WORKERS: usize = 3;
/// `/ride list` lines per page (`walker.md` §16.3).
pub const LIST_PAGE: usize = 10;

/// A figure request for `body` with nothing else (the rider of an evaluation, the mount).
#[must_use]
pub fn plain_request(body: Body) -> FigureRequest {
    FigureRequest {
        body,
        equipment: Vec::new(),
        instance: None,
        instance_key: None,
        static_only: false,
        avatar_height: None,
        bone_scale: None,
        seed: None,
        chargen_stage: None,
        chargen_action: None,
        class: None,
        always_selectable: false,
    }
}

/// Facts about the walker the checks need (`walker.md` §16.1, §16.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Context {
    /// A `--server` session.
    pub server: bool,
    /// Fly mode.
    pub fly: bool,
    /// A scenario session: the feed owns the mount state (`walker.md` §16.1).
    pub scenario: bool,
    /// The area at the feet (`walker.md` §16.5).
    pub area: AreaAt,
}

type Resolved = Result<Arc<MountProfile>, MountRefusal>;
/// The mount data, opened on the first request (or why it could not be).
type DataSlot = Option<Result<Arc<MountData>, String>>;

/// Counts a live worker thread; the count drops with it.
struct WorkerGuard(Arc<AtomicUsize>);

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

enum Stage {
    Resolving(Receiver<Resolved>, Arc<AtomicBool>),
    Evaluating(Box<MountService>),
}

/// A request being resolved or evaluated.
struct Pending {
    spec: MountSpec,
    summon: bool,
    stage: Stage,
}

impl Drop for Pending {
    fn drop(&mut self) {
        if let Stage::Resolving(_, cancel) = &self.stage {
            cancel.store(true, Ordering::Relaxed);
        }
    }
}

/// The walker's mount (module docs).
pub struct WalkerMount {
    /// `--mount`: mounted when the walk controller is first placed.
    choice: Option<MountSpec>,
    /// The last source mounted (for `/ride` without arguments), else `--mount`'s.
    remembered: Option<MountSpec>,
    dir: PathBuf,
    data: Arc<Mutex<DataSlot>>,
    rider: Option<Body>,
    pending: Option<Pending>,
    workers: Arc<AtomicUsize>,
    ride_service: Option<MountService>,
    setup: Option<MountSetup>,
    clips: Option<Arc<KeyClips>>,
    notice: Option<(String, Instant)>,
    messages: Vec<LocalLine>,
    listing: Option<Receiver<Vec<LocalLine>>>,
    last_feet: Option<[f64; 3]>,
    ground_speed: f32,
    server: bool,
    /// A scenario dismount waits to be sent as the feed's `dismount` (`walker.md` §16.1).
    feed_dismount: bool,
}

impl std::fmt::Debug for WalkerMount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalkerMount")
            .field("choice", &self.choice)
            .field("pending", &self.pending.is_some())
            .finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Opens the mount data once (shared by every request).
fn open_data(
    slot: &Mutex<DataSlot>,
    dir: &std::path::Path,
) -> Result<Arc<MountData>, MountRefusal> {
    let mut slot = lock(slot);
    if slot.is_none() {
        *slot = Some(
            MountData::open(dir)
                .map(Arc::new)
                .map_err(|e| e.to_string()),
        );
    }
    slot.clone()
        .unwrap_or_else(|| Err("not opened".into()))
        .map_err(MountRefusal::Unavailable)
}

/// One `/ride list` line: index, DID, type labels and the effect's name (`walker.md` §16.3).
#[must_use]
pub fn list_line(index: usize, m: &tessera_gamedata::mount::OfferedMount) -> String {
    match &m.mount {
        Some(info) => {
            let labels = if info.type_labels.is_empty() {
                "-".to_owned()
            } else {
                info.type_labels.join("+")
            };
            format!(
                "{index} {} {labels} {}",
                info.effect,
                info.name.as_deref().unwrap_or("(no name)")
            )
        }
        None => format!("{index} {} (no mount effect)", m.skill),
    }
}

impl WalkerMount {
    /// The mount of a walker over the install in `dir`, with `--mount`'s `choice`.
    #[must_use]
    pub fn new(choice: Option<MountSpec>, dir: PathBuf) -> Self {
        Self {
            choice,
            remembered: choice,
            dir,
            data: Arc::new(Mutex::new(None)),
            rider: None,
            pending: None,
            workers: Arc::new(AtomicUsize::new(0)),
            ride_service: None,
            setup: None,
            clips: None,
            notice: None,
            messages: Vec::new(),
            listing: None,
            last_feet: None,
            ground_speed: 0.0,
            server: false,
            feed_dismount: false,
        }
    }

    /// A `--server` session: every request is refused (`walker.md` §16.1).
    pub fn set_server(&mut self, on: bool) {
        self.server = on;
    }

    /// Whether this is a `--server` session.
    #[must_use]
    pub fn server(&self) -> bool {
        self.server
    }

    /// Sets the avatar's body (`None` until known).
    pub fn set_rider(&mut self, body: Option<Body>) {
        self.rider = body;
    }

    /// The source `/ride` without arguments mounts (`walker.md` §16.3).
    #[must_use]
    pub fn remembered(&self) -> Option<MountSpec> {
        self.remembered
    }

    /// Chat lines since the last call (refusals and interruptions as `Error`, listings and
    /// notices as `Log`, `walker.md` §16.3).
    pub fn take_messages(&mut self) -> Vec<LocalLine> {
        std::mem::take(&mut self.messages)
    }

    /// Takes a scenario dismount to send as the feed's `dismount` (`walker.md` §16.1).
    pub fn take_feed_dismount(&mut self) -> bool {
        std::mem::take(&mut self.feed_dismount)
    }

    fn post(&mut self, kind: LineKind, text: String) {
        if self.messages.len() < MAX_MESSAGES {
            self.messages.push(LocalLine {
                kind,
                text,
                mask: None,
            });
        }
    }

    fn say(&mut self, text: String, now: Instant) {
        tracing::info!("mount: {text}");
        self.post(LineKind::Error, text.clone());
        self.notice = Some((text, now));
    }

    fn refuse(&mut self, r: &MountRefusal, now: Instant) {
        tracing::warn!("mount refused: {r}");
        self.say(r.to_string(), now);
    }

    /// Whether a request is being resolved or evaluated.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    /// Whether the `--mount` choice has not yet been applied (`walker.md` §16.12 resolution):
    /// while true the frame loop holds `/ride` dispatcher requests, so a `--command "/ride …"`
    /// acts after the initial `--mount` has been requested, not before (the observed
    /// `--command "/ride off"` with `--mount` now ends on foot).
    #[must_use]
    pub fn choice_pending(&self) -> bool {
        self.choice.is_some()
    }

    /// Starts a worker thread named `name` running `f`, unless [`MAX_WORKERS`] run already.
    fn spawn(&self, name: &str, f: impl FnOnce() + Send + 'static) -> Result<(), MountRefusal> {
        if self.workers.fetch_add(1, Ordering::Relaxed) >= MAX_WORKERS {
            self.workers.fetch_sub(1, Ordering::Relaxed);
            return Err(MountRefusal::Pending);
        }
        let guard = WorkerGuard(Arc::clone(&self.workers));
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                let _guard = guard;
                f();
            })
            .map(|_| ())
            .map_err(|e| MountRefusal::Unavailable(e.to_string()))
    }

    /// The checks before a request (`walker.md` §16.1, §16.3, §16.5) that need no data:
    /// the rider's species and sex when they pass.
    fn check(
        &self,
        physics: Option<&PhysicsWalker>,
        ctx: Context,
        from_feed: bool,
    ) -> Result<(u32, u32), MountRefusal> {
        if ctx.server || self.server {
            return Err(MountRefusal::Server);
        }
        if ctx.scenario && !from_feed {
            return Err(MountRefusal::Scenario);
        }
        if ctx.fly {
            return Err(MountRefusal::Fly);
        }
        if self.busy() {
            return Err(MountRefusal::Pending);
        }
        let p = physics.ok_or(MountRefusal::NoCollision)?;
        if p.mount()
            .is_some_and(|m| m.state().phase() != MountPhase::OnFoot)
        {
            return Err(MountRefusal::Busy);
        }
        if matches!(p.state().element.key(), 1..=3) {
            return Err(MountRefusal::InWater);
        }
        match self.rider {
            Some(Body::Character { species, sex }) => Ok((species, sex)),
            Some(_) => Err(MountRefusal::NotPlayable),
            None => Err(MountRefusal::Unavailable(
                "the avatar is not known yet".into(),
            )),
        }
    }

    /// A mount request (`walker.md` §16.2–§16.5): `summon_by_default` is the caller's
    /// default timing (the command line's is instant, the chat command's and the key's
    /// summon). Refusals are reported on the HUD and in [`WalkerMount::take_messages`].
    pub fn request(
        &mut self,
        spec: MountSpec,
        summon_by_default: bool,
        physics: Option<&PhysicsWalker>,
        figures: Option<&Arc<FigureContext>>,
        ctx: Context,
        now: Instant,
    ) {
        self.request_from(spec, summon_by_default, false, physics, figures, ctx, now);
    }

    #[allow(clippy::too_many_arguments)]
    fn request_from(
        &mut self,
        spec: MountSpec,
        summon_by_default: bool,
        from_feed: bool,
        physics: Option<&PhysicsWalker>,
        figures: Option<&Arc<FigureContext>>,
        ctx: Context,
        now: Instant,
    ) {
        let (species, sex) = match self.check(physics, ctx, from_feed) {
            Ok(b) => b,
            Err(e) => return self.refuse(&e, now),
        };
        if figures.is_none() {
            return self.refuse(
                &MountRefusal::Unavailable("character data not loaded".into()),
                now,
            );
        }
        let summon = match spec.timing {
            MountTiming::Default => summon_by_default,
            MountTiming::Summon => true,
            MountTiming::Instant => false,
        };
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        let data = Arc::clone(&self.data);
        let dir = self.dir.clone();
        let source = spec.source;
        let area = ctx.area;
        let spawned = self.spawn("tessera-mount-resolve", move || {
            let r = (|| -> Resolved {
                let d = open_data(&data, &dir)?;
                if flag.load(Ordering::Relaxed) {
                    return Err(MountRefusal::Unavailable("cancelled".into()));
                }
                let body =
                    rider_body(d.game_data(), species, sex).ok_or(MountRefusal::NotPlayable)?;
                let profile = resolve_mount(&d, &body, source)?;
                // §16.5: the area at the feet allows the type (`mounts.md` §4.7 steps 5–6).
                let pids = d
                    .game_data()
                    .registry()
                    .map(|r| WalkerMountPids::resolve(r.dictionary()))
                    .map_err(|e| MountRefusal::Unavailable(e.to_string()))?;
                area_allows(area, profile.mount_type, &pids, |did| d.object(did))?;
                Ok(Arc::new(profile))
            })();
            if !flag.load(Ordering::Relaxed) {
                let _ = tx.send(r);
            }
        });
        match spawned {
            Ok(()) => {
                self.pending = Some(Pending {
                    spec,
                    summon,
                    stage: Stage::Resolving(rx, cancel),
                });
            }
            Err(e) => self.refuse(&e, now),
        }
    }

    /// A request by effect (or skill) DID (`walker.md` §16.2 SOURCE `DID`): the scenario
    /// feed's `mount` step (`walker.md` §16.1), allowed in scenario sessions.
    pub fn request_effect(
        &mut self,
        did: u32,
        summon: bool,
        physics: Option<&PhysicsWalker>,
        figures: Option<&Arc<FigureContext>>,
        ctx: Context,
        now: Instant,
    ) {
        let spec = MountSpec {
            source: MountSource::Did(did),
            timing: if summon {
                MountTiming::Summon
            } else {
                MountTiming::Instant
            },
            lock: None,
        };
        self.request_from(spec, summon, true, physics, figures, ctx, now);
    }

    /// A dismount by the player (`walker.md` §16.3 `/ride off`): cancels summoning or a
    /// pending request, dismounts while mounting or mounted. In a scenario session a
    /// dismount while mounted is sent to the feed instead (§16.1).
    pub fn dismount(&mut self, physics: Option<&mut PhysicsWalker>, ctx: Context, now: Instant) {
        if self.pending.take().is_some() {
            self.post(LineKind::Log, "Mount request cancelled.".into());
            self.notice = Some(("Mount request cancelled.".into(), now));
        }
        let Some(p) = physics else { return };
        let moving = {
            let v = p.state().velocity;
            v[0].hypot(v[1]) > 0.05
        };
        if ctx.scenario
            && p.mount().is_some_and(|m| {
                matches!(
                    m.state().phase(),
                    MountPhase::Mounting | MountPhase::Mounted
                )
            })
        {
            self.feed_dismount = true;
            return;
        }
        if let Some(m) = p.mount_mut()
            && let Some(ev) = m.state_mut().dismount(true, moving)
        {
            self.say(ev.text().into(), now);
        }
    }

    /// `/ride` without arguments and the `toggle_mount` key (`walker.md` §16.3, §16.4).
    pub fn toggle(
        &mut self,
        physics: Option<&mut PhysicsWalker>,
        figures: Option<&Arc<FigureContext>>,
        ctx: Context,
        now: Instant,
    ) {
        let riding = physics.as_ref().is_some_and(|p| {
            p.mount()
                .is_some_and(|m| m.state().phase() != MountPhase::OnFoot)
        });
        if riding || self.busy() {
            self.dismount(physics, ctx, now);
        } else {
            let spec = self.remembered.unwrap_or(MountSpec::new(MountSource::Auto));
            let spec = MountSpec {
                timing: MountTiming::Summon,
                ..spec
            };
            self.request(spec, true, physics.as_deref(), figures, ctx, now);
        }
    }

    /// Fly mode, a teleport or the feed's `none` (`walker.md` §16.1, §16.5): a pending
    /// request and summoning are cancelled ("Summoning interrupted."); mounting and mounted
    /// dismount by other at once.
    pub fn interrupt(&mut self, physics: Option<&mut PhysicsWalker>, now: Instant) {
        let mut interrupted = self.pending.take().is_some();
        if let Some(m) = physics.and_then(PhysicsWalker::mount_mut)
            && let Some(ev) = m.state_mut().interrupt()
        {
            interrupted |= ev == MountEvent::SummonInterrupted;
        }
        if interrupted {
            self.say(MountEvent::SummonInterrupted.text().into(), now);
        }
    }

    /// `/ride list PAGE` (`walker.md` §16.3): the body's offered mounts, [`LIST_PAGE`] per
    /// page, posted as `Log` lines once read (on a worker thread).
    pub fn list(&mut self, page: u16, now: Instant) {
        let Some(Body::Character { species, sex }) = self.rider else {
            return self.refuse(&MountRefusal::NotPlayable, now);
        };
        if self.listing.is_some() {
            return self.refuse(&MountRefusal::Pending, now);
        }
        let (tx, rx) = channel();
        let data = Arc::clone(&self.data);
        let dir = self.dir.clone();
        let spawned = self.spawn("tessera-mount-list", move || {
            let lines = (|| -> Result<Vec<LocalLine>, MountRefusal> {
                let d = open_data(&data, &dir)?;
                let body =
                    rider_body(d.game_data(), species, sex).ok_or(MountRefusal::NotPlayable)?;
                d.with_presenter(|p| {
                    let offered = p.offered(body.species, body.tall);
                    let first = usize::from(page.max(1) - 1) * LIST_PAGE;
                    let lines: Vec<LocalLine> = offered
                        .iter()
                        .enumerate()
                        .skip(first)
                        .take(LIST_PAGE)
                        .map(|(i, m)| LocalLine {
                            kind: LineKind::Log,
                            text: list_line(i, m),
                            mask: None,
                        })
                        .collect();
                    if lines.is_empty() {
                        vec![LocalLine {
                            kind: LineKind::Log,
                            text: "No more mounts.".into(),
                            mask: None,
                        }]
                    } else {
                        lines
                    }
                })
            })();
            let _ = tx.send(lines.unwrap_or_else(|e| vec![LocalLine::error(e.to_string())]));
        });
        match spawned {
            Ok(()) => self.listing = Some(rx),
            Err(e) => self.refuse(&e, now),
        }
    }

    /// Installs an evaluated request on the physics walker (the checks that can change
    /// while it was evaluated run again, `walker.md` §16.1); the source becomes the
    /// remembered one.
    #[allow(clippy::too_many_arguments)]
    fn install(
        &mut self,
        spec: MountSpec,
        summon: bool,
        profile: Arc<MountProfile>,
        setup: MountSetup,
        physics: Option<&mut PhysicsWalker>,
        ctx: Context,
        now: Instant,
    ) -> bool {
        if ctx.fly {
            self.refuse(&MountRefusal::Fly, now);
            return false;
        }
        let Some(p) = physics else {
            self.refuse(&MountRefusal::NoCollision, now);
            return false;
        };
        if p.mount()
            .is_some_and(|m| m.state().phase() != MountPhase::OnFoot)
        {
            self.refuse(&MountRefusal::Busy, now);
            return false;
        }
        if matches!(p.state().element.key(), 1..=3) {
            self.refuse(&MountRefusal::InWater, now);
            return false;
        }
        let mover =
            MountMover::new(profile, setup.timeline, summon, spec.lock).with_jump(setup.jump);
        p.set_mount(Some(mover));
        self.remembered = Some(MountSpec {
            timing: MountTiming::Default,
            ..spec
        });
        self.setup = Some(setup);
        self.clips = None;
        true
    }

    /// Per frame, after the controls ran (`walker.md` §16.8): starts `--mount` once the
    /// walk controller is placed, collects resolutions and evaluations, puts the mover on
    /// the physics walker, follows its events, chooses the clips of the state key and
    /// gives the character layer its view. Returns the camera offsets (forward, up).
    pub fn frame(
        &mut self,
        physics: Option<&mut PhysicsWalker>,
        characters: &mut CharacterLayer,
        ctx: Context,
        now: Instant,
        dt: f32,
    ) -> [f32; 2] {
        let figures = characters.figure_context();
        let mut physics = physics;
        if let Some(c) = self.choice
            && physics.is_some()
            && self.rider.is_some()
            && figures.is_some()
        {
            self.choice = None;
            self.request(c, false, physics.as_deref(), figures.as_ref(), ctx, now);
        }
        if let Some(rx) = &self.listing {
            match rx.try_recv() {
                Ok(lines) => {
                    self.listing = None;
                    for l in lines {
                        self.post(l.kind, l.text);
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.listing = None,
            }
        }
        self.poll_pending(physics.as_deref_mut(), figures.as_ref(), ctx, now);
        // The ride.
        let mut offsets = [0.0; 2];
        let Some(p) = physics else {
            characters.set_player_mount(None, now);
            return offsets;
        };
        for ev in p.take_mount_events() {
            self.say(ev.text().into(), now);
            if ev == MountEvent::WaterDismount {
                tracing::info!("mount: dismounted in water (by other)");
            }
        }
        let feet = p.state().position;
        self.ground_speed = self.last_feet.map_or(0.0, |l| {
            #[allow(clippy::cast_possible_truncation)]
            let d = (feet[0] - l[0]).hypot(feet[1] - l[1]) as f32;
            if dt > 0.0 && dt.is_finite() {
                d / dt
            } else {
                0.0
            }
        });
        self.last_feet = Some(feet);
        let Some(m) = p.mount() else {
            self.ride_service = None;
            self.setup = None;
            self.clips = None;
            characters.set_player_mount(None, now);
            return offsets;
        };
        let key = m.clip_key();
        if let Some(svc) = &mut self.ride_service {
            svc.poll();
            if let Some(c) = svc.clips(&key) {
                self.clips = Some(c);
            }
        }
        let profile = m.profile();
        if m.state().is_mounted()
            && let Some([y, z]) = profile.camera
        {
            offsets = [y, z];
        }
        if let Some(setup) = &self.setup {
            let riding = profile.rides()
                && matches!(
                    m.state().phase(),
                    MountPhase::Mounting | MountPhase::Mounted | MountPhase::Dismounting
                );
            let summoning = m.state().phase() == MountPhase::Summoning;
            let clips = self.clips.as_ref();
            let view = PlayerMountView {
                figure: Some(plain_request(Body::Object(tessera_net::DataId(
                    setup.mesh.mesh.entity.0,
                )))),
                mesh: setup.mesh.mesh.clone(),
                rider_model_scale: setup.rider_model_scale,
                alpha: m.state().mount_alpha(),
                drawn: m.state().mount_drawn(),
                rider_clips: (riding || summoning)
                    .then(|| clips.map(|c| c.rider.layers.clone()))
                    .flatten(),
                mount_clips: clips.map(|c| c.mount.layers.clone()).unwrap_or_default(),
                rider_shown: clips.and_then(|c| c.rider.shown),
                mount_shown: clips.and_then(|c| c.mount.shown),
                back: key.propel < 0,
            };
            characters.set_player_mount(Some(view), now);
        }
        offsets
    }

    /// Collects the pending request's resolution or evaluation.
    fn poll_pending(
        &mut self,
        physics: Option<&mut PhysicsWalker>,
        figures: Option<&Arc<FigureContext>>,
        ctx: Context,
        now: Instant,
    ) {
        let Some(mut pending) = self.pending.take() else {
            return;
        };
        match &mut pending.stage {
            Stage::Resolving(rx, _) => match rx.try_recv() {
                Ok(Ok(profile)) => {
                    for n in &profile.notes {
                        tracing::info!(effect = %profile.effect, "mount: {n}");
                    }
                    match (figures, self.rider) {
                        (Some(f), Some(body)) => {
                            if self.workers.load(Ordering::Relaxed) >= MAX_WORKERS {
                                // Wait for a cancelled thread to finish.
                                self.refuse(&MountRefusal::Pending, now);
                                return;
                            }
                            match MountService::start(
                                Arc::clone(&f.catalog),
                                f.assembly,
                                plain_request(body),
                                profile,
                            ) {
                                Ok(s) => {
                                    pending.stage = Stage::Evaluating(Box::new(s));
                                    self.pending = Some(pending);
                                }
                                Err(e) => self.refuse(&e, now),
                            }
                        }
                        _ => self.refuse(
                            &MountRefusal::Unavailable("character data not loaded".into()),
                            now,
                        ),
                    }
                }
                Ok(Err(e)) => self.refuse(&e, now),
                Err(TryRecvError::Empty) => self.pending = Some(pending),
                Err(TryRecvError::Disconnected) => {
                    self.refuse(&MountRefusal::Unavailable("resolution stopped".into()), now);
                }
            },
            Stage::Evaluating(svc) => {
                svc.poll();
                match svc.setup().cloned() {
                    None => self.pending = Some(pending),
                    Some(Err(e)) => self.refuse(&e, now),
                    Some(Ok(setup)) => {
                        let profile = Arc::clone(svc.profile());
                        let (spec, summon) = (pending.spec, pending.summon);
                        let Stage::Evaluating(svc) = std::mem::replace(
                            &mut pending.stage,
                            Stage::Resolving(channel().1, Arc::default()),
                        ) else {
                            return;
                        };
                        if self.install(spec, summon, profile, setup, physics, ctx, now) {
                            self.ride_service = Some(*svc);
                        }
                    }
                }
            }
        }
    }

    /// The mount columns of the frame log (`walker.md` §16.10).
    #[must_use]
    pub fn log_row(&self, physics: Option<&PhysicsWalker>) -> MountRow {
        physics.map_or_else(MountRow::default, |p| {
            mount_row(p, self.clips.as_deref(), self.ground_speed)
        })
    }

    /// The HUD line (`walker.md` §16.9), when a mount is involved or a notice is shown:
    /// `mount <state> <name> <DID> <type labels> | gait <key> <label> lock <label or -> |
    /// <walk or run> | <ground speed> m/s | rider <clip DID> mount <clip DID>`.
    #[must_use]
    pub fn hud_line(&self, physics: Option<&PhysicsWalker>, now: Instant) -> Option<String> {
        if let Some((text, at)) = &self.notice
            && now.saturating_duration_since(*at) < NOTICE_TIME
        {
            return Some(format!("mount: {text}"));
        }
        if self.busy() {
            return Some("mount: preparing…".into());
        }
        let p = physics?;
        let m = p.mount()?;
        let st = m.state();
        let state = match st.phase() {
            MountPhase::Summoning => format!("summoning {:.1}s", st.summon_left()),
            ph => ph.word().to_owned(),
        };
        let profile = m.profile();
        let name = profile
            .info
            .name
            .clone()
            .unwrap_or_else(|| profile.effect.to_string());
        let labels = if profile.info.type_labels.is_empty() {
            "-".to_owned()
        } else {
            profile.info.type_labels.join("+")
        };
        let row = self.log_row(Some(p));
        Some(hud_text(
            &state,
            &name,
            &profile.effect.to_string(),
            &labels,
            profile.in_group,
            &row,
        ))
    }

    /// Whether the rider is in water now (for the HUD and tests).
    #[must_use]
    pub fn in_water(physics: &PhysicsWalker) -> bool {
        matches!(
            physics.state().element,
            MovementState::WaterControlledSurface
                | MovementState::WaterNoSurface
                | MovementState::WaterUncontrolledSurface
        )
    }
}

/// The HUD line of `walker.md` §16.9 from its parts and the log row (gait and lock with
/// their `MountedMovementSpeed` labels, the ground speed).
#[must_use]
pub fn hud_text(
    state: &str,
    name: &str,
    effect: &str,
    labels: &str,
    in_group: bool,
    row: &MountRow,
) -> String {
    let gait = match gait_label(row.gait) {
        Some(l) => format!("{} {l}", row.gait),
        None => row.gait.to_string(),
    };
    let lock = gait_label(row.locked_gear).unwrap_or("-");
    let group = if in_group {
        ""
    } else {
        " (not this body's group)"
    };
    format!(
        "mount {state} {name} {effect} {labels}{group} | gait {gait} lock {lock} | {} | {:.2} m/s | rider {:#x} mount {:#x}",
        if row.style == 1 { "walk" } else { "run" },
        row.ground_speed,
        row.rider_clip,
        row.mount_clip,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walker::mount::profile::{MountKind, WarSteedSetup};
    use crate::walker::mount::state::Timeline;
    use crate::walker::movement::run::StaticWorld;
    use crate::walker::movement::setup::MoverSetup;
    use tessera_formats::Did;
    use tessera_formats::landblock::HeightGrid;
    use tessera_formats::{LandblockData, LandblockId};
    use tessera_gamedata::mount::{GaitTable, MountCategory, MountClass, MountInfo};
    use tessera_physics::controller::MovementCommands;
    use tessera_physics::geometry::{CollisionLimits, CollisionSetBuilder, GridMesh};
    use tessera_world::LandblockKey;

    fn world() -> (StaticWorld, [f64; 3]) {
        let raw = (10.0_f32 * 65_535.0 / 2_000.0).round() as u16;
        let mut w = StaticWorld::default();
        let c = LandblockKey::new(1, 100, 100).unwrap();
        for dy in -1..=1 {
            for dx in -1..=1 {
                let k = c.offset(dx, dy).unwrap();
                let data =
                    LandblockData::from_heights(k.data_did(), HeightGrid::from_fn(|_, _| raw));
                let mut b = CollisionSetBuilder::new(CollisionLimits::default());
                b.set_landblock(LandblockId::from_did(k.data_did()).unwrap());
                b.add_grid(Arc::new(GridMesh::terrain(&data))).unwrap();
                w.sets.insert(k, Arc::new(b.build()));
            }
        }
        let o = c.origin();
        let z = f64::from(raw) * 2_000.0 / 65_535.0 + 0.01;
        (w, [o[0] + 96.0, o[1] + 96.0, z])
    }

    fn walker() -> PhysicsWalker {
        let (w, feet) = world();
        let mut p = PhysicsWalker::new(&MoverSetup::builtin(), 1, feet, 0.0);
        p.check_placement();
        for _ in 0..10 {
            p.step_commands(&MovementCommands::default(), false, None, false, &w);
        }
        p
    }

    fn profile() -> Arc<MountProfile> {
        Arc::new(MountProfile {
            effect: Did(0x7000_0001),
            skill: None,
            info: MountInfo {
                effect: Did(0x7000_0001),
                skill: None,
                name: Some("Test Horse".into()),
                icon: None,
                mount_type: Some(1),
                type_labels: vec!["Horse".into()],
                war_steed: false,
                class: MountClass::Basic,
                category: MountCategory::Normal,
                entity: Some(Did(0x4700_0001)),
                scale: None,
            },
            mount_type: 1,
            kind: MountKind::Normal,
            in_group: true,
            entries: Vec::new(),
            speed_factor: 1.62,
            war_steed: None::<WarSteedSetup>,
            gaits: GaitTable::default(),
            induction: None,
            start_script: None,
            dismount_by_player: None,
            camera: None,
            notes: Vec::new(),
        })
    }

    fn setup() -> MountSetup {
        MountSetup {
            mesh: super::super::eval::MeshChoice {
                mesh: tessera_formats::gameplay::mounts::MountMesh {
                    entity: Did(0x4700_0001),
                    holding_location: 1,
                    bone_offset: [0.0; 3],
                    scale: [1.0; 3],
                    scale_driven: false,
                    use_parent_scale: true,
                    behavior: 0,
                    table: Did(0x0700_0001),
                    node_id: 1,
                },
                fade_in: 0.25,
                fade_out: 0.25,
            },
            timeline: Timeline::default(),
            rider_model_scale: [1.0; 3],
            jump: crate::walker::mount::ride::MountedJump::default(),
            mount_animated: true,
            notes: Vec::new(),
        }
    }

    fn mount() -> WalkerMount {
        let mut m = WalkerMount::new(None, PathBuf::from("/nonexistent"));
        m.set_rider(Some(Body::Character { species: 1, sex: 1 }));
        m
    }

    fn texts(m: &mut WalkerMount) -> Vec<String> {
        m.take_messages().into_iter().map(|l| l.text).collect()
    }

    /// `walker.md` §16.1, §16.5: the checks in order, with their texts.
    #[test]
    fn requests_are_checked_before_resolution() {
        let now = Instant::now();
        let p = walker();
        let spec = MountSpec::new(MountSource::Auto);
        let mut m = mount();
        let run = |m: &mut WalkerMount, p: Option<&PhysicsWalker>, ctx: Context| {
            m.request(spec, true, p, None, ctx, now);
            texts(m)
        };
        let server = Context {
            server: true,
            ..Context::default()
        };
        assert_eq!(
            run(&mut m, Some(&p), server),
            ["Mounts are not available with --server."]
        );
        let scenario = Context {
            scenario: true,
            ..Context::default()
        };
        assert_eq!(
            run(&mut m, Some(&p), scenario),
            ["The scenario controls mounts in this session."]
        );
        let fly = Context {
            fly: true,
            ..Context::default()
        };
        assert_eq!(
            run(&mut m, Some(&p), fly),
            ["Mounts are not available in fly mode."]
        );
        assert_eq!(
            run(&mut m, None, Context::default()),
            ["Mounts need the collision walker."]
        );
        // A figure DID cannot ride.
        let mut fig = WalkerMount::new(None, PathBuf::from("/nonexistent"));
        fig.set_rider(Some(Body::Object(tessera_net::DataId(0x7000_0002))));
        assert_eq!(
            run(&mut fig, Some(&p), Context::default()),
            ["This figure cannot ride."]
        );
        // Mounted already.
        let mut q = walker();
        q.set_mount(Some(MountMover::new(
            profile(),
            Timeline::default(),
            false,
            None,
        )));
        assert_eq!(
            run(&mut m, Some(&q), Context::default()),
            ["Already mounted. Use /ride off first."]
        );
        // Nothing pending after refusals; the HUD shows the last reason.
        assert!(!m.busy());
        assert_eq!(
            m.hud_line(Some(&p), now).as_deref(),
            Some("mount: Already mounted. Use /ride off first.")
        );
    }

    /// `walker.md` §16.11 check 5 (the session's side): the feet in water refuse.
    #[test]
    fn water_refuses() {
        let now = Instant::now();
        let mut p = walker();
        let mut s = *p.state();
        s.element = MovementState::WaterNoSurface;
        assert!(p.controller_mut().set_state(s));
        let mut m = mount();
        m.request(
            MountSpec::new(MountSource::Auto),
            true,
            Some(&p),
            None,
            Context::default(),
            now,
        );
        assert_eq!(texts(&mut m), ["You cannot mount in water."]);
    }

    /// `walker.md` §16.3: the remembered source is the last one *mounted*; a failed request
    /// leaves it; the install re-checks fly mode and the physics walker.
    #[test]
    fn remembered_source_and_install_checks() {
        let now = Instant::now();
        let steed = MountSpec {
            source: MountSource::Dir(3),
            timing: MountTiming::Summon,
            lock: Some(6),
        };
        let mut m = WalkerMount::new(Some(MountSpec::new(MountSource::Auto)), PathBuf::new());
        assert_eq!(m.remembered(), Some(MountSpec::new(MountSource::Auto)));
        let mut p = walker();
        let fly = Context {
            fly: true,
            ..Context::default()
        };
        assert!(!m.install(steed, true, profile(), setup(), Some(&mut p), fly, now));
        assert_eq!(texts(&mut m), ["Mounts are not available in fly mode."]);
        assert!(!m.install(
            steed,
            true,
            profile(),
            setup(),
            None,
            Context::default(),
            now
        ));
        assert_eq!(texts(&mut m), ["Mounts need the collision walker."]);
        assert_eq!(
            m.remembered(),
            Some(MountSpec::new(MountSource::Auto)),
            "unchanged"
        );
        assert!(m.install(
            steed,
            false,
            profile(),
            setup(),
            Some(&mut p),
            Context::default(),
            now
        ));
        assert_eq!(
            m.remembered(),
            Some(MountSpec {
                timing: MountTiming::Default,
                ..steed
            })
        );
        assert_eq!(
            p.mount().map(|x| x.state().phase()),
            Some(MountPhase::Mounted)
        );
        // A second install while mounted is refused as busy.
        assert!(!m.install(
            steed,
            false,
            profile(),
            setup(),
            Some(&mut p),
            Context::default(),
            now
        ));
        assert_eq!(texts(&mut m), ["Already mounted. Use /ride off first."]);
        // In a scenario session the player's dismount goes to the feed; nothing changes.
        let scenario = Context {
            scenario: true,
            ..Context::default()
        };
        m.dismount(Some(&mut p), scenario, now);
        assert!(m.take_feed_dismount());
        assert_eq!(
            p.mount().map(|x| x.state().phase()),
            Some(MountPhase::Mounted)
        );
        // Fly mode (an interrupt) dismounts by other.
        m.interrupt(Some(&mut p), now);
        assert_eq!(
            p.mount().map(|x| x.state().phase()),
            Some(MountPhase::OnFoot)
        );
    }

    /// `walker.md` §16.9: the HUD line's gait and lock labels and the ground speed.
    #[test]
    fn hud_line_follows_the_spec() {
        let row = MountRow {
            gait: 7,
            locked_gear: 6,
            style: 2,
            ground_speed: 8.3,
            control_speed: 16.0,
            rider_clip: 0x0500_0001,
            mount_clip: 0x0500_0002,
            ..MountRow::default()
        };
        assert_eq!(
            hud_text(
                "mounted",
                "Steed",
                "0x70000001",
                "Warsteed_Medium",
                true,
                &row
            ),
            "mount mounted Steed 0x70000001 Warsteed_Medium | gait 7 Gallop lock Canter | run | 8.30 m/s | rider 0x5000001 mount 0x5000002"
        );
        let row = MountRow {
            gait: 2,
            style: 1,
            ..MountRow::default()
        };
        assert!(
            hud_text("dismounting", "H", "0x1", "-", false, &row)
                .contains("(not this body's group) | gait 2 Neutral lock - | walk | 0.00 m/s")
        );
    }
}
