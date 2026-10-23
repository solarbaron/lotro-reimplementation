// SPDX-License-Identifier: GPL-3.0-or-later
//! Glue between the client runtime and the world walker: walker settings and the sandbox
//! configuration from the command line, background world start-up once the archives are
//! open, screenshots.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context as _;
use tessera_net::sandbox::world::{SpawnPoint, StaticConfig};
use tessera_net::sandbox::{CrowdConfig, MAX_RESOLVE_TIMEOUT, MAX_SPEED, SandboxConfig};
use tessera_net::{RegionId, Transform, Vec3};
use tessera_render::{OffscreenFormat, Renderer, RgbaImage};
use tessera_world::coords::MAX_BLOCK;
use tessera_world::frills::FrillSettings;
use tessera_world::{BundleOptions, DistanceSettings, LANDBLOCK_SIZE, RingConfig, StreamConfig};

use crate::avatar::AvatarSpec;
use crate::cli::{Args, RenderPref, render_pref};
use crate::runtime::{DataStatus, Runtime};
use crate::walker::boot::WorldBoot;
use crate::walker::characters::CharacterConfig;
use crate::walker::characters::catalog::CharacterCatalog;
use crate::walker::controls::MoveMode;
use crate::walker::env::ClockConfig;
use crate::walker::fxparse::{System, Systems};
use crate::walker::objects::ObjectConfig;
use crate::walker::source::SourceShared;
use crate::walker::{CameraPlan, FlightPlan, Walker, WalkerConfig};

/// Where the walker starts without `--spawn` or `--place`: region 1, the centre of
/// landblock (126, 140), the block of `coordinates.md` §8.4 B. The centre is at map
/// `12.0S, 45.6W`; the example's own point `12.3S, 45.6W` is block-local (80, 20) of it.
pub const DEFAULT_SPAWN: &str = "1,126,140";
/// Landblocks around the player whose figures (NPCs, weenie placements) the sandbox
/// serves (Chebyshev radius): 5 × 5 blocks, within the walker's near ring (3).
pub const FIGURE_RADIUS: u8 = 2;
/// How long the walker's sandbox waits for a spawn or teleport lookup (terrain height,
/// place name) before giving up; also the resolve timeout of a scenario run with an
/// install (`scenario-feed.md` §3.2).
pub const WALKER_RESOLVE_TIMEOUT: Duration = Duration::from_secs(300);

/// The spawn point the command line asks for.
///
/// # Errors
///
/// When the spawn text or place name is invalid (clap already checked `--spawn`).
pub fn spawn_point(args: &Args) -> anyhow::Result<SpawnPoint> {
    if let Some(path) = &args.fly_path {
        let region = path.region.unwrap_or(args.region);
        let g = path.from;
        let bx = (g[0] / LANDBLOCK_SIZE)
            .floor()
            .clamp(0.0, f64::from(MAX_BLOCK));
        let by = (g[1] / LANDBLOCK_SIZE)
            .floor()
            .clamp(0.0, f64::from(MAX_BLOCK));
        return SpawnPoint::landblock(
            RegionId(u16::from(region)),
            bx as u8,
            by as u8,
            [g[0] - bx * LANDBLOCK_SIZE, g[1] - by * LANDBLOCK_SIZE, 0.0],
        )
        .context("fly path start");
    }
    if let Some(place) = &args.place {
        return SpawnPoint::named(place.clone()).context("--place");
    }
    if let (Some(l), None) = (&args.camera_loop, &args.spawn) {
        let p = l.points.first().copied().unwrap_or([80.0, 80.0]);
        return SpawnPoint::landblock(
            RegionId(u16::from(l.region)),
            l.x,
            l.y,
            [p[0].clamp(0.0, 160.0), p[1].clamp(0.0, 160.0), 0.0],
        )
        .context("camera loop start");
    }
    let text = args.spawn.as_deref().unwrap_or(DEFAULT_SPAWN);
    SpawnPoint::parse(text).with_context(|| format!("--spawn {text}"))
}

/// The sandbox configuration of a walker session.
///
/// # Errors
///
/// As [`spawn_point`].
pub fn sandbox_config(args: &Args) -> anyhow::Result<SandboxConfig> {
    let mut spawn_point = spawn_point(args)?;
    // The walker's `--heading` is the sandbox's too (the crowd stands in front of it).
    let heading = args
        .heading
        .map(tessera_net::model::heading_from_compass_degrees)
        .unwrap_or(0.0);
    spawn_point = spawn_point.with_heading(heading);
    let region = match &spawn_point.destination {
        tessera_net::Destination::Location { location, .. }
        | tessera_net::Destination::Ground { location, .. }
        | tessera_net::Destination::Anchored { location, .. } => location.region,
        _ => RegionId(u16::from(args.region)),
    };
    // Fallback when a name cannot be resolved: the default spawn block's centre.
    let fallback = SpawnPoint::parse(DEFAULT_SPAWN).ok();
    let spawn = match fallback.map(|f| f.destination) {
        Some(tessera_net::Destination::Ground { location, .. }) if location.region == region => {
            Transform::new(
                Vec3::new(location.x as f32, location.y as f32, 0.0),
                heading,
            )
        }
        _ => Transform::new(Vec3::new(20_000.0, 20_000.0, 0.0), heading),
    };
    Ok(SandboxConfig {
        region,
        spawn,
        spawn_point: Some(spawn_point),
        resolve_timeout: MAX_RESOLVE_TIMEOUT.min(WALKER_RESOLVE_TIMEOUT),
        statics: StaticConfig {
            radius: FIGURE_RADIUS,
            ..StaticConfig::default()
        },
        wanderers: 0,
        max_player_speed: MAX_SPEED,
        // Appearances come from the data (`Runtime::start_walker_session`).
        crowd: CrowdConfig {
            count: args.crowd,
            spacing: args.crowd_spacing,
            ahead: args.crowd_ahead,
            ..CrowdConfig::default()
        },
        ..SandboxConfig::default()
    })
}

/// The distance preferences of the command line over the built-in defaults
/// (`walker.md` §11.2, §11.8): the dedicated options first, then `--render-pref` for
/// `LandscapeDrawDistance`, `ObjectDrawDistance` and `ModelDetail` (§14.10; a value the key
/// does not accept counts as its built-in level, W26), then the built-in levels. That the
/// dedicated option wins over a `--render-pref` of the same key is a **Tessera choice**
/// (§14.10 only says the override wins over the file and the built-in values).
#[must_use]
pub fn distance_settings(args: &Args) -> DistanceSettings {
    use tessera_world::DetailLevel;
    use tessera_world::draw_distance::LandscapeLevel;
    let d = DistanceSettings::default();
    let pref = |key: &str| render_pref(&args.render_prefs, key);
    let detail = |v: &str| {
        DetailLevel::from_name(v)
            .or_else(|| v.trim().parse().ok().and_then(DetailLevel::from_stored))
    };
    DistanceSettings {
        landscape: args.landscape_distance.unwrap_or_else(|| {
            pref("LandscapeDrawDistance")
                .and_then(|v| {
                    LandscapeLevel::from_name(v)
                        .or_else(|| v.trim().parse().ok().and_then(LandscapeLevel::from_stored))
                })
                .unwrap_or(d.landscape)
        }),
        object_draw_distance: args.object_draw_distance.unwrap_or_else(|| {
            pref("ObjectDrawDistance")
                .and_then(detail)
                .unwrap_or(d.object_draw_distance)
        }),
        model_detail: args.model_detail.unwrap_or_else(|| {
            pref("ModelDetail")
                .and_then(detail)
                .unwrap_or(d.model_detail)
        }),
        ..d
    }
}

