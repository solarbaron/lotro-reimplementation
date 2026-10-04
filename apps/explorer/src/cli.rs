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
}

impl ViewKind {
    /// Every view, in tab order.
    pub const ALL: [ViewKind; 7] = [
        ViewKind::Info,
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
        }
    }

    /// The view to open by default for `did`.
    #[must_use]
    pub fn default_for(did: u32) -> Self {
        match did >> 24 {
            0x40 | 0x41 => Self::Texture,
            0x25 => Self::Strings,
            0x70..=0x7F => Self::Props,
            _ if Self::Record.applies_to(did) => Self::Record,
            _ => Self::Hex,
        }
    }
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

    /// Font file (.ttf/.otf) for the UI. Default: TESSERA_EXPLORER_FONT, then the
    /// system's sans-serif font.
    #[arg(long, value_name = "FILE")]
    pub font: Option<PathBuf>,
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
        assert_eq!(ViewKind::default_for(0x0600_0001), ViewKind::Hex);
        assert!(ViewKind::Icon.applies_to(0x7000_0001));
        assert!(!ViewKind::Strings.applies_to(0x4100_0001));
        for v in ViewKind::ALL {
            assert!(!v.title().is_empty());
        }
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
