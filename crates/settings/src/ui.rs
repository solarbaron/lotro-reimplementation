// SPDX-License-Identifier: GPL-3.0-or-later
//! The game-UI preferences of the client (ROADMAP M5), kept in the `[UI]` section of the
//! settings file.
//!
//! The section is stored in [`Settings::extra`] under the key [`UI_SECTION`], so files
//! written by this version round-trip unchanged through older ones, and unknown keys inside
//! the section are kept. Every key is optional; a missing or malformed key takes its default
//! and a malformed one is reported by [`UiPrefs::from_settings`].
//!
//! The UI-scaling keys of `ui-scaling.md` §4.4 and §7.1 (decision 0040) live here:
//! `ui_scale` (the user scale *S*), `ui_safe_aspect` (the safe area's width), `ui_art_filter`,
//! `ui_text_scale`, `ui_text_source`, the saved placements `[UI.panels]` and the per-window
//! factors `[UI.group_scale]`. The older `ui_scale_global` key is migrated once into
//! `ui_scale` and then removed (§4.4, §7.7: Tessera has no global factor any more).
//!
//! ```toml
//! [UI]
//! UserSkin = "My Skin"          # skin-definition.md §10 D6; empty = no skin
//! skins_root = "/path/to/skins" # D1: another skins root instead of the default
//! plugins_dir = "/path/Plugins" # ARCHITECTURE §18.2: the user's Plugins folder (read-only)
//! walker_hud = "on"             # walker.md §6: on | off
//! walker_minimap = "auto"       # walker.md §6: auto | on | off
//! ui_cursor = "os"              # decision 0015 §2.5: os | software
//! ui_scale_filter = "linear"    # decision 0015 §2.2: linear | nearest (previews/stretch)
//! ui_scale = "auto"             # ui-scaling.md §4.1/§4.2: "auto" or 0.5-4.0 step 0.05
//! ui_safe_aspect = "21:9"       # ui-scaling.md §5.1: 4:3 | 16:10 | 16:9 | 21:9 | full | ratio
//! ui_art_filter = "sharp"       # ui-scaling.md §7.6: sharp | smooth | nearest
//! ui_text_scale = 1.0           # ui-scaling.md §7.5: 0.80-1.50 step 0.05
//! ui_text_source = "game"       # ui-scaling.md §6.4.3: game | tessera
//! ui_scale_cursor = 1.0         # UIScale_Cursor (ui-behaviour.md §9.3), now a factor on Se
//! tooltip_enable = true         # TooltipEnable
//! tooltip_delay = 0.25          # TooltipDelay, seconds
//! canvas_mib = 128              # decision 0015 U5
//! texture_mib = 256
//! image_lru_mib = 64
//! image_pinned_mib = 512
//!
//! [UI.chat]                     # chat-window.md §10 items 1 and 7
//! opacity = 25                  # GOP_ChatOpacity override, 0-100
//!
//! [UI.panels]                   # ui-scaling.md §5.8: saved root-panel placements
//! "0x41000001/0x10000001" = { ref = "safe", ax = "left", dx = 12.0, ay = "top", dy = 8.0 }
//!
//! [UI.group_scale]              # ui-scaling.md §7.4: per-window factors by group number
//! "7" = 1.25
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::Settings;
use crate::model::MAX_WARNINGS_PER_SECTION;

/// Key of the section in [`Settings::extra`] (skin-definition.md §10 D6 names `[UI]`).
pub const UI_SECTION: &str = "UI";
/// Key of the selected skin's name (skin-definition.md §10 D6).
pub const USER_SKIN_KEY: &str = "UserSkin";
/// Longest stored skin name or path, in bytes (longer values are ignored with a warning).
pub const MAX_UI_TEXT_BYTES: usize = 1024;
/// Range of each memory budget, in MiB.
pub const BUDGET_MIB_RANGE: (u64, u64) = (1, 16_384);

/// Range of the user UI scale *S* (`ui-scaling.md` §4.1): 0.50-4.00.
pub const UI_SCALE_RANGE: (f32, f32) = (0.5, 4.0);
/// Step of the user UI scale and the text and group factors (`ui-scaling.md` §4.1, §7.4, §7.5).
pub const UI_SCALE_STEP: f32 = 0.05;
/// Range of the text-size factor `ui_text_scale` (`ui-scaling.md` §7.5): 0.80-1.50.
pub const TEXT_SCALE_RANGE: (f32, f32) = (0.80, 1.50);
/// Range of a custom safe-area aspect ratio (`ui-scaling.md` §5.1): 4/3 to 4.0.
pub const SAFE_ASPECT_RANGE: (f32, f32) = (4.0 / 3.0, 4.0);
/// Range a per-window factor is clamped to before the data min/max intersection (`ui-scaling.md`
/// §7.4): 0.50-2.00.
pub const GROUP_SCALE_RANGE: (f32, f32) = (0.5, 2.0);
/// Largest number of `[UI.panels]` entries kept (bounds allocation on hostile input).
pub const MAX_UI_PANELS: usize = 512;
/// Largest number of `[UI.group_scale]` entries kept (bounds allocation on hostile input).
pub const MAX_GROUP_SCALES: usize = 256;
/// Largest absolute value, in UI units, of a saved placement offset (`dx`, `dy`).
pub const MAX_PANEL_OFFSET: f32 = 100_000.0;
/// Largest saved placement size (`w`, `h`), in UI units.
pub const MAX_PANEL_DIM: u32 = 100_000;
/// Longest `[UI.panels]` key kept, in bytes (`"0xLLLLLLLL/0xEEEEEEEE"` is 21).
pub const MAX_PANEL_KEY_BYTES: usize = 64;

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

