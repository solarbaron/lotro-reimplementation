// SPDX-License-Identifier: GPL-3.0-or-later
//! The plugin file index: every file of the whole `Plugins/` tree, matched
//! case-insensitively, built by a directory walk on a worker and swapped in whole; and the
//! call-time header probe of `SetBackground(path)`.
//!
//! Spec: decision 0015 §4.3–§4.4 (index of the whole tree, worker walk, swap, 64-byte
//! header read, `.png`/`.bmp` refused); `plugin-descriptor.md` §3.2 (path rules), §4.2
//! ("the plugin file index"); `lua-plugin-api.md` §4.4, §13.8; `plugin-runtime.md` §8.4
//! (no load before the first index), §12.

use std::collections::{HashMap, HashSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use tessera_material::ui_image::{
    ImageFileFormat, ImageOrigin, PROBE_LEN, UiImageError, probe_header,
};

use crate::discovery::{AvailablePlugin, RefreshReport};
use crate::host::HostApi;
use crate::paths::{is_confined, name_order, split_resource_path};

/// Most entries a walk records; a larger tree is indexed up to this many (and logged).
pub const MAX_INDEX_ENTRIES: usize = 1_000_000;

/// Deepest folder level a walk descends to below `Plugins/`.
pub const MAX_INDEX_DEPTH: usize = 32;

/// Most cached header probes; the cache is cleared when it is full or an index is swapped.
const MAX_PROBE_CACHE: usize = 16_384;

/// Most bytes of cached probe keys and paths; the cache is cleared when it would hold more.
const MAX_PROBE_CACHE_BYTES: usize = 4 * 1024 * 1024;

/// Longest plugin image path looked up, in bytes. A longer path cannot name a file the
/// index holds (Tessera bound): it is refused before any lookup and never cached.
pub const MAX_IMAGE_PATH: usize = 1024;

/// How much of an over-long path an error message keeps.
const SHOWN_PATH: usize = 128;

fn fold(s: &str) -> String {
    s.to_ascii_lowercase()
}

/// An index of the files and folders of a `Plugins/` tree.
#[derive(Debug, Clone, Default)]
pub struct PluginFileIndex {
    root: PathBuf,
    /// Folded relative path (`/`-separated) → actual relative paths, in name order.
    files: HashMap<String, Vec<String>>,
    /// Folded relative paths of folders.
    dirs: HashSet<String>,
    /// Folded names of the entries directly inside `Plugins/`: (name, is folder).
    top: Vec<(String, bool)>,
    entries: usize,
    truncated: bool,
}

impl PluginFileIndex {
    /// An empty index of `root` (no folder, or a failed walk).
    #[must_use]
    pub fn empty(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            ..Self::default()
        }
    }

    /// Walk `root` (on the calling thread; the host runs this on a worker). Symlinked
    /// folders are not entered; a symlinked file is indexed only when it resolves inside
    /// `root`. Never fails: an unreadable or missing root gives an empty index.
    #[must_use]
    pub fn build(root: &Path) -> Self {
        let mut idx = Self::empty(root);
        let Ok(canon_root) = root.canonicalize() else {
            tracing::info!(dir = %root.display(), "no plugins folder; the plugin file index is empty");
            return idx;
        };
        let mut stack: Vec<(PathBuf, String, usize)> = vec![(root.to_path_buf(), String::new(), 0)];
        while let Some((dir, rel, depth)) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in rd.flatten() {
                if idx.entries >= MAX_INDEX_ENTRIES {
                    idx.truncated = true;
                    break;
                }
                let Ok(name) = entry.file_name().into_string() else {
                    continue;
                };
                let Ok(meta) = entry.metadata() else { continue };
                let path = entry.path();
                let child_rel = if rel.is_empty() {
                    name.clone()
                } else {
                    format!("{rel}/{name}")
                };
                let (is_dir, is_file) = if meta.file_type().is_symlink() {
                    match path.canonicalize() {
                        Ok(target) if target.starts_with(&canon_root) => (false, target.is_file()),
                        _ => (false, false),
                    }
                } else {
                    (meta.is_dir(), meta.is_file())
                };
                if depth == 0 {
                    idx.top.push((fold(&name), is_dir));
                }
                if is_dir {
                    idx.entries += 1;
                    idx.dirs.insert(fold(&child_rel));
                    if depth + 1 < MAX_INDEX_DEPTH {
                        stack.push((path, child_rel, depth + 1));
                    }
                } else if is_file {
                    idx.entries += 1;
                    idx.files
                        .entry(fold(&child_rel))
                        .or_default()
                        .push(child_rel);
                }
            }
        }
        for v in idx.files.values_mut() {
            v.sort_by(|a, b| name_order(a, b));
        }
        if idx.truncated {
            tracing::warn!(entries = idx.entries, "plugin file index truncated");
        }
        tracing::info!(
            files = idx.files.len(),
            folders = idx.dirs.len(),
            "plugin file index built"
        );
        idx
    }

    /// The indexed `Plugins/` folder.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Number of files indexed.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Did the walk stop at [`MAX_INDEX_ENTRIES`]?
    #[must_use]
    pub fn is_truncated(&self) -> bool {
        self.truncated
    }

    /// The file a plugin path names (`/` or `\` separators, relative to `Plugins/`,
    /// matched case-insensitively; absolute paths, drive letters, `.`, `..` and empty
    /// components never match). Needs no I/O.
    #[must_use]
    pub fn lookup(&self, path: &str) -> Option<PathBuf> {
        let comps = split_resource_path(path)?;
        let key = fold(&comps.join("/"));
        let actual = self.files.get(&key)?.first()?;
        Some(self.root.join(actual))
    }

    /// Is there a folder at this plugin path?
    #[must_use]
    pub fn is_folder(&self, path: &str) -> bool {
        split_resource_path(path).is_some_and(|c| self.dirs.contains(&fold(&c.join("/"))))
    }

    /// The user-copy test of `plugin-class-helper.md` §5.1: a folder `Turbine` or a file
    /// `Turbine.lua` directly inside `Plugins/`.
    #[must_use]
    pub fn has_user_api_copy(&self) -> bool {
        self.top
            .iter()
            .any(|(n, is_dir)| (*is_dir && n == "turbine") || (!*is_dir && n == "turbine.lua"))
    }
}

