// SPDX-License-Identifier: GPL-3.0-or-later
//! The map window's Tessera settings (`map-windows.md` §3.1, §3.4, §6.9, §8), kept in the
//! `[Map]` section of the settings file.
//!
//! Every key is optional: a key that is absent means "the install's default" (the client
//! reads `GameplayOptionDefaults` at run time; nothing here is a game default). A
//! malformed key is reported by [`MapPrefs::from_settings`] and treated as absent. The
//! section is stored in [`Settings::extra`] under [`MAP_SECTION`], so unknown keys in it
//! are kept and files written by this version round-trip through older ones.
//!
//! ```toml
//! [Map]
//! map_fullscreen = true                  # §3.1 presentation switch
//! map_coordinates = true                 # §3.4 coordinate display
//! map_discovery = "all"                  # §8: all | none (Tessera default: all)
//! near_filter = "0x00000000ffffffff"     # §6.9 note filters, 64-bit values written in hex
//! distant_filter = "0x00000000ffffffff"
//! radar_filter = "0x00000000ffffffff"
//! ```

use crate::Settings;

/// Key of the section in [`Settings::extra`].
pub const MAP_SECTION: &str = "Map";
/// The presentation switch (`map-windows.md` §3.1).
pub const MAP_FULLSCREEN_KEY: &str = "map_fullscreen";
/// The coordinate display switch (`map-windows.md` §3.4).
pub const MAP_COORDINATES_KEY: &str = "map_coordinates";
/// The discovery rule, `all` or `none` (`map-windows.md` §8).
pub const MAP_DISCOVERY_KEY: &str = "map_discovery";
/// The near maps' note filter (`map-windows.md` §6.9).
pub const NEAR_FILTER_KEY: &str = "near_filter";
/// The distant maps' note filter (`map-windows.md` §6.9).
pub const DISTANT_FILTER_KEY: &str = "distant_filter";
/// The minimap's note filter (`map-windows.md` §6.9).
pub const RADAR_FILTER_KEY: &str = "radar_filter";

/// The `[Map]` preferences: `None` keeps the install's default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MapPrefs {
    /// `map_fullscreen`.
    pub fullscreen: Option<bool>,
    /// `map_coordinates`.
    pub coordinates: Option<bool>,
    /// `map_discovery = all` (`Some(true)`) or `none` (`Some(false)`).
    pub discovery_all: Option<bool>,
    /// `near_filter`.
    pub near: Option<u64>,
    /// `distant_filter`.
    pub distant: Option<u64>,
    /// `radar_filter`.
    pub radar: Option<u64>,
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

fn discovery(v: &toml::Value) -> Option<bool> {
    match v.as_str()?.trim().to_ascii_lowercase().as_str() {
        "all" => Some(true),
        "none" => Some(false),
        _ => None,
    }
}

/// A filter value: a hex string (`0x…`, at most 16 digits) or a non-negative integer.
fn filter(v: &toml::Value) -> Option<u64> {
    match v {
        toml::Value::Integer(i) => u64::try_from(*i).ok(),
        toml::Value::String(s) => {
            let t = s.trim();
            let hex = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X"))?;
            if hex.is_empty() || hex.len() > 16 {
                return None;
            }
            u64::from_str_radix(hex, 16).ok()
        }
        _ => None,
    }
}

fn hex(v: u64) -> toml::Value {
    toml::Value::String(format!("{v:#018x}"))
}

impl MapPrefs {
    /// Reads the `[Map]` section. Never fails: a malformed value is left unset and adds a
    /// warning.
    #[must_use]
    pub fn from_settings(settings: &Settings) -> (Self, Vec<String>) {
        let mut p = Self::default();
        let mut warn = Vec::new();
        let Some(section) = settings.extra.get(MAP_SECTION) else {
            return (p, warn);
        };
        let Some(t) = section.as_table() else {
            warn.push(format!("[{MAP_SECTION}] is not a table; using defaults"));
            return (p, warn);
        };
        let mut read = |key: &str, parse: fn(&toml::Value) -> Option<_>| -> Option<bool> {
            let v = t.get(key)?;
            let r = parse(v);
            if r.is_none() {
                warn.push(format!("[{MAP_SECTION}] {key}: invalid value ignored"));
            }
            r
        };
        p.fullscreen = read(MAP_FULLSCREEN_KEY, boolean);
        p.coordinates = read(MAP_COORDINATES_KEY, boolean);
        p.discovery_all = read(MAP_DISCOVERY_KEY, discovery);
        for (key, slot) in [
            (NEAR_FILTER_KEY, &mut p.near),
            (DISTANT_FILTER_KEY, &mut p.distant),
            (RADAR_FILTER_KEY, &mut p.radar),
        ] {
            if let Some(v) = t.get(key) {
                *slot = filter(v);
                if slot.is_none() {
                    warn.push(format!("[{MAP_SECTION}] {key}: invalid value ignored"));
                }
            }
        }
        (p, warn)
    }