/// The user UI scale *S* (`ui-scaling.md` §4.1, §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum UiScale {
    /// `"auto"`: *S* is derived from the display (`ui-scaling.md` §4.2).
    #[default]
    Auto,
    /// A fixed user scale, already rounded to [`UI_SCALE_STEP`] and clamped to
    /// [`UI_SCALE_RANGE`].
    Fixed(f32),
}

impl UiScale {
    /// Parses the stored value: the string `"auto"` (any ASCII case, trimmed), or a number
    /// (or a numeric string) rounded to [`UI_SCALE_STEP`] and clamped to [`UI_SCALE_RANGE`]
    /// (`ui-scaling.md` §4.1). Returns `None` for anything else.
    #[must_use]
    pub fn parse_value(v: &toml::Value) -> Option<Self> {
        match v {
            toml::Value::String(s) => {
                let t = s.trim();
                if t.eq_ignore_ascii_case("auto") {
                    Some(Self::Auto)
                } else {
                    t.parse::<f64>().ok().and_then(Self::from_number)
                }
            }
            _ => number(v).and_then(Self::from_number),
        }
    }

    fn from_number(n: f64) -> Option<Self> {
        snap(n, UI_SCALE_RANGE).map(Self::Fixed)
    }

    fn to_value(self) -> toml::Value {
        match self {
            Self::Auto => toml::Value::String("auto".to_owned()),
            Self::Fixed(f) => toml::Value::Float(f64::from(f)),
        }
    }
}

/// The safe-area width (`ui-scaling.md` §5.1): the chosen aspect ratio of the centred area
/// the HUD is anchored inside on wide screens.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SafeAspect {
    /// 4:3.
    R4_3,
    /// 16:10.
    R16_10,
    /// 16:9.
    R16_9,
    /// 21:9 (the default; ratio 2.4 so the common 21:9 resolutions are used in full).
    #[default]
    R21_9,
    /// No limit: the safe area is the full screen.
    Full,
    /// A custom ratio, clamped to [`SAFE_ASPECT_RANGE`].
    Custom(f32),
}

impl SafeAspect {
    /// The aspect ratio *a* of §5.1, or `None` for [`SafeAspect::Full`] (no limit).
    #[must_use]
    pub fn ratio(self) -> Option<f32> {
        match self {
            Self::R4_3 => Some(4.0 / 3.0),
            Self::R16_10 => Some(1.6),
            Self::R16_9 => Some(16.0 / 9.0),
            Self::R21_9 => Some(2.4),
            Self::Full => None,
            Self::Custom(r) => Some(r),
        }
    }

    /// Parses the stored value: one of `"4:3"`, `"16:10"`, `"16:9"`, `"21:9"`, `"full"` (any
    /// ASCII case, trimmed), or a number (or a numeric string) clamped to
    /// [`SAFE_ASPECT_RANGE`]. Returns `None` for anything else.
    #[must_use]
    pub fn parse_value(v: &toml::Value) -> Option<Self> {
        match v {
            toml::Value::String(s) => {
                let t = s.trim();
                match t.to_ascii_lowercase().as_str() {
                    "4:3" => Some(Self::R4_3),
                    "16:10" => Some(Self::R16_10),
                    "16:9" => Some(Self::R16_9),
                    "21:9" => Some(Self::R21_9),
                    "full" => Some(Self::Full),
                    _ => t.parse::<f64>().ok().and_then(Self::from_number),
                }
            }
            _ => number(v).and_then(Self::from_number),
        }
    }

    fn from_number(n: f64) -> Option<Self> {
        if n.is_finite() {
            Some(Self::Custom(n.clamp(
                f64::from(SAFE_ASPECT_RANGE.0),
                f64::from(SAFE_ASPECT_RANGE.1),
            ) as f32))
        } else {
            None
        }
    }

    fn to_value(self) -> toml::Value {
        let name = match self {
            Self::R4_3 => "4:3",
            Self::R16_10 => "16:10",
            Self::R16_9 => "16:9",
            Self::R21_9 => "21:9",
            Self::Full => "full",
            Self::Custom(r) => return toml::Value::Float(f64::from(r)),
        };
        toml::Value::String(name.to_owned())
    }
}

/// `ui_art_filter` (`ui-scaling.md` §7.6): how scaled art is sampled while it is painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArtFilter {
    /// Crisp, the default.
    #[default]
    Sharp,
    /// Soft art, for users who prefer it.
    Smooth,
    /// Pixel art (nearest texel).
    Nearest,
}

impl ArtFilter {
    fn name(self) -> &'static str {
        match self {
            Self::Sharp => "sharp",
            Self::Smooth => "smooth",
            Self::Nearest => "nearest",
        }
    }

    /// Parses `sharp`, `smooth` or `nearest` (ASCII case ignored).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "sharp" => Some(Self::Sharp),
            "smooth" => Some(Self::Smooth),
            "nearest" => Some(Self::Nearest),
            _ => None,
        }
    }
}

/// `ui_text_source` (`ui-scaling.md` §6.4.3 item 4): which fonts text is laid out with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextSource {
    /// The install's own fonts, the default.
    #[default]
    Game,
    /// Tessera's bundled fonts.
    Tessera,
}

impl TextSource {
    fn name(self) -> &'static str {
        match self {
            Self::Game => "game",
            Self::Tessera => "tessera",
        }
    }

    /// Parses `game` or `tessera` (ASCII case ignored).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "game" => Some(Self::Game),
            "tessera" => Some(Self::Tessera),
            _ => None,
        }
    }
}

