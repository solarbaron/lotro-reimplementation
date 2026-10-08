// SPDX-License-Identifier: GPL-3.0-or-later
//! The data directory (`test-server.md` §3): location, install guard, layout, locks, atomic
//! writes and the server key.
//!
//! Nothing in the data directory is ever committed, published or sent to clients. The
//! server never opens a file inside a game installation: the install guard only lists
//! directory names (§3.1).

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tessera_protocol::transport::ServerIdentity;

use crate::error::ServerError;

/// The environment variable that names the data directory (§2.1).
pub const DATA_DIR_ENV: &str = "TESSERA_SERVER_DIR";

/// Most directory entries the install guard lists per directory (bounds its time).
const GUARD_MAX_ENTRIES: usize = 200_000;

/// The platform's default data directory plus `tessera-server` (§3.1), if the platform's
/// base directory is known.
#[must_use]
pub fn default_data_dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    }?;
    Some(base.join("tessera-server"))
}

/// Resolves symbolic links in `path` as far as it exists, and appends the rest.
fn resolve(path: &Path) -> PathBuf {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |c| c.join(path))
    };
    let mut existing = abs.clone();
    let mut rest = Vec::new();
    loop {
        if let Ok(c) = fs::canonicalize(&existing) {
            let mut out = c;
            for part in rest.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (
            existing.file_name().map(|n| n.to_os_string()),
            existing.parent(),
        ) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                existing = parent.to_path_buf();
            }
            _ => return abs,
        }
    }
}

fn is_game_archive_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    (lower.starts_with("client_") && lower.ends_with(".dat")) || lower.ends_with(".datx")
}

fn dir_has(dir: &Path, pred: impl Fn(&str) -> bool) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries
        .take(GUARD_MAX_ENTRIES)
        .filter_map(Result::ok)
        .any(|e| e.file_name().to_str().is_some_and(&pred))
}

/// The install guard for a path (§3.1, `server-content.md` §10 E2): true when the path
/// itself (a directory) or any of its ancestor directories contains a file whose name
/// matches `client_*.dat` or ends in `.datx`. Symbolic links are resolved first. Only
/// directory names are listed; no such file is ever opened.
#[must_use]
pub fn install_guard_hit(path: &Path) -> bool {
    let resolved = resolve(path);
    let mut current: Option<&Path> = Some(&resolved);
    while let Some(p) = current {
        if p.is_dir() && dir_has(p, is_game_archive_name) {
            return true;
        }
        current = p.parent();
    }
    false
}

/// The install guard for a data directory: [`install_guard_hit`], or any file ending in
/// `.dat` or `.datx` inside it.
#[must_use]
pub fn data_dir_guard_hit(path: &Path) -> bool {
    if install_guard_hit(path) {
        return true;
    }
    let resolved = resolve(path);
    dir_has(&resolved, |n| {
        let l = n.to_ascii_lowercase();
        l.ends_with(".dat") || l.ends_with(".datx")
    })
}

