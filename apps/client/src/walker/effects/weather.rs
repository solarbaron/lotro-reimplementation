// SPDX-License-Identifier: GPL-3.0-or-later
//! The weather and water systems the client runs itself (`particle-runtime.md` §11.2,
//! Tessera rule 17; `particles.md` §7.1.3, §8.8, §8.8.1).
//!
//! - **The ten systems** ([`WeatherSystem`]) are named by UNIQUEDB keys; their DIDs are read
//!   from the install's UNIQUEDB map at start-up (on the effects layer's start-up worker,
//!   [`read_weather_dids`]), their records are read on the workers like any effect record,
//!   and each system is created once, when its record has loaded, and kept for the layer's
//!   life (a new region builds them again with the layer's new manager).
//! - **The global wind** (§11.2.1): every world frame its target is the wind vector
//!   *w* = (*c*₃, *c*₄, 0) of the environment state in use ([`super::super::env::Precipitation`]),
//!   and every placement of the frame takes its eased value ([`tessera_particles::Wind`]), so a
//!   system with physics flag 0x20 takes the environment's wind at its start.
//! - **The precipitation step** (§11.2.2–§11.2.4) runs while the landscape is visible and the
//!   `[Render]` option `PrecipitationEffects` is on: the fall direction ([`fall_direction`]),
//!   the sky-exposure tests every 0.5 s along −*f* over 100 m from the camera and the
//!   avatar ([`Exposure`], through the walker's collision world), the gust forces, the
//!   branch and its thresholds ([`plan`]), the camera systems placed with the camera's
//!   frame, and the impact spawns ([`impact_bound`]).
//! - **The water systems** are placed in every world frame, whatever the weather (§11.2.2
//!   step 4).
//! - **The drops** (§11.2.6, rule 17): the walker keeps the rain and the snow drop set
//!   ([`tessera_particles::weather::DropSet`]); each update counts its drops with
//!   `[Render] AtmosphericsDetail` (built-in `Medium`,
//!   [`super::EffectLayer::set_atmospherics_detail`]; nothing in the walker sets it yet, so
//!   the built-in level is used) and does nothing else when the count is 0 (no exposure
//!   test, no force, no drop, no spawn, no exposure reported: the gate of `particles.md`
//!   §8.8.1, P59; [`UpdateGate`]). The two sets share one bound of
//!   [`MAX_DROP_PLACEMENTS_PER_FRAME`] placements per world frame (each one a collision
//!   test on the frame thread); the drops beyond it stay marked for a later frame. The field,
//!   placement, fall and reference surface of steps 2–5 run on the walker's collision world
//!   ([`WalkerDropWorld`]) and on the water probe of §5.7 at the camera; the spawns of
//!   §11.2.3 step 4 go to the vapour, ripple and gust systems, and the streaks of step 7 are
//!   drawn in the translucent stage of the world pass with the surface of UNIQUEDB key 0x2B
//!   (rain) or 0x3B (snow), every frame in which the precipitation step runs
//!   ([`super::EffectLayer::draw`]). Snow follows the interim Tessera rule of rule 17: the
//!   rain steps with the snow parameters, one flake per drop drawn as a streak of at most
//!   0.08 m. The vertex colours are multiplied by the environment state's `colour_4`
//!   ([`super::EffectLayer::set_precipitation_colour`]; white until the walker sets it, and
//!   nothing in the walker sets it yet).
//! - **The player's figure** (§11.2.5, rule 17): every bone of the avatar's skeleton with a
//!   parent bone counts as marked; the water surface and the lava flag are the walker's own
//!   water state at the feet ([`super::super::movement::CellMaskedWater`], `walker.md`
//!   §13.5); `Volume_IsCoin` has no source in the walker and counts as unset.
//!
//! **Tessera rules and choices**: the gust, vapour and water systems are placed at the
//! camera's position with no rotation, and every spawn position is handed to them relative
//! to their placement, so the particle appears at the world point the spec gives (§11.2.4
//! step 6 answer); the exposure tests and the drop segments see every body of the collision
//! world ([`tessera_physics::geometry::QueryFilter::all`]), tested in pieces of at most
//! [`RAY_PIECE`] m from the start ([`first_contact`]: the same first contact at a fraction
//! of the cost of one sweep over the whole ray's box); a position whose landblock has
//! no installed collision set counts as "cannot place" (not exposed; a drop is tried again
//! in the next update); the snow gust's ratio *n* / 3,000 is the one at the first snow gust
//! spawn of the layer's life (§11.2.3 step 4).

use tessera_particles::glam::{Mat3, Mat4, Quat, Vec3};
use tessera_particles::rng::Ran2;
use tessera_particles::runtime::AttachSpec;
use tessera_particles::weather::{
    DropInput, DropParams, DropSet, DropUpdate, DropWorld, FigureInput, FigureSpawn, FigureWater,
    GUST_COUNT_SCALE, RIPPLE_RAISE, SegmentTest, drop_count, figure_spawns, gust_probability,
    ripple_in_reach,
};
use tessera_particles::{Environment, LightList, NoAttachments, Wind};
use tessera_physics::controller::WaterQuery;
use tessera_physics::geometry::{CollisionQuery, CollisionView, QueryFilter};
use tessera_world::{WorldPos, WorldStreamer};

use super::super::env::Precipitation;
use super::data::EffectRecord;

/// One of the ten engine-run systems (`particles.md` §7.1.3; `particle-runtime.md` §11.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeatherSystem {
    /// 0x7B `CameraRainTorrentParticleSystem`.
    RainTorrent,
    /// 0x2A `CameraRainImpactParticleSystem`.
    RainImpact,
    /// 0x6C `RainVaporParticleSystem`.
    RainVapour,
    /// 0x83 `RainGustParticleSystem`.
    RainGust,
    /// 0xFD `CameraInBlizzardParticleSystem`.
    SnowBlizzard,
    /// 0x100 `CameraSnowImpactParticleSystem`.
    SnowImpact,
    /// 0xFE `SnowGustParticleSystem`.
    SnowGust,
    /// 0x4A `WaterRipplesParticleSystem`.
    WaterRipples,
    /// 0xA1 `WaterBubblesParticleSystem`.
    WaterBubbles,
    /// 0x1E7 `CoinSwimRipplesParticleSystem`.
    CoinRipples,
}

impl WeatherSystem {
    /// The ten systems, in [`WeatherSystem::index`] order.
    pub const ALL: [Self; WEATHER_SYSTEMS] = [
        Self::RainTorrent,
        Self::RainImpact,
        Self::RainVapour,
        Self::RainGust,
        Self::SnowBlizzard,
        Self::SnowImpact,
        Self::SnowGust,
        Self::WaterRipples,
        Self::WaterBubbles,
        Self::CoinRipples,
    ];

    /// The UNIQUEDB key that names the system's record (`particles.md` §7.1.3).
    #[must_use]
    pub fn key(self) -> u32 {
        match self {
            Self::RainTorrent => 0x7B,
            Self::RainImpact => 0x2A,
            Self::RainVapour => 0x6C,
            Self::RainGust => 0x83,
            Self::SnowBlizzard => 0xFD,
            Self::SnowImpact => 0x100,
            Self::SnowGust => 0xFE,
            Self::WaterRipples => 0x4A,
            Self::WaterBubbles => 0xA1,
            Self::CoinRipples => 0x1E7,
        }
    }

    /// Its index in [`WeatherSystem::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// Placed with the camera's frame (§11.2.4 step 6).
    #[must_use]
    pub fn at_camera(self) -> bool {
        matches!(
            self,
            Self::RainTorrent | Self::RainImpact | Self::SnowBlizzard | Self::SnowImpact
        )
    }
}

/// Number of engine-run systems.
pub const WEATHER_SYSTEMS: usize = 10;

/// Owner IDs of the weather systems in the effect manager: this base | the system's index.
/// Its range lies apart from the session owners (`1 << 32`), the beams (`2 << 32`), the
/// creation-stage owners (`3 << 32`) and the static and placed-figure owners (a landblock
/// DID, `0x8…`, in the high word); the assertions below hold it there.
pub const WEATHER_OWNER_BASE: u64 = 4 << 32;

