// SPDX-License-Identifier: GPL-3.0-or-later
//! The game-UI preferences of the client (ROADMAP M5), kept in the `[UI]` section of the
//! settings file.
//!
//! The section is stored in [`Settings::extra`] under the key [`UI_SECTION`], so files
//! written by this version round-trip unchanged through older ones, and unknown keys inside
//! the section are kept. Every key is optional; a missing or malformed key takes its default
//! and a malformed one is reported by [`UiPrefs::from_settings`].
//!
//! ```toml
//! [UI]
//! UserSkin = "My Skin"          # skin-definition.md §10 D6; empty = no skin
//! skins_root = "/path/to/skins" # D1: another skins root instead of the default
//! plugins_dir = "/path/Plugins" # ARCHITECTURE §18.2: the user's Plugins folder (read-only)
//! walker_hud = "on"             # walker.md §6: on | off
//! walker_minimap = "auto"       # walker.md §6: auto | on | off
//! ui_cursor = "os"              # decision 0015 §2.5: os | software
//! ui_scale_filter = "linear"    # decision 0015 §2.2: linear | nearest
//! ui_scale_global = 1.0         # UIScale_Global (ui-behaviour.md §9.1)
//! ui_scale_cursor = 1.0         # UIScale_Cursor (ui-behaviour.md §9.3)
//! tooltip_enable = true         # TooltipEnable
//! tooltip_delay = 0.25          # TooltipDelay, seconds
//! canvas_mib = 128              # decision 0015 U5
//! texture_mib = 256
//! image_lru_mib = 64
//! image_pinned_mib = 512
//!
//! [UI.chat]                     # chat-window.md §10 items 1 and 7
//! opacity = 25                  # GOP_ChatOpacity override, 0–100
//! ```

use std::path::PathBuf;

use crate::Settings;

/// Key of the section in [`Settings::extra`] (skin-definition.md §10 D6 names `[UI]`).
pub const UI_SECTION: &str = "UI";
/// Key of the selected skin's name (skin-definition.md §10 D6).
pub const USER_SKIN_KEY: &str = "UserSkin";
/// Longest stored skin name or path, in bytes (longer values are ignored with a warning).
pub const MAX_UI_TEXT_BYTES: usize = 1024;
/// Range of `ui_scale_global` (larger panels on large screens; ui-behaviour.md §9.1 uses
/// 0.4–2 in practice).
pub const SCALE_RANGE: (f32, f32) = (0.25, 4.0);
/// Range of each memory budget, in MiB.
pub const BUDGET_MIB_RANGE: (u64, u64) = (1, 16_384);

/// `walker_minimap` (walker.md §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MinimapMode {
    /// Off with a game UI attached, on without one.
    #[default]
    Auto,
    /// Always drawn (under the game UI when one is attached).
    On,
    /// Never drawn.
    Off,
}

impl MinimapMode {
    /// Whether the walker minimap is drawn.
    #[must_use]
    pub fn shown(self, ui_attached: bool) -> bool {
        match self {
            Self::Auto => !ui_attached,
            Self::On => true,
            Self::Off => false,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::On => "on",
            Self::Off => "off",
        }
    }

    /// Parses `auto`, `on` or `off` (ASCII case ignored).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "on" => Some(Self::On),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}

/// `ui_cursor` (decision 0015 §2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorMode {
    /// The OS cursor (no frame of latency).
    #[default]
    Os,
    /// Drawn by the renderer as the last draw of the frame.
    Software,
}

impl CursorMode {
    fn name(self) -> &'static str {
        match self {
            Self::Os => "os",
            Self::Software => "software",
        }
    }

    /// Parses `os` or `software` (ASCII case ignored).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "os" => Some(Self::Os),
            "software" => Some(Self::Software),
            _ => None,
        }
    }
}

/// `ui_scale_filter` (decision 0015 §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScaleFilter {
    /// Bilinear.
    #[default]
    Linear,
    /// Nearest texel.
    Nearest,
}