/// A server data directory (§3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    /// A data directory at `root` (not created or checked).
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// `server.toml`.
    #[must_use]
    pub fn config_path(&self) -> PathBuf {
        self.root.join("server.toml")
    }

    /// `server.key`.
    #[must_use]
    pub fn key_path(&self) -> PathBuf {
        self.root.join("server.key")
    }

    /// `server.lock`.
    #[must_use]
    pub fn lock_path(&self) -> PathBuf {
        self.root.join("server.lock")
    }

    /// `accounts.toml`.
    #[must_use]
    pub fn accounts_path(&self) -> PathBuf {
        self.root.join("accounts.toml")
    }

    /// `accounts.lock`.
    #[must_use]
    pub fn accounts_lock_path(&self) -> PathBuf {
        self.root.join("accounts.lock")
    }

    /// `characters/`.
    #[must_use]
    pub fn characters_dir(&self) -> PathBuf {
        self.root.join("characters")
    }

    /// `characters/<canonical account>.toml`.
    #[must_use]
    pub fn character_path(&self, account: &str) -> PathBuf {
        self.characters_dir().join(format!("{account}.toml"))
    }

    /// `world-state.toml`.
    #[must_use]
    pub fn world_state_path(&self) -> PathBuf {
        self.root.join("world-state.toml")
    }

    /// Resolves a configured path: relative paths are taken against the data directory.
    #[must_use]
    pub fn resolve(&self, p: &Path) -> PathBuf {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.root.join(p)
        }
    }

    /// Applies the install guard to the data directory (status 4 when refused).
    pub fn check_guard(&self) -> Result<(), ServerError> {
        if data_dir_guard_hit(&self.root) {
            return Err(ServerError::InstallGuard {
                path: self.root.clone(),
            });
        }
        Ok(())
    }

    /// Creates the directory (mode 0700 on Unix) if it is missing, after the install guard.
    pub fn create(&self) -> Result<(), ServerError> {
        self.check_guard()?;
        create_private_dir(&self.root)?;
        Ok(())
    }

    /// Fails when the directory does not exist (commands that never create it).
    pub fn require_exists(&self) -> Result<(), ServerError> {
        self.check_guard()?;
        if !self.root.is_dir() {
            return Err(ServerError::Runtime(format!(
                "{}: the data directory does not exist",
                self.root.display()
            )));
        }
        Ok(())
    }

    /// Removes stale `.…tmp` files left by an interrupted atomic write (§3.7).
    pub fn remove_stale_tmp(&self) {
        for dir in [self.root.clone(), self.characters_dir()] {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for e in entries.filter_map(Result::ok).take(GUARD_MAX_ENTRIES) {
                let name = e.file_name();
                let Some(n) = name.to_str() else { continue };
                if n.starts_with('.')
                    && n.ends_with(".tmp")
                    && e.path().is_file()
                    && let Err(err) = fs::remove_file(e.path())
                {
                    tracing::warn!(file = %e.path().display(), error = %err, "cannot remove a stale temporary file");
                }
            }
        }
    }
}

/// Creates a directory and its parents; the directory itself gets mode 0700 on Unix.
pub(crate) fn create_private_dir(path: &Path) -> Result<(), ServerError> {
    if path.is_dir() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)
            .map_err(|e| ServerError::io(path, e))
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(path).map_err(|e| ServerError::io(path, e))
    }
}

fn open_lock_file(path: &Path) -> Result<File, ServerError> {
    let mut o = OpenOptions::new();
    o.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    o.open(path).map_err(|e| ServerError::io(path, e))
}

/// The exclusive server lock (`server.lock`, §3.7), held until dropped.
#[derive(Debug)]
pub struct ServerLock {
    file: File,
    path: PathBuf,
}

impl ServerLock {
    /// Takes the lock without waiting. Fails with [`ServerError::Locked`] (status 4) naming
    /// the PID of the holder when another process holds it. With `write_pid` (the `run`
    /// command) the file then holds this process's PID.
    pub fn acquire(dir: &DataDir, write_pid: bool) -> Result<Self, ServerError> {
        let path = dir.lock_path();
        let mut file = open_lock_file(&path)?;
        match file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => {
                let mut s = String::new();
                let _ = (&file).take(64).read_to_string(&mut s);
                return Err(ServerError::Locked {
                    pid: s.trim().parse().ok(),
                });
            }
            Err(std::fs::TryLockError::Error(e)) => return Err(ServerError::io(&path, e)),
        }
        if write_pid {
            let _ = file.set_len(0);
            let _ = file.write_all(format!("{}\n", std::process::id()).as_bytes());
            let _ = file.flush();
        }
        Ok(Self { file, path })
    }
}

impl Drop for ServerLock {
    fn drop(&mut self) {
        let _ = self.file.set_len(0);
        if let Err(e) = self.file.unlock() {
            tracing::debug!(file = %self.path.display(), error = %e, "unlock failed");
        }
    }
}

