// SPDX-License-Identifier: GPL-3.0-or-later
//! Where skins live and how they are found (`skin-definition.md` §3, §9.1; D1–D4, D9,
//! D10; `install-layout.md` §8).
//!
//! Everything here only reads: nothing is created, written, renamed or locked below the
//! settings folder (D1).

use std::cmp::Ordering;
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::definition::{Definition, DefinitionError, MAX_DEFINITION_BYTES, parse_definition};
use super::tables::NameResolver;

/// File name of a definition file, matched ignoring ASCII case (§3.2).
pub const DEFINITION_FILE: &str = "SkinDefinition.xml";
/// Folder levels walked below the skins root (D4).
pub const MAX_WALK_DEPTH: usize = 16;
/// Entries per folder; a larger folder is skipped (D4).
pub const MAX_FOLDER_ENTRIES: usize = 4_096;
/// Skins listed at most (D4).
pub const MAX_SKINS: usize = 1_024;

/// The skins root of a settings folder: `<settings folder>/ui/skins` (§3.1, D1), each
/// component matched ignoring ASCII case when the exact name does not exist (D2). The path
/// is returned even when it does not exist.
#[must_use]
pub fn skins_root_in(settings_folder: &Path) -> PathBuf {
    let ui = child_ci(settings_folder, "ui");
    child_ci(&ui, "skins")
}

/// The default skins root for a settings folder found by `tessera-install`
/// (`install-layout.md` §8.1, including `--prefs`).
#[must_use]
pub fn default_skins_root(settings: &tessera_install::SettingsDir) -> PathBuf {
    skins_root_in(&settings.path)
}

/// `parent/name`, or the entry of `parent` matching `name` ignoring ASCII case (smallest by
/// byte value), when the exact one does not exist (D2).
fn child_ci(parent: &Path, name: &str) -> PathBuf {
    let exact = parent.join(name);
    if std::fs::symlink_metadata(&exact).is_ok() {
        return exact;
    }
    let mut best: Option<std::ffi::OsString> = None;
    if let Ok(rd) = std::fs::read_dir(parent) {
        for e in rd.flatten().take(MAX_FOLDER_ENTRIES) {
            let n = e.file_name();
            if n.to_str().is_some_and(|s| s.eq_ignore_ascii_case(name))
                && best
                    .as_ref()
                    .is_none_or(|b| n.as_encoded_bytes() < b.as_encoded_bytes())
            {
                best = Some(n);
            }
        }
    }
    best.map_or(exact, |b| parent.join(b))
}

/// One skin: a definition file that parsed and passed §4.8 (§3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skin {
    /// The definition file.
    pub definition_file: PathBuf,
    /// The skin folder (the definition file's folder), base of every `FileName` (§6.4).
    pub folder: PathBuf,
    /// The parsed definition.
    pub definition: Definition,
}

impl Skin {
    /// The skin's name (§4.3).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.definition.name
    }
}

/// A definition file that is not a skin, with the reason (D9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedDefinition {
    /// The file.
    pub path: PathBuf,
    /// Why.
    pub error: DefinitionError,
}

/// A problem of the walk itself (not of a definition file). The `Display` wording is the
/// one the logs and the options entry share (D9).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum WalkIssue {
    /// A folder with more than [`MAX_FOLDER_ENTRIES`] entries was skipped (D4).
    #[error("folder {0:?} has more than {MAX_FOLDER_ENTRIES} entries; skipped")]
    TooManyEntries(PathBuf),
    /// A folder deeper than [`MAX_WALK_DEPTH`] levels was not walked (D4).
    #[error("folder {0:?} is more than {MAX_WALK_DEPTH} levels below the skins root; not walked")]
    TooDeep(PathBuf),
    /// More than [`MAX_SKINS`] skins; the rest were not parsed (D4).
    #[error("more than {MAX_SKINS} skins; the rest are ignored")]
    TooManySkins,
    /// A folder could not be listed.
    #[error("folder {0:?} cannot be listed: {1}")]
    Unreadable(PathBuf, String),
    /// A definition file whose name differs only in letter case from another one in the
    /// same folder that sorts first by byte value; only that one is used (D2: on the
    /// original's case-insensitive file system a folder holds one definition file).
    #[error("definition file {path:?} ignored: {by:?} differs from it only in letter case (D2)")]
    ShadowedDefinition {
        /// The file that is ignored.
        path: PathBuf,
        /// The file used instead.
        by: PathBuf,
    },
}