/// The reference box a saved placement is anchored inside (`ui-scaling.md` §5.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelRef {
    /// The safe area *A* (safe-area panels), the default.
    #[default]
    Safe,
    /// The full screen *F* (full-screen roots).
    Screen,
}

impl PanelRef {
    fn name(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Screen => "screen",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "safe" => Some(Self::Safe),
            "screen" => Some(Self::Screen),
            _ => None,
        }
    }
}

/// The horizontal anchor of a saved placement (`ui-scaling.md` §5.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnchorX {
    /// Offset from the reference box's left edge.
    #[default]
    Left,
    /// Offset of the box centre from the reference box's centre.
    Centre,
    /// Offset from the reference box's right edge.
    Right,
}

impl AnchorX {
    fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Centre => "centre",
            Self::Right => "right",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "left" => Some(Self::Left),
            "centre" => Some(Self::Centre),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

/// The vertical anchor of a saved placement (`ui-scaling.md` §5.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnchorY {
    /// Offset from the reference box's top edge.
    #[default]
    Top,
    /// Offset of the box centre from the reference box's centre.
    Centre,
    /// Offset from the reference box's bottom edge.
    Bottom,
}

impl AnchorY {
    fn name(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Centre => "centre",
            Self::Bottom => "bottom",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "top" => Some(Self::Top),
            "centre" => Some(Self::Centre),
            "bottom" => Some(Self::Bottom),
            _ => None,
        }
    }
}

/// One saved root-panel placement (`ui-scaling.md` §5.8). The map key is the root's layout
/// DID and element ID, written `"0xLLLLLLLL/0xEEEEEEEE"`.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelPlacement {
    /// The reference box *R*: the safe area for safe-area panels, the full screen for
    /// full-screen roots.
    pub reference: PanelRef,
    /// Horizontal anchor.
    pub ax: AnchorX,
    /// Offset from the horizontal anchor, in UI units (clamped to [`MAX_PANEL_OFFSET`]).
    pub dx: f32,
    /// Vertical anchor.
    pub ay: AnchorY,
    /// Offset from the vertical anchor, in UI units (clamped to [`MAX_PANEL_OFFSET`]).
    pub dy: f32,
    /// Layout width, in UI units; present only for an element that saves its size.
    pub w: Option<u32>,
    /// Layout height, in UI units; present only for an element that saves its size.
    pub h: Option<u32>,
    /// Saved visibility; present only for an element that saves it.
    pub shown: Option<bool>,
}

impl PanelPlacement {
    /// Parse one `[UI.panels]` entry. The current anchored form (`ui-scaling.md` §5.8) is
    /// tried first; a legacy normalized-corner entry (the fraction form of `ui-layout.md`
    /// §9.9) is migrated so an older settings file still loads ([`PanelPlacement::from_legacy`]).
    fn from_table(t: &toml::Table) -> Option<Self> {
        Self::from_anchored(t).or_else(|| Self::from_legacy(t))
    }

    /// The anchored form (`ui-scaling.md` §5.8): `ref`, `ax`, `dx`, `ay`, `dy`, optional
    /// `w`/`h` and `shown`.
    fn from_anchored(t: &toml::Table) -> Option<Self> {
        let reference = t
            .get("ref")
            .and_then(toml::Value::as_str)
            .and_then(PanelRef::parse)?;
        let ax = t
            .get("ax")
            .and_then(toml::Value::as_str)
            .and_then(AnchorX::parse)?;
        let ay = t
            .get("ay")
            .and_then(toml::Value::as_str)
            .and_then(AnchorY::parse)?;
        let dx = t.get("dx").and_then(number).and_then(clamp_offset)?;
        let dy = t.get("dy").and_then(number).and_then(clamp_offset)?;
        let w = opt_dim(t.get("w"))?;
        let h = opt_dim(t.get("h"))?;
        let shown = match t.get("shown") {
            None => None,
            Some(toml::Value::Boolean(b)) => Some(*b),
            Some(_) => return None,
        };
        Some(Self {
            reference,
            ax,
            dx,
            ay,
            dy,
            w,
            h,
            shown,
        })
    }

    /// Migrate a legacy normalized-corner entry (`ui-scaling.md` §5.8 "Migration"): the fraction
    /// form of `ui-layout.md` §9.9, with `x`, `y`, `width`, `height` as screen fractions in
    /// [0, 1] and an optional `shown` bool or `visible` 0/1. The anchor direction on each axis is
    /// recovered scale-free from which third of the (unit) reference the box centre lies in. The
    /// UI-unit offset cannot be resolved without the current configuration (§5.8 applies the
    /// conversion "at the current configuration", which the settings layer does not have), so the
    /// window is docked to the chosen anchor (offset 0) and keeps its laid-out size; the UI
    /// finalises the placement the next time the user moves it. Returns `None` when the table is
    /// not a legacy entry.
    fn from_legacy(t: &toml::Table) -> Option<Self> {
        let x = t.get("x").and_then(number)?;
        let y = t.get("y").and_then(number)?;
        let w = t.get("width").and_then(number)?;
        let h = t.get("height").and_then(number)?;
        if !(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite()) {
            return None;
        }
        let cx = x + w / 2.0;
        let cy = y + h / 2.0;
        let ax = if cx < 1.0 / 3.0 {
            AnchorX::Left
        } else if cx >= 2.0 / 3.0 {
            AnchorX::Right
        } else {
            AnchorX::Centre
        };
        let ay = if cy < 1.0 / 3.0 {
            AnchorY::Top
        } else if cy >= 2.0 / 3.0 {
            AnchorY::Bottom
        } else {
            AnchorY::Centre
        };
        let shown = match (t.get("shown"), t.get("visible")) {
            (Some(toml::Value::Boolean(b)), _) => Some(*b),
            (_, Some(v)) => number(v).map(|n| n != 0.0),
            _ => None,
        };
        Some(Self {
            reference: PanelRef::default(),
            ax,
            dx: 0.0,
            ay,
            dy: 0.0,
            w: None,
            h: None,
            shown,
        })
    }

