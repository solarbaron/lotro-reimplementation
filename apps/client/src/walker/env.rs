// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's environment (`walker.md` §14.3): the game clock with its start time and
//! rate (`environment.md` §11.1, epoch OPEN, so settable), the day description of the sky
//! key under the camera with its keyframe blending and cross-fade (`tessera-world`'s
//! [`Environment`]), interior fog of the camera's cell (§7.2, §12), and from them the
//! frame's lighting: sun or moon with its shadow, ambient and hemispheric terms, distance
//! fog (`lighting.md` §8.1, §9, §10), the point lights of the Light entities near the eye
//! (§8.3) and the global distance limit *G* (`walker.md` §11.3). The sky objects of the
//! description in use are drawn around the camera (`environment.md` §11.4, §12).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tessera_formats::Did;
use tessera_formats::PropertyValue;
use tessera_formats::terrain_type::FloatCurve as StoredCurve;
use tessera_material::{CurveState, FloatCurve};
use tessera_render::glam::Vec3;
use tessera_render::{Fog, FrameLighting, Light, Rgba, SkyDraw, SkyObject, SunLight, SunShadow};
use tessera_world::draw_distance::{FogClip, global_limit};
use tessera_world::environment::{SkyValues, property_colour, sky_key_of};
use tessera_world::lighting::LightRecord;
use tessera_world::{
    Environment, EnvironmentFrame, LandblockKey, SkyLibrary, WorldPos, WorldStreamer,
};

use super::fxparse::{System, Systems, clock_seconds};
use super::objects::ObjectLayer;
use super::resolver::Resolver;
use super::terrain::TerrainLight;
use super::view::Anchor;

/// The game day the clock starts on unless set (`environment.md` §14: the day the clock
/// reads at *G* = 0 in this install's calendar; Tessera Decision, the epoch is OPEN).
pub const DEFAULT_DAY: i64 = 12_240;
/// The time of day the clock starts at unless set (Tessera Decision: midday, the time the
/// checklist's performance spots use, `m4-5-visual.md` §3).
pub const DEFAULT_TIME_OF_DAY: f64 = 0.5;
/// Light entities farther than this from the eye (metres) are not in the frame's table.
pub const LIGHT_RADIUS: f64 = 200.0;
/// Most lights in a frame's table (the renderer's limit, `docs/specs/render.md` §4.2).
pub const MAX_TABLE_LIGHTS: usize = 255;
/// Radius of the sphere the sun's shadow map covers (metres; Tessera Decision of
/// `lighting.md` §9 "simple form").
pub const SHADOW_RADIUS: f32 = 90.0;
/// View depth at which the sun's shadow has faded out (metres).
pub const SHADOW_DISTANCE: f32 = 150.0;
/// Shadow-map edge in texels.
pub const SHADOW_MAP: u32 = 2048;
/// Lowest sun elevation (sine) at which the sun casts shadows.
const SHADOW_MIN_ELEVATION: f32 = 0.05;
/// The clear colour without a sky (linear).
pub const NO_SKY: Rgba = [0.42, 0.58, 0.80, 1.0];
/// One time-of-day key step: one 24th of a game day.
pub const TIME_STEP: f64 = 1.0 / 24.0;
/// Fastest clock rate (`--time-rate` and the `=` key share it).
pub const MAX_TIME_RATE: f64 = 100_000.0;
/// Slowest rate the `-` key halves down to.
pub const MIN_KEY_RATE: f64 = 1.0 / 64.0;
/// Largest |`--day`|: day numbers whose *G* the clock holds under any calendar of up to a
/// day of 50,000 s (`GameClock` clamps *G* to ±10¹² s).
pub const MAX_START_DAY: i64 = 20_000_000;

/// The rate after the `-` (`factor` < 1) or `=` (`factor` > 1) key: the rate is never
/// pushed past [`MIN_KEY_RATE`] or [`MAX_TIME_RATE`] by a key, but a rate already beyond
/// them is not moved the wrong way; a stopped clock (rate 0) is restarted by `=` at
/// [`MIN_KEY_RATE`] and stays stopped with `-`.
#[must_use]
pub fn scaled_rate(rate: f64, factor: f64) -> f64 {
    if !(rate.is_finite() && factor.is_finite() && factor > 0.0) {
        return if rate.is_finite() { rate.max(0.0) } else { 1.0 };
    }
    if rate <= 0.0 {
        return if factor > 1.0 { MIN_KEY_RATE } else { 0.0 };
    }
    let r = rate * factor;
    if factor >= 1.0 {
        r.min(MAX_TIME_RATE.max(rate))
    } else {
        r.max(MIN_KEY_RATE.min(rate))
    }
}