impl ScaleFilter {
    fn name(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Nearest => "nearest",
        }
    }

    /// Parses `linear` or `nearest` (ASCII case ignored).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "linear" => Some(Self::Linear),
            "nearest" => Some(Self::Nearest),
            _ => None,
        }
    }
}

/// Chat-window preferences (`[UI.chat]`, chat-window.md §10 items 1 and 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChatPrefs {
    /// Override of the window opacity step (`GOP_ChatOpacity`, 0–100); `None` = the
    /// install's default.
    pub opacity: Option<u8>,
}

/// The game-UI preferences (module docs).
#[derive(Debug, Clone, PartialEq)]
pub struct UiPrefs {
    /// The selected skin's name; empty = no skin (D6).
    pub user_skin: String,
    /// Another skins root (D1).
    pub skins_root: Option<PathBuf>,
    /// The user's Plugins folder, opened read-only.
    pub plugins_dir: Option<PathBuf>,
    /// `walker_hud`: the walker's HUD text (walker.md §6).
    pub walker_hud: bool,
    /// `walker_minimap` (walker.md §6).
    pub walker_minimap: MinimapMode,
    /// `ui_cursor`.
    pub cursor: CursorMode,
    /// `ui_scale_filter`.
    pub scale_filter: ScaleFilter,
    /// `ui_scale_global`.
    pub scale_global: f32,
    /// `ui_scale_cursor`.
    pub scale_cursor: f32,
    /// `tooltip_enable`.
    pub tooltip_enable: bool,
    /// `tooltip_delay`, seconds (0–10).
    pub tooltip_delay: f32,
    /// U5: CPU canvases, MiB.
    pub canvas_mib: u64,
    /// U5: GPU surface textures, MiB.
    pub texture_mib: u64,
    /// U5: unpinned decoded images (LRU), MiB.
    pub image_lru_mib: u64,
    /// U5: pinned decoded images, hard cap, MiB.
    pub image_pinned_mib: u64,
    /// `[UI.chat]`.
    pub chat: ChatPrefs,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            user_skin: String::new(),
            skins_root: None,
            plugins_dir: None,
            walker_hud: true,
            walker_minimap: MinimapMode::Auto,
            cursor: CursorMode::Os,
            scale_filter: ScaleFilter::Linear,
            scale_global: 1.0,
            scale_cursor: 1.0,
            tooltip_enable: true,
            tooltip_delay: 0.25,
            canvas_mib: 128,
            texture_mib: 256,
            image_lru_mib: 64,
            image_pinned_mib: 512,
            chat: ChatPrefs::default(),
        }
    }
}

