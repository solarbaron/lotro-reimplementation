// SPDX-License-Identifier: GPL-3.0-or-later
//! A validated, usable game install.

use std::path::{Path, PathBuf};

use crate::archive::{ArchiveFile, ArchiveKind};
use crate::discover::{self, SearchEnv, Source};
use crate::error::{Error, Result};
use crate::fingerprint::SchemaFingerprint;
use crate::settings::{self, SettingsDir, SettingsQuery};
use crate::validate::{
    self, GameType, ProjectVersion, SplitSet, ValidationReport, ValidationWarning,
};

/// Steam-specific facts about an install found through Steam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamInfo {
    /// The library holding the game.
    pub library: PathBuf,
    /// Steam `buildid`, a coarse version label.
    pub buildid: Option<u64>,
    /// The Proton tool name from `compatdata/<id>/version`.
    pub proton_version: Option<String>,
}

/// A usable install directory. Everything here was read without writing to
/// the install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameInstall {
    /// The install directory (holds `client_general.dat`).
    pub root: PathBuf,
    /// Every archive found, sorted by lower-case file name.
    pub archives: Vec<ArchiveFile>,
    /// Language names whose `client_local_<Lang>.dat` passes the checks,
    /// sorted (same as [`ValidationReport::languages`]).
    pub languages: Vec<String>,
    /// The `x64` directory holding the 64-bit client, if present.
    pub client_dir: Option<PathBuf>,
    /// The game type (compatible or unknown; unsupported games never become a
    /// `GameInstall`).
    pub game_type: GameType,
    /// Split sets (unverified until membership records are supplied).
    pub split_sets: Vec<SplitSet>,
    /// The `ProjectVersion` file.
    pub project_version: ProjectVersion,
    /// True when content archives are missing (§3.3).
    pub incomplete: bool,
    /// Non-fatal findings from validation.
    pub warnings: Vec<ValidationWarning>,
    /// Every discovery source that named this install, in search order.
    pub sources: Vec<Source>,
    /// The Wine/Proton prefix the install belongs to, if any.
    pub prefix: Option<PathBuf>,
    /// A settings-folder override from the source (§8.1 step 2).
    pub settings_override: Option<PathBuf>,
    /// Steam facts, if found through Steam.
    pub steam: Option<SteamInfo>,
}

impl GameInstall {
    /// Validate `dir` and open it as an install. Fails when it is not usable
    /// or holds an unsupported game.
    ///
    /// The Steam library and Wine/Proton prefix are derived from the path
    /// itself (no search): `<library>/steamapps/common/<installdir>` of the
    /// game's app manifest gets that library's `compatdata/212500/pfx`
    /// (§5.3); a directory below a `drive_c` gets that prefix (§5.6). This
    /// keeps the settings folder (§8.1 steps 4–5) the same however the
    /// install was found.
    pub fn open(dir: &Path) -> Result<Self> {
        let mut install = Self::from_report(validate::validate(dir))?;
        let ctx = discover::context_from_path(dir);
        install.prefix = ctx.prefix;
        install.steam = ctx.steam;
        Ok(install)
    }

    /// Turn a validation report into an install, if it is usable. Only the
    /// report is used: `prefix` and `steam` stay empty (see
    /// [`GameInstall::open`]).
    pub fn from_report(report: ValidationReport) -> Result<Self> {
        if let GameType::Unsupported(game) = &report.game_type {
            return Err(Error::UnsupportedGame {
                path: report.root,
                game: game.clone(),
            });
        }
        if !report.usable {
            return Err(Error::NotUsable {
                path: report.root,
                problems: report.problems,
            });
        }
        let incomplete = report.incomplete();
        Ok(Self {
            root: report.root,
            archives: report.archives,
            languages: report.languages,
            client_dir: report.client_dir,
            game_type: report.game_type,
            split_sets: report.split_sets,
            project_version: report.project_version,
            incomplete,
            warnings: report.warnings,
            sources: Vec::new(),
            prefix: None,
            settings_override: None,
            steam: None,
        })
    }

    /// The archive of `kind` with member index `aux_index` (0 for the base).
    pub fn archive(&self, kind: &ArchiveKind, aux_index: u32) -> Option<&ArchiveFile> {
        self.archives
            .iter()
            .find(|a| &a.kind == kind && a.aux_index == aux_index)
    }

    /// Choose the language (§3.5) among the usable ones: `preferred` if
    /// present (exact case first, then case-insensitive), else `English`,
    /// else the first in lexicographic order. A `preferred` language that is
    /// absent or whose archive is broken is logged as a warning and the
    /// default is used instead.
    pub fn select_language(&self, preferred: Option<&str>) -> Option<&str> {
        let find = |name: &str| {
            self.languages
                .iter()
                .find(|l| l.as_str() == name)
                .or_else(|| self.languages.iter().find(|l| l.eq_ignore_ascii_case(name)))
        };
        let chosen = preferred.and_then(find);
        if let Some(p) = preferred
            && chosen.is_none()
        {
            tracing::warn!(
                requested = p,
                available = ?self.languages,
                "requested language is not available or its archive is broken; using the default"
            );
        }
        chosen
            .or_else(|| find("English"))
            .or_else(|| self.languages.first())
            .map(String::as_str)
    }

    /// The archive files the data layer should open for `kind`, as
    /// `(member index, path)` in member order. For a split set these are its
    /// members that pass the header checks, consecutive from the base and
    /// excluding stale leftovers; for any other kind, the base archive if it
    /// passes. Case duplicates were already resolved during validation.
    /// Empty when the base archive is missing or broken.
    pub fn set_member_paths(&self, kind: &ArchiveKind) -> Vec<(u32, &Path)> {
        let ok = |n: u32| self.archive(kind, n).filter(|a| a.status.is_ok());
        let members: Vec<u32> = match self.split_sets.iter().find(|s| &s.kind == kind) {
            Some(set) => set
                .members
                .iter()
                .copied()
                .filter(|n| !set.stale.contains(n))
                .collect(),
            None => vec![0],
        };
        let mut out = Vec::new();
        for (expected, n) in (0u32..).zip(members) {
            if n != expected {
                break;
            }
            match ok(n) {
                Some(a) => out.push((n, a.path.as_path())),
                None => break,
            }
        }
        out
    }

    /// Every archive the data layer should open, grouped by kind in the
    /// order of [`GameInstall::archives`]: see
    /// [`GameInstall::set_member_paths`].
    pub fn usable_archives(&self) -> Vec<(&ArchiveKind, u32, &Path)> {
        let mut out = Vec::new();
        for a in self.archives.iter().filter(|a| a.aux_index == 0) {
            out.extend(
                self.set_member_paths(&a.kind)
                    .into_iter()
                    .map(|(n, p)| (&a.kind, n, p)),
            );
        }
        out
    }

    /// The level-0 (header-only) fingerprint of this install (§9.4).
    pub fn schema_fingerprint(&self) -> SchemaFingerprint {
        SchemaFingerprint::compute(&self.archives)
    }

    /// Resolve the per-user settings folder (§8.1).
    pub fn settings_dir(&self, query: &SettingsQuery<'_>, env: &SearchEnv) -> SettingsDir {
        settings::find_settings_dir(self, query, env)
    }
}
