// SPDX-License-Identifier: GPL-3.0-or-later
//! The client's [`UiEnv`]: layouts, images, text and strings for `tessera-ui`, answered at
//! once from what is loaded (ARCHITECTURE §21: no blocking I/O on the main thread).
//!
//! - **Layouts** come from the [`UiData`] built on the start-up thread, which decodes every
//!   layout of the `UILAYOUT` map before the UI attaches. A layout reached only through a
//!   base reference outside that map is decoded on first use (a counted, logged exception;
//!   none was seen on the reference install).
//! - **Images** (render surfaces by DID, user-skin files, plugin and Image-item files) are
//!   decoded on the UI's asset workers with `tessera-material::ui_image`; until one arrives
//!   it is [`ImageState::Pending`] and draws nothing (decision 0015 Decision 1.8). A failed
//!   image becomes the placeholder (`skin-definition.md` D16). Decoded images live in a
//!   cache whose unreferenced entries are evicted past the LRU budget or near the entry
//!   cap [`MAX_IMAGE_ENTRIES`] (failed entries too, under the entry cap); images that a
//!   canvas still references are pinned, and past the pinned cap new images get the
//!   placeholder (decision 0015 §4.5, U5). The byte total is kept as a running sum and the
//!   pinned bytes are sampled at most once per frame, so a request and the per-frame
//!   figures cost O(1) whatever the cache size.
//! - **Fonts** are decoded on the workers into [`FontFace`]s (`fonts.md` §4.1); text is
//!   shaped on the main thread by [`super::text`] once its fonts are there. At most
//!   [`MAX_FONT_ENTRIES`] distinct font DIDs are tracked; past that a new DID does not
//!   resolve and its slot falls back (`fonts.md` §9.1 rule 4).
//! - **Strings** resolve through `tessera-text` (`string-table.md` §9). The start-up thread
//!   loads every string table the decoded layouts reference ([`super::boot`]); a table
//!   loaded later on the main thread is counted ([`EnvStats::late_string_tables`]) and
//!   logged, like a late layout.
//!
//! The environment shares an [`EnvState`] with [`super::UiHost`] through [`EnvHandle`];
//! the host drains worker results once per frame and tells the UI what arrived.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, PoisonError};

use tessera_assets::{CancelToken, PoolConfig, Priority, WorkerPool};
use tessera_dat::ArchiveSet;
use tessera_formats::did::Did;
use tessera_formats::string_info::StringInfo;
use tessera_formats::{PropertyDictionary, Record, Surface, UiLayout};
use tessera_material::ui_image::{self, ImageOrigin, UiImage};
use tessera_ui::env::{ImageRef, ImageState, PreparedText, TextRequest, UiEnv};
use tessera_ui::skin::{LoadRequest, LoadTicket, SkinFile, SkinFileLedger, SkinOverlay};
use tessera_ui::text::{FontFace, TesseraFont};

use super::text::{TextPids, TextShaper};

/// Most images kept in the cache map (ready, pending and failed together); requests past
/// it get the placeholder (Tessera bound against hostile plugin paths).
pub const MAX_IMAGE_ENTRIES: usize = 16_384;
/// Most image or font jobs in flight on the UI workers.
pub const MAX_JOBS_IN_FLIGHT: usize = 256;
/// Entries kept free below [`MAX_IMAGE_ENTRIES`] by eviction, so that a frame's new
/// requests find room.
pub const IMAGE_ENTRY_HEADROOM: usize = 1024;
/// Most distinct font DIDs tracked (ready, pending and failed together); Tessera bound
/// against hostile plugin font IDs.
pub const MAX_FONT_ENTRIES: usize = 1024;
/// Caches up to this many entries are walked every frame for exact U5 figures; larger
/// ones only when eviction may be needed, or every [`PINNED_SAMPLE_PERIOD`] frames.
pub const SMALL_IMAGE_CACHE: usize = 1024;
/// Frames between two samples of the pinned bytes of a large cache.
pub const PINNED_SAMPLE_PERIOD: u64 = 32;
/// Most LRU keys looked at by one request that needs room under the entry cap.
const MAX_EVICT_PROBES: usize = 64;
/// Size of Tessera's fallback font in pixels when the install gives no locale font
/// (`fonts.md` §11).
pub const FALLBACK_FONT_PX: u32 = 14;

/// The install side of the UI environment, built off the main thread (see
/// [`super::boot`]). Everything is read-only and shared.
pub struct UiData {
    /// The archive set.
    pub set: Arc<ArchiveSet>,
    /// The property dictionary (the schema of layout records).
    pub dict: Arc<PropertyDictionary>,
    /// Decoded layouts by DID (`None` = missing or malformed).
    pub layouts: Mutex<HashMap<u32, Option<Arc<UiLayout>>>>,
    /// `UILAYOUT` keys → layout DIDs (`ui-layout.md` §3.2).
    pub layout_keys: HashMap<u32, u32>,
    /// `UICore_Element_InputMapID` values → InputMap names (`keymap.md` §12.5.4).
    pub input_maps: HashMap<u32, String>,
    /// String access over the language archive.
    pub strings: Option<tessera_text::Strings>,
    /// The locale default font (`fonts.md` §9.1), when the locale record names one.
    pub locale_font: Option<u32>,
    /// The locale leading factor *f*₀ (`fonts.md` §7.1); 0 without a locale record.
    pub locale_leading: f32,
    /// The install directory (Image-item paths, `ui-layout.md` §6.2).
    pub game_dir: PathBuf,
}

