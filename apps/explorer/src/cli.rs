// SPDX-License-Identifier: GPL-3.0-or-later
//! Command-line options.

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

/// The views of the central panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum)]
pub enum ViewKind {
    /// Entry, archive and routing information.
    Info,
    /// Hex dump of the record.
    Hex,
    /// Surfaces and texture lists.
    Texture,
    /// String tables.
    Strings,
    /// Property sets of objects and property-set records.
    Props,
    /// Layered icons.
    Icon,
    /// Decoded record tables (DID maps, enum mappers, dictionary, lists, sounds).
    Record,
    /// 3D mesh and material viewer (meshes, setups, templates, entities, appearances,
    /// objects with geometry, surface records, materials).
    Mesh,
}

impl ViewKind {
    /// Every view, in tab order.
    pub const ALL: [ViewKind; 8] = [
        ViewKind::Info,
        ViewKind::Mesh,
        ViewKind::Texture,
        ViewKind::Strings,
        ViewKind::Props,
        ViewKind::Icon,
        ViewKind::Record,
        ViewKind::Hex,
    ];

    /// Tab caption.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Info => "Info",
            Self::Hex => "Hex",
            Self::Texture => "Texture",
            Self::Strings => "Strings",
            Self::Props => "Properties",
            Self::Icon => "Icon",
            Self::Record => "Record",
            Self::Mesh => "3D",
        }
    }

    /// Whether the view applies to `did`.
    #[must_use]
    pub fn applies_to(self, did: u32) -> bool {
        let prefix = did >> 24;
        match self {
            Self::Info | Self::Hex => true,
            Self::Texture => matches!(prefix, 0x40 | 0x41),
            Self::Strings => prefix == 0x25,
            Self::Props => matches!(prefix, 0x70..=0x7F),
            Self::Icon => matches!(prefix, 0x41 | 0x70..=0x7F),
            Self::Record => tessera_formats::AnyRecord::is_decodable(did),
            Self::Mesh => crate::scene::SceneSource::of(did).is_some(),
        }
    }

    /// The view to open by default for `did`.
    #[must_use]
    pub fn default_for(did: u32) -> Self {
        match did >> 24 {
            0x40 | 0x41 => Self::Texture,
            0x25 => Self::Strings,
            0x70..=0x7F => Self::Props,
            0x06 | 0x04 | 0x1F | 0x47 | 0x20 | 0x31 | 0x30 => Self::Mesh,
            _ if Self::Record.applies_to(did) => Self::Record,
            _ => Self::Hex,
        }
    }
}

/// Surface shading of the mesh view (`--mesh-shading`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, ValueEnum)]
pub enum ShadingArg {
    /// Materials, lit.
    #[default]
    Lit,
    /// Base colour, unlit.
    Base,
    /// World-space normals as colours.
    Normals,
    /// UV checkerboard.
    Uv,
    /// Wireframe only (surfaces hidden).
    Wire,
}

