// SPDX-License-Identifier: GPL-3.0-or-later
//! The character view ("Character" tab, M4): a character viewer and an animation browser.
//!
//! - **Character viewer**: choose a playable body (species and sex from the race table,
//!   `appearance.md` §2.2), or open any 0x47 entity, 0x70–0x76 weenie object or 0x04
//!   setup; wear and hold items with dye codes and wield state; bone sliders and the
//!   height control. The figure is assembled by `tessera-mesh` on a worker
//!   ([`crate::character::build_character`]), skinned by `tessera-render`'s skinned
//!   pipeline and animated by `tessera-anim`.
//! - **Animation browser**: every 0x05 clip with duration, tracks and kind
//!   ([`crate::character::ClipCatalog`]), filtered to those that fit the figure's skeleton;
//!   play, pause, scrub and loop on the figure, with a bone overlay and the root-motion
//!   trails; the clips the figure's script tables select for idle, walk and run.
//!
//! Selecting a record while the view is shown changes it: a 0x47, 0x04 or non-item
//! 0x70–0x76 record becomes the figure, a 0x05 clip plays, a 0x79 (or item 0x70–0x76)
//! record is previewed on the figure.
//!
//! # Threads
//!
//! Figures, clips and the clip catalogue are built by [`Job`]s on the asset workers. Poses
//! are sampled by an [`AnimationScheduler`] on the same workers (one job per frame,
//! double-buffered, `skeleton-anim.md` §10); the UI thread only advances play time,
//! submits, and hands the last finished pose to the render thread ([`PoseDraw`]).

use std::sync::Arc;
use std::time::Instant;

use eframe::egui::{self, Color32, Rect, RichText, Ui, pos2, vec2};
use tessera_anim::glam::{Mat4, Quat, Vec3};
use tessera_anim::{
    AnimationScheduler, Animator, InstanceId, MotionDelta, Playback, RootMotionMode,
    SchedulerConfig, Skeleton, Submitted, Wrap, clip_time, motion_between,
};
use tessera_assets::Priority;
use tessera_formats::Did;
use tessera_render::MeshShading;

use crate::app::{Current, Loaded};
use crate::character::{
    Catalog, CharacterProduct, CharacterRequest, ClipCatalog, ClipProduct, Figure, FigurePart, Job,
    build_catalog, build_character, build_clip_catalog, load_clip,
};
use crate::charspec::{
    CharacterArgs, ClipArg, FigureArg, ItemArg, MAX_DYE_CODE, MAX_ITEMS, MAX_TIME, SpeciesArg,
    StateArg, parse_item,
};
use crate::mesh_view::MeshSettings;
use crate::viewport::{DrawRequest, DyeOverride, OrbitView, PoseDraw, Viewport, fit_viewport};
use crate::views::Action;

/// Longest step of play time per UI frame (s); longer gaps (a stalled window) do not jump.
pub const MAX_STEP: f32 = 0.1;
/// Rows of the clip browser shown at once before scrolling.
const CLIP_ROWS: f32 = 14.0;

/// Colours of the overlays.
const BONE_COLOR: [f32; 4] = [0.2, 0.95, 0.4, 1.0];
const ROOT_TRAIL_COLOR: [f32; 4] = [0.3, 0.75, 1.0, 1.0];
const MOTION_TRAIL_COLOR: [f32; 4] = [1.0, 0.8, 0.2, 1.0];
/// Colour of warnings (problems that do not stop the view).
const WARN_COLOR: Color32 = Color32::from_rgb(0xE0, 0xB0, 0x60);

/// The clip to play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClipChoice {
    /// The reference pose.
    None,
    /// The clip the script tables select for a motion state.
    State(StateArg),
    /// A 0x05 clip.
    Did(u32),
}

impl From<ClipArg> for ClipChoice {
    fn from(a: ClipArg) -> Self {
        match a {
            ClipArg::None => Self::None,
            ClipArg::State(s) => Self::State(s),
            ClipArg::Did(d) => Self::Did(d),
        }
    }
}

impl From<ClipChoice> for ClipArg {
    fn from(c: ClipChoice) -> Self {
        match c {
            ClipChoice::None => Self::None,
            ClipChoice::State(s) => Self::State(s),
            ClipChoice::Did(d) => Self::Did(d),
        }
    }
}

/// What a selected record does to the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seed {
    /// Nothing (another record type).
    Nothing,
    /// Becomes the figure.
    Figure(SeedFigure),
    /// Plays.
    Clip(u32),
    /// Is previewed on the figure.
    Item(u32),
    /// A 0x70–0x76 object: item or figure, decided on a worker.
    Object(u32),
}

/// A figure DID ([`FigureArg`] without the playable case, which is `Copy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedFigure {
    /// A 0x47 entity or a 0x70–0x76 object.
    Object(u32),
    /// A 0x04 setup.
    Setup(u32),
}

impl Seed {
    /// The effect of selecting `did` (`seed` of the module documentation).
    #[must_use]
    pub fn of(did: u32) -> Self {
        match did >> 24 {
            0x47 => Self::Figure(SeedFigure::Object(did)),
            0x04 => Self::Figure(SeedFigure::Setup(did)),
            0x05 => Self::Clip(did),
            0x79 => Self::Item(did),
            0x70..=0x76 => Self::Object(did),
            _ => Self::Nothing,
        }
    }
}

/// Playback state.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayState {
    /// Advancing with real time.
    pub playing: bool,
    /// Play time τ (s).
    pub time: f32,
    /// Speed factor on top of the clip's rate.
    pub speed: f32,
    /// Loop (else play once and hold the last frame).
    pub looping: bool,
    /// Move the figure by the clip's extracted motion (`skeleton-anim.md` §7.3).
    pub root_motion: bool,
    /// Accumulated placement from root motion: (position, yaw in radians).
    pub placement: (Vec3, f32),
}

impl Default for PlayState {
    fn default() -> Self {
        Self {
            playing: true,
            time: 0.0,
            speed: 1.0,
            looping: true,
            root_motion: false,
            placement: (Vec3::ZERO, 0.0),
        }
    }
}

impl PlayState {
    /// Advances play time by `dt` real seconds for a clip of `duration` played at `rate`;
    /// returns the play time before the step, in the same cycle count as the new time (when
    /// a looping τ is wrapped back by whole cycles, the returned time is shifted by the same
    /// amount, so root motion over the step stays one step's worth).
    pub fn advance(&mut self, dt: f32, rate: f32, duration: f32) -> f32 {
        let mut before = self.time;
        if !self.playing {
            return before;
        }
        let dt = if dt.is_finite() {
            dt.clamp(0.0, MAX_STEP)
        } else {
            0.0
        };
        let rate = if rate.is_finite() { rate } else { 1.0 };
        let speed = if self.speed.is_finite() {
            self.speed
        } else {
            1.0
        };
        let mut t = self.time + dt * rate * speed;
        if !(duration.is_finite() && duration > 0.0) {
            t = 0.0;
        } else if self.looping {
            // Keep τ within a few cycles so f32 time stays precise; root motion uses
            // motion_between, which counts whole cycles, so `before` moves back by the same
            // whole cycles.
            if t > duration * 1024.0 || t < -duration * 1024.0 {
                let shift = t - t.rem_euclid(duration);
                t -= shift;
                before -= shift;
            }
        } else {
            t = t.clamp(0.0, duration);
            if t >= duration {
                self.playing = false;
            }
        }
        self.time = if t.is_finite() { t } else { 0.0 };
        if before.is_finite() {
            before
        } else {
            self.time
        }
    }

    /// Moves the placement by extracted motion `m` (`skeleton-anim.md` §7.3) of a figure
    /// drawn at model scale `scale`: the motion was removed from the bones, which are drawn
    /// scaled, so its translation is scaled too (and the feet do not slide).
    pub fn apply_motion(&mut self, m: MotionDelta, scale: f32) {
        let (p, yaw) = self.placement;
        let np = p + Quat::from_rotation_z(yaw) * (m.translation * usable_scale(scale));
        let (axis, angle) = m.rotation.to_axis_angle();
        let dyaw = if axis.z < 0.0 { -angle } else { angle };
        if np.is_finite() && dyaw.is_finite() {
            self.placement = (np, yaw + dyaw);
        }
    }

    /// The clip time shown for play time τ.
    #[must_use]
    pub fn clip_time(&self, duration: f32) -> f32 {
        clip_time(self.time, duration, self.wrap())
    }

    /// The wrap mode.
    #[must_use]
    pub fn wrap(&self) -> Wrap {
        if self.looping { Wrap::Loop } else { Wrap::Once }
    }

    /// The figure's placement matrix with `scale` (position, yaw about +Z, then the
    /// uniform model scale at the origin, `appearance.md` §6.1).
    #[must_use]
    pub fn placement_matrix(&self, scale: f32) -> Mat4 {
        let (p, yaw) = self.placement;
        let s = usable_scale(scale);
        let m = Mat4::from_translation(p)
            * Mat4::from_quat(Quat::from_rotation_z(yaw))
            * Mat4::from_scale(Vec3::splat(s));
        if m.is_finite() { m } else { Mat4::IDENTITY }
    }
}

/// `scale` when it is a usable model scale, else 1.
fn usable_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

/// Where the trails of a clip start: the placement `placement` (model scale included) with
/// the cycle's motion so far `m` undone. With [`PlayState::apply_motion`] moving the
/// figure, the drawn extracted-motion trail starts where the cycle started and ends at the
/// figure.
#[must_use]
pub fn trail_base(placement: Mat4, m: MotionDelta) -> Mat4 {
    placement * Mat4::from_quat(m.rotation.inverse()) * Mat4::from_translation(-m.translation)
}

