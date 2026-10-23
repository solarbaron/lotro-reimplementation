// SPDX-License-Identifier: GPL-3.0-or-later
//! Tessera's own saved choices of the NPC windows and the quest journal, kept in the
//! `[GameWindows]` section of the settings file:
//!
//! - the dialogue font size: the index the conversation window's font-size arrows chose in
//!   the text pane's font list (`npc-windows.md` §5.2 Tessera rule);
//! - the quest tracker's visibility (`quest-journal.md` §4.8 `Quest_ShowTrackerCheckBox`,
//!   kept as a Tessera setting);
//! - the quest log's font index (the font-size arrows of the details pane, §4.7);
//! - the overhead quest icons switch (`quest-journal.md` §8.2; the client's `--quest-icons`
//!   overrides it for one run).
//!
//! These are Tessera's keys, not the original client's options. Every key is optional: an
//! absent key means the default (font indices: the window's own font, tracker and icons:
//! on). A key with a value of the wrong type, or a font index outside
//! `0..=`[`MAX_FONT_INDEX`] (the lists hold seven fonts), is reported by
//! [`WindowPrefs::from_settings`] and read as absent; nothing is migrated or rewritten. The
//! section is stored in [`Settings::extra`] under [`WINDOWS_SECTION`], so unknown keys in it
//! are kept and files written by this version round-trip through older ones.
//!
//! ```toml
//! [GameWindows]
//! dialogue_font_index = 3      # 0..=6
//! quest_tracker_shown = true
//! quest_log_font_index = 2     # 0..=6
//! quest_icons = false
//! ```
//!
//! Callers write with [`WindowPrefs::store`] inside a [`crate::SettingsStore::update`]; the
//! client does that on a worker thread, never on its frame thread (`ARCHITECTURE.md` §21).

use crate::Settings;

/// Key of the section in [`Settings::extra`].
pub const WINDOWS_SECTION: &str = "GameWindows";
/// The dialogue font index (`npc-windows.md` §5.2).
pub const DIALOGUE_FONT_KEY: &str = "dialogue_font_index";
/// The quest tracker's visibility (`quest-journal.md` §4.8).
pub const TRACKER_SHOWN_KEY: &str = "quest_tracker_shown";
/// The quest log's font index (`quest-journal.md` §4.7).
pub const QUEST_LOG_FONT_KEY: &str = "quest_log_font_index";
/// The overhead quest icons switch (`quest-journal.md` §8.2).
pub const QUEST_ICONS_KEY: &str = "quest_icons";
/// The largest font index kept: the font lists of the dialogue text pane and the quest
/// log's sample hold seven fonts (`npc-windows.md` §5.2, `quest-journal.md` §4.7).
pub const MAX_FONT_INDEX: u32 = 6;

/// The `[GameWindows]` preferences: `None` is "not saved" (the default applies).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowPrefs {
    /// `dialogue_font_index`, in `0..=MAX_FONT_INDEX`.
    pub dialogue_font: Option<u32>,
    /// `quest_tracker_shown`.
    pub tracker_shown: Option<bool>,
    /// `quest_log_font_index`, in `0..=MAX_FONT_INDEX`.
    pub quest_log_font: Option<u32>,
    /// `quest_icons`.
    pub quest_icons: Option<bool>,
}

