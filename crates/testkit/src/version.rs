// SPDX-License-Identifier: GPL-3.0-or-later
//! The install version of a game directory and the work-tree refusal (decision 0031 tier 2,
//! reused by decision 0045).

use std::path::{Path, PathBuf};

use crate::CensusError;

/// The short install version of `game_dir`: the short level-1 content fingerprint
/// (`install-layout.md` §9.4) when an archive carries an iteration record or an install
/// stamp, else `schema-` and the short level-0 fingerprint. `None` when the install cannot
/// be opened.
#[must_use]
pub fn install_version(game_dir: &Path) -> Option<String> {
    let inst = tessera_install::GameInstall::open(game_dir).ok()?;
    let entries: Vec<tessera_install::ContentEntry> = inst
        .archives
        .iter()
        .filter(|a| a.status.is_ok())
        .map(|a| content_entry(&a.path))
        .collect();
    let content = entries
        .iter()
        .any(|e| e.iteration.is_some() || e.install_stamp.is_some());
    Some(if content {
        tessera_install::ContentFingerprint::compute(&entries)
            .short()
            .to_owned()
    } else {
        format!("schema-{}", inst.schema_fingerprint().short())
    })
}

fn record<T>(
    a: &tessera_dat::Archive,
    did: u32,
    parse: impl Fn(&[u8]) -> Result<T, tessera_install::RecordError>,
) -> Option<T> {
    let entry = a.lookup(did).ok()??;
    let bytes = a.read_entry(&entry).ok()?;
    parse(&bytes).ok()
}

fn content_entry(path: &Path) -> tessera_install::ContentEntry {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match tessera_dat::Archive::open(path) {
        Ok(a) => tessera_install::ContentEntry {
            iteration: record(
                &a,
                tessera_install::DID_ITERATION,
                tessera_install::IterationRecord::parse,
            ),
            membership: record(
                &a,
                tessera_install::DID_SET_MEMBERSHIP,
                tessera_install::SetMembership::parse,
            ),
            install_stamp: record(
                &a,
                tessera_install::DID_INSTALL_STAMP,
                tessera_install::InstallStamp::parse,
            ),
            file_name,
        },
        Err(_) => tessera_install::ContentEntry {
            file_name,
            iteration: None,
            membership: None,
            install_stamp: None,
        },
    }
}

/// Whether `s` is named like an install version: 16 lowercase hex digits, optionally after
/// `schema-`.
#[must_use]
pub fn is_version_name(s: &str) -> bool {
    let hex = s.strip_prefix("schema-").unwrap_or(s);
    hex.len() == tessera_install::SHORT_LEN
        && hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Refuses `dir` when the nearest existing ancestor of it (canonicalised) or any ancestor
/// of that holds a `.git` entry.
///
/// # Errors
/// [`CensusError::InsideWorkTree`] when refused, [`CensusError::Io`] when the existing
/// ancestor cannot be canonicalised.
pub fn check_outside_work_trees(dir: &Path) -> Result<(), CensusError> {
    let mut probe = dir.to_path_buf();
    while !probe.exists() {
        if !probe.pop() {
            break;
        }
    }
    if probe.as_os_str().is_empty() {
        probe = PathBuf::from(".");
    }
    let real = std::fs::canonicalize(&probe).map_err(|source| CensusError::Io {
        path: probe.clone(),
        source,
    })?;
    for a in real.ancestors() {
        if a.join(".git").exists() {
            return Err(CensusError::InsideWorkTree {
                dir: dir.to_path_buf(),
                repo: a.to_path_buf(),
            });
        }
    }
    Ok(())
}
