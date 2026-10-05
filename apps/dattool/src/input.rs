// SPDX-License-Identifier: GPL-3.0-or-later
//! Turning command-line arguments into opened archives.
//!
//! An `<archive>` argument is a path to a `.dat`/`.datx` file, or a short archive name
//! (`general`, `client_surface_aux_1.datx`, `cell_2`) looked up in the game directory.
//! A `<dir>` argument is an install directory (every archive in it is opened as a set);
//! where a command accepts `<dir|archive>`, a file or an archive name selects one archive.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use tessera_dat::{Archive, ArchiveId, ArchiveKind, ArchiveSet, Did, Entry, SetOptions};

use crate::output::Protected;

/// Options shared by every subcommand.
#[derive(Debug, Clone, Default)]
pub struct Ctx {
    /// `--game-dir`, used to resolve bare archive names.
    pub game_dir: Option<PathBuf>,
}

impl Ctx {
    /// The directory bare archive names are looked up in: `--game-dir`, then
    /// `TESSERA_GAME_DIR`, then the first install found by discovery.
    pub fn resolve_game_dir(&self) -> Option<PathBuf> {
        if let Some(d) = &self.game_dir {
            return Some(d.clone());
        }
        if let Some(d) = std::env::var_os(tessera_install::GAME_DIR_ENV).filter(|v| !v.is_empty()) {
            return Some(PathBuf::from(d));
        }
        match tessera_install::discover() {
            Ok(d) => d.default_install().map(|i| i.root.clone()),
            Err(e) => {
                tracing::debug!(error = %e, "install discovery failed");
                None
            }
        }
    }
}

/// Display label of an archive file: its set identity (`surface_aux_1`) or the file name.
pub fn label_of(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    ArchiveId::from_file_name(&name)
        .or_else(|| ArchiveId::from_file_name(&name.to_ascii_lowercase()))
        .map(|id| id.to_string())
        .unwrap_or(name)
}

/// Normalises an archive name: `client_Surface_aux_1.datx` → `surface_aux_1`.
fn normalize_name(arg: &str) -> String {
    let a = arg.to_ascii_lowercase();
    let a = a.strip_prefix("client_").unwrap_or(&a);
    let a = a
        .strip_suffix(".datx")
        .or_else(|| a.strip_suffix(".dat"))
        .unwrap_or(a);
    a.to_owned()
}

/// Finds the archive file named `arg` (see module docs) in `dir`.
fn find_in_dir(dir: &Path, arg: &str) -> Result<Option<PathBuf>> {
    let want = normalize_name(arg);
    let rd = std::fs::read_dir(dir).with_context(|| format!("cannot list {}", dir.display()))?;
    let mut names: Vec<String> = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let lower = name.to_ascii_lowercase();
        if !(lower.ends_with(".dat") || lower.ends_with(".datx")) {
            continue;
        }
        if let Some(id) = ArchiveId::from_file_name(&lower) {
            if id.to_string().eq_ignore_ascii_case(&want) {
                return Ok(Some(e.path()));
            }
            names.push(id.to_string());
        }
    }
    names.sort();
    tracing::debug!(dir = %dir.display(), known = ?names, "archive name not found");
    Ok(None)
}

/// Resolves an `<archive>` argument to a file path.
pub fn resolve_archive_path(arg: &str, ctx: &Ctx) -> Result<PathBuf> {
    let p = Path::new(arg);
    if p.is_file() {
        return Ok(p.to_owned());
    }
    if p.is_dir() {
        bail!(
            "{} is a directory; this command needs one archive file (or a name like `general`)",
            p.display()
        );
    }
    if arg.contains('/') || arg.contains('\\') {
        bail!("{arg}: no such file");
    }
    let Some(dir) = ctx.resolve_game_dir() else {
        bail!(
            "{arg}: no such file, and no game directory to look the name up in \
             (pass --game-dir or set {})",
            tessera_install::GAME_DIR_ENV
        );
    };
    find_in_dir(&dir, arg)?.with_context(|| {
        format!(
            "{arg}: no such file, and no archive of that name in {}",
            dir.display()
        )
    })
}

