// SPDX-License-Identifier: GPL-3.0-or-later
//! Command-line options.

use std::path::PathBuf;

use clap::Parser;
use tessera_formats::did::DidKind;

/// Tessera client.
///
/// Reads the data files of an existing install in place and read-only. Without an
/// install it runs in no-data mode and shows an original test scene.
#[derive(Debug, Clone, Parser)]
#[command(name = "tessera-client", version, about)]
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

    /// Seconds `--smoke` waits for the data loader before rendering anyway.
    #[arg(long, default_value_t = 120)]
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
}
