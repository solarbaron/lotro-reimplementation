// SPDX-License-Identifier: GPL-3.0-or-later
//! Background data loader: install discovery, archive-set opening, statistics and the
//! showcase texture.
//!
//! Everything here runs on one worker thread so the window appears immediately. Results
//! reach the main thread as [`LoaderEvent`]s over a channel. The install is only ever
//! read.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tessera_dat::{ArchiveSet, SetMember, SetOptions};
use tessera_formats::{Record, Surface};
use tessera_install::{GameInstall, SearchEnv};

/// First DID of the render-surface class.
pub const SURFACE_DID_START: u32 = 0x4100_0000;
/// One past the last DID of the render-surface class.
pub const SURFACE_DID_END: u32 = 0x4200_0000;
/// Directory entries examined by the automatic surface pick before giving up.
pub const MAX_SURFACE_ATTEMPTS: usize = 4096;
/// Smallest edge accepted by the automatic pick (tiny mask/LUT surfaces make a poor demo).
pub const MIN_AUTO_EDGE: u32 = 16;
/// Largest edge accepted by the automatic pick.
pub const MAX_AUTO_EDGE: u32 = 4096;

/// What the loader should do.
#[derive(Debug, Clone, Default)]
pub struct LoaderConfig {
    /// `--game-dir`.
    pub game_dir: Option<PathBuf>,
    /// `--no-data`: skip discovery entirely.
    pub no_data: bool,
    /// `--surface`: show this DID instead of the automatic pick.
    pub surface: Option<u32>,
    /// Walk every directory tree to count entries.
    pub count_entries: bool,
}

/// A surface decoded to RGBA8, ready for upload.
#[derive(Clone)]
pub struct DecodedTexture {
    /// Surface DID.
    pub did: u32,
    /// Pixel format name of the stored surface.
    pub format: &'static str,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for DecodedTexture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecodedTexture")
            .field("did", &format_args!("{:#010x}", self.did))
            .field("format", &self.format)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

/// Totals over an archive set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ArchiveStats {
    /// Archives opened.
    pub archives: usize,
    /// Sum of file sizes in bytes.
    pub bytes: u64,
    /// Directory entries (when counted).
    pub entries: Option<u64>,
    /// Non-fatal problems reported while opening the set.
    pub problems: usize,
}

/// Progress reported by the loader thread.
#[derive(Debug)]
pub enum LoaderEvent {
    /// No install is used: no-data mode.
    NoData {
        /// Why.
        reason: String,
    },
    /// The archive set is open (sent before any texture or statistics work).
    Opened {
        /// The install directory.
        root: PathBuf,
        /// The set, shared with the main thread.
        set: Arc<ArchiveSet>,
        /// Totals; `entries` is filled in later by [`LoaderEvent::EntriesCounted`].
        stats: ArchiveStats,
    },
    /// Every directory tree was walked.
    EntriesCounted(u64),
    /// The showcase texture was decoded.
    Texture(DecodedTexture),
    /// No texture could be decoded; the procedural one stays.
    NoTexture {
        /// Why.
        reason: String,
    },
    /// The loader finished (always the last event).
    Finished,
}

/// Handle on the loader thread. Dropping it cancels the work and joins the thread.
#[derive(Debug)]
pub struct DataLoader {
    rx: Receiver<LoaderEvent>,
    cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    finished: bool,
}

impl DataLoader {
    /// Start the loader thread.
    pub fn spawn(config: LoaderConfig) -> std::io::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        let handle = std::thread::Builder::new()
            .name("tessera-data".into())
            .spawn(move || {
                let started = Instant::now();
                let ctx = Ctx { tx, cancel: flag };
                run(&config, &ctx);
                tracing::info!(
                    elapsed_ms = started.elapsed().as_millis(),
                    "data loader done"
                );
                ctx.send(LoaderEvent::Finished);
            })?;
        Ok(Self {
            rx,
            cancel,
            handle: Some(handle),
            finished: false,
        })
    }

    /// Whether [`LoaderEvent::Finished`] has been received.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Next event without blocking.
    pub fn try_next(&mut self) -> Option<LoaderEvent> {
        if self.finished {
            return None;
        }
        match self.rx.try_recv() {
            Ok(ev) => Some(self.note(ev)),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.finished = true;
                None
            }
        }
    }

    /// Next event, waiting at most `timeout`.
    pub fn next_timeout(&mut self, timeout: Duration) -> Option<LoaderEvent> {
        if self.finished {
            return None;
        }
        match self.rx.recv_timeout(timeout) {
            Ok(ev) => Some(self.note(ev)),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => {
                self.finished = true;
                None
            }
        }
    }

    fn note(&mut self, ev: LoaderEvent) -> LoaderEvent {
        if matches!(ev, LoaderEvent::Finished) {
            self.finished = true;
        }
        ev
    }

    /// Ask the thread to stop and wait for it.
    pub fn shutdown(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take()
            && h.join().is_err()
        {
            tracing::error!("data loader thread panicked");
        }
    }
}