/// An entry of the skin list (§9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinChoice<'a> {
    /// "No skin" (Tessera's own text, D6).
    NoSkin,
    /// A skin, by name; choosing it activates the first discovered skin of that name
    /// ignoring ASCII case (§9.3).
    Skin(&'a str),
}

/// The skins found below a skins root (§3.2), in discovery order (D3).
#[derive(Debug, Clone, Default)]
pub struct SkinSet {
    root: PathBuf,
    skins: Vec<Arc<Skin>>,
    rejected: Vec<RejectedDefinition>,
    issues: Vec<WalkIssue>,
}

impl SkinSet {
    /// Discovers and parses every definition file below `root` (§3.2 with D2–D4, D9).
    /// Run once when the UI starts (§9.4); running it again is the developer reload of
    /// D10. A missing root gives an empty set.
    #[must_use]
    pub fn discover(root: &Path, resolver: &dyn NameResolver) -> Self {
        let mut set = Self {
            root: root.to_path_buf(),
            ..Self::default()
        };
        let Ok(canonical_root) = std::fs::canonicalize(root) else {
            tracing::debug!(root = %root.display(), "no skins root");
            return set;
        };
        if !canonical_root.is_dir() {
            return set;
        }
        let mut candidates = Vec::new();
        let mut visited = HashSet::new();
        visited.insert(canonical_root.clone());
        let mut walk = Walk {
            root: &canonical_root,
            visited,
            candidates: &mut candidates,
            issues: &mut set.issues,
        };
        walk.folder(root, 0);
        for path in candidates {
            if set.skins.len() >= MAX_SKINS {
                record(&mut set.issues, WalkIssue::TooManySkins);
                break;
            }
            match load_definition(&path, resolver) {
                Ok(definition) => {
                    for d in &definition.diagnostics {
                        tracing::warn!(file = %path.display(), line = d.line, reason = %d.kind,
                            "skin definition entry rejected");
                    }
                    let folder = path.parent().map(Path::to_path_buf).unwrap_or_default();
                    set.skins.push(Arc::new(Skin {
                        definition_file: path,
                        folder,
                        definition,
                    }));
                }
                Err(error) => {
                    tracing::warn!(file = %path.display(), line = error.line(), reason = %error,
                        "skin definition rejected");
                    set.rejected.push(RejectedDefinition { path, error });
                }
            }
        }
        set
    }

    /// The skins root this set was discovered from.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The skins in discovery order.
    #[must_use]
    pub fn skins(&self) -> &[Arc<Skin>] {
        &self.skins
    }

    /// The definition files that are not skins, with reasons (D9).
    #[must_use]
    pub fn rejected(&self) -> &[RejectedDefinition] {
        &self.rejected
    }

    /// Problems of the walk (limits, unreadable folders).
    #[must_use]
    pub fn issues(&self) -> &[WalkIssue] {
        &self.issues
    }

    /// The skin list shown to the user (§9.1): "no skin", then every skin name (duplicates
    /// and empty names included) sorted by UTF-16 code unit.
    #[must_use]
    pub fn choices(&self) -> Vec<SkinChoice<'_>> {
        let mut names: Vec<&str> = self.skins.iter().map(|s| s.name()).collect();
        names.sort_by(|a, b| cmp_utf16(a, b));
        std::iter::once(SkinChoice::NoSkin)
            .chain(names.into_iter().map(SkinChoice::Skin))
            .collect()
    }

    /// The skin that choosing `name` activates (§9.3): the first in discovery order whose
    /// name equals `name` ignoring ASCII case. An empty `name` means "no skin" (D6), so a
    /// skin with an empty name cannot be chosen.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&Arc<Skin>> {
        if name.is_empty() {
            return None;
        }
        self.skins
            .iter()
            .find(|s| s.name().eq_ignore_ascii_case(name))
    }
}

