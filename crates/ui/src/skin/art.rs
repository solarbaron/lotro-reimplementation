// SPDX-License-Identifier: GPL-3.0-or-later
//! Art replacement (`skin-definition.md` §6, §7; D2, D7, D11, D12, D16, D25–D27).
//!
//! - [`resolve_image`]: the per-image precedence of §6.3 (user skin, then game skin, then
//!   the base art; only DIDs in the base table are affected).
//! - [`ArtReplacement`] / [`SkinFile`]: a mapping's `FileName` turned into a path (§6.4),
//!   its cache key (the lower-cased path, §6.6) and its decoder (D7), without file-system
//!   work.
//! - [`SkinFile::load_with`]: letter-case resolution (D2) through the activation's
//!   [`SkinDirIndex`], then read and decode on an asset worker through
//!   `tessera-material::ui_image` (TGA, DDS, JPEG; images over 4,096 pixels on an axis are
//!   cropped at decode, D25; each image carries its source-format tag, D26).
//! - [`SkinFileLedger`]: per-activation load states with [`LoadTicket`]s, failed loads
//!   remembered until the next switch and logged once (D27); pending is not failed (D16).

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex, PoisonError};

use tessera_material::ui_image::{
    self, ImageFileFormat, ImageOrigin, MAX_FILE_BYTES, UiImage, UiImageError,
};

use super::definition::Mapping;
use super::tables::{BaseTable, GameSkin};
use crate::paint::{ImageKey, MAX_PATH_KEY};

/// What to draw for an element's base image set from a render-surface DID (§6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageChoice<'a> {
    /// The DID itself, unchanged (no skin active, the DID is not in the base table, or no
    /// active skin maps its art asset).
    Base(u32),
    /// A game skin's replacement DID (§6.3 step 4).
    GameSkin {
        /// The replacement render surface.
        did: u32,
        /// The art-asset value it replaces.
        art_asset: u32,
    },
    /// A user-skin file (§6.3 step 3). If it cannot be loaded the element shows the
    /// placeholder, never the base DID (§6.5, D16).
    SkinFile(&'a ArtReplacement),
}

/// The precedence of §6.3 for an image requested by `did`: the active user skin's
/// replacements (`user`, keyed by art-asset value), then the active game skin, then `did`.
/// With neither active, or a DID that is not in `base`, the result is `did`.
#[must_use]
pub fn resolve_image<'a>(
    did: u32,
    base: &BaseTable,
    user: Option<&'a HashMap<u32, ArtReplacement>>,
    game: Option<&GameSkin>,
) -> ImageChoice<'a> {
    if user.is_none() && game.is_none() {
        return ImageChoice::Base(did);
    }
    let Some(art) = base.art_asset(did) else {
        return ImageChoice::Base(did);
    };
    if let Some(r) = user.and_then(|u| u.get(&art)) {
        return ImageChoice::SkinFile(r);
    }
    if let Some(replacement) = game.and_then(|g| g.replacement(art)) {
        return ImageChoice::GameSkin {
            did: replacement,
            art_asset: art,
        };
    }
    ImageChoice::Base(did)
}

/// Why a `FileName` gives no usable path (§6.4, D12). Such a mapping shows the placeholder.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SkinPathError {
    /// A doubled-separator (network) path (D12).
    #[error("network paths are not supported")]
    NetworkPath,
    /// A drive-letter path with no Wine prefix to map it through, a drive letter that is
    /// not an ASCII letter, or a `..` component in a drive-letter path (D12;
    /// `install-layout.md` §5.2).
    #[error("drive-letter path {0:?} cannot be mapped")]
    UnmappedDrive(String),
    /// The lower-cased path is longer than the cache-key limit.
    #[error("path longer than {MAX_PATH_KEY} bytes")]
    TooLong,
}