/// The rate a paused clock resumes at: the rate before the pause, or 1 when that was 0
/// (`--time-rate 0`), so "clock running" always means a running clock.
#[must_use]
pub fn resume_rate(stored: f64) -> f64 {
    if stored.is_finite() && stored > 0.0 {
        stored
    } else {
        1.0
    }
}

/// The clock settings of a walker (`--day`, `--time-of-day`, `--time-rate`, `--day-type`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockConfig {
    /// Game day number *n* at the start.
    pub day: i64,
    /// Time of day *t* at the start, 0–1.
    pub time_of_day: f64,
    /// Game seconds per real second.
    pub rate: f64,
    /// Day type *D* (`environment.md` §11.2).
    pub day_type: u32,
}

impl Default for ClockConfig {
    fn default() -> Self {
        Self {
            day: DEFAULT_DAY,
            time_of_day: DEFAULT_TIME_OF_DAY,
            rate: 1.0,
            day_type: tessera_world::environment::INITIAL_DAY_TYPE,
        }
    }
}

/// A stored float curve (`terrain_type::FloatCurve`, the form of property waveforms) as the
/// material crate's evaluator takes it (`material.md` §5.3).
#[must_use]
pub fn material_curve(c: &StoredCurve) -> FloatCurve {
    match c {
        StoredCurve::Constant(v) => FloatCurve::constant(*v),
        StoredCurve::Waveform { mode, w } => {
            let mut params = [0.0; 11];
            params[..w.len().min(10)].copy_from_slice(&w[..w.len().min(10)]);
            FloatCurve {
                mode: mode.get(),
                params,
                param_count: 10,
                loop_flag: 0,
                keys: Vec::new(),
            }
        }
        StoredCurve::Keyframed {
            w,
            period,
            looping,
            keys,
        } => {
            let mut params = [0.0; 11];
            params[..w.len().min(10)].copy_from_slice(&w[..w.len().min(10)]);
            params[10] = *period;
            FloatCurve {
                mode: 10,
                params,
                param_count: 11,
                loop_flag: u8::from(*looping),
                keys: keys.iter().map(|k| (k.value, k.position)).collect(),
            }
        }
    }
}

/// What the environment gives a frame.
#[derive(Debug, Clone)]
pub struct FrameEnv {
    /// The lit pass's lighting (lights filled by [`WorldEnv::lights`]).
    pub lighting: FrameLighting,
    /// The clear colour (linear).
    pub clear: Rgba,
    /// The global limit *G* (`walker.md` §11.3).
    pub g: f32,
    /// Fog far distance in force (after the limits of `lighting.md` §10), `None` without fog.
    pub fog_far: Option<f32>,
    /// Time of day.
    pub time_of_day: f64,
    /// Game day number.
    pub day: i64,
    /// Whether it is night (`sound.md` §10.3.5 rule).
    pub night: bool,
    /// The sky key in force.
    pub sky_key: Option<u32>,
    /// The day description in use.
    pub day_description: Option<Did>,
    /// Visible sky objects.
    pub sky_objects: usize,
}

/// sRGB-encoded unit value to linear light (the clear colour is given linear).
fn to_linear(c: f32) -> f32 {
    super::geometry::srgb_unit_to_linear(c)
}

/// The sky key of the indoor record when no `Environment_IndoorAmbiance` is found
/// (`landblock.md` §4.13).
pub const DEFAULT_INDOOR_SKY_KEY: u32 = 1;

