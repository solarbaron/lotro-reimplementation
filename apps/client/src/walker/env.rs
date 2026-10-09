// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's environment (`walker.md` §14.3): the game clock with its start time and
//! rate (`environment.md` §11.1, epoch OPEN, so settable), the outdoor and indoor tracks
//! (`environment.md` §11.6, `landblock.md` §4.13; `tessera-world`'s [`Environment`], each
//! with its sky-key cross-fade), the interior overrides of the indoor track (§7.2), the
//! composed state of the track in use ([`compose_state`], §11.7) smoothed by the 5 s
//! sun-change fade ([`SunChangeFade`], §11.6 E34, E37), and from it the frame's lighting:
//! sun or moon with its shadow, ambient and hemispheric terms, distance fog (`lighting.md`
//! §8.1, §9, §10), the point lights of the Light entities near the eye (§8.3) and the
//! global distance limit *G* (`walker.md` §11.3). The sky objects of the track in use are
//! drawn around the camera (`environment.md` §11.4, §12).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tessera_formats::Did;
use tessera_formats::PropertyValue;
use tessera_formats::terrain_type::FloatCurve as StoredCurve;
use tessera_material::{CurveState, FloatCurve};
use tessera_render::glam::Vec3;
use tessera_render::lighting::reference::selection_radius;
use tessera_render::{
    Fog, FrameLighting, Light, LightReach, Rgba, SkyDraw, SkyObject, SunLight, SunShadow,
};
use tessera_world::draw_distance::{FogClip, global_limit};
use tessera_world::environment::{
    EnvState, SkyValues, SunChangeFade, outdoor_sun_present, property_colour,
};
use tessera_world::lighting::LightRecord;
use tessera_world::{
    Environment, EnvironmentFrame, LandblockBundle, LandblockKey, SkyLibrary, WorldPos,
    WorldStreamer,
};

use super::fxparse::{System, Systems, clock_seconds};
use super::objects::ObjectLayer;
use super::resolver::Resolver;
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
/// The sun-present tolerance (`landblock.md` §4.13, `environment.md` §11.7 step 9): a sun
/// colour within this of black in every channel (inclusive), or a brightness below it, gives
/// no main light.
pub const SUN_EPSILON: f32 = tessera_world::environment::SUN_EPSILON;
/// Fog end limits (`environment.md` §11.7 step 5, `lighting.md` §10): the keyframe's end is
/// limited to at most [`FOG_FAR_MAX`] **before** the interior override; after it the end
/// is raised to at least [`FOG_FAR_MIN`] and the start limited to
/// [`FOG_NEAR_MIN`] … end − [`FOG_NEAR_GAP`].
pub const FOG_FAR_MAX: f32 = 10_000.0;
/// Smallest fog end (after the interior override).
pub const FOG_FAR_MIN: f32 = 20.0;
/// Smallest fog start.
pub const FOG_NEAR_MIN: f32 = -10_000.0;
/// Least distance between fog start and end.
pub const FOG_NEAR_GAP: f32 = 20.0;
/// `fog_extra` above which the sky objects are not drawn while fog is on
/// (`environment.md` §11.7 step 10).
pub const SKY_FOG_EXTRA: f32 = 0.99;

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

/// Whether blended keyframe values give the outdoor record a main light by the test of
/// `environment.md` §11.7 step 9 (`landblock.md` §4.13): the blended `effect` is the null
/// DID, the sun colour is not black within [`SUN_EPSILON`] (inclusive; the blended colour
/// has alpha 1, `environment.md` §11.3 step 4) and the brightness *bd* is at least
/// [`SUN_EPSILON`] (a brightness that is not a number counts as below). The tracks carry the
/// flag in [`SkyValues::sun_present`], which a sky-key cross-fade keeps on while either side
/// has a sun (§11.6 step 4).
#[must_use]
pub fn sun_present(values: &SkyValues) -> bool {
    outdoor_sun_present(values.effect, values.sun_colour, values.sun_brightness)
}

