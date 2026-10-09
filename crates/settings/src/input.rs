// SPDX-License-Identifier: GPL-3.0-or-later
//! The input preferences of the client, kept in the `[Input]` section of the settings file
//! (`keymap.md` §7.7, decision 0014 D3).
//!
//! The section is stored in [`Settings::extra`] under the key [`INPUT_SECTION`], like the
//! `[UI]` section, so it round-trips through older versions and unknown keys inside it are
//! kept. Both keys are optional: a key present in the file overrides the platform's value,
//! and where neither gives one the defaults below apply (`keymap.md` §7.7, "Where *T*, *W*,
//! *H* come from").
//!
//! ```toml
//! [Input]
//! double_click_ms = 500   # double-click time T in milliseconds (100–5000)
//! double_click_px = 4     # size of the double-click rectangle, logical pixels (1–64)
//! ```

use crate::Settings;

/// Key of the section in [`Settings::extra`] (`keymap.md` §7.7 names `[Input]`).
pub const INPUT_SECTION: &str = "Input";
/// `double_click_ms` when neither the settings file nor the platform gives one.
pub const DEFAULT_DOUBLE_CLICK_MS: u32 = 500;
/// `double_click_px` when neither the settings file nor the platform gives one.
pub const DEFAULT_DOUBLE_CLICK_PX: u32 = 4;
/// Accepted range of `double_click_ms` (the mapper clamps into the same range).
pub const DOUBLE_CLICK_MS_RANGE: (u32, u32) = (100, 5000);
/// Accepted range of `double_click_px`, logical pixels.
pub const DOUBLE_CLICK_PX_RANGE: (u32, u32) = (1, 64);

/// The `[Input]` section (module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InputPrefs {
    /// `double_click_ms`, when the file sets it.
    pub double_click_ms: Option<u32>,
    /// `double_click_px`, when the file sets it.
    pub double_click_px: Option<u32>,
}

fn integer_in(v: &toml::Value, range: (u32, u32)) -> Option<u32> {
    let i = v.as_integer()?;
    let i = u32::try_from(i).ok()?;
    (range.0..=range.1).contains(&i).then_some(i)
}

impl InputPrefs {
    /// Reads the `[Input]` section. Never fails: a malformed value is left unset and adds a
    /// warning (at most one per key).
    #[must_use]
    pub fn from_settings(settings: &Settings) -> (Self, Vec<String>) {
        let mut p = Self::default();
        let mut warn = Vec::new();
        let Some(section) = settings.extra.get(INPUT_SECTION) else {
            return (p, warn);
        };
        let Some(t) = section.as_table() else {
            warn.push(format!("[{INPUT_SECTION}] is not a table; using defaults"));
            return (p, warn);
        };
        for (key, v) in t {
            match key.as_str() {
                "double_click_ms" => match integer_in(v, DOUBLE_CLICK_MS_RANGE) {
                    Some(ms) => p.double_click_ms = Some(ms),
                    None => warn.push(format!("[{INPUT_SECTION}] {key}: invalid value ignored")),
                },
                "double_click_px" => match integer_in(v, DOUBLE_CLICK_PX_RANGE) {
                    Some(px) => p.double_click_px = Some(px),
                    None => warn.push(format!("[{INPUT_SECTION}] {key}: invalid value ignored")),
                },
                _ => {}
            }
        }
        (p, warn)
    }

    /// *T* in milliseconds: the file's value, else `platform`, else the default.
    #[must_use]
    pub fn double_click_ms(&self, platform: Option<u32>) -> u32 {
        self.double_click_ms
            .or(platform)
            .unwrap_or(DEFAULT_DOUBLE_CLICK_MS)
    }

    /// The rectangle size in logical pixels: the file's value, else `platform`, else the
    /// default.
    #[must_use]
    pub fn double_click_px(&self, platform: Option<u32>) -> u32 {
        self.double_click_px
            .or(platform)
            .unwrap_or(DEFAULT_DOUBLE_CLICK_PX)
    }

    /// Writes the keys that are set into `settings`; other keys of the section are kept,
    /// and keys that are unset are removed.
    pub fn store(&self, settings: &mut Settings) {
        let entry = settings
            .extra
            .entry(INPUT_SECTION.to_owned())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if !entry.is_table() {
            *entry = toml::Value::Table(toml::Table::new());
        }
        let Some(t) = entry.as_table_mut() else {
            return;
        };
        for (key, v) in [
            ("double_click_ms", self.double_click_ms),
            ("double_click_px", self.double_click_px),
        ] {
            match v {
                Some(v) => {
                    t.insert(key.into(), toml::Value::Integer(i64::from(v)));
                }
                None => {
                    t.remove(key);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_without_a_section() {
        let (p, w) = InputPrefs::from_settings(&Settings::default());
        assert_eq!(p, InputPrefs::default());
        assert!(w.is_empty());
        assert_eq!(p.double_click_ms(None), DEFAULT_DOUBLE_CLICK_MS);
        assert_eq!(p.double_click_px(Some(6)), 6, "the platform value applies");
    }

    #[test]
    fn file_values_override_and_bad_values_warn() {
        let mut s = Settings::default();
        let mut t = toml::Table::new();
        t.insert("double_click_ms".into(), toml::Value::Integer(350));
        t.insert("double_click_px".into(), toml::Value::Integer(0));
        t.insert("other".into(), toml::Value::Boolean(true));
        s.extra.insert(INPUT_SECTION.into(), toml::Value::Table(t));
        let (p, w) = InputPrefs::from_settings(&s);
        assert_eq!(p.double_click_ms, Some(350));
        assert_eq!(p.double_click_px, None);
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(
            p.double_click_ms(Some(900)),
            350,
            "a key in the file overrides"
        );
        let mut out = s.clone();
        p.store(&mut out);
        let t = out.extra[INPUT_SECTION].as_table().expect("table");
        assert_eq!(t.get("double_click_ms"), Some(&toml::Value::Integer(350)));
        assert!(t.get("double_click_px").is_none());
        assert_eq!(
            t.get("other"),
            Some(&toml::Value::Boolean(true)),
            "unknown keys kept"
        );
        assert_eq!(InputPrefs::from_settings(&out).0, p);
    }
}