/// The clip time a command line names for play time `time`: the time within the clip
/// (`duration` > 0) as the view shows it, rounded to milliseconds, within `--time`'s range.
#[must_use]
pub fn command_line_time(time: f32, duration: f32, wrap: Wrap) -> f32 {
    let t = if duration.is_finite() && duration > 0.0 {
        clip_time(time, duration, wrap)
    } else {
        time
    };
    let t = ((t * 1000.0).round() / 1000.0).clamp(0.0, MAX_TIME);
    if t.is_finite() { t } else { 0.0 }
}

/// The pose drawn while an instance's first sample is still on the workers (the figure was
/// just rebuilt for new items or a detail level): the last pose drawn when it was sampled
/// on the same rig, else the reference pose. Either way at `placement` (model scale
/// included), and held parts whose location the pose lacks are not drawn.
#[must_use]
pub fn stand_in_pose(
    last: Option<&PoseDraw>,
    last_rig: Option<&Skeleton>,
    rig: &Skeleton,
    placement: Mat4,
) -> PoseDraw {
    let mut out = PoseDraw {
        placement,
        ..PoseDraw::default()
    };
    if let (Some(last), Some(r)) = (last, last_rig)
        && r.same_rig(rig)
    {
        out.palette.clone_from(&last.palette);
        out.attach.clone_from(&last.attach);
    }
    out
}

/// The animated instance: the animator and the scheduler slot of the shown figure.
struct Instance {
    /// The product whose skeleton it uses.
    product: Arc<CharacterProduct>,
    scheduler: AnimationScheduler,
    id: InstanceId,
    animator: Animator,
    /// Frame numbers submitted and wanted (`wanted > submitted` = a submit is pending).
    submitted: u64,
    wanted: u64,
    /// The clip bound in the animator.
    clip: Option<Arc<ClipProduct>>,
    /// Slider bytes applied.
    sliders: Vec<u8>,
    slider_problems: Vec<String>,
}

impl std::fmt::Debug for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Instance")
            .field("submitted", &self.submitted)
            .field("wanted", &self.wanted)
            .finish_non_exhaustive()
    }
}

/// State of the character view; it persists while records are browsed.
#[derive(Debug)]
pub struct CharacterState {
    /// The command-line options it started from.
    pub args: CharacterArgs,
    catalog: Option<Job<Catalog>>,
    clip_catalog: Option<Job<ClipCatalog>>,
    /// The figure (`None`: the default body once the catalogue is known).
    pub figure: Option<FigureArg>,
    /// Why the figure could not be resolved.
    pub figure_error: Option<String>,
    /// Worn and held items.
    pub items: Vec<ItemArg>,
    /// An item previewed from the selection.
    pub preview: Option<ItemArg>,
    /// Slider bytes chosen in the view (`None`: the avatar's). Applied to the animated
    /// instance only; changing them does not rebuild the figure.
    pub sliders: Option<Vec<u8>>,
    /// `AC_BoneScale_Test` chosen in the view (neutral in M4, `appearance.md` §6.2
    /// Decision; not applied and not part of the figure's request).
    pub bone_scale: Option<f32>,
    /// Detail level.
    pub lod: u8,
    job: Option<(CharacterRequest, Job<CharacterProduct>)>,
    /// The figure on screen.
    pub shown: Option<Arc<CharacterProduct>>,
    /// The clip to play.
    pub clip: ClipChoice,
    clip_job: Option<((u32, usize), Job<ClipProduct>)>,
    /// Playback.
    pub play: PlayState,
    /// A fixed play time (`--time`): playback stays paused there.
    pub fixed_time: Option<f32>,
    /// Bone overlay.
    pub bones: bool,
    /// Root-motion trails.
    pub trail: bool,
    instance: Option<Instance>,
    last_tick: Option<Instant>,
    /// The camera.
    pub orbit: OrbitView,
    framed: Option<u32>,
    reframe: bool,
    scene_id: Option<u64>,
    /// The record last applied as a seed.
    seeded: Option<u32>,
    object_seed: Option<(u32, Job<bool>)>,
    /// Text of the "add item" box.
    item_text: String,
    item_error: Option<String>,
    /// Clip browser filter.
    clip_filter: String,
    /// Clip browser: only clips that fit the skeleton.
    fit_only: bool,
    /// The pose sent last (to know when the render shows it).
    last_pose: Option<Arc<PoseDraw>>,
    /// The rig of the last pose sampled by the workers ([`stand_in_pose`]).
    last_rig: Option<Arc<Skeleton>>,
    /// No inspector panel (`--hide-panels`).
    pub compact: bool,
    /// The record selected when the view first showed: the command line's figure and clip
    /// win over its seed (later selections replace them).
    startup_did: Option<u32>,
}

impl Default for CharacterState {
    fn default() -> Self {
        Self::new(CharacterArgs::default(), false)
    }
}

impl CharacterState {
    /// State from the command line; `screenshot` pauses at `--time` (0 when not given).
    #[must_use]
    pub fn new(args: CharacterArgs, screenshot: bool) -> Self {
        let fixed_time = args.time.or(screenshot.then_some(0.0));
        let play = PlayState {
            playing: fixed_time.is_none(),
            time: fixed_time.unwrap_or(0.0),
            ..PlayState::default()
        };
        Self {
            catalog: None,
            clip_catalog: None,
            figure: args.figure.clone(),
            figure_error: None,
            items: args.items.clone(),
            preview: None,
            sliders: args.sliders.clone(),
            bone_scale: None,
            lod: 0,
            job: None,
            shown: None,
            clip: args
                .clip
                .map_or(ClipChoice::State(StateArg::Idle), Into::into),
            clip_job: None,
            play,
            fixed_time,
            bones: args.bones,
            trail: args.trail,
            instance: None,
            last_tick: None,
            orbit: OrbitView::new(),
            framed: None,
            reframe: false,
            scene_id: None,
            seeded: None,
            object_seed: None,
            item_text: String::new(),
            item_error: None,
            clip_filter: String::new(),
            fit_only: true,
            last_pose: None,
            last_rig: None,
            compact: false,
            startup_did: None,
            args,
        }
    }

    /// Forgets everything tied to the install (a new install was opened); the options
    /// chosen stay.
    pub fn reset_install(&mut self) {
        self.catalog = None;
        self.clip_catalog = None;
        self.job = None;
        self.shown = None;
        self.clip_job = None;
        self.instance = None;
        self.object_seed = None;
        self.seeded = None;
        self.framed = None;
        self.scene_id = None;
        self.last_pose = None;
        self.last_rig = None;
    }

    /// Applies the selected record `did` (see the module documentation), once per DID.
    /// The record selected when the view first shows does not replace the command line's
    /// figure or clip; every later one does.
    pub fn seed(&mut self, did: u32) {
        if self.seeded == Some(did) {
            return;
        }
        self.seeded = Some(did);
        let startup = self.startup_did.is_none();
        if startup {
            self.startup_did = Some(did);
        }
        match Seed::of(did) {
            Seed::Nothing => {}
            Seed::Figure(f) => {
                if self.cli_keeps_figure(did) {
                    return;
                }
                self.figure = Some(match f {
                    SeedFigure::Object(d) => FigureArg::Object(d),
                    SeedFigure::Setup(d) => FigureArg::Setup(d),
                });
                self.preview = None;
            }
            Seed::Clip(d) => {
                if startup && self.args.clip.is_some() {
                    return;
                }
                self.clip = ClipChoice::Did(d);
                self.restart();
            }
            Seed::Item(d) => self.preview_item(d),
            Seed::Object(_) => {
                // Decided on a worker (ensure_object_seed).
            }
        }
    }

    /// Whether selecting `did` leaves the figure alone: it is the start-up selection and
    /// the command line names a figure.
    fn cli_keeps_figure(&self, did: u32) -> bool {
        self.startup_did == Some(did) && self.args.figure.is_some()
    }

    /// Previews item `did`, unless it is worn already (a replayed command line wears it
    /// and selects it).
    fn preview_item(&mut self, did: u32) {
        if self.items.iter().all(|i| i.did != did) {
            self.preview = Some(ItemArg::new(did));
        }
    }

    /// Back to the start of the clip (keeps paused/playing).
    pub fn restart(&mut self) {
        self.play.time = self.fixed_time.unwrap_or(0.0);
        self.play.placement = (Vec3::ZERO, 0.0);
        if let Some(i) = &mut self.instance {
            i.wanted = i.wanted.max(i.submitted) + 1;
        }
    }

    /// The figure's request, once the catalogue is known; `None` (with
    /// [`CharacterState::figure_error`] set) when there is no figure to build.
    ///
    /// The request is the rebuild key: slider bytes and the height control are not in it.
    /// The figure is assembled with the avatar's own sliders, and the view's bytes are
    /// applied to the animated instance ([`apply_sliders`]), so dragging a slider never
    /// re-assembles or re-uploads the figure.
    fn request(&mut self, catalog: &Catalog) -> Option<CharacterRequest> {
        let arg = match &self.figure {
            Some(f) => f.clone(),
            None => {
                let Some(b) = catalog.default_body() else {
                    self.figure_error = Some(catalog.no_body_reason());
                    return None;
                };
                FigureArg::Playable {
                    species: SpeciesArg::Key(b.species),
                    sex: b.sex,
                }
            }
        };
        match catalog.resolve_figure(&arg) {
            Ok(figure) => {
                self.figure_error = None;
                let mut req = CharacterRequest::new(figure);
                req.items = self.items.clone();
                // The preview takes a slot only when one is free (equipment_section says
                // so otherwise).
                if let Some(p) = self.preview
                    && req.items.len() < MAX_ITEMS
                {
                    req.items.push(p);
                }
                req.lod = self.lod;
                Some(req)
            }
            Err(e) => {
                self.figure_error = Some(e);
                None
            }
        }
    }