/// Fog start and end after the limits of `environment.md` §11.7 step 5: the keyframe's end
/// is limited to at most [`FOG_FAR_MAX`], then the interior overrides (`near`, `far`)
/// replace the keyframe values, then the end is raised to at least [`FOG_FAR_MIN`] and the
/// start set to max([`FOG_NEAR_MIN`], min(start, max(end − [`FOG_NEAR_GAP`],
/// [`FOG_NEAR_MIN`]))). So an overriding end above [`FOG_FAR_MAX`] is kept. Non-finite
/// values become the limits.
#[must_use]
pub fn fog_distances(
    keyframe_near: f32,
    keyframe_far: f32,
    near: Option<f32>,
    far: Option<f32>,
) -> (f32, f32) {
    let keyframe_far = if keyframe_far.is_finite() {
        keyframe_far.min(FOG_FAR_MAX)
    } else {
        FOG_FAR_MAX
    };
    let far = far.filter(|f| f.is_finite()).unwrap_or(keyframe_far);
    let far = far.max(FOG_FAR_MIN);
    let near = near.unwrap_or(keyframe_near);
    let near = if near.is_finite() {
        near.min((far - FOG_NEAR_GAP).max(FOG_NEAR_MIN))
            .max(FOG_NEAR_MIN)
    } else {
        FOG_NEAR_MIN
    };
    (near, far)
}

/// A sky key or `InteriorFogType` from a stored value: present only with property type 8
/// (enum value) or 10 (property ID); any other type counts as absent (`environment.md`
/// §7.1.1, `walker.md` §14.3 row W-E8).
#[must_use]
pub fn stored_key(v: &PropertyValue) -> Option<u32> {
    match v {
        PropertyValue::Enum(k) | PropertyValue::PropertyId(k) => Some(*k),
        _ => None,
    }
}

/// The sky key of the outdoor record when no `Environment_LocalSky` is found
/// (`environment.md` §7.1.1 step 5).
pub const DEFAULT_OUTDOOR_SKY_KEY: u32 = 2;

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
    /// Fog is on and the blended `fog_extra` exceeds [`SKY_FOG_EXTRA`]: the sky objects are
    /// not drawn (`environment.md` §11.7 step 10).
    pub sky_fogged: bool,
}

/// The lighting of one frame composed from the record in use ([`compose`]).
#[derive(Debug, Clone)]
pub struct Composed {
    /// The lit pass's lighting (no lights yet).
    pub lighting: FrameLighting,
    /// The fog colour in force (the clear colour's source).
    pub fog_colour: [f32; 3],
    /// See [`FrameEnv::sky_fogged`].
    pub sky_fogged: bool,
}

/// `fog_extra` at or below this counts as 0 (`environment.md` §11.7 step 5).
pub const FOG_EXTRA_MIN: f32 = 0.001;

/// The composed state of the record in use (`walker.md` §14.3, `landblock.md` §4.13,
/// `environment.md` §11.7): `values` are the record's blended values (`None` without a sky
/// library or with `sky` off: the defaults of `lighting.md` §8.1, no fog and no interior
/// overrides); `indoor` selects the indoor record, which takes the interior overrides and
/// never has a main light. The outdoor record has one while [`SkyValues::sun_present`].
/// Fog follows the fog switch (and `InteriorFogType` indoors) with the limits of
/// [`fog_distances`]; `fog_extra` is kept only with fog on and above [`FOG_EXTRA_MIN`].
#[must_use]
pub fn compose_state(
    values: Option<&SkyValues>,
    indoor: Option<&IndoorInputs>,
    systems: &Systems,
) -> EnvState {
    let defaults = SkyValues::defaults();
    let v = values.unwrap_or(&defaults);
    let mut li = v.lighting();
    let mut fog_on = systems.on(System::Fog) && values.is_some();
    // Without a sky (library missing or switched off) the defaults stand: the interior
    // overrides belong to the indoor record's keyframe values (`walker.md` §14.3).
    let overrides = indoor.filter(|_| values.is_some());
    if let Some(ind) = overrides {
        if let Some((x, y)) = ind.ambient_xy() {
            li.ambient_up = x;
            li.ambient_down = y;
        }
        fog_on = ind.fog_on(fog_on);
        if let Some(c) = ind.fog_colour {
            li.fog_colour = c;
        }
    }
    let (near, far) = fog_distances(
        li.fog_near,
        li.fog_far,
        overrides.and_then(|i| i.fog_near),
        overrides.and_then(|i| i.fog_far),
    );
    let travel = Vec3::from_array(li.sun_travel)
        .try_normalize()
        .unwrap_or(Vec3::NEG_Z);
    EnvState {
        // Sun and moon give no light indoors (§4.13), nor a black or dim sun or a weather
        // effect outdoors (`environment.md` §11.7 step 9).
        sun_present: indoor.is_none() && v.sun_present,
        sun_travel: travel.to_array(),
        sun_colour: v.sun_colour,
        sun_brightness: v.sun_brightness,
        sun_specular: v.sun_specular,
        ambient_up: li.ambient_up,
        ambient_down: li.ambient_down,
        fog_on,
        fog_near: near,
        fog_far: far,
        fog_colour: li.fog_colour,
        fog_extra: if fog_on && v.fog_extra > FOG_EXTRA_MIN {
            v.fog_extra
        } else {
            0.0
        },
        effect: v.effect,
    }
}

