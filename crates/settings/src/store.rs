// SPDX-License-Identifier: GPL-3.0-or-later
//! Locating, loading (tolerantly) and atomically saving the settings file.
//!
//! # The read-only install guard
//!
//! Every write this module makes (the settings file, its temporary file,
//! backups, the lock file, pruning old backups) first resolves the real
//! destination and refuses it when it lies inside a game install. A path is
//! "inside" when, after resolution, it starts with:
//!
//! * a game directory recorded in the settings being saved (or, for writes
//!   made while loading, recorded in the file being loaded),
//! * the directory named by `TESSERA_GAME_DIR`, when set, or
//! * any directory that directly holds a `client_general.dat` entry
//!   (case-insensitive), which is what defines an install directory
//!   (`docs/specs/install-layout.md`, "Install directory").
//!
//! Resolution makes the path absolute, follows symlinks through every
//! existing component (including the final one), and folds `.`/`..` in the
//! components that do not exist yet the way the kernel will once the missing
//! directories are created. After the parent directory is created it is
//! canonicalized and checked again, so the check always applies to the
//! directory that is actually written. Any failure to resolve refuses the
//! write.

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{LoadWarning, Result, SettingsError};
use crate::migrate::SCHEMA_VERSION;
use crate::model::Settings;

/// Name of the per-user directory under the platform config directory.
pub const APP_DIR_NAME: &str = "tessera";
/// File name of the settings file.
pub const FILE_NAME: &str = "settings.toml";
/// Environment variable that overrides the settings file path (a full path
/// to the file). Useful for portable setups and tests.
pub const SETTINGS_FILE_ENV: &str = "TESSERA_SETTINGS_FILE";
/// Largest settings file that is read or written (1 MiB).
pub const MAX_FILE_SIZE: u64 = 1024 * 1024;
/// How many backup files (`<name>.<reason>-<time>[-n].bak`) are kept next to
/// the settings file; older ones are deleted. Only files whose names match
/// that exact pattern (with `reason` one of `corrupt`, `partial`,
/// `newer-v<N>`, `pre-v<N>`) are ever counted or deleted, the backup just
/// made is never deleted, and the newest backup of each kind is kept.
pub const MAX_BACKUPS: usize = 8;

/// Environment variable naming the game install; treated as read-only by
/// every write in this crate.
const GAME_DIR_ENV: &str = "TESSERA_GAME_DIR";
/// A directory that directly holds this file is a game install.
const INSTALL_MARKER: &str = "client_general.dat";
/// Directory entries examined per ancestor when looking for the marker with
/// a case-insensitive name.
const MAX_MARKER_SCAN: usize = 4096;
/// Directory entries examined when listing backups.
const MAX_BACKUP_SCAN: usize = 100_000;
/// Longest settings path accepted, in components.
const MAX_PATH_COMPONENTS: usize = 1024;
/// Symlink hops followed at the final path component.
const MAX_SYMLINK_HOPS: usize = 40;

/// How [`SettingsStore::load`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadStatus {
    /// No settings file exists yet; defaults were used.
    Missing,
    /// The file loaded without problems (a migration may have run).
    Clean,
    /// The file loaded, but some parts were reset to defaults or dropped.
    Recovered,
    /// Nothing in the file could be used; all settings are defaults.
    Defaulted,
}

/// Outcome of [`SettingsStore::load`].
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    /// The settings to use.
    pub settings: Settings,
    /// How loading went.
    pub status: LoadStatus,
    /// Problems found (each was also logged).
    pub warnings: Vec<LoadWarning>,
    /// Where a copy of the original file is, if one was made (damaged
    /// file, partial recovery, newer schema or pre-migration copy). When an
    /// identical backup of the same kind already existed, that one is
    /// returned instead of writing another copy.
    pub backup: Option<PathBuf>,
    /// The schema version the file declared, when a file was parsed.
    pub file_schema_version: Option<u32>,
}

/// Handle on one settings file path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsStore {
    path: PathBuf,
}