impl Drop for DataLoader {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct Ctx {
    tx: Sender<LoaderEvent>,
    cancel: Arc<AtomicBool>,
}

impl Ctx {
    fn send(&self, ev: LoaderEvent) {
        // The receiver is gone only during shutdown; nothing to do then.
        let _ = self.tx.send(ev);
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

fn run(config: &LoaderConfig, ctx: &Ctx) {
    if config.no_data {
        ctx.send(LoaderEvent::NoData {
            reason: "--no-data given".into(),
        });
        return;
    }
    let install = match find_install(config.game_dir.as_deref()) {
        Ok(i) => i,
        Err(reason) => {
            tracing::warn!(
                "{reason}; running in no-data mode (set {} or pass --game-dir)",
                tessera_install::GAME_DIR_ENV
            );
            ctx.send(LoaderEvent::NoData { reason });
            return;
        }
    };
    log_install(&install);
    if ctx.cancelled() {
        return;
    }

    let started = Instant::now();
    let set = match ArchiveSet::open(&install.root, SetOptions::default()) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            let reason = format!("cannot open the archive set: {e}");
            tracing::error!("{reason}");
            ctx.send(LoaderEvent::NoData { reason });
            return;
        }
    };
    let stats = ArchiveStats {
        archives: set.members().len(),
        bytes: set.members().iter().map(|m| m.archive.file_len()).sum(),
        entries: None,
        problems: set.problems().len(),
    };
    for p in set.problems() {
        tracing::warn!(
            path = %p.path.as_deref().map(Path::display).map(|d| d.to_string()).unwrap_or_default(),
            "archive set problem: {}",
            p.message
        );
    }
    tracing::info!(
        archives = stats.archives,
        total_mib = stats.bytes / (1024 * 1024),
        elapsed_ms = started.elapsed().as_millis(),
        "archive set opened"
    );
    ctx.send(LoaderEvent::Opened {
        root: install.root.clone(),
        set: Arc::clone(&set),
        stats,
    });

    // Texture first so the picture appears before the (slower) entry count.
    let texture = match config.surface {
        Some(did) => decode_did(&set, did),
        None => find_first_surface(&set, ctx),
    };
    match texture {
        Ok(t) => {
            tracing::info!(
                did = format_args!("{:#010x}", t.did),
                format = t.format,
                width = t.width,
                height = t.height,
                "showcase texture decoded"
            );
            ctx.send(LoaderEvent::Texture(t));
        }
        Err(reason) => {
            tracing::warn!("no showcase texture: {reason}");
            ctx.send(LoaderEvent::NoTexture { reason });
        }
    }