    /// The clip DID to play for the shown figure.
    #[must_use]
    pub fn clip_did(&self) -> Option<u32> {
        match self.clip {
            ClipChoice::None => None,
            ClipChoice::Did(d) => Some(d),
            ClipChoice::State(s) => self
                .shown
                .as_ref()
                .and_then(|p| p.clips.chosen(s))
                .map(|r| r.clip.0),
        }
    }

    /// The playback rate of the clip: the script table's rate for a selected state.
    fn clip_rate(&self) -> f32 {
        match self.clip {
            ClipChoice::State(s) => self
                .shown
                .as_ref()
                .and_then(|p| p.clips.chosen(s))
                .map(|r| r.rate)
                .filter(|r| r.is_finite() && *r > 0.0)
                .unwrap_or(1.0),
            _ => 1.0,
        }
    }

    /// The loaded clip of the instance, if bound.
    fn bound_clip(&self) -> Option<&Arc<ClipProduct>> {
        self.instance.as_ref().and_then(|i| i.clip.as_ref())
    }

    /// The command line that reproduces the view (for notes and checklists).
    #[must_use]
    pub fn command_line(&self, select: u32) -> String {
        let args = CharacterArgs {
            figure: self.figure.clone().or_else(|| {
                self.shown.as_ref().and_then(|p| match p.request.figure {
                    Figure::Playable { species, sex } => Some(FigureArg::Playable {
                        species: SpeciesArg::Key(species),
                        sex,
                    }),
                    _ => None,
                })
            }),
            items: self
                .items
                .iter()
                .copied()
                .chain(self.preview)
                .take(MAX_ITEMS)
                .collect(),
            clip: Some(self.clip.into()),
            time: Some(command_line_time(
                self.play.time,
                self.bound_clip()
                    .and_then(|c| c.bound.as_ref())
                    .map_or(0.0, |b| b.duration()),
                self.play.wrap(),
            )),
            sliders: self.sliders.clone(),
            bones: self.bones,
            trail: self.trail,
        };
        let yaw = self.orbit.orbit.yaw.to_degrees();
        let pitch = self.orbit.orbit.pitch.to_degrees();
        format!(
            "--select {select:#010x} --view character {} --mesh-angle {yaw:.0},{pitch:.0}",
            args.to_command_line()
        )
    }
}

// ---------------------------------------------------------------------------------------
// Per-frame work (no drawing)
// ---------------------------------------------------------------------------------------

fn notify(ctx: &egui::Context) -> impl Fn() + Send + 'static {
    let ctx = ctx.clone();
    move || ctx.request_repaint()
}

/// Starts and polls the jobs, keeps the animated instance in step with the shown figure
/// and clip, advances play time and submits sampling. Returns whether anything is still
/// loading.
pub(crate) fn update(st: &mut CharacterState, l: &Loaded, egui_ctx: &egui::Context) -> bool {
    let pool = &l.stores.pool;
    if st.catalog.is_none() {
        let ctx = Arc::clone(&l.ctx);
        st.catalog = Some(Job::spawn(
            pool,
            Priority::Immediate,
            notify(egui_ctx),
            move |_| build_catalog(&ctx),
        ));
    }
    if st.clip_catalog.is_none() {
        let ctx = Arc::clone(&l.ctx);
        st.clip_catalog = Some(Job::spawn(
            pool,
            Priority::Prefetch,
            notify(egui_ctx),
            move |job| build_clip_catalog(&ctx, job),
        ));
    }
    let Some(catalog) = st.catalog.as_mut().and_then(Job::value) else {
        // A failed catalogue is final (its error is on screen).
        return !st.catalog.as_ref().is_some_and(|j| j.peek_failed());
    };
    ensure_object_seed(st, l, egui_ctx);

    // The figure.
    if let Some(req) = st.request(&catalog) {
        let current = st.job.as_ref().map(|(r, _)| r);
        if current != Some(&req) {
            let ctx = Arc::clone(&l.ctx);
            let cat = Arc::clone(&catalog);
            let r = req.clone();
            let job = Job::spawn(pool, Priority::Immediate, notify(egui_ctx), move |job| {
                build_character(&ctx, &cat, &r, job)
            });
            st.job = Some((req, job));
        }
    }
    if let Some((_, job)) = &mut st.job
        && let Some(p) = job.value()
        && st.shown.as_ref().is_none_or(|s| !Arc::ptr_eq(s, &p))
    {
        st.shown = Some(p);
    }
    let Some(product) = st.shown.clone() else {
        return true;
    };

    // The animated instance follows the shown figure.
    let rebuild = st
        .instance
        .as_ref()
        .is_none_or(|i| !Arc::ptr_eq(&i.product, &product));
    if rebuild {
        st.instance = product.skeleton.as_ref().and_then(|sk| {
            let mut scheduler = AnimationScheduler::new(pool, SchedulerConfig::default());
            let id = scheduler.add(Arc::clone(sk), product.attach.clone()).ok()?;
            let keep_clip = st
                .instance
                .take()
                .and_then(|old| old.clip)
                .filter(|c| c.bound.as_ref().is_some_and(|b| b.skeleton().same_rig(sk)));
            Some(Instance {
                product: Arc::clone(&product),
                scheduler,
                id,
                animator: Animator::new(Arc::clone(sk)),
                submitted: 0,
                wanted: 1,
                clip: keep_clip,
                sliders: Vec::new(),
                slider_problems: Vec::new(),
            })
        });
        if let Some(i) = &mut st.instance {
            // Sliders: the view's bytes, else the avatar's.
            let bytes = st
                .sliders
                .clone()
                .unwrap_or_else(|| product.sliders.values.clone());
            apply_sliders(i, &bytes);
        }
    }

    // The clip.
    let mut loading = st.job.as_mut().is_some_and(|(_, j)| !j.is_finished());
    let want = st.clip_did();
    let rig = product
        .skeleton
        .as_ref()
        .map_or(0, |s| Arc::as_ptr(s) as usize);
    match want {
        None => {
            st.clip_job = None;
            if let Some(i) = &mut st.instance
                && i.clip.take().is_some()
            {
                i.animator = Animator::new(Arc::clone(i.animator.skeleton()));
                i.wanted = i.submitted + 1;
            }
        }
        Some(did) => {
            let key = (did, rig);
            if st.clip_job.as_ref().is_none_or(|(k, _)| *k != key) {
                let ctx = Arc::clone(&l.ctx);
                let sk = product.skeleton.clone();
                let job = Job::spawn(pool, Priority::Immediate, notify(egui_ctx), move |job| {
                    load_clip(&ctx, did, sk, job)
                });
                st.clip_job = Some((key, job));
            }
            if let Some((_, job)) = &mut st.clip_job {
                loading |= !job.is_finished();
                // A clip that failed to load stops the previous one (the overlay names the
                // failure) instead of playing on under the new clip's name.
                if job.peek_failed()
                    && let Some(i) = &mut st.instance
                    && i.clip.take().is_some()
                {
                    i.animator = Animator::new(Arc::clone(i.animator.skeleton()));
                    i.wanted = i.submitted + 1;
                }
                if let (Some(c), Some(i)) = (job.value(), &mut st.instance)
                    && i.clip.as_ref().is_none_or(|old| !Arc::ptr_eq(old, &c))
                {
                    let fits = c
                        .bound
                        .as_ref()
                        .is_some_and(|b| b.skeleton().same_rig(i.animator.skeleton()));
                    i.clip = fits.then_some(c);
                    if i.clip.is_none() {
                        i.animator = Animator::new(Arc::clone(i.animator.skeleton()));
                    }
                    i.wanted = i.submitted + 1;
                }
            }
        }
    }

    // Play time.
    let now = Instant::now();
    let dt = st
        .last_tick
        .replace(now)
        .map_or(0.0, |t| now.duration_since(t).as_secs_f32());
    if let Some(t) = st.fixed_time {
        st.play.playing = false;
        if (st.play.time - t).abs() > f32::EPSILON {
            st.play.time = t;
        }
    }
    let rate = st.clip_rate();
    if let Some(i) = &mut st.instance {
        let clip = i.clip.clone();
        let duration = clip
            .as_ref()
            .and_then(|c| c.bound.as_ref())
            .map_or(0.0, |b| b.duration());
        let before = st.play.advance(dt, rate, duration);
        if let Some(b) = clip.as_ref().and_then(|c| c.bound.as_ref()) {
            if st.play.root_motion && st.play.playing {
                let m = motion_between(b, before, st.play.time, st.play.wrap());
                st.play.apply_motion(m, product.model_scale);
            }
            // The animator plays the view's play time (an immediate switch: the UI owns τ).
            let mut p = Playback::new(Arc::clone(b), st.play.wrap());
            p.time = st.play.time;
            p.rate = rate;
            p.root_motion = RootMotionMode::InPlace;
            let _ = i.animator.play(0, p, 0.0, false);
            if before != st.play.time || i.submitted == 0 {
                i.wanted = i.wanted.max(i.submitted + 1);
            }
        }
        // Sampling on the workers.
        i.scheduler.collect();
        if i.wanted > i.submitted {
            let frame = i.wanted;
            if let Ok(Submitted::Queued) = i.scheduler.submit(i.id, &i.animator, frame) {
                i.submitted = frame;
            }
        }
        if i.scheduler
            .pose(i.id)
            .is_none_or(|p| p.frame != i.submitted)
        {
            loading = true;
        }
    }
    if let Some((_, j)) = &mut st.object_seed {
        loading |= !j.is_finished();
    }
    loading
}