/// Why a skin file could not be loaded. Every variant means the placeholder (§6.5, D16).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SkinImageError {
    /// No usable path.
    #[error(transparent)]
    Path(#[from] SkinPathError),
    /// The file does not exist (no entry matches, ASCII case ignored, D2).
    #[error("file not found")]
    NotFound,
    /// The file could not be read, or is not a regular file.
    #[error("cannot read the file: {0}")]
    Io(String),
    /// Larger than `ui_image::MAX_FILE_BYTES` (D4).
    #[error("file of {len} bytes exceeds the limit of {MAX_FILE_BYTES} bytes")]
    FileTooLarge {
        /// File length.
        len: u64,
    },
    /// The decoder failed (unknown extension, unsupported format, damaged data, D4, D7,
    /// D15, D18).
    #[error(transparent)]
    Decode(#[from] UiImageError),
    /// The decoded-image cache refused the image at its hard cap (D20, UR §4.5). The
    /// caller reports it through [`SkinFileLedger::complete`] like a failed load, so the
    /// element shows the placeholder until the next switch.
    #[error("the decoded-image cache is full")]
    CacheFull,
}

/// A skin image file: its path, cache key and decoder.
///
/// Building one does no file-system work (it runs on the UI thread at activation); the
/// letter-case resolution of D2 happens in [`SkinFile::load_with`] on an asset worker.
/// The file opened is a function of the cache key alone: below the skin folder (or the
/// drive root of a drive-letter path) each component is matched ignoring ASCII case, and
/// when several entries match, the smallest by byte value wins (D2), whatever the
/// spelling in `FileName`. Two `FileName`s that differ only in case therefore share both
/// the key and the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinFile {
    path: PathBuf,
    base: PathBuf,
    fallback_base: Option<PathBuf>,
    components: Vec<String>,
    key: String,
    format: Result<ImageFileFormat, UiImageError>,
}

impl SkinFile {
    /// The path as joined from the skin folder and `FileName` (§6.4), with a drive letter
    /// mapped lexically into the Wine prefix (D12), before the letter-case resolution of
    /// D2.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The cache key: the path with `/` separators, ASCII lower-cased (§6.4 step 4, §6.6).
    /// Two `FileName`s that differ only in letter case share it, and open the same file.
    #[must_use]
    pub fn cache_key(&self) -> &str {
        &self.key
    }

    /// The key of this file in the decoded-image cache.
    #[must_use]
    pub fn image_key(&self) -> ImageKey {
        ImageKey::Path(self.key.clone())
    }

    /// The decoder chosen by the final extension (D7), or why there is none (D15).
    ///
    /// # Errors
    ///
    /// [`UiImageError::UnsupportedFileType`] for an extension other than `.dds`, `.jpg` or
    /// `.tga`.
    pub fn format(&self) -> Result<ImageFileFormat, UiImageError> {
        self.format.clone()
    }

    /// [`SkinFile::resolve_on_disk_with`] with a throw-away directory index.
    #[must_use]
    pub fn resolve_on_disk(&self) -> Option<PathBuf> {
        self.resolve_on_disk_with(&SkinDirIndex::new())
    }

    /// The file on disk (D2): starting at the skin folder (or the mapped drive root), each
    /// component is the entry matching it ignoring ASCII case, the smallest by byte value
    /// when several do; `..` is kept as text (§6.4). Directory listings come from `dirs`
    /// (pass the active overlay's [`SkinOverlay::dir_index`](super::SkinOverlay::dir_index)
    /// so each folder is listed at most once per activation). Read-only; run it on an
    /// asset worker. `None` when a component has no match.
    #[must_use]
    pub fn resolve_on_disk_with(&self, dirs: &SkinDirIndex) -> Option<PathBuf> {
        let mut cur = match &self.fallback_base {
            Some(fallback) if std::fs::metadata(&self.base).is_err() => fallback.clone(),
            _ => self.base.clone(),
        };
        for comp in &self.components {
            if comp == ".." {
                cur.push(comp);
                continue;
            }
            let name = dirs.lookup(&cur, comp)?;
            cur.push(name);
        }
        Some(cur)
    }

    /// [`SkinFile::load_with`] with a throw-away directory index.
    ///
    /// # Errors
    ///
    /// As [`SkinFile::load_with`].
    pub fn load(&self) -> Result<UiImage, SkinImageError> {
        self.load_with(&SkinDirIndex::new())
    }

