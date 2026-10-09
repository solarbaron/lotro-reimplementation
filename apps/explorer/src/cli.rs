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
    /// Character viewer and animation browser (M4): available for every record; a 0x47,
    /// 0x04 or weenie record becomes the figure, a 0x05 clip plays, an item is previewed.
    Character,
    /// A landblock (any landblock-family record, M4.5): terrain map, generated scenery,
    /// statics, cells, landblock water and the collision summary.
    Landblock,
    /// Sky descriptions (0x15) and day descriptions (0x1A, M4.5): the day-description
    /// choice and the blended keyframe timeline over the game day.
    Sky,
    /// Gameplay cards (items, skills, effects, traits, creatures, quests, vendors, loot,
    /// recipes, map data, particles) of game objects, their property sets and the gameplay
    /// records with their own layout ([`crate::gameplay`]).
    Gameplay,
}

impl ViewKind {
    /// Every view, in tab order.
    pub const ALL: [ViewKind; 12] = [
        ViewKind::Info,
        ViewKind::Gameplay,
        ViewKind::Mesh,
        ViewKind::Landblock,
        ViewKind::Sky,
        ViewKind::Character,
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
            Self::Character => "Character",
            Self::Landblock => "Landblock",
            Self::Sky => "Sky",
            Self::Gameplay => "Gameplay",
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
            Self::Record => crate::decoders::is_record_decodable(did),
            Self::Mesh => crate::scene::SceneSource::of(did).is_some(),
            Self::Character => true,
            Self::Landblock => crate::world::landblock_of(did).is_some(),
            Self::Sky => matches!(prefix, 0x15 | 0x1A),
            Self::Gameplay => crate::gameplay::applies_to(did),
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
            0x05 => Self::Character,
            0x80 if Self::Landblock.applies_to(did) => Self::Landblock,
            0x15 | 0x1A => Self::Sky,
            0x66 | 0x01 => Self::Mesh,
            0x14 | 0x39 | 0x44 => Self::Gameplay,
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

/// A list given as one option value (clap would otherwise treat a `Vec` as repeated
/// values).
#[derive(Debug, Clone, PartialEq)]
pub struct Listed<T>(pub Vec<T>);

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

    /// Character view: the figure, `SPECIES,SEX` (`human,m`, labels from the race table)
    /// or a 0x47 / 0x70–0x76 / 0x04 DID. Implies `--view character`.
    #[arg(long, value_name = "FIGURE", value_parser = crate::charspec::parse_figure)]
    pub character: Option<crate::charspec::FigureArg>,

    /// Character view: wear or hold an item, `DID[@dye=CODE][@dye2=CODE][@sheathed]
    /// [@slot=primary|secondary|ranged|BITS]` (repeatable, at most 32 times).
    #[arg(long, value_name = "ITEM", value_parser = crate::charspec::parse_item)]
    pub wear: Vec<crate::charspec::ItemArg>,

    /// Character view: the clip, `idle`, `walk`, `run` (as the script tables select),
    /// `none` (reference pose) or a 0x05 DID.
    #[arg(long, value_name = "CLIP", value_parser = crate::charspec::parse_clip)]
    pub clip: Option<crate::charspec::ClipArg>,

    /// Character view: pause at this clip play time in seconds (screenshots default to 0).
    #[arg(long, value_name = "SECS", value_parser = crate::charspec::parse_time)]
    pub time: Option<f32>,

    /// Character view: bone-slider bytes as hex pairs, one per visible slider.
    #[arg(long, value_name = "HEX", value_parser = |s: &str| crate::charspec::parse_sliders(s).map(Listed))]
    pub sliders: Option<Listed<u8>>,

    /// Character view: draw the bone overlay.
    #[arg(long)]
    pub bones: bool,

    /// Character view: draw the root-motion trails.
    #[arg(long)]
    pub trail: bool,

    /// Render the character view headless into a contact sheet (PNG) and exit, without a
    /// window: one tile per camera angle (rows, `--angles`) and play time (columns,
    /// `--times`). Uses `--character`, `--wear`, `--clip`, `--sliders`, `--bones`,
    /// `--trail` and `--mesh-wireframe`.
    #[arg(long, value_name = "PNG")]
    pub sheet: Option<PathBuf>,

    /// Sheet: play times in seconds, comma-separated (columns).
    #[arg(long, value_name = "T,T,…", value_parser = |s: &str| crate::charspec::parse_times(s).map(Listed))]
    pub times: Option<Listed<f32>>,

    /// Sheet: camera angles `YAW,PITCH;YAW,PITCH…` in degrees (rows).
    #[arg(long, value_name = "Y,P;Y,P…", value_parser = |s: &str| crate::charspec::parse_angle_list(s).map(Listed))]
    pub angles: Option<Listed<[f32; 2]>>,

    /// Sheet: frame the sphere `HEIGHT,RADIUS` (metres, on the figure's vertical axis)
    /// instead of the whole figure, e.g. `1.65,0.3` for a head.
    #[arg(long, value_name = "Z,R", value_parser = crate::charspec::parse_focus)]
    pub focus: Option<[f32; 2]>,

    /// Sheet: tile size `WIDTHxHEIGHT` in pixels.
    #[arg(long, value_name = "WxH", value_parser = crate::charspec::parse_tile)]
    pub tile: Option<[u32; 2]>,

    /// Particle preview of a particle-system descriptor (0x39): opens its gameplay card
    /// with the preview, or, with `--sheet`, renders the preview headless into a contact
    /// sheet: one tile per camera angle (rows, `--angles`) and simulated time in seconds
    /// (columns, `--times`), tile size `--tile`, seed `--particle-seed`.
    #[arg(long, value_name = "DID", value_parser = parse_did_arg)]
    pub particles: Option<u32>,

    /// Particle preview and sheet: the seed of the simulation's random stream.
    #[arg(long, value_name = "N", default_value_t = 1)]
    pub particle_seed: u32,

    /// Hide the archive tree, the DID list and the character view's inspector (character
    /// view screenshots get the window's width; the 3D view keeps its inspector).
    #[arg(long)]
    pub hide_panels: bool,

    /// Font file (.ttf/.otf) for the UI. Default: TESSERA_EXPLORER_FONT, then the
    /// system's sans-serif font.
    #[arg(long, value_name = "FILE")]
    pub font: Option<PathBuf>,
}

impl Args {
    /// Parses `words` (the program name first) like [`clap::Parser::try_parse_from`], then
    /// applies the checks clap cannot express: at most
    /// [`crate::charspec::MAX_ITEMS`] `--wear` options.
    ///
    /// # Errors
    ///
    /// A clap error (printed and exited on by `main`).
    pub fn try_parse_checked<I, T>(words: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        use clap::CommandFactory;
        let args = Self::try_parse_from(words)?;
        if args.wear.len() > crate::charspec::MAX_ITEMS {
            return Err(Self::command().error(
                clap::error::ErrorKind::TooManyValues,
                format!(
                    "--wear is given {} times; at most {} items are accepted",
                    args.wear.len(),
                    crate::charspec::MAX_ITEMS
                ),
            ));
        }
        Ok(args)
    }