/// Decides a 0x70–0x76 seed (item or figure) on a worker.
fn ensure_object_seed(st: &mut CharacterState, l: &Loaded, egui_ctx: &egui::Context) {
    if let Some(did) = st.seeded
        && let Seed::Object(_) = Seed::of(did)
        && st.object_seed.as_ref().is_none_or(|(d, _)| *d != did)
    {
        let ctx = Arc::clone(&l.ctx);
        let job = Job::spawn(
            &l.stores.pool,
            Priority::Immediate,
            notify(egui_ctx),
            move |_| is_item(&ctx, did),
        );
        st.object_seed = Some((did, job));
    }
    let Some((did, job)) = &mut st.object_seed else {
        return;
    };
    let did = *did;
    let Some(r) = job.poll().cloned() else {
        return;
    };
    st.object_seed = None;
    match r {
        Ok(item) if *item => st.preview_item(did),
        Ok(_) => {
            if !st.cli_keeps_figure(did) {
                st.figure = Some(FigureArg::Object(did));
                st.preview = None;
            }
        }
        Err(e) => tracing::info!(did = format_args!("{did:#010x}"), error = %e, "seed"),
    }
}

/// Whether game object `did` is an item (a worn appearance, or a slot and a `PhysObj`).
fn is_item(ctx: &crate::data::DataContext, did: u32) -> Result<bool, crate::ExplorerError> {
    let set_did = tessera_formats::property_set::object_property_set_did(Did(did))
        .ok_or_else(|| crate::ExplorerError::NotApplicable(format!("{did:#010x} is no object")))?;
    let bytes = ctx
        .game
        .read(set_did)?
        .ok_or(crate::ExplorerError::NotFound(set_did.0))?;
    let registry = ctx.game.registry()?;
    let set =
        tessera_formats::PropertySetRecord::parse_expecting(&bytes, set_did, registry.as_ref())?
            .set;
    let pids = tessera_formats::item::ItemPids::from_names(registry.dictionary())?;
    let v = tessera_formats::item::ItemVisual::from_property_set(&set, &pids);
    Ok(!v.worn.is_empty() || (v.phys_obj.is_some() && v.default_slot.is_some()))
}

fn apply_sliders(i: &mut Instance, bytes: &[u8]) {
    let Some(sk) = i.product.skeleton.as_ref() else {
        return;
    };
    let (sliders, problems) = i.product.sliders.build(sk, bytes);
    i.slider_problems = problems;
    i.sliders = bytes.to_vec();
    let s = (!sliders.is_empty()).then(|| Arc::new(sliders));
    if i.scheduler.set_bone_sliders(i.id, s).is_ok() {
        i.wanted = i.wanted.max(i.submitted) + 1;
    }
}

/// The pose to draw: the last finished sample with overlays, and whether it is a fresh
/// sample (else a [`stand_in_pose`] while the instance's first sample is on the workers).
fn pose_draw(st: &CharacterState) -> Option<(PoseDraw, bool)> {
    let product = st.shown.as_ref()?;
    let placement = st.play.placement_matrix(product.model_scale);
    let mut out = PoseDraw {
        placement,
        ..PoseDraw::default()
    };
    let Some(i) = &st.instance else {
        return Some((out, false));
    };
    let sk = i.animator.skeleton();
    let Some(pose) = i.scheduler.pose(i.id) else {
        return Some((
            stand_in_pose(
                st.last_pose.as_deref(),
                st.last_rig.as_deref(),
                sk,
                placement,
            ),
            false,
        ));
    };
    out.palette.clone_from(&pose.skin);
    out.attach.clone_from(&pose.attach);
    if st.bones {
        for (a, b) in crate::character::bone_lines(sk, &pose.model, placement) {
            out.lines.push((a, b, BONE_COLOR));
        }
    }
    if st.trail
        && let Some(c) = &i.clip
    {
        // Trails start where the cycle started: undo the cycle's motion so far.
        let base = if st.play.root_motion {
            let m = c
                .bound
                .as_ref()
                .and_then(|b| tessera_anim::motion_at(b, st.play.clip_time(b.duration())))
                .unwrap_or_default();
            trail_base(placement, m)
        } else {
            placement
        };
        let lines = |pts: &[[f32; 3]], color: [f32; 4], out: &mut Vec<_>| {
            for w in pts.windows(2) {
                let a = base.transform_point3(Vec3::from_array(w[0]));
                let b = base.transform_point3(Vec3::from_array(w[1]));
                out.push((a.to_array(), b.to_array(), color));
            }
        };
        lines(&c.root_trail, ROOT_TRAIL_COLOR, &mut out.lines);
        // The extracted motion on the ground plane.
        lines(&c.motion_trail, MOTION_TRAIL_COLOR, &mut out.lines);
    }
    Some((out, true))
}

/// Whether the character view is complete (screenshot mode): the figure and the clip are
/// built (or failed), the last submitted pose is back, and the frame on screen shows it.
#[must_use]
pub(crate) fn character_ready(st: &CharacterState, vp: Option<&Viewport>) -> bool {
    let Some(catalog) = &st.catalog else {
        return false;
    };
    if catalog.peek_failed() {
        // Final: the catalogue's error is on screen.
        return true;
    }
    if st.object_seed.is_some() {
        return false;
    }
    let figure_done = match &st.job {
        Some((_, j)) => j_finished(j),
        None => st.figure_error.is_some(),
    };
    if !figure_done {
        return false;
    }
    let Some(product) = &st.shown else {
        // Failed: the error is on screen.
        return st.job.as_ref().is_some_and(|(_, j)| j_failed(j)) || st.figure_error.is_some();
    };
    if st.clip_did().is_some() && !st.clip_job.as_ref().is_some_and(|(_, j)| j_finished(j)) {
        return false;
    }
    if let Some(i) = &st.instance {
        if i.wanted > i.submitted {
            return false;
        }
        if i.scheduler
            .pose(i.id)
            .is_none_or(|p| p.frame != i.submitted)
        {
            return false;
        }
    }
    if !product.scene.drawable() {
        return true;
    }
    let Some(vp) = vp else { return false };
    vp.error().is_some()
        || st
            .scene_id
            .is_some_and(|id| vp.shows(id) && vp.texture().is_some() && st.last_pose.is_some())
}

fn j_finished<T>(j: &Job<T>) -> bool {
    // `Job::poll` needs `&mut`; readiness only needs to know whether a result arrived,
    // which `update` polls every frame before this is asked.
    j.peek_finished()
}

fn j_failed<T>(j: &Job<T>) -> bool {
    j.peek_failed()
}

// ---------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------

/// Draws the character view.
#[allow(clippy::too_many_arguments)]
pub(crate) fn character_view(
    ui: &mut Ui,
    l: &Loaded,
    cur: &mut Current,
    st: &mut CharacterState,
    settings: &mut MeshSettings,
    vp: &mut Viewport,
    actions: &mut Vec<Action>,
) {
    st.seed(cur.nav.did);
    let loading = update(st, l, ui.ctx());
    let catalog = st.catalog.as_mut().and_then(Job::value);
    toolbar(ui, st, settings, catalog.as_deref(), loading, cur.nav.did);
    if let Some(e) = st.catalog.as_mut().and_then(Job::error) {
        ui.colored_label(Color32::LIGHT_RED, format!("catalogue: {e}"));
    }
    if let Some(c) = &catalog {
        for p in &c.problems {
            ui.colored_label(WARN_COLOR, format!("catalogue: {p}"));
        }
    }
    if let Some(e) = &st.figure_error {
        ui.colored_label(Color32::LIGHT_RED, e);
    }
    if let Some(e) = st.job.as_mut().and_then(|(_, j)| j.error()) {
        ui.colored_label(Color32::LIGHT_RED, e);
    }
    if !st.compact {
        egui::Panel::right("character-inspector")
            .resizable(true)
            .default_size(460.0)
            .min_size(260.0)
            .show(ui, |ui| inspector(ui, st, cur.nav.did, actions));
    }
    viewport_widget(ui, st, settings, vp);
    if st.play.playing || loading {
        ui.ctx().request_repaint();
    }
}

fn shading_name(s: MeshShading) -> &'static str {
    match s {
        MeshShading::Lit => "Lit",
        MeshShading::BaseColor => "Base colour",
        MeshShading::Normals => "Normals",
        MeshShading::UvChecker => "UV checker",
        MeshShading::Hidden => "Hidden (wire only)",
    }
}