    /// Reads and decodes the file (run it on an asset worker). Only a regular file is
    /// opened, read-only and without blocking on special files; at most
    /// `ui_image::MAX_FILE_BYTES` bytes are read (D4). The decoded image is cropped to
    /// 4,096 pixels per axis (D25) and tagged with its source format (D26).
    ///
    /// # Errors
    ///
    /// A [`SkinImageError`]; the caller shows the placeholder and records the failure
    /// ([`SkinFileLedger::complete`]).
    pub fn load_with(&self, dirs: &SkinDirIndex) -> Result<UiImage, SkinImageError> {
        let format = self.format.clone()?;
        let path = self
            .resolve_on_disk_with(dirs)
            .ok_or(SkinImageError::NotFound)?;
        let bytes = read_bounded(&path)?;
        Ok(ui_image::decode_as(format, &bytes)?)
    }
}

/// Reads a whole regular file of at most `ui_image::MAX_FILE_BYTES` bytes. The type is
/// checked before opening (a FIFO or device is never opened), the file is opened without
/// blocking where the platform allows it, and the type is checked again on the open file.
fn read_bounded(path: &Path) -> Result<Vec<u8>, SkinImageError> {
    let io = |e: std::io::Error| match e.kind() {
        std::io::ErrorKind::NotFound => SkinImageError::NotFound,
        _ => SkinImageError::Io(e.to_string()),
    };
    let not_regular = || SkinImageError::Io("not a regular file".into());
    let limit = MAX_FILE_BYTES as u64;
    let meta = std::fs::metadata(path).map_err(io)?;
    if !meta.is_file() {
        return Err(not_regular());
    }
    if meta.len() > limit {
        return Err(SkinImageError::FileTooLarge { len: meta.len() });
    }
    let file = open_read_nonblocking(path).map_err(io)?;
    let meta = file.metadata().map_err(io)?;
    if !meta.is_file() {
        return Err(not_regular());
    }
    if meta.len() > limit {
        return Err(SkinImageError::FileTooLarge { len: meta.len() });
    }
    let mut bytes = Vec::new();
    let cap = usize::try_from(meta.len()).unwrap_or(MAX_FILE_BYTES);
    bytes
        .try_reserve(cap)
        .map_err(|_| SkinImageError::Io("out of memory".into()))?;
    file.take(limit + 1).read_to_end(&mut bytes).map_err(io)?;
    if bytes.len() as u64 > limit {
        return Err(SkinImageError::FileTooLarge {
            len: bytes.len() as u64,
        });
    }
    Ok(bytes)
}

/// Opens read-only; on Unix with `O_NONBLOCK`, so that a FIFO swapped in after the type
/// check cannot block the worker (for a regular file the flag changes nothing).
#[cfg(unix)]
fn open_read_nonblocking(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
}

#[cfg(not(unix))]
fn open_read_nonblocking(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::File::open(path)
}

/// Entries listed per folder at most by [`SkinDirIndex`]; a larger folder resolves no
/// component (its files get the placeholder).
pub const MAX_INDEXED_ENTRIES: usize = 65_536;
/// Entries kept by one [`SkinDirIndex`] in total; folders listed beyond it are used once
/// and not kept.
const MAX_INDEX_TOTAL: usize = 1 << 20;

/// One listed folder: ASCII-lower-cased name → the entry with the smallest bytes among
/// those that fold to it. `None` when the folder had too many entries.
type Listing = Option<Arc<HashMap<String, std::ffi::OsString>>>;

#[derive(Debug, Default)]
struct DirIndexInner {
    dirs: HashMap<PathBuf, Listing>,
    entries: usize,
}

/// Folder listings for the letter-case resolution of D2, shared by the asset workers of
/// one activation (cheap to clone; clones share the listings). Each folder is listed at
/// most once while the index lives; a new activation starts a new index, so files added,
/// renamed or edited on disk are seen at the next switch (§6.6). Bounded: at most
/// [`MAX_INDEXED_ENTRIES`] entries per folder and about a million in total.
#[derive(Debug, Clone, Default)]
pub struct SkinDirIndex {
    inner: Arc<Mutex<DirIndexInner>>,
}