impl std::fmt::Debug for UiData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiData")
            .field("layout_keys", &self.layout_keys.len())
            .field("input_maps", &self.input_maps.len())
            .field("locale_font", &self.locale_font)
            .finish_non_exhaustive()
    }
}

impl UiData {
    /// A layout, decoding it now when the start-up pass did not.
    #[must_use]
    pub fn layout(&self, did: u32) -> (Option<Arc<UiLayout>>, bool) {
        let mut cache = self.layouts.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(l) = cache.get(&did) {
            return (l.clone(), false);
        }
        let l = decode_layout(&self.set, &self.dict, did);
        cache.insert(did, l.clone());
        (l, true)
    }
}

/// Decodes one layout record.
#[must_use]
pub fn decode_layout(
    set: &ArchiveSet,
    dict: &PropertyDictionary,
    did: u32,
) -> Option<Arc<UiLayout>> {
    let payload = set.read(did).ok()?;
    UiLayout::parse_expecting(&payload, did, dict)
        .ok()
        .map(Arc::new)
}

/// A finished worker job.
enum Done {
    Image(ImageRef, Result<UiImage, String>),
    Skin(LoadTicket, SkinFile, ImageRef, Result<UiImage, String>),
    Font(u32, Result<FontFace, String>),
}

#[derive(Debug, Clone)]
enum Slot {
    Ready(Arc<UiImage>),
    Pending,
    Failed,
}

#[derive(Debug, Clone)]
enum FontSlot {
    Ready(Arc<FontFace>),
    Pending,
    Failed,
}

/// Figures of the image and font stores (diagnostics, U5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnvStats {
    /// Images decoded.
    pub images_decoded: u64,
    /// Images that failed (placeholder).
    pub images_failed: u64,
    /// Images evicted from the LRU.
    pub images_evicted: u64,
    /// Requests answered with the placeholder because a cap was reached.
    pub capped: u64,
    /// Bytes of decoded images held by the cache.
    pub image_bytes: u64,
    /// Bytes of those images that something else still references (pinned).
    pub pinned_bytes: u64,
    /// Fonts decoded.
    pub fonts_decoded: u64,
    /// Fonts that failed.
    pub fonts_failed: u64,
    /// Layouts decoded on the main thread (not preloaded).
    pub late_layouts: u64,
    /// String tables loaded on the main thread (not preloaded by the start-up thread).
    pub late_string_tables: u64,
    /// Font requests refused because [`MAX_FONT_ENTRIES`] DIDs are tracked.
    pub fonts_capped: u64,
    /// Jobs in flight.
    pub in_flight: u64,
}

/// The state shared by the environment and the host.
pub struct EnvState {
    data: Option<Arc<UiData>>,
    pool: WorkerPool,
    tx: Sender<Done>,
    rx: Receiver<Done>,
    images: HashMap<ImageRef, Slot>,
    lru: VecDeque<ImageRef>,
    lru_budget: u64,
    pinned_cap: u64,
    /// Bytes of every ready image in the map (running sum).
    ready_bytes: u64,
    /// Pinned bytes at the last walk of the map.
    pinned_sample: u64,
    polls: u64,
    placeholder: Arc<UiImage>,
    skin: Option<Arc<SkinOverlay>>,
    ledger: SkinFileLedger,
    plugins_dir: Option<PathBuf>,
    fonts: HashMap<u32, FontSlot>,
    fallback_font: Option<Arc<FontFace>>,
    shaper: TextShaper,
    images_arrived: bool,
    fonts_arrived: bool,
    in_flight: usize,
    rng: u64,
    stats: EnvStats,
}

impl std::fmt::Debug for EnvState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnvState")
            .field("data", &self.data)
            .field("images", &self.images.len())
            .field("fonts", &self.fonts.len())
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

/// The host's handle on the environment's state.
pub type EnvHandle = Rc<RefCell<EnvState>>;

/// What arrived since the last [`EnvState::poll`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Arrivals {
    /// At least one image (or placeholder) became ready.
    pub images: bool,
    /// At least one font became ready.
    pub fonts: bool,
}

