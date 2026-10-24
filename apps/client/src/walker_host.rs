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
use crate::cli::Args;
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
/// (`walker.md` §11.2, §11.8).
#[must_use]
pub fn distance_settings(args: &Args) -> DistanceSettings {
    let d = DistanceSettings::default();
    DistanceSettings {
        landscape: args.landscape_distance.unwrap_or(d.landscape),
        object_draw_distance: args.object_draw_distance.unwrap_or(d.object_draw_distance),
        model_detail: args.model_detail.unwrap_or(d.model_detail),
        ..d
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
    settings.apply_render_preferences(|key| prefs.get("Render", key));
}

/// Applies the `[Render]` options of the user's `UserPreferences.ini` that the particle
/// effects read: `PrecipitationEffects` (`particle-runtime.md` §11.2.2, built-in on) and
/// `VideoPostEffects` (rule 16: off skips the systems drawn after post-processing; built-in
/// on). Unknown or missing values keep the current ones. Windowed runs only, like the far
/// view's (`far-view.md` FD21).
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

/// Walker settings from the command line (and the `[Graphics]` settings).
#[must_use]
pub fn walker_config(args: &Args) -> WalkerConfig {
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
                distance_level: args.frill_distance.map_or(2, i32::from),
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
        frame_log: args.frame_log.is_some()
            || args.fly_path.is_some()
            || args.timing_secs().is_some(),
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
        bench: args.timing_secs().map(|secs| crate::walker::BenchPlan {
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
            precipitation: true,
            video_post: true,
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
        tracing::info!(
            precipitation = config.effects.precipitation,
            video_post = config.effects.video_post,
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
            "[Render]\nPrecipitationEffects=False\nVideoPostEffects=False\n",
        ));
        let mut config = h.config.clone();
        assert!(config.effects.precipitation && config.effects.video_post);
        h.take_preferences(&mut config);
        assert!(!config.effects.precipitation && !config.effects.video_post);

        // Neither the far view nor the effects: nothing reads the file.
        let a = Args::try_parse_from(["t", "--no-particles"]).unwrap();
        let mut h = WalkerHost::new(&a, Arc::new(Mutex::new(SourceShared::default())));
        assert!(!h.wants_preferences());
        h.start_preferences(Path::new("/nonexistent"));
        assert!(h.prefs.rx.is_none());
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