fn toolbar(
    ui: &mut Ui,
    st: &mut CharacterState,
    settings: &mut MeshSettings,
    catalog: Option<&Catalog>,
    loading: bool,
    did: u32,
) {
    ui.horizontal_wrapped(|ui| {
        // Body.
        let current = st
            .shown
            .as_ref()
            .map_or_else(|| "…".to_owned(), |p| p.title.clone());
        egui::ComboBox::from_id_salt("character-body")
            .selected_text(current)
            .width(200.0)
            .show_ui(ui, |ui| {
                if let Some(c) = catalog {
                    for b in &c.bodies {
                        let this = FigureArg::Playable {
                            species: SpeciesArg::Key(b.species),
                            sex: b.sex,
                        };
                        if ui
                            .selectable_label(st.figure.as_ref() == Some(&this), b.title())
                            .on_hover_text(format!("avatar {}", b.avatar))
                            .clicked()
                        {
                            st.figure = Some(this);
                            st.preview = None;
                        }
                    }
                }
                if matches!(Seed::of(did), Seed::Figure(_) | Seed::Object(_))
                    && ui
                        .selectable_label(false, format!("the selection {did:#010x}"))
                        .clicked()
                {
                    st.figure = Some(match Seed::of(did) {
                        Seed::Figure(SeedFigure::Setup(d)) => FigureArg::Setup(d),
                        _ => FigureArg::Object(did),
                    });
                }
            });
        ui.separator();
        // Clip.
        for (label, choice) in [
            ("Bind", ClipChoice::None),
            ("Idle", ClipChoice::State(StateArg::Idle)),
            ("Walk", ClipChoice::State(StateArg::Walk)),
            ("Run", ClipChoice::State(StateArg::Run)),
        ] {
            if ui
                .selectable_label(st.clip == choice, label)
                .on_hover_text(match choice {
                    ClipChoice::None => "The reference pose (no clip)".to_owned(),
                    _ => {
                        "The clip the figure's script tables select (script-table.md §8)".to_owned()
                    }
                })
                .clicked()
                && st.clip != choice
            {
                st.clip = choice;
                st.restart();
            }
        }
        if let ClipChoice::Did(d) = st.clip {
            ui.label(RichText::new(format!("clip {d:#010x}")).monospace());
        }
        ui.separator();
        let label = if st.play.playing {
            "⏸ Pause"
        } else {
            "▶ Play"
        };
        if ui.button(label).clicked() {
            st.play.playing = !st.play.playing;
            st.fixed_time = None;
        }
        if ui.button("⏮").on_hover_text("Back to the start").clicked() {
            st.fixed_time = None;
            st.restart();
        }
        ui.toggle_value(&mut st.play.looping, "Loop");
        ui.add(
            egui::Slider::new(&mut st.play.speed, 0.0..=2.0)
                .text("speed")
                .fixed_decimals(2),
        );
        let duration = st
            .bound_clip()
            .and_then(|c| c.bound.as_ref())
            .map_or(0.0, |b| b.duration());
        if duration > 0.0 {
            let mut t = st.play.clip_time(duration);
            let r = ui.add(
                egui::Slider::new(&mut t, 0.0..=duration)
                    .text(format!("/ {duration:.2} s"))
                    .fixed_decimals(3),
            );
            if r.changed() {
                st.play.playing = false;
                st.fixed_time = None;
                st.play.time = t;
            }
        }
        if loading {
            ui.spinner();
        }
    });
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("character-shading")
            .selected_text(shading_name(settings.shading))
            .show_ui(ui, |ui| {
                for s in [
                    MeshShading::Lit,
                    MeshShading::BaseColor,
                    MeshShading::Normals,
                    MeshShading::UvChecker,
                    MeshShading::Hidden,
                ] {
                    ui.selectable_value(&mut settings.shading, s, shading_name(s));
                }
            });
        ui.toggle_value(&mut settings.wireframe, "Wireframe");
        ui.toggle_value(&mut settings.grid, "Grid");
        ui.toggle_value(&mut st.bones, "Bones")
            .on_hover_text("Bone overlay: each bone joined to its parent");
        ui.toggle_value(&mut st.trail, "Trail").on_hover_text(
            "Root trail over one cycle (blue: the top bone that moves most, \
             skeleton-anim.md §7.2) and extracted motion (yellow, §7.3)",
        );
        if ui
            .toggle_value(&mut st.play.root_motion, "Root motion")
            .on_hover_text("Move the figure by the clip's extracted motion (skeleton-anim.md §7.3)")
            .changed()
        {
            st.play.placement = (Vec3::ZERO, 0.0);
        }
        ui.checkbox(&mut settings.dye_on, "Dye colour");
        if settings.dye_on {
            ui.color_edit_button_srgb(&mut settings.dye_color);
        }
        if ui.button("Frame").clicked() {
            st.reframe = true;
        }
        if ui.button("Reset view").clicked() {
            st.orbit.reset_angles();
            st.reframe = true;
        }
    });
}

fn inspector(ui: &mut Ui, st: &mut CharacterState, did: u32, actions: &mut Vec<Action>) {
    egui::ScrollArea::vertical()
        .id_salt("character-inspector-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let Some(p) = st.shown.clone() else {
                let failed = st.figure_error.is_some()
                    || st.job.as_ref().is_some_and(|(_, j)| j.peek_failed())
                    || st.catalog.as_ref().is_some_and(Job::peek_failed);
                if failed {
                    ui.label(RichText::new("nothing to show: see the error above").weak());
                } else {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("assembling …");
                    });
                }
                return;
            };
            ui.label(RichText::new(&p.title).strong().size(16.0));
            let cmd = st.command_line(did);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("command line:").weak().small());
                if ui.small_button("copy").clicked() {
                    ui.ctx().copy_text(cmd.clone());
                }
            });
            ui.label(RichText::new(&cmd).monospace().small());
            egui::Grid::new("character-facts")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    for (k, v) in &p.facts {
                        ui.label(RichText::new(k).weak());
                        ui.label(RichText::new(v).monospace());
                        ui.end_row();
                    }
                    ui.label(RichText::new("built in").weak());
                    ui.label(format!("{} ms", p.build_ms));
                    ui.end_row();
                });
            for n in &p.scene.notes {
                ui.label(
                    RichText::new(n)
                        .small()
                        .color(Color32::from_rgb(0xE0, 0xB0, 0x60)),
                );
            }
            egui::CollapsingHeader::new("Clips: idle, walk, run (script tables)")
                .default_open(true)
                .show(ui, |ui| clips_section(ui, st, &p, actions));
            egui::CollapsingHeader::new("Animation browser")
                .default_open(true)
                .show(ui, |ui| browser_section(ui, st, &p, actions));
            egui::CollapsingHeader::new(format!("Equipment ({})", st.items.len()))
                .default_open(true)
                .show(ui, |ui| equipment_section(ui, st, &p, actions));
            egui::CollapsingHeader::new(format!("Parts ({})", p.scene.parts.len()))
                .default_open(false)
                .show(ui, |ui| parts_section(ui, &p, actions));
            if !p.sliders.defs.is_empty() || p.sliders.record.is_some() {
                egui::CollapsingHeader::new(format!("Sliders ({})", p.sliders.visible()))
                    .default_open(false)
                    .show(ui, |ui| sliders_section(ui, st, &p));
            }
            if !p.diagnostics.is_empty() {
                egui::CollapsingHeader::new(format!("Diagnostics ({})", p.diagnostics.len()))
                    .default_open(false)
                    .show(ui, |ui| {
                        for d in &p.diagnostics {
                            ui.label(RichText::new(d).small().monospace());
                        }
                    });
            }
        });
}

fn did_link(ui: &mut Ui, did: Did, actions: &mut Vec<Action>) {
    if ui
        .link(RichText::new(did.to_string()).monospace())
        .clicked()
    {
        actions.push(Action::Open(did.0));
    }
}

fn clips_section(
    ui: &mut Ui,
    st: &mut CharacterState,
    p: &CharacterProduct,
    actions: &mut Vec<Action>,
) {
    if let Some(e) = &p.clips.error {
        ui.colored_label(Color32::from_rgb(0xE0, 0xB0, 0x60), e);
    }
    if !p.clips.tables.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("tables:").weak());
            for t in p.clips.tables.iter().take(8) {
                did_link(ui, *t, actions);
            }
        });
    }
    for s in &p.clips.states {
        ui.horizontal_wrapped(|ui| {
            let choice = ClipChoice::State(s.state);
            if ui
                .selectable_label(st.clip == choice, RichText::new(s.state.name()).strong())
                .clicked()
            {
                st.clip = choice;
                st.restart();
            }
            match &s.chosen {
                Some(r) => {
                    did_link(ui, r.clip, actions);
                    ui.label(
                        RichText::new(format!(
                            "layer {} · rate {:.2} · {} · {:.2} m/cycle",
                            r.layer.1,
                            r.rate,
                            if r.loops { "loop" } else { "once" },
                            r.displacement
                        ))
                        .small(),
                    );
                }
                None => {
                    ui.label(RichText::new("none").weak());
                }
            }
        });
        egui::CollapsingHeader::new(format!(
            "{} candidates ({}{})",
            s.state.name(),
            s.candidates.len(),
            if s.issues > 0 {
                format!(", {} resolver issues", s.issues)
            } else {
                String::new()
            }
        ))
        .id_salt(("cands", s.state.name()))
        .show(ui, |ui| {
            for r in &s.candidates {
                ui.horizontal(|ui| {
                    if ui
                        .small_button("▶")
                        .on_hover_text("Play this clip")
                        .clicked()
                    {
                        st.clip = ClipChoice::Did(r.clip.0);
                        st.restart();
                    }
                    did_link(ui, r.clip, actions);
                    ui.label(
                        RichText::new(format!(
                            "{} · w {:.2} · rate {:.2} · ramp {:.2}",
                            r.layer.1, r.weight, r.rate, r.ramp_in
                        ))
                        .small(),
                    );
                });
            }
        });
    }
}

