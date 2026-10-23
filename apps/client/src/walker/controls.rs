// SPDX-License-Identifier: GPL-3.0-or-later
//! Input and movement for the walker: raw window events go through the
//! `tessera-input` mapper (Tessera's keymap file over the built-in bindings,
//! `keymap.md`), become a move intent and look changes, and drive the free-fly or the
//! walk controller at a fixed step (`keymap.md` §9).
//!
//! Walk mode has two controllers. With the world's collision sets (ROADMAP M4.5, decision
//! 0013) and once the mover is loaded ([`Controls::enable_physics`]), the avatar moves with
//! the `tessera-physics` character controller on its own 1/60 s accumulator
//! ([`PhysicsWalker`]): walls, objects and ceilings block it, it climbs steps, slides,
//! jumps, wades and swims, and enters interior cells. Without them (the collision system
//! switched off, or before the mover is loaded) the `tessera-input` walk controller follows
//! terrain heights on the 30 Hz step, as in M3–M4.
//!
//! Controller positions are f32 relative to a **controller origin** (f64 region-global)
//! that is moved only when the viewer gets more than [`REBASE_DISTANCE`] away from it,
//! so f32 precision stays sub-millimetre without disturbing motion every landblock.
//!
//! # The walk-mode camera (`walker.md` §11.5–§11.8)
//!
//! The third-person camera follows the in-world camera state ([`CameraProfile`]: the record
//! the `UNIQUEDB` key [`THIRD_PERSON_CAMERA_KEY`] names, else [`DEFAULT_THIRD_PERSON_CAMERA_KEY`],
//! read off the frame thread by [`load_camera_profile`]; until a record arrives, or without
//! one, [`CameraProfile::FALLBACK`] with the §11.8 values). At placement it stands behind
//! the character at the state's initial distance and pitch (§11.7 steps 2–4); the wheel
//! moves the distance goal within [minimum, maximum] and the shown distance follows it with
//! the zoom smoothing time ([`smooth_towards`]); the camera looks at the feet raised by
//! *f* × *H* × *s_z* ([`CameraProfile::look_at_raise`], §11.7 step 1) plus the target
//! block's offset in the character's frame. The target fade of §11.7 is drawn by the
//! character layer ([`CameraProfile::fade_goal`]).

use std::path::Path;
use std::time::{Duration, Instant};

use tessera_formats::Did;
use tessera_formats::environment::{CameraBody, CameraState};
use tessera_input::controller::view_direction;
use tessera_input::{
    BindingSpec, FixedTimestep, FlyConfig, FlyController, GroundState, HeightQuery, InputMapper,
    IntentLatch, Keymap, LookSettings, MoveIntent, RawEvent, StandardActions, WalkConfig,
    WalkController, default_keymap, default_registry,
};
use tessera_physics::controller::{MovementState, TURN_KEY_SPEED_DEGREES, turn_rate_on_foot};
use tessera_render::glam::Vec3;
use tessera_world::WorldPos;

use super::characters::CharacterLayer;
use super::movement::setup::MoverSetup;
use super::movement::{CollisionWorld, MoveInput, PhysicsWalker};

/// Distance from the controller origin that triggers a rebase.
pub const REBASE_DISTANCE: f32 = 8_000.0;
/// Eye height above the feet in walk mode.
pub const EYE_HEIGHT: f32 = 1.7;
/// How far the walk-mode third-person camera is kept above the terrain (`walker.md` §5).
pub const EYE_ABOVE_TERRAIN: f64 = 0.8;
/// The `UNIQUEDB` key of the game layer's third-person camera state (`walker.md` §11.6,
/// label `gmThirdPersonCamera`; BIN).
pub const THIRD_PERSON_CAMERA_KEY: u32 = 0x1000_0041;
/// The engine's `UNIQUEDB` key of the default third-person camera state, used when
/// [`THIRD_PERSON_CAMERA_KEY`] is missing (`walker.md` §11.6 Tessera Decision).
pub const DEFAULT_THIRD_PERSON_CAMERA_KEY: u32 = 0x0A;
/// The look-at height *H* × *s_z* the camera uses until the character's
/// `Entity_Camera_Lookat_Height` is known (implementer OPEN of `walker.md` §14.11): the
/// walker's eye height.
pub const DEFAULT_LOOK_AT_HEIGHT: f32 = EYE_HEIGHT;
/// Below this *H* × *s_z* the look-at point is not raised (`walker.md` §11.7 step 1).
pub const MIN_LOOK_AT_RAISE: f32 = 0.0002;
/// Largest distance, smoothing time and height factor a camera record may give (hostile
/// data bound; Tessera choice).
const MAX_CAMERA_DISTANCE: f32 = 1_000.0;
const MAX_SMOOTHING: f32 = 10.0;
const MAX_HEIGHT_FACTOR: f32 = 10.0;

/// The third-person camera state the walker follows (`walker.md` §11.5.3 type 1, §11.7):
/// the record's values in the walker's units (metres, degrees, seconds), checked.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraProfile {
    /// The camera-state record (0x08) it came from; `None` for [`CameraProfile::FALLBACK`].
    pub record: Option<Did>,
    /// Initial distance, metres (already within [`Self::min_distance`, `Self::max_distance`]).
    pub initial_distance: f32,
    /// Minimum distance, metres.
    pub min_distance: f32,
    /// Maximum distance, metres.
    pub max_distance: f32,
    /// Initial pitch, degrees (negative looks down, `walker.md` §11.7 OPEN Inferred).
    pub initial_pitch: f32,
    /// Whether the pitch is limited to [`Self::pitch_low`, `Self::pitch_high`].
    pub constrain_pitch: bool,
    /// Lowest pitch, degrees.
    pub pitch_low: f32,
    /// Highest pitch, degrees.
    pub pitch_high: f32,
    /// Vertical field of view, degrees (`walker.md` §11.7 OPEN Decision).
    pub fov: f32,
    /// Near clip distance, metres.
    pub near: f32,
    /// Zoom smoothing time, seconds.
    pub zoom_smoothing: f32,
    /// The target block's height factor *f*.
    pub height_factor: f32,
    /// The target block's offset in the character's frame (x right, y forward, z up),
    /// metres.
    pub target_offset: [f32; 3],
    /// Below this camera-to-look-at distance the character is drawn at
    /// [`Self::fade_opacity`], metres (0: no target fade).
    pub fade_distance: f32,
    /// The target fade opacity, 0 to 1.
    pub fade_opacity: f32,
}

impl CameraProfile {
    /// The walker's profile when the install's camera state cannot be read, and until it
    /// is: the values of `walker.md` §11.8 (the in-world state of §11.7).
    pub const FALLBACK: Self = Self {
        record: None,
        initial_distance: 6.0,
        min_distance: 1.0,
        max_distance: 20.0,
        initial_pitch: -15.0,
        constrain_pitch: true,
        pitch_low: -80.0,
        pitch_high: 60.0,
        fov: 45.0,
        near: 0.1,
        zoom_smoothing: 0.4,
        height_factor: 0.925,
        target_offset: [0.0; 3],
        fade_distance: 0.7,
        fade_opacity: 0.1,
    };

    /// The profile of a third-person camera state `state` (record `did`); `None` for
    /// another controller type or values that make no camera (non-finite numbers, a
    /// maximum distance below the minimum or at most 0, an empty pitch range). Values are
    /// limited to sane ranges (hostile data); the initial distance is clamped to [minimum,
    /// maximum] (`walker.md` §11.7 step 4).
    #[must_use]
    pub fn from_state(did: Did, state: &CameraState) -> Option<Self> {
        let CameraBody::ThirdPerson(b) = &state.body else {
            return None;
        };
        let finite = |v: f32| v.is_finite().then_some(v);
        let min_distance = finite(b.min_distance)?.clamp(0.0, MAX_CAMERA_DISTANCE);
        let max_distance = finite(b.max_distance)?.clamp(0.0, MAX_CAMERA_DISTANCE);
        if max_distance <= 0.0 || max_distance < min_distance {
            return None;
        }
        let (pitch_low, pitch_high) = (
            finite(b.pitch_low)?.clamp(-90.0, 90.0),
            finite(b.pitch_high)?.clamp(-90.0, 90.0),
        );
        if b.constrain_pitch && pitch_low > pitch_high {
            return None;
        }
        let smoothing = |v: f32| finite(v).map_or(0.0, |v| v.clamp(0.0, MAX_SMOOTHING));
        let offset = b.target.offset;
        Some(Self {
            record: Some(did),
            initial_distance: finite(b.initial_distance)?.clamp(min_distance, max_distance),
            min_distance,
            max_distance,
            initial_pitch: finite(b.initial_pitch)?.clamp(-90.0, 90.0),
            constrain_pitch: b.constrain_pitch,
            pitch_low,
            pitch_high,
            fov: finite(b.fov).filter(|f| *f > 1.0 && *f < 179.0)?,
            near: finite(state.near_clip)
                .filter(|n| *n > 0.0)
                .map_or(Self::FALLBACK.near, |n| n.min(10.0)),
            zoom_smoothing: smoothing(b.zoom_smoothing),
            height_factor: finite(b.target.height_factor)
                .map_or(0.0, |f| f.clamp(0.0, MAX_HEIGHT_FACTOR)),
            target_offset: if offset.iter().all(|v| v.is_finite() && v.abs() <= 100.0) {
                offset
            } else {
                [0.0; 3]
            },
            fade_distance: finite(b.target_fade_distance)
                .map_or(0.0, |d| d.clamp(0.0, MAX_CAMERA_DISTANCE)),
            fade_opacity: finite(b.target_fade_opacity).map_or(1.0, |o| o.clamp(0.0, 1.0)),
        })
    }

