// SPDX-License-Identifier: GPL-3.0-or-later
//! The P1 panels' entries of Tessera's settings file (`[UI]`):
//!
//! - `compare_equipped_items` (on/off, default on): the comparison of `item-tooltips.md`
//!   §8.3 (Tessera rule, settings key `ui.compare_equipped_items`);
//! - the bag windows' column settings of `character-panels.md` §4.1
//!   (`UI_UserSetting_NumItemsInRow`, for example `UI.Bag1Columns`): a name `S.K` is key
//!   `K` of section `S` (a name without a dot is a key of `[UI]`), an integer of 1 …
//!   [`MAX_BAG_COLUMNS`].
//!
//! Reading never fails: a malformed value keeps its default (logged).

use std::collections::BTreeMap;

use tessera_settings::Settings;
use tessera_settings::toml;

/// The settings section of the P1 keys without a section of their own.
pub const UI_SECTION: &str = tessera_settings::UI_SECTION;
/// The comparison switch's key in [`UI_SECTION`].
pub const COMPARE_KEY: &str = "compare_equipped_items";
/// Most columns a bag window's setting may hold.
pub const MAX_BAG_COLUMNS: i32 = 64;
/// Most column settings read (hostile settings files).
pub const MAX_COLUMN_SETTINGS: usize = 64;

/// The P1 entries of the settings (module docs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P1Settings {
    /// Comparisons with equipped items on hover (default on).
    pub compare_equipped: bool,
    /// Column settings by their full name (`UI.Bag1Columns` …).
    pub columns: BTreeMap<String, i32>,
}

impl Default for P1Settings {
    fn default() -> Self {
        Self {
            compare_equipped: true,
            columns: BTreeMap::new(),
        }
    }
}

/// Splits a setting name into section and key (module docs).
fn split(name: &str) -> (&str, &str) {
    name.split_once('.').unwrap_or((UI_SECTION, name))
}

fn section<'a>(settings: &'a Settings, name: &str) -> Option<&'a toml::Table> {
    settings.extra.get(name).and_then(toml::Value::as_table)
}

/// Whether `name` looks like a bag column setting this module keeps (`…BagNColumns`).
fn is_column_key(key: &str) -> bool {
    key.strip_prefix("Bag")
        .and_then(|r| r.strip_suffix("Columns"))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

impl P1Settings {
    /// Reads the P1 entries of `settings`.
    #[must_use]
    pub fn from_settings(settings: &Settings) -> Self {
        let mut out = Self::default();
        if let Some(ui) = section(settings, UI_SECTION)
            && let Some(v) = ui.get(COMPARE_KEY)
        {
            match v.as_bool() {
                Some(b) => out.compare_equipped = b,
                None => {
                    tracing::warn!("settings: [{UI_SECTION}] {COMPARE_KEY}: invalid value ignored")
                }
            }
        }
        for (sname, table) in settings
            .extra
            .iter()
            .filter_map(|(n, v)| Some((n, v.as_table()?)))
        {
            for (key, v) in table {
                if out.columns.len() >= MAX_COLUMN_SETTINGS {
                    return out;
                }
                if !is_column_key(key) {
                    continue;
                }
                match v
                    .as_integer()
                    .and_then(|i| i32::try_from(i).ok())
                    .filter(|c| (1..=MAX_BAG_COLUMNS).contains(c))
                {
                    Some(c) => {
                        out.columns.insert(format!("{sname}.{key}"), c);
                    }
                    None => tracing::warn!("settings: [{sname}] {key}: invalid value ignored"),
                }
            }
        }
        out
    }

    /// Reads the user's settings file (defaults when it cannot be read).
    #[must_use]
    pub fn load_default() -> Self {
        match tessera_settings::SettingsStore::open_default() {
            Ok(store) => Self::from_settings(&store.load().settings),
            Err(e) => {
                tracing::debug!("settings unavailable ({e}); P1 defaults");
                Self::default()
            }
        }
    }

    /// The columns of setting `name`, if the user has one.
    #[must_use]
    pub fn columns(&self, name: &str) -> Option<i32> {
        self.columns.get(&full_name(name)).copied()
    }
}

/// A setting name with its section (`Bag1Columns` → `UI.Bag1Columns`).
#[must_use]
pub fn full_name(name: &str) -> String {
    let (s, k) = split(name);
    format!("{s}.{k}")
}

/// Writes column setting `name` = `columns` into `settings` (clamped to 1 …
/// [`MAX_BAG_COLUMNS`]); other keys are kept.
pub fn store_columns(settings: &mut Settings, name: &str, columns: i32) {
    let (s, k) = split(name);
    let entry = settings
        .extra
        .entry(s.to_owned())
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    if !entry.is_table() {
        *entry = toml::Value::Table(toml::Table::new());
    }
    if let Some(t) = entry.as_table_mut() {
        t.insert(
            k.to_owned(),
            toml::Value::Integer(i64::from(columns.clamp(1, MAX_BAG_COLUMNS))),
        );
    }
}

/// Saves column settings to the user's settings file (logged when it cannot be written).
pub fn save_columns(changes: &[(String, i32)]) {
    if changes.is_empty() {
        return;
    }
    let store = match tessera_settings::SettingsStore::open_default() {
        Ok(s) => s,
        Err(e) => {
            tracing::info!("bag columns not saved: {e}");
            return;
        }
    };
    if let Err(e) = store.update(|s| {
        for (name, c) in changes {
            store_columns(s, name, *c);
        }
    }) {
        tracing::info!("bag columns not saved: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_switch_and_the_columns() {
        let mut s = Settings::default();
        assert_eq!(P1Settings::from_settings(&s), P1Settings::default());
        let mut ui = toml::Table::new();
        ui.insert(COMPARE_KEY.into(), toml::Value::Boolean(false));
        ui.insert("Bag1Columns".into(), toml::Value::Integer(8));
        ui.insert("Bag2Columns".into(), toml::Value::Integer(0));
        ui.insert("Bag3Columns".into(), toml::Value::String("x".into()));
        ui.insert("Other".into(), toml::Value::Integer(3));
        s.extra.insert(UI_SECTION.into(), toml::Value::Table(ui));
        let p = P1Settings::from_settings(&s);
        assert!(!p.compare_equipped);
        assert_eq!(p.columns("UI.Bag1Columns"), Some(8));
        assert_eq!(p.columns("Bag1Columns"), Some(8));
        assert_eq!(p.columns("UI.Bag2Columns"), None, "out of range");
        assert_eq!(p.columns("UI.Bag3Columns"), None, "not an integer");
        assert_eq!(p.columns.len(), 1);
    }

    #[test]
    fn stored_columns_read_back() {
        let mut s = Settings::default();
        store_columns(&mut s, "UI.Bag4Columns", 7);
        store_columns(&mut s, "UI.Bag5Columns", 500);
        let p = P1Settings::from_settings(&s);
        assert_eq!(p.columns("UI.Bag4Columns"), Some(7));
        assert_eq!(p.columns("UI.Bag5Columns"), Some(MAX_BAG_COLUMNS));
    }
}
