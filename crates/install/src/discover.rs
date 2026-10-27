// SPDX-License-Identifier: GPL-3.0-or-later
//! Locating installs.
//!
//! Spec-Ref: docs/specs/install-layout.md#5-locating-installs
//!
//! Discovery reads only metadata files, directory listings and archive
//! headers. It never writes anything and never touches the network.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::fsutil::{self, MAX_DIRS_PER_WALK, WalkLimits};
use crate::install::{GameInstall, SteamInfo};
use crate::onelauncher;
use crate::steam;
use crate::validate::{self, ValidationProblem};

/// Environment variable naming the game directory explicitly.
pub const GAME_DIR_ENV: &str = "TESSERA_GAME_DIR";

/// Folder of the standalone installer under `Program Files (x86)`, and its
/// older vendor folder (§5.5).
const STANDALONE_RELATIVE: [&[&str]; 2] = [
    &[
        "Program Files (x86)",
        "StandingStoneGames",
        "The Lord of the Rings Online",
    ],
    &[
        "Program Files (x86)",
        "Turbine",
        "The Lord of the Rings Online",
    ],
];

/// Maximum walk depth below a prefix's `Program Files` (§5.8).
const PREFIX_WALK_DEPTH: usize = 5;
/// Maximum walk depth for the last-resort home scan (§5.6.4).
const HOME_SCAN_DEPTH: usize = 3;

/// Everything discovery reads from the process environment, made explicit so
/// tests can run against fake home directories without touching real
/// environment variables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchEnv {
    /// The user's home directory.
    pub home: Option<PathBuf>,
    /// Value of [`GAME_DIR_ENV`], if set and non-empty.
    pub game_dir: Option<PathBuf>,
    /// `WINEPREFIX`.
    pub wine_prefix: Option<PathBuf>,
    /// `XDG_DATA_HOME` (default `~/.local/share`).
    pub xdg_data_home: Option<PathBuf>,
    /// `XDG_CACHE_HOME` (default `~/.cache`).
    pub xdg_cache_home: Option<PathBuf>,
    /// `XDG_DOCUMENTS_DIR`.
    pub xdg_documents_dir: Option<PathBuf>,
    /// The host user name (`USER` / `USERNAME`).
    pub user_name: Option<String>,
    /// The native Windows Documents known folder. This crate does not call
    /// platform APIs; a front end that can resolve the known folder sets it.
    pub documents_dir: Option<PathBuf>,
    /// Additional Steam roots to try after the built-in ones.
    pub extra_steam_roots: Vec<PathBuf>,
    /// Whether to look at fixed system-wide locations (Windows default
    /// install paths). Off for isolated test environments.
    pub system_locations: bool,
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

impl SearchEnv {
    /// Read the search environment from the current process.
    pub fn from_process() -> Self {
        let home = env_path("HOME")
            .or_else(|| env_path("USERPROFILE"))
            .or_else(std::env::home_dir);
        Self {
            home,
            game_dir: env_path(GAME_DIR_ENV),
            wine_prefix: env_path("WINEPREFIX"),
            xdg_data_home: env_path("XDG_DATA_HOME"),
            xdg_cache_home: env_path("XDG_CACHE_HOME"),
            xdg_documents_dir: env_path("XDG_DOCUMENTS_DIR"),
            user_name: std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .ok()
                .filter(|s| !s.is_empty()),
            documents_dir: None,
            extra_steam_roots: Vec::new(),
            system_locations: true,
        }
    }

    /// An environment that only knows a home directory. Useful for tests.
    pub fn isolated(home: impl Into<PathBuf>) -> Self {
        Self {
            home: Some(home.into()),
            ..Self::default()
        }
    }

    /// `XDG_DATA_HOME` or `~/.local/share`.
    pub fn data_home(&self) -> Option<PathBuf> {
        self.xdg_data_home
            .clone()
            .or_else(|| self.home.as_ref().map(|h| h.join(".local/share")))
    }