    if config.count_entries && !ctx.cancelled() {
        if let Some(n) = log_member_stats(&set, ctx) {
            ctx.send(LoaderEvent::EntriesCounted(n));
        }
    } else {
        for m in set.members() {
            log_member(m, None);
        }
    }
}

fn find_install(explicit: Option<&Path>) -> Result<GameInstall, String> {
    let env = SearchEnv::from_process();
    let discovery = tessera_install::discover_with(&env, explicit)
        .map_err(|e| format!("install not usable: {e}"))?;
    for r in &discovery.rejected {
        let problems: Vec<String> = r.problems.iter().map(ToString::to_string).collect();
        tracing::info!(path = %r.path.display(), "rejected install candidate: {}", problems.join("; "));
    }
    discovery
        .installs
        .into_iter()
        .next()
        .ok_or_else(|| "no install found".to_owned())
}

fn log_install(install: &GameInstall) {
    tracing::info!(
        root = %install.root.display(),
        archives = install.archives.len(),
        languages = %install.languages.join(","),
        client_dir = %install.client_dir.as_deref().map(|p| p.display().to_string()).unwrap_or_else(|| "-".into()),
        incomplete = install.incomplete,
        sources = ?install.sources,
        "install found"
    );
    tracing::debug!(project_version = ?install.project_version, game_type = ?install.game_type, "install identity");
    for w in &install.warnings {
        tracing::warn!("install: {w}");
    }
}

fn log_member(m: &SetMember, entries: Option<u64>) {
    let h = m.archive.header();
    tracing::info!(
        archive = %m.id,
        size_mib = m.archive.file_len() / (1024 * 1024),
        engine = h.engine_version,
        data_version = h.game_data_version,
        free_kib = h.free_bytes / 1024,
        entries = %entries.map_or_else(|| "-".to_owned(), |n| n.to_string()),
        "archive"
    );
}

/// Walks every directory tree; returns the total, or `None` when cancelled.
fn log_member_stats(set: &ArchiveSet, ctx: &Ctx) -> Option<u64> {
    let started = Instant::now();
    let mut total = 0u64;
    for m in set.members() {
        let mut n = 0u64;
        for e in m.archive.entries() {
            if ctx.cancelled() {
                return None;
            }
            match e {
                Ok(_) => n += 1,
                Err(err) => {
                    tracing::warn!(archive = %m.id, "directory walk stopped: {err}");
                    break;
                }
            }
        }
        log_member(m, Some(n));
        total += n;
    }
    tracing::info!(
        entries = total,
        elapsed_ms = started.elapsed().as_millis(),
        "directory entries counted"
    );
    Some(total)
}

fn decode_payload(did: u32, bytes: &[u8]) -> Result<DecodedTexture, String> {
    let surface = Surface::parse_expecting(bytes, did).map_err(|e| e.to_string())?;
    let image = surface.to_rgba8().map_err(|e| e.to_string())?;
    Ok(DecodedTexture {
        did,
        format: surface.format.name(),
        width: image.width,
        height: image.height,
        rgba: image.pixels,
    })
}

/// Decode one explicitly requested surface.
fn decode_did(set: &ArchiveSet, did: u32) -> Result<DecodedTexture, String> {
    let (located, bytes) = set
        .read_with_entry(did)
        .map_err(|e| format!("{did:#010x}: {e}"))?;
    if let Some(m) = set.archive_of(&located) {
        tracing::debug!(did = format_args!("{did:#010x}"), archive = %m.id, "surface located");
    }
    decode_payload(did, &bytes).map_err(|e| format!("{did:#010x}: {e}"))
}

/// The first render surface (routed archives first) that decodes and has a sensible size.
fn find_first_surface(set: &ArchiveSet, ctx: &Ctx) -> Result<DecodedTexture, String> {
    let routed = set.candidates(SURFACE_DID_START);
    let rest = (0..set.members().len()).filter(|i| !routed.contains(i));
    let order: Vec<usize> = routed.iter().copied().chain(rest).collect();
    let mut attempts = 0usize;
    let mut last_err = String::from("no render surfaces in any archive");
    for i in order {
        let Some(m) = set.get(i) else { continue };
        for entry in m.archive.entries_range(SURFACE_DID_START..SURFACE_DID_END) {
            if ctx.cancelled() {
                return Err("cancelled".into());
            }
            attempts += 1;
            if attempts > MAX_SURFACE_ATTEMPTS {
                return Err(format!(
                    "none of the first {MAX_SURFACE_ATTEMPTS} surfaces decoded (last error: {last_err})"
                ));
            }
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    last_err = format!("{}: directory walk failed: {e}", m.id);
                    break;
                }
            };
            let bytes = match m.archive.read_entry(&entry) {
                Ok(b) => b,
                Err(e) => {
                    last_err = format!("{:#010x}: {e}", entry.did);
                    continue;
                }
            };
            match decode_payload(entry.did, &bytes) {
                Ok(t) if acceptable_auto_size(t.width, t.height) => {
                    tracing::debug!(archive = %m.id, attempts, "surface picked");
                    return Ok(t);
                }
                Ok(t) => {
                    tracing::trace!(
                        did = format_args!("{:#010x}", t.did),
                        t.width,
                        t.height,
                        "surface skipped (size)"
                    );
                }
                Err(e) => {
                    tracing::trace!(
                        did = format_args!("{:#010x}", entry.did),
                        "surface skipped: {e}"
                    );
                    last_err = format!("{:#010x}: {e}", entry.did);
                }
            }
        }
    }
    Err(last_err)
}

/// Size filter for the automatic pick.
#[must_use]
pub fn acceptable_auto_size(width: u32, height: u32) -> bool {
    (MIN_AUTO_EDGE..=MAX_AUTO_EDGE).contains(&width)
        && (MIN_AUTO_EDGE..=MAX_AUTO_EDGE).contains(&height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_size_filter() {
        assert!(acceptable_auto_size(256, 256));
        assert!(acceptable_auto_size(16, 4096));
        assert!(!acceptable_auto_size(1, 1));
        assert!(!acceptable_auto_size(8192, 16));
    }

    #[test]
    fn no_data_mode_reports_and_finishes() {
        let mut loader = DataLoader::spawn(LoaderConfig {
            no_data: true,
            ..LoaderConfig::default()
        })
        .expect("spawn");
        let mut saw_no_data = false;
        while let Some(ev) = loader.next_timeout(Duration::from_secs(10)) {
            match ev {
                LoaderEvent::NoData { .. } => saw_no_data = true,
                LoaderEvent::Finished => break,
                other => panic!("unexpected event {other:?}"),
            }
        }
        assert!(saw_no_data);
        assert!(loader.is_finished());
    }

    #[test]
    fn missing_game_dir_falls_back_to_no_data() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut loader = DataLoader::spawn(LoaderConfig {
            game_dir: Some(dir.path().to_owned()),
            ..LoaderConfig::default()
        })
        .expect("spawn");
        let first = loader.next_timeout(Duration::from_secs(10));
        assert!(
            matches!(first, Some(LoaderEvent::NoData { .. })),
            "{first:?}"
        );
    }
}