fn browser_section(
    ui: &mut Ui,
    st: &mut CharacterState,
    p: &CharacterProduct,
    actions: &mut Vec<Action>,
) {
    // The selected clip's facts.
    if let Some((_, job)) = &mut st.clip_job {
        match job.poll() {
            Some(Ok(c)) => {
                let c = Arc::clone(c);
                clip_facts(ui, &c, actions);
            }
            Some(Err(e)) => {
                ui.colored_label(Color32::LIGHT_RED, e.clone());
            }
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("loading the clip …");
                });
            }
        }
    }
    ui.separator();
    if let Some(e) = st.clip_catalog.as_mut().and_then(Job::error) {
        ui.colored_label(Color32::LIGHT_RED, format!("clip catalogue: {e}"));
        return;
    }
    let Some(cat) = st.clip_catalog.as_mut().and_then(Job::value) else {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("scanning the 0x05 clips …");
        });
        return;
    };
    let bones = p.bones();
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut st.clip_filter)
                .hint_text("filter DIDs (hex)")
                .desired_width(140.0),
        );
        ui.checkbox(&mut st.fit_only, format!("fits {bones} bones"));
    });
    let filter = st
        .clip_filter
        .trim()
        .trim_start_matches("0x")
        .to_ascii_lowercase();
    let rows: Vec<&crate::character::CatalogClip> = cat
        .clips
        .iter()
        .filter(|c| !st.fit_only || c.tracks as usize == bones)
        .filter(|c| filter.is_empty() || format!("{:08x}", c.did).contains(&filter))
        .collect();
    ui.label(
        RichText::new(format!(
            "{} of {} clips ({} failed; scanned in {} ms)",
            rows.len(),
            cat.clips.len(),
            cat.failed,
            cat.ms
        ))
        .weak()
        .small(),
    );
    for e in &cat.errors {
        ui.label(RichText::new(e).small().color(WARN_COLOR));
    }
    let row_h = ui.text_style_height(&egui::TextStyle::Monospace) + 2.0;
    let current = st.clip_did();
    let mut pick = None;
    egui::ScrollArea::vertical()
        .id_salt("clip-browser")
        .max_height(row_h * CLIP_ROWS)
        .auto_shrink([false, true])
        .show_rows(ui, row_h, rows.len(), |ui, range| {
            for c in &rows[range] {
                let text = format!(
                    "{:#010x}  {:6.2} s  {:3} tracks  {}",
                    c.did,
                    c.duration,
                    c.tracks,
                    match c.spline {
                        Some(true) => "spline",
                        Some(false) => "interleaved",
                        None => "?",
                    }
                );
                if ui
                    .selectable_label(current == Some(c.did), RichText::new(text).monospace())
                    .clicked()
                {
                    pick = Some(c.did);
                }
            }
        });
    if let Some(d) = pick {
        st.clip = ClipChoice::Did(d);
        st.restart();
    }
}

fn clip_facts(ui: &mut Ui, c: &ClipProduct, actions: &mut Vec<Action>) {
    let i = &c.info;
    ui.horizontal_wrapped(|ui| {
        did_link(ui, Did(i.did), actions);
        ui.label(format!(
            "{:.3} s · {} frames · {} tracks · {}",
            i.duration, i.frames, i.tracks, i.kind
        ));
    });
    if let Some(e) = i.envelope_duration {
        ui.label(
            RichText::new(format!(
                "envelope duration {e} differs (havok-data.md §8.4)"
            ))
            .small(),
        );
    }
    match &i.extracted_motion {
        Some((n, t, yaw)) => {
            ui.label(
                RichText::new(format!(
                    "extracted motion: {n} samples, ends at ({:.2}, {:.2}, {:.2}) m, yaw {yaw:.1}°",
                    t[0], t[1], t[2]
                ))
                .small(),
            );
        }
        None => {
            ui.label(
                RichText::new("no extracted motion (in place)")
                    .small()
                    .weak(),
            );
        }
    }
    ui.label(
        RichText::new(format!(
            "root bone {} travels {:.3} m over the clip{}",
            c.root_bone.map_or_else(|| "–".into(), |b| b.to_string()),
            c.root_travel,
            if i.track_weights {
                " · per-track weights"
            } else {
                ""
            }
        ))
        .small(),
    );
    if let Some(e) = &c.bind_error {
        ui.colored_label(Color32::from_rgb(0xE0, 0xB0, 0x60), e);
    }
    egui::CollapsingHeader::new(format!(
        "annotation tracks ({}, {} timed annotations)",
        i.annotation_tracks, i.annotations
    ))
    .id_salt(("ann", i.did))
    .show(ui, |ui| {
        for (k, n) in i.track_names.iter().enumerate() {
            ui.label(RichText::new(format!("{k:3} {n}")).monospace().small());
        }
    });
}

fn equipment_section(
    ui: &mut Ui,
    st: &mut CharacterState,
    p: &CharacterProduct,
    actions: &mut Vec<Action>,
) {
    if !matches!(p.request.figure, Figure::Playable { .. }) {
        ui.label(
            RichText::new("items are worn by playable bodies only")
                .weak()
                .small(),
        );
    }
    let mut remove = None;
    for (k, it) in st.items.iter_mut().enumerate() {
        ui.horizontal_wrapped(|ui| {
            if ui.small_button("✕").on_hover_text("Take off").clicked() {
                remove = Some(k);
            }
            did_link(ui, Did(it.did), actions);
            item_controls(ui, it, ("item", k));
        });
    }
    if let Some(k) = remove {
        st.items.remove(k);
    }
    if let Some(mut pv) = st.preview {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("previewing").weak());
            did_link(ui, Did(pv.did), actions);
            item_controls(ui, &mut pv, ("preview", 0));
            if ui.small_button("keep").clicked() && st.items.len() < MAX_ITEMS {
                st.items.push(pv);
                st.preview = None;
            } else {
                st.preview = Some(pv);
            }
            if ui.small_button("✕").clicked() {
                st.preview = None;
            }
        });
        if st.preview.is_some() && st.items.len() >= MAX_ITEMS {
            ui.colored_label(
                Color32::LIGHT_RED,
                format!("the previewed item is not drawn: {MAX_ITEMS} items are worn already"),
            );
        }
    }
    ui.horizontal(|ui| {
        let r = ui.add(
            egui::TextEdit::singleline(&mut st.item_text)
                .hint_text("0x79… [@dye=0.06][@sheathed][@slot=secondary]")
                .desired_width(260.0),
        );
        let go = ui.button("Wear").clicked()
            || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
        if go {
            match parse_item(&st.item_text) {
                Ok(it) if st.items.len() < MAX_ITEMS => {
                    st.items.push(it);
                    st.item_text.clear();
                    st.item_error = None;
                }
                Ok(_) => st.item_error = Some("too many items".into()),
                Err(e) => st.item_error = Some(e.to_string()),
            }
        }
    });
    if let Some(e) = &st.item_error {
        ui.colored_label(Color32::LIGHT_RED, e);
    }
    if !p.held.is_empty() {
        ui.label(RichText::new("held").strong());
        for h in &p.held {
            ui.label(
                RichText::new(format!(
                    "{} → {} ({}) on {} · setup {} · {} parts",
                    h.what,
                    h.location_label,
                    if h.sheathed { "sheathed" } else { "wielded" },
                    h.bone
                        .as_ref()
                        .map_or_else(|| "the origin".into(), ToString::to_string),
                    h.setup,
                    h.parts
                ))
                .small(),
            );
        }
    }
}

fn item_controls(ui: &mut Ui, it: &mut ItemArg, salt: (&str, usize)) {
    let mut dyed = it.dye.is_some();
    if ui
        .checkbox(&mut dyed, "dye")
        .on_hover_text("Dye code of the first channel (appearance.md §4.9); off: the item's own")
        .changed()
    {
        it.dye = dyed.then_some(0.0);
    }
    if let Some(d) = &mut it.dye {
        ui.push_id(salt, |ui| {
            ui.add(
                egui::DragValue::new(d)
                    .range(0.0..=MAX_DYE_CODE)
                    .speed(0.01)
                    .max_decimals(3),
            );
        });
    }
    ui.checkbox(&mut it.sheathed, "sheathed")
        .on_hover_text("Stowed (appearance.md §5.4) instead of in hand");
}

fn parts_section(ui: &mut Ui, p: &CharacterProduct, actions: &mut Vec<Action>) {
    for (part, info) in p.scene.parts.iter().zip(&p.parts) {
        ui.horizontal_wrapped(|ui| {
            did_link(ui, Did(part.mesh), actions);
            let what = match info {
                FigurePart::Body {
                    slot,
                    priority,
                    origin,
                } => format!(
                    "{slot} · {} · {origin}",
                    priority.map_or_else(|| "default".into(), |v| format!("priority {v}"))
                ),
                FigurePart::Held { held } => p.held.get(*held).map_or_else(
                    || "held".into(),
                    |h| format!("held at {}", h.location_label),
                ),
            };
            ui.label(RichText::new(what).small());
            if part.skinned {
                ui.label(RichText::new("skinned").small().color(Color32::LIGHT_GREEN));
            }
            if let Some(e) = &part.error {
                ui.colored_label(Color32::LIGHT_RED, e);
            }
        });
        for i in &part.issues {
            ui.label(RichText::new(format!("    {i}")).small().weak());
        }
    }
}

fn sliders_section(ui: &mut Ui, st: &mut CharacterState, p: &CharacterProduct) {
    for n in &p.sliders.notes {
        ui.label(
            RichText::new(n)
                .small()
                .color(Color32::from_rgb(0xE0, 0xB0, 0x60)),
        );
    }
    if let Some(i) = &st.instance {
        for n in &i.slider_problems {
            ui.label(RichText::new(n).small().weak());
        }
    }
    let visible = p.sliders.visible();
    let mut bytes = st
        .sliders
        .clone()
        .unwrap_or_else(|| p.sliders.values.clone());
    bytes.resize(visible, 0x80);
    let mut changed = false;
    let mut k = 0usize;
    for d in &p.sliders.defs {
        if !d.visible {
            ui.label(
                RichText::new(format!("{} ({}): fixed, always applied", d.name, d.group))
                    .small()
                    .weak(),
            );
            continue;
        }
        if let Some(b) = bytes.get_mut(k) {
            let mut v = i32::from(*b);
            if ui
                .add(egui::Slider::new(&mut v, 0..=255).text(format!("{} · {}", d.group, d.name)))
                .changed()
            {
                *b = u8::try_from(v.clamp(0, 255)).unwrap_or(0x80);
                changed = true;
            }
        }
        k += 1;
    }
    if ui.button("Defaults").clicked() {
        st.sliders = None;
        if let Some(i) = &mut st.instance {
            let b = p.sliders.values.clone();
            apply_sliders(i, &b);
        }
    }
    if changed {
        st.sliders = Some(bytes.clone());
        if let Some(i) = &mut st.instance {
            apply_sliders(i, &bytes);
        }
    }
    ui.separator();
    let mut h = st.bone_scale.unwrap_or(0.5);
    if ui
        .add(egui::Slider::new(&mut h, 0.0..=1.0).text("height (AC_BoneScale_Test)"))
        .on_hover_text(
            "Neutral in M4: nothing in the data says which bones it scales \
             (appearance.md §6.2 Decision, OPEN O9)",
        )
        .changed()
    {
        st.bone_scale = Some(h);
    }
}