/// The default settings file path: `$TESSERA_SETTINGS_FILE` when set and
/// non-empty, otherwise `<config dir>/tessera/settings.toml` (on Linux
/// `$XDG_CONFIG_HOME` or `~/.config`).
///
/// # Errors
///
/// [`SettingsError::NoConfigDir`] when no config directory is known.
pub fn default_path() -> Result<PathBuf> {
    default_path_from(std::env::var_os(SETTINGS_FILE_ENV), dirs::config_dir())
}

fn default_path_from(env: Option<OsString>, config_dir: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = env.filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    config_dir
        .map(|d| d.join(APP_DIR_NAME).join(FILE_NAME))
        .ok_or(SettingsError::NoConfigDir)
}

impl SettingsStore {
    /// A store for an explicit file path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// A store at [`default_path`].
    ///
    /// # Errors
    ///
    /// [`SettingsError::NoConfigDir`].
    pub fn open_default() -> Result<Self> {
        default_path().map(Self::new)
    }

    /// The settings file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Load the settings, never failing (and never blocking on a FIFO or
    /// device at the settings path).
    ///
    /// The file itself is never moved, renamed or modified by loading.
    ///
    /// * Missing file: defaults, [`LoadStatus::Missing`].
    /// * Unreadable file or not a regular file: defaults plus a warning.
    /// * Larger than [`MAX_FILE_SIZE`]: defaults plus a warning; the file is
    ///   left alone and [`SettingsStore::save`] will refuse to replace it.
    /// * Not UTF-8, not TOML, or a failed migration: a
    ///   `.corrupt-<time>.bak` copy is made and defaults are used
    ///   ([`LoadStatus::Defaulted`]); the next save replaces the file.
    /// * Some sections, values or list entries malformed: the rest is kept,
    ///   the bad parts reset or dropped, and a `.partial-<time>.bak` copy is
    ///   made ([`LoadStatus::Recovered`]).
    /// * Older schema: migrated, after a `.pre-v<N>-<time>.bak` copy.
    /// * Newer schema: loaded as far as understood, after a
    ///   `.newer-v<N>-<time>.bak` copy.
    ///
    /// Backups are written next to the real file (after following
    /// symlinks) and are subject to the read-only install guard (see the
    /// module docs); a refused or failed backup is reported as
    /// [`LoadWarning::BackupFailed`].
    pub fn load(&self) -> Loaded {
        let mut warnings = Vec::new();
        let bytes = match read_bounded(&self.path) {
            Ok(Some(b)) => b,
            Ok(None) => {
                return Loaded {
                    settings: Settings::default(),
                    status: LoadStatus::Missing,
                    warnings,
                    backup: None,
                    file_schema_version: None,
                };
            }
            Err(ReadError::Io(e)) => {
                warnings.push(LoadWarning::Unreadable {
                    message: e.to_string(),
                });
                return self.finish_defaults(warnings, None, None);
            }
            Err(ReadError::TooLarge) => {
                warnings.push(LoadWarning::TooLarge {
                    limit: MAX_FILE_SIZE,
                });
                return self.finish_defaults(warnings, None, None);
            }
        };
        let text = match String::from_utf8(bytes) {
            Ok(t) => t,
            Err(e) => {
                warnings.push(LoadWarning::NotUtf8);
                return self.finish_defaults(warnings, None, Some(e.as_bytes()));
            }
        };
        let parsed = Settings::parse_lenient(&text);
        warnings.extend(parsed.warnings);
        if parsed.defaulted {
            let version = Some(parsed.schema_version);
            return self.finish_defaults(warnings, version, Some(text.as_bytes()));
        }
        let reason = if parsed.schema_version > SCHEMA_VERSION {
            Some(format!("newer-v{}", parsed.schema_version))
        } else if warnings.iter().any(|w| {
            matches!(
                w,
                LoadWarning::Section { .. }
                    | LoadWarning::Entry { .. }
                    | LoadWarning::Suppressed { .. }
            )
        }) {
            Some("partial".to_owned())
        } else if parsed.migrated {
            Some(format!("pre-v{SCHEMA_VERSION}"))
        } else {
            None
        };
        let backup = reason.and_then(|reason| {
            self.backup(
                &reason,
                text.as_bytes(),
                &parsed.settings.game_dirs(),
                &mut warnings,
            )
        });
        let status = if warnings.is_empty() {
            LoadStatus::Clean
        } else {
            LoadStatus::Recovered
        };
        log_warnings(&self.path, &warnings);
        Loaded {
            settings: parsed.settings,
            status,
            warnings,
            backup,
            file_schema_version: Some(parsed.schema_version),
        }
    }