impl EnvState {
    /// A state over `data` (or none: no install, only plugin files and the fallback font),
    /// with `workers` UI asset workers and the U5 image budgets in bytes.
    #[must_use]
    pub fn new(
        data: Option<Arc<UiData>>,
        workers: usize,
        lru_budget: u64,
        pinned_cap: u64,
    ) -> Self {
        let (tx, rx) = channel();
        let pool = WorkerPool::new(PoolConfig {
            workers: workers.max(1),
            max_queued: 4096,
            thread_name: "tessera-ui".to_owned(),
        });
        let pids = data
            .as_ref()
            .map(|d| TextPids::resolve(&d.dict))
            .unwrap_or_default();
        Self {
            data,
            pool,
            tx,
            rx,
            images: HashMap::new(),
            lru: VecDeque::new(),
            lru_budget,
            pinned_cap,
            ready_bytes: 0,
            pinned_sample: 0,
            polls: 0,
            placeholder: Arc::new(ui_image::placeholder()),
            skin: None,
            ledger: SkinFileLedger::new(),
            plugins_dir: None,
            fonts: HashMap::new(),
            fallback_font: None,
            shaper: TextShaper::new(pids),
            images_arrived: false,
            fonts_arrived: false,
            in_flight: 0,
            rng: 0x2545_F491_4F6C_DD1D,
            stats: EnvStats::default(),
        }
    }

    /// The install data, when there is an install.
    #[must_use]
    pub fn data(&self) -> Option<&Arc<UiData>> {
        self.data.as_ref()
    }

    /// The text of a StringInfo value (`string-table.md`): a literal as it is, else rendered
    /// through the install's string tables; a table loaded here, on the main thread, is
    /// counted ([`EnvStats::late_string_tables`]).
    pub(crate) fn resolve_string(&mut self, info: &StringInfo) -> Option<String> {
        if let tessera_formats::string_info::StringSource::Literal(s) = &info.source {
            return Some(s.to_string());
        }
        let data = self.data.clone()?;
        let strings = data.strings.as_ref()?;
        let before = strings.cache_stats().misses;
        let r = strings.resolve(
            info,
            &tessera_text::Values::new(),
            &tessera_text::RenderOptions::default(),
        );
        let loaded = strings.cache_stats().misses.saturating_sub(before);
        if loaded > 0 {
            // ARCHITECTURE §21: a table the start-up thread did not load was read here, on
            // the main thread. Counted and logged, like a late layout.
            self.stats.late_string_tables += loaded;
            tracing::debug!(tables = loaded, "string table loaded on the main thread");
        }
        Some(r.plain_text())
    }

    /// The UI's worker pool (the plugin file index walk runs here too).
    #[must_use]
    pub fn pool(&self) -> &WorkerPool {
        &self.pool
    }

    /// Text property IDs (also handed to the plugin UI layer).
    #[must_use]
    pub fn text_pids(&self) -> TextPids {
        self.shaper.pids()
    }

    /// Sets the `Plugins/` folder plugin image paths are relative to.
    pub fn set_plugins_dir(&mut self, dir: Option<PathBuf>) {
        self.plugins_dir = dir;
    }

    /// Activates a skin overlay (or none): the skin images are dropped from the cache and
    /// the ledger starts anew (`skin-definition.md` §6.6, D20). The caller then calls
    /// `Ui::set_skin`, which re-sets every base image.
    pub fn set_skin(&mut self, skin: Option<Arc<SkinOverlay>>) {
        self.skin = skin;
        self.ledger.clear();
        self.images
            .retain(|k, _| !matches!(k, ImageRef::Skin { .. }));
        self.lru.retain(|k| !matches!(k, ImageRef::Skin { .. }));
        let _ = self.walk();
    }

    /// Figures. O(1): the image bytes are a running sum and the pinned bytes the last
    /// sample (module docs).
    #[must_use]
    pub fn stats(&self) -> EnvStats {
        let mut s = self.stats;
        s.in_flight = self.in_flight as u64;
        s.image_bytes = self.ready_bytes;
        s.pinned_bytes = self.pinned_sample;
        s
    }

    /// Entries in the image map (ready, pending and failed).
    #[must_use]
    pub fn image_entries(&self) -> usize {
        self.images.len()
    }

    /// Entries in the font map.
    #[must_use]
    pub fn font_entries(&self) -> usize {
        self.fonts.len()
    }

    /// Recomputes the byte total and samples the pinned bytes; returns the unpinned bytes.
    fn walk(&mut self) -> u64 {
        let (mut bytes, mut pinned) = (0u64, 0u64);
        for slot in self.images.values() {
            if let Slot::Ready(i) = slot {
                let b = i.byte_size() as u64;
                bytes = bytes.saturating_add(b);
                if Arc::strong_count(i) > 1 {
                    pinned = pinned.saturating_add(b);
                }
            }
        }
        self.ready_bytes = bytes;
        self.pinned_sample = pinned;
        bytes.saturating_sub(pinned)
    }

    /// Puts `slot` under `key`, keeping the byte total.
    fn put(&mut self, key: ImageRef, slot: Slot) {
        if let Slot::Ready(i) = &slot {
            self.ready_bytes = self.ready_bytes.saturating_add(i.byte_size() as u64);
        }
        if let Some(Slot::Ready(old)) = self.images.insert(key, slot) {
            self.ready_bytes = self.ready_bytes.saturating_sub(old.byte_size() as u64);
        }
    }

    /// Removes `key`, keeping the byte total.
    fn remove(&mut self, key: &ImageRef) {
        if let Some(Slot::Ready(old)) = self.images.remove(key) {
            self.ready_bytes = self.ready_bytes.saturating_sub(old.byte_size() as u64);
        }
    }

