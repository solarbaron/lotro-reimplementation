// SPDX-License-Identifier: GPL-3.0-or-later
//! Command-line options.

use std::path::PathBuf;

use clap::Parser;
use tessera_formats::did::DidKind;

/// Largest `--settle-timeout` and `--data-timeout` (one day), so deadlines never overflow.
pub const MAX_TIMEOUT_SECS: u64 = 86_400;
/// Largest `--fly-speed`: boosted (× 5) it stays within the sandbox's speed limit.
pub const MAX_FLY_SPEED: f32 = 2_000.0;
const _: () = assert!(MAX_FLY_SPEED * 5.0 <= tessera_net::sandbox::MAX_SPEED);

/// Tessera client.
///
/// Reads the data files of an existing install in place and read-only. Without an
/// install it runs in no-data mode and shows an original test scene.
#[derive(Debug, Clone, Parser)]
#[command(name = "tessera-client", version, about)]
#[command(group(
    clap::ArgGroup::new("headless_task")
        .args(["screenshot", "fly_path"])
        .multiple(true)
))]
pub struct Args {
    /// Install directory (holds `client_general.dat`). Defaults to `TESSERA_GAME_DIR`,
    /// then automatic discovery.
    #[arg(long, value_name = "DIR")]
    pub game_dir: Option<PathBuf>,

    /// Do not look for an install; run the original test scene only.
    #[arg(long, conflicts_with = "game_dir")]
    pub no_data: bool,

    /// Render-surface DID to show (e.g. `0x41000123`). Default: the first surface in the
    /// archives that decodes.
    #[arg(long, value_name = "DID", value_parser = parse_surface_did)]
    pub surface: Option<u32>,

    /// Skip counting directory entries of every archive (faster start-up log).
    #[arg(long)]
    pub no_entry_count: bool,

    /// Plugins folder to scan (read-only). Default: `<data dir>/Plugins`.
    #[arg(long, value_name = "DIR")]
    pub plugins: Option<PathBuf>,

    /// Plugin to load after discovery (by descriptor name). Repeatable.
    #[arg(long = "load-plugin", value_name = "NAME")]
    pub load_plugins: Vec<String>,

    /// Tessera's own data folder (plugin data). Default: `$XDG_DATA_HOME/tessera`.
    #[arg(long, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,

    /// Initialise everything, render frames offscreen without a window, then exit.
    #[arg(long)]
    pub smoke: bool,

    /// Number of frames rendered by `--smoke`.
    #[arg(long, default_value_t = 8, value_parser = clap::value_parser!(u32).range(1..=10_000))]
    pub frames: u32,

    /// Seconds `--smoke` waits for the data loader before rendering anyway (at most a day).
    #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u64).range(0..=MAX_TIMEOUT_SECS))]
    pub data_timeout: u64,

    /// Write the last `--smoke` frame as a binary PPM image to this path.
    #[arg(long, value_name = "FILE", requires = "smoke")]
    pub smoke_out: Option<PathBuf>,

    /// Windowed mode: close the window after this many presented frames (for scripted
    /// checks of the window path).
    #[arg(long, value_name = "N", conflicts_with = "smoke")]
    pub exit_after_frames: Option<u64>,

    /// Window / offscreen size as `WIDTHxHEIGHT`.
    #[arg(long, default_value = "1280x720", value_parser = parse_size)]
    pub size: (u32, u32),

    /// Disable vsync (present mode `AutoNoVsync`).
    #[arg(long)]
    pub no_vsync: bool,

    // ----- world walker -----
    /// Show the M0 texture showcase instead of the world walker.
    #[arg(long, conflicts_with_all = ["spawn", "place", "fly_path", "screenshot"])]
    pub showcase: bool,

    /// Spawn at a landblock: `R,X,Y` (block centre), `R,X,Y,x,y` (block-local) or
    /// `R,X,Y,x,y,z` (exact height).
    #[arg(long, value_name = "R,X,Y[,x,y[,z]]", value_parser = parse_spawn, conflicts_with = "place")]
    pub spawn: Option<String>,