fn viewport_widget(
    ui: &mut Ui,
    st: &mut CharacterState,
    settings: &mut MeshSettings,
    vp: &mut Viewport,
) {
    let size = ui.available_size().max(vec2(64.0, 64.0));
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    let ppp = ui.ctx().pixels_per_point();
    let (width, height) = fit_viewport(
        (rect.width() * ppp).round() as u32,
        (rect.height() * ppp).round() as u32,
        vp.max_side(),
    );
    let aspect = width as f32 / height as f32;
    let (shift, scroll) = ui.input(|i| (i.modifiers.shift, i.smooth_scroll_delta.y));
    if resp.dragged() {
        let d = resp.drag_delta();
        if resp.dragged_by(egui::PointerButton::Secondary)
            || resp.dragged_by(egui::PointerButton::Middle)
            || shift
        {
            st.orbit.pan(d.x, d.y, rect.height());
        } else {
            st.orbit.rotate(d.x, d.y);
        }
    }
    if resp.hovered() && scroll != 0.0 {
        st.orbit.zoom(scroll / 50.0);
    }
    if resp.double_clicked() {
        st.reframe = true;
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(14, 15, 17));
    let Some(product) = st.shown.clone() else {
        return;
    };
    let scene = &product.scene;
    if st.framed != Some(scene.did) || st.reframe {
        if st.framed.is_none()
            && let Some([yaw, pitch]) = settings.angles
        {
            st.orbit.orbit.yaw = yaw.to_radians();
            st.orbit.orbit.pitch = pitch.to_radians();
        }
        // Frame the scaled figure: the bounds are in the setup frame (scale at the origin).
        let (c, r) = frame_sphere(scene, product.model_scale);
        st.orbit.frame(c, r, aspect);
        st.framed = Some(scene.did);
        st.reframe = false;
    }
    let id = vp.set_scene(scene);
    st.scene_id = Some(id);
    let drawn = pose_draw(st);
    if drawn.as_ref().is_some_and(|(_, fresh)| *fresh) {
        st.last_rig = st
            .instance
            .as_ref()
            .map(|i| Arc::clone(i.animator.skeleton()));
    }
    let pose = drawn.map(|(p, _)| Arc::new(p));
    // Keep the same Arc while the pose does not change, so equal requests stay equal.
    let pose = match (&st.last_pose, pose) {
        (Some(old), Some(new)) if **old == *new => Some(Arc::clone(old)),
        (_, new) => new,
    };
    st.last_pose.clone_from(&pose);
    vp.request(DrawRequest {
        scene: id,
        width,
        height,
        camera: st.orbit.camera(),
        shading: settings.shading,
        wireframe: settings.wireframe,
        normals: false,
        grid: settings.grid,
        highlight: false,
        isolate: false,
        hidden_parts: Vec::new(),
        hidden: Vec::new(),
        selected: None,
        dye: settings.dye_on.then_some(DyeOverride {
            color: settings.dye_color.map(|c| f32::from(c) / 255.0),
            channel: settings.dye_channel,
        }),
        diffuse_tint: settings.diffuse_tint,
        explode: 0.0,
        pose,
    });
    if let Some(t) = vp.texture() {
        painter.image(
            t.id(),
            rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    let mut overlay = Vec::new();
    if let Some(e) = vp.error() {
        overlay.push((e.to_owned(), Color32::LIGHT_RED));
    }
    let want = st.clip_did();
    let (clip_error, clip_loaded) = match &mut st.clip_job {
        Some(((d, _), j)) if Some(*d) == want => (j.error(), j.is_finished()),
        _ => (None, false),
    };
    let gray = Color32::from_gray(200);
    let clip_text = match (want, st.bound_clip(), clip_error) {
        (Some(d), _, Some(e)) => (
            format!("{} · clip {d:#010x} failed: {e}", product.title),
            Color32::LIGHT_RED,
        ),
        (Some(d), Some(c), None) if c.info.did == d => {
            let dur = c.bound.as_ref().map_or(0.0, |b| b.duration());
            (
                format!(
                    "{} · clip {d:#010x} · t = {:.3} / {dur:.3} s",
                    product.title,
                    st.play.clip_time(dur)
                ),
                gray,
            )
        }
        (Some(d), _, None) if !clip_loaded => (
            format!("{} · clip {d:#010x} loading …", product.title),
            gray,
        ),
        (Some(d), _, None) => (
            format!(
                "{} · clip {d:#010x} (not playable on this skeleton)",
                product.title
            ),
            gray,
        ),
        (None, _, _) => (format!("{} · reference pose", product.title), gray),
    };
    overlay.push(clip_text);
    let mut y = rect.top() + 6.0;
    for (text, color) in overlay {
        painter.text(
            pos2(rect.left() + 8.0, y),
            egui::Align2::LEFT_TOP,
            text,
            egui::FontId::proportional(13.0),
            color,
        );
        y += 18.0;
    }
    painter.text(
        pos2(rect.left() + 8.0, rect.bottom() - 6.0),
        egui::Align2::LEFT_BOTTOM,
        "Z up · drag: orbit · right/middle/Shift drag: pan · wheel: zoom · double-click: frame",
        egui::FontId::proportional(11.0),
        Color32::from_gray(120),
    );
}

/// The sphere to frame a figure on: the bind-pose bounds scaled by the model scale, with
/// the width counted at half (bind poses hold the arms out; animated figures do not).
#[must_use]
pub fn frame_sphere(scene: &crate::scene::SceneProduct, scale: f32) -> ([f32; 3], f32) {
    let s = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let Some((lo, hi)) = scene.aabb else {
        return (scene.sphere.0.map(|v| v * s), scene.sphere.1 * s);
    };
    let c = [0, 1, 2].map(|i| (lo[i] + hi[i]) * 0.5 * s);
    let h = [0, 1, 2].map(|i| (hi[i] - lo[i]).abs() * 0.5 * s);
    let r = (h[0] * 0.5).max(h[1]).max(h[2]) * 1.1;
    (c, if r.is_finite() && r > 1e-3 { r } else { 1.0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_by_record_type() {
        assert_eq!(
            Seed::of(0x4700_060A),
            Seed::Figure(SeedFigure::Object(0x4700_060A))
        );
        assert_eq!(
            Seed::of(0x0400_0553),
            Seed::Figure(SeedFigure::Setup(0x0400_0553))
        );
        assert_eq!(Seed::of(0x0500_8D08), Seed::Clip(0x0500_8D08));
        assert_eq!(Seed::of(0x7900_066F), Seed::Item(0x7900_066F));
        assert_eq!(Seed::of(0x7000_6986), Seed::Object(0x7000_6986));
        assert_eq!(Seed::of(0x2800_0000), Seed::Nothing);
    }

    #[test]
    fn seeding_respects_the_command_line_once() {
        let args = CharacterArgs::parse_words([
            "--character",
            "human,m",
            "--clip",
            "walk",
            "--time",
            "0.5",
        ])
        .unwrap();
        let mut st = CharacterState::new(args, true);
        assert_eq!(st.fixed_time, Some(0.5));
        assert!(!st.play.playing);
        // The first selection does not replace the command line's figure …
        st.seed(0x4700_060B);
        assert!(matches!(st.figure, Some(FigureArg::Playable { .. })));
        // … later ones do; an item is previewed; the same DID twice is applied once.
        st.seed(0x0500_0001);
        assert_eq!(st.clip, ClipChoice::Did(0x0500_0001));
        st.seed(0x4700_060E);
        assert_eq!(st.figure, Some(FigureArg::Object(0x4700_060E)));
        st.seed(0x0500_0002);
        assert_eq!(st.clip, ClipChoice::Did(0x0500_0002));
        st.seed(0x7900_066F);
        assert_eq!(st.preview, Some(ItemArg::new(0x7900_066F)));
        st.preview = None;
        st.seed(0x7900_066F);
        assert_eq!(st.preview, None);
        // Screenshot mode without --time pauses at 0.
        let st = CharacterState::new(CharacterArgs::default(), true);
        assert_eq!((st.fixed_time, st.play.playing), (Some(0.0), false));
        assert_eq!(st.clip, ClipChoice::State(StateArg::Idle));
    }

    #[test]
    fn the_first_click_counts_after_a_start_without_select() {
        // Regression: started with --character/--clip and no --select (the root DID map
        // is selected), the first figure or clip clicked was dropped.
        let args =
            CharacterArgs::parse_words(["--character", "human,m", "--clip", "walk"]).unwrap();
        let mut st = CharacterState::new(args, false);
        st.seed(tessera_props::ROOT_DID_MAP.0);
        assert!(matches!(st.figure, Some(FigureArg::Playable { .. })));
        assert_eq!(st.clip, ClipChoice::State(StateArg::Walk));
        st.seed(0x4700_060E);
        assert_eq!(st.figure, Some(FigureArg::Object(0x4700_060E)));
        st.seed(0x0500_0002);
        assert_eq!(st.clip, ClipChoice::Did(0x0500_0002));
        // A start-up clip selection keeps the command line's clip.
        let args = CharacterArgs::parse_words(["--clip", "walk"]).unwrap();
        let mut st = CharacterState::new(args, false);
        st.seed(0x0500_0002);
        assert_eq!(st.clip, ClipChoice::State(StateArg::Walk));
        assert!(!st.cli_keeps_figure(0x0500_0002), "no --character given");
    }

    #[test]
    fn replayed_command_lines_do_not_wear_an_item_twice() {
        let mut st = CharacterState::new(CharacterArgs::default(), false);
        st.seed(0x7900_066F);
        assert_eq!(st.preview, Some(ItemArg::new(0x7900_066F)));
        let line = st.command_line(0x7900_066F);
        let words: Vec<&str> = line.split(' ').collect();
        let again = CharacterArgs::parse_words(words).unwrap();
        assert_eq!(again.items, vec![ItemArg::new(0x7900_066F)]);
        // Replaying: the selection is worn already, so it is not previewed again.
        let mut st = CharacterState::new(again, false);
        st.seed(0x7900_066F);
        assert_eq!(st.preview, None);
        assert_eq!(st.items.len(), 1);
    }

    fn catalog_with_human() -> Catalog {
        Catalog {
            bodies: vec![crate::character::BodyEntry {
                species: 0x17,
                label: "Human".into(),
                sex: crate::charspec::SEX_MALE,
                sex_label: "m".into(),
                avatar: Did(0x7000_0001),
            }],
            ..Catalog::default()
        }
    }

    #[test]
    fn sliders_do_not_rebuild_the_figure() {
        // Regression: slider bytes and the height control were part of the rebuild key,
        // so every slider tick re-assembled and re-uploaded the figure.
        let cat = catalog_with_human();
        let mut st = CharacterState::new(CharacterArgs::default(), false);
        let a = st.request(&cat).unwrap();
        st.sliders = Some(vec![0x10, 0x20]);
        st.bone_scale = Some(0.9);
        let b = st.request(&cat).unwrap();
        assert_eq!(a, b);
        assert_eq!((b.sliders, b.bone_scale), (None, None));
        // Items and the detail level still rebuild.
        st.lod = 1;
        assert_ne!(st.request(&cat).unwrap(), a);
    }

    #[test]
    fn the_preview_takes_a_free_slot_only() {
        let cat = catalog_with_human();
        let mut st = CharacterState::new(CharacterArgs::default(), false);
        st.items = vec![ItemArg::new(0x7900_0001); MAX_ITEMS];
        st.preview = Some(ItemArg::new(0x7900_0002));
        assert_eq!(st.request(&cat).unwrap().items.len(), MAX_ITEMS);
        // The command line stays parseable.
        let line = st.command_line(0x2800_0000);
        let again = CharacterArgs::parse_words(line.split(' ')).unwrap();
        assert_eq!(again.items.len(), MAX_ITEMS);
        st.items.pop();
        let r = st.request(&cat).unwrap();
        assert_eq!(r.items.last(), Some(&ItemArg::new(0x7900_0002)));
    }

    #[test]
    fn a_catalogue_without_bodies_or_failed_settles() {
        // Regression: no bodies left the view "assembling …" for ever and screenshot mode
        // waited for its timeout.
        let cat = Catalog {
            problems: vec!["the install has no race table (RaceTableDirectory)".into()],
            ..Catalog::default()
        };
        let mut st = CharacterState::new(CharacterArgs::default(), true);
        st.catalog = Some(Job::ready(cat.clone()));
        assert!(!character_ready(&st, None));
        assert!(st.request(&cat).is_none());
        let e = st.figure_error.clone().unwrap();
        assert!(
            e.contains("no playable bodies") && e.contains("RaceTableDirectory"),
            "{e}"
        );
        assert!(character_ready(&st, None));
        // A failed catalogue is final too.
        let mut st = CharacterState::new(CharacterArgs::default(), true);
        st.catalog = Some(Job::failed("no registry"));
        assert!(character_ready(&st, None));
    }

    #[test]
    fn wrapping_play_time_keeps_one_step_of_motion() {
        // Regression: after 1024 cycles τ was wrapped while `before` was not, so the root
        // motion of that step ran back over 1024 cycles.
        let d = 1.8;
        let mut p = PlayState {
            time: d * 1024.0 - 0.01,
            ..PlayState::default()
        };
        let before = p.advance(0.05, 1.0, d);
        assert!(p.time < d, "wrapped: {}", p.time);
        assert!(
            (p.time - before - 0.05).abs() < 1e-3,
            "{before} → {}",
            p.time
        );
    }

    #[test]
    fn root_motion_is_scaled_like_the_bones() {
        // Regression: the figure moved by unscaled motion while bones and trails were
        // drawn at the model scale.
        let m = MotionDelta {
            translation: Vec3::new(0.0, 3.167, 0.0),
            rotation: Quat::from_rotation_z(0.3),
        };
        let mut p = PlayState::default();
        p.apply_motion(m, 0.7);
        assert!((p.placement.0 - Vec3::new(0.0, 0.7 * 3.167, 0.0)).length() < 1e-5);
        assert!((p.placement.1 - 0.3).abs() < 1e-5);
        // The trail starts where the cycle started and ends at the figure.
        let base = trail_base(p.placement_matrix(0.7), m);
        assert!(base.transform_point3(Vec3::ZERO).length() < 1e-5);
        assert!((base.transform_point3(m.translation) - p.placement.0).length() < 1e-5);
        // Garbage scales count as 1; garbage motion is ignored.
        let mut q = PlayState::default();
        q.apply_motion(m, f32::NAN);
        assert!((q.placement.0.y - 3.167).abs() < 1e-5);
        q.apply_motion(
            MotionDelta {
                translation: Vec3::splat(f32::INFINITY),
                rotation: Quat::IDENTITY,
            },
            1.0,
        );
        assert!(q.placement.0.is_finite());
    }

    fn chain(n: usize) -> Arc<Skeleton> {
        use tessera_havok::testkit::SkeletonSpec;
        Arc::new(
            Skeleton::from_setup_record(&SkeletonSpec::chain(n).to_setup_prefix(0x0400_0001))
                .unwrap(),
        )
    }

    #[test]
    fn a_rebuilt_figure_keeps_its_last_pose() {
        // Regression: a rebuilt instance drew its first frames unscaled in the bind pose
        // with held items at the feet.
        let (a, b, other) = (chain(3), chain(3), chain(4));
        let placement = PlayState::default().placement_matrix(0.7);
        let last = PoseDraw {
            palette: vec![Mat4::from_translation(Vec3::X); 3],
            attach: vec![(5, Mat4::IDENTITY)],
            ..PoseDraw::default()
        };
        let p = stand_in_pose(Some(&last), Some(&a), &b, placement);
        assert_eq!(p.placement, placement);
        assert_eq!((p.palette.len(), p.attach.len()), (3, 1));
        // Another rig: the reference pose, scaled, with no holding locations (held parts
        // are not drawn).
        let p = stand_in_pose(Some(&last), Some(&a), &other, placement);
        assert_eq!(p.placement, placement);
        assert!(p.palette.is_empty() && p.attach.is_empty());
        let p = stand_in_pose(None, None, &a, placement);
        assert!(p.palette.is_empty() && p.attach.is_empty());
    }

    #[test]
    fn command_line_times_parse_back() {
        // Regression: the raw play time of a long looping clip exceeded --time's range.
        assert_eq!(command_line_time(90_000.0, 0.0, Wrap::Loop), MAX_TIME);
        let t = command_line_time(90_000.25, 2.0, Wrap::Loop);
        assert!((t - 0.25).abs() < 2e-3, "{t}");
        assert_eq!(command_line_time(5.0, 2.0, Wrap::Once), 2.0);
        assert_eq!(command_line_time(f32::NAN, 0.0, Wrap::Loop), 0.0);
        let mut st = CharacterState::new(CharacterArgs::default(), false);
        st.play.time = 90_000.0;
        let line = st.command_line(0x2800_0000);
        assert!(
            CharacterArgs::parse_words(line.split(' ')).is_ok(),
            "{line}"
        );
    }

    #[test]
    fn play_time_advances_wraps_and_stops() {
        let mut p = PlayState::default();
        p.advance(0.05, 1.0, 1.0);
        assert!((p.time - 0.05).abs() < 1e-6);
        // Long stalls are clamped.
        p.advance(10.0, 1.0, 1.0);
        assert!((p.time - 0.15).abs() < 1e-6);
        p.time = 0.95;
        p.advance(0.1, 1.0, 1.0);
        assert!((p.clip_time(1.0) - 0.05).abs() < 1e-4);
        p.looping = false;
        p.time = 0.95;
        p.advance(0.1, 1.0, 1.0);
        assert_eq!(p.time, 1.0);
        assert!(!p.playing);
        // Garbage never poisons the time.
        let mut q = PlayState::default();
        q.advance(f32::NAN, f32::INFINITY, 1.0);
        assert!(q.time.is_finite());
        q.advance(0.05, 1.0, 0.0);
        assert_eq!(q.time, 0.0);
        q.speed = f32::NAN;
        q.advance(0.05, 1.0, 1.0);
        assert!(q.time.is_finite());
        let m = PlayState::default().placement_matrix(0.7);
        assert!((m.x_axis.x - 0.7).abs() < 1e-6);
        assert_eq!(
            PlayState::default().placement_matrix(f32::NAN),
            Mat4::IDENTITY
        );
    }

    #[test]
    fn command_lines_reproduce_the_view() {
        let args = CharacterArgs::parse_words([
            "--character",
            "human,m",
            "--wear",
            "0x790006C3@dye=0.5",
            "--clip",
            "run",
            "--time",
            "0.25",
            "--bones",
        ])
        .unwrap();
        let st = CharacterState::new(args.clone(), true);
        let line = st.command_line(0x2800_0000);
        assert!(
            line.starts_with("--select 0x28000000 --view character"),
            "{line}"
        );
        let words: Vec<&str> = line.split(' ').collect();
        let again = CharacterArgs::parse_words(words).unwrap();
        assert_eq!(again.figure, args.figure);
        assert_eq!(again.items, args.items);
        assert_eq!(again.clip, args.clip);
        assert_eq!(again.time, args.time);
        assert!(again.bones);
    }
}