    /// `XDG_CACHE_HOME` or `~/.cache`.
    pub fn cache_home(&self) -> Option<PathBuf> {
        self.xdg_cache_home
            .clone()
            .or_else(|| self.home.as_ref().map(|h| h.join(".cache")))
    }
}

/// Where a candidate install was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// An explicit `--game-dir` option.
    Explicit,
    /// The [`GAME_DIR_ENV`] environment variable.
    Environment,
    /// A Steam library.
    Steam {
        /// The library root.
        library: PathBuf,
    },
    /// A third-party launcher's per-game config.
    LauncherConfig {
        /// The config file.
        config: PathBuf,
    },
    /// A default path of the standalone installer.
    Standalone,
    /// A walk of a Wine/Proton prefix's `Program Files`.
    WinePrefix {
        /// The prefix (directory holding `drive_c`).
        prefix: PathBuf,
    },
    /// The last-resort bounded scan of home game folders.
    HomeScan,
}

impl Source {
    /// The kind of location, for a log line: never the path a source holds (a Steam
    /// library, a launcher config, a prefix are all below the user's home).
    fn kind(&self) -> &'static str {
        match self {
            Self::Explicit => "named by the user",
            Self::Environment => "environment variable",
            Self::Steam { .. } => "Steam library",
            Self::LauncherConfig { .. } => "launcher config",
            Self::Standalone => "standalone installer path",
            Self::WinePrefix { .. } => "Wine prefix",
            Self::HomeScan => "home scan",
        }
    }
}

/// The kinds of `sources`, for a log line ([`Source::kind`]).
fn source_kinds(sources: &[Source]) -> Vec<&'static str> {
    sources.iter().map(Source::kind).collect()
}

/// The kind of a validation problem, for a log line: no path, no file content.
fn problem_kind(problem: &ValidationProblem) -> &'static str {
    match problem {
        ValidationProblem::NotADirectory => "not a directory",
        ValidationProblem::CannotList(_) => "cannot list",
        ValidationProblem::MissingCoreArchive(_) => "missing core archive",
        ValidationProblem::CoreArchiveFailed { .. } => "core archive failed",
        ValidationProblem::NoLanguageArchive => "no language archive",
        ValidationProblem::NoUsableLanguageArchive => "no usable language archive",
        ValidationProblem::UnsupportedGame(_) => "unsupported game",
    }
}

/// A candidate that was found but is not usable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected {
    /// The directory.
    pub path: PathBuf,
    /// Sources that named it.
    pub sources: Vec<Source>,
    /// Why it was rejected (including [`ValidationProblem::UnsupportedGame`]).
    pub problems: Vec<ValidationProblem>,
}

/// The result of a discovery run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Discovery {
    /// Usable installs in search order. The first is the default.
    pub installs: Vec<GameInstall>,
    /// Candidates that were found but are not usable (including installs of
    /// unsupported games).
    pub rejected: Vec<Rejected>,
}

impl Discovery {
    /// The default install: the first usable one in search order.
    pub fn default_install(&self) -> Option<&GameInstall> {
        self.installs.first()
    }
}

#[derive(Debug)]
struct Candidate {
    path: PathBuf,
    sources: Vec<Source>,
    prefix: Option<PathBuf>,
    settings_override: Option<PathBuf>,
    steam: Option<SteamInfo>,
}

#[derive(Debug, Default)]
struct Candidates {
    list: Vec<Candidate>,
    /// Canonical path → index in `list`.
    index: HashMap<PathBuf, usize>,
}

impl Candidates {
    fn add(
        &mut self,
        path: PathBuf,
        source: Source,
        prefix: Option<PathBuf>,
        settings_override: Option<PathBuf>,
        steam: Option<SteamInfo>,
    ) {
        if !fsutil::is_dir(&path) {
            tracing::debug!(source = source.kind(), "candidate does not exist");
            return;
        }
        let canon = fsutil::canonical(&path);
        if let Some(c) = self.index.get(&canon).map(|&i| &mut self.list[i]) {
            if !c.sources.contains(&source) {
                c.sources.push(source);
            }
            c.prefix = c.prefix.take().or(prefix);
            c.settings_override = c.settings_override.take().or(settings_override);
            c.steam = c.steam.take().or(steam);
            return;
        }
        tracing::debug!(source = source.kind(), "candidate");
        self.index.insert(canon, self.list.len());
        self.list.push(Candidate {
            path,
            sources: vec![source],
            prefix,
            settings_override,
            steam,
        });
    }
}