    /// The height the look-at point is raised above the feet (`walker.md` §11.7 step 1):
    /// *f* × *H* × *s_z* for a character whose look-at height times z scale is
    /// `height_times_scale`, 0 when that is below [`MIN_LOOK_AT_RAISE`] (or not finite).
    #[must_use]
    pub fn look_at_raise(&self, height_times_scale: f32) -> f32 {
        if !height_times_scale.is_finite() || height_times_scale < MIN_LOOK_AT_RAISE {
            return 0.0;
        }
        self.height_factor * height_times_scale
    }

    /// The look-at point (`walker.md` §11.7 step 1) of a character with feet `feet`, facing
    /// `yaw` (radians, 0 = east, counter-clockwise), whose look-at height times z scale is
    /// `height_times_scale`: the feet raised by [`Self::look_at_raise`], plus the target
    /// block's offset turned into the character's frame (x right, y forward, z up;
    /// Inferred) and the mount's `camera_offset` (forward, up; `mounts.md` §14.9).
    #[must_use]
    pub fn look_at_point(
        &self,
        feet: Vec3,
        yaw: f32,
        height_times_scale: f32,
        camera_offset: [f32; 2],
    ) -> Vec3 {
        let forward = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
        let right = Vec3::new(yaw.sin(), -yaw.cos(), 0.0);
        let [ox, oy, oz] = self.target_offset;
        let [ahead, up] = camera_offset;
        feet + Vec3::Z * (self.look_at_raise(height_times_scale) + oz + up)
            + right * ox
            + forward * (oy + ahead)
    }

    /// `distance` limited to [minimum, maximum]; a value that is not finite gives the
    /// initial distance.
    #[must_use]
    pub fn clamp_distance(&self, distance: f32) -> f32 {
        if distance.is_finite() {
            distance.clamp(self.min_distance, self.max_distance)
        } else {
            self.initial_distance
        }
    }

    /// `pitch` (radians) limited to the pitch range when the state constrains it, and
    /// always to just short of straight up or down (the camera never turns over).
    #[must_use]
    pub fn clamp_pitch(&self, pitch: f32) -> f32 {
        let limit = 89.0f32.to_radians();
        let p = if pitch.is_finite() {
            pitch
        } else {
            self.initial_pitch.to_radians()
        };
        let p = if self.constrain_pitch {
            p.clamp(self.pitch_low.to_radians(), self.pitch_high.to_radians())
        } else {
            p
        };
        p.clamp(-limit, limit)
    }

    /// The character's target opacity for a camera `distance` metres from the look-at
    /// point (`walker.md` §11.7 "Target fade"): the fade opacity closer than the fade
    /// distance, else 1.
    #[must_use]
    pub fn fade_goal(&self, distance: f32) -> f32 {
        if self.fade_distance > 0.0 && distance.is_finite() && distance < self.fade_distance {
            self.fade_opacity
        } else {
            1.0
        }
    }
}

impl Default for CameraProfile {
    fn default() -> Self {
        Self::FALLBACK
    }
}

/// Why the install's camera state could not be used ([`load_camera_profile`]).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CameraProfileError {
    /// Neither `UNIQUEDB` key names a record.
    #[error("no third-person camera state in the UNIQUEDB map")]
    NotNamed,
    /// The record is missing or unreadable.
    #[error("camera state {0} missing or unreadable")]
    Missing(Did),
    /// The record does not decode.
    #[error("camera state {0}: {1}")]
    Undecodable(Did, String),
    /// The record decodes but is not a usable third-person state.
    #[error("camera state {0} is not a usable third-person state")]
    Unusable(Did),
}

/// The walker's camera profile (`walker.md` §11.6): the record that `unique` gives for
/// [`THIRD_PERSON_CAMERA_KEY`], else for [`DEFAULT_THIRD_PERSON_CAMERA_KEY`], read through
/// `read` and decoded. Blocking: call it off the frame thread.
///
/// # Errors
///
/// [`CameraProfileError`] when no key names a record or the record is not a usable
/// third-person state; the caller keeps [`CameraProfile::FALLBACK`].
pub fn select_camera_profile(
    unique: impl Fn(u32) -> Option<Did>,
    read: impl Fn(Did) -> Option<Vec<u8>>,
) -> Result<CameraProfile, CameraProfileError> {
    let did = unique(THIRD_PERSON_CAMERA_KEY)
        .or_else(|| unique(DEFAULT_THIRD_PERSON_CAMERA_KEY))
        .filter(|d| !d.is_null())
        .ok_or(CameraProfileError::NotNamed)?;
    let bytes = read(did).ok_or(CameraProfileError::Missing(did))?;
    let state = CameraState::parse(&bytes)
        .map_err(|e| CameraProfileError::Undecodable(did, e.to_string()))?;
    CameraProfile::from_state(did, &state).ok_or(CameraProfileError::Unusable(did))
}

/// [`select_camera_profile`] over an install's game data: the `UNIQUEDB` map is the root
/// DID map's entry [`tessera_formats::gameplay::common::UNIQUEDB_ROOT_KEY`] (the client
/// addresses maps by key, `records.md` §8.2), else the entry labelled `UNIQUEDB`.
/// Blocking: call it off the frame thread.
///
/// # Errors
///
/// As [`select_camera_profile`].
pub fn load_camera_profile<S>(
    data: &tessera_props::GameData<S>,
) -> Result<CameraProfile, CameraProfileError>
where
    S: tessera_props::RecordSource,
{
    use tessera_formats::gameplay::common::{UNIQUEDB_LABEL, UNIQUEDB_ROOT_KEY};
    let map = data.root_did_map().ok().and_then(|root| {
        let did = root
            .get(UNIQUEDB_ROOT_KEY)
            .map(|e| e.value)
            .or_else(|| root.by_label(UNIQUEDB_LABEL))?;
        data.did_map(did).ok().flatten()
    });
    let Some(map) = map else {
        return Err(CameraProfileError::NotNamed);
    };
    select_camera_profile(
        |key| map.get(key).map(|e| e.value),
        |did| data.read(did).ok().flatten(),
    )
}

/// One step of `current` towards `goal` over `dt` seconds with smoothing time `tau`
/// (`walker.md` §11.7 "Every frame afterwards"; the curve is the implementer OPEN of
/// §14.11: an exponential approach with time constant `tau`). `tau` at most 0, or a step
/// that is not finite, reaches the goal at once.
#[must_use]
pub fn smooth_towards(current: f32, goal: f32, dt: f32, tau: f32) -> f32 {
    if !current.is_finite() || tau.is_nan() || tau <= 0.0 || !dt.is_finite() {
        return goal;
    }
    let k = 1.0 - (-dt.max(0.0) / tau).exp();
    let next = current + (goal - current) * k;
    if next.is_finite() { next } else { goal }
}

/// The walk-mode third-person camera kept above the terrain (`walker.md` §5: "kept 0.8 m
/// above the terrain"; §11.7: the camera looks at the look-at point): an `eye` less than
/// [`EYE_ABOVE_TERRAIN`] above `ground` (the terrain height under it) is raised to it and
/// turned towards the look-at point. Only the walk-mode camera is kept: callers pass
/// `feet` (the avatar's) only in walk mode with the camera outside every cell (`walker.md`
/// §14.12 (a)); without `feet` (free-fly, scripted cameras) the eye and direction are
/// returned unchanged.
///
/// The look-at point is taken on the unraised view ray `eye` + *t* × `dir` (the walk camera
/// stands on that ray, [`Controls::eye`]): the point of it straight above or below `feet`
/// (nearest to the vertical through them). That is the camera's look-at point whenever the
/// point is straight above the feet (the on-foot camera without a sideways or forward
/// target offset); [`keep_above_terrain_towards`] with [`Controls::camera_target`] takes the
/// exact point in every case.
#[must_use]
pub fn keep_above_terrain(
    eye: WorldPos,
    dir: Vec3,
    ground: Option<f32>,
    feet: Option<WorldPos>,
) -> (WorldPos, Vec3) {
    let target = feet.map(|f| target_on_ray(&eye, dir, &f));
    keep_above_terrain_towards(eye, dir, ground, target)
}

/// [`keep_above_terrain`] towards the look-at point `target` (walk mode only; `None`: the
/// eye and direction unchanged).
#[must_use]
pub fn keep_above_terrain_towards(
    eye: WorldPos,
    dir: Vec3,
    ground: Option<f32>,
    target: Option<WorldPos>,
) -> (WorldPos, Vec3) {
    let (Some(h), Some(target)) = (ground, target) else {
        return (eye, dir);
    };
    let min = f64::from(h) + EYE_ABOVE_TERRAIN;
    if eye.z >= min || !eye.z.is_finite() {
        return (eye, dir);
    }
    let mut eye = eye;
    eye.z = min;
    let to = Vec3::new(
        (target.x - eye.x) as f32,
        (target.y - eye.y) as f32,
        (target.z - eye.z) as f32,
    );
    (eye, to.try_normalize().unwrap_or(dir))
}