/// Compares by UTF-16 code units (§9.1).
fn cmp_utf16(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

fn load_definition(
    path: &Path,
    resolver: &dyn NameResolver,
) -> Result<Definition, DefinitionError> {
    let io = |e: std::io::Error| DefinitionError::Io(e.to_string());
    let file = std::fs::File::open(path).map_err(io)?;
    let len = file.metadata().map_err(io)?.len();
    if len > MAX_DEFINITION_BYTES as u64 {
        return Err(DefinitionError::TooLarge { len });
    }
    let mut bytes = Vec::new();
    file.take(MAX_DEFINITION_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if bytes.len() > MAX_DEFINITION_BYTES {
        return Err(DefinitionError::TooLarge {
            len: bytes.len() as u64,
        });
    }
    parse_definition(&bytes, resolver)
}

/// Sort key of D3: the name with ASCII case folded, then its bytes.
fn entry_order(a: &std::ffi::OsStr, b: &std::ffi::OsStr) -> Ordering {
    let (a, b) = (a.as_encoded_bytes(), b.as_encoded_bytes());
    a.iter()
        .map(u8::to_ascii_lowercase)
        .cmp(b.iter().map(u8::to_ascii_lowercase))
        .then_with(|| a.cmp(b))
}

/// Logs a walk issue (warning level, its `Display` wording) and keeps it for the options
/// entry (D9).
fn record(issues: &mut Vec<WalkIssue>, issue: WalkIssue) {
    tracing::warn!(reason = %issue, "skin discovery");
    issues.push(issue);
}

struct Walk<'a> {
    root: &'a Path,
    visited: HashSet<PathBuf>,
    candidates: &'a mut Vec<PathBuf>,
    issues: &'a mut Vec<WalkIssue>,
}

impl Walk<'_> {
    /// Walks `dir` (at `depth` levels below the root): entries sorted (D3); a matching
    /// file name is a candidate, a folder is walked before the next entry (§3.2).
    fn folder(&mut self, dir: &Path, depth: usize) {
        let rd = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(e) => {
                record(
                    self.issues,
                    WalkIssue::Unreadable(dir.to_path_buf(), e.to_string()),
                );
                return;
            }
        };
        let mut names = Vec::new();
        for e in rd.flatten() {
            if names.len() >= MAX_FOLDER_ENTRIES {
                record(self.issues, WalkIssue::TooManyEntries(dir.to_path_buf()));
                return;
            }
            names.push(e.file_name());
        }
        names.sort_by(|a, b| entry_order(a, b));
        // D2: the definition file name is matched ignoring ASCII case; of several entries
        // that match, the first by byte value (the first in this order) is the folder's
        // definition file and the others are shadowed.
        let mut definition: Option<PathBuf> = None;
        for name in names {
            let path = dir.join(&name);
            // Follow links only when they stay inside the root (D4).
            let Ok(canonical) = std::fs::canonicalize(&path) else {
                continue;
            };
            if !canonical.starts_with(self.root) {
                tracing::debug!(path = %path.display(), "link out of the skins root not followed");
                continue;
            }
            let Ok(meta) = std::fs::metadata(&canonical) else {
                continue;
            };
            if meta.is_file() {
                if name
                    .to_str()
                    .is_some_and(|n| n.eq_ignore_ascii_case(DEFINITION_FILE))
                {
                    match &definition {
                        None => {
                            definition = Some(path.clone());
                            self.candidates.push(path);
                        }
                        Some(by) => record(
                            self.issues,
                            WalkIssue::ShadowedDefinition {
                                path,
                                by: by.clone(),
                            },
                        ),
                    }
                }
            } else if meta.is_dir() {
                if depth + 1 > MAX_WALK_DEPTH {
                    record(self.issues, WalkIssue::TooDeep(path));
                    continue;
                }
                if self.visited.insert(canonical) {
                    self.folder(&path, depth + 1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_order_is_case_sensitive() {
        assert_eq!(cmp_utf16("Z", "a"), Ordering::Less);
        // U+1F600 (surrogates D83D…) sorts before U+FF5E by UTF-16 code unit, unlike by
        // scalar value.
        assert_eq!(cmp_utf16("\u{1F600}", "\u{FF5E}"), Ordering::Less);
    }

    #[test]
    fn entry_order_folds_case_first() {
        let o = |a: &str, b: &str| entry_order(a.as_ref(), b.as_ref());
        assert_eq!(o("extra", "SkinDefinition.xml"), Ordering::Less);
        assert_eq!(o("Beta", "alpha"), Ordering::Greater);
        assert_eq!(o("A", "a"), Ordering::Less);
    }
}
