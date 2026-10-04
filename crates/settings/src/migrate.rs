// SPDX-License-Identifier: GPL-3.0-or-later
//! Schema versioning and the migration hook.
//!
//! The settings document carries a top-level integer `schema_version`. When a
//! future change to the document shape is not backwards compatible, bump
//! [`SCHEMA_VERSION`] and append a [`Migration`] to [`MIGRATIONS`] that
//! rewrites a version `N` document (as a raw TOML table) into version `N + 1`.
//! Loading runs every step from the file's version up to the current one.
//!
//! Additive changes (a new optional field or section) need no migration:
//! every field has a default and unknown keys are tolerated.

use crate::error::{Result, SettingsError};

/// The schema version this build reads and writes.
pub const SCHEMA_VERSION: u32 = 1;

/// The top-level key that holds the schema version.
pub const SCHEMA_VERSION_KEY: &str = "schema_version";

/// Signature of one migration step: rewrite the table in place, or explain
/// why it cannot be migrated.
pub type MigrationFn = fn(&mut toml::Table) -> std::result::Result<(), String>;

/// One migration step from schema version `from` to `from + 1`.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    /// The version this step accepts.
    pub from: u32,
    /// The rewrite.
    pub apply: MigrationFn,
}

/// The registered migration steps, in ascending `from` order.
///
/// Empty while [`SCHEMA_VERSION`] is 1, the first published schema.
pub const MIGRATIONS: &[Migration] = &[];

/// Migrate `table` from schema version `from` to [`SCHEMA_VERSION`] using
/// [`MIGRATIONS`]. Returns the version reached.
///
/// # Errors
///
/// [`SettingsError::Migration`] when a step fails or is missing, and
/// [`SettingsError::NewerSchema`] when `from` is above [`SCHEMA_VERSION`].
pub fn migrate(table: &mut toml::Table, from: u32) -> Result<u32> {
    migrate_with(table, from, SCHEMA_VERSION, MIGRATIONS)
}

/// Like [`migrate`] but with an explicit target version and step list; this
/// is the hook tests and future tooling use.
///
/// # Errors
///
/// See [`migrate`].
pub fn migrate_with(
    table: &mut toml::Table,
    from: u32,
    target: u32,
    steps: &[Migration],
) -> Result<u32> {
    if from > target {
        return Err(SettingsError::NewerSchema {
            found: from,
            supported: target,
        });
    }
    let mut version = from;
    while version < target {
        let step =
            steps
                .iter()
                .find(|m| m.from == version)
                .ok_or_else(|| SettingsError::Migration {
                    from: version,
                    message: format!("no migration step registered for version {version}"),
                })?;
        (step.apply)(table).map_err(|message| SettingsError::Migration {
            from: version,
            message,
        })?;
        tracing::info!(from = version, to = version + 1, "migrated settings schema");
        version += 1;
    }
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rename_dirs(t: &mut toml::Table) -> std::result::Result<(), String> {
        let v = t.remove("dirs").ok_or("missing dirs")?;
        t.insert("game".into(), v);
        Ok(())
    }

    fn add_flag(t: &mut toml::Table) -> std::result::Result<(), String> {
        t.insert("flag".into(), toml::Value::Boolean(true));
        Ok(())
    }

    const STEPS: &[Migration] = &[
        Migration {
            from: 1,
            apply: rename_dirs,
        },
        Migration {
            from: 2,
            apply: add_flag,
        },
    ];

    #[test]
    fn current_version_needs_no_steps() {
        let mut t = toml::Table::new();
        assert_eq!(migrate(&mut t, SCHEMA_VERSION).unwrap(), SCHEMA_VERSION);
        assert!(t.is_empty());
    }

    #[test]
    fn runs_chain_in_order() {
        let mut t: toml::Table = toml::from_str("[dirs]\na = 1\n").unwrap();
        assert_eq!(migrate_with(&mut t, 1, 3, STEPS).unwrap(), 3);
        assert!(t.contains_key("game"));
        assert!(!t.contains_key("dirs"));
        assert_eq!(t.get("flag"), Some(&toml::Value::Boolean(true)));
    }

    #[test]
    fn failing_step_reports_version() {
        let mut t = toml::Table::new();
        match migrate_with(&mut t, 1, 3, STEPS) {
            Err(SettingsError::Migration { from: 1, .. }) => {}
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn missing_step_and_newer() {
        let mut t = toml::Table::new();
        assert!(matches!(
            migrate_with(&mut t, 0, 1, STEPS),
            Err(SettingsError::Migration { from: 0, .. })
        ));
        assert!(matches!(
            migrate(&mut t, SCHEMA_VERSION + 1),
            Err(SettingsError::NewerSchema { .. })
        ));
    }

    #[test]
    fn registry_is_consistent() {
        // Every step below the current version exists exactly once.
        let mut froms: Vec<u32> = MIGRATIONS.iter().map(|m| m.from).collect();
        froms.sort_unstable();
        let expected: Vec<u32> = (1..=SCHEMA_VERSION)
            .take_while(|&v| v < SCHEMA_VERSION)
            .collect();
        assert_eq!(froms, expected);
        assert!(MIGRATIONS.iter().all(|m| m.from < SCHEMA_VERSION));
    }
}