/// The `FrillDistance` level (`scenery.md` §8.3: 0–5): `--frill-distance`, else a
/// `--render-pref FrillDistance=VALUE` ([`frill_level`]: a level name of `walker.md` §11.2
/// or a number 0–5; any other value counts as the built-in level, W26), else the built-in
/// level. **Tessera choice** (no spec states it): the dedicated option wins over a
/// `--render-pref` of the same key, as for the other distance keys ([`distance_settings`]).
#[must_use]
pub fn frill_distance_level(args: &Args) -> i32 {
    let builtin = FrillSettings::default().distance_level;
    args.frill_distance.map_or_else(
        || {
            render_pref(&args.render_prefs, "FrillDistance")
                .and_then(frill_level)
                .unwrap_or(builtin)
        },
        i32::from,
    )
}

/// The level of a `FrillDistance` value (`walker.md` §11.2: 0 `Disabled`, 1 `Low`,
/// 2 `Medium`, 3 `High`, 4 `VeryHigh`, 5 `UltraHigh`): the name compared case-insensitively
/// (W26) or a number 0–5, surrounding white space ignored; `None` for anything else.
#[must_use]
pub fn frill_level(stored: &str) -> Option<i32> {
    const NAMES: [&str; 6] = ["disabled", "low", "medium", "high", "veryhigh", "ultrahigh"];
    let v = stored.trim();
    let lower = v.to_ascii_lowercase();
    NAMES
        .iter()
        .position(|n| *n == lower)
        .and_then(|i| i32::try_from(i).ok())
        .or_else(|| v.parse::<i32>().ok().filter(|n| (0..=5).contains(n)))
}

/// The level of a stored `[Render] LandscapeLightingQuality` value (`far-view.md` FD18,
/// FV29): `Low` 0, `High` 1, `VeryHigh` 2, names compared case-insensitively (`walker.md`
/// W26), or a number 0–2, surrounding white space ignored. Every other value, the names
/// of other options' scales (`VeryLow`, `Medium`, `UltraHigh`, `Disabled`, …) and other
/// numbers included, counts as the built-in level 0 (no seam overlay).
#[must_use]
pub fn landscape_lighting_level(stored: &str) -> u32 {
    let v = stored.trim();
    match v.to_ascii_lowercase().as_str() {
        "low" => 0,
        "high" => 1,
        "veryhigh" => 2,
        _ => v.parse::<u32>().ok().filter(|n| *n <= 2).unwrap_or(0),
    }
}

/// The built-in `[Render] PrecipitationEffects` of the effects (`particle-runtime.md`
/// §11.2.2): on.
const BUILTIN_PRECIPITATION: bool = true;
/// The built-in `[Render] VideoPostEffects` of the effects (`particle-runtime.md` rule 16):
/// on.
const BUILTIN_VIDEO_POST: bool = true;

/// The `[Render]` keys the walker reads, by consumer (`walker.md` §14.10): the distance
/// settings (§11.2, through [`distance_settings`] and [`frill_distance_level`]), the far
/// view (`far-view.md` FD17) and the particle effects (`particle-runtime.md` §11.2.2,
/// §11.2.6 step 1, rule 16).
pub const RENDER_PREF_KEYS: [&str; 11] = [
    "LandscapeDrawDistance",
    "ObjectDrawDistance",
    "ModelDetail",
    "FrillDistance",
    "FarLandscapeNormalMaps",
    "MultiPassLighting",
    "LandscapeLightingQuality",
    "DistantImposters",
    "PrecipitationEffects",
    "VideoPostEffects",
    "AtmosphericsDetail",
];

/// The `[Render]` keys the walker fixes itself and never takes from a preference
/// (`far-view.md` FD16, FD17, FD22).
pub const FIXED_RENDER_PREF_KEYS: [&str; 2] = ["GraphicsCore", "NearFarSeamBlending"];

/// A `--render-pref` key the walker ignores ([`ignored_render_prefs`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoredRenderPref {
    /// The key as first given.
    pub key: String,
    /// One of [`FIXED_RENDER_PREF_KEYS`] (else no consumer reads it).
    pub fixed: bool,
}

/// The keys of `prefs` that the walker ignores, each once (case-insensitive, in order of
/// first use): keys it fixes itself and keys no consumer reads (`walker.md` §14.10: "an
/// unknown key is reported once and ignored").
#[must_use]
pub fn ignored_render_prefs(prefs: &[RenderPref]) -> Vec<IgnoredRenderPref> {
    let mut out: Vec<IgnoredRenderPref> = Vec::new();
    for p in prefs {
        let known = |list: &[&str]| list.iter().any(|k| k.eq_ignore_ascii_case(&p.key));
        if known(&RENDER_PREF_KEYS) || out.iter().any(|o| o.key.eq_ignore_ascii_case(&p.key)) {
            continue;
        }
        out.push(IgnoredRenderPref {
            key: p.key.clone(),
            fixed: known(&FIXED_RENDER_PREF_KEYS),
        });
    }
    out
}

/// Applies the `--render-pref` overrides (`walker.md` §14.10, `far-view.md` FD22) to the
/// far-view settings and the effects' settings, after the user's file (windowed) or on the
/// built-in values (headless): `FarLandscapeNormalMaps`, `MultiPassLighting`,
/// `LandscapeLightingQuality` and `DistantImposters` (with the value forms of
/// [`tessera_world::far_view::settings::preference_bool`] and FD18's level table,
/// [`landscape_lighting_level`]), `PrecipitationEffects`,
/// `VideoPostEffects` and `AtmosphericsDetail` ([`atmospherics_level`]). A key given with a
/// value it does not accept takes its built-in value (W26), whatever the file said; a key
/// not given keeps the current value. The graphics core and the seam switch are never
/// touched (FD16, FD17).
pub fn apply_render_overrides(
    far: &mut tessera_world::far_view::FarViewSettings,
    effects: &mut crate::walker::effects::EffectConfig,
    prefs: &[RenderPref],
) {
    use tessera_world::far_view::settings::preference_bool;
    if prefs.is_empty() {
        return;
    }
    let get = |key: &str| render_pref(prefs, key);
    let builtin = tessera_world::far_view::FarViewSettings::default();
    let flag = |key: &str, current: &mut bool, builtin: bool| {
        if let Some(v) = get(key) {
            *current = preference_bool(v).unwrap_or(builtin);
        }
    };
    flag(
        "FarLandscapeNormalMaps",
        &mut far.normal_maps,
        builtin.normal_maps,
    );
    flag(
        "MultiPassLighting",
        &mut far.multi_pass_lighting,
        builtin.multi_pass_lighting,
    );
    flag("DistantImposters", &mut far.imposters, builtin.imposters);
    if let Some(v) = get("LandscapeLightingQuality") {
        // FD18: an unaccepted value is level 0, the built-in level (FV28, FV29).
        far.lighting_quality = landscape_lighting_level(v);
    }
    flag(
        "PrecipitationEffects",
        &mut effects.precipitation,
        BUILTIN_PRECIPITATION,
    );
    flag(
        "VideoPostEffects",
        &mut effects.video_post,
        BUILTIN_VIDEO_POST,
    );
    if let Some(v) = get("AtmosphericsDetail") {
        effects.atmospherics = atmospherics_level(v);
    }
}