/// The runtime-ID ranges of the effect manager's owner kinds, `[first, end)` (session
/// owners and beams are given their whole `1 << 32` block).
#[cfg(test)]
pub(crate) const OWNER_RANGES: [(&str, u64, u64); 5] = [
    (
        "session",
        super::session::SESSION_OWNER_BASE,
        super::session::SESSION_OWNER_BASE + (1 << 32),
    ),
    (
        "beam",
        super::session::BEAM_OWNER_BASE,
        super::session::BEAM_OWNER_BASE + (1 << 32),
    ),
    (
        "stage",
        super::data::STAGE_OWNER_BASE,
        super::data::STAGE_OWNER_BASE + super::data::MAX_STAGE_ELEMENTS as u64,
    ),
    (
        "weather",
        WEATHER_OWNER_BASE,
        WEATHER_OWNER_BASE + WEATHER_SYSTEMS as u64,
    ),
    // The smallest landblock DID (`0x8000_0000`) in the high word, to the end.
    ("static and placed figure", 0x8000_0000 << 32, u64::MAX),
];

const _: () = {
    // The weather range sits above the stage range and below every landblock DID.
    assert!(
        super::data::STAGE_OWNER_BASE + super::data::MAX_STAGE_ELEMENTS as u64
            <= WEATHER_OWNER_BASE
    );
    assert!(WEATHER_OWNER_BASE + WEATHER_SYSTEMS as u64 <= 0x8000_0000 << 32);
    assert!(super::session::BEAM_OWNER_BASE + (1 << 32) <= super::data::STAGE_OWNER_BASE);
};

/// Seconds of the global clock between two sky-exposure tests (§11.2.3 step 2).
pub const EXPOSURE_INTERVAL: f64 = 0.5;
/// Length of the sky-exposure test along −*f*, metres (§11.2.3 step 2).
pub const EXPOSURE_REACH: f32 = 100.0;
/// A wind-plus-gravity sum this long or shorter gives the fall direction (0, 0, −1)
/// (§11.2.3 step 1).
pub const FALL_EPSILON: f32 = 0.000_001;
/// The gravity of the fall direction, m/s² (§11.2.3 step 1).
pub const FALL_GRAVITY: f32 = -9.8;
/// Rain above this places the gust and vapour systems (§11.2.4 step 3).
pub const RAIN_GUST_MIN: f32 = 0.05;
/// A branch strength above this places the camera systems (§11.2.4 steps 3–4).
pub const STRENGTH_MIN: f32 = 0.001;
/// The snow value above which the snow gust system runs (`particles.md` §8.8.1).
pub const SNOW_GUST_MIN: f32 = 1.001;
/// Smallest −(*f* · *a*) that can spawn an impact particle (§11.2.4 step 5).
pub const IMPACT_MIN_DOT: f32 = 0.1;
/// Lower bound of the impact draw (§11.2.4 step 5).
pub const IMPACT_DRAW_MIN: i64 = -8;

/// The fall direction *f* (§11.2.3 step 1): the unit vector of (`wind` + (0, 0, −9.8)), or
/// (0, 0, −1) when that sum is [`FALL_EPSILON`] long or shorter (or not finite); the same
/// rule as [`tessera_particles::weather::fall_direction`].
#[must_use]
pub fn fall_direction(wind: Vec3) -> Vec3 {
    tessera_particles::weather::fall_direction(wind)
}

/// The branch of an update (§11.2.4 step 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Branch {
    /// *r* > *s*.
    Rain,
    /// *r* ≤ *s*.
    Snow,
}

/// What one precipitation step places (§11.2.4 steps 1–4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherPlan {
    /// The branch.
    pub branch: Branch,
    /// The branch's strength *g*.
    pub strength: f32,
    /// The branch's gust system is placed (and, for rain, the vapour system).
    pub gust: bool,
    /// The branch's two camera systems (torrent or blizzard, and impact) are placed.
    pub camera: bool,
}

impl WeatherPlan {
    /// The systems this plan places.
    #[must_use]
    pub fn systems(&self) -> Vec<WeatherSystem> {
        let mut out = Vec::new();
        match self.branch {
            Branch::Rain => {
                if self.gust {
                    out.extend([WeatherSystem::RainGust, WeatherSystem::RainVapour]);
                }
                if self.camera {
                    out.extend([WeatherSystem::RainTorrent, WeatherSystem::RainImpact]);
                }
            }
            Branch::Snow => {
                if self.gust {
                    out.push(WeatherSystem::SnowGust);
                }
                if self.camera {
                    out.extend([WeatherSystem::SnowBlizzard, WeatherSystem::SnowImpact]);
                }
            }
        }
        out
    }

    /// The branch's impact system, when this plan places it.
    #[must_use]
    pub fn impact(&self) -> Option<WeatherSystem> {
        self.camera.then_some(match self.branch {
            Branch::Rain => WeatherSystem::RainImpact,
            Branch::Snow => WeatherSystem::SnowImpact,
        })
    }
}

/// The placements of a precipitation step (§11.2.4 steps 1–4) for rain `r`, snow `s`, the
/// camera exposure `exposed` and the camera inside an interior `inside`. Non-finite values
/// count as 0.
#[must_use]
pub fn plan(r: f32, s: f32, exposed: bool, inside: bool) -> WeatherPlan {
    let r = if r.is_finite() { r } else { 0.0 };
    let s = if s.is_finite() { s } else { 0.0 };
    let open = exposed && !inside;
    if r > s {
        let g = 2.0 * (r - 0.5).max(0.0);
        WeatherPlan {
            branch: Branch::Rain,
            strength: g,
            gust: r > RAIN_GUST_MIN,
            camera: open && g > STRENGTH_MIN,
        }
    } else {
        let g = (s - 1.0).max(0.0);
        let gust = g > STRENGTH_MIN;
        WeatherPlan {
            branch: Branch::Snow,
            strength: g,
            gust,
            camera: gust && open,
        }
    }
}

/// The upper bound of the impact draw (§11.2.4 step 5): with *d* = −(*f* · *a*),
/// ⌊(*d* / 0.9)² × 10 × *g*⌋ when *d* ≥ [`IMPACT_MIN_DOT`], else `None` (no draw). Values
/// that are not finite give `None`; the bound is limited to 1,000,000 (hostile input).
#[must_use]
pub fn impact_bound(d: f32, g: f32) -> Option<i64> {
    if !(d.is_finite() && g.is_finite() && d >= IMPACT_MIN_DOT) {
        return None;
    }
    let v = (d / 0.9) * (d / 0.9) * 10.0 * g;
    if !v.is_finite() {
        return None;
    }
    Some(v.floor().clamp(-1.0e6, 1.0e6) as i64)
}

/// Whether the impact system gets one particle in this update (§11.2.4 step 5): an integer
/// *k* drawn uniformly from −8 to [`impact_bound`] inclusive, and *k* ≥ 0. `fall` is the
/// branch's fall direction, `forward` the camera's local +y axis in world space. No draw
/// when there is no bound.
pub fn impact_spawn(fall: Vec3, forward: Vec3, g: f32, rng: &mut Ran2) -> bool {
    let d = -fall.dot(forward);
    match impact_bound(d, g) {
        Some(hi) => rng.range_int(IMPACT_DRAW_MIN, hi) >= 0,
        None => false,
    }
}

/// One update's sky-exposure results (§11.2.3 step 2): tested when more than
/// [`EXPOSURE_INTERVAL`] s of the global clock have passed since the last test, kept until
/// the next.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Exposure {
    last: Option<f64>,
    /// The camera was exposed at the last test.
    pub camera: bool,
    /// The player's position was exposed at the last test.
    pub player: bool,
}

impl Exposure {
    /// Whether a test is due at clock time `now`.
    #[must_use]
    pub fn due(&self, now: f64) -> bool {
        match self.last {
            None => true,
            Some(t) => now.is_nan() || now - t > EXPOSURE_INTERVAL,
        }
    }

    /// Keeps the results of a test at `now`.
    pub fn record(&mut self, now: f64, camera: bool, player: bool) {
        self.last = Some(now);
        self.camera = camera;
        self.player = player;
    }
}

/// Most drops the rain and the snow set place together in one world frame (Tessera
/// engineering bound, module docs): each placement is a collision test of *L* = 100 m on the
/// frame thread, so a refill (the start of rain, a region change, a teleport) spreads over
/// frames (1,800 drops at `Medium` and *r* = 1 in about half a second at 60 frames per
/// second) instead of running up to 1,024 tests in one frame.
pub const MAX_DROP_PLACEMENTS_PER_FRAME: usize = 64;

/// Which parts of a frame's precipitation updates run (`particle-runtime.md` §11.2.6 step 1,
/// `particles.md` P59): an update with no drop does nothing else (no exposure test, no
/// force recomputation, no drop, no spawn) and reports no exposure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UpdateGate {
    /// The rain update runs (the precipitation step runs and its drop count is at least 1).
    pub rain_ran: bool,
    /// The snow update runs.
    pub snow_ran: bool,
    /// The rain update's exposure test runs in this frame (it runs and a test is due).
    pub test_rain: bool,
    /// The snow update's exposure test runs in this frame.
    pub test_snow: bool,
}