/// The point of the ray `eye` + *t* × `dir` (*t* ≥ 0) nearest to the vertical line through
/// `feet` ([`keep_above_terrain`]); a ray that is (nearly) vertical gives the feet raised
/// by the fallback profile's look-at raise.
fn target_on_ray(eye: &WorldPos, dir: Vec3, feet: &WorldPos) -> WorldPos {
    let (hx, hy) = (f64::from(dir.x), f64::from(dir.y));
    let len2 = hx * hx + hy * hy;
    let t = ((feet.x - eye.x) * hx + (feet.y - eye.y) * hy) / len2;
    if len2 < 1e-8 || !t.is_finite() {
        let raise = CameraProfile::FALLBACK.look_at_raise(DEFAULT_LOOK_AT_HEIGHT);
        return WorldPos::new(feet.region, feet.x, feet.y, feet.z + f64::from(raise));
    }
    let t = t.max(0.0);
    WorldPos::new(
        eye.region,
        eye.x + hx * t,
        eye.y + hy * t,
        eye.z + f64::from(dir.z) * t,
    )
}

/// How the viewer moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveMode {
    /// Free camera.
    Fly,
    /// Avatar on the ground with a third-person camera.
    Walk,
}

impl MoveMode {
    /// Display name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Fly => "fly",
            Self::Walk => "walk",
        }
    }
}

/// One-shot requests raised by input this frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ControlEvents {
    /// Quit (menu key).
    pub quit: bool,
    /// Take a screenshot.
    pub screenshot: bool,
    /// Toggle the detailed HUD.
    pub toggle_debug: bool,
    /// The mode changed.
    pub mode_changed: bool,
    /// Reset the camera.
    pub reset_camera: bool,
    /// A `menu` press that reached game code while a game UI is attached (the UI declined
    /// it): the host runs the Escape rule instead of quitting (`walker.md` §5).
    pub menu: bool,
    /// The `toggle_mount` action was pressed (walk mode only; `walker.md` §16.4).
    pub toggle_mount: bool,
}

/// Loads the keymap: `path` if given, else the default keymap file when it exists, laid
/// over the built-in bindings. Problems are logged; the result is always usable.
#[must_use]
pub fn load_keymap(path: Option<&Path>) -> Keymap {
    let base = default_keymap();
    let path = match path {
        Some(p) => Some(p.to_path_buf()),
        None => tessera_input::default_keymap_path().ok(),
    };
    let Some(path) = path else { return base };
    match Keymap::load_file(&path) {
        Ok(Some(parsed)) => {
            for w in &parsed.warnings {
                tracing::warn!(file = %path.display(), "keymap: {w}");
            }
            tracing::info!(file = %path.display(), "keymap loaded");
            base.overlay(&parsed.keymap)
        }
        Ok(None) => {
            tracing::debug!(file = %path.display(), "no keymap file; built-in bindings");
            base
        }
        Err(e) => {
            tracing::warn!(file = %path.display(), "keymap unreadable ({e}); built-in bindings");
            base
        }
    }
}

/// The key-help line of the HUD, from the bindings in force (`keymap` after the user's
/// overlay): for an axis its first keyboard/mouse key each way (`W/S`), for a button its
/// first one, `-` when unbound. Gamepad bindings are left out to keep the line short.
#[must_use]
pub fn help_text(keymap: &Keymap) -> String {
    use tessera_input::contexts::{CAMERA, FLY, GLOBAL, MOVEMENT};
    let chords = |context: &str, action: &str| -> Vec<(String, f32)> {
        keymap
            .get(context, action)
            .unwrap_or(&[])
            .iter()
            .filter_map(|b| match b {
                BindingSpec::Digital { chord, value } => Some((chord.to_string(), *value)),
                BindingSpec::Analog { .. } => None,
            })
            .filter(|(c, _)| !c.contains("Pad"))
            .collect()
    };
    let button = |context: &str, action: &str| -> String {
        chords(context, action)
            .into_iter()
            .next()
            .map_or_else(|| "-".to_owned(), |(c, _)| c)
    };
    let axis = |context: &str, action: &str| -> String {
        let list = chords(context, action);
        let plus = list.iter().find(|(_, v)| *v > 0.0).map(|(c, _)| c.as_str());
        let minus = list.iter().find(|(_, v)| *v < 0.0).map(|(c, _)| c.as_str());
        match (plus, minus) {
            (Some(p), Some(m)) => format!("{p}/{m}"),
            (Some(one), None) | (None, Some(one)) => one.to_owned(),
            (None, None) => "-".to_owned(),
        }
    };
    format!(
        "{} move, {} turn, {} strafe, {} look, {} up/down, {} fly/walk, {} mount, {} details, {} screenshot, {} quit",
        axis(MOVEMENT, "move_forward"),
        axis(MOVEMENT, "turn"),
        axis(MOVEMENT, "strafe"),
        button(CAMERA, "mouselook"),
        axis(FLY, "fly_vertical"),
        button(GLOBAL, "toggle_fly"),
        button(MOVEMENT, "toggle_mount"),
        button(GLOBAL, "toggle_debug"),
        button(GLOBAL, "screenshot"),
        button(GLOBAL, "menu"),
    )
}

/// Input mapper, controllers and camera state (module docs).
#[derive(Debug)]
pub struct Controls {
    mapper: InputMapper,
    actions: StandardActions,
    look: LookSettings,
    timestep: FixedTimestep,
    latch: IntentLatch,
    start: Instant,
    autorun: bool,
    /// Autorun at walking speed (scripted runs, `--autorun walk`).
    autowalk: bool,
    mode: MoveMode,
    origin: [f64; 2],
    region: u8,
    fly: FlyController,
    walk: WalkController,
    /// Third-person camera pitch (walk mode), radians.
    cam_pitch: f32,
    /// Third-person camera distance shown, metres (follows [`Self::zoom_goal`]).
    zoom: f32,
    /// The distance the wheel asks for, within the profile's range.
    zoom_goal: f32,
    /// The camera state followed (`walker.md` §11.5–§11.7).
    profile: CameraProfile,
    /// The character's look-at height × z scale (*H* × *s_z*, `walker.md` §11.7 step 1).
    look_at_height: f32,
    /// The user turned the pitch or zoomed since the last placement or camera reset (a
    /// profile arriving later then keeps the user's view).
    camera_touched: bool,
    placed: bool,
    /// A game UI is attached: `menu` opens the game menu instead of quitting.
    ui_attached: bool,
    /// The mover of the collision walk mode, once loaded.
    mover: Option<MoverSetup>,
    /// The collision walk mode's controller (walk mode on `tessera-physics`).
    physics: Option<PhysicsWalker>,
    /// The camera's look-at offsets while mounted, (forward, up) m (`mounts.md` §14.9).
    camera_offset: [f32; 2],
}

impl Controls {
    /// Controls with `keymap`, starting in `mode`.
    #[must_use]
    pub fn new(keymap: &Keymap, mode: MoveMode, fly_speed: f32) -> Self {
        let (mut mapper, warnings) = InputMapper::new(default_registry(), keymap);
        for w in warnings {
            tracing::warn!("keymap: {w}");
        }
        // The registry is the built-in one, so every id exists.
        let actions = StandardActions::new(mapper.registry())
            .unwrap_or_else(|| unreachable!("the default registry defines every standard action"));
        match mode {
            MoveMode::Fly => actions.enter_fly_mode(&mut mapper),
            MoveMode::Walk => actions.enter_walk_mode(&mut mapper),
        }
        let fly = FlyController::new(
            Vec3::ZERO,
            0.0,
            -0.2,
            FlyConfig {
                speed: fly_speed,
                ..FlyConfig::default()
            },
        );
        Self {
            mapper,
            actions,
            look: LookSettings::default(),
            timestep: FixedTimestep::new(30.0),
            latch: IntentLatch::default(),
            start: Instant::now(),
            autorun: false,
            autowalk: false,
            mode,
            origin: [0.0, 0.0],
            region: 0,
            fly,
            walk: WalkController::new(Vec3::ZERO, 0.0, WalkConfig::default()),
            cam_pitch: CameraProfile::FALLBACK.initial_pitch.to_radians(),
            zoom: CameraProfile::FALLBACK.initial_distance,
            zoom_goal: CameraProfile::FALLBACK.initial_distance,
            profile: CameraProfile::FALLBACK,
            look_at_height: DEFAULT_LOOK_AT_HEIGHT,
            camera_touched: false,
            placed: false,
            mover: None,
            physics: None,
            camera_offset: [0.0; 2],
            ui_attached: false,
        }
    }

    /// The input mapper (the game UI's step 1 feeds it with the UI as its sink) and the
    /// mapper's clock.
    pub fn mapper_mut(&mut self) -> (&mut InputMapper, Duration) {
        let now = self.start.elapsed();
        (&mut self.mapper, now)
    }

    /// Whether a game UI is attached (`walker.md` §5: Escape then gives the game menu).
    pub fn set_ui_attached(&mut self, on: bool) {
        self.ui_attached = on;
    }