/// Opens one archive read-only.
pub fn open_archive(arg: &str, ctx: &Ctx) -> Result<(String, Archive)> {
    let path = resolve_archive_path(arg, ctx)?;
    let archive =
        Archive::open(&path).with_context(|| format!("cannot open {}", path.display()))?;
    Ok((label_of(&path), archive))
}

/// Either one archive or every archive of a directory.
#[derive(Debug)]
pub enum Input {
    /// A single archive file.
    Archive {
        /// Display label.
        label: String,
        /// The opened archive.
        archive: Box<Archive>,
    },
    /// All archives of an install directory.
    Set(ArchiveSet),
}

/// An archive file of a directory input that could not be opened (its header failed the
/// checks of `docs/specs/dat-container.md` §10, I-1/I-2/I-4/I-8, or it could not be read).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unopened {
    /// The file.
    pub path: PathBuf,
    /// Display label, as [`label_of`] gives it.
    pub label: String,
    /// Whether the name is one of the archive kinds of the spec (`general`, `local_<Lang>`,
    /// `cell_<r>`, ... and their aux parts). Any other `client_<x>` file is not part of the
    /// set (install-layout.md §3.1), so failing to open it is not trouble.
    pub recognised: bool,
    /// Why it could not be opened.
    pub error: String,
}

/// Where a DID was found.
#[derive(Debug)]
pub struct Found<'a> {
    /// Label of the archive holding it.
    pub origin: String,
    /// Its directory entry.
    pub entry: Entry,
    /// For a set: whether a routed candidate held it.
    pub routed: Option<bool>,
    archive: &'a Archive,
}

impl Found<'_> {
    /// The (inflated) payload.
    pub fn read(&self) -> Result<Vec<u8>> {
        self.archive
            .read_entry(&self.entry)
            .with_context(|| format!("cannot read {:#010x} from {}", self.entry.did, self.origin))
    }

    /// The stored (on-disk, possibly compressed) payload.
    pub fn read_stored(&self) -> Result<Vec<u8>> {
        self.archive
            .read_raw(&self.entry)
            .with_context(|| format!("cannot read {:#010x} from {}", self.entry.did, self.origin))
    }
}

impl Input {
    /// Opens a `<dir|archive>` argument.
    pub fn open(arg: &str, ctx: &Ctx) -> Result<Self> {
        let p = Path::new(arg);
        if p.is_dir() {
            let set = ArchiveSet::open(p, SetOptions::default())
                .with_context(|| format!("cannot open the archives in {}", p.display()))?;
            for prob in set.problems() {
                match &prob.path {
                    Some(path) => tracing::warn!(path = %path.display(), "{}", prob.message),
                    None => tracing::warn!("{}", prob.message),
                }
            }
            return Ok(Self::Set(set));
        }
        let (label, archive) = open_archive(arg, ctx)?;
        Ok(Self::Archive {
            label,
            archive: Box::new(archive),
        })
    }

    /// The archive files of a directory input that could not be opened. They are not part
    /// of [`Input::archives`]; commands report them and treat a recognised one as an input
    /// that could not be read completely.
    pub fn unopened(&self) -> Vec<Unopened> {
        let Self::Set(set) = self else {
            return Vec::new();
        };
        set.problems()
            .iter()
            .filter_map(|p| {
                // `ArchiveSet::open` reports a file it skipped as "not opened: <error>"; its
                // other problems (membership, ignored duplicates) concern files that are
                // either open or deliberately left out.
                let error = p.message.strip_prefix("not opened: ")?;
                let path = p.path.as_ref()?;
                if set.members().iter().any(|m| &m.path == path) {
                    return None;
                }
                let kind = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(ArchiveId::from_file_name)
                    .map(|id| id.kind);
                Some(Unopened {
                    path: path.clone(),
                    label: label_of(path),
                    recognised: kind.is_some_and(|k| !matches!(k, ArchiveKind::Other(_))),
                    error: error.to_owned(),
                })
            })
            .collect()
    }