    fn to_value(&self) -> toml::Value {
        let mut t = toml::Table::new();
        t.insert(
            "ref".into(),
            toml::Value::String(self.reference.name().to_owned()),
        );
        t.insert("ax".into(), toml::Value::String(self.ax.name().to_owned()));
        t.insert("dx".into(), toml::Value::Float(f64::from(self.dx)));
        t.insert("ay".into(), toml::Value::String(self.ay.name().to_owned()));
        t.insert("dy".into(), toml::Value::Float(f64::from(self.dy)));
        if let Some(w) = self.w {
            t.insert("w".into(), toml::Value::Integer(i64::from(w)));
        }
        if let Some(h) = self.h {
            t.insert("h".into(), toml::Value::Integer(i64::from(h)));
        }
        if let Some(shown) = self.shown {
            t.insert("shown".into(), toml::Value::Boolean(shown));
        }
        toml::Value::Table(t)
    }
}

/// Chat-window preferences (`[UI.chat]`, chat-window.md §10 items 1 and 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChatPrefs {
    /// Override of the window opacity step (`GOP_ChatOpacity`, 0-100); `None` = the
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
    /// Legacy global factor. Tessera has no persisted global UI factor any more
    /// (`ui-scaling.md` §7.7): a loaded file always reads this back as 1.0, because a stored
    /// `ui_scale_global` is migrated into [`UiPrefs::ui_scale`] instead (§4.4) and
    /// [`UiPrefs::store`] never writes it. The field remains only as a runtime value for
    /// callers that still apply a native-panel scale at attach time; **any value set here is
    /// discarded by [`UiPrefs::store`]** and will not survive a save/load cycle.
    pub scale_global: f32,
    /// `ui_scale_cursor` (a factor on top of *Se*, `ui-scaling.md` §6.7).
    pub scale_cursor: f32,
    /// `ui_scale`: the user UI scale *S* (`ui-scaling.md` §4.1, §4.2).
    pub ui_scale: UiScale,
    /// `ui_safe_aspect`: the safe area's width (`ui-scaling.md` §5.1).
    pub safe_aspect: SafeAspect,
    /// `ui_art_filter` (`ui-scaling.md` §7.6).
    pub art_filter: ArtFilter,
    /// `ui_text_scale`: the text-size factor, 0.80-1.50 (`ui-scaling.md` §7.5).
    pub text_scale: f32,
    /// `ui_text_source` (`ui-scaling.md` §6.4.3 item 4).
    pub text_source: TextSource,
    /// `[UI.panels]`: saved root-panel placements, by `"0xLLLLLLLL/0xEEEEEEEE"` key
    /// (`ui-scaling.md` §5.8).
    pub panels: BTreeMap<String, PanelPlacement>,
    /// `[UI.group_scale]`: per-window factors by group number (`ui-scaling.md` §7.4), each
    /// rounded to [`UI_SCALE_STEP`] and clamped to [`GROUP_SCALE_RANGE`].
    pub group_scale: BTreeMap<u32, f32>,
    /// `tooltip_enable`.
    pub tooltip_enable: bool,
    /// `tooltip_delay`, seconds (0-10).
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
            ui_scale: UiScale::Auto,
            safe_aspect: SafeAspect::R21_9,
            art_filter: ArtFilter::Sharp,
            text_scale: 1.0,
            text_source: TextSource::Game,
            panels: BTreeMap::new(),
            group_scale: BTreeMap::new(),
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

/// Rounds `v` to the nearest [`UI_SCALE_STEP`] (a value exactly halfway rounds up) and clamps
/// it into `range` (`ui-scaling.md` §4.1). `None` when `v` is not finite. The small bias makes
/// an exact half round up despite the binary representation of decimal steps. The step is an
/// exact `f64` 0.05 (converting the `f32` [`UI_SCALE_STEP`] would skew the division).
fn snap(v: f64, range: (f32, f32)) -> Option<f32> {
    if !v.is_finite() {
        return None;
    }
    const STEP: f64 = 0.05;
    let rounded = ((v / STEP) + 0.5 + 1e-7).floor() * STEP;
    Some(rounded.clamp(f64::from(range.0), f64::from(range.1)) as f32)
}

fn clamp_offset(v: f64) -> Option<f32> {
    if v.is_finite() {
        Some(v.clamp(f64::from(-MAX_PANEL_OFFSET), f64::from(MAX_PANEL_OFFSET)) as f32)
    } else {
        None
    }
}

/// Parses an optional saved size (`w`/`h`): absent is `Some(None)`; a valid integer in
/// `1..=MAX_PANEL_DIM` is `Some(Some(n))`; anything else is `None` (the whole entry is then
/// dropped).
fn opt_dim(v: Option<&toml::Value>) -> Option<Option<u32>> {
    match v {
        None => Some(None),
        Some(v) => match v.as_integer().and_then(|i| u32::try_from(i).ok()) {
            Some(n) if (1..=MAX_PANEL_DIM).contains(&n) => Some(Some(n)),
            _ => None,
        },
    }
}