/// Parses a colour given as 6 hex digits (`ff8800`, `#FF8800`).
pub fn parse_rgb_arg(s: &str) -> Result<[u8; 3], String> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!(
            "not a colour: {s} (give 6 hex digits, e.g. ff8800)"
        ));
    }
    let v = u32::from_str_radix(h, 16).map_err(|e| e.to_string())?;
    Ok([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// Parses a DID given as exactly 8 hex digits (`0x41000123`, `41000123`, `0x4100_0123`),
/// the search box's form for one DID ([`crate::query::parse_query`]). Shorter hex is a
/// prefix there, so it is refused here rather than read differently.
pub fn parse_did_arg(s: &str) -> Result<u32, String> {
    match crate::query::parse_query(s) {
        crate::query::DidQuery::Exact(did) => Ok(did),
        _ => Err(format!(
            "not a DID: {s} (give 8 hex digits, e.g. 0x41000123 or 41000123)"
        )),
    }
}

/// GUI asset browser for the archives of an install (read-only).
#[derive(Debug, Clone, Parser)]
#[command(name = "tessera-explorer", version, about)]
pub struct Args {
    /// Install directory. Default: TESSERA_GAME_DIR, then the saved directory, then
    /// automatic discovery.
    #[arg(long, value_name = "DIR")]
    pub game_dir: Option<PathBuf>,

    /// Language of string tables (as in client_local_LANGUAGE.dat; default English).
    #[arg(long)]
    pub language: Option<String>,

    /// DID to select at start-up (hex).
    #[arg(long, value_name = "DID", value_parser = parse_did_arg)]
    pub select: Option<u32>,

    /// View to open for --select (default: the natural view for the DID's type).
    #[arg(long, value_enum)]
    pub view: Option<ViewKind>,

    /// Render until the selection is ready, save a screenshot of the window as PNG and
    /// exit (for automated checks). Implies --no-persist.
    #[arg(long, value_name = "PNG")]
    pub screenshot: Option<PathBuf>,

    /// Give up waiting for the selection after this many seconds in --screenshot mode
    /// (the screenshot is still taken; the exit code is 2).
    #[arg(long, default_value_t = 120, value_name = "SECS")]
    pub screenshot_timeout: u64,

    /// In --screenshot mode, step through this many rows of the DID list first (one
    /// per frame, like holding the Down key) to exercise decoding while browsing; frame
    /// statistics are logged at exit.
    #[arg(long, default_value_t = 0, value_name = "ROWS")]
    pub browse: u32,

    /// Run this search at start-up, as if typed in the search box (hex DID, prefix or
    /// name).
    #[arg(long, value_name = "QUERY")]
    pub search: Option<String>,

    /// Do not write the settings file.
    #[arg(long)]
    pub no_persist: bool,

    /// Initial window width (logical pixels; needs --height).
    #[arg(long, requires = "height")]
    pub width: Option<f32>,

    /// Initial window height (logical pixels; needs --width).
    #[arg(long, requires = "width")]
    pub height: Option<f32>,

    /// Mesh view: surface shading at start-up.
    #[arg(long, value_enum, value_name = "SHADING")]
    pub mesh_shading: Option<ShadingArg>,

    /// Mesh view: start with the wireframe overlay on.
    #[arg(long)]
    pub mesh_wireframe: bool,

    /// Mesh view: start with vertex-normal lines on.
    #[arg(long)]
    pub mesh_normals: bool,

    /// Mesh view: let vertex colours (RGB) modulate the base colour. Off by default:
    /// their use is OPEN (mesh.md §5.4).
    #[arg(long, conflicts_with = "mesh_no_vertex_colors")]
    pub mesh_vertex_colors: bool,

    /// Mesh view: vertex colours off (the default; kept for older scripts).
    #[arg(long, hide = true)]
    pub mesh_no_vertex_colors: bool,

    /// Mesh view: let the materials' diffuse colour 2000 modulate the base colour. Off
    /// by default: its meaning is OPEN (material.md §19).
    #[arg(long)]
    pub mesh_diffuse_tint: bool,

    /// Mesh view: detail level to draw (0 = full detail; clamped per part).
    #[arg(long, value_name = "LEVEL")]
    pub mesh_lod: Option<u8>,

    /// Mesh view: which geometry link (objects) or entry (appearances) to show.
    #[arg(long, value_name = "N")]
    pub mesh_choice: Option<u8>,

    /// Mesh view: preview this dye colour (6 hex digits) on dyeable materials.
    #[arg(long, value_name = "RRGGBB", value_parser = parse_rgb_arg)]
    pub dye: Option<[u8; 3]>,

    /// Mesh view: select draw DRAW of part PART at start-up (`--mesh-select 0,1`).
    #[arg(long, value_name = "PART,DRAW", value_parser = parse_pair_arg)]
    pub mesh_select: Option<[u32; 2]>,

    /// Mesh view: yaw and pitch of the camera in degrees (`--mesh-angle 90,30`).
    #[arg(long, value_name = "YAW,PITCH", value_parser = parse_angles_arg)]
    pub mesh_angle: Option<[f32; 2]>,

    /// Font file (.ttf/.otf) for the UI. Default: TESSERA_EXPLORER_FONT, then the
    /// system's sans-serif font.
    #[arg(long, value_name = "FILE")]
    pub font: Option<PathBuf>,
}

/// Parses `a,b` as two unsigned integers.
pub fn parse_pair_arg(s: &str) -> Result<[u32; 2], String> {
    let (a, b) = s.split_once(',').ok_or_else(|| format!("not A,B: {s}"))?;
    let p = |t: &str| {
        t.trim()
            .parse::<u32>()
            .map_err(|_| format!("not a number: {t}"))
    };
    Ok([p(a)?, p(b)?])
}

/// Parses `yaw,pitch` in degrees.
pub fn parse_angles_arg(s: &str) -> Result<[f32; 2], String> {
    let (a, b) = s
        .split_once(',')
        .ok_or_else(|| format!("not YAW,PITCH: {s}"))?;
    let p = |t: &str| {
        t.trim()
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(|| format!("not a number: {t}"))
    };
    Ok([p(a)?, p(b)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn did_args() {
        assert_eq!(parse_did_arg("0x41000123"), Ok(0x4100_0123));
        assert_eq!(parse_did_arg("41000123"), Ok(0x4100_0123));
        assert_eq!(parse_did_arg("2500_0001"), Ok(0x2500_0001));
        assert!(parse_did_arg("").is_err());
        assert!(parse_did_arg("0x123456789").is_err());
        assert!(parse_did_arg("xyz").is_err());
        // Regression: short hex is a prefix in the search box, so it is no DID here,
        // and a sign is not hex.
        assert!(parse_did_arg("0x41").is_err());
        assert!(parse_did_arg("41").is_err());
        assert!(parse_did_arg("+41").is_err());
        assert!(parse_did_arg("+4100_0123").is_err());
        assert!(parse_did_arg("0x41*").is_err());
    }

    #[test]
    fn window_size_needs_both_sides() {
        assert!(Args::try_parse_from(["x", "--width", "800"]).is_err());
        assert!(Args::try_parse_from(["x", "--height", "600"]).is_err());
        let a = Args::try_parse_from(["x", "--width", "800", "--height", "600"]).unwrap();
        assert_eq!((a.width, a.height), (Some(800.0), Some(600.0)));
    }

    #[test]
    fn views() {
        assert_eq!(ViewKind::default_for(0x4100_0001), ViewKind::Texture);
        assert_eq!(ViewKind::default_for(0x2500_0001), ViewKind::Strings);
        assert_eq!(ViewKind::default_for(0x7900_0001), ViewKind::Props);
        assert_eq!(ViewKind::default_for(0x2800_0000), ViewKind::Record);
        assert_eq!(ViewKind::default_for(0x0600_0001), ViewKind::Mesh);
        assert_eq!(ViewKind::default_for(0x4700_0001), ViewKind::Mesh);
        assert_eq!(ViewKind::default_for(0x0100_0001), ViewKind::Hex);
        assert!(ViewKind::Mesh.applies_to(0x7900_0001));
        assert!(ViewKind::Mesh.applies_to(0x3100_0001));
        assert!(!ViewKind::Mesh.applies_to(0x4100_0001));
        assert!(ViewKind::Icon.applies_to(0x7000_0001));
        assert!(!ViewKind::Strings.applies_to(0x4100_0001));
        for v in ViewKind::ALL {
            assert!(!v.title().is_empty());
        }
    }

    #[test]
    fn colour_and_angle_args() {
        assert_eq!(parse_rgb_arg("ff8800"), Ok([0xFF, 0x88, 0x00]));
        assert_eq!(parse_rgb_arg("#0A0b0C"), Ok([10, 11, 12]));
        assert!(parse_rgb_arg("fff").is_err());
        assert!(parse_rgb_arg("gg0000").is_err());
        assert!(parse_rgb_arg("+f8800").is_err());
        assert_eq!(parse_angles_arg("90, -10.5"), Ok([90.0, -10.5]));
        assert!(parse_angles_arg("90").is_err());
        assert!(parse_angles_arg("nan,0").is_err());
        assert_eq!(parse_pair_arg("0, 3"), Ok([0, 3]));
        assert!(parse_pair_arg("-1,3").is_err());
        let a = Args::parse_from([
            "x",
            "--view",
            "mesh",
            "--mesh-shading",
            "uv",
            "--mesh-wireframe",
            "--mesh-lod",
            "2",
            "--dye",
            "336699",
        ]);
        assert_eq!(a.view, Some(ViewKind::Mesh));
        assert!(!a.mesh_vertex_colors && !a.mesh_diffuse_tint);
        let b = Args::parse_from(["x", "--mesh-vertex-colors", "--mesh-diffuse-tint"]);
        assert!(b.mesh_vertex_colors && b.mesh_diffuse_tint);
        assert!(Args::parse_from(["x", "--mesh-no-vertex-colors"]).mesh_no_vertex_colors);
        assert!(
            Args::try_parse_from(["x", "--mesh-vertex-colors", "--mesh-no-vertex-colors"]).is_err()
        );
        assert_eq!(a.mesh_shading, Some(ShadingArg::Uv));
        assert!(a.mesh_wireframe && !a.mesh_normals);
        assert_eq!((a.mesh_lod, a.dye), (Some(2), Some([0x33, 0x66, 0x99])));
    }

    #[test]
    fn args_parse() {
        let a = Args::parse_from([
            "x",
            "--select",
            "0x41000001",
            "--view",
            "texture",
            "--screenshot",
            "/tmp/a.png",
        ]);
        assert_eq!(a.select, Some(0x4100_0001));
        assert_eq!(a.view, Some(ViewKind::Texture));
        assert!(a.screenshot.is_some());
    }
}
