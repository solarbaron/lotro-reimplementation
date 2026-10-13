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

use std::path::Path;
use std::time::{Duration, Instant};

use tessera_input::controller::view_direction;
use tessera_input::{
    BindingSpec, FixedTimestep, FlyConfig, FlyController, HeightQuery, InputMapper, IntentLatch,
    Keymap, LookSettings, MoveIntent, RawEvent, StandardActions, WalkConfig, WalkController,
    default_keymap, default_registry,
};
use tessera_physics::controller::{TURN_KEY_SPEED_DEGREES, turn_rate_on_foot};
use tessera_render::glam::Vec3;
use tessera_world::WorldPos;

use super::movement::setup::MoverSetup;
use super::movement::{CollisionWorld, MoveInput, PhysicsWalker};

/// Distance from the controller origin that triggers a rebase.
pub const REBASE_DISTANCE: f32 = 8_000.0;
/// Eye height above the feet in walk mode.
pub const EYE_HEIGHT: f32 = 1.7;
/// Default third-person camera distance.
pub const DEFAULT_ZOOM: f32 = 7.0;
/// How far the walk-mode third-person camera is kept above the terrain (`walker.md` §5).
pub const EYE_ABOVE_TERRAIN: f64 = 0.8;

/// The walk-mode third-person camera kept above the terrain (`walker.md` §5: "kept 0.8 m
/// above the terrain and then aimed at the head"): an `eye` less than
/// [`EYE_ABOVE_TERRAIN`] above `ground` (the terrain height under it) is raised to it and
/// turned towards the head of the avatar standing at `feet`. Only the walk-mode camera
/// is kept: callers pass `feet` only in walk mode with the camera outside every cell
/// (`walker.md` §14.12 (a)); without `feet` (free-fly, scripted cameras) the eye and
/// direction are returned unchanged.
#[must_use]
pub fn keep_above_terrain(
    eye: WorldPos,
    dir: Vec3,
    ground: Option<f32>,
    feet: Option<WorldPos>,
) -> (WorldPos, Vec3) {
    let (Some(h), Some(feet)) = (ground, feet) else {
        return (eye, dir);
    };
    let min = f64::from(h) + EYE_ABOVE_TERRAIN;
    let below = eye.z < min;
    if !below {
        return (eye, dir);
    }
    let mut eye = eye;
    eye.z = min;
    let head = Vec3::new(
        (feet.x - eye.x) as f32,
        (feet.y - eye.y) as f32,
        (feet.z + f64::from(EYE_HEIGHT) - eye.z) as f32,
    );
    (eye, head.try_normalize().unwrap_or(dir))
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
    /// Third-person camera pitch (walk mode).
    cam_pitch: f32,
    /// Third-person camera distance.
    zoom: f32,
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
            cam_pitch: -0.25,
            zoom: DEFAULT_ZOOM,
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
    /// `pitch` (radians, negative = down); `pitch` also tilts the walk camera.
    pub fn place(&mut self, pos: &WorldPos, yaw: f32, fly_height: f32, pitch: f32) {
        self.region = pos.region;
        self.origin = [pos.x, pos.y];
        let p = Vec3::new(0.0, 0.0, pos.z as f32);
        self.walk.teleport(p);
        self.walk.set_yaw(yaw);
        let eye = p + Vec3::Z * fly_height;
        self.fly.teleport(eye);
        self.fly.set_orientation(yaw, pitch);
        self.cam_pitch = pitch.clamp(-1.4, 0.6);
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
            self.place(pos, yaw, fly_height, pitch);
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
            self.zoom = DEFAULT_ZOOM;
            self.cam_pitch = -0.25;
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
                self.cam_pitch = (self.cam_pitch + dpitch).clamp(-1.4, 0.6);
                if zoom.is_finite() && zoom != 0.0 {
                    self.zoom = (self.zoom * 0.85f32.powf(zoom)).clamp(1.5, 60.0);
                }
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
                let (feet, yaw) = self.physics_pose().unwrap_or_else(|| {
                    (
                        self.walk.interpolated(alpha),
                        self.walk.interpolated_yaw(alpha),
                    )
                });
                let [ahead, up] = self.camera_offset;
                let head = feet
                    + Vec3::Z * (EYE_HEIGHT + up)
                    + Vec3::new(yaw.cos(), yaw.sin(), 0.0) * ahead;
                let dir = view_direction(yaw, self.cam_pitch);
                (head - dir * self.zoom + Vec3::Z * 0.6, dir)
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

    /// The avatar's feet and yaw in walk mode.
    #[must_use]
    pub fn avatar(&self) -> Option<(WorldPos, f32)> {
        (self.mode == MoveMode::Walk).then(|| {
            let alpha = self.timestep.alpha();
            let (feet, yaw) = self.physics_pose().unwrap_or_else(|| {
                (
                    self.walk.interpolated(alpha),
                    self.walk.interpolated_yaw(alpha),
                )
            });
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
        let (e, d) = keep_above_terrain(eye, dir, Some(50.0), Some(feet));
        assert!((e.z - 50.8).abs() < 1e-9);
        assert!(d.x > 0.0 && d.z > 0.0, "aimed up at the head: {d:?}");
        let high = WorldPos::new(1, 100.0, 100.0, 60.0);
        assert_eq!(
            keep_above_terrain(high, dir, Some(50.0), Some(feet)),
            (high, dir)
        );
    }

    fn flat(_: f64, _: f64) -> Option<f32> {
        Some(100.0)
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