/// The property IDs the indoor record reads (`landblock.md` §4.13), looked up by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IndoorPids {
    /// `Environment_IndoorAmbiance`.
    pub indoor_ambiance: Option<u32>,
    /// `InteriorUpperAmbientLight`.
    pub upper: Option<u32>,
    /// `InteriorLowerAmbientLight`.
    pub lower: Option<u32>,
    /// `InteriorAmbientLight`.
    pub ambient: Option<u32>,
    /// `InteriorFogType`.
    pub fog_type: Option<u32>,
    /// `InteriorFogColor`.
    pub fog_colour: Option<u32>,
    /// `InteriorFogNear`.
    pub fog_near: Option<u32>,
    /// `InteriorFogFar`.
    pub fog_far: Option<u32>,
}

impl IndoorPids {
    /// Looks every property up with `pid_of` (name → PID).
    #[must_use]
    pub fn from_names(pid_of: impl Fn(&str) -> Option<u32>) -> Self {
        Self {
            indoor_ambiance: pid_of("Environment_IndoorAmbiance"),
            upper: pid_of("InteriorUpperAmbientLight"),
            lower: pid_of("InteriorLowerAmbientLight"),
            ambient: pid_of("InteriorAmbientLight"),
            fog_type: pid_of("InteriorFogType"),
            fog_colour: pid_of("InteriorFogColor"),
            fog_near: pid_of("InteriorFogNear"),
            fog_far: pid_of("InteriorFogFar"),
        }
    }
}

/// The inputs of the indoor environment record (`landblock.md` §4.13), read at the camera
/// position with the lookup order cell, landblock info, block map.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IndoorInputs {
    /// Sky key B: `Environment_IndoorAmbiance`, else [`DEFAULT_INDOOR_SKY_KEY`].
    pub sky_key: u32,
    /// `InteriorUpperAmbientLight` (RGB, 0–1).
    pub upper: Option<[f32; 3]>,
    /// `InteriorLowerAmbientLight`.
    pub lower: Option<[f32; 3]>,
    /// `InteriorAmbientLight`.
    pub ambient: Option<[f32; 3]>,
    /// `InteriorFogType` (enum key).
    pub fog_type: Option<u32>,
    /// `InteriorFogColor`.
    pub fog_colour: Option<[f32; 3]>,
    /// `InteriorFogNear`.
    pub fog_near: Option<f32>,
    /// `InteriorFogFar`.
    pub fog_far: Option<f32>,
}

impl IndoorInputs {
    /// Reads the inputs with `lookup(pid)`, which returns the first property set of the
    /// lookup order that has `pid` (`landblock.md` §4.13). Colours are read as B, G, R, A
    /// (`lighting.md` §4.4 Tessera Decision); values of the wrong type or not finite count
    /// as absent.
    #[must_use]
    pub fn read<'a>(pids: &IndoorPids, lookup: impl Fn(u32) -> Option<&'a PropertyValue>) -> Self {
        let get = |pid: Option<u32>| pid.and_then(&lookup);
        let colour = |pid| match get(pid) {
            Some(PropertyValue::Colour(c)) => {
                let [r, g, b, _] = property_colour(*c);
                Some([r, g, b])
            }
            _ => None,
        };
        let float = |pid| match get(pid) {
            Some(PropertyValue::Float(f)) if f.is_finite() => Some(*f),
            _ => None,
        };
        Self {
            sky_key: get(pids.indoor_ambiance)
                .and_then(sky_key_of)
                .unwrap_or(DEFAULT_INDOOR_SKY_KEY),
            upper: colour(pids.upper),
            lower: colour(pids.lower),
            ambient: colour(pids.ambient),
            fog_type: get(pids.fog_type).and_then(sky_key_of),
            fog_colour: colour(pids.fog_colour),
            fog_near: float(pids.fog_near),
            fog_far: float(pids.fog_far),
        }
    }

    /// The up- and down-facing ambient *X* and *Y* of the indoor record when any of the
    /// three ambient properties is present: upper (lower) plus the plain ambient, black for
    /// an absent one, every channel limited to at most 1 (`landblock.md` §4.13).
    #[must_use]
    pub fn ambient_xy(&self) -> Option<([f32; 3], [f32; 3])> {
        if self.upper.is_none() && self.lower.is_none() && self.ambient.is_none() {
            return None;
        }
        let a = self.ambient.unwrap_or([0.0; 3]);
        let add = |c: Option<[f32; 3]>| {
            let c = c.unwrap_or([0.0; 3]);
            std::array::from_fn(|i| (c[i] + a[i]).min(1.0))
        };
        Some((add(self.upper), add(self.lower)))
    }

    /// Whether the indoor record has fog: with `InteriorFogType` present exactly when it
    /// is 1 (Linear) and `switch` is on; otherwise `switch` (the keyframe's fog).
    #[must_use]
    pub fn fog_on(&self, switch: bool) -> bool {
        match self.fog_type {
            Some(t) => switch && t == 1,
            None => switch,
        }
    }
}