/// Discover installs using the current process environment (§5.1).
pub fn discover() -> Result<Discovery> {
    discover_with(&SearchEnv::from_process(), None)
}

/// Context derived from an install directory's own path, without a search:
/// the Steam library and Proton prefix when the directory is
/// `<library>/steamapps/common/<installdir>` of the game's app manifest
/// (§5.3, P17), else the Wine prefix when an ancestor is `drive_c` (§5.6).
#[derive(Debug, Default)]
pub(crate) struct PathContext {
    pub(crate) prefix: Option<PathBuf>,
    pub(crate) steam: Option<SteamInfo>,
}

fn name_is(p: &Path, name: &str) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case(name))
}

fn steam_info(app: &steam::SteamApp) -> SteamInfo {
    SteamInfo {
        library: app.library.clone(),
        buildid: app.manifest.as_ref().and_then(|m| m.buildid),
        proton_version: app.proton_version.clone(),
    }
}

fn context_of(dir: &Path) -> PathContext {
    if let Some(common) = dir.parent()
        && name_is(common, "common")
        && let Some(steamapps) = common.parent()
        && name_is(steamapps, "steamapps")
        && let Some(lib) = steamapps.parent()
    {
        let library = steam::SteamLibrary {
            path: lib.to_path_buf(),
            lists_app: true,
        };
        if let Some(app) = steam::find_in_library(&library)
            && fsutil::canonical(&app.game_dir) == fsutil::canonical(dir)
        {
            return PathContext {
                prefix: app.proton_prefix.clone(),
                steam: Some(steam_info(&app)),
            };
        }
    }
    if let Some(drive_c) = dir.ancestors().skip(1).find(|a| name_is(a, "drive_c"))
        && let Some(prefix) = drive_c.parent().and_then(normalize_prefix)
    {
        return PathContext {
            prefix: Some(prefix),
            steam: None,
        };
    }
    PathContext::default()
}

/// [`context_of`] for the path as given (made absolute) and, failing that,
/// its canonical form (symlinks resolved).
pub(crate) fn context_from_path(dir: &Path) -> PathContext {
    let given = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
    let canon = fsutil::canonical(dir);
    for p in [&given, &canon] {
        let ctx = context_of(p);
        if ctx.prefix.is_some() || ctx.steam.is_some() {
            return ctx;
        }
        if given == canon {
            break;
        }
    }
    PathContext::default()
}

/// Open the single directory the user named. Errors if it is not usable.
/// [`GameInstall::open`] derives the Steam/prefix context from the path, so
/// the install resolves the same settings folder as when it is found by
/// searching (§8.1 steps 4–5).
fn open_named(path: &Path, source: Source) -> Result<Discovery> {
    let mut install = GameInstall::open(path)?;
    tracing::info!(
        source = source.kind(),
        "using the install named by the user"
    );
    install.sources = vec![source];
    Ok(Discovery {
        installs: vec![install],
        rejected: Vec::new(),
    })
}