/// The M4.5 systems the command line leaves on (`--disable`).
#[must_use]
pub fn systems(args: &Args) -> Systems {
    let mut s = Systems::all();
    for x in args.disable.iter().flat_map(|d| d.0.iter()) {
        s.set(*x, false);
    }
    s
}

/// Tessera's `[Graphics]` settings from the user's settings file (never fails; problems are
/// logged). Reading may copy a damaged file to a backup next to it
/// (`tessera_settings::SettingsStore::load`); it never writes the game's files.
#[must_use]
pub fn graphics_prefs() -> tessera_settings::GraphicsPrefs {
    let Ok(store) = tessera_settings::SettingsStore::open_default() else {
        return tessera_settings::GraphicsPrefs::default();
    };
    let loaded = store.load();
    let (p, warnings) = tessera_settings::GraphicsPrefs::from_settings(&loaded.settings);
    for w in &warnings {
        tracing::warn!("settings: {w}");
    }
    p
}

/// Applies the `[Render]` section of the user's `UserPreferences.ini` (`install-layout.md`
/// §8.3, read case-insensitively by [`tessera_install::Ini::get`]) to the far-view
/// settings: `FarLandscapeNormalMaps`, `MultiPassLighting`, `LandscapeLightingQuality` and
/// `DistantImposters` (`far-view.md` FD17, FD18, through
/// [`tessera_world::far_view::FarViewSettings::apply_render_preferences`]). The graphics
/// core stays 5 and `NearFarSeamBlending` on whatever the file says (FD16, FD17, FD21), so
/// the file decides between the plain and the normal-mapped template and, with the
/// lighting inputs, whether the seam overlay is drawn.
pub fn apply_render_preferences(
    settings: &mut tessera_world::far_view::FarViewSettings,
    prefs: &tessera_install::Ini,
) {
    // `LandscapeLightingQuality` goes through FD18's table ([`landscape_lighting_level`]):
    // the world reader also takes other scales' names and numbers above 2 as levels.
    settings.apply_render_preferences(|key| {
        let v = prefs.get("Render", key)?;
        if key.eq_ignore_ascii_case("LandscapeLightingQuality") {
            Some(match landscape_lighting_level(v) {
                0 => "0",
                1 => "1",
                _ => "2",
            })
        } else {
            Some(v)
        }
    });
}

/// Applies the `[Render]` options of the user's `UserPreferences.ini` that the particle
/// effects read: `PrecipitationEffects` (`particle-runtime.md` §11.2.2, built-in on),
/// `VideoPostEffects` (rule 16: off skips the systems drawn after post-processing; built-in
/// on) and `AtmosphericsDetail` (§11.2.6 step 1, [`atmospherics_level`]: the drop counts).
/// Unknown or missing flag values keep the current ones; a present `AtmosphericsDetail` the
/// reader does not accept counts as the built-in `Medium` (`walker.md` W26). Windowed runs
/// only, like the far view's (`far-view.md` FD21).
pub fn apply_effect_preferences(
    effects: &mut crate::walker::effects::EffectConfig,
    prefs: &tessera_install::Ini,
) {
    let flag = |key: &str| {
        prefs
            .get("Render", key)
            .and_then(tessera_world::far_view::settings::preference_bool)
    };
    if let Some(b) = flag("PrecipitationEffects") {
        effects.precipitation = b;
    }
    if let Some(b) = flag("VideoPostEffects") {
        effects.video_post = b;
    }
    if let Some(v) = prefs.get("Render", "AtmosphericsDetail") {
        effects.atmospherics = atmospherics_level(v);
    }
}

/// The level of a stored `[Render] AtmosphericsDetail` value (`particle-runtime.md` §11.2.6
/// step 1: `Low` 0, `Medium` 1, `High` 2): the level name compared case-insensitively, or a
/// level number (0 or more), surrounding white space ignored. `None` for anything else,
/// which the drop counts read as the built-in `Medium` (`walker.md` W26, Tessera Decision:
/// the client's parsing of the stored value was not traced).
#[must_use]
pub fn atmospherics_level(stored: &str) -> Option<i64> {
    let v = stored.trim();
    match v.to_ascii_lowercase().as_str() {
        "low" => Some(0),
        "medium" => Some(1),
        "high" => Some(2),
        _ => v.parse::<i64>().ok().filter(|n| *n >= 0),
    }
}

/// The user's `UserPreferences.ini` next to the install `root` (`install-layout.md` §8.1,
/// §8.3), read only (capped at [`tessera_install::MAX_INI_SIZE`]); `None` when there is
/// none or it cannot be read (logged). Blocking: call it off the frame thread.
#[must_use]
pub fn read_user_preferences(root: &Path) -> Option<tessera_install::Ini> {
    let path = crate::audio::backend::find_preferences(root)?;
    match tessera_install::Ini::read(&path) {
        Ok(ini) => Some(ini),
        Err(e) => {
            tracing::warn!("preferences {}: {e}", path.display());
            None
        }
    }
}

/// Walker settings from the command line (and the `[Graphics]` settings), with the
/// `--render-pref` overrides applied over the built-in values ([`apply_render_overrides`];
/// a windowed run applies them again after the user's file, [`WalkerHost`]).
#[must_use]
pub fn walker_config(args: &Args) -> WalkerConfig {
    let mut config = walker_config_base(args);
    apply_render_overrides(
        &mut config.far_view.settings,
        &mut config.effects,
        &args.render_prefs,
    );
    config
}

