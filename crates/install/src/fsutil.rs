// SPDX-License-Identifier: GPL-3.0-or-later
//! File-system helpers: case-insensitive path resolution, Windows-path
//! translation into Wine prefixes, bounded directory walks and bounded reads.
//!
//! Spec-Ref: docs/specs/install-layout.md#2-terms
//! Spec-Ref: docs/specs/install-layout.md#52-paths-inside-a-prefix
//! Spec-Ref: docs/specs/install-layout.md#58-search-limits-and-de-duplication
//!
//! Everything here only reads. Nothing creates, renames, locks or writes.

use std::collections::VecDeque;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// One directory entry: its name (lossy UTF-8) and full path.
#[derive(Debug, Clone)]
pub(crate) struct DirEntry {
    pub name: String,
    pub path: PathBuf,
}

/// List a directory. Entries whose names are not valid UTF-8 are kept with a
/// lossy name; they can never match the ASCII names this crate looks for.
pub(crate) fn list_dir(dir: &Path) -> std::io::Result<Vec<DirEntry>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        out.push(DirEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: entry.path(),
        });
    }
    Ok(out)
}

/// Pick the case-insensitive match for `name` from a directory listing: the
/// exact-case entry if present, otherwise the lexicographically smallest match
/// (with a warning when there is more than one).
pub(crate) fn pick_ci<'a>(entries: &'a [DirEntry], name: &str) -> Option<&'a DirEntry> {
    let mut matches: Vec<&DirEntry> = entries
        .iter()
        .filter(|e| e.name.eq_ignore_ascii_case(name))
        .collect();
    if let Some(exact) = matches.iter().find(|e| e.name == name) {
        return Some(exact);
    }
    matches.sort_by(|a, b| a.name.cmp(&b.name));
    if matches.len() > 1 {
        tracing::warn!(
            name,
            chosen = %matches[0].path.display(),
            count = matches.len(),
            "several entries match case-insensitively; using the smallest"
        );
    }
    matches.first().copied()
}

/// Resolve `name` inside `parent` case-insensitively, returning the entry's
/// on-disk spelling (also on case-insensitive file systems such as NTFS and
/// APFS, where the requested spelling would otherwise be accepted as is).
/// When `parent` cannot be listed, an exact-case entry is still found.
/// Returns `None` when no entry matches.
pub(crate) fn resolve_ci(parent: &Path, name: &str) -> Option<PathBuf> {
    if let Ok(entries) = list_dir(parent)
        && let Some(e) = pick_ci(&entries, name)
    {
        return Some(e.path.clone());
    }
    let exact = parent.join(name);
    fs::symlink_metadata(&exact).is_ok().then_some(exact)
}

/// Resolve several components below `base`, each case-insensitively.
pub(crate) fn resolve_path_ci(base: &Path, components: &[&str]) -> Option<PathBuf> {
    let mut p = base.to_path_buf();
    for c in components {
        p = resolve_ci(&p, c)?;
    }
    Some(p)
}

/// Is `path` a directory (following symlinks)?
pub(crate) fn is_dir(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
}

/// Is `path` a regular file (following symlinks)?
pub(crate) fn is_file(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.is_file()).unwrap_or(false)
}

/// Canonical form of a path for de-duplication; falls back to the path itself.
pub(crate) fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Open `path` read-only and make sure it is a regular file (after following
/// symlinks).
///
/// On Unix the file is opened with `O_NONBLOCK`, so a FIFO or a device node
/// planted where a metadata file is expected cannot block the open; the file
/// type is then checked on the open handle (no check-then-open race). Reads
/// from a regular file are unaffected by `O_NONBLOCK`.
pub(crate) fn open_regular(path: &Path) -> std::io::Result<fs::File> {
    let mut opts = fs::OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NONBLOCK);
    }
    let file = opts.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "not a regular file",
        ));
    }
    Ok(file)
}

/// Read a whole small file, refusing files larger than `limit` bytes and
/// anything that is not a regular file.
pub(crate) fn read_capped(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = open_regular(path).map_err(|e| Error::io(path, e))?;
    let mut buf = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut buf)
        .map_err(|e| Error::io(path, e))?;
    if buf.len() as u64 > limit {
        return Err(Error::TooLarge {
            path: path.to_path_buf(),
            limit,
        });
    }
    Ok(buf)
}