impl UiPrefs {
    /// Reads the `[UI]` section. Never fails: a malformed value keeps its default and adds
    /// a warning (at most one per key, and at most [`MAX_WARNINGS_PER_SECTION`] in all).
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
        let mut bad = |key: &str| {
            if warn.len() < MAX_WARNINGS_PER_SECTION {
                warn.push(format!("[{UI_SECTION}] {key}: invalid value ignored"));
            }
        };
        // `ui_scale` wins over the legacy `ui_scale_global`; migrate only when it is absent.
        let mut saw_ui_scale = false;
        let mut legacy_global: Option<f64> = None;
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
                // Legacy key: captured for the one-time migration into `ui_scale`, not read
                // otherwise (`ui-scaling.md` §4.4, §7.7).
                "ui_scale_global" => legacy_global = number(v),
                "ui_scale" => {
                    saw_ui_scale = true;
                    match UiScale::parse_value(v) {
                        Some(s) => p.ui_scale = s,
                        None => bad(key),
                    }
                }
                "ui_safe_aspect" => match SafeAspect::parse_value(v) {
                    Some(a) => p.safe_aspect = a,
                    None => bad(key),
                },
                "ui_art_filter" => match v.as_str().and_then(ArtFilter::parse) {
                    Some(a) => p.art_filter = a,
                    None => bad(key),
                },
                "ui_text_scale" => match number(v).and_then(|n| snap(n, TEXT_SCALE_RANGE)) {
                    Some(x) => p.text_scale = x,
                    None => bad(key),
                },
                "ui_text_source" => match v.as_str().and_then(TextSource::parse) {
                    Some(s) => p.text_source = s,
                    None => bad(key),
                },
                "ui_scale_cursor" => match number(v) {
                    // ui-behaviour.md §9.3 resets values <= 0 or > 4 to 1 itself; here we only
                    // keep a finite factor so the stored value round-trips (a magnitude above
                    // f32::MAX would otherwise saturate to infinity and fail the next read).
                    Some(f) if (f as f32).is_finite() => p.scale_cursor = f as f32,
                    _ => bad(key),
                },
                "panels" => read_panels(v, &mut p.panels, &mut bad),
                "group_scale" => read_group_scale(v, &mut p.group_scale, &mut bad),
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
        // Migration: a valid legacy `ui_scale_global` seeds `ui_scale` when the new key is
        // absent (`ui-scaling.md` §4.4). The next `store` writes `ui_scale` and removes the
        // legacy key.
        if !saw_ui_scale && let Some(n) = legacy_global.and_then(UiScale::from_number) {
            p.ui_scale = n;
        }
        (p, warn)
    }

    /// Writes the section into `settings`, replacing only the keys this type knows; other
    /// keys of the section are kept. The legacy `ui_scale_global` key is removed
    /// (`ui-scaling.md` §4.4).
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
        // The global factor is gone (§7.7): never written, and removed on migration (§4.4).
        t.remove("ui_scale_global");
        t.insert("ui_scale".into(), self.ui_scale.to_value());
        t.insert("ui_safe_aspect".into(), self.safe_aspect.to_value());
        t.insert("ui_art_filter".into(), s(self.art_filter.name()));
        t.insert(
            "ui_text_scale".into(),
            toml::Value::Float(f64::from(self.text_scale)),
        );
        t.insert("ui_text_source".into(), s(self.text_source.name()));
        t.insert(
            "ui_scale_cursor".into(),
            toml::Value::Float(f64::from(self.scale_cursor)),
        );
        if self.panels.is_empty() {
            t.remove("panels");
        } else {
            let mut panels = toml::Table::new();
            for (key, placement) in self.panels.iter().take(MAX_UI_PANELS) {
                panels.insert(key.clone(), placement.to_value());
            }
            t.insert("panels".into(), toml::Value::Table(panels));
        }
        if self.group_scale.is_empty() {
            t.remove("group_scale");
        } else {
            let mut groups = toml::Table::new();
            for (group, factor) in self.group_scale.iter().take(MAX_GROUP_SCALES) {
                groups.insert(group.to_string(), toml::Value::Float(f64::from(*factor)));
            }
            t.insert("group_scale".into(), toml::Value::Table(groups));
        }
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

/// Reads `[UI.panels]` (`ui-scaling.md` §5.8): a table of placements keyed by
/// `"0xLLLLLLLL/0xEEEEEEEE"`. A malformed entry, an over-long key, or more than
/// [`MAX_UI_PANELS`] entries are skipped with a warning.
fn read_panels(
    v: &toml::Value,
    out: &mut BTreeMap<String, PanelPlacement>,
    bad: &mut impl FnMut(&str),
) {
    let Some(table) = v.as_table() else {
        bad("panels");
        return;
    };
    for (key, entry) in table {
        if out.len() >= MAX_UI_PANELS {
            bad("panels (too many entries)");
            break;
        }
        if key.is_empty() || key.len() > MAX_PANEL_KEY_BYTES {
            bad("panels (key)");
            continue;
        }
        match entry.as_table().and_then(PanelPlacement::from_table) {
            Some(placement) => {
                out.insert(key.clone(), placement);
            }
            None => bad("panels (entry)"),
        }
    }
}