    fn finish_defaults(
        &self,
        mut warnings: Vec<LoadWarning>,
        file_schema_version: Option<u32>,
        copy: Option<&[u8]>,
    ) -> Loaded {
        let backup = copy.and_then(|bytes| self.backup("corrupt", bytes, &[], &mut warnings));
        log_warnings(&self.path, &warnings);
        Loaded {
            settings: Settings::default(),
            status: LoadStatus::Defaulted,
            warnings,
            backup,
            file_schema_version,
        }
    }

    /// Write `bytes` (the file content just read) to a backup next to the
    /// real settings file, unless the guard refuses that directory.
    fn backup(
        &self,
        reason: &str,
        bytes: &[u8],
        game_dirs: &[&Path],
        warnings: &mut Vec<LoadWarning>,
    ) -> Option<PathBuf> {
        let result = write_target(&self.path)
            .map_err(|e| e.to_string())
            .and_then(|target| {
                guard(&target, game_dirs, &self.path)
                    .map_err(|e| format!("skipped: {e}"))
                    .map(|()| target)
            })
            .and_then(|target| backup_file(&target, reason, bytes).map_err(|e| e.to_string()));
        match result {
            Ok(p) => Some(p),
            Err(message) => {
                warnings.push(LoadWarning::BackupFailed { message });
                None
            }
        }
    }

    /// Strictly load the settings: `Ok(None)` when the file does not exist,
    /// an error for any problem (see [`Settings::from_toml_str`]). Never
    /// modifies the filesystem and never blocks on a FIFO.
    ///
    /// # Errors
    ///
    /// See [`SettingsError`].
    pub fn try_load(&self) -> Result<Option<Settings>> {
        let bytes = match read_bounded(&self.path) {
            Ok(Some(b)) => b,
            Ok(None) => return Ok(None),
            Err(ReadError::Io(source)) => {
                return Err(SettingsError::Io {
                    op: "read",
                    path: self.path.clone(),
                    source,
                });
            }
            Err(ReadError::TooLarge) => {
                return Err(SettingsError::TooLarge {
                    size: MAX_FILE_SIZE + 1,
                    limit: MAX_FILE_SIZE,
                });
            }
        };
        let text = String::from_utf8(bytes).map_err(|_| SettingsError::NotUtf8)?;
        Settings::from_toml_str(&text).map(Some)
    }

    /// Atomically save the settings: write a temporary file in the same
    /// directory, flush it to disk, then rename it over the settings file.
    /// Creates the parent directory if needed. A crash at any point leaves
    /// either the old or the new file, never a mix.
    ///
    /// When the settings path is a symlink, the file it points to is
    /// replaced (the temporary file is created next to the real file) and
    /// the link is kept.
    ///
    /// `save` writes the whole document and takes no lock: two programs that
    /// each hold a long-lived copy overwrite each other's changes. Use
    /// [`SettingsStore::update`] for read-modify-write.
    ///
    /// # Errors
    ///
    /// [`SettingsError::InsideGameDir`] (checked before anything is written,
    /// and again on the created directory) if the file would land inside a
    /// game install (see the module docs); [`SettingsError::WontReplace`]
    /// if the existing file is not a regular file or is larger than
    /// [`MAX_FILE_SIZE`]; [`SettingsError::TooLarge`],
    /// [`SettingsError::Serialize`] or [`SettingsError::Io`] (also when the
    /// path cannot be resolved).
    pub fn save(&self, settings: &Settings) -> Result<()> {
        let dirs = settings.game_dirs();
        let target = resolve_for_write(&self.path)?;
        guard(&target, &dirs, &self.path)?;
        let text = settings.to_toml_string()?;
        let size = text.len() as u64;
        if size > MAX_FILE_SIZE {
            return Err(SettingsError::TooLarge {
                size,
                limit: MAX_FILE_SIZE,
            });
        }
        refuse_foreign(&target)?;
        write_atomic(&target, text.as_bytes(), |real| {
            guard(real, &dirs, &self.path)
        })?;
        tracing::debug!(path = %target.display(), bytes = size, "saved settings");
        Ok(())
    }