/// The walker's environment (module docs).
pub struct WorldEnv {
    env: Option<Environment>,
    /// The indoor record's environment (`landblock.md` §4.13): blended on its own from sky
    /// key B, so the change between the two records is immediate.
    indoor: Option<Environment>,
    frame: Option<EnvironmentFrame>,
    /// The rate before a pause, while paused.
    paused: Option<f64>,
    curves: HashMap<(LandblockKey, u32), (FloatCurve, CurveState)>,
    /// Sky-object parts referenced in the object layer.
    sky_parts: HashSet<u32>,
    sky_draw: Vec<SkyObject>,
}

impl std::fmt::Debug for WorldEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorldEnv")
            .field("loaded", &self.env.is_some())
            .finish_non_exhaustive()
    }
}

impl WorldEnv {
    /// The environment of `library` (none without one: the defaults of `lighting.md` §8.1)
    /// with the clock set from `clock`.
    #[must_use]
    pub fn new(library: Option<Arc<SkyLibrary>>, clock: &ClockConfig) -> Self {
        let indoor = library
            .clone()
            .map(|lib| started_environment(lib, clock, false));
        let env = library.map(|lib| started_environment(lib, clock, true));
        Self {
            env,
            indoor,
            frame: None,
            paused: None,
            curves: HashMap::new(),
            sky_parts: HashSet::new(),
            sky_draw: Vec::new(),
        }
    }

    /// The two environments (outdoor and indoor record), for clock changes.
    fn each(&mut self) -> impl Iterator<Item = &mut Environment> {
        self.env.iter_mut().chain(self.indoor.iter_mut())
    }
}

/// An environment of `lib` with the clock set from `clock` (`warn`: log a day the clock
/// cannot hold).
fn started_environment(lib: Arc<SkyLibrary>, clock: &ClockConfig, warn: bool) -> Environment {
    let mut e = Environment::new(lib);
    e.set_day_type(clock.day_type);
    let cal = e.clock().calendar().clone();
    let at = |day: i64| {
        clock_seconds(
            cal.days_per_year,
            cal.day_length,
            cal.zero_offset,
            cal.zero_year,
            day,
            clock.time_of_day,
        )
        .filter(|g| g.abs() <= tessera_world::environment::MAX_GAME_SECONDS)
    };
    let g = at(clock.day).or_else(|| {
        if warn {
            tracing::warn!(
                day = clock.day,
                "the clock cannot hold this day; starting on day {DEFAULT_DAY}"
            );
        }
        at(DEFAULT_DAY)
    });
    if let Some(g) = g {
        e.set_game_seconds(g);
    }
    e.set_rate(clock.rate);
    e
}

impl WorldEnv {
    /// Whether a sky library was loaded.
    #[must_use]
    pub fn has_sky(&self) -> bool {
        self.env.is_some()
    }

    /// Moves the time of day by `steps` × [`TIME_STEP`] (keys `[` and `]`).
    pub fn step_time(&mut self, steps: f64) {
        let Some(t) = self.env.as_ref().map(|e| e.clock().read().time_of_day) else {
            return;
        };
        for e in self.each() {
            e.set_time_of_day(t + steps * TIME_STEP);
        }
    }

    /// Stops or restarts the clock (key `\`).
    pub fn toggle_pause(&mut self) -> bool {
        let Some(current) = self.env.as_ref().map(|e| e.clock().rate()) else {
            return false;
        };
        let (rate, paused) = match self.paused.take() {
            Some(rate) => (resume_rate(rate), false),
            None => {
                self.paused = Some(current);
                (0.0, true)
            }
        };
        for e in self.each() {
            e.set_rate(rate);
        }
        paused
    }