/// Reads `[UI.group_scale]` (`ui-scaling.md` §7.4): a table from group number to factor. A
/// non-numeric key, a malformed factor, or more than [`MAX_GROUP_SCALES`] entries are skipped
/// with a warning.
fn read_group_scale(v: &toml::Value, out: &mut BTreeMap<u32, f32>, bad: &mut impl FnMut(&str)) {
    let Some(table) = v.as_table() else {
        bad("group_scale");
        return;
    };
    for (key, entry) in table {
        if out.len() >= MAX_GROUP_SCALES {
            bad("group_scale (too many entries)");
            break;
        }
        let group = match key.trim().parse::<u32>() {
            Ok(g) => g,
            Err(_) => {
                bad("group_scale (key)");
                continue;
            }
        };
        match number(entry).and_then(|n| snap(n, GROUP_SCALE_RANGE)) {
            Some(factor) => {
                out.insert(group, factor);
            }
            None => bad("group_scale (factor)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Settings {
        fn to_toml_str_for_test(&self) -> String {
            self.to_toml_string().expect("serialize")
        }
    }

    fn reparse(p: &UiPrefs) -> UiPrefs {
        let mut s = Settings::default();
        p.store(&mut s);
        let back = Settings::from_toml_str(&s.to_toml_str_for_test()).expect("re-parse");
        UiPrefs::from_settings(&back).0
    }

    #[test]
    fn defaults_without_a_section() {
        let (p, w) = UiPrefs::from_settings(&Settings::default());
        assert_eq!(p, UiPrefs::default());
        assert!(w.is_empty());
        assert!(!p.walker_minimap.shown(true));
        assert!(p.walker_minimap.shown(false));
        assert_eq!(p.ui_scale, UiScale::Auto);
        assert_eq!(p.safe_aspect, SafeAspect::R21_9);
        assert_eq!(p.art_filter, ArtFilter::Sharp);
        assert!((p.text_scale - 1.0).abs() < 1e-6);
        assert_eq!(p.text_source, TextSource::Game);
        assert!(p.panels.is_empty());
        assert!(p.group_scale.is_empty());
    }

    #[test]
    fn round_trip_keeps_unknown_keys() {
        let text = r#"
[UI]
UserSkin = "Test Skin"
walker_hud = "off"
walker_minimap = "on"
ui_cursor = "software"
ui_scale = 1.5
ui_safe_aspect = "16:9"
ui_art_filter = "nearest"
ui_text_scale = 1.25
ui_text_source = "tessera"
future_key = 7

[UI.chat]
opacity = 40

[UI.panels]
"0x41000001/0x10000001" = { ref = "safe", ax = "left", dx = 12.0, ay = "top", dy = 8.0, w = 300, h = 200, shown = true }

[UI.group_scale]
"7" = 1.25
"12" = 0.75
"#;
        let s = Settings::from_toml_str(text).expect("parse");
        let (p, w) = UiPrefs::from_settings(&s);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(p.user_skin, "Test Skin");
        assert!(!p.walker_hud);
        assert_eq!(p.walker_minimap, MinimapMode::On);
        assert_eq!(p.cursor, CursorMode::Software);
        assert_eq!(p.ui_scale, UiScale::Fixed(1.5));
        assert_eq!(p.safe_aspect, SafeAspect::R16_9);
        assert_eq!(p.art_filter, ArtFilter::Nearest);
        assert!((p.text_scale - 1.25).abs() < 1e-6);
        assert_eq!(p.text_source, TextSource::Tessera);
        assert_eq!(p.chat.opacity, Some(40));
        assert_eq!(p.panels.len(), 1);
        let placement = &p.panels["0x41000001/0x10000001"];
        assert_eq!(placement.reference, PanelRef::Safe);
        assert_eq!(placement.w, Some(300));
        assert_eq!(placement.shown, Some(true));
        assert_eq!(p.group_scale.get(&7), Some(&1.25));
        assert_eq!(p.group_scale.get(&12), Some(&0.75));

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
    fn auto_round_trips() {
        let p = UiPrefs {
            ui_scale: UiScale::Auto,
            ..UiPrefs::default()
        };
        assert_eq!(reparse(&p).ui_scale, UiScale::Auto);
    }

    /// A full anchored placement (`ui-scaling.md` §5.8) survives a store/load cycle unchanged,
    /// for every anchor and with and without the optional size and visibility.
    #[test]
    fn panel_placement_round_trips_through_toml() {
        let key = "0x22FF0008/0x10000001".to_owned();
        for pp in [
            PanelPlacement {
                reference: PanelRef::Screen,
                ax: AnchorX::Right,
                dx: 20.0,
                ay: AnchorY::Top,
                dy: 40.0,
                w: None,
                h: None,
                shown: None,
            },
            PanelPlacement {
                reference: PanelRef::Safe,
                ax: AnchorX::Centre,
                dx: -12.5,
                ay: AnchorY::Bottom,
                dy: 6.0,
                w: Some(400),
                h: Some(500),
                shown: Some(false),
            },
        ] {
            let mut p = UiPrefs::default();
            p.panels.insert(key.clone(), pp.clone());
            let back = reparse(&p);
            assert_eq!(back.panels.get(&key), Some(&pp));
        }
    }

    /// A legacy normalized-corner `[UI.panels]` entry (`ui-scaling.md` §5.8 "Migration") still
    /// loads, migrated to anchored directions chosen by which third of the screen the box centre
    /// falls in, docked to the anchor (offset 0) since the UI-unit offset needs the live
    /// configuration.
    #[test]
    fn legacy_panel_entry_migrates() {
        let text = r#"
schema_version = 1

[UI.panels]
"0x41000001/0x10000001" = { x = 0.05, y = 0.05, width = 0.2, height = 0.2, visible = 1 }
"0x41000001/0x10000002" = { x = 0.8, y = 0.85, width = 0.15, height = 0.1, shown = false }
"#;
        let s = Settings::from_toml_str(text).expect("parse");
        let (p, warn) = UiPrefs::from_settings(&s);
        assert!(warn.is_empty(), "{warn:?}");
        assert_eq!(p.panels.len(), 2);
        let a = &p.panels["0x41000001/0x10000001"];
        assert_eq!((a.ax, a.ay), (AnchorX::Left, AnchorY::Top));
        assert_eq!((a.dx, a.dy), (0.0, 0.0));
        assert_eq!(a.shown, Some(true));
        assert_eq!((a.w, a.h), (None, None));
        assert_eq!(a.reference, PanelRef::Safe);
        let b = &p.panels["0x41000001/0x10000002"];
        assert_eq!((b.ax, b.ay), (AnchorX::Right, AnchorY::Bottom));
        assert_eq!(b.shown, Some(false));
    }

    #[test]
    fn scale_is_rounded_to_the_nearest_step_and_clamped() {
        // Halfway rounds up (1.025 -> 1.05); below the range clamps to 0.5; above to 4.0.
        for (input, want) in [
            (1.0_f64, 1.0_f32),
            (1.023, 1.0),
            (1.025, 1.05),
            (1.49, 1.5),
            (0.1, 0.5),
            (9.0, 4.0),
            (-3.0, 0.5),
        ] {
            let v = toml::Value::Float(input);
            match UiScale::parse_value(&v) {
                Some(UiScale::Fixed(f)) => {
                    assert!((f - want).abs() < 1e-6, "{input} -> {f}, want {want}");
                }
                other => panic!("{input} parsed as {other:?}"),
            }
        }
    }

    #[test]
    fn scale_accepts_auto_and_numeric_strings() {
        assert_eq!(
            UiScale::parse_value(&toml::Value::String("AUTO".into())),
            Some(UiScale::Auto)
        );
        assert_eq!(
            UiScale::parse_value(&toml::Value::String(" 2.0 ".into())),
            Some(UiScale::Fixed(2.0))
        );
        assert_eq!(
            UiScale::parse_value(&toml::Value::String("nonsense".into())),
            None
        );
        assert_eq!(
            UiScale::parse_value(&toml::Value::Integer(2)),
            Some(UiScale::Fixed(2.0))
        );
        assert_eq!(UiScale::parse_value(&toml::Value::Boolean(true)), None);
        // Not-a-number floats never parse.
        assert_eq!(UiScale::parse_value(&toml::Value::Float(f64::NAN)), None);
    }

    #[test]
    fn safe_aspect_named_and_custom() {
        for (s, want) in [
            ("4:3", SafeAspect::R4_3),
            ("16:10", SafeAspect::R16_10),
            ("16:9", SafeAspect::R16_9),
            ("21:9", SafeAspect::R21_9),
            ("FULL", SafeAspect::Full),
        ] {
            assert_eq!(
                SafeAspect::parse_value(&toml::Value::String(s.into())),
                Some(want)
            );
        }
        // A custom ratio is clamped to 4/3..4.0.
        assert_eq!(
            SafeAspect::parse_value(&toml::Value::Float(3.2)),
            Some(SafeAspect::Custom(3.2))
        );
        // 32:9 (3.5556) is within range; a value above 4.0 clamps down.
        assert_eq!(
            SafeAspect::parse_value(&toml::Value::Float(32.0 / 9.0)),
            Some(SafeAspect::Custom(32.0 / 9.0))
        );
        assert_eq!(
            SafeAspect::parse_value(&toml::Value::Float(6.0)),
            Some(SafeAspect::Custom(4.0))
        );
        assert_eq!(
            SafeAspect::parse_value(&toml::Value::Float(1.0)),
            Some(SafeAspect::Custom(SAFE_ASPECT_RANGE.0))
        );
        assert_eq!(SafeAspect::Full.ratio(), None);
        assert_eq!(SafeAspect::R21_9.ratio(), Some(2.4));
    }

    #[test]
    fn custom_aspect_round_trips() {
        let p = UiPrefs {
            safe_aspect: SafeAspect::Custom(3.0),
            ..UiPrefs::default()
        };
        assert_eq!(reparse(&p).safe_aspect, SafeAspect::Custom(3.0));
    }

    #[test]
    fn migrates_legacy_global_scale() {
        let text = r#"
[UI]
ui_scale_global = 1.5
"#;
        let s = Settings::from_toml_str(text).expect("parse");
        let (p, w) = UiPrefs::from_settings(&s);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(p.ui_scale, UiScale::Fixed(1.5));
        // The global factor is gone for everyone else.
        assert!((p.scale_global - 1.0).abs() < 1e-6);

        // The next save writes `ui_scale` and removes `ui_scale_global`.
        let mut s2 = s.clone();
        p.store(&mut s2);
        let ui = s2.extra.get(UI_SECTION).and_then(|v| v.as_table()).unwrap();
        assert!(ui.get("ui_scale_global").is_none());
        assert_eq!(
            ui.get("ui_scale").and_then(toml::Value::as_float),
            Some(1.5)
        );
    }

    #[test]
    fn explicit_scale_wins_over_legacy_global() {
        let text = r#"
[UI]
ui_scale = "auto"
ui_scale_global = 2.0
"#;
        let s = Settings::from_toml_str(text).expect("parse");
        let (p, w) = UiPrefs::from_settings(&s);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(p.ui_scale, UiScale::Auto);
    }

    #[test]
    fn out_of_range_legacy_global_migrates_clamped() {
        let text = r#"
[UI]
ui_scale_global = 99.0
"#;
        let s = Settings::from_toml_str(text).expect("parse");
        let (p, _) = UiPrefs::from_settings(&s);
        assert_eq!(p.ui_scale, UiScale::Fixed(4.0));
    }

    #[test]
    fn malformed_values_warn_and_keep_defaults() {
        let text = r#"
[UI]
walker_hud = 3
walker_minimap = "sometimes"
ui_scale = "nonsense"
ui_safe_aspect = "ultrawide"
ui_art_filter = "fuzzy"
ui_text_scale = "big"
ui_text_source = "latin"
canvas_mib = 0
tooltip_delay = -1
"#;
        let s = Settings::from_toml_str(text).expect("parse");
        let (p, w) = UiPrefs::from_settings(&s);
        assert_eq!(w.len(), 9, "{w:?}");
        assert_eq!(p, UiPrefs::default());
    }

    #[test]
    fn malformed_panels_are_skipped_with_a_warning() {
        let text = r#"
[UI]

[UI.panels]
"0x1/0x2" = { ref = "safe", ax = "left", dx = 1.0, ay = "top", dy = 2.0 }
"0x3/0x4" = { ref = "nowhere", ax = "left", dx = 1.0, ay = "top", dy = 2.0 }
"0x5/0x6" = { ref = "screen", ax = "left", dx = 1.0 }
"0x7/0x8" = "not a table"
"#;
        let s = Settings::from_toml_str(text).expect("parse");
        let (p, w) = UiPrefs::from_settings(&s);
        assert_eq!(p.panels.len(), 1);
        assert!(p.panels.contains_key("0x1/0x2"));
        assert_eq!(w.len(), 3, "{w:?}");
    }

    #[test]
    fn malformed_group_scale_is_skipped() {
        let text = r#"
[UI]

[UI.group_scale]
"7" = 1.25
"abc" = 1.0
"9" = "big"
"10" = 9.0
"#;
        let s = Settings::from_toml_str(text).expect("parse");
        let (p, w) = UiPrefs::from_settings(&s);
        assert_eq!(p.group_scale.get(&7), Some(&1.25));
        // 9.0 clamps to the 2.0 ceiling (not dropped).
        assert_eq!(p.group_scale.get(&10), Some(&2.0));
        assert_eq!(p.group_scale.len(), 2);
        assert_eq!(w.len(), 2, "{w:?}");
    }

    #[test]
    fn many_panels_are_bounded() {
        let mut body = String::from("[UI]\n\n[UI.panels]\n");
        for i in 0..(MAX_UI_PANELS + 50) {
            body.push_str(&format!(
                "\"0x{i:x}/0x0\" = {{ ref = \"safe\", ax = \"left\", dx = 0.0, ay = \"top\", dy = 0.0 }}\n"
            ));
        }
        let s = Settings::from_toml_str(&body).expect("parse");
        let (p, _) = UiPrefs::from_settings(&s);
        assert!(p.panels.len() <= MAX_UI_PANELS);
    }

    #[test]
    fn warnings_are_bounded() {
        let mut body = String::from("[UI]\n\n[UI.group_scale]\n");
        for i in 0..1000 {
            body.push_str(&format!("\"bad{i}\" = 1.0\n"));
        }
        let s = Settings::from_toml_str(&body).expect("parse");
        let (_, w) = UiPrefs::from_settings(&s);
        assert!(w.len() <= MAX_WARNINGS_PER_SECTION, "{}", w.len());
    }

    #[test]
    fn overflowing_scale_cursor_is_rejected_and_round_trips() {
        // A magnitude above f32::MAX would saturate to infinity on the `as f32` cast; the
        // handler must reject it (warn, keep the default) rather than store a non-finite value
        // that the next read cannot accept (ui-scaling.md §6.7).
        let text = "[UI]\nui_scale_cursor = 1e39\n";
        let s = Settings::from_toml_str(text).expect("parse");
        let (p, w) = UiPrefs::from_settings(&s);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(p.scale_cursor.is_finite());
        assert!((p.scale_cursor - 1.0).abs() < 1e-6);

        // What `store` writes must read back without a warning and unchanged.
        let mut s2 = s.clone();
        p.store(&mut s2);
        let (p2, w2) = UiPrefs::from_settings(&s2);
        assert!(w2.is_empty(), "{w2:?}");
        assert_eq!(p, p2);
    }

    #[test]
    fn finite_scale_cursor_round_trips() {
        // A finite in-range factor is kept as-is (the range reset of ui-behaviour.md §9.3
        // happens downstream, not here).
        let p = UiPrefs {
            scale_cursor: 2.0,
            ..UiPrefs::default()
        };
        assert!((reparse(&p).scale_cursor - 2.0).abs() < 1e-6);
    }

    #[test]
    fn store_discards_runtime_scale_global() {
        // `scale_global` is a runtime-only value (ui-scaling.md §4.4, §7.7): `store` never
        // persists it, so it always reads back as 1.0.
        let p = UiPrefs {
            scale_global: 2.0,
            ..UiPrefs::default()
        };
        let mut s = Settings::default();
        p.store(&mut s);
        let ui = s.extra.get(UI_SECTION).and_then(|v| v.as_table()).unwrap();
        assert!(ui.get("ui_scale_global").is_none());
        assert!((UiPrefs::from_settings(&s).0.scale_global - 1.0).abs() < 1e-6);
    }

    #[test]
    fn store_never_panics_on_non_table_section() {
        let mut s = Settings::default();
        s.extra
            .insert(UI_SECTION.to_owned(), toml::Value::Integer(5));
        UiPrefs::default().store(&mut s);
        // The section was replaced by a table and round-trips.
        let (p, _) = UiPrefs::from_settings(&s);
        assert_eq!(p, UiPrefs::default());
    }
}