/// Runs `f` while holding the accounts lock (`accounts.lock`, §3.7), waiting for it.
pub fn with_accounts_lock<T>(
    dir: &DataDir,
    f: impl FnOnce() -> Result<T, ServerError>,
) -> Result<T, ServerError> {
    let path = dir.accounts_lock_path();
    let file = open_lock_file(&path)?;
    file.lock().map_err(|e| ServerError::io(&path, e))?;
    let out = f();
    let _ = file.unlock();
    out
}

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Writes `contents` to `path` atomically (§3.7): a new file in the same directory (name
/// starting with `.` and ending in `.tmp`) created with `mode` on Unix, flushed to disk,
/// renamed over the target, and on Unix the directory synced.
pub fn atomic_write(path: &Path, contents: &[u8], mode: u32) -> std::io::Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp = dir.join(format!(
        ".{name}.{}.{}.tmp",
        std::process::id(),
        TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let mut o = OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(mode);
    }
    #[cfg(not(unix))]
    let _ = mode;
    let result = (|| {
        let mut f = o.open(&tmp)?;
        f.write_all(contents)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)?;
        #[cfg(unix)]
        if let Ok(d) = File::open(dir) {
            let _ = d.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Checks the key file's type, owner and mode (§3.3, Unix): a regular file owned by the
/// effective user with no group or other permission bits.
fn check_key_permissions(path: &Path) -> Result<(), ServerError> {
    let meta = fs::symlink_metadata(path).map_err(|e| ServerError::io(path, e))?;
    if !meta.file_type().is_file() {
        return Err(ServerError::KeyPermissions {
            path: path.to_path_buf(),
            message: "the server key must be a regular file (not a link or directory)".into(),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let euid = rustix::process::geteuid().as_raw();
        if meta.uid() != euid {
            return Err(ServerError::KeyPermissions {
                path: path.to_path_buf(),
                message: format!(
                    "the server key is owned by user {} but the server runs as user {euid}; fix the owner (chown) and run chmod 600",
                    meta.uid()
                ),
            });
        }
        if meta.mode() & 0o077 != 0 {
            return Err(ServerError::KeyPermissions {
                path: path.to_path_buf(),
                message: format!(
                    "the server key is readable by others (mode {:o}); fix it with: chmod 600 {}",
                    meta.mode() & 0o777,
                    path.display()
                ),
            });
        }
    }
    Ok(())
}

/// Loads the server key (§3.3). Fails when it is missing, unreadable, unsafe or invalid.
/// The server never replaces an existing key on its own.
pub fn load_key(dir: &DataDir) -> Result<ServerIdentity, ServerError> {
    let path = dir.key_path();
    check_key_permissions(&path)?;
    let mut text = String::new();
    File::open(&path)
        .and_then(|f| f.take(64 * 1024).read_to_string(&mut text))
        .map_err(|e| ServerError::io(&path, e))?;
    ServerIdentity::from_pkcs8_pem(&text).map_err(|_| ServerError::InvalidFile {
        path: path.clone(),
        message: "not an Ed25519 PKCS#8 private key (PEM, label PRIVATE KEY)".into(),
    })
}

/// Generates a key and writes it as a new file created exclusively with mode 0600, then
/// renamed into place (§3.3). Fails if a key appeared meanwhile.
fn write_new_key(dir: &DataDir) -> Result<ServerIdentity, ServerError> {
    let identity = ServerIdentity::generate()?;
    let path = dir.key_path();
    let pem = identity.to_pkcs8_pem();
    let tmp = dir
        .path()
        .join(format!(".server.key.{}.tmp", std::process::id()));
    let mut o = OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let r = (|| {
        let mut f = o.open(&tmp)?;
        f.write_all(pem.as_bytes())?;
        f.sync_all()?;
        drop(f);
        if path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "a key appeared meanwhile",
            ));
        }
        fs::rename(&tmp, &path)
    })();
    if let Err(e) = r {
        let _ = fs::remove_file(&tmp);
        return Err(ServerError::io(&path, e));
    }
    Ok(identity)
}

/// Loads the server key, generating it on first start (§2.2 step 5). Returns the identity
/// and whether it was generated now.
pub fn load_or_generate_key(dir: &DataDir) -> Result<(ServerIdentity, bool), ServerError> {
    if fs::symlink_metadata(dir.key_path()).is_ok() {
        return Ok((load_key(dir)?, false));
    }
    Ok((write_new_key(dir)?, true))
}

/// `keygen` (§2.3): creates the key if none exists. With an existing key it refuses unless
/// `force`; with `force` the old file is renamed to `server.key.old-<UTC time>` and its
/// fingerprint returned with the new identity.
pub fn keygen(
    dir: &DataDir,
    force: bool,
) -> Result<
    (
        ServerIdentity,
        Option<tessera_protocol::fingerprint::Fingerprint>,
    ),
    ServerError,
> {
    let path = dir.key_path();
    if fs::symlink_metadata(&path).is_err() {
        return Ok((write_new_key(dir)?, None));
    }
    if !force {
        return Err(ServerError::Usage(format!(
            "{} exists; use --force to replace it (every client that pinned it will fail with KEY_MISMATCH)",
            path.display()
        )));
    }
    let old = load_key(dir).ok().map(|k| k.fingerprint());
    let stamp = utc_compact(SystemTime::now());
    // Never replace an earlier backup (two forced keygens in one second): the first free
    // name of `server.key.old-<time>`, `-<time>.1`, `-<time>.2`, ... is taken by a hard
    // link, which fails if the name exists, and only then is the key itself removed.
    let mut linked = None;
    for n in 0..1_000u32 {
        let name = if n == 0 {
            format!("server.key.old-{stamp}")
        } else {
            format!("server.key.old-{stamp}.{n}")
        };
        let backup = dir.path().join(name);
        match fs::hard_link(&path, &backup) {
            Ok(()) => {
                linked = Some(backup);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(ServerError::io(&backup, e)),
        }
    }
    if linked.is_none() {
        return Err(ServerError::Usage(format!(
            "no free backup name for {}; remove old server.key.old-* files",
            path.display()
        )));
    }
    fs::remove_file(&path).map_err(|e| ServerError::io(&path, e))?;
    Ok((write_new_key(dir)?, old))
}

/// Days since 1970-01-01 to a proleptic Gregorian (year, month, day).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn utc_parts(t: SystemTime) -> (i64, u32, u32, u64, u64, u64) {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    (y, m, d, rem / 3_600, rem / 60 % 60, rem % 60)
}

/// An RFC 3339 UTC date-time with whole seconds, `YYYY-MM-DDTHH:MM:SSZ`.
#[must_use]
pub fn utc_rfc3339(t: SystemTime) -> String {
    let (y, m, d, hh, mm, ss) = utc_parts(t);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// A compact UTC time, `YYYYMMDDTHHMMSSZ` (§2.3).
#[must_use]
pub fn utc_compact(t: SystemTime) -> String {
    let (y, m, d, hh, mm, ss) = utc_parts(t);
    format!("{y:04}{m:02}{d:02}T{hh:02}{mm:02}{ss:02}Z")
}

/// Whether a path has no `..` components (used for file names built from account names).
#[must_use]
pub fn is_plain_relative(p: &Path) -> bool {
    p.components().all(|c| matches!(c, Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_367), (2025, 10, 6));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        let t = UNIX_EPOCH + std::time::Duration::from_secs(1_759_752_000 + 3_723);
        assert_eq!(utc_rfc3339(t), "2025-10-06T13:02:03Z");
        assert_eq!(utc_compact(t), "20251006T130203Z");
    }

    #[test]
    fn archive_names() {
        assert!(is_game_archive_name("client_cell_1.dat"));
        assert!(is_game_archive_name("CLIENT_GENERAL.DAT"));
        assert!(is_game_archive_name("x.datx"));
        assert!(!is_game_archive_name("client_cell_1.dat.bak"));
        assert!(!is_game_archive_name("other.dat"));
    }
}
