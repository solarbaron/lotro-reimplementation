// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-user settings folder discovery.
//!
//! Spec-Ref: docs/specs/install-layout.md#81-location
//!
//! Only resolves paths and checks for existence. Nothing is created here; a
//! caller that needs to create its own settings folder does so outside the
//! install directory.

use std::path::{Path, PathBuf};

use crate::discover::SearchEnv;
use crate::fsutil;
use crate::install::GameInstall;

/// Default settings folder name (the launcher config's
/// `Product.DocumentFolder` value in the reference install).
pub const DEFAULT_DOCUMENT_FOLDER: &str = "The Lord of the Rings Online";

/// Fixed preferences file name when `--prefs` is not given.
pub const PREFERENCES_FILE: &str = "UserPreferences.ini";

/// The Proton prefix user.
const PROTON_USER: &str = "steamuser";

/// Inputs to settings-folder resolution that come from the command line or
/// the launcher config.
#[derive(Debug, Clone, Copy, Default)]
pub struct SettingsQuery<'a> {
    /// The `--prefs <file>` argument, already translated to a host path.
    pub prefs_file: Option<&'a Path>,
    /// `Product.DocumentFolder` from the launcher config, if known.
    pub document_folder: Option<&'a str>,
}

/// Which rule produced a settings folder (§8.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsSource {
    /// Step 1: the parent of `--prefs <file>`.
    PrefsArgument,
    /// Step 2: an override from the discovery source.
    InstallOverride,
    /// Step 3: the native Windows Documents folder.
    WindowsDocuments,
    /// Steps 4–5: inside the install's Wine/Proton prefix.
    Prefix {
        /// The prefix.
        prefix: PathBuf,
        /// The Windows user folder name used.
        user: String,
    },
    /// Step 6: XDG documents, `~/Documents` or `~`.
    Home,
}

/// A resolved settings folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsDir {
    /// The folder.
    pub path: PathBuf,
    /// The preferences file (`UserPreferences.ini`, or the `--prefs` file).
    pub preferences: PathBuf,
    /// The rule that produced it.
    pub source: SettingsSource,
    /// Whether the folder exists.
    pub exists: bool,
    /// Whether the preferences file exists.
    pub has_preferences: bool,
}

impl SettingsDir {
    fn new(path: PathBuf, preferences: Option<PathBuf>, source: SettingsSource) -> Self {
        let preferences = preferences.unwrap_or_else(|| {
            fsutil::resolve_ci(&path, PREFERENCES_FILE)
                .unwrap_or_else(|| path.join(PREFERENCES_FILE))
        });
        Self {
            exists: fsutil::is_dir(&path),
            has_preferences: fsutil::is_file(&preferences),
            path,
            preferences,
            source,
        }
    }
}

fn resolve_child(parent: &Path, name: &str) -> PathBuf {
    fsutil::resolve_ci(parent, name).unwrap_or_else(|| parent.join(name))
}

/// The prefix's candidate user folders in order: `steamuser`, the host user
/// name, then every other folder under `drive_c/users` except `Public`.
fn prefix_users(prefix: &Path, env: &SearchEnv) -> Vec<(String, PathBuf)> {
    let Some(users) = fsutil::resolve_path_ci(prefix, &["drive_c", "users"]) else {
        return Vec::new();
    };
    let mut entries = fsutil::list_dir(&users).unwrap_or_default();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    let mut push = |name: &str, path: PathBuf| {
        if !out.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)) && fsutil::is_dir(&path) {
            out.push((name.to_owned(), path));
        }
    };
    for preferred in [Some(PROTON_USER), env.user_name.as_deref()]
        .into_iter()
        .flatten()
    {
        if let Some(e) = fsutil::pick_ci(&entries, preferred) {
            push(&e.name, e.path.clone());
        }
    }
    for e in &entries {
        if !e.name.eq_ignore_ascii_case("Public") {
            push(&e.name, e.path.clone());
        }
    }
    out
}

/// All candidate settings folders in §8.1 order. Steps 1 and 2 are always
/// listed; later steps only when the folder exists, except that the final
/// home fallback is always listed so there is a place to create one.
pub fn settings_candidates(
    install: &GameInstall,
    query: &SettingsQuery<'_>,
    env: &SearchEnv,
) -> Vec<SettingsDir> {
    let name = query.document_folder.unwrap_or(DEFAULT_DOCUMENT_FOLDER);
    let mut out = Vec::new();

    if let Some(file) = query.prefs_file {
        let parent = file
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        out.push(SettingsDir::new(
            parent,
            Some(file.to_path_buf()),
            SettingsSource::PrefsArgument,
        ));
    }
    if let Some(dir) = &install.settings_override {
        out.push(SettingsDir::new(
            dir.clone(),
            None,
            SettingsSource::InstallOverride,
        ));
    }
    if let Some(docs) = &env.documents_dir {
        let d = SettingsDir::new(
            resolve_child(docs, name),
            None,
            SettingsSource::WindowsDocuments,
        );
        if d.exists {
            out.push(d);
        }
    }
    if let Some(prefix) = &install.prefix {
        for (user, user_dir) in prefix_users(prefix, env) {
            let Some(docs) = fsutil::resolve_ci(&user_dir, "Documents") else {
                continue;
            };
            let d = SettingsDir::new(
                resolve_child(&docs, name),
                None,
                SettingsSource::Prefix {
                    prefix: prefix.clone(),
                    user,
                },
            );
            if d.exists {
                out.push(d);
            }
        }
    }
    let mut home_dirs = Vec::new();
    if let Some(x) = &env.xdg_documents_dir {
        home_dirs.push(resolve_child(x, name));
    }
    if let Some(home) = &env.home {
        home_dirs.push(resolve_child(&resolve_child(home, "Documents"), name));
        home_dirs.push(resolve_child(home, name));
    }
    let mut fallback = None;
    for p in home_dirs {
        let d = SettingsDir::new(p, None, SettingsSource::Home);
        if d.exists {
            out.push(d);
        } else if fallback.is_none() {
            fallback = Some(d);
        }
    }
    if let Some(f) = fallback {
        out.push(f);
    }
    out
}

/// Resolve the settings folder (§8.1): an explicit `--prefs` always wins;
/// otherwise the first candidate that already holds `UserPreferences.ini`,
/// else the first candidate that exists, else the first candidate.
pub fn find_settings_dir(
    install: &GameInstall,
    query: &SettingsQuery<'_>,
    env: &SearchEnv,
) -> SettingsDir {
    let cands = settings_candidates(install, query, env);
    if let Some(first) = cands.first()
        && first.source == SettingsSource::PrefsArgument
    {
        return first.clone();
    }
    cands
        .iter()
        .find(|c| c.has_preferences)
        .or_else(|| cands.iter().find(|c| c.exists))
        .or_else(|| cands.first())
        .cloned()
        .unwrap_or_else(|| {
            // No home directory at all: fall back to a relative folder.
            SettingsDir::new(
                PathBuf::from(name_or_default(query)),
                None,
                SettingsSource::Home,
            )
        })
}

fn name_or_default<'a>(query: &SettingsQuery<'a>) -> &'a str {
    query.document_folder.unwrap_or(DEFAULT_DOCUMENT_FOLDER)
}