    /// The entry count eviction keeps the map at.
    fn entry_target() -> usize {
        MAX_IMAGE_ENTRIES - IMAGE_ENTRY_HEADROOM
    }

    /// Whether jobs are still running.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.in_flight > 0
    }

    /// Drains finished worker jobs (once per frame, step 5/7). Never blocks.
    pub fn poll(&mut self) -> Arrivals {
        while let Ok(done) = self.rx.try_recv() {
            self.in_flight = self.in_flight.saturating_sub(1);
            match done {
                Done::Image(key, r) => {
                    let slot = match r {
                        Ok(img) => {
                            self.stats.images_decoded += 1;
                            Slot::Ready(Arc::new(img))
                        }
                        Err(e) => {
                            self.stats.images_failed += 1;
                            tracing::debug!(image = ?key, "UI image failed: {e}");
                            Slot::Failed
                        }
                    };
                    if self.images.contains_key(&key) {
                        self.put(key.clone(), slot);
                        self.lru.push_back(key);
                        self.images_arrived = true;
                    }
                }
                Done::Skin(ticket, file, key, r) => {
                    let outcome = r.as_ref().map(|_| ()).map_err(|e| e.clone());
                    let err = outcome
                        .as_ref()
                        .err()
                        .map(|e| tessera_ui::skin::SkinImageError::Io(e.clone()));
                    let current =
                        self.ledger
                            .complete(&file, ticket, err.as_ref().map_or(Ok(()), Err));
                    if current {
                        let slot = match r {
                            Ok(img) => {
                                self.stats.images_decoded += 1;
                                Slot::Ready(Arc::new(img))
                            }
                            // The placeholder is not stored as a ready image: it is
                            // not cached bytes and must never count as pinned.
                            Err(_) => {
                                self.stats.images_failed += 1;
                                Slot::Failed
                            }
                        };
                        self.put(key.clone(), slot);
                        self.lru.push_back(key);
                        self.images_arrived = true;
                    }
                }
                Done::Font(did, r) => {
                    let f = match r {
                        Ok(face) => {
                            self.stats.fonts_decoded += 1;
                            FontSlot::Ready(Arc::new(face))
                        }
                        Err(e) => {
                            self.stats.fonts_failed += 1;
                            tracing::warn!(
                                font = format_args!("{did:#010x}"),
                                "font not loaded: {e}"
                            );
                            FontSlot::Failed
                        }
                    };
                    self.fonts.insert(did, f);
                    self.fonts_arrived = true;
                }
            }
        }
        self.polls = self.polls.wrapping_add(1);
        // Unpinned bytes estimated from the last pinned sample; images unpinned since then
        // are found by the periodic sample.
        let over_bytes = self.ready_bytes.saturating_sub(self.pinned_sample) > self.lru_budget;
        let over_count = self.images.len() > Self::entry_target();
        if over_bytes
            || over_count
            || self.images.len() <= SMALL_IMAGE_CACHE
            || self.polls.is_multiple_of(PINNED_SAMPLE_PERIOD)
        {
            let unpinned = self.walk();
            self.evict(unpinned);
        }
        let a = Arrivals {
            images: self.images_arrived,
            fonts: self.fonts_arrived,
        };
        self.images_arrived = false;
        self.fonts_arrived = false;
        a
    }

    /// Evicts unreferenced images, oldest first, while the unpinned bytes exceed the LRU
    /// budget or the map holds more than [`MAX_IMAGE_ENTRIES`] − [`IMAGE_ENTRY_HEADROOM`]
    /// entries. Under the entry cap, failed entries (no bytes) are evicted too; under the
    /// byte budget alone they stay, so failed images are not retried every frame.
    fn evict(&mut self, mut unpinned: u64) {
        let target = Self::entry_target();
        let mut guard = self.lru.len();
        while (unpinned > self.lru_budget || self.images.len() > target) && guard > 0 {
            guard -= 1;
            let Some(key) = self.lru.pop_front() else {
                break;
            };
            let count_pressure = self.images.len() > target;
            match self.images.get(&key) {
                Some(Slot::Ready(i)) if Arc::strong_count(i) == 1 => {
                    unpinned = unpinned.saturating_sub(i.byte_size() as u64);
                    self.evict_key(&key);
                }
                Some(Slot::Failed) if count_pressure => self.evict_key(&key),
                Some(Slot::Ready(_) | Slot::Failed) => self.lru.push_back(key),
                // Pending again, or gone: a stale key.
                _ => {}
            }
        }
        if self.lru.len() > MAX_IMAGE_ENTRIES * 2 {
            let images = &self.images;
            self.lru.retain(|k| images.contains_key(k));
        }
    }

    fn evict_key(&mut self, key: &ImageRef) {
        self.remove(key);
        self.stats.images_evicted += 1;
        if let ImageRef::Skin { art_asset, .. } = key
            && let Some(file) = self
                .skin
                .as_ref()
                .and_then(|s| s.replacement(*art_asset))
                .and_then(|r| r.file.as_ref().ok())
        {
            self.ledger.evicted(file);
        }
    }

    /// Makes room for one new entry at the entry cap: evicts the oldest unpinned ready or
    /// failed entry among the first [`MAX_EVICT_PROBES`] LRU keys. Returns whether there
    /// is room now.
    fn make_room(&mut self) -> bool {
        for _ in 0..MAX_EVICT_PROBES {
            if self.images.len() < MAX_IMAGE_ENTRIES {
                return true;
            }
            let Some(key) = self.lru.pop_front() else {
                break;
            };
            match self.images.get(&key) {
                Some(Slot::Ready(i)) if Arc::strong_count(i) == 1 => self.evict_key(&key),
                Some(Slot::Failed) => self.evict_key(&key),
                Some(Slot::Ready(_)) => self.lru.push_back(key),
                _ => {}
            }
        }
        self.images.len() < MAX_IMAGE_ENTRIES
    }

    /// Whether the pinned bytes are past the pinned cap (O(1): the running total bounds
    /// them, the last sample gives them).
    fn pinned_over_cap(&self) -> bool {
        self.ready_bytes > self.pinned_cap && self.pinned_sample > self.pinned_cap
    }

    fn spawn(&mut self, job: impl FnOnce() -> Done + Send + 'static) -> bool {
        if self.in_flight >= MAX_JOBS_IN_FLIGHT {
            return false;
        }
        let tx = self.tx.clone();
        let r = self
            .pool
            .spawn(Priority::Visible, CancelToken::new(), move |_| {
                let _ = tx.send(job());
            });
        if r.is_ok() {
            self.in_flight += 1;
            true
        } else {
            false
        }
    }

    fn image(&mut self, key: &ImageRef) -> ImageState {
        match self.images.get(key) {
            Some(Slot::Ready(i)) => return ImageState::Ready(Arc::clone(i)),
            Some(Slot::Pending) => return ImageState::Pending,
            Some(Slot::Failed) => return ImageState::Ready(Arc::clone(&self.placeholder)),
            None => {}
        }
        if (self.images.len() >= MAX_IMAGE_ENTRIES && !self.make_room()) || self.pinned_over_cap() {
            self.stats.capped += 1;
            return ImageState::Ready(Arc::clone(&self.placeholder));
        }
        match key {
            ImageRef::Did(did) => {
                let Some(data) = self.data.clone() else {
                    return ImageState::Ready(Arc::clone(&self.placeholder));
                };
                let did = *did;
                let k = key.clone();
                if self.spawn(move || Done::Image(k, decode_surface(&data.set, did))) {
                    self.put(key.clone(), Slot::Pending);
                }
                ImageState::Pending
            }
            ImageRef::File(path) => {
                // Each root decodes with its own fixed origin: plugin files (TGA, JPEG,
                // `lua-plugin-api.md` §13.8) under `Plugins/`, Image-item files under the
                // install (`ui-layout.md` §6.2). `tessera-ui` gives no origin with the path,
                // so a plugin file still shadows an Image-item file of the same relative
                // path (`docs/perf/m5-ui-budgets.md`, open items).
                let roots: Vec<(PathBuf, ImageOrigin)> = self
                    .plugins_dir
                    .iter()
                    .map(|d| (d.clone(), ImageOrigin::Plugin))
                    .chain(
                        self.data
                            .as_ref()
                            .map(|d| (d.game_dir.clone(), ImageOrigin::Skin)),
                    )
                    .collect();
                let rel = path.clone();
                let k = key.clone();
                if self.spawn(move || Done::Image(k, decode_file_in(&roots, &rel))) {
                    self.put(key.clone(), Slot::Pending);
                }
                ImageState::Pending
            }
            ImageRef::Skin { art_asset, did } => self.skin_image(key, *art_asset, *did),
        }
    }

    fn skin_image(&mut self, key: &ImageRef, art_asset: u32, did: u32) -> ImageState {
        let Some(overlay) = self.skin.clone() else {
            // No skin any more: draw the base DID (the refresh re-sets it shortly).
            return self.image(&ImageRef::Did(did));
        };
        let Some(rep) = overlay.replacement(art_asset) else {
            return self.image(&ImageRef::Did(did));
        };
        match self.ledger.request(rep) {
            LoadRequest::Schedule(ticket) => {
                let Ok(file) = rep.file.clone() else {
                    return ImageState::Ready(Arc::clone(&self.placeholder));
                };
                let k = key.clone();
                let ov = Arc::clone(&overlay);
                let f2 = file.clone();
                if self.spawn(move || {
                    let r = f2.load_with(ov.dir_index()).map_err(|e| e.to_string());
                    Done::Skin(ticket, f2, k, r)
                }) {
                    self.put(key.clone(), Slot::Pending);
                    ImageState::Pending
                } else {
                    // Report the refused job so the ledger does not wait forever.
                    let _ = self.ledger.complete(
                        &file,
                        ticket,
                        Err(&tessera_ui::skin::SkinImageError::Io(
                            "worker queue full".into(),
                        )),
                    );
                    ImageState::Ready(Arc::clone(&self.placeholder))
                }
            }
            LoadRequest::Pending => ImageState::Pending,
            LoadRequest::Ready => match self.images.get(key) {
                Some(Slot::Ready(i)) => ImageState::Ready(Arc::clone(i)),
                _ => {
                    if let Ok(file) = &rep.file {
                        self.ledger.evicted(file);
                    }
                    ImageState::Pending
                }
            },
            LoadRequest::Placeholder => ImageState::Ready(Arc::clone(&self.placeholder)),
        }
    }

    /// A font by DID: loaded, or `None` (and a load is started when it is new).
    pub(crate) fn font(&mut self, did: u32) -> Option<Arc<FontFace>> {
        match self.fonts.get(&did) {
            Some(FontSlot::Ready(f)) => return Some(Arc::clone(f)),
            Some(FontSlot::Pending | FontSlot::Failed) => return None,
            None => {}
        }
        if self.fonts.len() >= MAX_FONT_ENTRIES {
            self.stats.fonts_capped += 1;
            return None;
        }
        let data = self.data.clone()?;
        if self.spawn(move || Done::Font(did, decode_font(&data.set, did))) {
            self.fonts.insert(did, FontSlot::Pending);
        }
        None
    }

    /// Whether `did` names a font that loaded, or may still load (pending or not yet
    /// asked for). `fonts.md` §9.1 rule 4: a DID that does not resolve leaves the slot.
    pub(crate) fn font_resolves(&self, did: u32) -> bool {
        match self.fonts.get(&did) {
            Some(FontSlot::Failed) => false,
            Some(_) => true,
            // A DID past the cap never loads.
            None => self.fonts.len() < MAX_FONT_ENTRIES,
        }
    }

    /// Tessera's fallback font (`fonts.md` §11), rasterised on first use.
    pub(crate) fn fallback_font(&mut self) -> Option<Arc<FontFace>> {
        if self.fallback_font.is_none() {
            match TesseraFont::bundled()
                .and_then(|f| f.rasterise(FALLBACK_FONT_PX, tessera_ui::text::DEFAULT_RANGES))
            {
                Ok(face) => self.fallback_font = Some(Arc::new(face)),
                Err(e) => tracing::warn!("Tessera fallback font: {e}"),
            }
        }
        self.fallback_font.clone()
    }

    fn next_random(&mut self) -> f32 {
        // xorshift64*; the UI's random items need no quality guarantee.
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        let v = x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40;
        v as f32 / (1u64 << 24) as f32
    }
}