    /// Read-modify-write under an advisory lock: lock
    /// `.<name>.lock` next to the real settings file, [`load`](Self::load)
    /// the current file, apply `f`, [`save`](Self::save), unlock. Returns
    /// the settings that were saved.
    ///
    /// Programs that share one settings file (for example the explorer and
    /// the client) should apply their changes through `update` instead of
    /// saving a snapshot taken at startup, so neither loses the other's
    /// changes. The lock file is left in place (removing it would race with
    /// other lockers) and is subject to the read-only install guard.
    ///
    /// # Errors
    ///
    /// Everything [`SettingsStore::save`] returns, plus
    /// [`SettingsError::Io`] when the lock cannot be created or taken.
    pub fn update(&self, f: impl FnOnce(&mut Settings)) -> Result<Settings> {
        let target = resolve_for_write(&self.path)?;
        let peeked = self.peek_game_dirs();
        let peeked: Vec<&Path> = peeked.iter().map(PathBuf::as_path).collect();
        guard(&target, &peeked, &self.path)?;
        let dir = parent_dir(&target);
        fs::create_dir_all(&dir).map_err(io_err("create directory", &dir))?;
        let lock_path = dir.join(format!(".{}.lock", file_name_lossy(&target)));
        guard(&lock_path, &peeked, &self.path)?;
        let lock = open_lock_file(&lock_path).map_err(io_err("open lock", &lock_path))?;
        lock.lock().map_err(io_err("lock", &lock_path))?;
        let mut loaded = self.load();
        f(&mut loaded.settings);
        let result = self.save(&loaded.settings);
        // Dropping the handle releases the lock; unlock explicitly anyway so
        // a failure to do so is at least logged.
        if let Err(e) = lock.unlock() {
            tracing::debug!(path = %lock_path.display(), error = %e, "cannot unlock settings");
        }
        result.map(|()| loaded.settings)
    }

    /// Game directories recorded in the current file, read without any side
    /// effects; empty when the file is missing or unusable.
    fn peek_game_dirs(&self) -> Vec<PathBuf> {
        let Ok(Some(bytes)) = read_bounded(&self.path) else {
            return Vec::new();
        };
        let Ok(text) = String::from_utf8(bytes) else {
            return Vec::new();
        };
        Settings::parse_lenient(&text)
            .settings
            .game_dirs()
            .into_iter()
            .map(Path::to_path_buf)
            .collect()
    }
}

/// Fail with [`SettingsError::InsideGameDir`] if writing the settings file
/// at `path` would land inside a game install: a directory recorded in
/// `settings`, the `TESSERA_GAME_DIR` directory, or a directory holding
/// `client_general.dat` (see the module docs). Touches nothing on disk
/// except to resolve paths and look for that marker file.
///
/// # Errors
///
/// [`SettingsError::InsideGameDir`], or [`SettingsError::Io`] when `path`
/// or a game directory cannot be resolved (the write would be refused).
pub fn check_not_inside_game_dirs(path: &Path, settings: &Settings) -> Result<()> {
    let target = resolve_for_write(path)?;
    guard(&target, &settings.game_dirs(), path)
}

