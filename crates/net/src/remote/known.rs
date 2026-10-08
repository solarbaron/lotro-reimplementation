// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's known-servers file (`tessera-protocol.md` §4.5): reading it, and appending
//! a key accepted on first use.
//!
//! The file belongs in Tessera's own settings directory (named
//! [`KNOWN_SERVERS_FILE`](tessera_protocol::connect::KNOWN_SERVERS_FILE)). As a safety net
//! against a misconfigured path, it is never written into a directory that holds archive
//! files (`*.dat`, `*.datx`), nor anywhere below a directory that holds a game install's
//! archives (`client_*.dat`, `client_*.datx`).

use std::io::{Read, Write};
use std::path::Path;

use tessera_protocol::connect::{KnownServers, KnownServersWarning, ServerKey, format_entry};
use tessera_protocol::fingerprint::Fingerprint;

use crate::NetError;

/// Largest known-servers file accepted. A longer file is refused as a whole (see
/// [`load_known_servers`]): an entry past the cut would otherwise be missed silently.
pub const MAX_KNOWN_SERVERS_BYTES: u64 = 1 << 20;

/// Reads the known-servers file at `path`. A missing file is empty; invalid lines and
/// repeated entries are logged and skipped (§4.5).
///
/// A file that exists but cannot be read completely is an error, never an empty set: a
/// found entry is authoritative (§4.5), so a client that could not read the file must not
/// fall back to trust on first use for a server the file may name.
///
/// # Errors
///
/// [`NetError::Io`] when the file exists but cannot be opened or read (permissions, I/O
/// error, a directory); [`NetError::InvalidConfig`] when it is larger than
/// [`MAX_KNOWN_SERVERS_BYTES`].
pub fn load_known_servers(path: &Path) -> Result<KnownServers, NetError> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(KnownServers::new()),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    // One byte more than the limit tells a file of exactly the limit from a longer one.
    file.take(MAX_KNOWN_SERVERS_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_KNOWN_SERVERS_BYTES {
        return Err(NetError::InvalidConfig(
            "known-servers file is larger than 1 MiB",
        ));
    }
    let text = String::from_utf8_lossy(&bytes);
    let (known, warnings) = KnownServers::parse(&text);
    for w in warnings {
        match w {
            KnownServersWarning::InvalidLine { line } => {
                tracing::warn!(path = %path.display(), line, "invalid known-servers line skipped");
            }
            KnownServersWarning::Duplicate { line, key } => {
                tracing::warn!(path = %path.display(), line, %key, "repeated known-servers entry ignored");
            }
        }
    }
    Ok(known)
}

/// Whether a directory listing holds archive files. `strict` matches only the archive
/// names of a game install (`client_*.dat`, `client_*.datx`); otherwise any `*.dat` or
/// `*.datx` counts.
fn holds_archives(dir: &Path, strict: bool) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().take(4096).any(|e| {
        let name = e.file_name();
        let name = name.to_string_lossy().to_ascii_lowercase();
        (name.ends_with(".dat") || name.ends_with(".datx"))
            && (!strict || name.starts_with("client_"))
    })
}

/// Whether writing `path` (creating missing directories) would put something inside a game
/// install. The file's own directory is refused when it holds any `*.dat`/`*.datx` file;
/// every existing ancestor above the file is refused when it holds a game install's
/// archives (`client_*.dat`/`client_*.datx`), so a subdirectory of an install is refused
/// too (checked before any directory is created), while a home directory that merely holds
/// some unrelated `.dat` file is not.
fn inside_install(path: &Path) -> bool {
    for (depth, dir) in path.ancestors().skip(1).take(256).enumerate() {
        let dir = if dir.as_os_str().is_empty() {
            Path::new(".")
        } else {
            dir
        };
        if dir.is_dir() && holds_archives(dir, depth > 0) {
            return true;
        }
    }
    false
}