impl UpdateGate {
    /// The gate for the precipitation step `step` (landscape visible and
    /// `PrecipitationEffects` on), rain `rain`, snow `snow` and quality factor `quality`;
    /// `rain_due` and `snow_due` say whether each update's exposure test is due.
    #[must_use]
    pub fn new(
        step: bool,
        rain: f32,
        snow: f32,
        quality: f32,
        rain_due: bool,
        snow_due: bool,
    ) -> Self {
        let rain_ran = step && drop_count(DropParams::RAIN.max_drops, rain, quality) > 0;
        let snow_ran = step && drop_count(DropParams::SNOW.max_drops, snow, quality) > 0;
        Self {
            rain_ran,
            snow_ran,
            test_rain: rain_ran && rain_due,
            test_snow: snow_ran && snow_due,
        }
    }

    /// The camera exposure *x* of §11.2.4 step 2: reported only by an update that runs.
    #[must_use]
    pub fn camera_exposed(&self, rain: &Exposure, snow: &Exposure) -> bool {
        (self.rain_ran && rain.camera) || (self.snow_ran && snow.camera)
    }

    /// The player's exposure for the rain on the figure (§11.2.5): reported only by a rain
    /// update that runs.
    #[must_use]
    pub fn player_exposed(&self, rain: &Exposure) -> bool {
        self.rain_ran && rain.player
    }
}

/// One particle the drops of a frame ask for (§11.2.3 step 4): emitter index `emitter` (0
/// for the spec's emitter 1) of `system`, at a point of the simulation frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropParticle {
    /// The system.
    pub system: WeatherSystem,
    /// The emitter index.
    pub emitter: usize,
    /// The point.
    pub at: Vec3,
}

/// The spawns of §11.2.3 step 4 from the drops of `rain` and `snow` in frame `wf`, handed to
/// `spawn` one by one (it returns whether the system added the particle; it gets the shared
/// generator, which the gust draws use too, in the same order). Rain: each spawn point into
/// the vapour system (emitter 2), then into the ripple system (emitter 2, 0.1 m up) when it
/// ended on the reference surface within 15 m of the camera, or else, for a point off the
/// reference surface, into the rain gust system (emitter 1) with probability
/// *n* / 3,000 × 0.015. Snow, only while *s* > 1.001: each point off the reference surface
/// into the snow gust system (emitter 1) with the probability of `snow_gust_ratio`, fixed at
/// the first snow gust spawn. Nothing for an update that did not run. Returns the particles
/// added.
pub fn drop_particles(
    wf: &WeatherFrame,
    rain: &DropSet,
    snow: &DropSet,
    snow_gust_ratio: &mut Option<f32>,
    rng: &mut Ran2,
    mut spawn: impl FnMut(DropParticle, &mut Ran2) -> bool,
) -> usize {
    let mut added = 0usize;
    let mut emit = |system, emitter, at, rng: &mut Ran2| {
        usize::from(spawn(
            DropParticle {
                system,
                emitter,
                at,
            },
            rng,
        ))
    };
    if wf.rain_ran {
        #[allow(clippy::cast_precision_loss)]
        let p = gust_probability(rain.count() as f32 / GUST_COUNT_SCALE);
        for s in rain.spawns() {
            added += emit(WeatherSystem::RainVapour, 1, s.position, rng);
            if ripple_in_reach(s, wf.camera) {
                let at = s.position + Vec3::new(0.0, 0.0, RIPPLE_RAISE);
                added += emit(WeatherSystem::WaterRipples, 1, at, rng);
            } else if !s.on_reference && rng.range(0.0, 1.0) < p {
                added += emit(WeatherSystem::RainGust, 0, s.position, rng);
            }
        }
    }
    if wf.snow_ran && wf.snow > SNOW_GUST_MIN {
        #[allow(clippy::cast_precision_loss)]
        let now_ratio = snow.count() as f32 / GUST_COUNT_SCALE;
        for s in snow.spawns().iter().filter(|s| !s.on_reference) {
            let ratio = *snow_gust_ratio.get_or_insert(now_ratio);
            if rng.range(0.0, 1.0) < gust_probability(ratio) {
                added += emit(WeatherSystem::SnowGust, 0, s.position, rng);
            }
        }
    }
    added
}

/// The rotation of a camera looking along `forward` with world +Z up: local +y is the
/// forward axis (§11.2.4 step 5), +z up, +x to the right. `None` for a direction that is
/// zero or not finite; a vertical direction takes +x as its right axis.
#[must_use]
pub fn camera_rotation(forward: Vec3) -> Option<Quat> {
    let y = forward.try_normalize()?;
    let x = y.cross(Vec3::Z).try_normalize().unwrap_or(Vec3::X);
    let z = x.cross(y).try_normalize()?;
    let q = Quat::from_mat3(&Mat3::from_cols(x, y, z));
    q.is_finite().then(|| q.normalize())
}

/// What the walker gives the weather step each world frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherInput {
    /// The precipitation values of the environment state in use.
    pub precipitation: Precipitation,
    /// The landscape is visible from the viewer (`environment.md` §11.6; the walker's
    /// outdoor-visible flag).
    pub landscape_visible: bool,
    /// The camera lies in an interior cell (§11.2.4 step 2).
    pub inside: bool,
    /// The camera's view direction (world frame).
    pub camera_dir: Vec3,
    /// The avatar's feet, when there is one (walk mode).
    pub avatar: Option<WorldPos>,
}

/// The weather state of the effects layer.
#[derive(Debug, Clone, PartialEq)]
pub struct Weather {
    /// The ten DIDs read from UNIQUEDB (`None`: the key names no record).
    pub dids: [Option<u32>; WEATHER_SYSTEMS],
    /// The running systems (keys of the layer's manager).
    pub keys: [Option<tessera_particles::SystemKey>; WEATHER_SYSTEMS],
    /// Systems whose record is missing or not a 0x39 system (never retried).
    pub failed: [bool; WEATHER_SYSTEMS],
    /// The eased global wind (§11.2.1 step 4: calm at start).
    pub wind: Wind,
    /// The rain update's exposure results.
    pub rain: Exposure,
    /// The snow update's exposure results.
    pub snow: Exposure,
    /// This frame's input (set before the placement).
    pub frame: Option<WeatherFrame>,
    /// The surface records of the rain and snow drops (UNIQUEDB keys 0x2B and 0x3B;
    /// `None`: the key names no record).
    pub materials: [Option<u32>; 2],
    /// The rain drop set (§11.2.6).
    pub rain_drops: DropSet,
    /// The snow drop set (§11.2.6 and the interim snow rule of rule 17).
    pub snow_drops: DropSet,
    /// The snow gust's ratio *n* / 3,000, fixed at the first snow gust spawn (§11.2.3
    /// step 4).
    pub snow_gust_ratio: Option<f32>,
    /// The environment state's `colour_4` that multiplies the streak colours (§11.2.6
    /// step 7; white until the walker sets it).
    pub tint: [f32; 4],
    /// The figure effects of this frame (§11.2.5), spawned at the placement.
    pub figure_spawns: Vec<FigureSpawn>,
    /// Counters.
    pub stats: WeatherStats,
}

/// Counters of the weather work.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WeatherStats {
    /// Rain drops in use in the last world frame (0 when gated).
    pub rain_drops: usize,
    /// Snow drops in use in the last world frame.
    pub snow_drops: usize,
    /// Drops placed in the last world frame.
    pub drops_placed: usize,
    /// Particles spawned from the drops in the last world frame (§11.2.3 step 4).
    pub drop_spawns: usize,
    /// Particles spawned by the figure effects in the last world frame (§11.2.5).
    pub figure_spawns: usize,
    /// Streak vertices drawn in the last frame (§11.2.6 step 7).
    pub streak_vertices: usize,
    /// The most streak vertices drawn in one frame so far.
    pub max_streak_vertices: usize,
}