/// Refuse `target` (already resolved) if it lies inside any game install.
/// `shown` is the path reported in the error.
fn guard(target: &Path, game_dirs: &[&Path], shown: &Path) -> Result<()> {
    let env = std::env::var_os(GAME_DIR_ENV)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    for dir in game_dirs.iter().copied().chain(env.as_deref()) {
        let resolved = resolve_path(dir).map_err(io_err("resolve", dir))?;
        if target.starts_with(&resolved) {
            return Err(SettingsError::InsideGameDir {
                path: shown.to_path_buf(),
                game_dir: dir.to_path_buf(),
            });
        }
    }
    if let Some(install) = target
        .parent()
        .and_then(|p| p.ancestors().find(|a| holds_install_marker(a)))
    {
        return Err(SettingsError::InsideGameDir {
            path: shown.to_path_buf(),
            game_dir: install.to_path_buf(),
        });
    }
    Ok(())
}

/// Whether `dir` directly holds an entry named `client_general.dat`
/// (case-insensitive, of any type).
fn holds_install_marker(dir: &Path) -> bool {
    if dir.as_os_str().is_empty() {
        return false;
    }
    if fs::symlink_metadata(dir.join(INSTALL_MARKER)).is_ok() {
        return true;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries
        .take(MAX_MARKER_SCAN)
        .filter_map(|e| e.ok())
        .any(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(INSTALL_MARKER))
        })
}

fn resolve_for_write(path: &Path) -> Result<PathBuf> {
    write_target(path).map_err(io_err("resolve", path))
}

/// The file a save to `path` really replaces: symlinks at the final
/// component are followed (bounded), then the result is resolved.
fn write_target(path: &Path) -> io::Result<PathBuf> {
    let names_file = |p: &Path| matches!(p.components().next_back(), Some(Component::Normal(_)));
    let mut p = path.to_path_buf();
    for _ in 0..MAX_SYMLINK_HOPS {
        if !names_file(&p) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "settings path does not name a file",
            ));
        }
        match fs::symlink_metadata(&p) {
            Ok(m) if m.file_type().is_symlink() => {
                let link = fs::read_link(&p)?;
                p = if link.is_absolute() {
                    link
                } else {
                    parent_dir(&p).join(link)
                };
            }
            _ => return resolve_path(&p),
        }
    }
    Err(io::Error::other("too many levels of symbolic links"))
}

/// Absolute form of `path` with every existing prefix canonicalized
/// (symlinks followed) and `.`/`..` folded in the part that does not exist
/// yet, as the kernel will resolve it once the missing directories are
/// created as plain directories.
fn resolve_path(path: &Path) -> io::Result<PathBuf> {
    let abs = std::path::absolute(path)?;
    if abs.components().count() > MAX_PATH_COMPONENTS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path has too many components",
        ));
    }
    let mut cur = PathBuf::new();
    for comp in abs.components() {
        match comp {
            Component::Prefix(_) | Component::RootDir => cur.push(comp.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir | Component::Normal(_) => {
                match fs::canonicalize(cur.join(comp.as_os_str())) {
                    Ok(c) => cur = c,
                    Err(_) => match comp {
                        // `cur` does not exist (or is not a directory), so
                        // nothing below it is a symlink: fold lexically.
                        Component::ParentDir => {
                            cur.pop();
                        }
                        _ => cur.push(comp.as_os_str()),
                    },
                }
            }
        }
    }
    Ok(cur)
}

/// Refuse to replace something at `target` that cannot be our settings file.
/// A directory is left to fail at the rename.
fn refuse_foreign(target: &Path) -> Result<()> {
    let reason = match fs::symlink_metadata(target) {
        Ok(m) if m.is_dir() || (m.is_file() && m.len() <= MAX_FILE_SIZE) => return Ok(()),
        Ok(m) if m.is_file() => format!("it is larger than {MAX_FILE_SIZE} bytes"),
        Ok(_) => "it is not a regular file".to_owned(),
        Err(_) => return Ok(()),
    };
    Err(SettingsError::WontReplace {
        path: target.to_path_buf(),
        reason,
    })
}