fn decode_surface(set: &ArchiveSet, did: u32) -> Result<UiImage, String> {
    let payload = set.read(did).map_err(|e| e.to_string())?;
    let surface = Surface::parse(&payload).map_err(|e| e.to_string())?;
    ui_image::from_surface(&surface).map_err(|e| e.to_string())
}

/// Most bytes read from one UI image file (the decoders' own limit applies too).
const MAX_FILE_READ: u64 = ui_image::MAX_FILE_BYTES as u64;

fn decode_file_in(roots: &[(PathBuf, ImageOrigin)], rel: &str) -> Result<UiImage, String> {
    let rel_path = safe_relative(rel).ok_or_else(|| format!("unsafe image path {rel:?}"))?;
    let mut last = String::from("no root");
    for (root, origin) in roots {
        let p = root.join(&rel_path);
        match read_bounded(&p) {
            Ok(bytes) => {
                return ui_image::decode_file(&p, *origin, &bytes).map_err(|e| e.to_string());
            }
            Err(e) => last = e,
        }
    }
    Err(last)
}

fn read_bounded(p: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read as _;
    let f = std::fs::File::open(p).map_err(|e| e.to_string())?;
    let meta = f.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("not a file".into());
    }
    let mut buf = Vec::new();
    f.take(MAX_FILE_READ + 1)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    if buf.len() as u64 > MAX_FILE_READ {
        return Err("file too large".into());
    }
    Ok(buf)
}