fn boolean(v: &toml::Value) -> Option<bool> {
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

fn font_index(v: &toml::Value) -> Option<u32> {
    let i = v.as_integer()?;
    u32::try_from(i).ok().filter(|i| *i <= MAX_FONT_INDEX)
}

impl WindowPrefs {
    /// Reads the `[GameWindows]` section. Never fails: a malformed or out-of-range value is
    /// left unset and adds a warning.
    #[must_use]
    pub fn from_settings(settings: &Settings) -> (Self, Vec<String>) {
        let mut p = Self::default();
        let mut warn = Vec::new();
        let Some(section) = settings.extra.get(WINDOWS_SECTION) else {
            return (p, warn);
        };
        let Some(t) = section.as_table() else {
            warn.push(format!(
                "[{WINDOWS_SECTION}] is not a table; using defaults"
            ));
            return (p, warn);
        };
        let mut invalid = |key: &str| {
            warn.push(format!("[{WINDOWS_SECTION}] {key}: invalid value ignored"));
        };
        for (key, slot) in [
            (DIALOGUE_FONT_KEY, &mut p.dialogue_font),
            (QUEST_LOG_FONT_KEY, &mut p.quest_log_font),
        ] {
            if let Some(v) = t.get(key) {
                *slot = font_index(v);
                if slot.is_none() {
                    invalid(key);
                }
            }
        }
        for (key, slot) in [
            (TRACKER_SHOWN_KEY, &mut p.tracker_shown),
            (QUEST_ICONS_KEY, &mut p.quest_icons),
        ] {
            if let Some(v) = t.get(key) {
                *slot = boolean(v);
                if slot.is_none() {
                    invalid(key);
                }
            }
        }
        (p, warn)
    }

    /// Writes the keys that are set into `settings` (font indices clamped to
    /// [`MAX_FONT_INDEX`]); unset keys and other keys of the section are left as they are.
    pub fn store(&self, settings: &mut Settings) {
        let entry = settings
            .extra
            .entry(WINDOWS_SECTION.to_owned())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if !entry.is_table() {
            *entry = toml::Value::Table(toml::Table::new());
        }
        let Some(t) = entry.as_table_mut() else {
            return;
        };
        for (key, v) in [
            (DIALOGUE_FONT_KEY, self.dialogue_font),
            (QUEST_LOG_FONT_KEY, self.quest_log_font),
        ] {
            if let Some(i) = v {
                t.insert(
                    key.into(),
                    toml::Value::Integer(i64::from(i.min(MAX_FONT_INDEX))),
                );
            }
        }
        for (key, v) in [
            (TRACKER_SHOWN_KEY, self.tracker_shown),
            (QUEST_ICONS_KEY, self.quest_icons),
        ] {
            if let Some(b) = v {
                t.insert(key.into(), toml::Value::Boolean(b));
            }
        }
    }

    /// Whether the quest tracker is shown (default on).
    #[must_use]
    pub fn tracker_shown_or_default(&self) -> bool {
        self.tracker_shown.unwrap_or(true)
    }

    /// Whether the overhead quest icons are on (default on).
    #[must_use]
    pub fn quest_icons_or_default(&self) -> bool {
        self.quest_icons.unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_section(t: toml::Table) -> Settings {
        let mut s = Settings::default();
        s.extra
            .insert(WINDOWS_SECTION.into(), toml::Value::Table(t));
        s
    }

    #[test]
    fn defaults_without_a_section() {
        let (p, w) = WindowPrefs::from_settings(&Settings::default());
        assert_eq!(p, WindowPrefs::default());
        assert!(w.is_empty());
        assert!(p.tracker_shown_or_default());
        assert!(p.quest_icons_or_default());
        assert_eq!(p.dialogue_font, None);
        assert_eq!(p.quest_log_font, None);
    }

    #[test]
    fn parses_every_key() {
        let s: toml::Table = toml::from_str(
            "[GameWindows]\ndialogue_font_index = 3\nquest_tracker_shown = false\n\
             quest_log_font_index = 0\nquest_icons = \"off\"\n",
        )
        .unwrap();
        let s = Settings {
            extra: s,
            ..Settings::default()
        };
        let (p, w) = WindowPrefs::from_settings(&s);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(
            p,
            WindowPrefs {
                dialogue_font: Some(3),
                tracker_shown: Some(false),
                quest_log_font: Some(0),
                quest_icons: Some(false),
            }
        );
        assert!(!p.tracker_shown_or_default());
        assert!(!p.quest_icons_or_default());
    }

    #[test]
    fn out_of_range_and_malformed_values_take_the_default() {
        let mut t = toml::Table::new();
        t.insert(DIALOGUE_FONT_KEY.into(), toml::Value::Integer(7));
        t.insert(QUEST_LOG_FONT_KEY.into(), toml::Value::Integer(-1));
        t.insert(TRACKER_SHOWN_KEY.into(), toml::Value::Integer(1));
        t.insert(QUEST_ICONS_KEY.into(), toml::Value::String("maybe".into()));
        let (p, w) = WindowPrefs::from_settings(&with_section(t));
        assert_eq!(p, WindowPrefs::default());
        assert_eq!(w.len(), 4, "{w:?}");
        // The edges of the range are kept.
        let mut t = toml::Table::new();
        t.insert(DIALOGUE_FONT_KEY.into(), toml::Value::Integer(6));
        t.insert(
            QUEST_LOG_FONT_KEY.into(),
            toml::Value::Float(2.0), // not an integer
        );
        let (p, w) = WindowPrefs::from_settings(&with_section(t));
        assert_eq!(p.dialogue_font, Some(MAX_FONT_INDEX));
        assert_eq!(p.quest_log_font, None);
        assert_eq!(w.len(), 1);
        // A section that is not a table.
        let mut s = Settings::default();
        s.extra
            .insert(WINDOWS_SECTION.into(), toml::Value::Integer(3));
        let (p, w) = WindowPrefs::from_settings(&s);
        assert_eq!(p, WindowPrefs::default());
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn store_round_trips_and_keeps_other_keys() {
        let mut t = toml::Table::new();
        t.insert("other".into(), toml::Value::Integer(5));
        t.insert(QUEST_ICONS_KEY.into(), toml::Value::Boolean(false));
        let mut s = with_section(t);
        // Only the set keys are written: quest_icons stays as the file had it.
        let p = WindowPrefs {
            dialogue_font: Some(2),
            tracker_shown: Some(true),
            quest_log_font: Some(99),
            quest_icons: None,
        };
        p.store(&mut s);
        let (back, w) = WindowPrefs::from_settings(&s);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(
            back,
            WindowPrefs {
                dialogue_font: Some(2),
                tracker_shown: Some(true),
                quest_log_font: Some(MAX_FONT_INDEX),
                quest_icons: Some(false),
            }
        );
        let kept = s.extra[WINDOWS_SECTION].as_table().unwrap();
        assert_eq!(kept.get("other"), Some(&toml::Value::Integer(5)));
        // Through the file text as well.
        let text = toml::to_string(&s.extra).unwrap();
        let again: toml::Table = toml::from_str(&text).unwrap();
        let s2 = Settings {
            extra: again,
            ..Settings::default()
        };
        assert_eq!(WindowPrefs::from_settings(&s2).0, back);
        // A section that is not a table is replaced.
        let mut s3 = Settings::default();
        s3.extra
            .insert(WINDOWS_SECTION.into(), toml::Value::Integer(1));
        back.store(&mut s3);
        assert_eq!(WindowPrefs::from_settings(&s3), (back, Vec::new()));
    }
}
