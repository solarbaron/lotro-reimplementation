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
        resolve_timeout: MAX_RESOLVE_TIMEOUT.min(Duration::from_secs(300)),
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

/// Walker settings from the command line.
#[must_use]
pub fn walker_config(args: &Args) -> WalkerConfig {
    let distances = distance_settings(args);
    // The far ring is the landscape extent E (`walker.md` §11.3, §11.8) unless
    // `--view-blocks` sets it.
    let far = args.view_blocks.map_or_else(
        || f64::from(distances.landscape_extent()),
        |b| f64::from(b) * LANDBLOCK_SIZE,
    );
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
            ..CharacterConfig::default()
        },
        autorun: args.autorun.as_deref().map(|g| g == "walk"),
        bench: args.duration.map(|secs| crate::walker::BenchPlan {
            duration: Duration::from_secs(secs),
            settle_timeout: Duration::from_secs(args.settle_timeout),
        }),
        audio: crate::audio::config_from_args(args),
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
            settings: tessera_world::far_view::FarViewSettings {
                far_radius: distances.landscape.groups(),
                ..tessera_world::far_view::FarViewSettings::default()
            },
            ..crate::walker::farview::FarViewConfig::default()
        },
        hud: !args.no_hud,
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
        }
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
                    match WorldBoot::spawn(set, root.clone(), self.bundles.clone()) {
                        Ok(b) => self.boot = Some(b),
                        Err(e) => self.failed = Some(format!("world start-up thread: {e}")),
                    }
                }
            }
        }
        let Some(boot) = &mut self.boot else { return };
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
}