    /// Writes the keys that are set into `settings`; unset keys and other keys of the
    /// section are left as they are.
    pub fn store(&self, settings: &mut Settings) {
        let entry = settings
            .extra
            .entry(MAP_SECTION.to_owned())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if !entry.is_table() {
            *entry = toml::Value::Table(toml::Table::new());
        }
        let Some(t) = entry.as_table_mut() else {
            return;
        };
        for (key, v) in [
            (MAP_FULLSCREEN_KEY, self.fullscreen),
            (MAP_COORDINATES_KEY, self.coordinates),
        ] {
            if let Some(b) = v {
                t.insert(key.into(), toml::Value::Boolean(b));
            }
        }
        if let Some(all) = self.discovery_all {
            t.insert(
                MAP_DISCOVERY_KEY.into(),
                toml::Value::String(if all { "all" } else { "none" }.into()),
            );
        }
        for (key, v) in [
            (NEAR_FILTER_KEY, self.near),
            (DISTANT_FILTER_KEY, self.distant),
            (RADAR_FILTER_KEY, self.radar),
        ] {
            if let Some(f) = v {
                t.insert(key.into(), hex(f));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_without_a_section() {
        let (p, w) = MapPrefs::from_settings(&Settings::default());
        assert_eq!(p, MapPrefs::default());
        assert!(w.is_empty());
    }

    #[test]
    fn round_trip_keeps_other_keys() {
        let mut s = Settings::default();
        let mut t = toml::Table::new();
        t.insert("other".into(), toml::Value::Integer(5));
        s.extra.insert(MAP_SECTION.into(), toml::Value::Table(t));
        let p = MapPrefs {
            fullscreen: Some(false),
            coordinates: Some(true),
            discovery_all: Some(false),
            near: Some(u64::MAX),
            distant: Some(0),
            radar: Some(0x1234),
        };
        p.store(&mut s);
        let (back, w) = MapPrefs::from_settings(&s);
        assert_eq!(back, p);
        assert!(w.is_empty(), "{w:?}");
        let kept = s.extra[MAP_SECTION].as_table().unwrap();
        assert_eq!(kept.get("other"), Some(&toml::Value::Integer(5)));
        // Through the file text as well.
        let text = toml::to_string(&s.extra).unwrap();
        let again: toml::Table = toml::from_str(&text).unwrap();
        let s2 = Settings {
            extra: again,
            ..Settings::default()
        };
        assert_eq!(MapPrefs::from_settings(&s2).0, p);
    }

    #[test]
    fn malformed_values_warn_and_stay_unset() {
        let mut s = Settings::default();
        let mut t = toml::Table::new();
        t.insert(MAP_FULLSCREEN_KEY.into(), toml::Value::Integer(1));
        t.insert(MAP_DISCOVERY_KEY.into(), toml::Value::String("some".into()));
        t.insert(NEAR_FILTER_KEY.into(), toml::Value::Integer(-1));
        t.insert(
            DISTANT_FILTER_KEY.into(),
            toml::Value::String("0x1ffffffffffffffff".into()),
        );
        t.insert(RADAR_FILTER_KEY.into(), toml::Value::String("12".into()));
        t.insert(
            MAP_COORDINATES_KEY.into(),
            toml::Value::String("off".into()),
        );
        s.extra.insert(MAP_SECTION.into(), toml::Value::Table(t));
        let (p, w) = MapPrefs::from_settings(&s);
        assert_eq!(
            p,
            MapPrefs {
                coordinates: Some(false),
                ..MapPrefs::default()
            }
        );
        assert_eq!(w.len(), 5, "{w:?}");
        s.extra.insert(MAP_SECTION.into(), toml::Value::Integer(3));
        let (p, w) = MapPrefs::from_settings(&s);
        assert_eq!(p, MapPrefs::default());
        assert_eq!(w.len(), 1);
    }
}