    /// Spawn at a named place: an area, territory or region name, map coordinates such as
    /// `12.3S, 45.6W`, or a pasted `/loc` line.
    #[arg(long, value_name = "NAME")]
    pub place: Option<String>,

    /// Region for `--place` names and map coordinates, and for `--fly-path` points
    /// without one.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=i64::from(crate::walker::parse::MAX_REGION)))]
    pub region: u8,

    /// Start in free-fly mode instead of walking.
    #[arg(long)]
    pub fly: bool,

    /// Base free-fly speed in m/s, at most 2000 (Shift multiplies by 5; the wheel scales
    /// it). Faster movement would exceed the world session's speed limit.
    #[arg(long, default_value_t = 60.0, value_parser = parse_fly_speed)]
    pub fly_speed: f32,

    /// Fly a straight line `FROM,TO`, each `X:Y`, `R:X:Y`, `X:Y:x:y` or `R:X:Y:x:y`
    /// (landblock, optional block-local offset), then exit.
    #[arg(long, value_name = "FROM,TO", value_parser = parse_fly_path_arg)]
    pub fly_path: Option<crate::walker::parse::FlyPath>,

    /// `--fly-path` speed in m/s.
    #[arg(long, default_value_t = 1_000.0, value_parser = parse_positive_f64)]
    pub speed: f64,

    /// `--fly-path` height above the terrain in metres.
    #[arg(long, default_value_t = 120.0, value_parser = parse_positive_f64)]
    pub fly_height: f64,

    /// Write per-frame times and residency as CSV (written when the walker exits).
    #[arg(long, value_name = "CSV")]
    pub frame_log: Option<PathBuf>,

    /// Render the spawn point offscreen once everything around it has loaded, save it
    /// as PNG and exit.
    #[arg(long, value_name = "PNG")]
    pub screenshot: Option<PathBuf>,

    /// Run the walker offscreen without a window (implied by `--screenshot`; needs
    /// `--screenshot` or `--fly-path`, which end the run).
    #[arg(long, requires = "headless_task")]
    pub headless: bool,

    /// Seconds `--screenshot` and `--fly-path` wait for the area to load (1 to 86,400).
    #[arg(long, default_value_t = 90, value_parser = clap::value_parser!(u64).range(1..=MAX_TIMEOUT_SECS))]
    pub settle_timeout: u64,

    /// Keymap file (Tessera's TOML keymap). Default: `keymap.toml` next to the settings.
    #[arg(long, value_name = "FILE")]
    pub keymap: Option<PathBuf>,

    /// Residency budget of landblock bundles in MiB.
    #[arg(long, default_value_t = 512, value_parser = clap::value_parser!(u64).range(16..=65_536))]
    pub residency_mib: u64,

    /// Terrain view distance in landblocks (far ring radius).
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u8).range(2..=30))]
    pub view_blocks: u8,

    /// Static objects are drawn up to this distance in metres.
    #[arg(long, default_value_t = 900.0, value_parser = parse_positive_f64)]
    pub object_distance: f64,

    /// GPU memory for object meshes and textures in MiB (terrain meshes get their own
    /// share, scaled with `--view-blocks`).
    #[arg(long, default_value_t = 768, value_parser = clap::value_parser!(u64).range(64..=65_536))]
    pub gpu_mib: u64,

    /// Largest texture edge used for objects.
    #[arg(long, default_value_t = 512, value_parser = clap::value_parser!(u32).range(16..=4096))]
    pub texture_cap: u32,

    /// Facing at the spawn as a compass heading in degrees (0 = north, 90 = east).
    #[arg(long, value_name = "DEG", value_parser = parse_degrees)]
    pub heading: Option<f32>,

    /// Camera pitch at the spawn in degrees (negative looks down).
    #[arg(long, value_name = "DEG", default_value_t = -14.0, allow_negative_numbers = true, value_parser = parse_pitch)]
    pub pitch: f32,

    /// Height of the fly camera above the spawn in metres.
    #[arg(long, default_value_t = 30.0, value_parser = parse_positive)]
    pub eye_height: f32,

    /// Also draw interior (cell) entities.
    #[arg(long)]
    pub interiors: bool,
}