/// Read at most `limit` bytes from the start of a regular file.
pub(crate) fn read_prefix(path: &Path, limit: usize) -> std::io::Result<Vec<u8>> {
    let file = open_regular(path)?;
    let mut buf = Vec::with_capacity(limit);
    file.take(limit as u64).read_to_end(&mut buf)?;
    Ok(buf)
}

/// Translate a Windows path (as found in a registry value, a launcher setting
/// or a `--prefs` argument) into a host path inside `prefix`.
///
/// `C:` maps to `<prefix>/drive_c`, `Z:` to the host root, other drive letters
/// go through `<prefix>/dosdevices/<letter>:` when that exists. Each component
/// is resolved case-insensitively when it exists. Returns `None` for paths that
/// cannot be translated (no drive letter, unknown drive, `..` components).
pub fn translate_windows_path(prefix: &Path, win: &str) -> Option<PathBuf> {
    let win = win.trim();
    let bytes = win.as_bytes();
    if bytes.len() < 2 || bytes[1] != b':' || !bytes[0].is_ascii_alphabetic() {
        tracing::debug!(path = win, "not a drive-letter Windows path");
        return None;
    }
    let letter = bytes[0].to_ascii_lowercase();
    let rest = &win[2..];
    let mut base = match letter {
        b'c' => prefix.join("drive_c"),
        b'z' => PathBuf::from("/"),
        other => {
            let link = dosdevices_link(prefix, char::from(other));
            if fs::metadata(&link).is_ok() {
                link
            } else {
                tracing::debug!(path = win, "unresolvable drive letter");
                return None;
            }
        }
    };
    for comp in rest.split(['\\', '/']) {
        match comp {
            "" | "." => continue,
            ".." => {
                tracing::debug!(path = win, "refusing '..' in a Windows path");
                return None;
            }
            c => {
                base = resolve_ci(&base, c).unwrap_or_else(|| base.join(c));
            }
        }
    }
    Some(base)
}

/// `<prefix>/dosdevices/<letter>:` as a plain path component. `Path::join("d:")` would
/// not do: on Windows a `d:` component is a drive prefix and replaces the whole path (the
/// link would name the host's own drive D:).
fn dosdevices_link(prefix: &Path, letter: char) -> PathBuf {
    let mut link = prefix.join("dosdevices").into_os_string();
    link.push(std::path::MAIN_SEPARATOR_STR);
    link.push(format!("{letter}:"));
    PathBuf::from(link)
}

/// Limits for a bounded walk.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WalkLimits {
    /// Maximum depth below the root (the root is depth 0).
    pub max_depth: usize,
    /// Maximum directories visited in this walk.
    pub max_dirs: usize,
}

/// Default directory cap per walk (§5.8).
pub(crate) const MAX_DIRS_PER_WALK: usize = 20_000;

/// Does this directory listing contain `client_general.dat`?
pub(crate) fn has_general_archive(entries: &[DirEntry]) -> bool {
    entries
        .iter()
        .any(|e| e.name.eq_ignore_ascii_case("client_general.dat") && is_file(&e.path))
}

/// Walk below `root` breadth-first looking for directories that directly hold
/// `client_general.dat`. Matches are not descended into. Directory symlinks
/// are not followed below the root; dot-directories, `dosdevices`, `backup`
/// and `compatdata` are skipped.
pub(crate) fn find_install_dirs(root: &Path, limits: WalkLimits) -> Vec<PathBuf> {
    let mut found = Vec::new();
    if !is_dir(root) {
        return found;
    }
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((root.to_path_buf(), 0));
    let mut visited = 0usize;
    while let Some((dir, depth)) = queue.pop_front() {
        if visited >= limits.max_dirs {
            tracing::info!(
                root = %root.display(),
                cap = limits.max_dirs,
                "directory cap reached; search truncated"
            );
            break;
        }
        visited += 1;
        let entries = match list_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                tracing::debug!(dir = %dir.display(), error = %e, "cannot list directory");
                continue;
            }
        };
        if has_general_archive(&entries) {
            found.push(dir);
            continue;
        }
        if depth >= limits.max_depth {
            continue;
        }
        for e in entries {
            if skip_dir_name(&e.name) {
                continue;
            }
            // symlink_metadata: do not follow directory symlinks while recursing.
            let Ok(meta) = fs::symlink_metadata(&e.path) else {
                continue;
            };
            if meta.is_dir() {
                queue.push_back((e.path, depth + 1));
            }
        }
    }
    found
}