    /// Whether a game UI is attached.
    #[must_use]
    pub fn ui_attached(&self) -> bool {
        self.ui_attached
    }

    /// Switches walk mode to the collision controller with `mover` (the avatar's capsule,
    /// the player movement record and conditions). A placed avatar keeps its position and
    /// facing. Whenever the collision controller is placed (here, [`Controls::place`],
    /// [`Controls::correct`], entering walk mode from fly mode), its first step lifts feet
    /// found below the terrain onto the support (`walker.md` §13.1).
    pub fn enable_physics(&mut self, mover: MoverSetup) {
        if self.placed {
            let feet = self.to_world(self.walk.position());
            let mut w = PhysicsWalker::new(
                &mover,
                feet.region,
                [feet.x, feet.y, feet.z],
                self.walk.yaw(),
            );
            w.check_placement();
            self.physics = Some(w);
        }
        self.mover = Some(mover);
    }

    /// The collision walk mode's controller, when active.
    #[must_use]
    pub fn physics(&self) -> Option<&PhysicsWalker> {
        self.physics.as_ref()
    }

    /// The collision walk mode's controller, mutable (mount requests, `walker.md` §16).
    pub fn physics_mut(&mut self) -> Option<&mut PhysicsWalker> {
        self.physics.as_mut()
    }

    /// The follow camera's target offsets while mounted (`mounts.md` §14.9 Decision):
    /// moved forward along the heading by `offset[0]` and raised by `offset[1]` metres.
    /// Values that are not finite count as 0.
    pub fn set_camera_offset(&mut self, offset: [f32; 2]) {
        self.camera_offset = offset.map(|v| {
            if v.is_finite() {
                v.clamp(-10.0, 10.0)
            } else {
                0.0
            }
        });
    }