    /// Multiplies the clock rate by `factor` (keys `-` and `=`, [`scaled_rate`]). While
    /// paused, the rate the clock resumes at is scaled and the clock stays stopped.
    pub fn scale_rate(&mut self, factor: f64) {
        let Some(current) = self.env.as_ref().map(|e| e.clock().rate()) else {
            return;
        };
        match &mut self.paused {
            Some(stored) => *stored = scaled_rate(resume_rate(*stored), factor),
            None => {
                let rate = scaled_rate(current, factor);
                for e in self.each() {
                    e.set_rate(rate);
                }
            }
        }
    }

    /// The clock rate (0 while paused).
    #[must_use]
    pub fn rate(&self) -> f64 {
        self.env.as_ref().map_or(0.0, |e| e.clock().rate())
    }

    /// Advances the clock by `dt` real seconds and returns the frame's environment
    /// (`landblock.md` §4.13): the outdoor record from `sky_key` (key A) under the camera,
    /// or, with `indoor` (the camera inside, outdoor-visible clear), the indoor record from
    /// its key B with the interior overrides and no sun or moon. Both records blend every
    /// frame, each on its own, so the change between them is immediate. `extent` is the
    /// landscape extent *E*; `eye` and `forward` place the shadow map.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        dt: f64,
        sky_key: Option<u32>,
        indoor: Option<&IndoorInputs>,
        systems: &Systems,
        extent: f32,
        eye: Vec3,
        forward: Vec3,
        time: f32,
    ) -> FrameEnv {
        let sky_on = systems.on(System::Sky);
        let key_b = indoor.map_or(DEFAULT_INDOOR_SKY_KEY, |i| i.sky_key);
        let step = |env: &mut Option<Environment>, key: Option<u32>| match (env, sky_on) {
            (Some(e), true) => Some(e.update(dt, key)),
            (Some(e), false) => {
                // The clock runs on; the values are the defaults.
                e.clock_mut().advance(dt);
                None
            }
            _ => None,
        };
        let outdoor = step(&mut self.env, sky_key);
        let inside = step(&mut self.indoor, Some(key_b));
        let frame = if indoor.is_some() { inside } else { outdoor };
        let values = frame
            .as_ref()
            .map_or_else(SkyValues::defaults, |f| f.values.clone());
        let mut li = values.lighting();
        let mut fog_on = systems.on(System::Fog) && frame.is_some();
        if let Some(ind) = indoor {
            if let Some((x, y)) = ind.ambient_xy() {
                li.ambient_up = x;
                li.ambient_down = y;
                li.frame_ambient = std::array::from_fn(|i| 0.5 * (x[i] + y[i]));
                li.frame_hemispheric = std::array::from_fn(|i| 0.5 * (x[i] - y[i]));
            }
            fog_on = ind.fog_on(fog_on);
            if let Some(c) = ind.fog_colour {
                li.fog_colour = c;
            }
            if let Some(n) = ind.fog_near {
                li.fog_near = n;
            }
            if let Some(f) = ind.fog_far {
                li.fog_far = f;
            }
        }
        let toward = Vec3::from_array(li.sun_travel) * -1.0;
        let toward = toward.try_normalize().unwrap_or(Vec3::Z);
        let fog = fog_on.then(|| {
            Fog {
                near: li.fog_near,
                far: li.fog_far,
                color: li.fog_colour,
            }
            .limited()
        });
        // Sun and moon give no light indoors (§4.13), so no shadow either.
        let sun_on = indoor.is_none();
        let shadow = (sun_on && systems.on(System::Shadows) && toward.z > SHADOW_MIN_ELEVATION)
            .then(|| {
                let f = Vec3::new(forward.x, forward.y, 0.0)
                    .try_normalize()
                    .unwrap_or(Vec3::ZERO);
                SunShadow::fit(
                    toward,
                    eye + f * (SHADOW_RADIUS * 0.6),
                    SHADOW_RADIUS,
                    SHADOW_DISTANCE,
                    SHADOW_MAP,
                )
            });
        let lighting = FrameLighting {
            up: Vec3::Z,
            ambient: li.frame_ambient,
            hemispheric: li.frame_hemispheric,
            sun: sun_on.then_some(SunLight {
                direction: toward,
                color: li.sun_colour,
                specular: li.sun_specular,
            }),
            fog,
            shadow,
            lights: Vec::new(),
            time,
        };
        let g = global_limit(
            Some(extent),
            fog.map(|f| FogClip {
                enabled: true,
                far: f.far,
            }),
        );
        let clear = if frame.is_some() {
            let c = li.fog_colour;
            [to_linear(c[0]), to_linear(c[1]), to_linear(c[2]), 1.0]
        } else {
            NO_SKY
        };
        let reading = self.env.as_ref().map(|e| e.clock().read());
        let out = FrameEnv {
            lighting,
            clear,
            g,
            fog_far: fog.map(|f| f.far),
            time_of_day: reading.map_or(DEFAULT_TIME_OF_DAY, |r| r.time_of_day),
            day: reading.map_or(DEFAULT_DAY, |r| r.day_number),
            night: frame.as_ref().is_some_and(|f| f.is_night),
            sky_key: frame.as_ref().and_then(|f| f.sky_key),
            day_description: frame.as_ref().and_then(|f| f.choice.map(|c| c.day)),
            sky_objects: frame
                .as_ref()
                .map_or(0, |f| f.objects.iter().filter(|o| o.visible).count()),
        };
        self.frame = frame;
        out
    }

    /// The point lights of the Light entities within [`LIGHT_RADIUS`] of `eye` (at most
    /// [`MAX_TABLE_LIGHTS`], nearest first): the frame's light table in the render frame of
    /// `anchor` (`lighting.md` §8.3: day/night variants by the Tessera rule, intensity
    /// curves on the global clock `time`, *σ* = 1), and the same lights as the terrain sees
    /// them. Outdoor lights only with `outdoor`, interior ones only with `interior`
    /// (`walker.md` §14.11: the camera's side of the portal system).
    pub fn lights(
        &mut self,
        streamer: &WorldStreamer,
        anchor: &Anchor,
        eye: &WorldPos,
        night: bool,
        time: f64,
        (outdoor, interior): (bool, bool),
    ) -> (Vec<Light>, Vec<TerrainLight>) {
        let mut lights = Vec::new();
        let mut terrain = Vec::new();
        let mut seen: HashSet<(LandblockKey, u32)> = HashSet::new();
        for near in streamer.lights_near(eye, LIGHT_RADIUS) {
            if lights.len() >= MAX_TABLE_LIGHTS {
                break;
            }
            let l = near.light;
            if (l.interior && !interior) || (!l.interior && !outdoor) {
                continue;
            }
            let (range, on) = l.params.at_time(night);
            if !on {
                continue;
            }
            let id = (near.key, l.index);
            seen.insert(id);
            let intensity = match l.params.constant_intensity() {
                Some(v) => v,
                None => {
                    let (curve, state) = self.curves.entry(id).or_insert_with(|| {
                        (material_curve(&l.params.intensity), CurveState::new())
                    });
                    curve.evaluate_finite(time, state)
                }
            };
            let Some(p) = anchor.to_render(&near.position) else {
                continue;
            };
            let rec = LightRecord::point(&l.params, p.to_array(), range, intensity, 1.0);
            lights.push(Light {
                kind: tessera_render::LightKind::Point {
                    position: p,
                    range: rec.range,
                    attenuation: rec.k,
                },
                color: rec.colour,
                specular: rec.specular,
            });
            terrain.push(TerrainLight {
                position: p,
                range: rec.range,
                baked: l.params.baked_landscape,
            });
        }
        // Curve states of lights that left the table are dropped.
        if self.curves.len() > 4 * MAX_TABLE_LIGHTS {
            self.curves.retain(|k, _| seen.contains(k));
        }
        (lights, terrain)
    }

    /// The visible sky objects of the description in use, with their models from the
    /// object layer (`environment.md` §11.4, §12): their parts are referenced there once
    /// resolved; objects whose models are not built yet are left out.
    pub fn sky(&mut self, resolver: &mut Resolver, objects: &mut ObjectLayer) -> Option<SkyDraw> {
        let frame = self.frame.as_ref()?;
        self.sky_draw.clear();
        let mut new_parts: Vec<u32> = Vec::new();
        for o in frame
            .objects
            .iter()
            .filter(|o| o.visible && o.tint[3] > 0.0)
        {
            if o.object.is_null() {
                continue;
            }
            let Some(parts) = resolver.object_parts(o.object) else {
                continue;
            };
            for p in parts.iter() {
                if self.sky_parts.insert(p.0) {
                    new_parts.push(p.0);
                }
                if let Some(model) = objects.model(p.0, 0) {
                    let mut s = SkyObject::new(model.clone(), o.angle_a, o.angle_b);
                    s.tint = o.tint;
                    self.sky_draw.push(s);
                }
            }
        }
        if !new_parts.is_empty() {
            objects.acquire_model_parts(&new_parts);
        }
        Some(SkyDraw {
            objects: self.sky_draw.clone(),
        })
    }

    /// Whether every visible sky object's model is built (or will never be).
    #[must_use]
    pub fn sky_settled(&self, objects: &ObjectLayer) -> bool {
        self.sky_parts.iter().all(|p| objects.part_settled(*p))
    }

    /// Releases the sky objects' parts (shutdown).
    pub fn release(&mut self, objects: &mut ObjectLayer) {
        let parts: Vec<u32> = self.sky_parts.drain().collect();
        objects.release_model_parts(&parts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_formats::terrain_type::{CURVE_PARAMS, CurveKey};

    #[test]
    fn stored_curves_convert_to_the_evaluator() {
        let c = material_curve(&StoredCurve::Constant(0.75));
        assert_eq!((c.mode, c.params[0], c.param_count), (1, 0.75, 1));
        let w: [f32; CURVE_PARAMS] = std::array::from_fn(|i| i as f32);
        let wave = StoredCurve::waveform(4, w).expect("mode 4");
        let c = material_curve(&wave);
        assert_eq!((c.mode, c.param_count), (4, 10));
        assert_eq!(c.params[..10], w[..10]);
        let k = StoredCurve::Keyframed {
            w,
            period: 2.0,
            looping: true,
            keys: vec![
                CurveKey {
                    value: 1.0,
                    position: 0.0,
                },
                CurveKey {
                    value: 3.0,
                    position: 100.0,
                },
            ],
        };
        let c = material_curve(&k);
        assert_eq!((c.mode, c.param_count, c.loop_flag), (10, 11, 1));
        assert_eq!(c.params[10], 2.0);
        assert_eq!(c.keys, vec![(1.0, 0.0), (3.0, 100.0)]);
        // Every converted curve evaluates to a finite value.
        let mut st = CurveState::new();
        for t in [0.0, 0.5, 1.0, 1.0e6] {
            assert!(c.evaluate_finite(t, &mut st).is_finite());
        }
    }

    #[test]
    fn rate_keys_keep_their_bounds_and_the_pause() {
        assert_eq!(scaled_rate(1.0, 2.0), 2.0);
        assert_eq!(scaled_rate(1.0, 0.5), 0.5);
        assert_eq!(scaled_rate(MIN_KEY_RATE, 0.5), MIN_KEY_RATE);
        assert_eq!(scaled_rate(80_000.0, 2.0), MAX_TIME_RATE);
        // A rate beyond the key range is not moved the wrong way (review finding: `=`
        // lowered `--time-rate 100000` to 65,536).
        assert_eq!(scaled_rate(MAX_TIME_RATE, 2.0), MAX_TIME_RATE);
        assert_eq!(scaled_rate(0.001, 0.5), 0.001);
        assert_eq!(scaled_rate(0.001, 2.0), 0.002);
        // A stopped clock: `=` starts it slowly, `-` leaves it stopped.
        assert_eq!(scaled_rate(0.0, 2.0), MIN_KEY_RATE);
        assert_eq!(scaled_rate(0.0, 0.5), 0.0);
        assert_eq!(scaled_rate(f64::NAN, 2.0), 1.0);
        assert_eq!(scaled_rate(3.0, f64::NAN), 3.0);
        // Resuming a clock paused at rate 0 runs it.
        assert_eq!(resume_rate(0.0), 1.0);
        assert_eq!(resume_rate(4.0), 4.0);
        assert_eq!(resume_rate(f64::NAN), 1.0);
    }

    #[test]
    fn without_a_sky_the_defaults_light_the_frame() {
        let mut env = WorldEnv::new(None, &ClockConfig::default());
        assert!(!env.has_sky());
        let f = env.update(
            0.016,
            None,
            None,
            &Systems::all(),
            2_080.0,
            Vec3::ZERO,
            Vec3::X,
            0.0,
        );
        assert_eq!(f.clear, NO_SKY);
        assert_eq!(f.fog_far, None);
        assert_eq!(f.g, 2_080.0);
        assert!(f.lighting.sun.is_some());
        // The default light travels along (1, 0, −0.4): the sun is above the horizon.
        assert!(f.lighting.shadow.is_some());
        let mut off = Systems::all();
        off.set(System::Shadows, false);
        let f = env.update(0.016, None, None, &off, 2_080.0, Vec3::ZERO, Vec3::X, 0.0);
        assert!(f.lighting.shadow.is_none());
        assert!(!env.toggle_pause(), "no clock to pause");
    }

    /// `landblock.md` §4.13: the indoor record (row Y5 worked through).
    #[test]
    fn the_indoor_record_takes_the_interior_overrides_and_has_no_sun() {
        use tessera_formats::PropertyValue;
        let pids = IndoorPids {
            indoor_ambiance: Some(1),
            upper: Some(2),
            lower: Some(3),
            ambient: Some(4),
            fog_type: Some(5),
            fog_colour: Some(6),
            fog_near: Some(7),
            fog_far: Some(8),
        };
        // The Y5 landblock-info set: ambient stored (8, 25, 8, 255), linear fog 0.059–90
        // in stored (2, 14, 11, 255).
        let set: HashMap<u32, PropertyValue> = HashMap::from([
            (4, PropertyValue::Colour([8, 25, 8, 255])),
            (5, PropertyValue::Enum(1)),
            (6, PropertyValue::Colour([2, 14, 11, 255])),
            (7, PropertyValue::Float(0.059)),
            (8, PropertyValue::Float(90.0)),
        ]);
        let ind = IndoorInputs::read(&pids, |pid| set.get(&pid));
        assert_eq!(ind.sky_key, DEFAULT_INDOOR_SKY_KEY);
        let c = |r: u8, g: u8, b: u8| [r, g, b].map(|v| f32::from(v) / 255.0);
        assert_eq!(ind.fog_colour, Some(c(11, 14, 2)));
        let (x, y) = ind.ambient_xy().expect("ambient present");
        assert_eq!((x, y), (c(8, 25, 8), c(8, 25, 8)));
        assert!(ind.fog_on(true) && !ind.fog_on(false));
        for (t, on) in [(0, false), (2, false), (7, false)] {
            let other = IndoorInputs {
                fog_type: Some(t),
                ..ind
            };
            assert_eq!(other.fog_on(true), on, "type {t}");
        }
        let none = IndoorInputs::default();
        assert!(none.ambient_xy().is_none());
        assert!(none.fog_on(true), "absent: the keyframe's fog");
        let bright = IndoorInputs {
            upper: Some([0.95, 0.0, 0.0]),
            ambient: Some([0.1, 0.2, 0.3]),
            ..none
        };
        let (x, y) = bright.ambient_xy().unwrap();
        assert_eq!(x[0], 1.0, "limited to 1");
        assert_eq!(y, [0.1, 0.2, 0.3], "lower absent: black + ambient");

        let mut env = WorldEnv::new(None, &ClockConfig::default());
        let f = env.update(
            0.016,
            None,
            Some(&bright),
            &Systems::all(),
            2_080.0,
            Vec3::ZERO,
            Vec3::X,
            0.0,
        );
        assert!(f.lighting.sun.is_none() && f.lighting.shadow.is_none());
        assert!((f.lighting.ambient[0] - 0.55).abs() < 1e-6);
        assert!((f.lighting.hemispheric[0] - 0.45).abs() < 1e-6);
        assert_eq!(f.lighting.ambient[1], 0.2);
        // Outdoors again: the sun is back at once.
        let f = env.update(
            0.016,
            None,
            None,
            &Systems::all(),
            2_080.0,
            Vec3::ZERO,
            Vec3::X,
            0.0,
        );
        assert!(f.lighting.sun.is_some());
    }
}