/// A plugin image that passed the call-time checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedImage {
    /// The file (inside `Plugins/`).
    pub path: PathBuf,
    /// Its container format.
    pub format: ImageFileFormat,
    /// Its declared size, when the header holds it.
    pub dimensions: Option<(u32, u32)>,
}

/// Why `SetBackground(path)` must fail at call time.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageCheckError {
    /// The path is not a valid plugin path or no such file is indexed.
    #[error("cannot open image file {0:?}")]
    NotFound(String),
    /// The file could not be read or resolves outside `Plugins/`.
    #[error("cannot read image file {path:?}: {reason}")]
    Unreadable {
        /// The plugin path.
        path: String,
        /// Why.
        reason: String,
    },
    /// The format is not supported (including `.png` and `.bmp`) or the header is bad.
    #[error("unsupported image file {path:?}: {source}")]
    Unsupported {
        /// The plugin path.
        path: String,
        /// The probe's error.
        #[source]
        source: UiImageError,
    },
}

/// What a worker scan produced: the file index and, when asked for, a discovery scan
/// (`plugin-descriptor.md` §4.2) done in the same job.
#[derive(Debug)]
pub(crate) struct ScanResult {
    index: PluginFileIndex,
    discovered: Option<(Vec<AvailablePlugin>, RefreshReport)>,
}

/// A walk that was swapped in, with the discovery scan done with it, if any.
#[derive(Debug)]
pub(crate) struct Swapped {
    pub discovered: Option<(Vec<AvailablePlugin>, RefreshReport)>,
}

/// The host's index state: the current index, a walk in progress, the probe cache.
#[derive(Debug, Default)]
pub(crate) struct IndexState {
    current: Option<Arc<PluginFileIndex>>,
    pending: Option<Receiver<ScanResult>>,
    /// A scan was asked for while one was running: run one more when it finishes (with
    /// discovery when `true`).
    rerun: Option<bool>,
    first_ready: bool,
    probes: HashMap<String, Result<CheckedImage, ImageCheckError>>,
    probe_bytes: usize,
}