impl SkinDirIndex {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of folders listed and kept.
    #[must_use]
    pub fn folders(&self) -> usize {
        self.lock().dirs.len()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, DirIndexInner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The entry of `dir` matching `name` ignoring ASCII case, the smallest by byte value
    /// when several match. When `dir` cannot be listed, an entry spelled exactly `name` is
    /// still found.
    fn lookup(&self, dir: &Path, name: &str) -> Option<std::ffi::OsString> {
        let cached = self.lock().dirs.get(dir).cloned();
        let listing = match cached {
            Some(l) => l,
            None => match list_folder(dir) {
                Ok(l) => {
                    let mut inner = self.lock();
                    let n = l.as_ref().map_or(0, |m| m.len());
                    if inner.entries.saturating_add(n) <= MAX_INDEX_TOTAL {
                        inner.entries += n;
                        inner.dirs.insert(dir.to_path_buf(), l.clone());
                    }
                    l
                }
                Err(_) => {
                    let exact = dir.join(name);
                    return std::fs::symlink_metadata(&exact)
                        .is_ok()
                        .then(|| std::ffi::OsString::from(name));
                }
            },
        };
        listing?.get(&name.to_ascii_lowercase()).cloned()
    }
}

/// Lists `dir` into the folded-name map of [`SkinDirIndex`]; `Ok(None)` for a folder of
/// more than [`MAX_INDEXED_ENTRIES`] entries.
fn list_folder(dir: &Path) -> std::io::Result<Listing> {
    let mut map: HashMap<String, std::ffi::OsString> = HashMap::new();
    for (count, entry) in std::fs::read_dir(dir)?.flatten().enumerate() {
        if count >= MAX_INDEXED_ENTRIES {
            tracing::warn!(folder = %dir.display(), limit = MAX_INDEXED_ENTRIES,
                "skin image folder has too many entries; its files are not resolved");
            return Ok(None);
        }
        let n = entry.file_name();
        let Some(folded) = n.to_str().map(str::to_ascii_lowercase) else {
            continue;
        };
        match map.entry(folded) {
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(n);
            }
            std::collections::hash_map::Entry::Occupied(mut o) => {
                if n.as_encoded_bytes() < o.get().as_encoded_bytes() {
                    o.insert(n);
                }
            }
        }
    }
    Ok(Some(Arc::new(map)))
}

/// A user skin's replacement for one art-asset value (§6): the mapping and its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtReplacement {
    /// The art-asset value.
    pub art_asset: u32,
    /// `FileName` exactly as written.
    pub file_name: String,
    /// 1-based line of the `Mapping`.
    pub line: u32,
    /// The image file, or why the path is unusable (placeholder).
    pub file: Result<SkinFile, SkinPathError>,
}

impl ArtReplacement {
    /// Builds the replacement of `mapping` in the skin folder `folder`. `wine_prefix` maps
    /// drive-letter paths (D12). No file-system work.
    #[must_use]
    pub fn new(mapping: &Mapping, folder: &Path, wine_prefix: Option<&Path>) -> Self {
        Self {
            art_asset: mapping.art_asset,
            file_name: mapping.file_name.clone(),
            line: mapping.line,
            file: skin_file(folder, &mapping.file_name, wine_prefix),
        }
    }

    /// Images are never scaled to the surface they replace (§6.10): this is the identity,
    /// kept as a named rule for callers that ask what size a replacement draws at.
    #[must_use]
    pub fn drawn_size(image: &UiImage) -> (u32, u32) {
        (image.width(), image.height())
    }
}

/// Joins a `FileName` to a skin folder (§6.4, D11, D12) and picks its decoder (D7).
/// Purely lexical: no file-system call. `folder` is taken as it is on disk (discovery
/// gives the real spelling); only the components of `FileName` are resolved, later, by
/// [`SkinFile::load_with`].
///
/// A drive-letter `FileName` replaces the folder and is mapped through `wine_prefix` as
/// `install-layout.md` §5.2 says: `C:` to `<prefix>/drive_c`, any other letter to
/// `<prefix>/dosdevices/<letter>:` (and `Z:`, when that link does not exist, to `/`).
///
/// # Errors
///
/// [`SkinPathError`] for a network path, an unmappable drive-letter path or an over-long
/// path.
pub fn skin_file(
    folder: &Path,
    file_name: &str,
    wine_prefix: Option<&Path>,
) -> Result<SkinFile, SkinPathError> {
    let (base, fallback_base, components) = split_file_name(folder, file_name, wine_prefix)?;
    let mut path = base.clone();
    for c in &components {
        path.push(c);
    }
    let display = path.to_string_lossy().replace('\\', "/");
    let key = display.to_ascii_lowercase();
    if key.len() > MAX_PATH_KEY {
        return Err(SkinPathError::TooLong);
    }
    let format = ui_image::format_for_path(&path, ImageOrigin::Skin);
    Ok(SkinFile {
        path,
        base,
        fallback_base,
        components,
        key,
        format,
    })
}