    /// Follows the camera state `profile` (`walker.md` §11.6: the install's record, read off
    /// the frame thread with [`load_camera_profile`]). Before the user has turned or zoomed
    /// the camera (since the last placement or reset) it takes the profile's initial
    /// distance and pitch; afterwards the user's view is kept within the new ranges. The
    /// same profile again changes nothing.
    #[allow(
        dead_code,
        reason = "called by the walker's frame loop once `walker/mod.rs` calls `follow_characters`"
    )]
    pub fn set_camera_profile(&mut self, profile: CameraProfile) {
        if profile == self.profile {
            return;
        }
        self.profile = profile;
        if self.camera_touched {
            self.zoom_goal = profile.clamp_distance(self.zoom_goal);
            self.zoom = profile.clamp_distance(self.zoom);
            self.cam_pitch = profile.clamp_pitch(self.cam_pitch);
        } else {
            self.reset_view();
        }
    }

    /// The camera state followed.
    #[must_use]
    #[allow(
        dead_code,
        reason = "called by the walker's frame loop once `walker/mod.rs` calls `follow_characters`"
    )]
    pub fn camera_profile(&self) -> &CameraProfile {
        &self.profile
    }

    /// The character's `Entity_Camera_Lookat_Height` times its z scale (*H* × *s_z*,
    /// `walker.md` §11.7 step 1), or `None` to go back to [`DEFAULT_LOOK_AT_HEIGHT`].
    /// Values that are not finite or negative count as `None`.
    #[allow(
        dead_code,
        reason = "called by the walker's frame loop once `walker/mod.rs` calls `follow_characters`"
    )]
    pub fn set_look_at_height(&mut self, height_times_scale: Option<f32>) {
        self.look_at_height = height_times_scale
            .filter(|h| h.is_finite() && *h >= 0.0)
            .map_or(DEFAULT_LOOK_AT_HEIGHT, |h| h.min(MAX_CAMERA_DISTANCE));
    }

    /// The walk camera's distance: (shown, goal), metres.
    #[must_use]
    #[allow(
        dead_code,
        reason = "called by the walker's frame loop once `walker/mod.rs` calls `follow_characters`"
    )]
    pub fn zoom(&self) -> (f32, f32) {
        (self.zoom, self.zoom_goal)
    }

    /// The walk camera's pitch, radians.
    #[must_use]
    #[allow(
        dead_code,
        reason = "called by the walker's frame loop once `walker/mod.rs` calls `follow_characters`"
    )]
    pub fn camera_pitch(&self) -> f32 {
        self.cam_pitch
    }

    /// Follows the character layer's camera state and the player's look-at height
    /// (`walker.md` §11.6–§11.7): [`Self::set_camera_profile`] with
    /// [`CharacterLayer::camera_profile`] and [`Self::set_look_at_height`] with
    /// [`CharacterLayer::player_look_at_height`]; then gives the layer the profile and the
    /// look-at point followed ([`CharacterLayer::set_camera_view`]), so that the target
    /// fade measures to the point the camera looks at. Cheap; the walker's frame loop calls
    /// it every frame after the character layer's update and before its draws (nothing is
    /// read here).
    #[allow(
        dead_code,
        reason = "called by the walker's frame loop once `walker/mod.rs` calls `follow_characters`"
    )]
    pub fn follow_characters(&mut self, characters: &mut CharacterLayer) {
        self.set_camera_profile(*characters.camera_profile());
        self.set_look_at_height(characters.player_look_at_height());
        characters.set_camera_view(self.profile, self.camera_target());
    }

    /// The profile's initial distance and pitch (`walker.md` §11.7 steps 3–4).
    fn reset_view(&mut self) {
        self.zoom = self.profile.initial_distance;
        self.zoom_goal = self.profile.initial_distance;
        self.cam_pitch = self
            .profile
            .clamp_pitch(self.profile.initial_pitch.to_radians());
        self.camera_touched = false;
    }

    /// The look-at point of the walk camera for feet `feet` facing `yaw` (controller
    /// frame): the feet raised by [`CameraProfile::look_at_raise`], plus the target block's
    /// offset turned into the character's frame (x right, y forward, z up; `walker.md`
    /// §11.7 step 1, Inferred) and the mount's offsets (`mounts.md` §14.9).
    fn look_at(&self, feet: Vec3, yaw: f32) -> Vec3 {
        self.profile
            .look_at_point(feet, yaw, self.look_at_height, self.camera_offset)
    }

    /// The walk avatar's feet (controller frame) and heading, interpolated: the collision
    /// controller's, else the terrain walk controller's.
    fn walk_pose(&self) -> (Vec3, f32) {
        let alpha = self.timestep.alpha();
        self.physics_pose().unwrap_or_else(|| {
            (
                self.walk.interpolated(alpha),
                self.walk.interpolated_yaw(alpha),
            )
        })
    }

    /// The walk camera's look-at point (`walker.md` §11.7 step 1; the point the camera
    /// orbits and the target fade measures to), in walk mode.
    #[must_use]
    #[allow(
        dead_code,
        reason = "called by `follow_characters`, and by the walker's terrain clamp once `walker/mod.rs` passes it to `keep_above_terrain_towards`"
    )]
    pub fn camera_target(&self) -> Option<WorldPos> {
        (self.mode == MoveMode::Walk).then(|| {
            let (feet, yaw) = self.walk_pose();
            self.to_world(self.look_at(feet, yaw))
        })
    }

    /// Whether walk mode runs on the collision controller.
    fn on_physics(&self) -> bool {
        self.mode == MoveMode::Walk && self.physics.is_some()
    }

    /// The collision controller's feet (controller frame) and heading, interpolated.
    fn physics_pose(&self) -> Option<(Vec3, f32)> {
        let p = self.physics.as_ref()?;
        let (feet, yaw) = p.interpolated();
        let w = WorldPos::new(p.region(), feet[0], feet[1], feet[2]);
        Some((self.to_local(&w), yaw))
    }

    /// Starts (or stops) moving forward on its own, walking or running (`--autorun`).
    pub fn set_autorun(&mut self, on: bool, walk: bool) {
        self.autorun = on;
        self.autowalk = walk;
    }

    /// The movement mode.
    #[must_use]
    pub fn mode(&self) -> MoveMode {
        self.mode
    }

    /// Whether [`Controls::place`] has been called.
    #[must_use]
    pub fn placed(&self) -> bool {
        self.placed
    }

    /// Feeds one raw input event.
    pub fn handle(&mut self, event: &RawEvent) {
        let now = self.start.elapsed();
        self.mapper.handle(event, now);
    }

    /// Puts the viewer at `pos` facing `yaw` (radians, 0 = east, counter-clockwise): the
    /// avatar's feet at `pos`, the fly camera `fly_height` above it looking down by
    /// `pitch` (radians, negative = down). The walk camera takes the camera state's
    /// initial distance and pitch, behind the character (`walker.md` §11.7 steps 2–4,
    /// §11.8); `pitch` (the `--pitch` option) sets the fly camera only (implementer OPEN of
    /// `walker.md` §14.11).
    pub fn place(&mut self, pos: &WorldPos, yaw: f32, fly_height: f32, pitch: f32) {
        self.region = pos.region;
        self.origin = [pos.x, pos.y];
        let p = Vec3::new(0.0, 0.0, pos.z as f32);
        self.walk.teleport(p);
        self.walk.set_yaw(yaw);
        let eye = p + Vec3::Z * fly_height;
        self.fly.teleport(eye);
        self.fly.set_orientation(yaw, pitch);
        self.reset_view();
        self.timestep.reset();
        self.placed = true;
        if let Some(m) = &self.mover {
            match &mut self.physics {
                Some(w) => {
                    w.teleport(pos.region, [pos.x, pos.y, pos.z], yaw);
                }
                None => {
                    let mut w = PhysicsWalker::new(m, pos.region, [pos.x, pos.y, pos.z], yaw);
                    w.check_placement();
                    self.physics = Some(w);
                }
            }
        }
    }

    /// Moves the viewer to `pos` keeping the facing, the camera pitch and the zoom (an
    /// authoritative correction of the session): the walk avatar's feet in walk mode, the
    /// fly camera in fly mode (the position each mode reports).
    pub fn correct(&mut self, pos: &WorldPos) {
        if !self.placed || pos.region != self.region {
            let yaw = self.heading();
            let pitch = self.fly.pitch();
            let fly_height = match self.mode {
                MoveMode::Fly => 0.0,
                MoveMode::Walk => self.fly.position().z - self.walk.position().z,
            };
            let view = (
                self.placed,
                self.cam_pitch,
                self.zoom,
                self.zoom_goal,
                self.camera_touched,
            );
            self.place(pos, yaw, fly_height, pitch);
            if view.0 {
                // A correction keeps the camera's pitch and zoom.
                (
                    _,
                    self.cam_pitch,
                    self.zoom,
                    self.zoom_goal,
                    self.camera_touched,
                ) = view;
            }
            return;
        }
        let p = self.to_local(pos);
        match self.mode {
            MoveMode::Fly => self.fly.teleport(p),
            MoveMode::Walk => self.walk.teleport(p),
        }
        if self.mode == MoveMode::Walk
            && let Some(w) = &mut self.physics
        {
            let yaw = w.state().heading;
            w.teleport(pos.region, [pos.x, pos.y, pos.z], yaw);
        }
        self.timestep.reset();
    }

    /// Sets the fly camera directly (scripted flights): position and view angles.
    pub fn set_fly_pose(&mut self, pos: &WorldPos, yaw: f32, pitch: f32) {
        if pos.region != self.region || !self.placed {
            self.region = pos.region;
            self.origin = [pos.x, pos.y];
            self.placed = true;
        }
        let p = self.to_local(pos);
        self.fly.teleport(p);
        self.fly.set_orientation(yaw, pitch);
    }

    fn to_local(&self, pos: &WorldPos) -> Vec3 {
        Vec3::new(
            (pos.x - self.origin[0]) as f32,
            (pos.y - self.origin[1]) as f32,
            pos.z as f32,
        )
    }

    fn to_world(&self, p: Vec3) -> WorldPos {
        WorldPos::new(
            self.region,
            self.origin[0] + f64::from(p.x),
            self.origin[1] + f64::from(p.y),
            f64::from(p.z),
        )
    }

    /// Per frame: publishes the input snapshot, applies look changes, runs the fixed
    /// ticks of the active controller and handles mode toggles. Walk mode steps the
    /// collision controller over `world` when both are there, else the terrain-height
    /// walk controller against `ground` (region-global heights).
    pub fn frame(
        &mut self,
        dt: f32,
        ground: &dyn Fn(f64, f64) -> Option<f32>,
        world: Option<&dyn CollisionWorld>,
    ) -> ControlEvents {
        let now: Duration = self.start.elapsed();
        self.mapper.update(now);
        let a = self.actions;
        let mut ev = ControlEvents {
            quit: !self.ui_attached && self.mapper.just_pressed(a.menu),
            menu: self.ui_attached && self.mapper.just_pressed(a.menu),
            screenshot: self.mapper.just_pressed(a.screenshot),
            toggle_debug: self.mapper.just_pressed(a.toggle_debug),
            reset_camera: self.mapper.just_pressed(a.reset_camera),
            mode_changed: false,
            toggle_mount: self.mapper.just_pressed(a.toggle_mount),
        };
        if self.mapper.just_pressed(a.toggle_fly) {
            self.toggle_mode();
            ev.mode_changed = true;
        }
        if self.mapper.just_pressed(a.autorun) {
            self.autorun = !self.autorun;
        }
        if ev.reset_camera {
            self.reset_view();
        }
        if !self.placed {
            return ev;
        }
        let (dyaw, dpitch) = a.look(&self.mapper, &self.look, dt, false);
        let zoom = self.mapper.delta(a.zoom) + self.mapper.value(a.zoom) * dt * 4.0;
        match self.mode {
            MoveMode::Fly => {
                self.fly.look(dyaw, dpitch);
                let notches = self.mapper.delta(a.fly_speed);
                if notches != 0.0 {
                    self.fly.scale_speed(notches);
                }
            }
            MoveMode::Walk => {
                self.walk.turn(dyaw);
                if let Some(w) = &mut self.physics {
                    w.turn(dyaw);
                }
                if dpitch.is_finite() && dpitch != 0.0 {
                    self.cam_pitch = self.profile.clamp_pitch(self.cam_pitch + dpitch);
                    self.camera_touched = true;
                }
                if zoom.is_finite() && zoom != 0.0 {
                    self.zoom_goal = self
                        .profile
                        .clamp_distance(self.zoom_goal * 0.85f32.powf(zoom));
                    self.camera_touched = true;
                }
                // `walker.md` §11.7: the distance moves towards its goal with the zoom
                // smoothing time.
                self.zoom = self.profile.clamp_distance(smooth_towards(
                    self.zoom,
                    self.zoom_goal,
                    dt,
                    self.profile.zoom_smoothing,
                ));
            }
        }
        let mut intent = a.intent(&self.mapper, self.autorun);
        if self.autorun && self.autowalk {
            intent.walk = true;
        }
        if self.on_physics()
            && let (Some(world), Some(w)) = (world, self.physics.as_mut())
        {
            // The collision controller owns its 1/60 s accumulator and latches the jump.
            // The player's input follows `collision.md` §11.8: the command values without
            // autorun, which is run lock there (forward held until forward or backward is
            // pressed); walk mode halves them, and the style follows the halved values.
            let raw = a.intent(&self.mapper, false);
            if w.run_lock() != self.autorun {
                w.set_run_lock(self.autorun);
            }
            let i = MoveIntent {
                walk: intent.walk,
                ..raw
            }
            .sanitized();
            let input = MoveInput {
                forward: i.forward,
                strafe: i.strafe,
                heading: None,
                jump: i.jump,
            };
            // §11.9: the keyboard turns at `[Input] TurnKeySpeed`, initially 150°/s.
            let turn = i.turn * turn_rate_on_foot(TURN_KEY_SPEED_DEGREES);
            let _ = w.frame(f64::from(dt.max(0.0)), &input, turn, i.walk, world);
            self.autorun = w.run_lock();
            // Keep the terrain walk controller with the avatar (mode toggles, rebase).
            let (feet, yaw) = {
                let s = w.state();
                (
                    WorldPos::new(w.region(), s.position[0], s.position[1], s.position[2]),
                    s.heading,
                )
            };
            let local = self.to_local(&feet);
            self.walk.teleport(local);
            self.walk.set_yaw(yaw);
            self.maybe_rebase();
            return ev;
        }
        self.latch.frame(intent);
        let ticks = self.timestep.advance(f64::from(dt.max(0.0)));
        let step = self.timestep.dt() as f32;
        let origin = self.origin;
        let q = |x: f32, y: f32| ground(origin[0] + f64::from(x), origin[1] + f64::from(y));
        for _ in 0..ticks {
            let i = self.latch.tick();
            match self.mode {
                MoveMode::Fly => self.fly.tick(&i, step),
                MoveMode::Walk => {
                    let _ = self.walk.tick(&i, &q as &dyn HeightQuery, step);
                }
            }
        }
        self.maybe_rebase();
        ev
    }

    fn toggle_mode(&mut self) {
        match self.mode {
            MoveMode::Fly => {
                // Land under the camera.
                let p = self.fly.position();
                self.walk.teleport(p);
                self.walk.set_yaw(self.fly.yaw());
                let w = self.to_world(p);
                let yaw = self.fly.yaw();
                if let Some(phys) = &mut self.physics {
                    phys.teleport(w.region, [w.x, w.y, w.z], yaw);
                }
                self.mode = MoveMode::Walk;
                self.actions.enter_walk_mode(&mut self.mapper);
            }
            MoveMode::Walk => {
                let (eye, _) = self.eye_and_dir();
                self.fly.teleport(eye);
                self.fly.set_orientation(self.walk.yaw(), self.cam_pitch);
                self.mode = MoveMode::Fly;
                self.actions.enter_fly_mode(&mut self.mapper);
            }
        }
        self.timestep.reset();
    }

    fn maybe_rebase(&mut self) {
        let p = match self.mode {
            MoveMode::Fly => self.fly.position(),
            MoveMode::Walk => self.walk.position(),
        };
        if p.x.abs().max(p.y.abs()) < REBASE_DISTANCE {
            return;
        }
        let shift = Vec3::new(p.x, p.y, 0.0);
        self.origin[0] += f64::from(p.x);
        self.origin[1] += f64::from(p.y);
        let (fy, fp) = (self.fly.yaw(), self.fly.pitch());
        self.fly.teleport(self.fly.position() - shift);
        self.fly.set_orientation(fy, fp);
        let wy = self.walk.yaw();
        self.walk.teleport(self.walk.position() - shift);
        self.walk.set_yaw(wy);
        self.timestep.reset();
    }

    /// Eye position (controller frame) and unit view direction, interpolated.
    fn eye_and_dir(&self) -> (Vec3, Vec3) {
        let alpha = self.timestep.alpha();
        match self.mode {
            MoveMode::Fly => (
                self.fly.interpolated(alpha),
                view_direction(self.fly.interpolated_yaw(alpha), self.fly.pitch()),
            ),
            MoveMode::Walk => {
                let (feet, yaw) = self.walk_pose();
                // `walker.md` §11.7: behind the character at the shown distance, looking at
                // the look-at point.
                let target = self.look_at(feet, yaw);
                let dir = view_direction(yaw, self.cam_pitch);
                (target - dir * self.zoom, dir)
            }
        }
    }

    /// Eye (world) and unit view direction for rendering.
    #[must_use]
    pub fn eye(&self) -> (WorldPos, Vec3) {
        let (e, d) = self.eye_and_dir();
        (self.to_world(e), d)
    }

    /// The focus of streaming and the HUD: the fly camera, or the avatar's feet.
    #[must_use]
    pub fn focus(&self) -> WorldPos {
        let alpha = self.timestep.alpha();
        if self.on_physics()
            && let Some((p, _)) = self.physics_pose()
        {
            return self.to_world(p);
        }
        let p = match self.mode {
            MoveMode::Fly => self.fly.interpolated(alpha),
            MoveMode::Walk => self.walk.interpolated(alpha),
        };
        self.to_world(p)
    }

    /// Facing in radians (0 = east, counter-clockwise).
    #[must_use]
    pub fn heading(&self) -> f32 {
        let alpha = self.timestep.alpha();
        if self.on_physics()
            && let Some((_, yaw)) = self.physics_pose()
        {
            return yaw;
        }
        match self.mode {
            MoveMode::Fly => self.fly.interpolated_yaw(alpha),
            MoveMode::Walk => self.walk.interpolated_yaw(alpha),
        }
    }

    /// Current velocity (world units per second).
    #[must_use]
    pub fn velocity(&self) -> Vec3 {
        if self.on_physics()
            && let Some(w) = &self.physics
        {
            return Vec3::from_array(w.state().velocity);
        }
        match self.mode {
            MoveMode::Fly => self.fly.velocity(),
            MoveMode::Walk => self.walk.velocity(),
        }
    }

    /// Whether the controller stands on the ground, for the gait of the moves it reports
    /// (`tessera-protocol.md` §8.1): `false` while jumping or falling (collision controller
    /// state 4, or the plain walk controller airborne) and for the free-fly camera.
    /// Swimming, wading and sliding count as on the ground.
    #[must_use]
    pub fn on_ground(&self) -> bool {
        if self.on_physics()
            && let Some(w) = &self.physics
        {
            return w.state().element != MovementState::EarthNoSurface;
        }
        match self.mode {
            MoveMode::Fly => false,
            MoveMode::Walk => self.walk.state() != GroundState::Airborne,
        }
    }

    /// The avatar's feet and yaw in walk mode.
    #[must_use]
    pub fn avatar(&self) -> Option<(WorldPos, f32)> {
        (self.mode == MoveMode::Walk).then(|| {
            let (feet, yaw) = self.walk_pose();
            (self.to_world(feet), yaw)
        })
    }

    /// Fly speed (units per second).
    #[must_use]
    pub fn fly_speed(&self) -> f32 {
        self.fly.config.speed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_input::KeyCode;

    /// Review finding (`walker.md` §5): only the walk-mode camera is kept 0.8 m above the
    /// terrain; a free-fly or scripted camera (no avatar) may go below it.
    #[test]
    fn only_the_walk_camera_is_kept_above_the_terrain() {
        let eye = WorldPos::new(1, 100.0, 100.0, 49.5);
        let dir = Vec3::X;
        assert_eq!(
            keep_above_terrain(eye, dir, Some(50.0), None),
            (eye, dir),
            "free-fly"
        );
        assert_eq!(keep_above_terrain(eye, dir, None, Some(eye)), (eye, dir));
        let feet = WorldPos::new(1, 107.0, 100.0, 50.0);
        let (e, _) = keep_above_terrain(eye, dir, Some(50.0), Some(feet));
        assert!((e.z - 50.8).abs() < 1e-9);
        let high = WorldPos::new(1, 100.0, 100.0, 60.0);
        assert_eq!(
            keep_above_terrain(high, dir, Some(50.0), Some(feet)),
            (high, dir)
        );
    }

    /// Review finding (`walker.md` §11.7): the raised camera turns towards the look-at point
    /// (the feet + f × H × s_z), not towards a head height. Flat ground, the fallback
    /// profile, 6 m behind, tilted up by 10°: the eye would be 0.53 m above the ground, so
    /// it is raised to 0.8 m and aimed at the 1.5725 m look-at point.
    #[test]
    fn the_raised_walk_camera_aims_at_the_look_at_point() {
        let feet = WorldPos::new(1, 100.0, 100.0, 50.0);
        let raise = CameraProfile::FALLBACK.look_at_raise(DEFAULT_LOOK_AT_HEIGHT);
        assert!((raise - 1.5725).abs() < 1e-5);
        let target = WorldPos::new(1, feet.x, feet.y, feet.z + f64::from(raise));
        let dir = view_direction(0.3, 10f32.to_radians());
        let eye = WorldPos::new(
            1,
            target.x - f64::from(dir.x) * 6.0,
            target.y - f64::from(dir.y) * 6.0,
            target.z - f64::from(dir.z) * 6.0,
        );
        assert!(eye.z < 50.8);
        let aim = |e: WorldPos| {
            Vec3::new(
                (target.x - e.x) as f32,
                (target.y - e.y) as f32,
                (target.z - e.z) as f32,
            )
            .normalize()
        };
        let (e, d) = keep_above_terrain(eye, dir, Some(50.0), Some(feet));
        assert!((e.z - 50.8).abs() < 1e-9);
        assert!(d.distance(aim(e)) < 1e-4, "{d:?} vs {:?}", aim(e));
        let (e2, d2) = keep_above_terrain_towards(eye, dir, Some(50.0), Some(target));
        assert_eq!(e2, e);
        assert!(d2.distance(d) < 1e-4);
        // A look-at point off the vertical of the feet (a sideways target offset): the exact
        // point given.
        let side = WorldPos::new(1, target.x + 0.4, target.y - 0.2, target.z);
        let (e3, d3) = keep_above_terrain_towards(eye, dir, Some(50.0), Some(side));
        let want = Vec3::new(
            (side.x - e3.x) as f32,
            (side.y - e3.y) as f32,
            (side.z - e3.z) as f32,
        )
        .normalize();
        assert!(d3.distance(want) < 1e-5);
        // Looking straight down: the feet raised by the fallback look-at height.
        let down = Vec3::new(0.0, 0.0, -1.0);
        let low = WorldPos::new(1, 100.0, 100.0, 50.2);
        let (_, d4) = keep_above_terrain(low, down, Some(50.0), Some(feet));
        assert!(d4.z > 0.9, "{d4:?}");
    }

    /// The walk camera's look-at point is on its view ray at the shown distance, and it is
    /// the point [`CameraProfile::look_at_point`] gives (the one the character layer's target
    /// fade measures to).
    #[test]
    fn the_camera_target_is_the_point_the_camera_looks_at() {
        let mut c = Controls::new(&default_keymap(), MoveMode::Walk, 40.0);
        let start = WorldPos::new(1, 20_000.0, 20_000.0, 100.0);
        c.place(&start, 0.7, 30.0, -0.25);
        c.set_camera_offset([0.5, 0.3]);
        let target = c.camera_target().expect("walk mode");
        let (eye, dir) = c.eye();
        let zoom = c.zoom().0;
        let on_ray = [
            eye.x + f64::from(dir.x * zoom),
            eye.y + f64::from(dir.y * zoom),
            eye.z + f64::from(dir.z * zoom),
        ];
        assert!(
            (on_ray[0] - target.x).abs() < 1e-3
                && (on_ray[1] - target.y).abs() < 1e-3
                && (on_ray[2] - target.z).abs() < 1e-3,
            "{on_ray:?} {target:?}"
        );
        let p = CameraProfile::FALLBACK.look_at_point(
            Vec3::new(0.0, 0.0, 100.0),
            0.7,
            DEFAULT_LOOK_AT_HEIGHT,
            [0.5, 0.3],
        );
        assert!((target.x - start.x - f64::from(p.x)).abs() < 1e-3);
        assert!((target.y - start.y - f64::from(p.y)).abs() < 1e-3);
        assert!((target.z - f64::from(p.z)).abs() < 1e-3);
        let mut fly = Controls::new(&default_keymap(), MoveMode::Fly, 40.0);
        fly.place(&start, 0.0, 30.0, -0.25);
        assert_eq!(fly.camera_target(), None);
    }

    fn flat(_: f64, _: f64) -> Option<f32> {
        Some(100.0)
    }

    /// A synthetic third-person camera state (values invented for the test).
    fn third_person(distance: f32, min: f32, max: f32) -> CameraState {
        use tessera_formats::environment::{TargetBlock, ThirdPersonBody};
        CameraState {
            entity: Did(0x4700_0010),
            collision_smoothing: 0.1,
            near_clip: 0.2,
            far_clip: 1_500.0,
            uses_physics: true,
            uses_water_physics: false,
            follows_under_water: false,
            auto_transitions_disabled: false,
            body: CameraBody::ThirdPerson(ThirdPersonBody {
                zoom_smoothing: 0.5,
                target: TargetBlock {
                    height_factor: 0.8,
                    offset: [0.0, 0.0, 0.25],
                    ..TargetBlock::default()
                },
                constrain_pitch: true,
                pitch_high: 40.0,
                pitch_low: -70.0,
                min_distance: min,
                max_distance: max,
                fov: 50.0,
                initial_pitch: -20.0,
                initial_distance: distance,
                target_fade_distance: 0.5,
                target_fade_opacity: 0.25,
                ..ThirdPersonBody::default()
            }),
        }
    }

    /// `walker.md` §11.6: key 0x10000041 first, then 0x0A; the record decodes into the
    /// profile; other controller types and unreadable records are refused.
    #[test]
    fn the_camera_state_comes_from_the_uniquedb_keys() {
        let state = third_person(8.0, 2.0, 12.0);
        let bytes = state.to_bytes();
        let read = |d: Did| (d == Did(0x0800_0007)).then(|| bytes.clone());
        let p = select_camera_profile(
            |k| (k == THIRD_PERSON_CAMERA_KEY).then_some(Did(0x0800_0007)),
            read,
        )
        .expect("profile");
        assert_eq!(p.record, Some(Did(0x0800_0007)));
        assert_eq!(
            (p.initial_distance, p.min_distance, p.max_distance),
            (8.0, 2.0, 12.0)
        );
        assert_eq!(
            (p.initial_pitch, p.pitch_low, p.pitch_high),
            (-20.0, -70.0, 40.0)
        );
        assert_eq!((p.fov, p.near, p.zoom_smoothing), (50.0, 0.2, 0.5));
        assert_eq!(
            (p.height_factor, p.fade_distance, p.fade_opacity),
            (0.8, 0.5, 0.25)
        );
        // The engine key when the game layer's is missing.
        let p = select_camera_profile(
            |k| (k == DEFAULT_THIRD_PERSON_CAMERA_KEY).then_some(Did(0x0800_0007)),
            |d: Did| (d == Did(0x0800_0007)).then(|| bytes.clone()),
        )
        .expect("profile");
        assert_eq!(p.record, Some(Did(0x0800_0007)));
        assert_eq!(
            select_camera_profile(|_| None, |_| None),
            Err(CameraProfileError::NotNamed)
        );
        assert!(matches!(
            select_camera_profile(|_| Some(Did(0x0800_0001)), |_| Some(vec![1, 2, 3])),
            Err(CameraProfileError::Undecodable(..))
        ));
        let mut cinematic = state;
        cinematic.body = CameraBody::Cinematic(tessera_formats::environment::CinematicBody {
            fov: 60.0,
            target_smoothing: 0.1,
            angle_smoothing: 0.1,
        });
        let c = cinematic.to_bytes();
        assert_eq!(
            select_camera_profile(|_| Some(Did(0x0800_0005)), |_| Some(c.clone())),
            Err(CameraProfileError::Unusable(Did(0x0800_0005)))
        );
        // An initial distance outside the range is clamped (§11.7 step 4); a range that
        // makes no camera is refused.
        let p = CameraProfile::from_state(Did(1), &third_person(40.0, 2.0, 12.0)).unwrap();
        assert_eq!(p.initial_distance, 12.0);
        assert_eq!(
            CameraProfile::from_state(Did(1), &third_person(5.0, 9.0, 4.0)),
            None
        );
        assert_eq!(
            CameraProfile::from_state(Did(1), &third_person(f32::NAN, 1.0, 4.0)),
            None
        );
    }

    /// `walker.md` §11.7–§11.8: placed behind the character at 6 m and −15°, the wheel
    /// moves the goal within 1–20 m and the shown distance follows with the 0.4 s
    /// smoothing.
    #[test]
    fn zoom_is_clamped_and_smoothed() {
        let p = CameraProfile::FALLBACK;
        assert_eq!(p.clamp_distance(0.2), 1.0);
        assert_eq!(p.clamp_distance(55.0), 20.0);
        assert_eq!(p.clamp_distance(f32::NAN), 6.0);
        // The exponential approach: one time constant covers 1 − 1/e of the way.
        let d = smooth_towards(6.0, 16.0, 0.4, 0.4);
        assert!(
            (d - (6.0 + 10.0 * (1.0 - (-1.0f32).exp()))).abs() < 1e-4,
            "{d}"
        );
        assert_eq!(smooth_towards(6.0, 16.0, 0.1, 0.0), 16.0, "no smoothing");
        assert_eq!(smooth_towards(f32::NAN, 3.0, 0.1, 0.4), 3.0);

        let mut c = Controls::new(&default_keymap(), MoveMode::Walk, 40.0);
        let start = WorldPos::new(1, 20_000.0, 20_000.0, 100.0);
        c.place(&start, 0.0, 30.0, -0.25);
        assert_eq!(c.zoom(), (6.0, 6.0));
        assert!((c.camera_pitch() - (-15.0f32).to_radians()).abs() < 1e-6);
        let (eye, dir) = c.eye();
        // Behind (west of) the character facing east, above the look-at point.
        assert!(eye.x < start.x && dir.z < 0.0, "{eye:?} {dir:?}");
        let target_z = 100.0 + f64::from(0.925 * DEFAULT_LOOK_AT_HEIGHT);
        let back = 6.0 * (-15.0f64).to_radians().cos();
        assert!((eye.x - (start.x - back)).abs() < 1e-3, "{eye:?}");
        assert!(
            (eye.z - (target_z + 6.0 * 15.0f64.to_radians().sin())).abs() < 1e-3,
            "{eye:?}"
        );
        // Ten wheel notches out: the goal stops at the maximum, the shown distance follows.
        for _ in 0..30 {
            c.handle(&RawEvent::Wheel { dx: 0.0, dy: -1.0 });
            let _ = c.frame(1.0 / 60.0, &flat, None);
        }
        assert_eq!(c.zoom().1, 20.0, "goal clamped to the maximum");
        assert!(c.zoom().0 < 20.0, "the shown distance lags: {:?}", c.zoom());
        for _ in 0..240 {
            let _ = c.frame(1.0 / 60.0, &flat, None);
        }
        assert!((c.zoom().0 - 20.0).abs() < 0.05, "{:?}", c.zoom());
        for _ in 0..60 {
            c.handle(&RawEvent::Wheel { dx: 0.0, dy: 1.0 });
            let _ = c.frame(1.0 / 60.0, &flat, None);
        }
        assert_eq!(c.zoom().1, 1.0, "goal clamped to the minimum");
    }

    /// `walker.md` §11.7 step 1: the look-at point is the feet raised by f × H × s_z, not
    /// raised below 0.0002; the target block's offset is in the character's frame.
    #[test]
    fn the_look_at_point_is_the_raised_feet() {
        let p = CameraProfile::FALLBACK;
        assert!(
            (p.look_at_raise(1.75) - 1.618_75).abs() < 1e-5,
            "1.75 m body → 1.62 m"
        );
        assert_eq!(p.look_at_raise(0.0001), 0.0);
        assert_eq!(p.look_at_raise(f32::NAN), 0.0);

        let mut c = Controls::new(&default_keymap(), MoveMode::Walk, 40.0);
        c.set_camera_profile(
            CameraProfile::from_state(Did(7), &third_person(4.0, 1.0, 10.0)).unwrap(),
        );
        c.set_look_at_height(Some(1.5));
        let start = WorldPos::new(1, 20_000.0, 20_000.0, 100.0);
        // Facing north: the camera is south of the character.
        c.place(&start, std::f32::consts::FRAC_PI_2, 30.0, -0.25);
        let (eye, dir) = c.eye();
        let pitch = (-20.0f64).to_radians();
        let target = [start.x, start.y, 100.0 + 0.8 * 1.5 + 0.25];
        let want = [
            target[0],
            target[1] - 4.0 * pitch.cos(),
            target[2] - 4.0 * pitch.sin(),
        ];
        assert!(
            (eye.x - want[0]).abs() < 1e-3 && (eye.y - want[1]).abs() < 1e-3,
            "{eye:?} {want:?}"
        );
        assert!((eye.z - want[2]).abs() < 1e-3, "{eye:?} {want:?}");
        assert!(dir.y > 0.9);
        // Back to the default height when unknown.
        c.set_look_at_height(None);
        let (eye2, _) = c.eye();
        assert!((eye2.z - eye.z - f64::from(0.8 * (DEFAULT_LOOK_AT_HEIGHT - 1.5))).abs() < 1e-3);
    }

    /// `walker.md` §11.7 "Target fade": closer than the fade distance the character is
    /// drawn at the fade opacity.
    #[test]
    fn the_target_fades_below_the_fade_distance() {
        let p = CameraProfile::FALLBACK;
        assert_eq!(p.fade_goal(0.69), 0.1);
        assert_eq!(p.fade_goal(0.7), 1.0);
        assert_eq!(p.fade_goal(6.0), 1.0);
        assert_eq!(p.fade_goal(f32::NAN), 1.0);
        let none = CameraProfile {
            fade_distance: 0.0,
            ..p
        };
        assert_eq!(none.fade_goal(0.1), 1.0, "no target fade");
    }

    /// A profile that arrives after the user zoomed keeps the user's view within its
    /// ranges; before, it gives its initial view.
    #[test]
    fn a_late_profile_keeps_the_users_view() {
        let late = CameraProfile::from_state(Did(7), &third_person(4.0, 1.0, 10.0)).unwrap();
        let mut c = Controls::new(&default_keymap(), MoveMode::Walk, 40.0);
        c.place(
            &WorldPos::new(1, 20_000.0, 20_000.0, 100.0),
            0.0,
            30.0,
            -0.25,
        );
        c.set_camera_profile(late);
        assert_eq!(c.zoom(), (4.0, 4.0));
        let mut c = Controls::new(&default_keymap(), MoveMode::Walk, 40.0);
        c.place(
            &WorldPos::new(1, 20_000.0, 20_000.0, 100.0),
            0.0,
            30.0,
            -0.25,
        );
        for _ in 0..20 {
            c.handle(&RawEvent::Wheel { dx: 0.0, dy: -1.0 });
            let _ = c.frame(1.0 / 60.0, &flat, None);
        }
        c.set_camera_profile(late);
        assert_eq!(c.zoom().1, 10.0, "the user's goal, within the new range");
    }

    #[test]
    fn walk_follows_ground_and_keys_move() {
        let mut c = Controls::new(&default_keymap(), MoveMode::Walk, 40.0);
        let start = WorldPos::new(1, 20_000.0, 20_000.0, 100.0);
        c.place(&start, 0.0, 30.0, -0.25);
        c.handle(&RawEvent::Key {
            key: KeyCode::W,
            pressed: true,
            repeat: false,
        });
        for _ in 0..60 {
            let _ = c.frame(1.0 / 60.0, &flat, None);
        }
        let f = c.focus();
        assert!(f.x > start.x + 3.0, "moved east: {f:?}");
        assert!((f.z - 100.0).abs() < 1e-3, "on the ground: {f:?}");
        let (eye, dir) = c.eye();
        assert!(eye.x < f.x, "camera behind the avatar");
        assert!(dir.x > 0.5);
    }

    #[test]
    fn help_follows_the_keymap() {
        let base = default_keymap();
        let text = help_text(&base);
        assert!(text.contains("F3 details"), "{text}");
        assert!(text.contains("Ctrl+M mount"), "{text}");
        assert!(text.starts_with("W/S move, A/D turn, E/Q strafe"), "{text}");
        let mut user = Keymap::new();
        user.set(
            tessera_input::contexts::GLOBAL,
            "toggle_debug",
            vec![BindingSpec::parse("F9").unwrap()],
        )
        .unwrap();
        user.set(tessera_input::contexts::GLOBAL, "menu", Vec::new())
            .unwrap();
        let text = help_text(&base.overlay(&user));
        assert!(
            text.contains("F9 details") && !text.contains("F3"),
            "{text}"
        );
        assert!(text.contains("- quit"), "{text}");
    }

    #[test]
    fn correction_moves_and_keeps_the_view() {
        for mode in [MoveMode::Walk, MoveMode::Fly] {
            let mut c = Controls::new(&default_keymap(), mode, 40.0);
            c.place(
                &WorldPos::new(1, 20_000.0, 20_000.0, 100.0),
                1.0,
                30.0,
                -0.4,
            );
            let heading = c.heading();
            let (_, dir) = c.eye();
            let target = WorldPos::new(1, 20_050.0, 19_990.0, 105.0);
            c.correct(&target);
            let f = c.focus();
            assert!(
                (f.x - target.x).abs() < 1e-3 && (f.y - target.y).abs() < 1e-3,
                "{mode:?}: {f:?}"
            );
            assert!((c.heading() - heading).abs() < 1e-6);
            assert!((c.eye().1 - dir).length() < 1e-5, "{mode:?}");
        }
    }

    /// Flat collision ground at 100 m on the 3 × 3 landblocks around r1 (100, 100).
    fn flat_collision() -> (super::super::movement::run::StaticWorld, f64) {
        use std::sync::Arc;
        use tessera_formats::landblock::{HeightGrid, height_from_raw};
        use tessera_formats::{LandblockData, LandblockId};
        use tessera_physics::geometry::{CollisionLimits, CollisionSetBuilder, GridMesh};
        let raw = (100.0f32 * 65_535.0 / 2_000.0).round() as u16;
        let mut world = super::super::movement::run::StaticWorld::default();
        let centre = tessera_world::LandblockKey::new(1, 100, 100).unwrap();
        for dy in -1..=1 {
            for dx in -1..=1 {
                let k = centre.offset(dx, dy).unwrap();
                let data =
                    LandblockData::from_heights(k.data_did(), HeightGrid::from_fn(|_, _| raw));
                let mut b = CollisionSetBuilder::new(CollisionLimits::default());
                b.set_landblock(LandblockId::from_did(k.data_did()).unwrap());
                b.add_grid(Arc::new(GridMesh::terrain(&data))).unwrap();
                world.sets.insert(k, Arc::new(b.build()));
            }
        }
        (world, f64::from(height_from_raw(raw)))
    }

    #[test]
    fn physics_walk_mode_moves_toggles_and_lands_on_the_ground() {
        let (world, g) = flat_collision();
        let ground = |_: f64, _: f64| Some(g as f32);
        let mut c = Controls::new(&default_keymap(), MoveMode::Walk, 40.0);
        let start = WorldPos::new(1, 16_080.0, 16_080.0, g);
        c.place(&start, 0.0, 30.0, -0.25);
        assert!(
            c.physics().is_none(),
            "no mover yet: the terrain walk controller"
        );
        c.enable_physics(MoverSetup::builtin());
        assert!(c.physics().is_some());
        c.handle(&RawEvent::Key {
            key: KeyCode::W,
            pressed: true,
            repeat: false,
        });
        for _ in 0..60 {
            let _ = c.frame(1.0 / 60.0, &ground, Some(&world));
        }
        let f = c.focus();
        assert!(
            f.x > start.x + 3.0,
            "moved east on the collision controller: {f:?}"
        );
        assert!((f.z - g).abs() < 0.02, "on the ground: {f:?}");
        let steps = c.physics().unwrap().state().steps;
        assert!(steps >= 55, "{steps}");
        c.handle(&RawEvent::Key {
            key: KeyCode::W,
            pressed: false,
            repeat: false,
        });
        // Fly mode, the camera below the terrain, back to walk mode: lands on the ground.
        c.toggle_mode();
        assert_eq!(c.mode(), MoveMode::Fly);
        let low = WorldPos::new(1, 16_090.0, 16_080.0, g - 4.0);
        c.set_fly_pose(&low, 0.0, -0.2);
        c.toggle_mode();
        assert_eq!(c.mode(), MoveMode::Walk);
        assert!(c.physics().unwrap().placement_pending());
        for _ in 0..120 {
            let _ = c.frame(1.0 / 60.0, &ground, Some(&world));
        }
        let f = c.focus();
        assert!((f.z - g).abs() < 0.02, "lifted onto the ground: {f:?}");
        assert!((f.x - low.x).abs() < 0.5, "{f:?}");
        // A correction below the terrain lands too.
        c.correct(&WorldPos::new(1, 16_070.0, 16_080.0, g - 2.0));
        for _ in 0..60 {
            let _ = c.frame(1.0 / 60.0, &ground, Some(&world));
        }
        assert!((c.focus().z - g).abs() < 0.02, "{:?}", c.focus());
        // Without the world the terrain walk controller runs instead.
        let before = c.physics().unwrap().state().steps;
        for _ in 0..10 {
            let _ = c.frame(1.0 / 60.0, &ground, None);
        }
        assert_eq!(c.physics().unwrap().state().steps, before);
    }

    #[test]
    fn toggle_and_rebase_keep_position() {
        let mut c = Controls::new(&default_keymap(), MoveMode::Fly, 1_000.0);
        c.place(
            &WorldPos::new(1, 10_000.0, 10_000.0, 50.0),
            0.0,
            30.0,
            -0.25,
        );
        c.handle(&RawEvent::Key {
            key: KeyCode::W,
            pressed: true,
            repeat: false,
        });
        for _ in 0..900 {
            let _ = c.frame(1.0 / 60.0, &flat, None);
        }
        let f = c.focus();
        assert!(f.x > 17_000.0, "flew far: {f:?}");
        assert!(c.origin[0] > 10_000.0, "rebased");
        let before = c.focus();
        c.handle(&RawEvent::Key {
            key: KeyCode::W,
            pressed: false,
            repeat: false,
        });
        c.toggle_mode();
        assert_eq!(c.mode(), MoveMode::Walk);
        let after = c.focus();
        assert!((after.x - before.x).abs() < 50.0);
    }
}