/// A relative path with only normal components (no root, no `..`), separators `/` or `\`.
#[must_use]
pub fn safe_relative(rel: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for part in rel.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return None,
            p if p.contains(':') => return None,
            p => out.push(p),
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

fn decode_font(set: &ArchiveSet, did: u32) -> Result<FontFace, String> {
    let font_bytes = set.read(did).map_err(|e| e.to_string())?;
    let font = tessera_formats::font::Font::parse(&font_bytes).map_err(|e| e.to_string())?;
    let fg = set.read(font.foreground.0).map_err(|e| e.to_string())?;
    let bg = if font.has_background() {
        Some(set.read(font.background.0).map_err(|e| e.to_string())?)
    } else {
        None
    };
    FontFace::from_payloads(&font_bytes, &fg, bg.as_deref()).map_err(|e| e.to_string())
}

/// The locale default font DID from a locale record (`string-table.md` §10.3): the record
/// ends exactly after its last field, and the font DID is the third-last 32-bit field.
/// `None` for a record too short to hold it or a value without the font prefix 0x42.
#[must_use]
pub fn locale_font_from_record(bytes: &[u8]) -> Option<u32> {
    // The fixed part alone is 4 + 2 + 2 + 1 + 2 + 5 bytes, plus at least nine string
    // counts, plus 12 bytes of the three trailing fields.
    const MIN_LEN: usize = 16 + 9 + 12;
    if bytes.len() < MIN_LEN {
        return None;
    }
    let at = bytes.len() - 12;
    let v = u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?);
    (v >> 24 == 0x42).then_some(v)
}