/// The base folder, an optional fallback base, and the components of §6.4 steps 1–3 with
/// D12: absolute names replace the folder (drive letters through the Wine prefix, network
/// paths refused); relative names are split at `\` and `/` (`..` is kept as text, as the
/// original joins it).
type SplitName = (PathBuf, Option<PathBuf>, Vec<String>);

fn split_file_name(
    folder: &Path,
    file_name: &str,
    wine_prefix: Option<&Path>,
) -> Result<SplitName, SkinPathError> {
    let b = file_name.as_bytes();
    let sep = |c: u8| c == b'\\' || c == b'/';
    if b.len() >= 2 && sep(b[0]) && sep(b[1]) {
        return Err(SkinPathError::NetworkPath);
    }
    let parts = |s: &str| -> Vec<String> {
        s.split(['\\', '/'])
            .filter(|c| !c.is_empty() && *c != ".")
            .map(str::to_owned)
            .collect()
    };
    if file_name.chars().nth(1) == Some(':') {
        let unmapped = || SkinPathError::UnmappedDrive(file_name.to_owned());
        let prefix = wine_prefix.ok_or_else(unmapped)?;
        // The second character is ':', so an ASCII first character makes the rest start
        // at byte 2.
        let letter = b[0];
        if !letter.is_ascii_alphabetic() {
            return Err(unmapped());
        }
        let letter = char::from(letter.to_ascii_lowercase());
        let components = parts(&file_name[2..]);
        if components.iter().any(|c| c == "..") {
            return Err(unmapped());
        }
        let (base, fallback) = match letter {
            'c' => (prefix.join("drive_c"), None),
            'z' => (dosdevices_link(prefix, 'z'), Some(PathBuf::from("/"))),
            other => (dosdevices_link(prefix, other), None),
        };
        return Ok((base, fallback, components));
    }
    Ok((folder.to_path_buf(), None, parts(file_name)))
}

/// `<prefix>/dosdevices/<letter>:` with the link name as a plain path component (lexical,
/// as above). `Path::join("z:")` would not do: on Windows a `z:` component is a drive
/// prefix and replaces the whole path.
fn dosdevices_link(prefix: &Path, letter: char) -> PathBuf {
    let mut link = prefix.join("dosdevices").into_os_string();
    link.push(std::path::MAIN_SEPARATOR_STR);
    link.push(format!("{letter}:"));
    PathBuf::from(link)
}

/// Load state of one skin file in the current activation (D16, D27).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileState {
    /// Scheduled on an asset worker; elements showing it draw nothing meanwhile (D16).
    Pending,
    /// Decoded and handed to the image cache.
    Loaded,
    /// The load failed; the placeholder is shown until the next switch (D27).
    Failed(Arc<str>),
}

/// Identifies one scheduled load (one [`LoadRequest::Schedule`]). Only the ledger that
/// issued it, in the same activation, accepts its outcome; a result that arrives after a
/// switch ([`SkinFileLedger::clear`]) or from another ledger is ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LoadTicket {
    ledger: u64,
    seq: u64,
}

/// What an element asking for a skin file should do now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadRequest {
    /// First request in this activation (or the first after
    /// [`SkinFileLedger::evicted`]): schedule [`SkinFile::load_with`] on an asset worker,
    /// then report the outcome with this ticket to [`SkinFileLedger::complete`]. Draw
    /// nothing meanwhile.
    Schedule(LoadTicket),
    /// Already scheduled: draw nothing until it arrives (D16).
    Pending,
    /// Loaded: take it from the image cache under [`SkinFile::image_key`]. If the cache
    /// no longer holds it (evicted, UR §4.5), call [`SkinFileLedger::evicted`] and request
    /// again.
    Ready,
    /// Failed before (or the path is unusable): draw the placeholder (D27).
    Placeholder,
}