impl IndexState {
    /// Start a walk of `root` on a worker, with a discovery scan when `discover` is set;
    /// the previous index (and available list) serves until the result is swapped in.
    /// While a walk runs, the request is remembered and one more walk starts when it
    /// finishes (requests never pile up).
    pub(crate) fn start_build(&mut self, host: &dyn HostApi, root: PathBuf, discover: bool) {
        if self.pending.is_some() {
            self.rerun = Some(self.rerun.unwrap_or(false) || discover);
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        host.spawn_worker(Box::new(move || {
            let discovered = discover.then(|| crate::discovery::discover(&root));
            let index = PluginFileIndex::build(&root);
            let _ = tx.send(ScanResult { index, discovered });
        }));
    }

    /// Swap in a finished walk (on the main thread). A walk that died yields an empty
    /// index. Returns `None` while nothing finished.
    pub(crate) fn poll(&mut self, host: &dyn HostApi, root: &Path) -> Option<Swapped> {
        let rx = self.pending.as_ref()?;
        let result = match rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => {
                tracing::warn!("the plugin index walk failed; using an empty index");
                ScanResult {
                    index: PluginFileIndex::empty(root),
                    discovered: None,
                }
            }
        };
        Some(Swapped {
            discovered: self.finish(host, root, result),
        })
    }

    /// Block until the pending walk finishes or `timeout` passes (tools and tests).
    /// Returns whether a first index is ready, and the discovery scan of the walk that
    /// finished, if any.
    pub(crate) fn wait(
        &mut self,
        host: &dyn HostApi,
        root: &Path,
        timeout: std::time::Duration,
    ) -> (bool, Option<(Vec<AvailablePlugin>, RefreshReport)>) {
        let mut found = None;
        if let Some(rx) = &self.pending {
            let result = match rx.recv_timeout(timeout) {
                Ok(r) => r,
                Err(mpsc::RecvTimeoutError::Timeout) => return (self.first_ready, None),
                Err(mpsc::RecvTimeoutError::Disconnected) => ScanResult {
                    index: PluginFileIndex::empty(root),
                    discovered: None,
                },
            };
            found = self.finish(host, root, result);
        }
        (self.first_ready, found)
    }

    fn finish(
        &mut self,
        host: &dyn HostApi,
        root: &Path,
        result: ScanResult,
    ) -> Option<(Vec<AvailablePlugin>, RefreshReport)> {
        self.swap(result.index);
        if let Some(discover) = self.rerun.take() {
            self.start_build(host, root.to_path_buf(), discover);
        }
        result.discovered
    }

    /// Is a walk running?
    pub(crate) fn walking(&self) -> bool {
        self.pending.is_some()
    }

    fn swap(&mut self, idx: PluginFileIndex) {
        self.current = Some(Arc::new(idx));
        self.pending = None;
        self.first_ready = true;
        self.probes.clear();
        self.probe_bytes = 0;
    }

    /// Install an index built on the calling thread.
    pub(crate) fn set(&mut self, idx: PluginFileIndex) {
        self.swap(idx);
    }

    pub(crate) fn first_ready(&self) -> bool {
        self.first_ready
    }

    pub(crate) fn current(&self) -> Option<Arc<PluginFileIndex>> {
        self.current.clone()
    }

    /// The call-time check of `SetBackground(path)` (0015 §4.4): existence from the index
    /// (or, before the first index, a direct confined lookup), then one header read of at
    /// most 64 bytes; the result is cached per path.
    ///
    /// A path longer than [`MAX_IMAGE_PATH`] is refused as not found before any lookup,
    /// and not-found results are not cached, so the cache only ever holds paths of files
    /// that exist; it is bounded by entry count and by total bytes.
    pub(crate) fn check_image(
        &mut self,
        root: &Path,
        path: &str,
    ) -> Result<CheckedImage, ImageCheckError> {
        if path.len() > MAX_IMAGE_PATH {
            return Err(ImageCheckError::NotFound(shown_path(path)));
        }
        if let Some(hit) = self.probes.get(path) {
            return hit.clone();
        }
        let found = match &self.current {
            Some(idx) => idx.lookup(path),
            None => split_resource_path(path)
                .and_then(|c| crate::paths::resolve_ci(root, &c, crate::paths::Kind::File)),
        };
        let Some(file) = found else {
            return Err(ImageCheckError::NotFound(path.to_owned()));
        };
        let result = probe_file(root, &file, path);
        let cost = 2 * path.len() + file.as_os_str().len() + 64;
        if self.probes.len() >= MAX_PROBE_CACHE
            || self.probe_bytes.saturating_add(cost) > MAX_PROBE_CACHE_BYTES
        {
            self.probes.clear();
            self.probe_bytes = 0;
        }
        self.probe_bytes += cost;
        self.probes.insert(path.to_owned(), result.clone());
        result
    }
}