/// [`walker_config`] before the `--render-pref` overrides of the far view and the effects.
fn walker_config_base(args: &Args) -> WalkerConfig {
    let distances = distance_settings(args);
    // The far ring is the landscape extent E (`walker.md` §11.3, §11.8) unless
    // `--view-blocks` sets it. With the far view on, full-detail terrain covers the
    // original's 3 × 3 near field (`far-view.md` §2, §3.2, Tessera Decision FD9) and the
    // sectors draw the rest out to E; `--view-blocks` keeps a larger full-detail ring.
    let far = match (args.view_blocks, args.far_view) {
        (Some(b), _) => f64::from(b) * LANDBLOCK_SIZE,
        (None, true) => crate::walker::farview::NEAR_FIELD_RING,
        (None, false) => f64::from(distances.landscape_extent()),
    };
    let rings = RingConfig {
        near: (3.0 * LANDBLOCK_SIZE).min(far),
        mid: (6.0 * LANDBLOCK_SIZE).min(far),
        far,
        hysteresis: LANDBLOCK_SIZE,
    };
    let flight = args.fly_path.map(|path| FlightPlan {
        path,
        region: path.region.unwrap_or(args.region),
        speed: args.speed,
        height: args.fly_height,
        settle_timeout: Duration::from_secs(args.settle_timeout),
    });
    WalkerConfig {
        mode: if args.fly
            || flight.is_some()
            || args.camera_loop.is_some()
            || args.turn_rate.is_some()
        {
            MoveMode::Fly
        } else {
            MoveMode::Walk
        },
        fly_speed: args.fly_speed,
        region: args.region,
        stream: StreamConfig {
            rings,
            residency_budget: usize::try_from(args.residency_mib << 20).unwrap_or(usize::MAX),
            frills: FrillSettings {
                distance_level: frill_distance_level(args),
                draw_frills: systems(args).on(System::Frills),
                ..FrillSettings::default()
            },
            ..StreamConfig::default()
        },
        objects: ObjectConfig {
            distance: args.object_distance.unwrap_or(far),
            gpu_budget: args.gpu_mib << 20,
            texture_cap: args.texture_cap,
            ..ObjectConfig::default()
        },
        interiors: args.interiors,
        keymap: args.keymap.clone(),
        flight,
        frame_log: args.frame_log.is_some() || args.fly_path.is_some() || args.duration.is_some(),
        heading: args
            .heading
            .map(tessera_net::model::heading_from_compass_degrees),
        pitch: args.pitch.to_radians(),
        eye_height: args.eye_height,
        characters: CharacterConfig {
            markers: args.markers,
            anim_distance: args.anim_distance,
            draw_distance: args
                .character_distance
                .unwrap_or(CharacterConfig::default().draw_distance),
            updated_avatars: graphics_prefs().avatar_update_visible,
            face_compositing: graphics_prefs().avatar_face_compositing,
            ..CharacterConfig::default()
        },
        autorun: args.autorun.as_deref().map(|g| g == "walk"),
        bench: args.duration.map(|secs| crate::walker::BenchPlan {
            duration: Duration::from_secs(secs),
            settle_timeout: Duration::from_secs(args.settle_timeout),
        }),
        audio: crate::audio::config_from_args(args),
        effects: crate::walker::effects::EffectConfig {
            enabled: !args.no_particles,
            interiors: args.interiors,
            seed: args.particle_seed.unwrap_or_else(|| {
                if args.walker_headless() {
                    1
                } else {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(1, |d| d.as_secs() as u32)
                }
            }),
            // Built-in values; windowed runs take the user's `[Render]` values
            // ([`apply_effect_preferences`]).
            precipitation: BUILTIN_PRECIPITATION,
            video_post: BUILTIN_VIDEO_POST,
            atmospherics: None,
        },
        systems: systems(args),
        clock: ClockConfig {
            day: args.day.unwrap_or(crate::walker::env::DEFAULT_DAY),
            time_of_day: args
                .time_of_day
                .unwrap_or(crate::walker::env::DEFAULT_TIME_OF_DAY),
            rate: args.time_rate.unwrap_or(1.0),
            day_type: args
                .day_type
                .unwrap_or(tessera_world::environment::INITIAL_DAY_TYPE),
        },
        distances,
        // The collision mover takes the avatar's float height (`walker.md` §13.2).
        avatar: args.avatar.clone().unwrap_or_default(),
        far_band: args.far_band,
        far_view: crate::walker::farview::FarViewConfig {
            enabled: args.far_view,
            near_field: if args.view_blocks.is_some() {
                crate::walker::farview::NearFieldMode::Ring
            } else {
                crate::walker::farview::NearFieldMode::Original
            },
            settings: tessera_world::far_view::FarViewSettings {
                far_radius: distances.landscape.groups(),
                ..tessera_world::far_view::FarViewSettings::default()
            },
            ..crate::walker::farview::FarViewConfig::default()
        },
        hud: !args.no_hud,
        // `--mount SPEC` (P5, `walker.md` §16.2).
        mount: args.mount,
        camera: match (&args.camera_loop, args.turn_rate) {
            (Some(path), _) => Some(CameraPlan::Loop {
                path: path.clone(),
                speed: args.loop_speed,
                height: args.loop_height,
                pitch: args.loop_pitch.to_radians(),
            }),
            (None, Some(rate)) => Some(CameraPlan::Turn {
                rate: rate.to_radians(),
            }),
            (None, None) => None,
        },
        ..WalkerConfig::default()
    }
}

/// The bundle options of a walker: the content layers of `--content-layers`.
#[must_use]
pub fn bundle_options(args: &Args) -> BundleOptions {
    BundleOptions {
        active_content_layers: args.content_layers.clone(),
        ..BundleOptions::default()
    }
}

/// The avatar's appearance: `--avatar` resolved against the catalog, else the default
/// body, else (a catalog without bodies) none.
#[must_use]
pub fn avatar_appearance(
    spec: &AvatarSpec,
    catalog: &CharacterCatalog,
) -> Option<tessera_net::Appearance> {
    match catalog.appearance(spec) {
        Ok(a) => Some(a),
        Err(e) => {
            tracing::warn!("--avatar {spec}: {e}; using the default body");
            catalog.appearance(&AvatarSpec::default()).ok().or_else(|| {
                catalog.bodies.first().map(|b| {
                    tessera_net::Appearance::new(tessera_net::Body::Character {
                        species: b.species,
                        sex: b.sex,
                    })
                })
            })
        }
    }
}

/// The walker's life cycle inside the client.
#[derive(Debug)]
pub struct WalkerHost {
    /// State shared with the sandbox source.
    pub shared: Arc<Mutex<SourceShared>>,
    config: WalkerConfig,
    avatar: AvatarSpec,
    bundles: BundleOptions,
    boot: Option<WorldBoot>,
    started: bool,
    /// The walker, once the world is loaded.
    pub walker: Option<Walker>,
    /// Why the walker could not start.
    pub failed: Option<String>,
    /// Whether the user's `UserPreferences.ini` may be read (windowed runs only: headless
    /// and test runs keep the built-in settings, like the audio import).
    user_prefs: bool,
    /// The user's preferences, read on a thread of their own while the world starts up.
    prefs: PendingPreferences,
    /// The `--render-pref` overrides, applied again after the user's file (`walker.md`
    /// §14.10: they win over it).
    render_prefs: Vec<RenderPref>,
}