#[derive(Debug)]
struct Slot {
    state: FileState,
    ticket: Option<u64>,
}

static NEXT_LEDGER: AtomicU64 = AtomicU64::new(1);

/// Per-activation record of skin-file loads (D27). Create a new one, or [`clear`] this
/// one, at every skin switch, together with dropping the skin entries of the image cache
/// (§6.6, D20). Outcomes of loads scheduled before the switch are ignored (their
/// [`LoadTicket`]s no longer match).
///
/// The protocol: [`request`] when an element shows a skin file; on
/// [`LoadRequest::Schedule`] run [`SkinFile::load_with`] on a worker, insert the image
/// into the decoded-image cache, and report [`complete`] with `Ok` (or with
/// [`SkinImageError::CacheFull`] when the cache refused it at its hard cap, D20, or with
/// the load error). When [`LoadRequest::Ready`] meets a cache miss, report [`evicted`].
///
/// [`clear`]: SkinFileLedger::clear
/// [`request`]: SkinFileLedger::request
/// [`complete`]: SkinFileLedger::complete
/// [`evicted`]: SkinFileLedger::evicted
#[derive(Debug)]
pub struct SkinFileLedger {
    id: u64,
    next_seq: u64,
    states: HashMap<String, Slot>,
}

impl Default for SkinFileLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl SkinFileLedger {
    /// An empty ledger.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: NEXT_LEDGER.fetch_add(1, AtomicOrdering::Relaxed),
            next_seq: 0,
            states: HashMap::new(),
        }
    }

    /// What to do for an element that shows `replacement` now. Unusable paths fail at the
    /// first request and are logged once (D27).
    pub fn request(&mut self, replacement: &ArtReplacement) -> LoadRequest {
        let file = match &replacement.file {
            Ok(f) => f,
            Err(e) => {
                let key = format!("\0{}", replacement.file_name);
                if let std::collections::hash_map::Entry::Vacant(v) = self.states.entry(key) {
                    tracing::warn!(file = %replacement.file_name, line = replacement.line,
                        reason = %e, "skin image cannot be loaded; showing the placeholder");
                    v.insert(Slot {
                        state: FileState::Failed(e.to_string().into()),
                        ticket: None,
                    });
                }
                return LoadRequest::Placeholder;
            }
        };
        match self.states.get(file.cache_key()) {
            None => {
                let seq = self.next_seq;
                self.next_seq = self.next_seq.wrapping_add(1);
                self.states.insert(
                    file.cache_key().to_owned(),
                    Slot {
                        state: FileState::Pending,
                        ticket: Some(seq),
                    },
                );
                LoadRequest::Schedule(LoadTicket {
                    ledger: self.id,
                    seq,
                })
            }
            Some(slot) => match slot.state {
                FileState::Pending => LoadRequest::Pending,
                FileState::Loaded => LoadRequest::Ready,
                FileState::Failed(_) => LoadRequest::Placeholder,
            },
        }
    }

    /// Records the outcome of the load scheduled with `ticket`. It is accepted only while
    /// `file` is pending under that very ticket in this ledger; a stale outcome (scheduled
    /// before a switch, or by another ledger) is ignored and `false` is returned. A failure
    /// is remembered (no new attempt before the next switch) and logged at warning level
    /// once per path (D27).
    pub fn complete(
        &mut self,
        file: &SkinFile,
        ticket: LoadTicket,
        outcome: Result<(), &SkinImageError>,
    ) -> bool {
        let current = ticket.ledger == self.id
            && self
                .states
                .get(file.cache_key())
                .is_some_and(|s| s.state == FileState::Pending && s.ticket == Some(ticket.seq));
        if !current {
            tracing::debug!(path = %file.path().display(),
                "stale skin image load result ignored");
            return false;
        }
        let state = match outcome {
            Ok(()) => FileState::Loaded,
            Err(e) => {
                tracing::warn!(path = %file.path().display(), reason = %e,
                    "skin image failed to load; showing the placeholder");
                FileState::Failed(e.to_string().into())
            }
        };
        self.states.insert(
            file.cache_key().to_owned(),
            Slot {
                state,
                ticket: None,
            },
        );
        true
    }

    /// The decoded-image cache no longer holds `file` (evicted under UR §4.5): forget that
    /// it was loaded, so the next [`request`](Self::request) schedules it again. Returns
    /// whether the file was loaded. Pending and failed files are left as they are.
    pub fn evicted(&mut self, file: &SkinFile) -> bool {
        let loaded = self
            .states
            .get(file.cache_key())
            .is_some_and(|s| s.state == FileState::Loaded);
        if loaded {
            self.states.remove(file.cache_key());
        }
        loaded
    }

    /// The state of a file, by cache key.
    #[must_use]
    pub fn state(&self, cache_key: &str) -> Option<&FileState> {
        self.states.get(cache_key).map(|s| &s.state)
    }

    /// Forgets every state (a skin switch). Outcomes of loads scheduled before are ignored
    /// from now on.
    pub fn clear(&mut self) {
        self.states.clear();
    }

    /// Number of failed files.
    #[must_use]
    pub fn failures(&self) -> usize {
        self.states
            .values()
            .filter(|s| matches!(s.state, FileState::Failed(_)))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(file: &str) -> Mapping {
        Mapping {
            art_asset: 7,
            name: "a".into(),
            file_name: file.into(),
            line: 3,
        }
    }

    #[test]
    fn join_rules() {
        let folder = Path::new("/s/skins/Mine");
        let f = skin_file(folder, "art\\Sub/Frame.TGA", None).unwrap();
        assert_eq!(f.path(), Path::new("/s/skins/Mine/art/Sub/Frame.TGA"));
        assert_eq!(f.cache_key(), "/s/skins/mine/art/sub/frame.tga");
        assert_eq!(f.format(), Ok(ImageFileFormat::Tga));
        let f = skin_file(folder, "\\x.dds", None).unwrap();
        assert_eq!(f.path(), Path::new("/s/skins/Mine/x.dds"));
        let f = skin_file(folder, "../Other/x.jpg", None).unwrap();
        assert_eq!(f.path(), Path::new("/s/skins/Mine/../Other/x.jpg"));
        assert_eq!(
            skin_file(folder, "\\\\server\\share\\x.tga", None),
            Err(SkinPathError::NetworkPath)
        );
        assert!(matches!(
            skin_file(folder, "C:\\x.tga", None),
            Err(SkinPathError::UnmappedDrive(_))
        ));
        let f = skin_file(folder, "x.iff", None).unwrap();
        assert!(f.format().is_err());
    }

    #[test]
    fn precedence() {
        let mut base = BaseTable::new();
        base.insert(100, 7);
        base.insert(200, 8);
        let mut user = HashMap::new();
        user.insert(
            7,
            ArtReplacement::new(&mapping("x.tga"), Path::new("/f"), None),
        );
        let mut game = GameSkin::new("g", 1);
        game.insert(7, 900);
        game.insert(8, 901);
        assert_eq!(
            resolve_image(100, &base, None, None),
            ImageChoice::Base(100)
        );
        assert!(matches!(
            resolve_image(100, &base, Some(&user), Some(&game)),
            ImageChoice::SkinFile(_)
        ));
        assert_eq!(
            resolve_image(200, &base, Some(&user), Some(&game)),
            ImageChoice::GameSkin {
                did: 901,
                art_asset: 8
            }
        );
        assert_eq!(
            resolve_image(200, &base, Some(&user), None),
            ImageChoice::Base(200)
        );
        assert_eq!(
            resolve_image(300, &base, Some(&user), Some(&game)),
            ImageChoice::Base(300)
        );
    }

    #[test]
    fn drive_letters_are_mapped_lexically() {
        let prefix = Path::new("/pfx");
        let f = skin_file(Path::new("/s"), "C:\\Users\\Pic.TGA", Some(prefix)).unwrap();
        assert_eq!(f.path(), Path::new("/pfx/drive_c/Users/Pic.TGA"));
        assert_eq!(f.cache_key(), "/pfx/drive_c/users/pic.tga");
        let f = skin_file(Path::new("/s"), "z:/usr/x.dds", Some(prefix)).unwrap();
        assert_eq!(f.path(), Path::new("/pfx/dosdevices/z:/usr/x.dds"));
        let f = skin_file(Path::new("/s"), "S:\\a\\b.jpg", Some(prefix)).unwrap();
        assert_eq!(f.path(), Path::new("/pfx/dosdevices/s:/a/b.jpg"));
        for bad in ["C:\\a\\..\\b.tga", "1:\\x.tga", "\u{e9}:\\x.tga"] {
            assert!(
                matches!(
                    skin_file(Path::new("/s"), bad, Some(prefix)),
                    Err(SkinPathError::UnmappedDrive(_))
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn ledger_remembers_failures() {
        let r = ArtReplacement::new(&mapping("missing.tga"), Path::new("/nonexistent"), None);
        let mut l = SkinFileLedger::new();
        let LoadRequest::Schedule(t) = l.request(&r) else {
            panic!("first request schedules")
        };
        assert_eq!(l.request(&r), LoadRequest::Pending);
        let file = r.file.as_ref().unwrap();
        let err = file.load().unwrap_err();
        assert_eq!(err, SkinImageError::NotFound);
        assert!(l.complete(file, t, Err(&err)));
        assert_eq!(l.request(&r), LoadRequest::Placeholder);
        assert_eq!(l.failures(), 1);
        // A second outcome under the same ticket is not accepted.
        assert!(!l.complete(file, t, Ok(())));
        assert_eq!(l.request(&r), LoadRequest::Placeholder);
        l.clear();
        assert!(matches!(l.request(&r), LoadRequest::Schedule(_)));
        let bad = ArtReplacement::new(&mapping("\\\\n\\x.tga"), Path::new("/f"), None);
        assert_eq!(l.request(&bad), LoadRequest::Placeholder);
    }

    #[test]
    fn stale_outcomes_are_ignored() {
        let r = ArtReplacement::new(&mapping("x.tga"), Path::new("/nonexistent"), None);
        let file = r.file.as_ref().unwrap();
        let mut l = SkinFileLedger::new();
        let LoadRequest::Schedule(old) = l.request(&r) else {
            panic!()
        };
        l.clear();
        // A failure from before the switch does not fail the new activation...
        assert!(!l.complete(file, old, Err(&SkinImageError::NotFound)));
        let LoadRequest::Schedule(new) = l.request(&r) else {
            panic!("the file is scheduled anew after a switch")
        };
        // ...and a success from before the switch does not make it ready.
        assert!(!l.complete(file, old, Ok(())));
        assert_eq!(l.request(&r), LoadRequest::Pending);
        // A ticket of another ledger is refused too.
        let mut other = SkinFileLedger::new();
        let LoadRequest::Schedule(foreign) = other.request(&r) else {
            panic!()
        };
        assert!(!l.complete(file, foreign, Ok(())));
        assert!(l.complete(file, new, Ok(())));
        assert_eq!(l.request(&r), LoadRequest::Ready);
    }

    #[test]
    fn eviction_and_cache_refusal() {
        let r = ArtReplacement::new(&mapping("x.tga"), Path::new("/nonexistent"), None);
        let file = r.file.as_ref().unwrap();
        let mut l = SkinFileLedger::new();
        let LoadRequest::Schedule(t) = l.request(&r) else {
            panic!()
        };
        assert!(!l.evicted(file), "a pending file is not evicted");
        assert!(l.complete(file, t, Ok(())));
        assert_eq!(l.request(&r), LoadRequest::Ready);
        assert!(l.evicted(file));
        let LoadRequest::Schedule(t) = l.request(&r) else {
            panic!("an evicted file is scheduled again")
        };
        assert!(l.complete(file, t, Err(&SkinImageError::CacheFull)));
        assert_eq!(l.request(&r), LoadRequest::Placeholder);
        assert!(!l.evicted(file), "a failed file stays failed");
        assert_eq!(l.request(&r), LoadRequest::Placeholder);
    }
}