impl Args {
    /// Whether the world walker runs (instead of the M0 showcase).
    #[must_use]
    pub fn walker(&self) -> bool {
        !self.showcase && !self.no_data && !self.smoke
    }

    /// Whether the walker renders offscreen.
    #[must_use]
    pub fn walker_headless(&self) -> bool {
        self.walker() && (self.headless || self.screenshot.is_some())
    }
}

/// Checks a `--spawn` value with the sandbox's spawn-point grammar.
pub fn parse_spawn(s: &str) -> Result<String, String> {
    let p = tessera_net::sandbox::world::SpawnPoint::parse(s).map_err(|e| e.to_string())?;
    if matches!(p.destination, tessera_net::Destination::Named(_)) {
        return Err("expected R,X,Y[,x,y[,z]] (use --place for names)".into());
    }
    Ok(s.to_owned())
}

fn parse_fly_path_arg(s: &str) -> Result<crate::walker::parse::FlyPath, String> {
    crate::walker::parse::parse_fly_path(s)
}

fn parse_degrees(s: &str) -> Result<f32, String> {
    let v: f32 = s.trim().parse().map_err(|e| format!("{e}"))?;
    if v.is_finite() && (-360.0..=360.0).contains(&v) {
        Ok(v)
    } else {
        Err("must be degrees in -360..=360".into())
    }
}

fn parse_pitch(s: &str) -> Result<f32, String> {
    let v: f32 = s.trim().parse().map_err(|e| format!("{e}"))?;
    if v.is_finite() && (-89.0..=89.0).contains(&v) {
        Ok(v)
    } else {
        Err("must be degrees in -89..=89".into())
    }
}

fn parse_fly_speed(s: &str) -> Result<f32, String> {
    let v: f32 = s.trim().parse().map_err(|e| format!("{e}"))?;
    if v.is_finite() && v > 0.0 && v <= MAX_FLY_SPEED {
        Ok(v)
    } else {
        Err(format!("must be a number in (0, {MAX_FLY_SPEED}]"))
    }
}

fn parse_positive(s: &str) -> Result<f32, String> {
    let v: f32 = s.trim().parse().map_err(|e| format!("{e}"))?;
    if v.is_finite() && v > 0.0 && v <= 1.0e4 {
        Ok(v)
    } else {
        Err("must be a number in (0, 10000]".into())
    }
}

fn parse_positive_f64(s: &str) -> Result<f64, String> {
    let v: f64 = s.trim().parse().map_err(|e| format!("{e}"))?;
    if v.is_finite() && v > 0.0 && v <= 1.0e5 {
        Ok(v)
    } else {
        Err("must be a number in (0, 100000]".into())
    }
}

/// Parses a render-surface DID in hex (`0x41...`) or decimal and checks its type prefix.
pub fn parse_surface_did(s: &str) -> Result<u32, String> {
    let t = s.trim();
    let value = match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(&hex.replace('_', ""), 16),
        None => t.replace('_', "").parse::<u32>(),
    }
    .map_err(|e| format!("not a DID: {e}"))?;
    if tessera_formats::Did(value).kind() != DidKind::RenderSurface {
        return Err(format!(
            "{value:#010x} is not a render-surface DID (0x41xxxxxx)"
        ));
    }
    Ok(value)
}