/// Longest the walker's creation waits for the user's preferences after the world
/// start-up has finished, from the start of the read (Tessera choice; the wait is spread
/// over frames, the frame thread never blocks on it).
pub const PREFERENCES_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// The user's preferences being read on a thread of their own ([`WalkerHost`]).
#[derive(Debug, Default)]
struct PendingPreferences {
    rx: Option<std::sync::mpsc::Receiver<Option<tessera_install::Ini>>>,
    /// When the wait for them ends.
    deadline: Option<std::time::Instant>,
    /// The preferences read (`None`: none, or not read in time).
    ini: Option<tessera_install::Ini>,
}

impl PendingPreferences {
    /// Waits for the reader of `rx` until `deadline`.
    fn new(
        rx: std::sync::mpsc::Receiver<Option<tessera_install::Ini>>,
        deadline: std::time::Instant,
    ) -> Self {
        Self {
            rx: Some(rx),
            deadline: Some(deadline),
            ini: None,
        }
    }

    /// Whether the read is over at `now` (the reader sent, it is gone, or the deadline
    /// passed; then the preferences are kept, or given up with a warning). Never blocks.
    fn settled(&mut self, now: std::time::Instant) -> bool {
        let Some(rx) = &self.rx else {
            return true;
        };
        match rx.try_recv() {
            Ok(ini) => self.ini = ini,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {}
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                if self.deadline.is_some_and(|d| now < d) {
                    return false;
                }
                tracing::warn!(
                    "the user's preferences were not read in time; built-in far-view settings"
                );
            }
        }
        self.rx = None;
        true
    }
}

impl WalkerHost {
    /// A host for `args` (nothing starts until the archives are open).
    #[must_use]
    pub fn new(args: &Args, shared: Arc<Mutex<SourceShared>>) -> Self {
        for p in ignored_render_prefs(&args.render_prefs) {
            if p.fixed {
                tracing::warn!(
                    key = %p.key,
                    "--render-pref: the walker fixes this key itself; ignored"
                );
            } else {
                tracing::warn!(key = %p.key, "--render-pref: no walker setting reads this key; ignored");
            }
        }
        Self {
            shared,
            config: walker_config(args),
            avatar: args.avatar.clone().unwrap_or_default(),
            bundles: bundle_options(args),
            boot: None,
            started: false,
            walker: None,
            failed: None,
            user_prefs: !args.walker_headless(),
            prefs: PendingPreferences::default(),
            render_prefs: args.render_prefs.clone(),
        }
    }

    /// Whether the user's preferences are read: the run may read them (windowed only,
    /// `far-view.md` FD21) and something reads their `[Render]` section, the far view
    /// (`far-view.md` §6.1, §6.5, §9.1) or the particle effects (`particle-runtime.md`
    /// §11.2.2 and rule 16).
    fn wants_preferences(&self) -> bool {
        self.user_prefs && (self.config.far_view.enabled || self.config.effects.enabled)
    }

    /// Starts reading the user's preferences off the frame thread, for the far view and the
    /// particle effects, when [`Self::wants_preferences`] holds.
    fn start_preferences(&mut self, root: &Path) {
        if !self.wants_preferences() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let root = root.to_path_buf();
        match std::thread::Builder::new()
            .name("user-preferences".into())
            .spawn(move || {
                let _ = tx.send(read_user_preferences(&root));
            }) {
            Ok(_) => {
                self.prefs =
                    PendingPreferences::new(rx, std::time::Instant::now() + PREFERENCES_WAIT);
            }
            Err(e) => tracing::warn!("preferences thread: {e}; built-in far-view settings"),
        }
    }

    /// Applies the user's `[Render]` preferences to `config`'s far view and particle
    /// effects, when they were read ([`PendingPreferences::settled`] has returned `true`).
    fn take_preferences(&mut self, config: &mut WalkerConfig) {
        let Some(ini) = self.prefs.ini.take() else {
            return;
        };
        apply_render_preferences(&mut config.far_view.settings, &ini);
        apply_effect_preferences(&mut config.effects, &ini);
        // `--render-pref` wins over the file (`walker.md` §14.10, `far-view.md` FD22).
        apply_render_overrides(
            &mut config.far_view.settings,
            &mut config.effects,
            &self.render_prefs,
        );
        tracing::info!(
            precipitation = config.effects.precipitation,
            video_post = config.effects.video_post,
            atmospherics = ?config.effects.atmospherics,
            "particle effects: [Render] preferences applied"
        );
        let s = &config.far_view.settings;
        tracing::info!(
            normal_maps = s.normal_maps,
            multi_pass_lighting = s.multi_pass_lighting,
            lighting_quality = s.lighting_quality,
            imposters = s.imposters,
            template = ?s.template(),
            seam_overlay = s.seam_overlay(),
            "far view: [Render] preferences applied"
        );
    }

    /// Starts the world start-up when the archives are open and creates the walker when
    /// it finishes. Never blocks.
    pub fn poll(&mut self, rt: &mut Runtime, renderer: &mut Renderer<'_>) {
        if self.walker.is_some() || self.failed.is_some() {
            return;
        }
        match &rt.status {
            DataStatus::NoData(reason) => {
                self.failed = Some(format!("no game data: {reason}"));
                return;
            }
            DataStatus::Loading => return,
            DataStatus::Ready { root, .. } => {
                if !self.started {
                    let Some(set) = rt.archives.clone() else {
                        return;
                    };
                    self.started = true;
                    let root = root.clone();
                    self.start_preferences(&root);
                    match WorldBoot::spawn(set, root.clone(), self.bundles.clone()) {
                        Ok(b) => self.boot = Some(b),
                        Err(e) => self.failed = Some(format!("world start-up thread: {e}")),
                    }
                }
            }
        }
        let Some(boot) = &mut self.boot else { return };
        // The walker is built with the user's preferences: wait for their reader (bounded
        // by [`PREFERENCES_WAIT`], over frames) before taking the start-up result.
        if !self.prefs.settled(std::time::Instant::now()) {
            return;
        }
        let Some(result) = boot.try_take() else {
            return;
        };
        self.boot = None;
        let (ctx, catalog) = match result {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("world walker unavailable: {e}");
                self.failed = Some(e);
                return;
            }
        };
        rt.start_walker_session(
            std::time::Instant::now(),
            avatar_appearance(&self.avatar, &catalog),
            catalog.crowd(),
        );
        let Some(set) = rt.archives.clone() else {
            self.failed = Some("archive set closed".into());
            return;
        };
        let mut config = self.config.clone();
        if let DataStatus::Ready { root, .. } = &rt.status {
            config.audio.game_dir = Some(root.clone());
        }
        self.take_preferences(&mut config);
        match Walker::new(
            config,
            set,
            ctx,
            Arc::clone(&self.shared),
            catalog,
            renderer,
        ) {
            Ok(w) => {
                self.walker = Some(w);
                rt.capture_walker_events(true);
            }
            Err(e) => {
                rt.capture_walker_events(false);
                tracing::error!("world walker: {e:#}");
                self.failed = Some(format!("{e:#}"));
            }
        }
    }
}