enum ReadError {
    Io(io::Error),
    TooLarge,
}

/// Open for reading without ever blocking on a FIFO.
fn open_nonblocking(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NONBLOCK);
    }
    opts.open(path)
}

fn open_lock_file(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NONBLOCK);
    }
    let f = opts.open(path)?;
    if !f.metadata()?.is_file() {
        return Err(io::Error::other("lock path is not a regular file"));
    }
    Ok(f)
}

/// Read at most [`MAX_FILE_SIZE`] bytes. `Ok(None)` when the file is absent.
/// The type is checked before opening, and the file is opened non-blocking
/// and checked again, so a FIFO or device is rejected without blocking.
fn read_bounded(path: &Path) -> std::result::Result<Option<Vec<u8>>, ReadError> {
    let not_regular = || ReadError::Io(io::Error::other("not a regular file"));
    match fs::metadata(path) {
        Ok(m) if !m.is_file() => return Err(not_regular()),
        Ok(m) if m.len() > MAX_FILE_SIZE => return Err(ReadError::TooLarge),
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(ReadError::Io(e)),
    }
    let file = match open_nonblocking(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(ReadError::Io(e)),
    };
    let meta = file.metadata().map_err(ReadError::Io)?;
    if !meta.is_file() {
        return Err(not_regular());
    }
    if meta.len() > MAX_FILE_SIZE {
        return Err(ReadError::TooLarge);
    }
    let mut buf = Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0));
    file.take(MAX_FILE_SIZE + 1)
        .read_to_end(&mut buf)
        .map_err(ReadError::Io)?;
    if buf.len() as u64 > MAX_FILE_SIZE {
        return Err(ReadError::TooLarge);
    }
    Ok(Some(buf))
}

fn log_warnings(path: &Path, warnings: &[LoadWarning]) {
    for w in warnings {
        tracing::warn!(path = %path.display(), "{w}");
    }
}

fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// One backup file found next to the settings file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct BackupFile {
    path: PathBuf,
    /// The full reason (`corrupt`, `newer-v7`, ...).
    reason: String,
    /// The reason without its version (`corrupt`, `newer`, ...).
    kind: &'static str,
    stamp: u64,
    seq: u32,
}

/// Parse `<base>.<reason>-<12 digits>[-<n>].bak`; `None` for any other name.
fn parse_backup_name(file: &str, base: &str) -> Option<(String, &'static str, u64, u32)> {
    let rest = file
        .strip_prefix(base)?
        .strip_prefix('.')?
        .strip_suffix(".bak")?;
    let (kind, after_kind) = ["corrupt", "partial", "newer-v", "pre-v"]
        .into_iter()
        .find_map(|k| rest.strip_prefix(k).map(|r| (k, r)))?;
    let (kind, tail) = if let Some(k) = kind.strip_suffix("-v") {
        let digits = after_kind.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || digits > 10 {
            return None;
        }
        (k, &after_kind[digits..])
    } else {
        (kind, after_kind)
    };
    let reason = rest[..rest.len() - tail.len()].to_owned();
    let tail = tail.strip_prefix('-')?;
    let (stamp, seq) = match tail.split_once('-') {
        Some((s, n)) => (s, Some(n)),
        None => (tail, None),
    };
    if stamp.len() != 12 || !stamp.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let seq = match seq {
        None => 0,
        Some(n) if (1..=3).contains(&n.len()) && n.bytes().all(|b| b.is_ascii_digit()) => {
            n.parse().ok()?
        }
        Some(_) => return None,
    };
    let kind = match kind {
        "corrupt" => "corrupt",
        "partial" => "partial",
        "newer" => "newer",
        _ => "pre",
    };
    Some((reason, kind, stamp.parse().ok()?, seq))
}