/// Records that `fingerprint` is trusted for `server` by appending one line to the
/// known-servers file at `path` (creating it and its directory). Does nothing when the
/// file already has an entry for `server` (the first entry wins, §4.5).
///
/// # Errors
///
/// [`NetError::InvalidConfig`] when the path lies in or below a directory that looks like
/// a game install, or the existing file is too large; [`NetError::Io`] when the existing
/// file cannot be read or the file cannot be written. Nothing is appended to a file that
/// could not be read completely.
pub fn record_known_server(
    path: &Path,
    server: &ServerKey,
    fingerprint: &Fingerprint,
) -> Result<(), NetError> {
    if inside_install(path) {
        return Err(NetError::InvalidConfig(
            "known-servers file must not live in or below a directory with game archives",
        ));
    }
    if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    if load_known_servers(path)?.lookup(server).is_some() {
        return Ok(());
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    // Start on a fresh line if the file does not end with one.
    let len = file.metadata()?.len();
    let mut line = String::new();
    if len > 0 {
        let mut last = [0u8; 1];
        if let Ok(mut f) = std::fs::File::open(path) {
            use std::io::{Seek, SeekFrom};
            if f.seek(SeekFrom::End(-1)).is_ok()
                && f.read_exact(&mut last).is_ok()
                && last[0] != b'\n'
            {
                line.push('\n');
            }
        }
    }
    line.push_str(&format_entry(server, fingerprint));
    file.write_all(line.as_bytes())?;
    file.sync_all()?;
    tracing::info!(path = %path.display(), %server, %fingerprint, "server key recorded");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FP: &str = "sha256:06e3fd8fda29bb60ab59557de61edb0aecdb231134be30e75b455f8e1b792fa9";

    fn dir(name: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("tessera-net-known-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn record_and_load() {
        let d = dir("rl");
        let path = d.join("sub").join("known_servers");
        let key = ServerKey::parse("192.0.2.10:47000").unwrap();
        let fp = Fingerprint::parse(FP).unwrap();
        assert!(load_known_servers(&path).unwrap().lookup(&key).is_none());
        record_known_server(&path, &key, &fp).unwrap();
        record_known_server(&path, &key, &fp).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 1, "{text}");
        assert_eq!(load_known_servers(&path).unwrap().lookup(&key), Some(&fp));
        // A file without a final newline gets one before the new entry.
        std::fs::write(&path, "# comment").unwrap();
        record_known_server(&path, &key, &fp).unwrap();
        assert_eq!(load_known_servers(&path).unwrap().lookup(&key), Some(&fp));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn refuses_install_dirs() {
        let d = dir("inst");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("client_general.dat"), b"x").unwrap();
        let key = ServerKey::parse("192.0.2.10:47000").unwrap();
        let fp = Fingerprint::parse(FP).unwrap();
        assert!(record_known_server(&d.join("known_servers"), &key, &fp).is_err());
        // Nothing is created below an install either, however deep (finding: only the
        // immediate parent was checked, so `<install>/tessera/known_servers` was written).
        let deep = d.join("tessera").join("more").join("known_servers");
        assert!(record_known_server(&deep, &key, &fp).is_err());
        assert!(
            !d.join("tessera").exists(),
            "directory created inside the install"
        );
        std::fs::create_dir_all(d.join("existing")).unwrap();
        assert!(record_known_server(&d.join("existing").join("known_servers"), &key, &fp).is_err());
        assert!(!d.join("existing").join("known_servers").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn unrelated_dat_files_above_are_fine() {
        // A `.dat` that is not an install archive above the file's directory does not
        // block the file.
        let d = dir("home");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("notes.dat"), b"x").unwrap();
        let key = ServerKey::parse("192.0.2.10:47000").unwrap();
        let fp = Fingerprint::parse(FP).unwrap();
        let path = d.join("settings").join("known_servers");
        record_known_server(&path, &key, &fp).unwrap();
        assert_eq!(load_known_servers(&path).unwrap().lookup(&key), Some(&fp));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn incomplete_reads_are_errors() {
        // Finding: an oversized or unreadable file used to load as an empty or truncated
        // set, so trust on first use accepted a key the file contradicts, and recording
        // appended a duplicate entry.
        let d = dir("big");
        std::fs::create_dir_all(&d).unwrap();
        let key = ServerKey::parse("192.0.2.10:47000").unwrap();
        let fp = Fingerprint::parse(FP).unwrap();
        let path = d.join("known_servers");
        let mut text = "#".repeat(usize::try_from(MAX_KNOWN_SERVERS_BYTES).unwrap());
        text.push('\n');
        text.push_str(&format_entry(&key, &fp));
        std::fs::write(&path, &text).unwrap();
        assert!(matches!(
            load_known_servers(&path),
            Err(NetError::InvalidConfig(_))
        ));
        assert!(record_known_server(&path, &key, &fp).is_err());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            text,
            "file changed"
        );
        // A file of exactly the limit is read.
        let exact = "#".repeat(usize::try_from(MAX_KNOWN_SERVERS_BYTES).unwrap());
        std::fs::write(&path, &exact).unwrap();
        assert!(load_known_servers(&path).unwrap().lookup(&key).is_none());
        // A directory where the file should be is an I/O error, not an empty set.
        let as_dir = d.join("is_a_dir");
        std::fs::create_dir_all(&as_dir).unwrap();
        assert!(matches!(load_known_servers(&as_dir), Err(NetError::Io(_))));
        // Missing is empty.
        assert!(
            load_known_servers(&d.join("missing"))
                .unwrap()
                .lookup(&key)
                .is_none()
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