/// The start of an over-long path, for an error message.
fn shown_path(path: &str) -> String {
    let mut end = SHOWN_PATH.min(path.len());
    while !path.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &path[..end])
}

fn probe_file(root: &Path, file: &Path, path: &str) -> Result<CheckedImage, ImageCheckError> {
    if !is_confined(root, file) {
        return Err(ImageCheckError::Unreadable {
            path: path.to_owned(),
            reason: "outside the plugins folder".to_owned(),
        });
    }
    let unreadable = |e: std::io::Error| ImageCheckError::Unreadable {
        path: path.to_owned(),
        reason: e.to_string(),
    };
    let f = std::fs::File::open(file).map_err(unreadable)?;
    let mut head = Vec::with_capacity(PROBE_LEN);
    f.take(PROBE_LEN as u64)
        .read_to_end(&mut head)
        .map_err(unreadable)?;
    let probe = probe_header(file, ImageOrigin::Plugin, &head).map_err(|source| {
        ImageCheckError::Unsupported {
            path: path.to_owned(),
            source,
        }
    })?;
    Ok(CheckedImage {
        path: file.to_path_buf(),
        format: probe.format,
        dimensions: probe.dimensions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An 18-byte TGA header: uncompressed true-colour, 2×1, 32 bits.
    fn tga() -> Vec<u8> {
        let mut h = vec![0u8; 18];
        h[2] = 2;
        h[12] = 2;
        h[14] = 1;
        h[16] = 32;
        h.extend_from_slice(&[0; 8]);
        h
    }

    #[test]
    fn index_and_probe() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Plugins");
        std::fs::create_dir_all(root.join("Author/Img")).unwrap();
        std::fs::write(root.join("Author/Img/Icon.TGA"), tga()).unwrap();
        std::fs::write(root.join("Author/Img/pic.png"), b"\x89PNG").unwrap();
        std::fs::write(root.join("Author/Img/bad.jpg"), b"nope").unwrap();
        std::fs::create_dir_all(root.join("turbine")).unwrap();
        let idx = PluginFileIndex::build(&root);
        assert!(idx.has_user_api_copy());
        assert!(idx.lookup("author\\img/icon.tga").is_some());
        assert!(idx.lookup("Author/Img/../Img/Icon.TGA").is_none());
        assert!(idx.lookup("/Author/Img/Icon.TGA").is_none());
        assert!(idx.is_folder("AUTHOR/img"));

        let mut st = IndexState::default();
        st.set(idx);
        let ok = st.check_image(&root, "Author/Img/icon.tga").unwrap();
        assert_eq!(ok.format, ImageFileFormat::Tga);
        assert_eq!(ok.dimensions, Some((2, 1)));
        assert!(matches!(
            st.check_image(&root, "Author/Img/pic.png"),
            Err(ImageCheckError::Unsupported { .. })
        ));
        assert!(matches!(
            st.check_image(&root, "Author/Img/bad.jpg"),
            Err(ImageCheckError::Unsupported { .. })
        ));
        assert!(matches!(
            st.check_image(&root, "Author/missing.tga"),
            Err(ImageCheckError::NotFound(_))
        ));
        // Not-found results and over-long paths are never cached (review finding: the
        // cache could hold 16,384 copies of huge plugin-supplied paths).
        let long = format!("Author/{}", "a".repeat(MAX_IMAGE_PATH));
        match st.check_image(&root, &long) {
            Err(ImageCheckError::NotFound(p)) => assert!(p.len() < 2 * SHOWN_PATH),
            other => panic!("{other:?}"),
        }
        assert_eq!(st.probes.len(), 3);
        assert!(st.probe_bytes < MAX_PROBE_CACHE_BYTES);
        let missing = PluginFileIndex::build(&dir.path().join("none"));
        assert_eq!(missing.file_count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_out_of_the_tree_are_not_indexed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Plugins");
        std::fs::create_dir_all(root.join("A")).unwrap();
        std::fs::write(dir.path().join("secret.tga"), tga()).unwrap();
        std::os::unix::fs::symlink(dir.path().join("secret.tga"), root.join("A/link.tga")).unwrap();
        std::os::unix::fs::symlink(dir.path(), root.join("A/up")).unwrap();
        let idx = PluginFileIndex::build(&root);
        assert!(idx.lookup("A/link.tga").is_none());
        assert!(idx.lookup("A/up/secret.tga").is_none());
    }
}