/// Our backups of `base` in `dir` (regular files only, not symlinks),
/// oldest first by the stamp in their names.
fn list_backups(dir: &Path, base: &str) -> Vec<BackupFile> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<BackupFile> = entries
        .take(MAX_BACKUP_SCAN)
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name();
            let (reason, kind, stamp, seq) = parse_backup_name(name.to_str()?, base)?;
            Some(BackupFile {
                path: e.path(),
                reason,
                kind,
                stamp,
                seq,
            })
        })
        .collect();
    out.sort_by(|a, b| (a.stamp, a.seq, &a.path).cmp(&(b.stamp, b.seq, &b.path)));
    out
}

/// Copy `bytes` to a fresh `<name>.<reason>-<secs>[-n].bak` next to
/// `target` (already resolved and guarded), then prune old backups. If the
/// newest backup with the same reason already holds exactly `bytes`, that
/// file is returned and nothing is written.
fn backup_file(target: &Path, reason: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    let dir = parent_dir(target);
    let name = file_name_lossy(target);
    let existing = list_backups(&dir, &name);
    if let Some(newest) = existing.iter().rev().find(|b| b.reason == reason)
        && let Ok(Some(old)) = read_bounded(&newest.path)
        && old == bytes
    {
        return Ok(newest.path.clone());
    }
    let stamp = unix_secs();
    let mut opened = None;
    for n in 0..100u32 {
        let candidate = if n == 0 {
            dir.join(format!("{name}.{reason}-{stamp:012}.bak"))
        } else {
            dir.join(format!("{name}.{reason}-{stamp:012}-{n}.bak"))
        };
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(f) => {
                opened = Some((candidate, f));
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    let (path, mut f) = opened.ok_or_else(|| io::Error::other("no free backup file name"))?;
    if let Err(e) = f.write_all(bytes).and_then(|()| f.sync_all()) {
        drop(f);
        let _ = fs::remove_file(&path);
        return Err(e);
    }
    tracing::warn!(backup = %path.display(), "backed up settings file");
    prune_backups(&dir, &name, &path);
    Ok(path)
}

/// Keep at most [`MAX_BACKUPS`] of our backups of `name` in `dir`, deleting
/// the oldest by name stamp. Never deletes `keep` or the newest backup of
/// each kind.
fn prune_backups(dir: &Path, name: &str, keep: &Path) {
    let list = list_backups(dir, name);
    if list.len() <= MAX_BACKUPS {
        return;
    }
    let mut protected: Vec<&Path> = vec![keep];
    for kind in ["corrupt", "partial", "newer", "pre"] {
        if let Some(b) = list.iter().rev().find(|b| b.kind == kind) {
            protected.push(&b.path);
        }
    }
    let mut excess = list.len() - MAX_BACKUPS;
    for b in &list {
        if excess == 0 {
            break;
        }
        if protected.contains(&b.path.as_path()) {
            continue;
        }
        match fs::remove_file(&b.path) {
            Ok(()) => excess -= 1,
            Err(e) => {
                tracing::debug!(path = %b.path.display(), error = %e, "cannot prune settings backup")
            }
        }
    }
}

fn parent_dir(path: &Path) -> PathBuf {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

fn file_name_lossy(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| FILE_NAME.to_owned())
}

fn io_err(op: &'static str, p: &Path) -> impl FnOnce(io::Error) -> SettingsError {
    let p = p.to_path_buf();
    move |source| SettingsError::Io {
        op,
        path: p,
        source,
    }
}

/// Write `bytes` to `target` (already resolved) via a temporary sibling file
/// and a rename. After the parent directory exists it is canonicalized and
/// the real destination is passed to `verify` before anything is created
/// in it.
fn write_atomic(
    target: &Path,
    bytes: &[u8],
    verify: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let dir = parent_dir(target);
    fs::create_dir_all(&dir).map_err(io_err("create directory", &dir))?;
    let dir = fs::canonicalize(&dir).map_err(io_err("resolve", &dir))?;
    let name = target.file_name().ok_or_else(|| SettingsError::Io {
        op: "resolve",
        path: target.to_path_buf(),
        source: io::Error::new(io::ErrorKind::InvalidInput, "path does not name a file"),
    })?;
    let path = dir.join(name);
    verify(&path)?;
    let name = name.to_string_lossy();
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let mut opened = None;
    for n in 0..16u32 {
        let tmp = dir.join(format!(".{name}.tmp-{pid}-{nanos}-{n}"));
        match OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(f) => {
                opened = Some((tmp, f));
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io_err("create", &tmp)(e)),
        }
    }
    let Some((tmp, mut file)) = opened else {
        return Err(SettingsError::Io {
            op: "create",
            path: dir,
            source: io::Error::other("no free temporary file name"),
        });
    };
    let result = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(io_err("write", &tmp))
        .and_then(|()| {
            drop(file);
            fs::rename(&tmp, &path).map_err(io_err("rename", &path))
        });
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
        return result;
    }
    sync_dir(&dir);
    Ok(())
}