/// One frame's weather work, in the layer's simulation frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherFrame {
    /// The camera's position (simulation frame).
    pub camera: Vec3,
    /// The camera's rotation ([`camera_rotation`]).
    pub rotation: Quat,
    /// The camera's local +y axis in world space.
    pub forward: Vec3,
    /// The precipitation step runs (landscape visible, `PrecipitationEffects` on).
    pub precipitation_step: bool,
    /// Rain *r* and snow *s*.
    pub rain: f32,
    /// See [`WeatherFrame::rain`].
    pub snow: f32,
    /// The fall direction of this frame (the same for rain and snow, §11.2.3 step 1).
    pub fall: Vec3,
    /// Camera exposure *x* (§11.2.4 step 2).
    pub exposed: bool,
    /// The camera is inside an interior.
    pub inside: bool,
    /// The rain update ran (its drop count is at least 1, §11.2.6 step 1).
    pub rain_ran: bool,
    /// The snow update ran.
    pub snow_ran: bool,
}

impl WeatherFrame {
    /// The rain gust system's forces are recomputed in this frame (§11.2.3 step 3: in every
    /// rain update that runs, §11.2.6 step 1).
    #[must_use]
    pub fn rain_gust_forces(&self) -> bool {
        self.precipitation_step && self.rain_ran
    }

    /// The snow gust system's forces are recomputed in this frame (§11.2.3 step 3: in a
    /// snow update that runs, only while *s* > 1.001, `particles.md` §8.8.1).
    #[must_use]
    pub fn snow_gust_forces(&self) -> bool {
        self.precipitation_step && self.snow_ran && self.snow > SNOW_GUST_MIN
    }
}

/// Whether `pos` is exposed to the sky along −`fall` (§11.2.3 step 2): a ray of
/// [`EXPOSURE_REACH`] m through the walker's collision world around `pos` finds nothing in
/// the way. A position whose landblock has no installed collision set cannot be placed and
/// is not exposed.
#[must_use]
pub fn sky_exposed(streamer: &WorldStreamer, pos: &WorldPos, fall: Vec3) -> bool {
    let Some(key) = pos.landblock() else {
        return false;
    };
    let view = streamer.collision_view(pos, 1);
    if !super::super::movement::holds(&view, key) {
        return false;
    }
    let from = view.to_view([pos.x, pos.y, pos.z]);
    let delta = (-fall * EXPOSURE_REACH).to_array();
    if !(from.iter().all(|v| v.is_finite()) && delta.iter().all(|v| v.is_finite())) {
        return false;
    }
    first_contact(&view, from, delta).is_none()
}

/// Longest piece of a long ray tested at once, metres (Tessera engineering choice): a
/// sweep visits every collision item whose box overlaps the box of its whole motion, so a
/// 100 m ray that leans 45° in the wind would visit the items of a box about 70 m on a side.
pub const RAY_PIECE: f32 = 8.0;
/// Each piece after the first starts this far back along the ray, metres, so that a
/// surface at the joint is not missed.
pub const RAY_PIECE_OVERLAP: f32 = 0.01;
/// Most pieces of one ray.
pub const MAX_RAY_PIECES: usize = 64;

/// The first contact point (the view's frame) of a ray from `from` along `delta` through
/// every body of `view`, tested in pieces of at most [`RAY_PIECE`] m in order from the
/// start (the first piece with a contact gives it, so the answer is the first contact of
/// the whole ray); `None` when nothing is in the way or the input is not finite.
#[must_use]
pub fn first_contact(
    view: &CollisionView<'_>,
    from: [f32; 3],
    delta: [f32; 3],
) -> Option<[f32; 3]> {
    let o = Vec3::from_array(from);
    let d = Vec3::from_array(delta);
    let len = d.length();
    if !(o.is_finite() && len.is_finite()) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let pieces = ((len / RAY_PIECE).ceil() as usize).clamp(1, MAX_RAY_PIECES);
    #[allow(clippy::cast_precision_loss)]
    let step = d / pieces as f32;
    let back = step.normalize_or_zero() * RAY_PIECE_OVERLAP;
    for k in 0..pieces {
        let (start, motion) = if k == 0 {
            (o, step)
        } else {
            #[allow(clippy::cast_precision_loss)]
            let a = o + step * k as f32;
            (a - back, step + back)
        };
        if let Some(hit) = view.raycast(start.to_array(), motion.to_array(), &QueryFilter::all()) {
            return Some(hit.point);
        }
    }
    None
}

/// The drop segments of §11.2.6 step 3 on the walker's collision world, in the effects
/// layer's simulation frame (region coordinates minus `base`).
pub struct WalkerDropWorld<'a> {
    view: CollisionView<'a>,
    region: u8,
    base: [f64; 2],
    /// Segments tested.
    pub tests: usize,
}

impl std::fmt::Debug for WalkerDropWorld<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalkerDropWorld")
            .field("region", &self.region)
            .field("tests", &self.tests)
            .finish_non_exhaustive()
    }
}

impl<'a> WalkerDropWorld<'a> {
    /// The collision world around `centre` (the camera; its landblock and the eight around
    /// it, which hold every drop of the 30 m field).
    #[must_use]
    pub fn new(streamer: &'a WorldStreamer, centre: &WorldPos, base: [f64; 2]) -> Self {
        Self {
            view: streamer.collision_view(centre, 1),
            region: centre.region,
            base,
            tests: 0,
        }
    }
}

impl DropWorld for WalkerDropWorld<'_> {
    /// The segment from `start` along `delta`: the first contact of a ray through every body
    /// of the view; "cannot run" when the point the segment is drawn through (its middle,
    /// at the camera's height) lies in a landblock without an installed collision set.
    fn segment(&mut self, start: Vec3, delta: Vec3) -> SegmentTest {
        let mid = start + delta * 0.5;
        let region = |p: Vec3| {
            [
                f64::from(p.x) + self.base[0],
                f64::from(p.y) + self.base[1],
                f64::from(p.z),
            ]
        };
        let m = region(mid);
        let Some(key) = WorldPos::new(self.region, m[0], m[1], m[2]).landblock() else {
            return SegmentTest::CannotRun;
        };
        if !super::super::movement::holds(&self.view, key) {
            return SegmentTest::CannotRun;
        }
        let from = self.view.to_view(region(start));
        let delta = delta.to_array();
        if !(from.iter().all(|v| v.is_finite()) && delta.iter().all(|v| v.is_finite())) {
            return SegmentTest::CannotRun;
        }
        self.tests += 1;
        match first_contact(&self.view, from, delta) {
            Some(point) => {
                let r = self.view.to_region(point);
                #[allow(clippy::cast_possible_truncation)]
                let p = Vec3::new(
                    (r[0] - self.base[0]) as f32,
                    (r[1] - self.base[1]) as f32,
                    r[2] as f32,
                );
                SegmentTest::Hit(p)
            }
            None => SegmentTest::Clear,
        }
    }
}

/// Most bone segments of the player's figure the figure effects visit in one frame.
pub const MAX_FIGURE_SEGMENTS: usize = 256;

/// The bone segments of a figure for §11.2.5, in the simulation frame: for each bone with
/// a parent bone (every one counts as marked, rule 17), the segment from the parent's
/// position to the bone's, through the figure's world transform (its position, rotation and
/// scale); at most [`MAX_FIGURE_SEGMENTS`]. `to_sim` turns region coordinates into the
/// simulation frame.
#[must_use]
pub fn figure_segments(
    view: &super::super::characters::FigureEffectView,
    to_sim: impl Fn([f64; 3]) -> Vec3,
) -> Vec<(Vec3, Vec3)> {
    let Some(skeleton) = view.skeleton.as_ref() else {
        return Vec::new();
    };
    let origin = to_sim(view.position);
    let scale = if view.scale.is_finite() {
        view.scale
    } else {
        Vec3::ONE
    };
    let rotation = if view.rotation.is_finite() {
        view.rotation.normalize()
    } else {
        Quat::IDENTITY
    };
    let world = Mat4::from_scale_rotation_translation(scale, rotation, origin);
    let at = |i: usize| {
        view.model
            .get(i)
            .map(|m| world.transform_point3(m.w_axis.truncate()))
    };
    let mut out = Vec::new();
    for i in 0..view.model.len().min(skeleton.bone_count()) {
        if out.len() >= MAX_FIGURE_SEGMENTS {
            break;
        }
        let Some(parent) = skeleton.parent(i) else {
            continue;
        };
        if let (Some(a), Some(b)) = (at(parent), at(i))
            && a.is_finite()
            && b.is_finite()
        {
            out.push((a, b));
        }
    }
    out
}

/// The player's figure for the effects of §11.2.5: its bone segments and the water at its
/// feet.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerFigure {
    /// The bone segments ([`figure_segments`]).
    pub segments: Vec<(Vec3, Vec3)>,
}

