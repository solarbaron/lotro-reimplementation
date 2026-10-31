// SPDX-License-Identifier: GPL-3.0-or-later
//! The install version of a game directory and the work-tree refusal (decision 0031 tier 2,
//! reused by decision 0045).

use std::path::{Path, PathBuf};

use crate::CensusError;

/// The short install version of `game_dir`: the short level-1 content fingerprint
/// (`install-layout.md` §9.4) when an archive carries an iteration record or an install
/// stamp, else `schema-` and the short level-0 fingerprint.
///
/// A record that is absent is an input of the fingerprint; a reader that fails is not. An
/// archive whose header check fails, that cannot be opened, or whose 0xFFFF0001, 0xFFFF0005
/// or 0xFFFF0003 lookup, read or parse fails is an error, so a broken reader fails the census
/// instead of choosing a new baseline key (decision 0045).
///
/// # Errors
/// [`CensusError::NoVersion`] when the install cannot be opened,
/// [`CensusError::VersionInput`] when an archive or one of its version records cannot be
/// read.
pub fn install_version(game_dir: &Path) -> Result<String, CensusError> {
    version_with_inputs(game_dir).map(|(v, _)| v)
}

/// [`install_version`] and one line per archive naming what went into the fingerprint
/// (file name and which version records it carries; names and numbers only).
pub(crate) fn version_with_inputs(game_dir: &Path) -> Result<(String, Vec<String>), CensusError> {
    let inst = tessera_install::GameInstall::open(game_dir)
        .map_err(|_| CensusError::NoVersion(game_dir.into()))?;
    let mut entries = Vec::with_capacity(inst.archives.len());
    for a in &inst.archives {
        if !a.status.is_ok() {
            return Err(CensusError::VersionInput {
                archive: a.path.clone(),
                reason: format!("archive header check failed: {:?}", a.status),
            });
        }
        entries.push(content_entry(&a.path)?);
    }
    let inputs = entries
        .iter()
        .map(|e| {
            format!(
                "{}: iteration {}, membership {}, install stamp {}",
                e.file_name,
                e.iteration
                    .as_ref()
                    .map_or_else(|| "absent".to_owned(), |i| format!("N = {}", i.count)),
                if e.membership.is_some() {
                    "present"
                } else {
                    "absent"
                },
                if e.install_stamp.is_some() {
                    "present"
                } else {
                    "absent"
                },
            )
        })
        .collect();
    let content = entries
        .iter()
        .any(|e| e.iteration.is_some() || e.install_stamp.is_some());
    let version = if content {
        tessera_install::ContentFingerprint::compute(&entries)
            .short()
            .to_owned()
    } else {
        format!("schema-{}", inst.schema_fingerprint().short())
    };
    Ok((version, inputs))
}

/// The record `did` of `a`: `Ok(None)` only when the directory has no entry for it; a failed
/// lookup, read or parse is an error.
fn record<T, E: std::fmt::Display>(
    a: &tessera_dat::Archive,
    path: &Path,
    did: u32,
    parse: impl Fn(&[u8]) -> Result<T, E>,
) -> Result<Option<T>, CensusError> {
    let fail = |what: &str, e: &dyn std::fmt::Display| CensusError::VersionInput {
        archive: path.to_path_buf(),
        reason: format!("{what} of record {did:#010X} failed: {e}"),
    };
    let Some(entry) = a.lookup(did).map_err(|e| fail("lookup", &e))? else {
        return Ok(None);
    };
    let bytes = a.read_entry(&entry).map_err(|e| fail("read", &e))?;
    parse(&bytes).map(Some).map_err(|e| fail("parse", &e))
}

pub(crate) fn content_entry(path: &Path) -> Result<tessera_install::ContentEntry, CensusError> {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let a = tessera_dat::Archive::open(path).map_err(|e| CensusError::VersionInput {
        archive: path.to_path_buf(),
        reason: format!("archive does not open: {e}"),
    })?;
    Ok(tessera_install::ContentEntry {
        iteration: record(
            &a,
            path,
            tessera_install::DID_ITERATION,
            tessera_install::IterationRecord::parse,
        )?,
        membership: record(
            &a,
            path,
            tessera_install::DID_SET_MEMBERSHIP,
            tessera_install::SetMembership::parse,
        )?,
        install_stamp: record(
            &a,
            path,
            tessera_install::DID_INSTALL_STAMP,
            tessera_install::InstallStamp::parse,
        )?,
        file_name,
    })
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

/// The canonical path of the nearest existing ancestor of `dir` (`dir` itself when it
/// exists).
fn nearest_existing(dir: &Path) -> Result<PathBuf, CensusError> {
    let mut probe = dir.to_path_buf();
    while !probe.exists() {
        if !probe.pop() {
            break;
        }
    }
    if probe.as_os_str().is_empty() {
        probe = PathBuf::from(".");
    }
    std::fs::canonicalize(&probe).map_err(|source| CensusError::Io {
        path: probe.clone(),
        source,
    })
}

/// Refuses `dir` when the nearest existing ancestor of it (canonicalised) or any ancestor
/// of that holds a `.git` entry.
///
/// # Errors
/// [`CensusError::InsideWorkTree`] when refused, [`CensusError::Io`] when the existing
/// ancestor cannot be canonicalised.
pub fn check_outside_work_trees(dir: &Path) -> Result<(), CensusError> {
    let real = nearest_existing(dir)?;
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

/// Refuses `dir` when it lies at or under the game install `game_dir` (both canonicalised;
/// for `dir`, its nearest existing ancestor): real-data tests never write to the install.
///
/// # Errors
/// [`CensusError::InsideInstall`] when refused, [`CensusError::Io`] when a path cannot be
/// canonicalised.
pub fn check_outside_install(dir: &Path, game_dir: &Path) -> Result<(), CensusError> {
    let real = nearest_existing(dir)?;
    let install = std::fs::canonicalize(game_dir).map_err(|source| CensusError::Io {
        path: game_dir.to_path_buf(),
        source,
    })?;
    if real.starts_with(&install) {
        return Err(CensusError::InsideInstall {
            dir: dir.to_path_buf(),
            install,
        });
    }
    Ok(())
}