/// The frame lighting of a composed state (`lighting.md` §8.1): ambient ½ (*X* + *Y*),
/// hemispheric ½ (*X* − *Y*), a main light (colour *Cd* × *bd*, specular *Cs*, toward
/// −travel) and its shadow only while "sun present", fog while fog is on. `eye` and
/// `forward` place the shadow map; `time` is the material clock.
#[must_use]
pub fn frame_lighting(
    state: &EnvState,
    systems: &Systems,
    eye: Vec3,
    forward: Vec3,
    time: f32,
) -> Composed {
    let x = state.ambient_up;
    let y = state.ambient_down;
    let toward = (Vec3::from_array(state.sun_travel) * -1.0)
        .try_normalize()
        .unwrap_or(Vec3::Z);
    let fog = state.fog_on.then_some(Fog {
        near: state.fog_near,
        far: state.fog_far,
        color: state.fog_colour,
    });
    let sun_on = state.sun_present;
    let shadow =
        (sun_on && systems.on(System::Shadows) && toward.z > SHADOW_MIN_ELEVATION).then(|| {
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
        ambient: std::array::from_fn(|i| 0.5 * (x[i] + y[i])),
        hemispheric: std::array::from_fn(|i| 0.5 * (x[i] - y[i])),
        sun: sun_on.then_some(SunLight {
            direction: toward,
            color: state.sun_colour.map(|c| c * state.sun_brightness),
            specular: state.sun_specular,
        }),
        fog,
        shadow,
        lights: Vec::new(),
        time,
    };
    Composed {
        lighting,
        fog_colour: state.fog_colour,
        sky_fogged: state.sky_fogged(),
    }
}

/// The frame lighting of the record in use without a sun-change fade: [`frame_lighting`]
/// of [`compose_state`].
#[must_use]
pub fn compose(
    values: Option<&SkyValues>,
    indoor: Option<&IndoorInputs>,
    systems: &Systems,
    eye: Vec3,
    forward: Vec3,
    time: f32,
) -> Composed {
    frame_lighting(
        &compose_state(values, indoor, systems),
        systems,
        eye,
        forward,
        time,
    )
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
/// position with the lookup order landblock info, block map ([`lookup_at`]).
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
    /// as absent: key B and `InteriorFogType` only with type 8 or 10 ([`stored_key`]),
    /// colours only with type 16, near and far only with type 9.
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
                .and_then(stored_key)
                .unwrap_or(DEFAULT_INDOOR_SKY_KEY),
            upper: colour(pids.upper),
            lower: colour(pids.lower),
            ambient: colour(pids.ambient),
            fog_type: get(pids.fog_type).and_then(stored_key),
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

/// The property lookup at a position (`landblock.md` §4.13, BIN4 (l), L46; `environment.md`
/// §7.1.1): the first of the landblock-info property set and the block-map values of
/// `bundle` that has `pid`. `bundle` is the landblock of the position: for a position in a
/// cell, the cell's owner (L46). The cell's own property set is **not** read (L32 corrected
/// 2026-10-08), and the client's run-time landblock set has no offline source (step 1
/// skipped, Inferred there).
#[must_use]
pub fn lookup_at(bundle: Option<&LandblockBundle>, pid: u32) -> Option<&PropertyValue> {
    let info = bundle.and_then(|b| b.info.as_ref());
    info.and_then(|i| i.properties.get(pid))
        .or_else(|| bundle.and_then(|b| b.block_props.value(pid)))
}

/// The indoor record's inputs with the camera in a cell of `bundle` (its owner landblock):
/// [`IndoorInputs::read`] over [`lookup_at`].
#[must_use]
pub fn indoor_inputs_of(bundle: Option<&LandblockBundle>, pids: &IndoorPids) -> IndoorInputs {
    indoor_inputs_at(bundle, pids)
}

/// Sky key A of the outdoor record (`environment.md` §7.1.1; the character's own
/// properties, steps 1–3, never apply offline): `Environment_LocalSky` (`local_sky`) at the
/// viewer's position by [`lookup_at`], counted only with type 8 or 10 ([`stored_key`]),
/// else [`DEFAULT_OUTDOOR_SKY_KEY`].
#[must_use]
pub fn outdoor_sky_key(bundle: Option<&LandblockBundle>, local_sky: Option<u32>) -> u32 {
    local_sky
        .and_then(|pid| lookup_at(bundle, pid))
        .and_then(stored_key)
        .unwrap_or(DEFAULT_OUTDOOR_SKY_KEY)
}

/// Where the property lookup at the camera position reads (`landblock.md` §4.13,
/// `environment.md` §7.1.1): with the camera in a cell, that cell's owner landblock (L46;
/// the cell is returned too, but its own set is not read, [`lookup_at`]); otherwise no
/// cell and the landblock under the eye (none off the grid). Key A, key B and the interior
/// overrides all read here, also when outdoor-visible is clear with no camera cell (§4.12
/// outdoor branch with a bit-0 cell; none in this install, LI8).
#[must_use]
pub fn lookup_target(
    camera_cell: Option<(LandblockKey, u16)>,
    eye: &WorldPos,
) -> (Option<LandblockKey>, Option<u16>) {
    match camera_cell {
        Some((k, c)) => (Some(k), Some(c)),
        None => (eye.landblock(), None),
    }
}

/// Sky key A as the walker draws with it: [`outdoor_sky_key`] at [`lookup_target`] over
/// `streamer`'s resident bundles.
#[must_use]
pub fn camera_sky_key(
    streamer: &WorldStreamer,
    camera_cell: Option<(LandblockKey, u16)>,
    eye: &WorldPos,
) -> u32 {
    let (key, _cell) = lookup_target(camera_cell, eye);
    let local_sky = streamer.context().systems().pids.environment.local_sky;
    outdoor_sky_key(
        key.and_then(|k| streamer.bundle(k)).map(|b| &**b),
        local_sky,
    )
}

/// The indoor record's inputs at [`lookup_target`] (`landblock.md` §4.13):
/// [`IndoorInputs::read`] over [`lookup_at`] with `bundle` the target landblock's.
#[must_use]
pub fn indoor_inputs_at(bundle: Option<&LandblockBundle>, pids: &IndoorPids) -> IndoorInputs {
    IndoorInputs::read(pids, |pid| lookup_at(bundle, pid))
}

/// The reach of a placed light whose range in force is `range` (`lighting.md` §8.5):
/// selection radius ρ from Range and Falloff (Range with `Light_GlobalLight`), key light,
/// baked into the landscape, and the two parent switches; not attached to an object (the
/// walker gives no light a parent), so a light with `Light_OnlyInfluenceParent` reaches no
/// object (row W-L2; the renderer applies the rule, `render.md` §4.3).
#[must_use]
pub fn light_reach(params: &tessera_world::LightParams, range: f32) -> LightReach {
    LightReach {
        radius: Some(selection_radius(range, params.falloff, params.global)),
        key_light: params.key_light,
        baked_landscape: params.baked_landscape,
        parent: None,
        only_influence_parent: params.only_influence_parent,
        never_influence_parent: params.never_influence_parent,
        ..LightReach::default()
    }
}

/// The walker's environment (module docs).
pub struct WorldEnv {
    env: Option<Environment>,
    /// The indoor record's environment (`landblock.md` §4.13, the indoor track of
    /// `environment.md` §11.6): blended on its own from sky key B; never has a sun.
    indoor: Option<Environment>,
    frame: Option<EnvironmentFrame>,
    /// The sun-change fade over the state of the record in use (`environment.md` §11.6,
    /// E34, E37): the only smoothing of the change between the two records.
    fade: SunChangeFade,
    /// The raw timer: real seconds since the walker started (`environment.md` §11.6
    /// *F*raw).
    raw: f64,
    /// The next update is immediate (start-up, a teleport; `landblock.md` §4.13 Tessera
    /// Decision).
    immediate: bool,
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
        let indoor = library.clone().map(|lib| {
            let mut e = started_environment(lib, clock, false);
            e.set_indoor(true);
            e
        });
        let env = library.map(|lib| started_environment(lib, clock, true));
        // A clock started at rate 0 (`--time-rate 0`) is a stopped clock: `\` restarts
        // it at rate 1 and `-` / `=` change the rate it restarts at (`walker.md` §14.3).
        let stopped = env.is_some() && !(clock.rate.is_finite() && clock.rate > 0.0);
        Self {
            env,
            indoor,
            frame: None,
            fade: SunChangeFade::new(),
            raw: 0.0,
            immediate: true,
            paused: stopped.then_some(0.0),
            curves: HashMap::new(),
            sky_parts: HashSet::new(),
            sky_draw: Vec::new(),
        }
    }

    /// The two environments (outdoor and indoor record), for clock changes.
    fn each(&mut self) -> impl Iterator<Item = &mut Environment> {
        self.env.iter_mut().chain(self.indoor.iter_mut())
    }

    /// Makes the next update immediate (`landblock.md` §4.13 Tessera Decision: the first
    /// update after start-up and after a teleport): both records switch to their keys at once
    /// (no sky-key cross-fade, `environment.md` §11.6 step 2) and no sun-change fade runs
    /// across the jump.
    pub fn mark_immediate(&mut self) {
        self.immediate = true;
        for e in self.each() {
            e.mark_immediate();
        }
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

    /// Moves the clock by `steps` × [`TIME_STEP`] of a game day (keys `[` and `]`): forward
    /// or back across midnight into the next or previous day, never wrapped within the day.
    pub fn step_time(&mut self, steps: f64) {
        let Some((g, day_length)) = self
            .env
            .as_ref()
            .map(|e| (e.clock().game_seconds(), e.clock().calendar().day_length))
        else {
            return;
        };
        let to = g + steps * TIME_STEP * day_length;
        if !to.is_finite() {
            return;
        }
        for e in self.each() {
            e.set_game_seconds(to);
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
    /// (`landblock.md` §4.13, `environment.md` §11.6): both records blend every frame, each
    /// on its own: the outdoor record from `sky_key` (key A, [`outdoor_sky_key`]) and the
    /// indoor record from its key B with the interior overrides and no sun or moon. The
    /// record in use (the indoor one with `indoor`, the camera inside with outdoor-visible
    /// clear) is composed ([`compose_state`]) and passed through the sun-change fade, whose
    /// result lights the frame: a change of record that changes "sun present" fades over
    /// 5 s, any other change shows at once. `extent` is the landscape extent *E*; `eye` and
    /// `forward` place the shadow map.
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
        if dt.is_finite() && dt > 0.0 {
            self.raw += dt;
        }
        let state = compose_state(frame.as_ref().map(|f| &f.values), indoor, systems);
        let shown = self.fade.apply(state, self.raw, self.immediate);
        self.immediate = false;
        let composed = frame_lighting(&shown, systems, eye, forward, time);
        let lighting = composed.lighting;
        let fog = lighting.fog;
        let g = global_limit(
            Some(extent),
            fog.map(|f| FogClip {
                enabled: true,
                far: f.far,
            }),
        );
        let clear = if frame.is_some() {
            let c = composed.fog_colour;
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
            sky_fogged: composed.sky_fogged,
        };
        self.frame = frame;
        out
    }

    /// The point lights of the Light entities within [`LIGHT_RADIUS`] of `eye` (at most
    /// [`MAX_TABLE_LIGHTS`], nearest first): the frame's light table in the render frame of
    /// `anchor` (`lighting.md` §8.3: day/night variants by the Tessera rule, intensity
    /// curves on the global clock `time`, *σ* = 1), and each light's reach for the
    /// renderer's selection (`lighting.md` §8.5 steps 1, 3 and 4: ρ from Range and Falloff,
    /// Range for `Light_GlobalLight`; key light; baked into the landscape; the parent
    /// switches, with no parent).
    /// Outdoor lights only with `outdoor`, interior ones only with `interior` (`walker.md`
    /// §14.12 (i): the camera's side of the portal system).
    pub fn lights(
        &mut self,
        streamer: &WorldStreamer,
        anchor: &Anchor,
        eye: &WorldPos,
        night: bool,
        time: f64,
        (outdoor, interior): (bool, bool),
    ) -> (Vec<Light>, Vec<LightReach>) {
        let mut lights = Vec::new();
        let mut reach = Vec::new();
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
            reach.push(light_reach(&l.params, range));
        }
        // Curve states of lights that left the table are dropped.
        if self.curves.len() > 4 * MAX_TABLE_LIGHTS {
            self.curves.retain(|k, _| seen.contains(k));
        }
        (lights, reach)
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

        // The overrides replace the keyframe ambient of the indoor record.
        let values = SkyValues::defaults();
        let c = compose(
            Some(&values),
            Some(&bright),
            &Systems::all(),
            Vec3::ZERO,
            Vec3::X,
            0.0,
        );
        assert!(c.lighting.sun.is_none() && c.lighting.shadow.is_none());
        assert!((c.lighting.ambient[0] - 0.55).abs() < 1e-6);
        assert!((c.lighting.hemispheric[0] - 0.45).abs() < 1e-6);
        assert_eq!(c.lighting.ambient[1], 0.2);
        // Without a sky library (or with `sky` off) the defaults of `lighting.md` §8.1 stand
        // and there is no fog (`walker.md` §14.3, review finding: the interior ambient was
        // applied); the indoor record still has no main light.
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
        let d = SkyValues::defaults().lighting();
        assert_eq!(f.lighting.ambient, d.frame_ambient);
        assert_eq!(f.lighting.hemispheric, d.frame_hemispheric);
        assert!(f.lighting.fog.is_none() && f.lighting.sun.is_none());
        // Outdoors again: "sun present" changes, so the sun fades in over 5 s
        // (`environment.md` §11.6 E37): the old state at the change (weight 0), then a sun.
        let mut out = |dt: f64| {
            env.update(
                dt,
                None,
                None,
                &Systems::all(),
                2_080.0,
                Vec3::ZERO,
                Vec3::X,
                0.0,
            )
        };
        assert!(out(0.016).lighting.sun.is_none());
        assert!(out(1.0).lighting.sun.is_some());
    }

    /// `environment.md` §11.6 "Switching between the tracks" (E37), `landblock.md` §4.13,
    /// `lighting.md` §8.1 (L40): going indoors by day fades the lighting over 5 s of the raw
    /// timer, the main light dimming toward black; an immediate update switches at once.
    #[test]
    fn the_change_of_record_is_smoothed_by_the_sun_change_fade() {
        let mut env = WorldEnv::new(Some(library()), &ClockConfig::default());
        let all = Systems::all();
        let inside = IndoorInputs {
            upper: Some([0.0; 3]),
            lower: Some([0.0; 3]),
            ..IndoorInputs::default()
        };
        let step = |env: &mut WorldEnv, dt: f64, indoor: Option<&IndoorInputs>| {
            env.update(dt, None, indoor, &all, 2_080.0, Vec3::ZERO, Vec3::X, 0.0)
        };
        let out = step(&mut env, 0.016, None);
        let day_sun = out.lighting.sun.expect("the defaults have a sun");
        // Into the cell: weight 0 at the change, then the fade runs.
        let f = step(&mut env, 0.5, Some(&inside));
        assert_eq!(f.lighting.ambient, out.lighting.ambient);
        let f = step(&mut env, 2.5, Some(&inside));
        let sun = f.lighting.sun.expect("the sun stays until the fade ends");
        assert!(sun.color[0] < day_sun.color[0] && sun.color[0] > 0.0);
        assert!(f.lighting.ambient[0] < out.lighting.ambient[0] && f.lighting.ambient[0] > 0.0);
        let f = step(&mut env, 2.6, Some(&inside));
        assert!(f.lighting.sun.is_none(), "5 s after the change");
        assert_eq!(f.lighting.ambient, [0.0; 3]);
        // A teleport (immediate update) switches at once.
        env.mark_immediate();
        let f = step(&mut env, 0.016, None);
        assert_eq!(f.lighting.sun, Some(day_sun));
    }

    /// A sky library with only a calendar (no day descriptions): the environment then
    /// runs its clock and gives the defaults.
    fn library() -> Arc<SkyLibrary> {
        use tessera_formats::environment::{GameTime, SkyDescription};
        let mut src = tessera_world::MemoryWorldSource::new();
        let sky = SkyDescription {
            did: Did(0x1500_0007),
            game_time: Did(0x3500_0000),
            ..SkyDescription::default()
        };
        let gt = GameTime {
            did: Did(0x3500_0000),
            day_length: 11_160.0,
            days_per_year: 360,
            zero_offset: 3_600.0,
            zero_year: 34,
            ..GameTime::default()
        };
        src.insert(sky.did, sky.to_bytes(), None);
        src.insert(gt.did, gt.to_bytes(), None);
        Arc::new(SkyLibrary::load_from(Arc::new(src), sky.did).expect("library"))
    }

    /// `walker.md` §14.3: `[` and `]` move the clock by 1/24 day across midnight, not
    /// round within the day (review finding).
    #[test]
    fn time_steps_cross_midnight() {
        let clock = ClockConfig {
            time_of_day: 0.98,
            rate: 0.0,
            ..ClockConfig::default()
        };
        let mut env = WorldEnv::new(Some(library()), &clock);
        let read = |e: &WorldEnv| e.env.as_ref().map(|x| x.clock().read()).unwrap();
        let before = read(&env);
        env.step_time(1.0);
        let after = read(&env);
        assert_eq!(after.day_number, before.day_number + 1, "the next day");
        assert!((after.time_of_day - (0.98 + TIME_STEP - 1.0)).abs() < 1e-9);
        env.step_time(-1.0);
        let back = read(&env);
        assert_eq!(back.day_number, before.day_number);
        assert!((back.time_of_day - 0.98).abs() < 1e-9);
        // Both records move together.
        let g = |e: &Option<Environment>| e.as_ref().map(|x| x.clock().game_seconds());
        assert_eq!(g(&env.env), g(&env.indoor));
    }

    /// `walker.md` §14.3: a clock stopped at rate 0 restarts at rate 1 with the first `\`
    /// (review finding: the first press only "paused" it again).
    #[test]
    fn a_clock_started_at_rate_0_restarts_with_one_press() {
        let stopped = ClockConfig {
            rate: 0.0,
            ..ClockConfig::default()
        };
        let mut env = WorldEnv::new(Some(library()), &stopped);
        assert_eq!(env.rate(), 0.0);
        assert!(!env.toggle_pause(), "running after one press");
        assert_eq!(env.rate(), 1.0);
        assert!(env.toggle_pause(), "stopped again");
        assert_eq!(env.rate(), 0.0);
        assert!(!env.toggle_pause());
        assert_eq!(env.rate(), 1.0, "the rate before the pause");
        // While stopped, `-` and `=` change the rate it restarts at.
        let mut env = WorldEnv::new(Some(library()), &stopped);
        env.scale_rate(2.0);
        assert_eq!(env.rate(), 0.0, "still stopped");
        assert!(!env.toggle_pause());
        assert_eq!(env.rate(), 2.0);
        // A running clock is paused by the first press, as before.
        let mut env = WorldEnv::new(Some(library()), &ClockConfig::default());
        assert!(env.toggle_pause());
        assert_eq!(env.rate(), 0.0);
    }

    /// `landblock.md` §4.13 and `environment.md` §11.7 step 9 (colour and brightness).
    #[test]
    fn a_black_or_dim_sun_gives_no_main_light() {
        let mut v = SkyValues::defaults();
        assert!(sun_present(&v));
        v.sun_colour = [SUN_EPSILON, 0.0, -SUN_EPSILON];
        assert!(!sun_present(&v), "black within 0.0002, inclusive");
        v.sun_colour = [0.0003, 0.0, 0.0];
        assert!(sun_present(&v));
        v.sun_brightness = 0.000_19;
        assert!(!sun_present(&v), "brightness below 0.0002");
        v.sun_brightness = SUN_EPSILON;
        assert!(sun_present(&v), "exactly 0.0002 keeps the light");
        v.sun_brightness = f32::NAN;
        assert!(!sun_present(&v));
        // A weather effect: no main light either (`environment.md` §11.7 step 9).
        let mut wet = SkyValues::defaults();
        wet.effect = Some(tessera_formats::environment::KeyframeEffect {
            object: Did(0x4700_0009),
            extra: 0,
        });
        assert!(!sun_present(&wet));
        let c = compose(
            Some(&wet.with_outdoor_sun()),
            None,
            &Systems::all(),
            Vec3::ZERO,
            Vec3::X,
            0.0,
        );
        assert!(c.lighting.sun.is_none());
        // No main light means no sun term and no shadow map; the light slot is free.
        let mut black = SkyValues::defaults();
        black.sun_colour = [0.0; 3];
        let black = black.with_outdoor_sun();
        let c = compose(
            Some(&black),
            None,
            &Systems::all(),
            Vec3::ZERO,
            Vec3::X,
            0.0,
        );
        assert!(c.lighting.sun.is_none() && c.lighting.shadow.is_none());
        let c = compose(
            Some(&SkyValues::defaults()),
            None,
            &Systems::all(),
            Vec3::ZERO,
            Vec3::X,
            0.0,
        );
        assert!(c.lighting.sun.is_some() && c.lighting.shadow.is_some());
    }

    /// `environment.md` §11.7 step 5: the 10,000 cap applies to the keyframe end before
    /// the interior override, the lower limits after it.
    #[test]
    fn fog_limits_stand_around_the_override() {
        assert_eq!(fog_distances(0.0, 20_000.0, None, None), (0.0, 10_000.0));
        assert_eq!(
            fog_distances(0.0, 20_000.0, None, Some(15_000.0)),
            (0.0, 15_000.0),
            "an overriding end is not capped"
        );
        assert_eq!(fog_distances(50.0, 10.0, None, None), (0.0, 20.0));
        assert_eq!(
            fog_distances(-20_000.0, 100.0, None, None),
            (-10_000.0, 100.0)
        );
        assert_eq!(
            fog_distances(0.0, 1_000.0, Some(990.0), None),
            (980.0, 1_000.0)
        );
        assert_eq!(
            fog_distances(f32::NAN, f32::NAN, None, None),
            (-10_000.0, 10_000.0)
        );
        let ind = IndoorInputs {
            fog_type: Some(1),
            fog_far: Some(15_000.0),
            ..IndoorInputs::default()
        };
        let c = compose(
            Some(&SkyValues::defaults()),
            Some(&ind),
            &Systems::all(),
            Vec3::ZERO,
            Vec3::X,
            0.0,
        );
        assert_eq!(c.lighting.fog.map(|f| f.far), Some(15_000.0));
    }

    /// `environment.md` §11.7 step 10.
    #[test]
    fn heavy_fog_hides_the_sky_objects() {
        let mut v = SkyValues::defaults();
        v.fog_extra = 0.995;
        let all = Systems::all();
        let c = compose(Some(&v), None, &all, Vec3::ZERO, Vec3::X, 0.0);
        assert!(c.sky_fogged);
        let mut no_fog = Systems::all();
        no_fog.set(System::Fog, false);
        assert!(!compose(Some(&v), None, &no_fog, Vec3::ZERO, Vec3::X, 0.0).sky_fogged);
        v.fog_extra = 0.99;
        assert!(!compose(Some(&v), None, &all, Vec3::ZERO, Vec3::X, 0.0).sky_fogged);
        // Fog switched off by `InteriorFogType` None: the sky is not hidden.
        v.fog_extra = 1.0;
        let none = IndoorInputs {
            fog_type: Some(2),
            ..IndoorInputs::default()
        };
        assert!(!compose(Some(&v), Some(&none), &all, Vec3::ZERO, Vec3::X, 0.0).sky_fogged);
    }

    /// `environment.md` §7.1.1: the keys and `InteriorFogType` count only with type 8 or
    /// 10, and the lookup stops at the first set that has the property.
    #[test]
    fn keys_count_only_with_enum_or_property_id_values() {
        use tessera_formats::PropertyValue;
        assert_eq!(stored_key(&PropertyValue::Enum(7)), Some(7));
        assert_eq!(stored_key(&PropertyValue::PropertyId(9)), Some(9));
        assert_eq!(stored_key(&PropertyValue::Int(1)), None);
        assert_eq!(stored_key(&PropertyValue::DidRef(Did(3))), None);
        let pids = IndoorPids {
            indoor_ambiance: Some(1),
            fog_type: Some(5),
            ..IndoorPids::default()
        };
        let set: HashMap<u32, PropertyValue> =
            HashMap::from([(1, PropertyValue::Int(4)), (5, PropertyValue::Int(1))]);
        let ind = IndoorInputs::read(&pids, |pid| set.get(&pid));
        assert_eq!(
            ind.sky_key, DEFAULT_INDOOR_SKY_KEY,
            "an int key B is absent"
        );
        assert_eq!(
            ind.fog_type, None,
            "an int fog type leaves the keyframe's fog"
        );
        assert_eq!(
            outdoor_sky_key(None, Some(1)),
            DEFAULT_OUTDOOR_SKY_KEY,
            "no landblock: the value 2"
        );
    }

    /// Review finding (`landblock.md` §4.13): without a camera cell the lookup still
    /// reads at the camera position, the landblock under the eye without a cell; in a
    /// cell, the cell and its owner.
    #[test]
    fn the_lookup_reads_at_the_camera_position() {
        let eye = WorldPos::new(1, 160.0 * 7.0 + 5.0, 160.0 * 9.0 + 5.0, 0.0);
        let under = eye.landblock().expect("on the grid");
        assert_eq!(lookup_target(None, &eye), (Some(under), None));
        let owner = LandblockKey::new(1, 3, 3).unwrap();
        assert_eq!(
            lookup_target(Some((owner, 9)), &eye),
            (Some(owner), Some(9)),
            "the cell's owner, not the landblock under the eye"
        );
        let off = WorldPos::new(0, 0.0, 0.0, 0.0);
        assert_eq!(lookup_target(None, &off).1, None);
        let ind = indoor_inputs_at(None, &IndoorPids::default());
        assert_eq!(ind.sky_key, DEFAULT_INDOOR_SKY_KEY);
    }

    #[test]
    fn light_reach_follows_range_falloff_and_switches() {
        let params = tessera_world::LightParams {
            falloff: 1.0,
            key_light: true,
            baked_landscape: true,
            ..tessera_world::LightParams::default()
        };
        let r = light_reach(&params, 20.0);
        assert!(
            r.radius.is_some_and(|x| (x - 17.0).abs() < 1e-4),
            "0.85 of the range at falloff 1"
        );
        assert!(r.key_light && r.baked_landscape && r.parent.is_none());
        assert!(!r.only_influence_parent && !r.never_influence_parent);
        let global = tessera_world::LightParams {
            global: true,
            ..params.clone()
        };
        assert_eq!(light_reach(&global, 20.0).radius, Some(20.0));
        // The parent switches are passed on (`lighting.md` §8.5 step 3, row W-L2).
        let only = tessera_world::LightParams {
            only_influence_parent: true,
            never_influence_parent: true,
            ..params
        };
        let r = light_reach(&only, 20.0);
        assert!(r.only_influence_parent && r.never_influence_parent && r.parent.is_none());
    }
}