fn skip_dir_name(name: &str) -> bool {
    name.starts_with('.')
        || name.eq_ignore_ascii_case("dosdevices")
        || name.eq_ignore_ascii_case("backup")
        || name.eq_ignore_ascii_case("compatdata")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn resolves_case_insensitively() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(t.path().join("Drive_C/PROGRAM FILES")).unwrap();
        let p = resolve_path_ci(t.path(), &["drive_c", "Program Files"]).unwrap();
        assert_eq!(p, t.path().join("Drive_C/PROGRAM FILES"));
        assert!(resolve_ci(t.path(), "nope").is_none());
    }

    #[test]
    fn translates_windows_paths() {
        let t = tempfile::tempdir().unwrap();
        let pfx = t.path();
        fs::create_dir_all(pfx.join("drive_c/Users/steamuser/Documents")).unwrap();
        let p = translate_windows_path(pfx, r"c:\users\STEAMUSER\documents\x.ini").unwrap();
        assert_eq!(p, pfx.join("drive_c/Users/steamuser/Documents/x.ini"));
        assert_eq!(
            translate_windows_path(pfx, r"Z:\tmp\foo").unwrap(),
            PathBuf::from("/tmp/foo")
        );
        assert!(translate_windows_path(pfx, r"D:\games").is_none());
        assert!(translate_windows_path(pfx, r"C:\..\etc").is_none());
        assert!(translate_windows_path(pfx, "relative").is_none());
        assert!(translate_windows_path(pfx, "").is_none());
    }

    #[test]
    fn walk_respects_limits_and_skips() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        for d in [
            "a/b/game",
            "backup/game",
            ".hidden/game",
            "x/y/z/w/v/u/game",
        ] {
            fs::create_dir_all(root.join(d)).unwrap();
            fs::write(root.join(d).join("CLIENT_GENERAL.DAT"), b"x").unwrap();
        }
        let found = find_install_dirs(
            root,
            WalkLimits {
                max_depth: 5,
                max_dirs: MAX_DIRS_PER_WALK,
            },
        );
        assert_eq!(found, vec![root.join("a/b/game")]);
        let none = find_install_dirs(
            root,
            WalkLimits {
                max_depth: 5,
                max_dirs: 1,
            },
        );
        assert!(none.is_empty());
    }

    #[test]
    fn read_capped_enforces_limit() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("f");
        fs::write(&f, [0u8; 10]).unwrap();
        assert_eq!(read_capped(&f, 10).unwrap().len(), 10);
        assert!(matches!(read_capped(&f, 9), Err(Error::TooLarge { .. })));
    }

    /// Create a FIFO with the `mkfifo` utility (no unsafe code in this crate).
    /// Returns false, with a note, when the utility is unavailable.
    #[cfg(unix)]
    pub(crate) fn make_fifo(path: &Path) -> bool {
        let ok = std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            eprintln!("mkfifo unavailable; skipping FIFO test");
        }
        ok
    }

    /// A FIFO where a metadata file is expected must not block the reader.
    #[cfg(unix)]
    #[test]
    fn fifo_is_refused_without_blocking() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("fifo");
        if !make_fifo(&f) {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let f2 = f.clone();
        std::thread::spawn(move || {
            let a = read_prefix(&f2, 64).is_err();
            let b = read_capped(&f2, 64).is_err();
            let _ = tx.send((a, b));
        });
        let (a, b) = rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("reading a FIFO blocked");
        assert!(a && b);
        // Directories are refused as well.
        assert!(read_prefix(t.path(), 64).is_err());
    }
}