/// The water at the avatar's feet `feet` (`walker.md` §13.5: the walker's own water state,
/// none inside interior cells), as the figure effects read it; `Volume_IsCoin` has no
/// source in the walker and stays unset.
#[must_use]
pub fn figure_water(streamer: &WorldStreamer, feet: &WorldPos) -> FigureWater {
    let view = streamer.collision_view(feet, 1);
    let p = view.to_view([feet.x, feet.y, feet.z]);
    match super::super::movement::CellMaskedWater(&view).water_at(p) {
        Some(w) if w.surface.is_finite() => {
            let r = view.to_region([p[0], p[1], w.surface]);
            #[allow(clippy::cast_possible_truncation)]
            let surface = r[2] as f32;
            FigureWater {
                surface: Some(surface),
                lava: w.flags.lava,
                coin: false,
            }
        }
        _ => FigureWater::default(),
    }
}

impl super::FxState {
    /// The weather work of a world frame before the placement (§11.2.1 step 3, §11.2.2,
    /// §11.2.3 steps 1–2, §11.2.5, §11.2.6): the wind target, the fall direction and, while
    /// the precipitation step runs, the drop counts, the exposure tests that are due for the
    /// updates that run, the drop sets, then the figure effects of the player's `figure`.
    /// `quality` is the factor *q* of `AtmosphericsDetail`. `input` `None` (no environment
    /// yet) places no weather system this frame.
    pub(super) fn weather_prepare(
        &mut self,
        input: Option<&WeatherInput>,
        eye: &WorldPos,
        streamer: &WorldStreamer,
        precipitation_effects: bool,
        quality: f32,
        figure: Option<&PlayerFigure>,
    ) {
        self.weather.figure_spawns.clear();
        let Some(w) = input else {
            self.weather.frame = None;
            self.weather.rain_drops.idle();
            self.weather.snow_drops.idle();
            return;
        };
        let now = self.now;
        self.weather
            .wind
            .set_target(now, w.precipitation.wind_vector());
        let rotation = camera_rotation(w.camera_dir).unwrap_or(Quat::IDENTITY);
        let wind = self.weather.wind.value(now);
        let fall = fall_direction(wind);
        let step = w.landscape_visible && precipitation_effects;
        let (r, sn) = (w.precipitation.rain, w.precipitation.snow);
        // §11.2.6 step 1: an update with no drop does nothing else.
        let gate = UpdateGate::new(
            step,
            r,
            sn,
            quality,
            self.weather.rain.due(now),
            self.weather.snow.due(now),
        );
        let UpdateGate {
            rain_ran,
            snow_ran,
            test_rain: due_rain,
            test_snow: due_snow,
        } = gate;
        if due_rain || due_snow {
            // Both updates test the same positions along the same fall direction.
            let camera = sky_exposed(streamer, eye, fall);
            let player = w
                .avatar
                .as_ref()
                .is_some_and(|p| sky_exposed(streamer, p, fall));
            if due_rain {
                self.weather.rain.record(now, camera, player);
            }
            if due_snow {
                self.weather.snow.record(now, camera, player);
            }
        }
        let camera = self.to_sim([eye.x, eye.y, eye.z]);
        let mut placed = 0usize;
        if rain_ran || snow_ran {
            // §11.2.6 step 5: the reference surface at the camera.
            let reference = self.water_probe(camera).map(|p| p.z);
            let mut world = WalkerDropWorld::new(streamer, eye, self.base);
            // One bound for both sets; the set that goes first alternates by frame.
            let mut budget = MAX_DROP_PLACEMENTS_PER_FRAME;
            let snow_first = self.frame % 2 == 1;
            let super::FxState { weather, rng, .. } = self;
            let mut sets = [
                (&mut weather.rain_drops, r, rain_ran),
                (&mut weather.snow_drops, sn, snow_ran),
            ];
            if snow_first {
                sets.reverse();
            }
            for (set, value, ran) in sets {
                if !ran {
                    set.idle();
                    continue;
                }
                let input = DropInput {
                    now,
                    camera,
                    wind,
                    value,
                    quality,
                    reference,
                };
                if let DropUpdate::Ran { placed: p, .. } =
                    set.update_with_budget(&input, &mut world, rng, &mut budget)
                {
                    placed += p;
                }
            }
        } else {
            self.weather.rain_drops.idle();
            self.weather.snow_drops.idle();
        }
        self.weather.stats.rain_drops = self.weather.rain_drops.count();
        self.weather.stats.snow_drops = self.weather.snow_drops.count();
        self.weather.stats.drops_placed = placed;
        // §11.2.2 step 3, §11.2.5: the player's figure.
        if let (Some(fig), Some(feet)) = (figure, w.avatar.as_ref()) {
            let input = FigureInput {
                rain: r,
                landscape_visible: w.landscape_visible,
                exposed: gate.player_exposed(&self.weather.rain),
                fall,
                water: figure_water(streamer, feet),
            };
            let super::FxState { weather, rng, .. } = self;
            for (a, b) in &fig.segments {
                figure_spawns(*a, *b, &input, rng, &mut weather.figure_spawns);
            }
        }
        self.weather.frame = Some(WeatherFrame {
            camera,
            rotation,
            forward: rotation * Vec3::Y,
            precipitation_step: step,
            rain: r,
            snow: sn,
            fall,
            exposed: gate.camera_exposed(&self.weather.rain, &self.weather.snow),
            inside: w.inside,
            rain_ran,
            snow_ran,
        });
    }

    /// Creates the weather systems whose records have loaded (once each; records are read
    /// on the workers).
    fn weather_systems(&mut self) {
        let now = self.now;
        // The drop materials (§11.2.6): surface records, loaded on the workers; their
        // textures are acquired once they have loaded.
        for did in self.weather.materials.into_iter().flatten() {
            match self.data.cached_record(did) {
                None => self.load(did),
                Some(Some(record)) => self.note_texture(&record),
                Some(None) => {}
            }
        }
        for (i, sys) in WeatherSystem::ALL.into_iter().enumerate() {
            let (Some(None), Some(false), Some(Some(did))) = (
                self.weather.keys.get(i).copied(),
                self.weather.failed.get(i).copied(),
                self.weather.dids.get(i).copied(),
            ) else {
                continue;
            };
            match self.data.cached_record(did) {
                None => self.load(did),
                Some(Some(record)) if matches!(&*record, EffectRecord::Particles { .. }) => {
                    self.note_texture(&record);
                    let node = tessera_formats::script_table::ParticleScriptNode::default();
                    let owner = WEATHER_OWNER_BASE | sys.index() as u64;
                    let key = self
                        .manager
                        .start_attached_own(
                            owner,
                            did,
                            now,
                            || super::make_effect(&record, now, &node),
                            AttachSpec::default(),
                        )
                        .map(|(_, k)| k);
                    if let Some(slot) = self.weather.keys.get_mut(i) {
                        *slot = key;
                    }
                    if key.is_none()
                        && let Some(f) = self.weather.failed.get_mut(i)
                    {
                        *f = true;
                    }
                }
                Some(_) => {
                    tracing::debug!("weather system {sys:?}: record {did:#x} is not a 0x39 system");
                    if let Some(f) = self.weather.failed.get_mut(i) {
                        *f = true;
                    }
                }
            }
        }
    }

    /// The running system of `sys`.
    fn weather_sim(&mut self, sys: WeatherSystem) -> Option<&mut tessera_particles::SystemSim> {
        let key = self.weather.keys.get(sys.index()).copied().flatten()?;
        self.manager.effect_mut(key)?.particles_mut()
    }

    /// One particle of emitter index `emitter` of `sys` at the simulation point `point`
    /// (handed to the system relative to its placement, §11.2.4 step 6); `false` when the
    /// system does not run or skips the spawn.
    fn spawn_weather(&mut self, sys: WeatherSystem, emitter: usize, point: Vec3) -> bool {
        let now = self.now;
        let Some(key) = self.weather.keys.get(sys.index()).copied().flatten() else {
            return false;
        };
        let super::FxState { manager, rng, .. } = self;
        manager
            .effect_mut(key)
            .and_then(|e| e.particles_mut())
            .is_some_and(|sim| sim.spawn_on_point(emitter, now, point, rng))
    }

