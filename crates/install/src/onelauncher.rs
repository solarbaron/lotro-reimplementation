// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only extraction of a few keys from third-party launcher game configs.
//!
//! Spec-Ref: docs/specs/install-layout.md#562-onelauncher-configs-reported-ol
//!
//! Only these keys are read: top-level `game_directory`,
//! `game_settings_directory`, `game_type`, and `user_prefix_path` in the
//! `[wine]` table. This is not a general TOML parser: it understands
//! `key = "basic string"` and `key = 'literal string'` lines and table
//! headers, and ignores everything else. The file is never written.

use std::path::{Path, PathBuf};

use crate::fsutil;

/// Largest config file read.
pub const MAX_CONFIG_SIZE: u64 = 64 * 1024;

/// Keys of interest from one game config.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LauncherGameConfig {
    /// The config file.
    pub path: PathBuf,
    /// `game_directory`.
    pub game_directory: Option<PathBuf>,
    /// `game_settings_directory`.
    pub game_settings_directory: Option<PathBuf>,
    /// `game_type`, e.g. the target game's tag or the sibling game's tag.
    pub game_type: Option<String>,
    /// `[wine] user_prefix_path`.
    pub wine_prefix: Option<PathBuf>,
}

fn unquote(v: &str) -> Option<String> {
    let v = v.trim();
    if let Some(rest) = v.strip_prefix('\'') {
        let end = rest.find('\'')?;
        return Some(rest[..end].to_owned());
    }
    let rest = v.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                'U' => {
                    let hex: String = chars.by_ref().take(8).collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

/// Extract the keys of interest from config text.
pub fn parse_config(text: &str) -> LauncherGameConfig {
    let mut cfg = LauncherGameConfig::default();
    let mut table = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            table = line
                .trim_start_matches('[')
                .split(']')
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let Some(value) = unquote(value) else {
            continue;
        };
        match (table.as_str(), key) {
            ("", "game_directory") => cfg.game_directory = Some(value.into()),
            ("", "game_settings_directory") => cfg.game_settings_directory = Some(value.into()),
            ("", "game_type") => cfg.game_type = Some(value),
            ("wine", "user_prefix_path") => cfg.wine_prefix = Some(value.into()),
            _ => {}
        }
    }
    cfg
}

/// Read every `<data>/onelauncher/games/*/config.toml`.
pub fn read_configs(data_home: &Path) -> Vec<LauncherGameConfig> {
    let Some(games) = fsutil::resolve_path_ci(data_home, &["onelauncher", "games"]) else {
        return Vec::new();
    };
    let Ok(mut dirs) = fsutil::list_dir(&games) else {
        return Vec::new();
    };
    dirs.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out = Vec::new();
    for d in dirs {
        let Some(file) = fsutil::resolve_ci(&d.path, "config.toml") else {
            continue;
        };
        match fsutil::read_capped(&file, MAX_CONFIG_SIZE) {
            Ok(bytes) => {
                let mut cfg = parse_config(&String::from_utf8_lossy(&bytes));
                cfg.path = file;
                out.push(cfg);
            }
            Err(e) => tracing::debug!(error = %e.class(), "cannot read launcher config"),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_keys() {
        let cfg = parse_config(
            "game_directory = \"/g/dir \\\"x\\\"\"\n\
             game_settings_directory = '/s/dir'\n\
             game_type = \"LOTRO\"\n\
             high_res_enabled = true\n\
             [wine]\n\
             user_prefix_path = \"/p\"\n\
             [other]\n\
             game_directory = \"/ignored\"\n",
        );
        assert_eq!(cfg.game_directory, Some(PathBuf::from("/g/dir \"x\"")));
        assert_eq!(cfg.game_settings_directory, Some(PathBuf::from("/s/dir")));
        assert_eq!(cfg.game_type.as_deref(), Some("LOTRO"));
        assert_eq!(cfg.wine_prefix, Some(PathBuf::from("/p")));
    }

    proptest::proptest! {
        #[test]
        fn never_panics(s in "\\PC{0,200}") {
            let _ = parse_config(&s);
        }
    }
}