fn on_off(v: &toml::Value) -> Option<bool> {
    match v {
        toml::Value::Boolean(b) => Some(*b),
        toml::Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "on" | "true" | "yes" => Some(true),
            "off" | "false" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn number(v: &toml::Value) -> Option<f64> {
    match v {
        toml::Value::Float(f) if f.is_finite() => Some(*f),
        toml::Value::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

fn text(v: &toml::Value) -> Option<&str> {
    v.as_str().filter(|s| s.len() <= MAX_UI_TEXT_BYTES)
}

impl UiPrefs {
    /// Reads the `[UI]` section. Never fails: a malformed value keeps its default and adds
    /// a warning (at most one per key).
    #[must_use]
    pub fn from_settings(settings: &Settings) -> (Self, Vec<String>) {
        let mut p = Self::default();
        let mut warn = Vec::new();
        let Some(section) = settings.extra.get(UI_SECTION) else {
            return (p, warn);
        };
        let Some(t) = section.as_table() else {
            warn.push(format!("[{UI_SECTION}] is not a table; using defaults"));
            return (p, warn);
        };
        let mut bad = |key: &str| warn.push(format!("[{UI_SECTION}] {key}: invalid value ignored"));
        for (key, v) in t {
            match key.as_str() {
                USER_SKIN_KEY => match text(v) {
                    Some(s) => p.user_skin = s.to_owned(),
                    None => bad(key),
                },
                "skins_root" => match text(v) {
                    Some(s) if !s.is_empty() => p.skins_root = Some(PathBuf::from(s)),
                    Some(_) => {}
                    None => bad(key),
                },
                "plugins_dir" => match text(v) {
                    Some(s) if !s.is_empty() => p.plugins_dir = Some(PathBuf::from(s)),
                    Some(_) => {}
                    None => bad(key),
                },
                "walker_hud" => match on_off(v) {
                    Some(b) => p.walker_hud = b,
                    None => bad(key),
                },
                "walker_minimap" => match v.as_str().and_then(MinimapMode::parse) {
                    Some(m) => p.walker_minimap = m,
                    None => bad(key),
                },
                "ui_cursor" => match v.as_str().and_then(CursorMode::parse) {
                    Some(m) => p.cursor = m,
                    None => bad(key),
                },
                "ui_scale_filter" => match v.as_str().and_then(ScaleFilter::parse) {
                    Some(m) => p.scale_filter = m,
                    None => bad(key),
                },
                "ui_scale_global" => match number(v) {
                    Some(f)
                        if (f64::from(SCALE_RANGE.0)..=f64::from(SCALE_RANGE.1)).contains(&f) =>
                    {
                        p.scale_global = f as f32;
                    }
                    _ => bad(key),
                },
                "ui_scale_cursor" => match number(v) {
                    // ui-behaviour.md §9.3 resets values ≤ 0 or > 4 to 1 itself.
                    Some(f) => p.scale_cursor = f as f32,
                    None => bad(key),
                },
                "tooltip_enable" => match on_off(v) {
                    Some(b) => p.tooltip_enable = b,
                    None => bad(key),
                },
                "tooltip_delay" => match number(v) {
                    Some(f) if (0.0..=10.0).contains(&f) => p.tooltip_delay = f as f32,
                    _ => bad(key),
                },
                "canvas_mib" | "texture_mib" | "image_lru_mib" | "image_pinned_mib" => {
                    match v.as_integer().and_then(|i| u64::try_from(i).ok()) {
                        Some(n) if (BUDGET_MIB_RANGE.0..=BUDGET_MIB_RANGE.1).contains(&n) => {
                            match key.as_str() {
                                "canvas_mib" => p.canvas_mib = n,
                                "texture_mib" => p.texture_mib = n,
                                "image_lru_mib" => p.image_lru_mib = n,
                                _ => p.image_pinned_mib = n,
                            }
                        }
                        _ => bad(key),
                    }
                }
                "chat" => match v.as_table() {
                    Some(c) => {
                        if let Some(o) = c.get("opacity") {
                            match o.as_integer().and_then(|i| u8::try_from(i).ok()) {
                                Some(n) if n <= 100 => p.chat.opacity = Some(n),
                                _ => bad("chat.opacity"),
                            }
                        }
                    }
                    None => bad(key),
                },
                // Unknown keys are kept by `store` (they stay in the table).
                _ => {}
            }
        }
        (p, warn)
    }

    /// Writes the section into `settings`, replacing only the keys this type knows; other
    /// keys of the section are kept.
    pub fn store(&self, settings: &mut Settings) {
        let entry = settings
            .extra
            .entry(UI_SECTION.to_owned())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if !entry.is_table() {
            *entry = toml::Value::Table(toml::Table::new());
        }
        let Some(t) = entry.as_table_mut() else {
            return;
        };
        let s = |v: &str| toml::Value::String(v.to_owned());
        t.insert(USER_SKIN_KEY.into(), s(&self.user_skin));
        let path = |t: &mut toml::Table, key: &str, p: &Option<PathBuf>| match p
            .as_ref()
            .and_then(|p| p.to_str())
        {
            Some(v) => {
                t.insert(key.into(), toml::Value::String(v.to_owned()));
            }
            None => {
                t.remove(key);
            }
        };
        path(t, "skins_root", &self.skins_root);
        path(t, "plugins_dir", &self.plugins_dir);
        t.insert(
            "walker_hud".into(),
            s(if self.walker_hud { "on" } else { "off" }),
        );
        t.insert("walker_minimap".into(), s(self.walker_minimap.name()));
        t.insert("ui_cursor".into(), s(self.cursor.name()));
        t.insert("ui_scale_filter".into(), s(self.scale_filter.name()));
        t.insert(
            "ui_scale_global".into(),
            toml::Value::Float(f64::from(self.scale_global)),
        );
        t.insert(
            "ui_scale_cursor".into(),
            toml::Value::Float(f64::from(self.scale_cursor)),
        );
        t.insert(
            "tooltip_enable".into(),
            toml::Value::Boolean(self.tooltip_enable),
        );
        t.insert(
            "tooltip_delay".into(),
            toml::Value::Float(f64::from(self.tooltip_delay)),
        );
        for (k, v) in [
            ("canvas_mib", self.canvas_mib),
            ("texture_mib", self.texture_mib),
            ("image_lru_mib", self.image_lru_mib),
            ("image_pinned_mib", self.image_pinned_mib),
        ] {
            t.insert(
                k.into(),
                toml::Value::Integer(i64::try_from(v).unwrap_or(i64::MAX)),
            );
        }
        let mut chat = t
            .get("chat")
            .and_then(toml::Value::as_table)
            .cloned()
            .unwrap_or_default();
        match self.chat.opacity {
            Some(o) => {
                chat.insert("opacity".into(), toml::Value::Integer(i64::from(o)));
            }
            None => {
                chat.remove("opacity");
            }
        }
        if chat.is_empty() {
            t.remove("chat");
        } else {
            t.insert("chat".into(), toml::Value::Table(chat));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_without_a_section() {
        let (p, w) = UiPrefs::from_settings(&Settings::default());
        assert_eq!(p, UiPrefs::default());
        assert!(w.is_empty());
        assert!(!p.walker_minimap.shown(true));
        assert!(p.walker_minimap.shown(false));
    }

    #[test]
    fn round_trip_keeps_unknown_keys() {
        let text = r#"
[UI]
UserSkin = "Test Skin"
walker_hud = "off"
walker_minimap = "on"
ui_cursor = "software"
ui_scale_global = 1.5
future_key = 7

[UI.chat]
opacity = 40
"#;
        let s = Settings::from_toml_str(text).expect("parse");
        let (p, w) = UiPrefs::from_settings(&s);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(p.user_skin, "Test Skin");
        assert!(!p.walker_hud);
        assert_eq!(p.walker_minimap, MinimapMode::On);
        assert_eq!(p.cursor, CursorMode::Software);
        assert!((p.scale_global - 1.5).abs() < 1e-6);
        assert_eq!(p.chat.opacity, Some(40));
        let mut s2 = s.clone();
        p.store(&mut s2);
        let back = Settings::from_toml_str(&s2.to_toml_str_for_test()).expect("re-parse");
        let (p2, _) = UiPrefs::from_settings(&back);
        assert_eq!(p, p2);
        let ui = back
            .extra
            .get(UI_SECTION)
            .and_then(|v| v.as_table())
            .unwrap();
        assert_eq!(
            ui.get("future_key").and_then(toml::Value::as_integer),
            Some(7)
        );
    }

    #[test]
    fn malformed_values_warn_and_keep_defaults() {
        let text = r#"
[UI]
walker_hud = 3
walker_minimap = "sometimes"
ui_scale_global = 99.0
canvas_mib = 0
tooltip_delay = -1
"#;
        let s = Settings::from_toml_str(text).expect("parse");
        let (p, w) = UiPrefs::from_settings(&s);
        assert_eq!(w.len(), 5, "{w:?}");
        assert_eq!(p, UiPrefs::default());
    }

    impl Settings {
        fn to_toml_str_for_test(&self) -> String {
            self.to_toml_string().expect("serialize")
        }
    }
}