/// Renders one walker frame offscreen at `size` and reads it back.
///
/// # Errors
///
/// Renderer errors.
pub fn capture(
    renderer: &mut Renderer<'_>,
    walker: &mut Walker,
    size: (u32, u32),
) -> anyhow::Result<RgbaImage> {
    let target = renderer.create_offscreen_target(size.0, size.1, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = renderer.begin_offscreen_frame(&target)?;
    walker.draw(renderer, &mut frame);
    renderer.end_frame(frame)?;
    Ok(renderer.read_pixels(&target)?)
}

/// Writes `image` as an 8-bit RGBA PNG.
///
/// # Errors
///
/// I/O or encoder errors.
pub fn save_png(path: &Path, image: &RgbaImage) -> anyhow::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let file =
        std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header()?;
    w.write_image_data(&image.data)?;
    w.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser as _;

    #[test]
    fn spawn_points_from_args() {
        let a = Args::try_parse_from(["t", "--spawn", "2,10,20"]).unwrap();
        let c = sandbox_config(&a).unwrap();
        assert_eq!(c.region, RegionId(2));
        assert_eq!(c.wanderers, 0);
        let a = Args::try_parse_from(["t", "--place", "Somewhere", "--region", "3"]).unwrap();
        let c = sandbox_config(&a).unwrap();
        assert_eq!(c.region, RegionId(3));
        assert!(matches!(
            c.spawn_point.map(|p| p.destination),
            Some(tessera_net::Destination::Named(_))
        ));
        let a = Args::try_parse_from(["t", "--fly-path", "2:10:20:5:6,2:30:20"]).unwrap();
        let c = sandbox_config(&a).unwrap();
        assert_eq!(c.region, RegionId(2));
        let w = walker_config(&a);
        assert_eq!(w.mode, MoveMode::Fly);
        assert!(w.frame_log);
        let f = w.flight.unwrap();
        assert_eq!(f.region, 2);
        assert_eq!(f.path.from, [1605.0, 3206.0]);
        let a = Args::try_parse_from(["t"]).unwrap();
        assert!(sandbox_config(&a).is_ok());
        assert!(!walker_config(&a).frame_log);
        assert_eq!(
            walker_config(&a).avatar,
            crate::avatar::AvatarSpec::default()
        );
    }

    #[test]
    fn the_mover_uses_the_avatar_of_the_command_line() {
        let a = Args::try_parse_from(["t", "--avatar", "elf,f"]).unwrap();
        let w = walker_config(&a);
        assert_eq!(Some(&w.avatar), a.avatar.as_ref());
        assert_ne!(w.avatar, crate::avatar::AvatarSpec::default());
    }

    #[test]
    fn the_character_distance_is_a_cap() {
        let a = Args::try_parse_from(["t"]).unwrap();
        assert_eq!(
            walker_config(&a).characters.draw_distance,
            CharacterConfig::default().draw_distance
        );
        let a = Args::try_parse_from(["t", "--character-distance", "50"]).unwrap();
        assert_eq!(walker_config(&a).characters.draw_distance, 50.0);
    }

    /// `far-view.md` FD16–FD18: the `[Render]` section reaches the far-view settings (keys
    /// case-insensitive, other sections ignored); the graphics core stays 5 and the seam
    /// switch on whatever the file says, so a file like the owner's `lotro-launcher` one
    /// gives the normal-mapped template and the seam overlay, one like the Steam-prefix
    /// file the plain template.
    #[test]
    fn the_render_preferences_reach_the_far_view() {
        use tessera_world::far_view::{FarViewSettings, Template};
        let ini = tessera_install::Ini::parse(
            "[Sound]\r\nDistantImposters=True\r\n[Render]\r\nfarlandscapenormalmaps=True\r\nNearFarSeamBlending=False\r\nMultiPassLighting=True\r\nLandscapeLightingQuality=VeryHigh\r\nDistantImposters=False\r\nGraphicsCore=D3D9\r\n",
        );
        let mut s = FarViewSettings::default();
        assert!(s.imposters);
        assert_eq!(s.template(), Template::Plain, "built-in values");
        assert!(!s.seam_overlay());
        apply_render_preferences(&mut s, &ini);
        assert!(s.normal_maps && s.seam_blending && !s.imposters);
        assert_eq!(s.graphics_core, 5);
        assert_eq!(s.template(), Template::NormalMapped);
        assert!(s.seam_overlay());
        let steam = tessera_install::Ini::parse(
            "[Render]\nFarLandscapeNormalMaps=False\nMultiPassLighting=True\nLandscapeLightingQuality=VeryHigh\n",
        );
        let mut p = FarViewSettings::default();
        apply_render_preferences(&mut p, &steam);
        assert_eq!(p.template(), Template::Plain);
        assert!(!p.seam_overlay());
        // A file without the keys keeps the settings.
        let mut t = FarViewSettings::default();
        apply_render_preferences(
            &mut t,
            &tessera_install::Ini::parse("[Render]\nGamma=1.0\n"),
        );
        assert_eq!(t, FarViewSettings::default());
    }

    /// `particle-runtime.md` §11.2.2 and rule 16: `PrecipitationEffects` and
    /// `VideoPostEffects` reach the effects layer's settings; missing keys keep them on.
    #[test]
    fn the_render_preferences_reach_the_effects() {
        use crate::walker::effects::EffectConfig;
        let mut e = EffectConfig::default();
        assert!(e.precipitation && e.video_post);
        apply_effect_preferences(
            &mut e,
            &tessera_install::Ini::parse("[Render]\nprecipitationeffects=False\n"),
        );
        assert!(!e.precipitation && e.video_post);
        apply_effect_preferences(
            &mut e,
            &tessera_install::Ini::parse(
                "[Render]\nPrecipitationEffects=True\nVideoPostEffects=False\n",
            ),
        );
        assert!(e.precipitation && !e.video_post);
        let a = Args::try_parse_from(["t"]).unwrap();
        let w = walker_config(&a);
        assert!(
            w.effects.precipitation && w.effects.video_post,
            "built-in values"
        );
        assert_eq!(w.effects.atmospherics, None, "built-in Medium");
    }

    /// `particle-runtime.md` §11.2.6 step 1: `AtmosphericsDetail` reaches the effects
    /// layer's settings as a level (names case-insensitive, or a number); a value the
    /// reader does not accept is the built-in `Medium` (`None`), a missing key keeps the
    /// current level.
    #[test]
    fn atmospherics_detail_reaches_the_effects() {
        use crate::walker::effects::EffectConfig;
        assert_eq!(atmospherics_level("Low"), Some(0));
        assert_eq!(atmospherics_level(" medium "), Some(1));
        assert_eq!(atmospherics_level("HIGH"), Some(2));
        assert_eq!(atmospherics_level("2"), Some(2));
        assert_eq!(atmospherics_level("7"), Some(7));
        assert_eq!(atmospherics_level("-1"), None);
        assert_eq!(atmospherics_level("Ultra"), None);
        assert_eq!(atmospherics_level(""), None);
        let mut e = EffectConfig::default();
        apply_effect_preferences(
            &mut e,
            &tessera_install::Ini::parse(
                "[Render]
atmosphericsdetail=High
",
            ),
        );
        assert_eq!(e.atmospherics, Some(2));
        apply_effect_preferences(
            &mut e,
            &tessera_install::Ini::parse(
                "[Render]
Gamma=1.0
",
            ),
        );
        assert_eq!(e.atmospherics, Some(2), "a missing key keeps the level");
        apply_effect_preferences(
            &mut e,
            &tessera_install::Ini::parse(
                "[Render]
AtmosphericsDetail=Bogus
",
            ),
        );
        assert_eq!(
            e.atmospherics, None,
            "an unknown value is the built-in level"
        );
    }

    /// Headless and test runs never read the user's preferences file (CLAUDE.md: real-data
    /// tests read only the install).
    #[test]
    fn headless_runs_keep_the_built_in_far_view_settings() {
        let a = Args::try_parse_from([
            "t",
            "--far-view",
            "on",
            "--screenshot",
            "/nonexistent/x.png",
        ])
        .unwrap();
        let mut h = WalkerHost::new(&a, Arc::new(Mutex::new(SourceShared::default())));
        assert!(!h.user_prefs);
        h.start_preferences(Path::new("/nonexistent"));
        assert!(h.prefs.rx.is_none());
    }

    /// `particle-runtime.md` §11.2.2 and rule 16: with the far view off (its default) and
    /// the effects on, a windowed run still reads the user's preferences and applies their
    /// effect options. The root names no install, so the reader finds no file and reads
    /// nothing of the user's; the read file is put in by hand.
    #[test]
    fn effects_alone_read_the_preferences() {
        let a = Args::try_parse_from(["t"]).unwrap();
        assert!(!a.far_view && !a.no_particles && !a.walker_headless());
        let mut h = WalkerHost::new(&a, Arc::new(Mutex::new(SourceShared::default())));
        assert!(h.wants_preferences());
        h.start_preferences(Path::new("/nonexistent"));
        assert!(h.prefs.rx.is_some(), "the read is started");
        h.prefs.rx = None;
        h.prefs.ini = Some(tessera_install::Ini::parse(
            "[Render]\nPrecipitationEffects=False\nVideoPostEffects=False\nAtmosphericsDetail=Low\n",
        ));
        let mut config = h.config.clone();
        assert!(config.effects.precipitation && config.effects.video_post);
        h.take_preferences(&mut config);
        assert!(!config.effects.precipitation && !config.effects.video_post);
        assert_eq!(config.effects.atmospherics, Some(0));

        // Neither the far view nor the effects: nothing reads the file.
        let a = Args::try_parse_from(["t", "--no-particles"]).unwrap();
        let mut h = WalkerHost::new(&a, Arc::new(Mutex::new(SourceShared::default())));
        assert!(!h.wants_preferences());
        h.start_preferences(Path::new("/nonexistent"));
        assert!(h.prefs.rx.is_none());
    }

    /// `far-view.md` FD22, `walker.md` §14.10: a headless run with the three overrides of
    /// the far view's real-data check gets the normal-mapped template and the seam overlay
    /// over the built-in values; the fixed keys stay fixed and, like unknown keys, are
    /// reported once.
    #[test]
    fn render_prefs_override_the_built_in_values_headless() {
        use tessera_world::far_view::Template;
        let a = Args::try_parse_from([
            "t",
            "--far-view",
            "on",
            "--screenshot",
            "/nonexistent/x.png",
            "--render-pref",
            "FarLandscapeNormalMaps=True",
            "--render-pref",
            "MultiPassLighting=True",
            "--render-pref",
            "LandscapeLightingQuality=High",
            "--render-pref",
            "GraphicsCore=2",
            "--render-pref",
            "nearfarseamblending=False",
            "--render-pref",
            "Bogus=1",
            "--render-pref",
            "BOGUS=2",
            "--render-pref",
            "AtmosphericsDetail=Low",
        ])
        .unwrap();
        assert!(a.walker_headless());
        let w = walker_config(&a);
        let s = &w.far_view.settings;
        assert_eq!(s.template(), Template::NormalMapped);
        assert!(s.seam_overlay());
        assert_eq!(s.graphics_core, 5, "GraphicsCore is fixed (FD16)");
        assert!(s.seam_blending, "NearFarSeamBlending is fixed (FD17)");
        assert_eq!(w.effects.atmospherics, Some(0));
        assert_eq!(
            ignored_render_prefs(&a.render_prefs),
            vec![
                IgnoredRenderPref {
                    key: "GraphicsCore".into(),
                    fixed: true
                },
                IgnoredRenderPref {
                    key: "nearfarseamblending".into(),
                    fixed: true
                },
                IgnoredRenderPref {
                    key: "Bogus".into(),
                    fixed: false
                },
            ],
            "each ignored key once"
        );
        // Without overrides the built-in values stay (FD17, FD21).
        let plain = walker_config(&Args::try_parse_from(["t", "--far-view", "on"]).unwrap());
        assert_eq!(plain.far_view.settings.template(), Template::Plain);
        assert!(!plain.far_view.settings.seam_overlay());
    }

    /// `walker.md` §14.10: in a windowed run the overrides are applied after the user's
    /// file, so they win over it; a value the key does not accept counts as the built-in
    /// value, not as the file's; keys the overrides do not name keep the file's value.
    #[test]
    fn render_prefs_win_over_the_users_file() {
        let a = Args::try_parse_from([
            "t",
            "--far-view",
            "on",
            "--render-pref",
            "FarLandscapeNormalMaps=False",
            "--render-pref",
            "DistantImposters=maybe",
            "--render-pref",
            "atmosphericsdetail=High",
            "--render-pref",
            "VideoPostEffects=nonsense",
        ])
        .unwrap();
        assert!(!a.walker_headless());
        let mut h = WalkerHost::new(&a, Arc::new(Mutex::new(SourceShared::default())));
        h.prefs.ini = Some(tessera_install::Ini::parse(
            "[Render]\nFarLandscapeNormalMaps=True\nMultiPassLighting=False\nDistantImposters=False\nAtmosphericsDetail=Low\nVideoPostEffects=False\nPrecipitationEffects=False\n",
        ));
        let mut config = h.config.clone();
        h.take_preferences(&mut config);
        let s = &config.far_view.settings;
        assert!(!s.normal_maps, "the override wins over the file");
        assert!(
            s.imposters,
            "an unaccepted override value is the built-in on, not the file's off"
        );
        assert!(!s.multi_pass_lighting, "not overridden: the file's value");
        assert_eq!(config.effects.atmospherics, Some(2));
        assert!(config.effects.video_post, "unaccepted: built-in on");
        assert!(
            !config.effects.precipitation,
            "not overridden: the file's value"
        );
    }

    /// `walker.md` §14.10 and §11.2: the distance keys reach the distance settings (names
    /// case-insensitive or stored numbers; an unaccepted value is the built-in level), and
    /// the far view's radius follows; the dedicated options win over them.
    #[test]
    fn render_prefs_reach_the_distance_settings() {
        use tessera_world::DetailLevel;
        use tessera_world::draw_distance::LandscapeLevel;
        let args = |extra: &[&str]| {
            let mut v = vec!["t"];
            v.extend_from_slice(extra);
            Args::try_parse_from(v).unwrap()
        };
        let a = args(&[
            "--render-pref",
            "LandscapeDrawDistance=low",
            "--render-pref",
            "ObjectDrawDistance=4",
            "--render-pref",
            "ModelDetail=Weird",
            "--render-pref",
            "FrillDistance=5",
        ]);
        let d = distance_settings(&a);
        assert_eq!(d.landscape, LandscapeLevel::Low);
        assert_eq!(d.object_draw_distance, DetailLevel::UltraHigh);
        assert_eq!(d.model_detail, DistanceSettings::default().model_detail);
        assert_eq!(frill_distance_level(&a), 5);
        let w = walker_config(&a);
        assert_eq!(w.far_view.settings.far_radius, LandscapeLevel::Low.groups());
        assert_eq!(w.stream.frills.distance_level, 5);
        let b = args(&[
            "--landscape-distance",
            "VeryHigh",
            "--frill-distance",
            "1",
            "--render-pref",
            "LandscapeDrawDistance=Low",
            "--render-pref",
            "FrillDistance=9",
        ]);
        assert_eq!(distance_settings(&b).landscape, LandscapeLevel::VeryHigh);
        assert_eq!(frill_distance_level(&b), 1);
        let c = args(&["--render-pref", "FrillDistance=9"]);
        assert_eq!(
            frill_distance_level(&c),
            FrillSettings::default().distance_level
        );
    }

    /// `far-view.md` FD18 (FV29): only `Low` 0, `High` 1, `VeryHigh` 2 and the numbers 0–2
    /// are levels of `LandscapeLightingQuality`; every other value, other scales' names
    /// included, is the built-in level 0, so no seam overlay, both from `--render-pref`
    /// (FD22) and from the user's file (FD17).
    #[test]
    fn landscape_lighting_quality_follows_fd18() {
        use tessera_world::far_view::FarViewSettings;
        for (v, level) in [
            ("Low", 0),
            ("high", 1),
            (" VERYHIGH ", 2),
            ("0", 0),
            ("1", 1),
            ("2", 2),
            ("Medium", 0),
            ("UltraHigh", 0),
            ("VeryLow", 0),
            ("Disabled", 0),
            ("3", 0),
            ("7", 0),
            ("-1", 0),
            ("", 0),
        ] {
            assert_eq!(landscape_lighting_level(v), level, "{v:?}");
        }
        let overlay = |value: &str| {
            let quality = format!("LandscapeLightingQuality={value}");
            let a = Args::try_parse_from([
                "t",
                "--far-view",
                "on",
                "--screenshot",
                "/nonexistent/x.png",
                "--render-pref",
                "FarLandscapeNormalMaps=True",
                "--render-pref",
                "MultiPassLighting=True",
                "--render-pref",
                quality.as_str(),
            ])
            .unwrap();
            walker_config(&a).far_view.settings.seam_overlay()
        };
        let file = |value: &str| {
            let ini = tessera_install::Ini::parse(&format!(
                "[Render]\nFarLandscapeNormalMaps=True\nMultiPassLighting=True\nLandscapeLightingQuality={value}\n"
            ));
            let mut s = FarViewSettings::default();
            apply_render_preferences(&mut s, &ini);
            s.seam_overlay()
        };
        for v in ["Medium", "UltraHigh", "3", "7", "Low", "0"] {
            assert!(!overlay(v), "override {v}");
            assert!(!file(v), "file {v}");
        }
        for v in ["High", "VeryHigh", "1", "2"] {
            assert!(overlay(v), "override {v}");
            assert!(file(v), "file {v}");
        }
        // A file value FD18 does not accept is level 0 even over a higher current level.
        let mut s = FarViewSettings {
            lighting_quality: 2,
            ..FarViewSettings::default()
        };
        apply_render_preferences(
            &mut s,
            &tessera_install::Ini::parse("[Render]\nLandscapeLightingQuality=Medium\n"),
        );
        assert_eq!(s.lighting_quality, 0);
    }

    /// `walker.md` §11.2 and §14.10: `FrillDistance` takes its six level names
    /// (case-insensitive) or the numbers 0–5; anything else is the built-in level.
    #[test]
    fn frill_distance_takes_names_and_numbers() {
        let level = |v: &str| {
            let pref = format!("FrillDistance={v}");
            frill_distance_level(
                &Args::try_parse_from(["t", "--render-pref", pref.as_str()]).unwrap(),
            )
        };
        for (v, n) in [
            ("Disabled", 0),
            ("low", 1),
            ("MEDIUM", 2),
            ("High", 3),
            ("veryhigh", 4),
            ("UltraHigh", 5),
            (" ultrahigh ", 5),
            ("0", 0),
            ("5", 5),
        ] {
            assert_eq!(level(v), n, "{v:?}");
        }
        let builtin = FrillSettings::default().distance_level;
        for v in ["VeryLow", "Ultra", "6", "-1", ""] {
            assert_eq!(level(v), builtin, "{v:?}");
        }
        assert_eq!(frill_level("Disabled"), Some(0));
        assert_eq!(frill_level("Off"), None);
    }

    /// The walker waits (over frames, never blocking) for preferences still being read,
    /// until they arrive or the deadline passes; a reader that is gone settles at once.
    #[test]
    fn the_walker_waits_for_preferences_still_being_read() {
        use std::time::{Duration, Instant};
        let now = Instant::now();
        let (tx, rx) = std::sync::mpsc::channel();
        let mut p = PendingPreferences::new(rx, now + Duration::from_secs(5));
        assert!(!p.settled(now), "still being read");
        assert!(!p.settled(now + Duration::from_secs(4)));
        tx.send(Some(tessera_install::Ini::parse(
            "[Render]\nDistantImposters=False\n",
        )))
        .unwrap();
        assert!(p.settled(now + Duration::from_secs(4)));
        assert!(p.ini.is_some(), "arrived late but in time: kept");
        assert!(p.settled(now), "settled stays settled");

        let (_tx, rx) = std::sync::mpsc::channel::<Option<tessera_install::Ini>>();
        let mut late = PendingPreferences::new(rx, now + Duration::from_secs(5));
        assert!(
            late.settled(now + Duration::from_secs(5)),
            "deadline passed"
        );
        assert!(late.ini.is_none());

        let (tx, rx) = std::sync::mpsc::channel::<Option<tessera_install::Ini>>();
        drop(tx);
        let mut gone = PendingPreferences::new(rx, now + Duration::from_secs(5));
        assert!(gone.settled(now));
        assert!(
            PendingPreferences::default().settled(now),
            "nothing to wait for"
        );
    }
}
