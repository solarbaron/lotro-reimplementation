// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only importer of the original client's key map (`keymap.md` §12).
//!
//! 1. [`load_install_data`] reads the default key maps (prefix 0x1D), the
//!    default action map (0x51) and the command and input-map names from
//!    the install through a caller-supplied `fetch` closure (§12.2).
//! 2. [`import_key_map`] parses the user's key-mapping file (§12.3–§12.7),
//!    builds the effective key map the way the original does at start-up
//!    (§12.8) and translates the part with Tessera counterparts into a
//!    Tessera [`Keymap`] document plus a report (§12.14).
//!
//! The importer never writes: not the original's file, not the settings
//! folder, not the install. Its result becomes Tessera's keymap only through
//! an explicit save ([`Keymap::save_file`]).

use std::fs;
use std::io::Read as _;
use std::path::Path;

pub mod model;
pub mod names;
mod read;
pub mod record;
mod source;
pub mod text;
mod translate;
mod write;

#[cfg(test)]
mod tests;

pub use model::{
    Control, Device, Guid, KeyBinding, KeyMap, MetaKey, NameTable, STANDARD_DEVICE_NAMES, Section,
    Vocabulary, effective_key_map,
};
pub use read::read_key_map;
pub use record::{ActionClass, ActionMap, KeyMapRecord, decode_action_map, decode_key_map_record};
pub use source::{InstallData, ROOT_DID_MAP, load_install_data};
pub use text::{Node, TreeError, parse_file};
pub use write::write_key_map;

use crate::error::{InputError, Result};
use crate::keymap::{Keymap, MAX_KEYMAP_WARNINGS};

/// Largest key-mapping file the importer reads (§12.14.1).
pub const MAX_SOURCE_SIZE: u64 = 1024 * 1024;

/// Options of the companion preferences import (§12.16).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ImportOptions {
    /// `JoystickDeadZone` of the original's preferences: the dead zone of
    /// imported pad axis entries (Tessera's 0.15 when `None`).
    pub pad_deadzone: Option<f32>,
}

/// What an import did (§12.14.1 step 6).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// Whether a user file was given and loaded (not rejected).
    pub file_loaded: bool,
    /// Why the user file was rejected, when it was (the defaults alone are
    /// then imported, as the original runs on them).
    pub rejected: Option<String>,
    /// Why nothing was translated because the install data is incomplete
    /// (no default key-map record, or no command or input-map names): the
    /// returned document is then empty, so Tessera's defaults stay, rather
    /// than one that unbinds every action of §12.14.2.
    pub install_incomplete: Option<String>,
    /// Bindings translated into Tessera entries (a binding copied to two
    /// contexts counts once).
    pub imported: usize,
    /// Bindings of translated commands that have no Tessera form.
    pub dropped: usize,
    /// Warnings, at most [`MAX_KEYMAP_WARNINGS`] plus one "truncated" note.
    pub warnings: Vec<String>,
    truncated: bool,
}

impl ImportReport {
    fn warn(&mut self, msg: String) {
        if self.warnings.len() < MAX_KEYMAP_WARNINGS {
            self.warnings.push(msg);
        } else if !self.truncated {
            self.truncated = true;
            self.warnings.push(format!(
                "more than {MAX_KEYMAP_WARNINGS} warnings; the rest were dropped"
            ));
        }
    }
}

/// Import the original's key map: the user file's bytes (`None` when there
/// is no file) over the install's default key maps.
///
/// Never fails: a file that cannot be used is reported as rejected and the
/// defaults alone are imported. When the install data itself is incomplete
/// (see [`ImportReport::install_incomplete`]) the document is empty. The
/// problems [`load_install_data`] recorded come first in the warnings.
/// Nothing is written anywhere.
#[must_use]
pub fn import_key_map(
    file: Option<&[u8]>,
    data: &InstallData,
    options: &ImportOptions,
) -> (Keymap, ImportReport) {
    let mut report = ImportReport::default();
    for p in &data.problems {
        report.warn(format!("install data: {p}"));
    }
    let vocab = &data.vocabulary;
    let missing: Vec<&str> = [
        (data.key_maps.is_empty(), "no default key-map record"),
        (vocab.commands.is_empty(), "no command names"),
        (vocab.input_maps.is_empty(), "no input-map names"),
    ]
    .into_iter()
    .filter_map(|(m, what)| m.then_some(what))
    .collect();
    if !missing.is_empty() {
        // §12.14.1 step 3 assumes the default records; without them every
        // pair of §12.14.2 would look unbound (keymap.md OPEN, §12.14.1).
        report.install_incomplete = Some(missing.join(", "));
        return (Keymap::new(), report);
    }
    let file_map = file.and_then(|bytes| {
        if bytes.len() as u64 > MAX_SOURCE_SIZE {
            report.rejected = Some(format!("larger than {MAX_SOURCE_SIZE} bytes"));
            return None;
        }
        let mut reports = Vec::new();
        let km = parse_file(bytes, &mut reports)
            .map_err(|e| e.to_string())
            .and_then(|nodes| read_key_map(&nodes, vocab, &mut reports));
        for r in reports {
            report.warn(r);
        }
        match km {
            Ok(km) => {
                report.file_loaded = true;
                Some(km)
            }
            Err(e) => {
                report.rejected = Some(e);
                None
            }
        }
    });
    let defaults: Vec<KeyMap> = data.key_maps.iter().map(|r| r.key_map.clone()).collect();
    let effective = effective_key_map(file_map.as_ref(), &defaults);
    let doc = translate::translate(&effective, vocab, options, &mut report);
    (doc, report)
}

/// Read a key-mapping file for [`import_key_map`]: opened read-only (shared,
/// never locked; non-blocking on Unix, so a FIFO is refused without
/// blocking), regular files only, at most [`MAX_SOURCE_SIZE`] bytes.
/// `Ok(None)` when the file does not exist.
///
/// # Errors
///
/// [`InputError::Io`] when it cannot be read, [`InputError::TooLarge`] past
/// the size limit.
pub fn read_source_file(path: &Path) -> Result<Option<Vec<u8>>> {
    let io = |source| InputError::Io {
        op: "read key-mapping file",
        path: path.to_owned(),
        source,
    };
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(e)),
    };
    if !meta.is_file() {
        return Err(io(std::io::Error::other("not a regular file")));
    }
    // Non-blocking (Unix), so a FIFO swapped in after the check above
    // cannot block the open; the handle is checked again.
    let f = match crate::keymap::open_nonblocking(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(e)),
    };
    if !f.metadata().map_err(io)?.is_file() {
        return Err(io(std::io::Error::other("not a regular file")));
    }
    let mut out = Vec::new();
    f.take(MAX_SOURCE_SIZE + 1)
        .read_to_end(&mut out)
        .map_err(io)?;
    if out.len() as u64 > MAX_SOURCE_SIZE {
        return Err(InputError::TooLarge {
            size: out.len() as u64,
            limit: MAX_SOURCE_SIZE,
        });
    }
    Ok(Some(out))
}