/// The locale leading factor *f*₀ from a locale record (`fonts.md` §7.1): the 32-bit field
/// after the font DID (`string-table.md` §10.3), read as an IEEE single-precision number.
/// 0 for a record too short to hold it, a value that is not finite, or a negative one.
#[must_use]
pub fn locale_leading_from_record(bytes: &[u8]) -> f32 {
    locale_font_from_record(bytes)
        .and_then(|_| {
            let at = bytes.len() - 8;
            Some(f32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
        })
        .filter(|v| v.is_finite() && *v >= 0.0)
        .unwrap_or(0.0)
}

/// The client's [`UiEnv`] (module docs).
#[derive(Debug, Clone)]
pub struct ClientEnv {
    state: EnvHandle,
}

impl ClientEnv {
    /// An environment over `state`.
    #[must_use]
    pub fn new(state: EnvHandle) -> Self {
        Self { state }
    }
}

impl UiEnv for ClientEnv {
    fn layout(&mut self, did: Did) -> Option<Arc<UiLayout>> {
        let mut st = self.state.borrow_mut();
        let data = st.data.clone()?;
        let (l, late) = data.layout(did.0);
        if late {
            st.stats.late_layouts += 1;
            tracing::debug!(
                layout = format_args!("{:#010x}", did.0),
                "layout decoded on the main thread"
            );
        }
        l
    }

    fn layout_by_key(&mut self, key: u32) -> Option<Did> {
        let st = self.state.borrow();
        st.data.as_ref()?.layout_keys.get(&key).map(|d| Did(*d))
    }

    fn image(&mut self, image: &ImageRef) -> ImageState {
        self.state.borrow_mut().image(image)
    }

    fn shape_text(&mut self, request: &TextRequest<'_>) -> Option<Arc<PreparedText>> {
        let mut st = self.state.borrow_mut();
        super::text::shape(&mut st, request)
    }

    fn resolve_string(&mut self, info: &StringInfo) -> Option<String> {
        self.state.borrow_mut().resolve_string(info)
    }

    fn input_map_name(&mut self, value: u32) -> Option<String> {
        let st = self.state.borrow();
        st.data.as_ref()?.input_maps.get(&value).cloned()
    }

    fn random(&mut self) -> f32 {
        self.state.borrow_mut().next_random()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_are_confined() {
        assert_eq!(
            safe_relative("A/B\\c.tga"),
            Some(PathBuf::from("A").join("B").join("c.tga"))
        );
        assert_eq!(safe_relative("../x.tga"), None);
        assert_eq!(safe_relative("C:/x.tga"), None);
        assert_eq!(safe_relative(""), None);
        assert_eq!(safe_relative("./a/./b"), Some(PathBuf::from("a").join("b")));
    }

    #[test]
    fn locale_font_never_panics() {
        for len in 0..80usize {
            let bytes: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            let _ = locale_font_from_record(&bytes);
        }
        let mut rec = vec![0u8; 60];
        let at = rec.len() - 12;
        rec[at..at + 4].copy_from_slice(&0x4200_0028u32.to_le_bytes());
        assert_eq!(locale_font_from_record(&rec), Some(0x4200_0028));
        rec[at + 4..at + 8].copy_from_slice(&0.5f32.to_le_bytes());
        assert!((locale_leading_from_record(&rec) - 0.5).abs() < 1e-6);
        rec[at + 4..at + 8].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(locale_leading_from_record(&rec), 0.0);
        rec[at + 3] = 0x41;
        assert_eq!(locale_font_from_record(&rec), None);
        assert_eq!(locale_leading_from_record(&rec), 0.0);
        assert_eq!(locale_leading_from_record(&[]), 0.0);
    }

    fn tiny() -> UiImage {
        ui_image::placeholder()
    }

    #[test]
    fn failed_entries_never_starve_the_cache() {
        let mut st = EnvState::new(None, 1, 64 << 20, 512 << 20);
        // A full map of failed images (no bytes), as a plugin asking for missing files
        // leaves it.
        for i in 0..MAX_IMAGE_ENTRIES {
            let k = ImageRef::File(format!("missing/{i}.tga"));
            st.put(k.clone(), Slot::Failed);
            st.lru.push_back(k);
        }
        assert_eq!(st.image_entries(), MAX_IMAGE_ENTRIES);
        // A new request finds room at once (no install: answered with the placeholder,
        // but not as "capped").
        let _ = st.image(&ImageRef::Did(0x4100_0001));
        assert_eq!(st.stats().capped, 0, "{:?}", st.stats());
        // The next poll brings the map below the entry cap with headroom.
        let _ = st.poll();
        assert!(st.image_entries() <= MAX_IMAGE_ENTRIES - IMAGE_ENTRY_HEADROOM);
        assert!(st.stats().images_evicted >= IMAGE_ENTRY_HEADROOM as u64);
    }

    #[test]
    fn small_unpinned_images_are_evicted_by_count() {
        let mut st = EnvState::new(None, 1, 64 << 20, 512 << 20);
        let bytes = tiny().byte_size() as u64;
        for i in 0..MAX_IMAGE_ENTRIES {
            let k = ImageRef::File(format!("icon/{i}.tga"));
            st.put(k.clone(), Slot::Ready(Arc::new(tiny())));
            st.lru.push_back(k);
        }
        assert_eq!(st.stats().image_bytes, bytes * MAX_IMAGE_ENTRIES as u64);
        let _ = st.poll();
        assert!(st.image_entries() <= MAX_IMAGE_ENTRIES - IMAGE_ENTRY_HEADROOM);
        assert_eq!(
            st.stats().image_bytes,
            bytes * st.image_entries() as u64,
            "the running total follows evictions"
        );
    }

    #[test]
    fn requests_and_figures_do_not_walk_a_large_cache() {
        let mut st = EnvState::new(None, 1, 64 << 20, 512 << 20);
        let mut held = Vec::new();
        for i in 0..8000 {
            let img = Arc::new(tiny());
            held.push(Arc::clone(&img));
            let k = ImageRef::File(format!("held/{i}.tga"));
            st.put(k.clone(), Slot::Ready(img));
            st.lru.push_back(k);
        }
        let _ = st.walk();
        let pinned = st.stats().pinned_bytes;
        assert_eq!(pinned, tiny().byte_size() as u64 * 8000);
        let t = std::time::Instant::now();
        for i in 0..256 {
            let _ = st.image(&ImageRef::Did(0x4100_0000 + i));
            let _ = st.stats();
        }
        // O(1) per request: far below the ~0.8 ms per request of a full walk.
        assert!(
            t.elapsed() < std::time::Duration::from_millis(100),
            "{:?}",
            t.elapsed()
        );
        drop(held);
    }

    #[test]
    fn fonts_past_the_cap_do_not_resolve() {
        let mut st = EnvState::new(None, 1, 1 << 20, 1 << 24);
        for i in 0..MAX_FONT_ENTRIES as u32 {
            st.fonts.insert(0x4200_0000 + i, FontSlot::Failed);
        }
        let new = 0x42FF_FFFF;
        assert!(st.font(new).is_none());
        assert!(!st.font_resolves(new), "a capped DID falls back");
        assert_eq!(st.font_entries(), MAX_FONT_ENTRIES);
        assert_eq!(st.stats().fonts_capped, 1);
    }

    #[test]
    fn file_roots_keep_their_origin() {
        let plugins = tempfile::tempdir().expect("tempdir");
        let install = tempfile::tempdir().expect("tempdir");
        let dds = b"DDS not really";
        std::fs::write(install.path().join("a.dds"), dds).expect("write");
        // Only under the install: decoded as an install file, never as a plugin file.
        let roots = vec![
            (plugins.path().to_path_buf(), ImageOrigin::Plugin),
            (install.path().to_path_buf(), ImageOrigin::Skin),
        ];
        let e = decode_file_in(&roots, "a.dds").expect_err("not a valid DDS");
        let as_plugin = decode_file_in(
            &[(install.path().to_path_buf(), ImageOrigin::Plugin)],
            "a.dds",
        )
        .expect_err("plugins may not use DDS");
        assert_ne!(e, as_plugin, "decoded with the Skin origin: {e}");
        // Without a plugins folder the install root still decodes as Skin.
        let roots = vec![(install.path().to_path_buf(), ImageOrigin::Skin)];
        let e2 = decode_file_in(&roots, "a.dds").expect_err("not a valid DDS");
        assert_eq!(e, e2);
    }

    #[test]
    fn plugin_files_decode_on_workers() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut tga = vec![0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 32, 0x28];
        tga.extend_from_slice(&[0x30, 0x20, 0x10, 0xFF]);
        std::fs::create_dir_all(dir.path().join("Me/P")).expect("mkdir");
        std::fs::write(dir.path().join("Me/P/a.tga"), &tga).expect("write");
        let state: EnvHandle = Rc::new(RefCell::new(EnvState::new(None, 1, 1 << 20, 1 << 24)));
        state
            .borrow_mut()
            .set_plugins_dir(Some(dir.path().to_path_buf()));
        let mut env = ClientEnv::new(Rc::clone(&state));
        let key = ImageRef::File("Me/P/a.tga".into());
        assert!(matches!(env.image(&key), ImageState::Pending));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while state.borrow().busy() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(2));
            let _ = state.borrow_mut().poll();
        }
        let _ = state.borrow_mut().poll();
        match env.image(&key) {
            ImageState::Ready(i) => assert_eq!(i.pixels(), &[0xFF10_2030]),
            other => panic!("{other:?}"),
        }
        // A missing file is the placeholder, never an endless wait.
        let missing = ImageRef::File("Me/P/none.tga".into());
        let _ = env.image(&missing);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while state.borrow().busy() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(2));
            let _ = state.borrow_mut().poll();
        }
        assert!(matches!(env.image(&missing), ImageState::Ready(_)));
        // Escaping paths never reach the file system.
        let bad = ImageRef::File("../etc/passwd".into());
        let _ = env.image(&bad);
    }
}