/// Best-effort flush of the directory entry after the rename.
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path_prefers_env() {
        let p = default_path_from(Some("/x/s.toml".into()), Some("/cfg".into())).unwrap();
        assert_eq!(p, PathBuf::from("/x/s.toml"));
        let p = default_path_from(Some("".into()), Some("/cfg".into())).unwrap();
        assert_eq!(p, PathBuf::from("/cfg/tessera/settings.toml"));
        assert!(matches!(
            default_path_from(None, None),
            Err(SettingsError::NoConfigDir)
        ));
    }

    #[test]
    fn resolve_missing_path() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a/b/c.toml");
        let c = resolve_path(&p).unwrap();
        assert!(c.ends_with("a/b/c.toml"));
        assert!(c.starts_with(fs::canonicalize(dir.path()).unwrap()));
    }

    /// Regression: `..` after a missing directory folds onto the parent
    /// instead of making resolution give up.
    #[test]
    fn resolve_folds_dotdot_after_missing_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let base = fs::canonicalize(dir.path()).unwrap();
        fs::create_dir(base.join("game")).unwrap();
        assert_eq!(
            resolve_path(&base.join("missing/../game/settings.toml")).unwrap(),
            base.join("game/settings.toml")
        );
        assert_eq!(
            resolve_path(&base.join("m1/m2/./../../game/x/../s.toml")).unwrap(),
            base.join("game/s.toml")
        );
        assert_eq!(
            resolve_path(&base.join("game/../cfg/settings.toml")).unwrap(),
            base.join("cfg/settings.toml")
        );
        assert!(write_target(&base.join("game/..")).is_err());
        assert!(write_target(Path::new("/")).is_err());
    }

    #[test]
    fn backup_names_parse_strictly() {
        let b = "settings.toml";
        let ok = |n: &str| parse_backup_name(n, b);
        assert_eq!(
            ok("settings.toml.corrupt-001791084283.bak"),
            Some(("corrupt".into(), "corrupt", 1_791_084_283, 0))
        );
        assert_eq!(
            ok("settings.toml.newer-v7-001791084296-3.bak"),
            Some(("newer-v7".into(), "newer", 1_791_084_296, 3))
        );
        assert_eq!(
            ok("settings.toml.pre-v1-000000000001.bak"),
            Some(("pre-v1".into(), "pre", 1, 0))
        );
        assert!(ok("settings.toml.partial-000000000001-12.bak").is_some());
        for bad in [
            "settings.toml.mine.bak",
            "settings.toml.my-handmade.bak",
            "settings.toml.corrupt-1791084283.bak",
            "settings.toml.corrupt-001791084283x.bak",
            "settings.toml.corrupt-001791084283-.bak",
            "settings.toml.corrupt-001791084283-1234.bak",
            "settings.toml.newer-001791084283.bak",
            "settings.toml.newer-v-001791084283.bak",
            "settings.toml.corruptX-001791084283.bak",
            "settings.toml.corrupt-001791084283.bak.bak",
            "settings.toml.toml.corrupt-001791084283.bak",
            "other.toml.corrupt-001791084283.bak",
            "settings.tomlcorrupt-001791084283.bak",
        ] {
            assert_eq!(ok(bad), None, "{bad}");
        }
    }
}