    /// The character-view options of the command line.
    #[must_use]
    pub fn character_args(&self) -> crate::charspec::CharacterArgs {
        crate::charspec::CharacterArgs {
            figure: self.character.clone(),
            items: self.wear.clone(),
            clip: self.clip,
            time: self.time,
            sliders: self.sliders.clone().map(|l| l.0),
            bones: self.bones,
            trail: self.trail,
        }
    }
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
        // M4.5: cells and collision meshes open in the 3D view, landblocks in the
        // landblock view, sky and day descriptions in the sky view.
        assert_eq!(ViewKind::default_for(0x0100_0001), ViewKind::Mesh);
        assert_eq!(ViewKind::default_for(0x6600_0001), ViewKind::Mesh);
        assert_eq!(ViewKind::default_for(0x8001_3040), ViewKind::Landblock);
        assert_eq!(ViewKind::default_for(0x8101_3040), ViewKind::Hex);
        assert_eq!(ViewKind::default_for(0x1500_0007), ViewKind::Sky);
        assert_eq!(ViewKind::default_for(0x1A00_0001), ViewKind::Sky);
        assert_eq!(ViewKind::default_for(0x1900_0000), ViewKind::Record);
        assert_eq!(ViewKind::default_for(0x0200_0001), ViewKind::Record);
        assert!(ViewKind::Record.applies_to(0x0100_0001));
        assert!(ViewKind::Mesh.applies_to(0x8001_3040));
        assert!(!ViewKind::Sky.applies_to(0x1900_0000));
        assert!(ViewKind::Mesh.applies_to(0x7900_0001));
        assert!(ViewKind::Mesh.applies_to(0x3100_0001));
        assert!(!ViewKind::Mesh.applies_to(0x4100_0001));
        assert!(ViewKind::Icon.applies_to(0x7000_0001));
        assert!(!ViewKind::Strings.applies_to(0x4100_0001));
        for v in ViewKind::ALL {
            assert!(!v.title().is_empty());
        }
        assert!(ViewKind::Character.applies_to(0x2800_0000));
        assert_eq!(ViewKind::default_for(0x0500_0001), ViewKind::Character);
        assert!(ViewKind::Gameplay.applies_to(0x7000_0001));
        assert!(ViewKind::Gameplay.applies_to(0x7900_0001));
        assert!(!ViewKind::Gameplay.applies_to(0x4100_0001));
        assert_eq!(ViewKind::default_for(0x3900_0001), ViewKind::Gameplay);
        assert_eq!(ViewKind::default_for(0x7000_0001), ViewKind::Props);
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
    fn character_args_parse() {
        let a = Args::parse_from([
            "x",
            "--character",
            "human,f",
            "--wear",
            "0x790006C3@dye=0.5",
            "--wear",
            "0x7900066F@sheathed",
            "--clip",
            "walk",
            "--time",
            "0.25",
            "--bones",
        ]);
        let c = a.character_args();
        assert!(c.any());
        assert_eq!(c.items.len(), 2);
        assert_eq!(c.time, Some(0.25));
        assert!(c.bones && !c.trail);
        assert!(Args::try_parse_from(["x", "--character", "nothing"]).is_err());
        assert!(Args::try_parse_from(["x", "--wear", "0x06000001"]).is_err());
        assert!(Args::try_parse_from(["x", "--clip", "jump"]).is_err());
        assert!(Args::try_parse_from(["x", "--time", "-2"]).is_err());
        assert!(!Args::parse_from(["x"]).character_args().any());
        let s = Args::parse_from([
            "x",
            "--sheet",
            "/tmp/s.png",
            "--times",
            "0,0.5",
            "--angles",
            "160,10;90,5",
            "--tile",
            "320x400",
        ]);
        assert_eq!(s.times, Some(Listed(vec![0.0, 0.5])));
        assert_eq!(s.angles.map(|a| a.0.len()), Some(2));
        assert_eq!(s.tile, Some([320, 400]));
        let s = Args::parse_from(["x", "--sliders", "80ff"]);
        assert_eq!(s.character_args().sliders, Some(vec![0x80, 0xFF]));
        assert!(Args::try_parse_from(["x", "--tile", "1x1"]).is_err());
        // At most MAX_ITEMS --wear options (regression: clap took any number).
        let mut words = vec!["x".to_owned()];
        for _ in 0..crate::charspec::MAX_ITEMS {
            words.extend(["--wear".to_owned(), "0x790006C3".to_owned()]);
        }
        assert!(Args::try_parse_checked(words.clone()).is_ok());
        words.extend(["--wear".to_owned(), "0x790006C3".to_owned()]);
        let e = Args::try_parse_checked(words).unwrap_err();
        assert_eq!(e.kind(), clap::error::ErrorKind::TooManyValues);
        // 0x77/0x78 records are no items.
        assert!(Args::try_parse_from(["x", "--wear", "0x7800A9F1"]).is_err());
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
        let p = Args::parse_from(["x", "--particles", "0x39000001", "--particle-seed", "7"]);
        assert_eq!((p.particles, p.particle_seed), (Some(0x3900_0001), 7));
        assert_eq!(Args::parse_from(["x"]).particle_seed, 1);
        assert!(Args::try_parse_from(["x", "--particles", "nope"]).is_err());
    }
}