/// Discover installs (§5.1).
///
/// `explicit` is a `--game-dir` option. When it, or `env.game_dir`, is given,
/// nothing else is searched and an unusable directory is an error. Otherwise
/// every source is searched, candidates are de-duplicated by canonical path
/// and validated, and all usable installs are returned in search order.
pub fn discover_with(env: &SearchEnv, explicit: Option<&Path>) -> Result<Discovery> {
    if let Some(dir) = explicit {
        return open_named(dir, Source::Explicit);
    }
    if let Some(dir) = &env.game_dir {
        return open_named(dir, Source::Environment);
    }

    let mut cands = Candidates::default();
    let libraries = steam::all_libraries(env);
    tracing::debug!(libraries = libraries.len(), "Steam libraries");

    // 3. Steam.
    for lib in &libraries {
        if let Some(app) = steam::find_in_library(lib) {
            let info = steam_info(&app);
            cands.add(
                app.game_dir,
                Source::Steam {
                    library: app.library,
                },
                app.proton_prefix,
                None,
                Some(info),
            );
        }
    }
    if cfg!(windows) && env.system_locations {
        // Adds the fixed fallback path; duplicates merge by canonical path.
        for app in steam::find_steam_apps(env) {
            cands.add(
                app.game_dir,
                Source::Steam {
                    library: app.library,
                },
                None,
                None,
                None,
            );
        }
    }

    // 4. Third-party launcher configs.
    let launcher_cfgs = env
        .data_home()
        .map(|d| onelauncher::read_configs(&d))
        .unwrap_or_default();
    for cfg in &launcher_cfgs {
        if let Some(dir) = &cfg.game_directory {
            let prefix = cfg.wine_prefix.as_deref().and_then(normalize_prefix);
            cands.add(
                dir.clone(),
                Source::LauncherConfig {
                    config: cfg.path.clone(),
                },
                prefix,
                cfg.game_settings_directory.clone(),
                None,
            );
        }
    }

    // 5. Standalone installer default paths (native, then inside prefixes).
    let prefixes = find_prefixes(env, &libraries, &launcher_cfgs);
    tracing::debug!(
        launcher_configs = launcher_cfgs.len(),
        prefixes = prefixes.len(),
        "launcher configs and Wine prefixes"
    );
    if cfg!(windows) && env.system_locations {
        for rel in STANDALONE_RELATIVE {
            let p = rel.iter().fold(PathBuf::from(r"C:\"), |acc, c| acc.join(c));
            cands.add(p, Source::Standalone, None, None, None);
        }
    }
    for pfx in &prefixes {
        for rel in STANDALONE_RELATIVE {
            let mut parts = vec!["drive_c"];
            parts.extend_from_slice(rel);
            if let Some(p) = fsutil::resolve_path_ci(pfx, &parts) {
                cands.add(p, Source::Standalone, Some(pfx.clone()), None, None);
            }
        }
    }

    // 6. Wine/Proton/Lutris/CrossOver prefixes.
    for pfx in &prefixes {
        for pf in ["Program Files", "Program Files (x86)"] {
            let Some(root) = fsutil::resolve_path_ci(pfx, &["drive_c", pf]) else {
                continue;
            };
            let limits = WalkLimits {
                max_depth: PREFIX_WALK_DEPTH,
                max_dirs: MAX_DIRS_PER_WALK,
            };
            for dir in fsutil::find_install_dirs(&root, limits) {
                cands.add(
                    dir,
                    Source::WinePrefix {
                        prefix: pfx.clone(),
                    },
                    Some(pfx.clone()),
                    None,
                    None,
                );
            }
        }
    }

    // 7. Bounded home scan.
    let mut scan_roots = Vec::new();
    if let Some(home) = &env.home {
        scan_roots.push(home.join("games"));
        scan_roots.push(home.join("Games"));
    }
    for lib in &libraries {
        if let Some(common) = fsutil::resolve_path_ci(&lib.path, &["steamapps", "common"]) {
            scan_roots.push(common);
        }
    }
    for root in dedup_canonical(scan_roots) {
        let limits = WalkLimits {
            max_depth: HOME_SCAN_DEPTH,
            max_dirs: MAX_DIRS_PER_WALK,
        };
        for dir in fsutil::find_install_dirs(&root, limits) {
            cands.add(dir, Source::HomeScan, None, None, None);
        }
    }

    let mut out = Discovery::default();
    let candidates = cands.list.len();
    for c in cands.list {
        let report = validate::validate(&c.path);
        if !report.usable {
            let problems: Vec<&str> = report.problems.iter().map(problem_kind).collect();
            tracing::debug!(sources = ?source_kinds(&c.sources), ?problems, "candidate rejected");
            out.rejected.push(Rejected {
                path: c.path,
                sources: c.sources,
                problems: report.problems,
            });
            continue;
        }
        match GameInstall::from_report(report) {
            Ok(mut install) => {
                tracing::info!(sources = ?source_kinds(&c.sources), "found install");
                install.sources = c.sources;
                // A source that found the install without its prefix (home
                // scan, standalone path) gets the same context an explicit
                // directory would.
                let ctx = if c.prefix.is_none() || c.steam.is_none() {
                    context_from_path(&c.path)
                } else {
                    PathContext::default()
                };
                install.prefix = c.prefix.or(ctx.prefix);
                install.settings_override = c.settings_override;
                install.steam = c.steam.or(ctx.steam);
                out.installs.push(install);
            }
            Err(e) => {
                // A usable report always converts; map the error just in case.
                tracing::debug!(sources = ?source_kinds(&c.sources), error = %e.class(), "candidate rejected");
                let problems = match e {
                    Error::NotUsable { problems, .. } => problems,
                    Error::UnsupportedGame { game, .. } => {
                        vec![ValidationProblem::UnsupportedGame(game)]
                    }
                    _ => Vec::new(),
                };
                out.rejected.push(Rejected {
                    path: c.path,
                    sources: c.sources,
                    problems,
                });
            }
        }
    }
    tracing::info!(
        candidates,
        usable = out.installs.len(),
        rejected = out.rejected.len(),
        "install search done"
    );
    Ok(out)
}

fn dedup_canonical(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for p in paths {
        if !fsutil::is_dir(&p) {
            continue;
        }
        if seen.insert(fsutil::canonical(&p)) {
            out.push(p);
        }
    }
    out
}

/// `X` is a prefix if `X/drive_c` exists; a Proton wrapper if `X/pfx/drive_c`
/// exists, in which case the prefix is `X/pfx` (§5.6.1).
pub fn normalize_prefix(x: &Path) -> Option<PathBuf> {
    if fsutil::resolve_ci(x, "drive_c").is_some_and(|p| fsutil::is_dir(&p)) {
        return Some(x.to_path_buf());
    }
    let pfx = fsutil::resolve_ci(x, "pfx")?;
    fsutil::resolve_ci(&pfx, "drive_c")
        .is_some_and(|p| fsutil::is_dir(&p))
        .then_some(pfx)
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(mut entries) = fsutil::list_dir(dir) else {
        return Vec::new();
    };
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
        .into_iter()
        .filter(|e| fsutil::is_dir(&e.path))
        .map(|e| e.path)
        .collect()
}

/// Gather Wine/Proton/CrossOver prefixes (§5.6.1), de-duplicated.
fn find_prefixes(
    env: &SearchEnv,
    libraries: &[steam::SteamLibrary],
    launcher_cfgs: &[onelauncher::LauncherGameConfig],
) -> Vec<PathBuf> {
    let mut raw: Vec<PathBuf> = Vec::new();
    if let Some(home) = &env.home
        && let Ok(mut entries) = fsutil::list_dir(home)
    {
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        for e in entries {
            if e.name.to_ascii_lowercase().contains("wine") && fsutil::is_dir(&e.path) {
                raw.push(e.path);
            }
        }
    }
    for lib in libraries {
        if let Some(cd) = fsutil::resolve_path_ci(&lib.path, &["steamapps", "compatdata"]) {
            raw.extend(subdirs(&cd));
        }
    }
    if let Some(home) = &env.home {
        raw.extend(subdirs(&home.join("games")));
        raw.extend(subdirs(&home.join("Games")));
        raw.extend(subdirs(
            &home.join("Library/Application Support/CrossOver/Bottles"),
        ));
    }
    if let Some(p) = &env.wine_prefix {
        raw.push(p.clone());
    }
    if let Some(cache) = env.cache_home() {
        raw.push(cache.join("onelauncher/wine/prefix"));
    }
    raw.extend(launcher_cfgs.iter().filter_map(|c| c.wine_prefix.clone()));

    let mut out: Vec<PathBuf> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for x in raw {
        if let Some(p) = normalize_prefix(&x)
            && seen.insert(fsutil::canonical(&p))
        {
            out.push(p);
        }
    }
    out
}