    /// The spawns of §11.2.3 step 4 from this frame's drops ([`drop_particles`]), each into
    /// its running system (handed to the system relative to its placement, §11.2.4 step 6).
    /// Returns the particles added.
    fn drop_spawns(&mut self, wf: &WeatherFrame) -> usize {
        let now = self.now;
        let super::FxState {
            manager,
            rng,
            weather,
            ..
        } = self;
        let Weather {
            keys,
            rain_drops,
            snow_drops,
            snow_gust_ratio,
            ..
        } = weather;
        drop_particles(
            wf,
            rain_drops,
            snow_drops,
            snow_gust_ratio,
            rng,
            |p, rng| {
                keys.get(p.system.index())
                    .copied()
                    .flatten()
                    .and_then(|k| manager.effect_mut(k))
                    .and_then(|e| e.particles_mut())
                    .is_some_and(|sim| sim.spawn_on_point(p.emitter, now, p.at, rng))
            },
        )
    }

    /// The figure effects of this frame (§11.2.5), each one particle of emitter 1 of its
    /// system. Returns the particles added.
    fn figure_effect_spawns(&mut self) -> usize {
        let spawns = std::mem::take(&mut self.weather.figure_spawns);
        let mut added = 0usize;
        for s in &spawns {
            let (sys, at) = match *s {
                FigureSpawn::Vapour(p) => (WeatherSystem::RainVapour, p),
                FigureSpawn::Bubble(p) => (WeatherSystem::WaterBubbles, p),
                FigureSpawn::Ripple(p) => (WeatherSystem::WaterRipples, p),
                FigureSpawn::CoinRipple(p) => (WeatherSystem::CoinRipples, p),
            };
            added += usize::from(self.spawn_weather(sys, 0, at));
        }
        self.weather.figure_spawns = spawns;
        added
    }

    /// Places `sys` for frame `frame` (§11.2.4 step 6): the camera systems with the camera's
    /// frame, the others at the camera's position without rotation (module docs), with the
    /// light list at the camera (rule 13).
    fn place_weather_system(
        &mut self,
        sys: WeatherSystem,
        wf: &WeatherFrame,
        frame: u64,
        env: &Environment,
        lights: &dyn Fn(Vec3) -> LightList,
    ) -> bool {
        if self
            .weather
            .keys
            .get(sys.index())
            .copied()
            .flatten()
            .is_none()
        {
            return false;
        }
        let owner = WEATHER_OWNER_BASE | sys.index() as u64;
        let world = if sys.at_camera() {
            Mat4::from_rotation_translation(wf.rotation, wf.camera)
        } else {
            Mat4::from_translation(wf.camera)
        };
        self.manager.set_owner_lights(owner, &lights(wf.camera));
        let now = self.now;
        let super::FxState { manager, rng, .. } = self;
        manager.place_owner(
            owner,
            frame,
            now,
            world,
            Vec3::ONE,
            Vec3::ONE,
            1.0,
            &|_| None,
            env,
            rng,
            &NoAttachments,
        );
        true
    }

    /// The weather placement of a world frame (§11.2.2 steps 2 and 4, §11.2.3 step 3,
    /// §11.2.4): the gust forces, the branch's systems, the impact spawn, then the three
    /// water systems. Returns the systems placed.
    pub(super) fn place_weather(
        &mut self,
        frame: u64,
        env: &Environment,
        lights: &dyn Fn(Vec3) -> LightList,
    ) -> usize {
        let Some(wf) = self.weather.frame else {
            return 0;
        };
        self.weather_systems();
        let now = self.now;
        let wind = env.wind;
        let mut placed = 0usize;
        if wf.precipitation_step {
            // §11.2.3 step 3: the rain gust in every rain update, the snow gust only while
            // s > 1.001 (`particles.md` §8.8.1); an update with no drop recomputes nothing
            // (§11.2.6 step 1, P59).
            if wf.rain_gust_forces()
                && let Some(sim) = self.weather_sim(WeatherSystem::RainGust)
            {
                sim.recompute_forces(wind);
            }
            if wf.snow_gust_forces()
                && let Some(sim) = self.weather_sim(WeatherSystem::SnowGust)
            {
                sim.recompute_forces(wind);
            }
            // §11.2.3 step 4: the spawns from the drops.
            self.weather.stats.drop_spawns = self.drop_spawns(&wf);
            let p = plan(wf.rain, wf.snow, wf.exposed, wf.inside);
            for sys in p.systems() {
                placed += usize::from(self.place_weather_system(sys, &wf, frame, env, lights));
            }
            // §11.2.4 step 5: one particle of emitter 1 at the impact system's own origin.
            if let Some(imp) = p.impact()
                && self
                    .weather
                    .keys
                    .get(imp.index())
                    .copied()
                    .flatten()
                    .is_some()
                && impact_spawn(wf.fall, wf.forward, p.strength, &mut self.rng)
            {
                let key = self.weather.keys.get(imp.index()).copied().flatten();
                let super::FxState { manager, rng, .. } = self;
                if let Some(sim) = key
                    .and_then(|k| manager.effect_mut(k))
                    .and_then(|e| e.particles_mut())
                {
                    sim.spawn_on(0, now, now, Vec3::ZERO, rng);
                }
            }
        } else {
            self.weather.stats.drop_spawns = 0;
        }
        // §11.2.2 step 3, §11.2.5: the figure effects of this frame.
        self.weather.stats.figure_spawns = self.figure_effect_spawns();
        // §11.2.2 step 4: the water systems, whatever the weather.
        for sys in [
            WeatherSystem::WaterRipples,
            WeatherSystem::WaterBubbles,
            WeatherSystem::CoinRipples,
        ] {
            placed += usize::from(self.place_weather_system(sys, &wf, frame, env, lights));
        }
        placed
    }
}

impl Weather {
    /// A weather state for the ten DIDs.
    #[must_use]
    pub fn new(dids: [Option<u32>; WEATHER_SYSTEMS]) -> Self {
        Self {
            dids,
            keys: [None; WEATHER_SYSTEMS],
            failed: [false; WEATHER_SYSTEMS],
            wind: Wind::new(),
            rain: Exposure::default(),
            snow: Exposure::default(),
            frame: None,
            materials: [None; 2],
            rain_drops: DropSet::new(DropParams::RAIN),
            snow_drops: DropSet::new(DropParams::SNOW),
            snow_gust_ratio: None,
            tint: [1.0; 4],
            figure_spawns: Vec::new(),
            stats: WeatherStats::default(),
        }
    }

    /// The weather state with the drop materials `materials` (rain, snow; [`read_drop_materials`]).
    #[must_use]
    pub fn with_materials(mut self, materials: [Option<u32>; 2]) -> Self {
        self.materials = materials;
        self
    }

    /// Forgets the running systems and the drops (the layer's manager was replaced, for a
    /// new region: the drops' positions belong to the old frame).
    pub fn forget_systems(&mut self) {
        self.keys = [None; WEATHER_SYSTEMS];
        self.frame = None;
        self.rain_drops.reset();
        self.snow_drops.reset();
        self.figure_spawns.clear();
    }

    /// The drop sets with their material (rain first), for drawing.
    #[must_use]
    pub fn drop_sets(&self) -> [(&DropSet, Option<u32>); 2] {
        [
            (&self.rain_drops, self.materials[0]),
            (&self.snow_drops, self.materials[1]),
        ]
    }

    /// The running systems.
    #[must_use]
    pub fn running(&self) -> usize {
        self.keys.iter().filter(|k| k.is_some()).count()
    }
}

/// The UNIQUEDB map (`particles.md` §7.1.3: root key
/// [`tessera_formats::gameplay::common::UNIQUEDB_ROOT_KEY`], else the entry labelled
/// `UNIQUEDB`). Blocking.
fn uniquedb<S>(
    data: &tessera_props::GameData<S>,
) -> Option<std::sync::Arc<tessera_formats::DataIdMap>>
where
    S: tessera_props::RecordSource,
{
    use tessera_formats::gameplay::common::{UNIQUEDB_LABEL, UNIQUEDB_ROOT_KEY};
    data.root_did_map().ok().and_then(|root| {
        let did = root
            .get(UNIQUEDB_ROOT_KEY)
            .map(|e| e.value)
            .or_else(|| root.by_label(UNIQUEDB_LABEL))?;
        data.did_map(did).ok().flatten()
    })
}

/// The DID of UNIQUEDB key `key`, `None` for a key the map lacks or a null DID.
fn unique_did(map: &tessera_formats::DataIdMap, key: u32) -> Option<u32> {
    map.get(key)
        .map(|e| e.value)
        .filter(|d| !d.is_null())
        .map(|d| d.0)
}

/// The ten DIDs of the UNIQUEDB map; `None` for a key the map lacks or a null DID.
/// Blocking: call it off the frame thread.
#[must_use]
pub fn read_weather_dids<S>(data: &tessera_props::GameData<S>) -> [Option<u32>; WEATHER_SYSTEMS]
where
    S: tessera_props::RecordSource,
{
    let Some(map) = uniquedb(data) else {
        return [None; WEATHER_SYSTEMS];
    };
    WeatherSystem::ALL.map(|s| unique_did(&map, s.key()))
}