/// Parses `WIDTHxHEIGHT` with both sides in `1..=16384`.
pub fn parse_size(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s
        .split_once(['x', 'X'])
        .ok_or_else(|| "expected WIDTHxHEIGHT".to_owned())?;
    let parse = |v: &str| -> Result<u32, String> {
        let n: u32 = v.trim().parse().map_err(|e| format!("{v:?}: {e}"))?;
        if (1..=16_384).contains(&n) {
            Ok(n)
        } else {
            Err(format!("{n} is outside 1..=16384"))
        }
    };
    Ok((parse(w)?, parse(h)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_did_parsing() {
        assert_eq!(parse_surface_did("0x41000123"), Ok(0x4100_0123));
        assert_eq!(parse_surface_did("0x4100_0123"), Ok(0x4100_0123));
        assert_eq!(
            parse_surface_did(&0x4100_0001u32.to_string()),
            Ok(0x4100_0001)
        );
        assert!(parse_surface_did("0x25000001").is_err());
        assert!(parse_surface_did("zz").is_err());
        assert!(parse_surface_did("0x1_0000_0000").is_err());
    }

    #[test]
    fn size_parsing() {
        assert_eq!(parse_size("640x360"), Ok((640, 360)));
        assert_eq!(parse_size("1X1"), Ok((1, 1)));
        assert!(parse_size("0x10").is_err());
        assert!(parse_size("640").is_err());
        assert!(parse_size("99999x10").is_err());
    }

    #[test]
    fn args_parse() {
        let a = Args::try_parse_from(["t", "--smoke", "--frames", "3", "--surface", "0x41000001"])
            .expect("valid args");
        assert!(a.smoke);
        assert_eq!(a.frames, 3);
        assert_eq!(a.surface, Some(0x4100_0001));
        assert!(Args::try_parse_from(["t", "--smoke-out", "x.ppm"]).is_err());
    }

    #[test]
    fn walker_args_parse() {
        let a = Args::try_parse_from(["t", "--spawn", "1,100,150,40,80"]).expect("spawn");
        assert!(a.walker() && !a.walker_headless());
        assert!(Args::try_parse_from(["t", "--spawn", "1,255,1"]).is_err());
        assert!(Args::try_parse_from(["t", "--spawn", "Stonebridge"]).is_err());
        let a = Args::try_parse_from([
            "t",
            "--fly-path",
            "1:10:100,1:200:100",
            "--speed",
            "800",
            "--headless",
            "--frame-log",
            "f.csv",
        ])
        .expect("fly path");
        assert!(a.walker_headless());
        assert_eq!(a.fly_path.map(|p| p.region), Some(Some(1)));
        assert!(Args::try_parse_from(["t", "--fly-path", "1:2"]).is_err());
        assert!(Args::try_parse_from(["t", "--speed", "-1"]).is_err());
        assert!(Args::try_parse_from(["t", "--place", "x", "--spawn", "1,2,3"]).is_err());
        let a = Args::try_parse_from(["t", "--screenshot", "a.png", "--place", "12.3S, 45.6W"])
            .expect("screenshot");
        assert!(a.walker_headless());
        assert!(!Args::try_parse_from(["t", "--showcase"]).unwrap().walker());
    }

    #[test]
    fn timeouts_are_bounded() {
        let max = u64::MAX.to_string();
        assert!(Args::try_parse_from(["t", "--settle-timeout", &max]).is_err());
        assert!(Args::try_parse_from(["t", "--smoke", "--data-timeout", &max]).is_err());
        assert!(Args::try_parse_from(["t", "--settle-timeout", "0"]).is_err());
        let a = Args::try_parse_from(["t", "--settle-timeout", "86400", "--data-timeout", "0"])
            .expect("in range");
        assert_eq!((a.settle_timeout, a.data_timeout), (86_400, 0));
    }

    #[test]
    fn headless_needs_an_end() {
        assert!(Args::try_parse_from(["t", "--headless"]).is_err());
        assert!(Args::try_parse_from(["t", "--headless", "--screenshot", "a.png"]).is_ok());
        assert!(Args::try_parse_from(["t", "--headless", "--fly-path", "1:2,3:4"]).is_ok());
    }

    #[test]
    fn fly_speed_is_capped() {
        assert!(Args::try_parse_from(["t", "--fly-speed", "2000"]).is_ok());
        assert!(Args::try_parse_from(["t", "--fly-speed", "2001"]).is_err());
    }
}