    /// The recognised archives of [`Input::unopened`].
    pub fn unopened_recognised(&self) -> Vec<Unopened> {
        let mut v = self.unopened();
        v.retain(|u| u.recognised);
        v
    }

    /// Every archive as `(label, archive)`, in set order.
    pub fn archives(&self) -> Vec<(String, &Archive)> {
        match self {
            Self::Archive { label, archive } => vec![(label.clone(), archive.as_ref())],
            Self::Set(set) => set
                .members()
                .iter()
                .map(|m| (m.id.to_string(), &m.archive))
                .collect(),
        }
    }

    /// The directory the input lives in (never written to).
    pub fn home_dir(&self) -> Option<PathBuf> {
        match self {
            Self::Archive { archive, .. } => archive
                .path()
                .and_then(Path::parent)
                .map(|p| {
                    if p.as_os_str().is_empty() {
                        Path::new(".")
                    } else {
                        p
                    }
                })
                .map(Path::to_owned),
            Self::Set(set) => Some(set.dir().to_owned()),
        }
    }

    /// The directories this input lives in, all of which output must stay out of: the
    /// directory as given and, for archives reached through symlinks, the directory each
    /// archive file really lives in.
    pub fn protected_dirs(&self) -> Vec<Protected> {
        let real_parent = |p: &Path| {
            std::fs::canonicalize(p)
                .ok()
                .and_then(|c| c.parent().map(Path::to_owned))
        };
        let mut v = Vec::new();
        match self {
            Self::Archive { archive, .. } => {
                let why = "the directory holding the input archive";
                v.extend(self.home_dir().map(|d| Protected::new(d, why)));
                v.extend(
                    archive
                        .path()
                        .and_then(real_parent)
                        .map(|d| Protected::new(d, why)),
                );
            }
            Self::Set(set) => {
                v.push(Protected::new(set.dir(), "the game directory"));
                for m in set.members() {
                    if let Some(d) = real_parent(&m.path)
                        && !v.iter().any(|p| p.path == d)
                    {
                        v.push(Protected::new(d, "the directory holding an input archive"));
                    }
                }
            }
        }
        v
    }

    /// Finds `did`.
    pub fn locate(&self, did: Did) -> Result<Option<Found<'_>>> {
        match self {
            Self::Archive { label, archive } => Ok(archive
                .lookup(did)
                .with_context(|| format!("directory lookup in {label} failed"))?
                .map(|entry| Found {
                    origin: label.clone(),
                    entry,
                    routed: None,
                    archive,
                })),
            Self::Set(set) => {
                let Some(l) = set.locate(did).context("directory lookup failed")? else {
                    return Ok(None);
                };
                let m = set
                    .archive_of(&l)
                    .context("located member is not part of the set")?;
                Ok(Some(Found {
                    origin: m.id.to_string(),
                    entry: l.entry,
                    routed: Some(l.routed),
                    archive: &m.archive,
                }))
            }
        }
    }

    /// Like [`Input::locate`] but fails when the DID is absent.
    pub fn require(&self, did: Did) -> Result<Found<'_>> {
        self.locate(did)?
            .with_context(|| format!("DID {did:#010x} not found"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_normalize() {
        assert_eq!(normalize_name("client_general.dat"), "general");
        assert_eq!(normalize_name("client_Surface_aux_1.datx"), "surface_aux_1");
        assert_eq!(normalize_name("cell_2"), "cell_2");
        assert_eq!(normalize_name("LOCAL_English"), "local_english");
    }

    #[test]
    fn labels() {
        assert_eq!(
            label_of(Path::new("/x/client_mesh_aux_1.datx")),
            "mesh_aux_1"
        );
        assert_eq!(
            label_of(Path::new("client_local_English.dat")),
            "local_English"
        );
        assert_eq!(label_of(Path::new("other.bin")), "other.bin");
    }
}