/// The surface records of the rain and snow drops (UNIQUEDB keys 0x2B `RainStripMaterial`
/// and 0x3B `Render_SnowMaterial`, `particle-runtime.md` §11.2.6); `None` for a key the map
/// lacks. Blocking: call it off the frame thread.
#[must_use]
pub fn read_drop_materials<S>(data: &tessera_props::GameData<S>) -> [Option<u32>; 2]
where
    S: tessera_props::RecordSource,
{
    let Some(map) = uniquedb(data) else {
        return [None; 2];
    };
    [DropParams::RAIN, DropParams::SNOW].map(|p| unique_did(&map, p.material_key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera_particles::weather::{DropInput, SegmentTest};

    fn frame(rain_ran: bool, snow_ran: bool, snow: f32) -> WeatherFrame {
        WeatherFrame {
            camera: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            forward: Vec3::Y,
            precipitation_step: true,
            rain: 1.0,
            snow,
            fall: Vec3::NEG_Z,
            exposed: false,
            inside: false,
            rain_ran,
            snow_ran,
        }
    }

    /// A drop set whose first `points.len()` drops lie at `points` (with their
    /// reference-surface flags), each close enough to its end point to spawn.
    fn set_with_spawns(params: DropParams, points: &[(Vec3, bool)]) -> DropSet {
        let mut set = DropSet::new(params);
        let mut rng = Ran2::new(1);
        let mut world = |s: Vec3, _: Vec3| SegmentTest::Hit(Vec3::new(s.x, s.y, 0.0));
        #[allow(clippy::cast_precision_loss)]
        let input = DropInput {
            now: 0.0,
            camera: Vec3::ZERO,
            wind: Vec3::ZERO,
            value: (points.len() as f32 + 0.5) / 3_000.0,
            quality: 1.0,
            reference: None,
        };
        let mut budget = usize::MAX;
        for _ in 0..8 {
            set.update_with_budget(&input, &mut world, &mut rng, &mut budget);
        }
        for (d, (p, on_reference)) in set.drops_mut().iter_mut().zip(points) {
            d.end = *p;
            d.lower = *p;
            d.t = 0.001;
            d.dir = Vec3::NEG_Z;
            d.expiry = 100.0;
            d.on_reference = *on_reference;
        }
        // Same clock: no fall, every drop spawns.
        set.update(&input, &mut world, &mut rng);
        assert_eq!(set.spawns().len(), points.len());
        set
    }

    /// `particle-runtime.md` §11.2.6 step 1 and `particles.md` P59: an update with no drop
    /// runs no exposure test, recomputes no force and reports no exposure, even when an
    /// earlier test found the camera and the player exposed.
    #[test]
    fn a_zero_count_update_does_nothing_else() {
        let mut rain = Exposure::default();
        rain.record(0.0, true, true);
        let mut snow = Exposure::default();
        snow.record(0.0, true, true);
        // r = 0.0005 at Medium: ⌊3,000 × 0.0005 × 0.6⌋ = 0.
        let g = UpdateGate::new(true, 0.0005, 0.0, 0.6, true, true);
        assert_eq!(g, UpdateGate::default());
        assert!(!g.camera_exposed(&rain, &snow));
        assert!(!g.player_exposed(&rain));
        let wf = frame(g.rain_ran, g.snow_ran, 1.5);
        assert!(!wf.rain_gust_forces() && !wf.snow_gust_forces());
        // At High the same rain gives one drop: the update runs, tests when due, and
        // reports.
        let g = UpdateGate::new(true, 0.0005, 0.0, 1.0, true, false);
        assert!(g.rain_ran && g.test_rain && !g.snow_ran && !g.test_snow);
        assert!(g.camera_exposed(&rain, &snow) && g.player_exposed(&rain));
        let g = UpdateGate::new(true, 0.0005, 0.0, 1.0, false, false);
        assert!(g.rain_ran && !g.test_rain, "a test runs only when due");
        // No precipitation step: nothing runs.
        assert_eq!(
            UpdateGate::new(false, 1.0, 2.0, 1.0, true, true),
            UpdateGate::default()
        );
        // The snow gust's forces only while s > 1.001.
        assert!(frame(false, true, 1.5).snow_gust_forces());
        assert!(!frame(false, true, 1.0).snow_gust_forces());
        assert!(frame(true, false, 0.0).rain_gust_forces());
    }

    /// §11.2.3 step 4: vapour emitter 2 for every rain spawn point, ripple emitter 2 0.1 m
    /// up for points on the reference surface within 15 m, else a rain gust (emitter 1) for
    /// points off it; never both a ripple and a gust for one point.
    #[test]
    fn drop_spawns_go_to_their_emitters() {
        let mut points = vec![
            (Vec3::new(5.0, 0.0, 2.0), true),  // ripple
            (Vec3::new(20.0, 0.0, 2.0), true), // on the surface, out of reach: vapour only
        ];
        points.extend((0..2_998).map(|k| {
            #[allow(clippy::cast_precision_loss)]
            let x = (k % 50) as f32 * 0.5;
            (Vec3::new(x, 1.0, 0.0), false)
        }));
        let rain = set_with_spawns(DropParams::RAIN, &points);
        let snow = DropSet::new(DropParams::SNOW);
        let mut ratio = None;
        let mut rng = Ran2::new(3);
        let mut seen: Vec<DropParticle> = Vec::new();
        let added = drop_particles(
            &frame(true, false, 0.0),
            &rain,
            &snow,
            &mut ratio,
            &mut rng,
            |p, _| {
                seen.push(p);
                true
            },
        );
        assert_eq!(added, seen.len());
        let of = |s: WeatherSystem| seen.iter().filter(|p| p.system == s).count();
        assert_eq!(of(WeatherSystem::RainVapour), 3_000);
        assert!(
            seen.iter()
                .filter(|p| p.system == WeatherSystem::RainVapour)
                .all(|p| p.emitter == 1)
        );
        let ripples: Vec<_> = seen
            .iter()
            .filter(|p| p.system == WeatherSystem::WaterRipples)
            .collect();
        assert_eq!(ripples.len(), 1);
        assert_eq!(ripples[0].emitter, 1);
        assert!((ripples[0].at - Vec3::new(5.0, 0.0, 2.1)).length() < 1e-5);
        // About 3,000 × 0.015 gusts from the points off the surface, emitter 1 (index 0).
        let gusts: Vec<_> = seen
            .iter()
            .filter(|p| p.system == WeatherSystem::RainGust)
            .collect();
        assert!(!gusts.is_empty() && gusts.len() < 200, "{}", gusts.len());
        assert!(gusts.iter().all(|p| p.emitter == 0 && p.at.y == 1.0));
        assert_eq!(ratio, None, "rain leaves the snow ratio alone");
        // An update that did not run spawns nothing.
        let none = drop_particles(
            &frame(false, false, 0.0),
            &rain,
            &snow,
            &mut ratio,
            &mut rng,
            |_, _| true,
        );
        assert_eq!(none, 0);
    }

    /// §11.2.3 step 4: snow gusts only while s > 1.001, from points off the reference
    /// surface, emitter 1, with the ratio fixed at the first snow gust spawn.
    #[test]
    fn snow_gusts_keep_their_first_ratio() {
        let rain = DropSet::new(DropParams::RAIN);
        let full: Vec<(Vec3, bool)> = (0..3_000)
            .map(|k| (Vec3::new(1.0, 0.0, 0.0), k % 2 == 0))
            .collect();
        let snow = set_with_spawns(DropParams::SNOW, &full);
        let mut ratio = None;
        let mut rng = Ran2::new(5);
        let mut seen: Vec<DropParticle> = Vec::new();
        drop_particles(
            &frame(false, true, 1.0),
            &rain,
            &snow,
            &mut ratio,
            &mut rng,
            |p, _| {
                seen.push(p);
                true
            },
        );
        assert!(seen.is_empty() && ratio.is_none(), "s = 1: no snow gust");
        drop_particles(
            &frame(false, true, 1.5),
            &rain,
            &snow,
            &mut ratio,
            &mut rng,
            |p, _| {
                seen.push(p);
                true
            },
        );
        assert_eq!(ratio, Some(1.0));
        assert!(!seen.is_empty());
        assert!(
            seen.iter()
                .all(|p| p.system == WeatherSystem::SnowGust && p.emitter == 0)
        );
        // A later, smaller set keeps the first ratio.
        let small = set_with_spawns(DropParams::SNOW, &full[..300]);
        drop_particles(
            &frame(false, true, 1.5),
            &rain,
            &small,
            &mut ratio,
            &mut rng,
            |_, _| true,
        );
        assert_eq!(ratio, Some(1.0));
    }

    /// The owner kinds of the effect manager never share an ID: the weather systems have a
    /// range of their own, apart from the creation-stage owners in particular.
    #[test]
    fn owner_ranges_are_pairwise_disjoint() {
        for (i, a) in OWNER_RANGES.iter().enumerate() {
            assert!(a.1 < a.2, "{} is empty", a.0);
            for b in &OWNER_RANGES[i + 1..] {
                assert!(a.2 <= b.1 || b.2 <= a.1, "{} overlaps {}", a.0, b.0);
            }
        }
        let key = tessera_world::LandblockKey::new(1, 0, 0).expect("key");
        let low = super::super::data::static_owner_id(key, 0).0;
        assert!(low >= OWNER_RANGES[4].1, "the smallest static owner ID");
        for sys in WeatherSystem::ALL {
            let id = WEATHER_OWNER_BASE | sys.index() as u64;
            assert!(OWNER_RANGES[3].1 <= id && id < OWNER_RANGES[3].2);
            assert!(!(OWNER_RANGES[2].1..OWNER_RANGES[2].2).contains(&id));
        }
    }

    /// §11.2.3 step 1.
    #[test]
    fn the_fall_direction_follows_the_wind() {
        assert_eq!(fall_direction(Vec3::ZERO), Vec3::NEG_Z);
        let f = fall_direction(Vec3::new(9.8, 0.0, 0.0));
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert!((f - Vec3::new(h, 0.0, -h)).length() < 1e-6, "{f:?}");
        assert_eq!(fall_direction(Vec3::new(0.0, 0.0, 9.8)), Vec3::NEG_Z);
        assert_eq!(fall_direction(Vec3::splat(f32::NAN)), Vec3::NEG_Z);
    }

    /// §11.2.4 steps 1–4: branch, thresholds and strengths.
    #[test]
    fn plans_follow_the_thresholds() {
        use WeatherSystem::*;
        // Rain branch: r > s.
        let p = plan(0.04, 0.0, true, false);
        assert_eq!((p.branch, p.gust, p.camera), (Branch::Rain, false, false));
        let p = plan(0.3, 0.0, true, false);
        assert_eq!(p.systems(), vec![RainGust, RainVapour], "g = 0 below r 0.5");
        let p = plan(0.8, 0.0, true, false);
        assert!((p.strength - 0.6).abs() < 1e-6);
        assert_eq!(
            p.systems(),
            vec![RainGust, RainVapour, RainTorrent, RainImpact]
        );
        assert_eq!(p.impact(), Some(RainImpact));
        assert!(!plan(0.8, 0.0, false, false).camera, "not exposed");
        assert!(!plan(0.8, 0.0, true, true).camera, "inside");
        // Snow branch: r ≤ s, also at r = s = 0.
        let p = plan(0.0, 0.0, true, false);
        assert_eq!(p.branch, Branch::Snow);
        assert!(p.systems().is_empty());
        let p = plan(0.2, 1.0005, true, false);
        assert!(p.systems().is_empty(), "g 0.0005");
        let p = plan(0.2, 1.5, true, false);
        assert!((p.strength - 0.5).abs() < 1e-6);
        assert_eq!(p.systems(), vec![SnowGust, SnowBlizzard, SnowImpact]);
        assert_eq!(plan(0.2, 1.5, false, false).systems(), vec![SnowGust]);
        assert_eq!(plan(f32::NAN, f32::NAN, true, false).branch, Branch::Snow);
    }

    /// §11.2.4 step 5: the bound of the impact draw and its distribution.
    #[test]
    fn impact_spawns_grow_when_looking_up() {
        assert_eq!(impact_bound(0.05, 1.0), None);
        assert_eq!(impact_bound(0.9, 1.0), Some(10));
        assert_eq!(impact_bound(0.45, 0.6), Some(1)); // 0.25 × 6 = 1.5
        assert_eq!(impact_bound(f32::NAN, 1.0), None);
        let mut rng = Ran2::new(5);
        let n = 20_000;
        // Looking straight up into vertical rain: d = 1, bound ⌊(1/0.9)² × 10⌋ = 12, so 13
        // of the 21 values spawn.
        let up = (0..n)
            .filter(|_| impact_spawn(Vec3::NEG_Z, Vec3::Z, 1.0, &mut rng))
            .count();
        let p = up as f64 / f64::from(n);
        assert!((p - 13.0 / 21.0).abs() < 0.02, "{p}");
        // Looking down or level: no draw.
        assert!(!(0..100).any(|_| impact_spawn(Vec3::NEG_Z, Vec3::X, 1.0, &mut rng)));
        assert!(!(0..100).any(|_| impact_spawn(Vec3::NEG_Z, Vec3::NEG_Z, 1.0, &mut rng)));
    }

    /// §11.2.3 step 2: a test every 0.5 s.
    #[test]
    fn exposure_is_tested_every_half_second() {
        let mut e = Exposure::default();
        assert!(e.due(0.0));
        e.record(1.0, true, false);
        assert!(!e.due(1.5));
        assert!(e.due(1.51));
        assert!(e.due(f64::NAN), "a clock that is not finite tests again");
        assert!(e.camera && !e.player);
    }

    #[test]
    fn camera_rotation_maps_y_to_forward() {
        let q = camera_rotation(Vec3::new(1.0, 1.0, 0.0)).expect("rotation");
        let y = q * Vec3::Y;
        assert!((y - Vec3::new(1.0, 1.0, 0.0).normalize()).length() < 1e-5);
        assert!((q * Vec3::Z - Vec3::Z).length() < 1e-5);
        let up = camera_rotation(Vec3::Z).expect("vertical");
        assert!((up * Vec3::Y - Vec3::Z).length() < 1e-5);
        assert!(camera_rotation(Vec3::ZERO).is_none());
    }

    /// §11.2.5, rule 17: every bone with a parent bone gives a segment, through the
    /// figure's position, rotation and scale.
    #[test]
    fn figure_segments_follow_the_pose() {
        use tessera_havok::Transform;
        let skeleton = tessera_anim::Skeleton::from_parts(&[-1, 0, 1], &[Transform::IDENTITY; 3])
            .expect("skeleton");
        let view = super::super::super::characters::FigureEffectView {
            region: 1,
            position: [110.0, 220.0, 5.0],
            rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            scale: Vec3::splat(2.0),
            drawn: true,
            model: vec![
                Mat4::IDENTITY,
                Mat4::from_translation(Vec3::new(0.0, 0.0, 1.0)),
                Mat4::from_translation(Vec3::new(1.0, 0.0, 1.0)),
            ],
            points: None,
            skeleton: Some(std::sync::Arc::new(skeleton)),
            scripts: None,
            opacity: 1.0,
        };
        let base = [100.0, 200.0];
        let seg = figure_segments(&view, |p| {
            Vec3::new(
                (p[0] - base[0]) as f32,
                (p[1] - base[1]) as f32,
                p[2] as f32,
            )
        });
        assert_eq!(seg.len(), 2, "the root has no parent");
        let near = |a: Vec3, b: Vec3| (a - b).length() < 1e-4;
        assert!(near(seg[0].0, Vec3::new(10.0, 20.0, 5.0)));
        assert!(near(seg[0].1, Vec3::new(10.0, 20.0, 7.0)));
        // The third bone 1 m along local +x, scaled by 2, turned a quarter turn about +z.
        assert!(near(seg[1].1, Vec3::new(10.0, 22.0, 7.0)), "{:?}", seg[1]);
        let none = super::super::super::characters::FigureEffectView {
            skeleton: None,
            ..view
        };
        assert!(figure_segments(&none, |_| Vec3::ZERO).is_empty());
    }

    #[test]
    fn systems_have_their_keys() {
        let keys: Vec<u32> = WeatherSystem::ALL.iter().map(|s| s.key()).collect();
        assert_eq!(
            keys,
            vec![0x7B, 0x2A, 0x6C, 0x83, 0xFD, 0x100, 0xFE, 0x4A, 0xA1, 0x1E7]
        );
        for (i, s) in WeatherSystem::ALL.iter().enumerate() {
            assert_eq!(s.index(), i);
        }
        assert_eq!(
            WeatherSystem::ALL.iter().filter(|s| s.at_camera()).count(),
            4
        );
    }
}
