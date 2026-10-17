// SPDX-License-Identifier: GPL-3.0-or-later
//! The eframe application: state, frame loop, navigation and the side panels. The
//! central views are drawn by the `views` module.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Key, RichText, TextureHandle, TextureOptions};
use tessera_assets::{AssetKey, AssetStore, Handle, PoolConfig, Priority, StoreConfig, WorkerPool};
use tessera_formats::Did;

use crate::cli::ViewKind;
use crate::data::{DataContext, Discovering, Opening, spawn_open};
use crate::decoders::{
    AUTO_LEVEL, ContextSource, IconDecoder, IconProduct, NameIndex, NameIndexDecoder, PropsDecoder,
    PropsProduct, RawDecoder, RawRecord, RecordDecoder, RecordProduct, StringsDecoder,
    StringsProduct, TextureDecoder, TextureProduct, TextureRequest,
};
use crate::frame_stats::{FrameStats, SLOW_FRAME};
use crate::hex::{human_bytes, thousands};
use crate::history::History;
use crate::image_ops::{ChannelMask, checkerboard};
use crate::index::{ArchiveIndex, Indexer};
use crate::mesh_view::{MeshOptions, MeshSettings, MeshState};
use crate::persist::{Persist, WINDOW_KEY};
use crate::query::{DidQuery, parse_query};
use crate::scene::{SceneDecoder, SceneRequest};
use crate::search::{Search, SearchResults};
use crate::viewport::Viewport;

/// What `--screenshot` asks for.
#[derive(Debug, Clone)]
pub struct ScreenshotPlan {
    /// Output PNG path.
    pub path: PathBuf,
    /// Longest wait for the selection before the screenshot is taken anyway.
    pub timeout: Duration,
    /// The window size asked for (logical pixels); a capture of another size is
    /// reported as a warning.
    pub size: Option<[f32; 2]>,
}

/// Start-up configuration of the explorer.
#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    /// Install directory given on the command line.
    pub game_dir: Option<PathBuf>,
    /// String-table language.
    pub language: Option<String>,
    /// DID to select once the install is open.
    pub select: Option<u32>,
    /// View for `select`.
    pub view: Option<ViewKind>,
    /// Screenshot mode.
    pub screenshot: Option<ScreenshotPlan>,
    /// In screenshot mode: DID-list rows to step through (one per frame) before the
    /// screenshot, as a responsiveness check.
    pub browse: u32,
    /// A search to run once the install is open (as if typed in the search box).
    pub search: Option<String>,
    /// UI font file (see [`crate::fonts::find_fonts`]).
    pub font: Option<PathBuf>,
    /// Start-up options of the mesh view.
    pub mesh: MeshOptions,
    /// Start-up options of the character view.
    pub character: crate::charspec::CharacterArgs,
    /// Hide the archive tree, the DID list and the character view's inspector.
    pub hide_panels: bool,
}

/// Exit code when the screenshot timed out waiting for the selection.
pub const EXIT_TIMEOUT: i32 = 2;
/// Exit code when the screenshot could not be written.
pub const EXIT_SCREENSHOT_FAILED: i32 = 3;
/// Exit code when the install could not be opened (the screenshot shows the error).
pub const EXIT_OPEN_FAILED: i32 = 4;

/// The exit code of a `--screenshot` run: the write result first, then an install that
/// failed to open, then a timeout.
#[must_use]
pub fn screenshot_exit_code(saved: bool, open_failed: bool, timed_out: bool) -> i32 {
    match (saved, open_failed, timed_out) {
        (false, _, _) => EXIT_SCREENSHOT_FAILED,
        (true, true, _) => EXIT_OPEN_FAILED,
        (true, false, true) => EXIT_TIMEOUT,
        (true, false, false) => 0,
    }
}

/// The event-loop set-up of `--screenshot` runs: X11 whenever an X display is set, so a
/// headless run under `xvfb-run` gets the requested window size even in a Wayland
/// session (winit prefers Wayland when `WAYLAND_DISPLAY` is set, and the compositor
/// then chooses the size on the user's desktop).
#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
))]
fn screenshot_event_loop() -> Option<eframe::EventLoopBuilderHook> {
    use winit::platform::x11::EventLoopBuilderExtX11 as _;
    if std::env::var_os("DISPLAY").is_none_or(|d| d.is_empty()) {
        return None;
    }
    tracing::info!("screenshot mode: using X11 (DISPLAY is set)");
    Some(Box::new(|b| {
        b.with_x11();
    }))
}

#[cfg(not(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
)))]
fn screenshot_event_loop() -> Option<eframe::EventLoopBuilderHook> {
    None
}

/// Whether a captured image of `captured` physical pixels matches a window asked to be
/// `requested` logical pixels at `ppp` pixels per point (within a pixel per side).
#[must_use]
pub fn capture_matches(captured: [usize; 2], requested: [f32; 2], ppp: f32) -> bool {
    (0..2).all(|i| ((captured[i] as f32) - requested[i] * ppp).abs() <= 1.5)
}

/// Frames rendered after the selection is ready before the screenshot is requested.
const SCREENSHOT_SETTLE_FRAMES: u32 = 8;

/// Runs the explorer window until it is closed; returns the process exit code.
///
/// # Errors
///
/// When no usable UI font is found ([`crate::fonts::find_fonts`]) or the window cannot
/// be created.
pub fn run(config: AppConfig, persist: Persist, size: Option<[f32; 2]>) -> anyhow::Result<i32> {
    let fonts = crate::fonts::find_fonts(config.font.as_deref())?;
    tracing::info!(
        text = %fonts.proportional.path.display(),
        mono = ?fonts.monospace.as_ref().map(|f| f.path.display().to_string()),
        symbols = ?fonts.symbols.as_ref().map(|f| f.path.display().to_string()),
        "UI fonts"
    );
    let font_defs = crate::fonts::font_definitions(&fonts);
    drop(fonts);
    let saved = persist.settings.window(WINDOW_KEY).cloned();
    let inner = size.unwrap_or_else(|| {
        saved
            .as_ref()
            .map_or([1400.0, 900.0], |w| [w.width as f32, w.height as f32])
    });
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Tessera Explorer")
        .with_app_id("tessera-explorer")
        .with_inner_size(inner)
        .with_min_inner_size([640.0, 400.0]);
    if config.screenshot.is_none()
        && let Some(w) = &saved
        && w.maximized
    {
        viewport = viewport.with_maximized(true);
    }
    let mut options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        ..eframe::NativeOptions::default()
    };
    if config.screenshot.is_some() {
        options.event_loop_builder = screenshot_event_loop();
    }
    let exit = Arc::new(AtomicI32::new(0));
    let exit_app = Arc::clone(&exit);
    eframe::run_native(
        "tessera-explorer",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_fonts(font_defs);
            Ok(Box::new(ExplorerApp::new(
                cc.egui_ctx.clone(),
                config,
                persist,
                exit_app,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(exit.load(Ordering::Relaxed))
}

/// A place in the explorer: a DID, optionally pinned to one archive, and a view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Nav {
    /// The DID.
    pub did: u32,
    /// The archive to read it from (`None`: through routing).
    pub member: Option<usize>,
    /// The view.
    pub view: ViewKind,
}

/// The asset stores, one per product, sharing one worker pool.
pub(crate) struct Stores {
    pub pool: WorkerPool,
    pub raw: AssetStore<ContextSource, RawDecoder>,
    pub texture: AssetStore<ContextSource, TextureDecoder>,
    pub strings: AssetStore<ContextSource, StringsDecoder>,
    pub props: AssetStore<ContextSource, PropsDecoder>,
    pub icon: AssetStore<ContextSource, IconDecoder>,
    pub record: AssetStore<ContextSource, RecordDecoder>,
    pub names: AssetStore<ContextSource, NameIndexDecoder>,
    pub scene: AssetStore<ContextSource, SceneDecoder>,
    pub landblock: AssetStore<ContextSource, crate::world::LandblockDecoder>,
    pub sky: AssetStore<ContextSource, crate::world::SkyDecoder>,
    pub gameplay: AssetStore<ContextSource, crate::gameplay::GameplayDecoder>,
}

impl Stores {
    fn new(ctx: &Arc<DataContext>) -> Self {
        let pool = WorkerPool::new(PoolConfig {
            thread_name: "explorer-asset".into(),
            ..PoolConfig::default()
        });
        let cfg = StoreConfig::default();
        let big = StoreConfig {
            cache_budget: 512 << 20,
            ..StoreConfig::default()
        };
        Self {
            raw: AssetStore::new(
                &pool,
                ContextSource,
                RawDecoder(Arc::clone(ctx)),
                cfg.clone(),
            ),
            texture: AssetStore::new(
                &pool,
                ContextSource,
                TextureDecoder::new(Arc::clone(ctx)),
                big,
            ),
            strings: AssetStore::new(
                &pool,
                ContextSource,
                StringsDecoder(Arc::clone(ctx)),
                cfg.clone(),
            ),
            props: AssetStore::new(
                &pool,
                ContextSource,
                PropsDecoder(Arc::clone(ctx)),
                cfg.clone(),
            ),
            icon: AssetStore::new(
                &pool,
                ContextSource,
                IconDecoder(Arc::clone(ctx)),
                cfg.clone(),
            ),
            record: AssetStore::new(
                &pool,
                ContextSource,
                RecordDecoder(Arc::clone(ctx)),
                cfg.clone(),
            ),
            names: AssetStore::new(
                &pool,
                ContextSource,
                NameIndexDecoder(Arc::clone(ctx)),
                cfg.clone(),
            ),
            landblock: AssetStore::new(
                &pool,
                ContextSource,
                crate::world::LandblockDecoder(Arc::clone(ctx)),
                cfg.clone(),
            ),
            sky: AssetStore::new(
                &pool,
                ContextSource,
                crate::world::SkyDecoder(Arc::clone(ctx)),
                cfg.clone(),
            ),
            gameplay: AssetStore::new(
                &pool,
                ContextSource,
                crate::gameplay::GameplayDecoder::new(Arc::clone(ctx)),
                cfg,
            ),
            scene: AssetStore::new(
                &pool,
                ContextSource,
                SceneDecoder(Arc::clone(ctx)),
                StoreConfig {
                    cache_budget: 768 << 20,
                    ..StoreConfig::default()
                },
            ),
            pool,
        }
    }

    /// Drains every completion queue; returns how many jobs finished.
    fn drain(&self) -> usize {
        self.raw.poll_completions(256).len()
            + self.texture.poll_completions(256).len()
            + self.strings.poll_completions(256).len()
            + self.props.poll_completions(256).len()
            + self.icon.poll_completions(256).len()
            + self.record.poll_completions(256).len()
            + self.names.poll_completions(256).len()
            + self.scene.poll_completions(256).len()
            + self.landblock.poll_completions(256).len()
            + self.sky.poll_completions(256).len()
            + self.gameplay.poll_completions(256).len()
    }
}

/// The opened install with its stores and indexes.
pub(crate) struct Loaded {
    pub ctx: Arc<DataContext>,
    pub stores: Stores,
    pub indexer: Indexer,
    /// Per member, once indexed.
    pub indexes: Vec<Option<Arc<ArchiveIndex>>>,
    pub names: Handle<NameIndex>,
}

/// State of the install.
pub(crate) enum Install {
    /// No directory chosen yet.
    None,
    /// Opening on a background thread.
    Opening(Opening),
    /// Opening failed.
    Failed { dir: PathBuf, error: String },
    /// Open.
    Open(Box<Loaded>),
}

/// Everything about the selected DID.
pub(crate) struct Current {
    pub nav: Nav,
    pub raw: Option<Handle<RawRecord>>,
    pub texture: Option<Handle<TextureProduct>>,
    /// The last texture product that was ready (kept on screen while a new level or
    /// channel selection loads).
    pub texture_shown: Option<Arc<TextureProduct>>,
    pub tex_req: TextureRequest,
    /// Whether the channel mask was chosen automatically (A8 surfaces switch to alpha).
    pub mask_auto: bool,
    pub strings: Option<Handle<StringsProduct>>,
    pub props: Option<Handle<PropsProduct>>,
    pub icon: Option<Handle<IconProduct>>,
    pub record: Option<Handle<RecordProduct>>,
    /// GPU textures by name.
    pub gpu: HashMap<String, TextureHandle>,
    pub zoom: f32,
    pub fit: bool,
    pub string_sel: Option<usize>,
    pub string_filter: String,
    pub string_filtered: Option<(String, Vec<usize>)>,
    pub string_values: String,
    pub props_collapsed: HashSet<usize>,
    pub props_visible: Option<Vec<usize>>,
    pub props_filter: String,
    pub record_filter: String,
    pub record_filtered: Option<(String, Vec<usize>)>,
    /// The mesh view's state.
    pub mesh: MeshState,
    /// The landblock and sky views' state.
    pub world: crate::world_view::WorldViewState,
    /// The gameplay view's state.
    pub gameplay: crate::gameplay::view::GameplayViewState,
}

impl Current {
    fn new(nav: Nav, mask: ChannelMask, mesh: MeshState) -> Self {
        Self {
            nav,
            raw: None,
            texture: None,
            texture_shown: None,
            tex_req: TextureRequest {
                level: AUTO_LEVEL,
                mask,
            },
            mask_auto: true,
            strings: None,
            props: None,
            icon: None,
            record: None,
            gpu: HashMap::new(),
            zoom: 1.0,
            fit: true,
            string_sel: None,
            string_filter: String::new(),
            string_filtered: None,
            string_values: String::new(),
            props_collapsed: HashSet::new(),
            props_visible: None,
            props_filter: String::new(),
            record_filter: String::new(),
            record_filtered: None,
            mesh,
            world: crate::world_view::WorldViewState::default(),
            gameplay: crate::gameplay::view::GameplayViewState::default(),
        }
    }

    fn cancel_all(&self) {
        if let Some(h) = &self.raw {
            h.cancel();
        }
        if let Some(h) = &self.texture {
            h.cancel();
        }
        if let Some(h) = &self.strings {
            h.cancel();
        }
        if let Some(h) = &self.props {
            h.cancel();
        }
        if let Some(h) = &self.icon {
            h.cancel();
        }
        if let Some(h) = &self.record {
            h.cancel();
        }
        self.mesh.cancel();
        self.world.cancel();
        self.gameplay.cancel();
    }
}

impl Drop for Current {
    fn drop(&mut self) {
        self.cancel_all();
    }
}

/// What the DID list shows.
pub(crate) enum ListSource {
    None,
    Group { member: usize, group: usize },
    Search(Arc<SearchResults>),
}

/// Screenshot progress.
pub(crate) struct ScreenshotState {
    pub plan: ScreenshotPlan,
    /// `--browse` rows still to step through.
    pub browse_left: u32,
    /// `--browse` rows actually stepped through.
    pub browsed: u32,
    pub started: Instant,
    pub ready_frames: u32,
    pub requested: bool,
    pub timed_out: bool,
    pub done: bool,
    pub writer: Option<std::thread::JoinHandle<()>>,
}

/// The explorer application.
pub struct ExplorerApp {
    pub(crate) egui_ctx: egui::Context,
    pub(crate) config: AppConfig,
    pub(crate) persist: Persist,
    pub(crate) install: Install,
    pub(crate) dir_input: String,
    pub(crate) discovering: Option<Discovering>,
    pub(crate) discovered: Option<Vec<PathBuf>>,
    pub(crate) history: History<Nav>,
    pub(crate) current: Option<Current>,
    pub(crate) list: ListSource,
    pub(crate) list_selected: Option<usize>,
    pub(crate) list_scroll_to: Option<usize>,
    pub(crate) search_text: String,
    pub(crate) search: Option<Search>,
    pub(crate) searching: Option<String>,
    pub(crate) frames: FrameStats,
    pub(crate) show_overlay: bool,
    pub(crate) checker: Option<TextureHandle>,
    pub(crate) screenshot: Option<ScreenshotState>,
    pub(crate) exit_code: Arc<AtomicI32>,
    pub(crate) pending: Option<Nav>,
    pub(crate) pending_search: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) last_frame_start: Option<Instant>,
    /// Archive whose tree node should be opened (once).
    pub(crate) tree_open: Option<usize>,
    /// The mesh view's render thread (started on first use).
    pub(crate) viewport: Option<Viewport>,
    /// The mesh view's display settings.
    pub(crate) mesh_settings: MeshSettings,
    /// The mesh state of the first selection (from the command line).
    pub(crate) mesh_start: Option<MeshState>,
    /// The character view's state (it persists while browsing).
    pub(crate) character: crate::anim_view::CharacterState,
    /// Whether the optional UI-scale preview window is open (`ui-scaling.md` §7.4, §9.2).
    pub(crate) show_scale_preview: bool,
    /// The UI-scale preview's control state.
    pub(crate) scale_preview: crate::scale_preview::ScalePreview,
}

impl std::fmt::Debug for ExplorerApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExplorerApp")
            .field("config", &self.config)
            .field("frames", &self.frames.frames())
            .finish_non_exhaustive()
    }
}

impl ExplorerApp {
    /// Creates the application and starts opening the install (on a background
    /// thread). The game directory is, in order: the configured one,
    /// `TESSERA_GAME_DIR`, the saved one; otherwise install discovery runs and the
    /// welcome screen asks.
    pub fn new(
        egui_ctx: egui::Context,
        config: AppConfig,
        persist: Persist,
        exit_code: Arc<AtomicI32>,
    ) -> Self {
        egui_ctx.set_visuals(egui::Visuals::dark());
        let screenshot = config.screenshot.clone().map(|plan| ScreenshotState {
            plan,
            browse_left: config.browse,
            browsed: 0,
            started: Instant::now(),
            ready_frames: 0,
            requested: false,
            timed_out: false,
            done: false,
            writer: None,
        });
        let dir = config
            .game_dir
            .clone()
            .or_else(|| std::env::var_os(tessera_install::GAME_DIR_ENV).map(PathBuf::from))
            .or_else(|| persist.settings.game.active_dir.clone());
        let pending = config.select.map(|did| Nav {
            did,
            member: None,
            view: config.view.unwrap_or_else(|| ViewKind::default_for(did)),
        });
        let mesh_settings = config.mesh.settings();
        let mesh_start = Some(config.mesh.start());
        let mut character = crate::anim_view::CharacterState::new(
            config.character.clone(),
            config.screenshot.is_some(),
        );
        character.compact = config.hide_panels;
        let mut app = Self {
            egui_ctx,
            config,
            persist,
            install: Install::None,
            dir_input: String::new(),
            discovering: None,
            discovered: None,
            history: History::new(),
            current: None,
            list: ListSource::None,
            list_selected: None,
            list_scroll_to: None,
            search_text: String::new(),
            search: None,
            searching: None,
            frames: FrameStats::new(),
            show_overlay: true,
            checker: None,
            screenshot,
            exit_code,
            pending: None,
            pending_search: None,
            status: None,
            last_frame_start: None,
            tree_open: None,
            viewport: None,
            mesh_settings,
            mesh_start,
            character,
            show_scale_preview: false,
            scale_preview: crate::scale_preview::ScalePreview::default(),
        };
        match dir {
            Some(d) => {
                app.dir_input = d.display().to_string();
                app.open_install(d);
            }
            None => app.start_discovery(),
        }
        // Set after `open_install`, which drops what was pending for a previous install.
        app.pending = pending;
        app.pending_search = app.config.search.clone();
        app
    }

    fn notifier(&self) -> impl Fn() + Send + Sync + Clone + 'static {
        let ctx = self.egui_ctx.clone();
        move || ctx.request_repaint()
    }

    pub(crate) fn start_discovery(&mut self) {
        let n = self.notifier();
        self.discovering = Some(Discovering::spawn(n));
    }

    /// Starts opening `dir` on a background thread. Everything tied to the previous
    /// install goes: the selection, the DID list, searches and the back/forward history
    /// (whose entries name archives by their position in that install's set).
    pub(crate) fn open_install(&mut self, dir: PathBuf) {
        self.current = None;
        self.character.reset_install();
        if let Some(vp) = &mut self.viewport {
            // The previous install's scene leaves the render thread too.
            vp.clear_scene();
        }
        self.list = ListSource::None;
        self.list_selected = None;
        self.list_scroll_to = None;
        self.search = None;
        self.searching = None;
        self.history = History::new();
        self.pending = None;
        self.pending_search = None;
        self.tree_open = None;
        self.status = None;
        let n = self.notifier();
        self.install = Install::Opening(spawn_open(dir, self.config.language.clone(), n));
    }

    fn poll_install(&mut self) {
        let Install::Opening(op) = &self.install else {
            return;
        };
        let Some(result) = op.poll() else { return };
        let dir = op.dir.clone();
        match result {
            Ok(ctx) => {
                let max_side = self.egui_ctx.input(|i| i.max_texture_side);
                ctx.max_texture_side
                    .store(max_side.max(1), Ordering::Relaxed);
                let ctx = Arc::new(ctx);
                tracing::info!(
                    dir = %dir.display(),
                    archives = ctx.set.members().len(),
                    locale = ?ctx.strings.as_ref().and_then(|s| s.locale()).map(|l| l.name.clone()),
                    "install opened"
                );
                let stores = Stores::new(&ctx);
                let names = stores.names.request(
                    AssetKey::from(tessera_props::ROOT_DID_MAP.0),
                    Priority::Prefetch,
                );
                let indexer = Indexer::spawn(Arc::clone(&ctx.set), self.notifier());
                let n = ctx.set.members().len();
                self.install = Install::Open(Box::new(Loaded {
                    ctx,
                    stores,
                    indexer,
                    indexes: vec![None; n],
                    names,
                }));
                if self.screenshot.is_none() {
                    let d = dir.clone();
                    self.persist.update(|s| s.remember_game_dir(d));
                }
                let start = self.pending.take().or_else(|| {
                    // Restore the last DID in normal runs.
                    (self.screenshot.is_none())
                        .then_some(self.persist.settings.explorer.last_did)
                        .flatten()
                        .map(|did| Nav {
                            did,
                            member: None,
                            view: ViewKind::default_for(did),
                        })
                });
                if let Some(nav) = start {
                    self.navigate(nav);
                }
            }
            Err(e) => {
                tracing::error!(dir = %dir.display(), error = %e, "opening the install failed");
                self.install = Install::Failed {
                    dir,
                    error: e.to_string(),
                };
            }
        }
    }

    pub(crate) fn loaded(&self) -> Option<&Loaded> {
        match &self.install {
            Install::Open(l) => Some(l),
            _ => None,
        }
    }

    /// Goes to `nav`, recording it in the history.
    pub(crate) fn navigate(&mut self, nav: Nav) {
        let nav = Nav {
            view: if nav.view.applies_to(nav.did) {
                nav.view
            } else {
                ViewKind::default_for(nav.did)
            },
            ..nav
        };
        if let Some(cur) = &mut self.current
            && cur.nav.did == nav.did
            && cur.nav.member == nav.member
        {
            cur.nav.view = nav.view;
            self.history.replace_current(nav);
            return;
        }
        self.history.visit(nav);
        self.show(nav);
        if self.screenshot.is_none() {
            let archive = nav
                .member
                .and_then(|m| self.loaded().and_then(|l| l.ctx.set.get(m)))
                .map(|m| m.path.clone());
            self.persist
                .update(|s| s.set_last_opened(archive, Some(nav.did)));
        }
    }

    /// Makes `nav` current without touching the history.
    fn show(&mut self, nav: Nav) {
        if let Some(old) = self.current.take() {
            old.cancel_all();
        }
        let mesh = self
            .mesh_start
            .take()
            .unwrap_or_else(|| MeshState::new(SceneRequest::default()));
        self.current = Some(Current::new(nav, ChannelMask::RGBA, mesh));
    }

    pub(crate) fn go_back(&mut self) {
        if let Some(&nav) = self.history.back() {
            self.show(nav);
        }
    }

    pub(crate) fn go_forward(&mut self) {
        if let Some(&nav) = self.history.forward() {
            self.show(nav);
        }
    }

    /// Navigates to `did` with its default view (from a link).
    pub(crate) fn open_did(&mut self, did: u32) {
        self.navigate(Nav {
            did,
            member: None,
            view: ViewKind::default_for(did),
        });
    }

    /// Starts the search typed in the search box.
    pub(crate) fn run_search(&mut self) {
        let text = self.search_text.trim().to_owned();
        let query = parse_query(&text);
        if let DidQuery::Exact(did) = query {
            self.open_did(did);
        }
        if matches!(query, DidQuery::Empty) {
            return;
        }
        let Some(loaded) = self.loaded() else {
            self.status = Some("no install is open".into());
            return;
        };
        let indexes: Vec<Arc<ArchiveIndex>> = loaded.indexes.iter().flatten().cloned().collect();
        let names = loaded.names.get();
        let complete = loaded.indexer.is_complete();
        let n = self.notifier();
        self.searching = Some(text.clone());
        self.search = Some(Search::spawn(text, query, indexes, names, complete, n));
    }

    fn poll_background(&mut self) {
        self.poll_install();
        if let Some(d) = &self.discovering
            && let Some(found) = d.poll()
        {
            self.discovering = None;
            if self.dir_input.is_empty()
                && let Some(first) = found.first()
            {
                self.dir_input = first.display().to_string();
            }
            self.discovered = Some(found);
        }
        if let Install::Open(l) = &mut self.install {
            for idx in l.indexer.poll() {
                if let Some(slot) = l.indexes.get_mut(idx.member) {
                    *slot = Some(Arc::new(idx));
                }
            }
            l.stores.drain();
        }
        if let Some(v) = &mut self.viewport {
            v.poll(&self.egui_ctx);
        }
        if let Some(s) = &self.search
            && let Some(r) = s.poll()
        {
            self.search = None;
            self.searching = None;
            self.list_selected = None;
            self.list = ListSource::Search(Arc::new(r));
        }
        if let Some(q) = &self.pending_search
            && let Some(l) = self.loaded()
        {
            let ready = match parse_query(q) {
                DidQuery::Name(_) => l.names.is_finished(),
                _ => l.indexer.is_complete(),
            };
            if ready {
                self.search_text = q.clone();
                self.pending_search = None;
                self.run_search();
            }
        }
        if matches!(self.list, ListSource::None) && self.current.is_some() {
            self.reveal_current();
        }
        self.persist.tick();
    }

    /// Whether background work is under way (the UI then polls with a short timer).
    fn busy(&self) -> bool {
        let opening = matches!(self.install, Install::Opening(_));
        let workers = self.loaded().is_some_and(|l| {
            let s = l.stores.pool.stats();
            s.queued + s.busy > 0 || !l.indexer.is_complete()
        });
        opening
            || workers
            || self.search.is_some()
            || self.discovering.is_some()
            || self.persist.is_dirty()
            || self.viewport.as_ref().is_some_and(Viewport::waiting)
            || self.screenshot.as_ref().is_some_and(|s| !s.done)
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let typing = ctx.egui_wants_keyboard_input();
        let (back, fwd, find, up, down) = ctx.input(|i| {
            let alt = i.modifiers.alt;
            (
                (alt && i.key_pressed(Key::ArrowLeft))
                    || i.pointer.button_pressed(egui::PointerButton::Extra1),
                (alt && i.key_pressed(Key::ArrowRight))
                    || i.pointer.button_pressed(egui::PointerButton::Extra2),
                i.modifiers.command && i.key_pressed(Key::F),
                !alt && i.key_pressed(Key::ArrowUp),
                !alt && i.key_pressed(Key::ArrowDown),
            )
        });
        if back {
            self.go_back();
        }
        if fwd {
            self.go_forward();
        }
        if find {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("did-search")));
        }
        if !typing && (up || down) {
            self.step_list(if down { 1 } else { -1 });
        }
    }

    /// Shows the current DID in the archive tree: selects its type group in the DID
    /// list (in the archive it is read from, else the first that holds it). Returns
    /// whether it was found in an index.
    pub(crate) fn reveal_current(&mut self) -> bool {
        let Some(cur) = &self.current else {
            return false;
        };
        let (did, pinned) = (cur.nav.did, cur.nav.member);
        let Some(l) = self.loaded() else { return false };
        let found = l
            .indexes
            .iter()
            .flatten()
            .filter(|i| pinned.is_none_or(|m| m == i.member))
            .find_map(|idx| {
                let pos = idx.entries.binary_search_by_key(&did, |e| e.did).ok()?;
                let g = idx.group_of(pos)?;
                Some((idx.member, g, pos - idx.groups[g].start))
            });
        let Some((member, group, row)) = found else {
            return false;
        };
        self.list = ListSource::Group { member, group };
        self.list_selected = Some(row);
        self.list_scroll_to = Some(row);
        self.tree_open = Some(member);
        true
    }

    /// Number of rows in the DID list.
    pub(crate) fn list_len(&self) -> usize {
        match &self.list {
            ListSource::None => 0,
            ListSource::Group { member, group } => self
                .loaded()
                .and_then(|l| l.indexes.get(*member)?.as_ref())
                .and_then(|i| i.groups.get(*group))
                .map_or(0, |g| g.len()),
            ListSource::Search(r) => r.len(),
        }
    }

    /// List row `row`, ready to draw.
    pub(crate) fn list_row_view(&self, row: usize) -> Option<ListRow> {
        let indexes: &[Option<Arc<ArchiveIndex>>] =
            self.loaded().map_or(&[], |l| l.indexes.as_slice());
        list_row_of(&self.list, indexes, row, |m| {
            self.loaded().map(|l| l.ctx.member_name(m))
        })
    }

    /// The (DID, member) of list row `row`; `None` also for a row without a DID.
    pub(crate) fn list_row(&self, row: usize) -> Option<(u32, Option<usize>)> {
        let r = self.list_row_view(row)?;
        Some((r.did?, r.member))
    }

    /// Moves the list selection by `delta` rows and opens the row (rows without a DID
    /// are selected but open nothing). Returns whether the selection moved.
    fn step_list(&mut self, delta: isize) -> bool {
        let len = self.list_len();
        if len == 0 {
            return false;
        }
        let next = match self.list_selected {
            None => 0,
            Some(i) => i.saturating_add_signed(delta).min(len - 1),
        };
        let moved = self.list_selected != Some(next);
        self.list_selected = Some(next);
        self.list_scroll_to = Some(next);
        if let Some((did, member)) = self.list_row(next) {
            let view = self
                .current
                .as_ref()
                .map(|c| c.nav.view)
                .filter(|v| v.applies_to(did))
                .unwrap_or_else(|| ViewKind::default_for(did));
            self.navigate(Nav { did, member, view });
        }
        moved
    }

    /// A click on list row `row`: selects it and opens its DID in the current view if
    /// that view applies (other than Info and Hex), else in the DID's default view. A row
    /// without a DID (a property name) opens nothing. Returns whether a DID was opened.
    pub(crate) fn activate_row(&mut self, row: usize) -> bool {
        let Some(r) = self.list_row_view(row) else {
            return false;
        };
        self.list_selected = Some(row);
        let Some(did) = r.did else { return false };
        let view = self
            .current
            .as_ref()
            .map(|c| c.nav.view)
            .filter(|v| v.applies_to(did) && *v != ViewKind::Info && *v != ViewKind::Hex)
            .unwrap_or_else(|| ViewKind::default_for(did));
        self.navigate(Nav {
            did,
            member: r.member,
            view,
        });
        true
    }

    // -----------------------------------------------------------------------------------
    // Panels
    // -----------------------------------------------------------------------------------

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("top-bar").show(ui, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(self.history.can_back(), egui::Button::new("◀"))
                    .on_hover_text("Back (Alt+Left)")
                    .clicked()
                {
                    self.go_back();
                }
                if ui
                    .add_enabled(self.history.can_forward(), egui::Button::new("▶"))
                    .on_hover_text("Forward (Alt+Right)")
                    .clicked()
                {
                    self.go_forward();
                }
                let edit = egui::TextEdit::singleline(&mut self.search_text)
                    .id(egui::Id::new("did-search"))
                    .hint_text("DID (0x41000123), prefix (0x41, 4100*) or name")
                    .desired_width(320.0);
                let r = ui.add(edit);
                if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    self.run_search();
                }
                if ui.button("Search").clicked() {
                    self.run_search();
                }
                if let Some(q) = &self.searching {
                    ui.spinner();
                    ui.label(format!("searching “{q}”"));
                }
                ui.separator();
                self.bookmarks_menu(ui);
                self.recent_menu(ui);
                ui.menu_button("Install", |ui| {
                    ui.label("Install directory:");
                    ui.text_edit_singleline(&mut self.dir_input);
                    if ui.button("Open").clicked() && !self.dir_input.trim().is_empty() {
                        let d = PathBuf::from(self.dir_input.trim());
                        self.open_install(d);
                        ui.close();
                    }
                    let known: Vec<PathBuf> = self
                        .persist
                        .settings
                        .game_dirs()
                        .into_iter()
                        .map(PathBuf::from)
                        .collect();
                    if !known.is_empty() {
                        ui.separator();
                        ui.label("Known installs:");
                        for d in known {
                            if ui.button(d.display().to_string()).clicked() {
                                self.dir_input = d.display().to_string();
                                self.open_install(d);
                                ui.close();
                            }
                        }
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_overlay, "Frame-time overlay");
                    ui.checkbox(&mut self.show_scale_preview, "UI scale preview");
                });
            });
            ui.add_space(2.0);
        });
    }

    fn bookmarks_menu(&mut self, ui: &mut egui::Ui) {
        let current = self.current.as_ref().map(|c| c.nav.did);
        let mut toggle = None;
        let mut open = None;
        ui.menu_button("Bookmarks", |ui| {
            if let Some(did) = current {
                let marked = self.persist.is_bookmarked(did);
                let text = if marked {
                    format!("★ Remove {did:#010x}")
                } else {
                    format!("☆ Bookmark {did:#010x}")
                };
                if ui.button(text).clicked() {
                    toggle = Some(did);
                    ui.close();
                }
                ui.separator();
            }
            if self.persist.settings.explorer.bookmarks.is_empty() {
                ui.label(RichText::new("no bookmarks").weak());
            }
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    for b in &self.persist.settings.explorer.bookmarks {
                        if ui.button(format!("{:#010x}  {}", b.did, b.label)).clicked() {
                            open = Some(b.did);
                            ui.close();
                        }
                    }
                });
        });
        if let Some(did) = toggle {
            let label = Did(did).kind().name().to_owned();
            self.persist.toggle_bookmark(did, &label);
        }
        if let Some(did) = open {
            self.open_did(did);
        }
    }

    fn recent_menu(&mut self, ui: &mut egui::Ui) {
        let mut open = None;
        ui.menu_button("Recent", |ui| {
            if self.persist.settings.explorer.recent_dids.is_empty() {
                ui.label(RichText::new("nothing yet").weak());
            }
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    for &did in &self.persist.settings.explorer.recent_dids {
                        if ui
                            .button(format!("{did:#010x}  {}", Did(did).kind().name()))
                            .clicked()
                        {
                            open = Some(did);
                            ui.close();
                        }
                    }
                });
        });
        if let Some(did) = open {
            self.open_did(did);
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status-bar").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let label = match &self.install {
                    Install::Open(l) => l.ctx.dir.display().to_string(),
                    Install::Opening(o) => format!("opening {} …", o.dir.display()),
                    Install::Failed { dir, .. } => format!("failed: {}", dir.display()),
                    Install::None => "no install".into(),
                };
                ui.label(RichText::new(label).weak());
                ui.separator();
                if let Some(l) = self.loaded() {
                    let (done, total) = l.indexer.counts();
                    if done < total {
                        ui.spinner();
                    }
                    ui.label(format!(
                        "{} archives · indexed {done}/{total} ({} entries)",
                        total,
                        thousands(l.indexer.scanned_total())
                    ));
                    let ps = l.stores.pool.stats();
                    ui.separator();
                    ui.label(format!(
                        "workers {} · busy {} · queued {}",
                        ps.workers, ps.busy, ps.queued
                    ));
                    if let Some(loc) = l.ctx.strings.as_ref().and_then(|s| s.locale()) {
                        ui.separator();
                        ui.label(format!("strings: {} ({})", loc.name, loc.language_id));
                    }
                    let names = l.names.state();
                    if !names.is_finished() {
                        ui.separator();
                        ui.label("loading names …");
                    }
                }
                if let Some(s) = &self.status {
                    ui.separator();
                    ui.label(RichText::new(s).color(Color32::from_rgb(0xE0, 0xA0, 0x40)));
                }
            });
        });
    }

    fn archive_tree(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("archive-tree")
            .default_size(250.0)
            .show(ui, |ui| {
                ui.heading("Archives");
                ui.separator();
                let Some(l) = self.loaded() else {
                    ui.label(RichText::new("no install open").weak());
                    return;
                };
                let selected_group = match self.list {
                    ListSource::Group { member, group } => Some((member, group)),
                    _ => None,
                };
                let tree_open = self.tree_open;
                let mut select = None;
                egui::ScrollArea::vertical()
                    .id_salt("tree-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for i in 0..l.ctx.set.members().len() {
                            let open = tree_open == Some(i);
                            if let Some(g) = archive_node(ui, l, i, open, selected_group) {
                                select = Some((i, g));
                            }
                        }
                        archive_problems(ui, l);
                    });
                if let Some((member, group)) = select {
                    self.list = ListSource::Group { member, group };
                    self.list_selected = None;
                }
                self.tree_open = None;
            });
    }

    fn did_list(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("did-list")
            .default_size(260.0)
            .show(ui, |ui| {
                let header = match &self.list {
                    ListSource::None => "DIDs".to_owned(),
                    ListSource::Group { member, group } => {
                        let name = self
                            .loaded()
                            .map(|l| l.ctx.member_name(*member))
                            .unwrap_or_default();
                        let prefix = self
                            .loaded()
                            .and_then(|l| l.indexes.get(*member)?.as_ref())
                            .and_then(|i| i.groups.get(*group))
                            .map_or(0, |g| g.prefix);
                        format!("{name} · {prefix:02X}")
                    }
                    ListSource::Search(r) => format!("“{}”", r.query),
                };
                ui.heading(header);
                let len = self.list_len();
                ui.label(RichText::new(format!("{} rows", thousands(len as u64))).weak());
                if let ListSource::Search(r) = &self.list {
                    if let Some(note) = &r.note {
                        ui.label(
                            RichText::new(note)
                                .small()
                                .color(Color32::from_rgb(0xE0, 0xB0, 0x60)),
                        );
                    }
                    if r.truncated {
                        ui.label(RichText::new("results truncated").small());
                    }
                }
                ui.separator();
                if len == 0 {
                    ui.label(
                        RichText::new("Pick a type group in the archive tree, or search.").weak(),
                    );
                    return;
                }
                let row_h = ui.text_style_height(&egui::TextStyle::Monospace);
                let spacing = ui.spacing().item_spacing.y;
                let mut scroll = egui::ScrollArea::vertical()
                    .id_salt("did-list-scroll")
                    .auto_shrink([false, false]);
                if let Some(row) = self.list_scroll_to.take() {
                    let offset = (row as f32 * (row_h + spacing) - 120.0).max(0.0);
                    scroll = scroll.vertical_scroll_offset(offset);
                }
                let current = self.current.as_ref().map(|c| (c.nav.did, c.nav.member));
                let mut clicked = None;
                scroll.show_rows(ui, row_h, len, |ui, range| {
                    // Rows must stay one line high for the virtualized layout.
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                    for row in range {
                        let Some(r) = self.list_row_view(row) else {
                            continue;
                        };
                        let text = RichText::new(&r.text).monospace();
                        let Some(did) = r.did else {
                            // Nothing to open (a property name without a DID).
                            let label = ui.label(text.weak());
                            if let Some(d) = &r.detail {
                                label.on_hover_text(d);
                            }
                            continue;
                        };
                        let selected = self.list_selected == Some(row)
                            || current == Some((did, r.member))
                            || (r.member.is_none() && current.map(|c| c.0) == Some(did));
                        let mut resp = ui.selectable_label(selected, text);
                        if let Some(d) = &r.detail {
                            resp = resp.on_hover_text(d);
                        }
                        if resp.clicked() {
                            clicked = Some(row);
                        }
                    }
                });
                if let Some(row) = clicked {
                    self.activate_row(row);
                }
            });
    }

    fn welcome(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Tessera Explorer");
            ui.label(concat!(
                "Browse the archives of an install read-only: ",
                "textures, icons, strings, property sets."
            ));
            ui.add_space(12.0);
            match &self.install {
                Install::Opening(o) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!("Opening {} …", o.dir.display()));
                    });
                    return;
                }
                Install::Failed { dir, error } => {
                    ui.colored_label(
                        Color32::LIGHT_RED,
                        format!("Could not open {}: {error}", dir.display()),
                    );
                    ui.add_space(8.0);
                }
                _ => {}
            }
            ui.label("Install directory (the folder holding client_general.dat):");
            let mut open = None;
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.dir_input).desired_width(480.0));
                if ui.button("Open").clicked() && !self.dir_input.trim().is_empty() {
                    open = Some(PathBuf::from(self.dir_input.trim()));
                }
            });
            let known: Vec<PathBuf> = self
                .persist
                .settings
                .game_dirs()
                .into_iter()
                .map(PathBuf::from)
                .collect();
            if !known.is_empty() {
                ui.add_space(8.0);
                ui.label("Known installs:");
                for d in known {
                    if ui.link(d.display().to_string()).clicked() {
                        open = Some(d);
                    }
                }
            }
            ui.add_space(8.0);
            match (&self.discovering, &self.discovered) {
                (Some(_), _) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Looking for installs …");
                    });
                }
                (None, Some(found)) => {
                    if found.is_empty() {
                        ui.label(RichText::new("No install found automatically.").weak());
                    } else {
                        ui.label("Found:");
                        for d in found.clone() {
                            if ui.link(d.display().to_string()).clicked() {
                                open = Some(d);
                            }
                        }
                    }
                    if ui.button("Search again").clicked() {
                        self.start_discovery();
                    }
                }
                (None, None) => {
                    if ui.button("Find installs").clicked() {
                        self.start_discovery();
                    }
                }
            }
            if let Some(d) = open {
                self.dir_input = d.display().to_string();
                self.open_install(d);
            }
        });
    }

    fn overlay(&mut self, ctx: &egui::Context) {
        if !self.show_overlay {
            return;
        }
        let stats = &self.frames;
        egui::Area::new(egui::Id::new("frame-overlay"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-8.0, -30.0])
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| overlay_body(ui, stats));
            });
    }

    /// The install directories a screenshot must not be written into: the one being
    /// opened, open or failed, the configured one, `TESSERA_GAME_DIR` and the saved one
    /// (whether or not the install opened).
    pub(crate) fn install_guards(&self) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = Vec::new();
        match &self.install {
            Install::Open(l) => dirs.push(l.ctx.dir.clone()),
            Install::Opening(o) => dirs.push(o.dir.clone()),
            Install::Failed { dir, .. } => dirs.push(dir.clone()),
            Install::None => {}
        }
        dirs.extend(self.config.game_dir.clone());
        dirs.extend(std::env::var_os(tessera_install::GAME_DIR_ENV).map(PathBuf::from));
        dirs.extend(self.persist.settings.game.active_dir.clone());
        dirs.retain(|d| !d.as_os_str().is_empty());
        dirs.dedup();
        dirs
    }

    /// One `--browse` step: moves the DID list down one row, once there is a list (the
    /// install open and the selection's archive indexed). Returns whether the frame went
    /// to browsing. Browsing ends when the rows are used up, the list ends, the install
    /// failed to open, or the screenshot timeout passes (which counts as a timeout).
    pub(crate) fn browse_step(&mut self) -> bool {
        let failed = matches!(self.install, Install::Failed { .. });
        let has_list = self.list_len() > 0;
        let Some(st) = &mut self.screenshot else {
            return false;
        };
        if st.browse_left == 0 {
            return false;
        }
        if failed {
            st.browse_left = 0;
            return false;
        }
        if st.started.elapsed() > st.plan.timeout {
            tracing::warn!(
                stepped = st.browsed,
                left = st.browse_left,
                "screenshot: --browse did not finish before the timeout"
            );
            st.browse_left = 0;
            st.timed_out = true;
            return false;
        }
        if !has_list {
            // Nothing to step through yet; wait without using up rows.
            return true;
        }
        // Like holding the Down key: one row per frame, ready or not.
        let moved = self.step_list(1);
        let Some(st) = &mut self.screenshot else {
            return false;
        };
        if moved {
            st.browse_left -= 1;
            st.browsed += 1;
        } else {
            tracing::info!(
                stepped = st.browsed,
                "screenshot: --browse reached the end of the list"
            );
            st.browse_left = 0;
        }
        true
    }

    /// Whether the screenshot can be taken: the selection is ready, or the install
    /// failed to open (the window then shows the error).
    fn capture_ready(&self) -> bool {
        matches!(self.install, Install::Failed { .. }) || self.view_ready()
    }

    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let ready = self.capture_ready();
        let open_failed = matches!(self.install, Install::Failed { .. });
        let guards = self.install_guards();
        let Some(st) = &mut self.screenshot else {
            return;
        };
        if st.done {
            return;
        }
        // A captured frame arrives as an input event.
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(Arc::clone(image)),
                _ => None,
            })
        });
        if let Some(image) = shot {
            st.done = true;
            if let Some(size) = st.plan.size
                && !capture_matches(image.size, size, ctx.pixels_per_point())
            {
                tracing::warn!(
                    captured_w = image.size[0],
                    captured_h = image.size[1],
                    requested_w = size[0],
                    requested_h = size[1],
                    ppp = ctx.pixels_per_point(),
                    "screenshot: the window does not have the requested size (a window \
                     manager or Wayland compositor chose another; for headless runs use \
                     xvfb-run with an X display)"
                );
            }
            let (path, timed_out) = (st.plan.path.clone(), st.timed_out);
            let exit = Arc::clone(&self.exit_code);
            // Encode off the UI thread; `on_exit` waits for it.
            let writer = std::thread::Builder::new()
                .name("explorer-screenshot".into())
                .spawn(move || {
                    let outcome = (open_failed, timed_out);
                    write_screenshot(&path, &image, &guards, &exit, outcome);
                });
            match writer {
                Ok(h) => st.writer = Some(h),
                Err(e) => {
                    tracing::error!(error = %e, "could not start the screenshot writer");
                    self.exit_code
                        .store(EXIT_SCREENSHOT_FAILED, Ordering::Relaxed);
                }
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if st.requested {
            return;
        }
        if self.browse_step() {
            ctx.request_repaint();
            return;
        }
        let Some(st) = &mut self.screenshot else {
            return;
        };
        if open_failed {
            if st.ready_frames == 0 {
                tracing::error!("screenshot: the install did not open; capturing the error");
            }
            st.ready_frames += 1;
        } else if ready {
            st.ready_frames += 1;
        } else if st.started.elapsed() > st.plan.timeout {
            tracing::warn!("screenshot: selection not ready before the timeout");
            st.timed_out = true;
            st.ready_frames = SCREENSHOT_SETTLE_FRAMES;
        }
        if st.ready_frames >= SCREENSHOT_SETTLE_FRAMES {
            st.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint();
    }

    /// Whether the install is open, the selection's view has its data, and the
    /// archive indexes are done (or 20 s have passed). Used by screenshot mode.
    fn view_ready(&self) -> bool {
        let Some(l) = self.loaded() else { return false };
        let indexed = l.indexer.is_complete()
            || self
                .screenshot
                .as_ref()
                .is_some_and(|s| s.started.elapsed() > Duration::from_secs(20));
        if !indexed || self.pending_search.is_some() || self.search.is_some() {
            return false;
        }
        let Some(cur) = &self.current else {
            return self.pending.is_none();
        };
        fn fin<T>(h: &Option<Handle<T>>) -> bool {
            h.as_ref().is_some_and(Handle::is_finished)
        }
        match cur.nav.view {
            ViewKind::Info | ViewKind::Hex => fin(&cur.raw),
            ViewKind::Texture => {
                fin(&cur.texture)
                    && cur
                        .texture
                        .as_ref()
                        .and_then(Handle::get)
                        .is_none_or(|p| p.image.is_none() || cur.gpu.contains_key(&texture_key(&p)))
            }
            ViewKind::Strings => fin(&cur.strings),
            ViewKind::Props => fin(&cur.props),
            ViewKind::Icon => fin(&cur.icon),
            ViewKind::Record => fin(&cur.record),
            ViewKind::Mesh => crate::mesh_view::mesh_ready(cur, self.viewport.as_ref()),
            ViewKind::Character => {
                crate::anim_view::character_ready(&self.character, self.viewport.as_ref())
            }
            ViewKind::Landblock => fin(&cur.world.landblock),
            ViewKind::Sky => crate::world_view::sky_ready(&cur.world),
            ViewKind::Gameplay => cur.gameplay.is_finished(),
        }
    }

    /// The checkerboard texture (created once).
    pub(crate) fn checker(&mut self, ctx: &egui::Context) -> TextureHandle {
        self.checker
            .get_or_insert_with(|| {
                ctx.load_texture(
                    "checkerboard",
                    checkerboard(),
                    TextureOptions {
                        magnification: egui::TextureFilter::Nearest,
                        minification: egui::TextureFilter::Nearest,
                        wrap_mode: egui::TextureWrapMode::Repeat,
                        mipmap_mode: None,
                    },
                )
            })
            .clone()
    }

    fn remember_window(&mut self, ctx: &egui::Context) {
        if self.screenshot.is_some() {
            return;
        }
        let (size, maximized) = ctx.input(|i| {
            let v = i.viewport();
            (v.inner_rect.map(|r| r.size()), v.maximized.unwrap_or(false))
        });
        let Some(size) = size else { return };
        let geom = tessera_settings::WindowGeometry {
            x: None,
            y: None,
            width: size.x.round().max(1.0) as u32,
            height: size.y.round().max(1.0) as u32,
            maximized,
            extra: Default::default(),
        };
        let changed = self.persist.settings.window(WINDOW_KEY).is_none_or(|w| {
            w.width != geom.width || w.height != geom.height || w.maximized != geom.maximized
        });
        if changed {
            self.persist.update(|s| {
                s.set_window(WINDOW_KEY, geom);
            });
        }
    }
}

/// The frame-time overlay's text and graph.
fn overlay_body(ui: &mut egui::Ui, stats: &FrameStats) {
    const SLOW_COLOR: Color32 = Color32::from_rgb(0xFF, 0x60, 0x40);
    const FAST_COLOR: Color32 = Color32::from_rgb(0x60, 0xC0, 0x60);
    let slow = stats.slow_frames();
    let color = if slow > 0 {
        Color32::from_rgb(0xFF, 0x80, 0x60)
    } else {
        Color32::from_rgb(0x80, 0xD0, 0x80)
    };
    let text = format!(
        "frame {:5.1} ms · mean {:4.1} · max {:5.1} · worst {:5.1}\n\
         slow (>50 ms): {slow}  (warm-up {})",
        stats.last_ms(),
        stats.recent_mean_ms(),
        stats.recent_max_ms(),
        stats.worst_ms(),
        stats.slow_warmup_frames(),
    );
    ui.label(RichText::new(text).monospace().small().color(color));
    let (rect, _) = ui.allocate_exact_size(egui::vec2(240.0, 36.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 2.0, Color32::from_black_alpha(80));
    let scale = |ms: f32| rect.bottom() - (ms / 60.0).min(1.0) * rect.height();
    let y50 = scale(SLOW_FRAME.as_secs_f32() * 1000.0);
    let red = egui::Stroke::new(1.0, Color32::from_rgb(0xC0, 0x40, 0x40));
    p.hline(rect.x_range(), y50, red);
    let n = stats.recent().len().max(1);
    let dx = rect.width() / n as f32;
    for (i, ms) in stats.recent().enumerate() {
        let x = rect.left() + i as f32 * dx;
        let c = if ms > 50.0 { SLOW_COLOR } else { FAST_COLOR };
        let top = scale(ms).min(rect.bottom() - 2.0);
        p.vline(x, top..=rect.bottom(), egui::Stroke::new(dx.max(1.0), c));
    }
}

/// One archive of the archive tree with its type groups; returns the group clicked.
fn archive_node(
    ui: &mut egui::Ui,
    l: &Loaded,
    i: usize,
    open: bool,
    selected_group: Option<(usize, usize)>,
) -> Option<usize> {
    let id = &l.ctx.set.member(i).id;
    let idx = l.indexes.get(i).and_then(Option::as_ref);
    let title = match idx {
        Some(idx) => format!("{id}  ({})", thousands(idx.entries.len() as u64)),
        None => format!("{id}  (indexing {} …)", thousands(l.indexer.scanned(i))),
    };
    let mut header = egui::CollapsingHeader::new(title).id_salt(("archive", i));
    if open && idx.is_some() {
        header = header.open(Some(true));
    }
    let mut clicked = None;
    header.show(ui, |ui| {
        let Some(idx) = idx else {
            ui.spinner();
            return;
        };
        if let Some(e) = &idx.error {
            ui.colored_label(Color32::LIGHT_RED, format!("walk stopped: {e}"));
        }
        for (g, group) in idx.groups.iter().enumerate() {
            let did = u32::from(group.prefix) << 24;
            let text = format!(
                "{:02X} {}  {}",
                group.prefix,
                Did(did).kind().name(),
                thousands(group.len() as u64)
            );
            let selected = selected_group == Some((i, g));
            if ui.selectable_label(selected, text).clicked() {
                clicked = Some(g);
            }
        }
    });
    clicked
}

/// The archive set's problems, under the archive tree.
fn archive_problems(ui: &mut egui::Ui, l: &Loaded) {
    let problems = l.ctx.set.problems();
    if problems.is_empty() {
        return;
    }
    ui.separator();
    egui::CollapsingHeader::new(format!("problems ({})", problems.len())).show(ui, |ui| {
        for p in problems {
            ui.label(RichText::new(&p.message).small());
        }
    });
}

/// Writes the captured frame (on the screenshot thread) and stores the exit code.
fn write_screenshot(
    path: &std::path::Path,
    image: &egui::ColorImage,
    guards: &[PathBuf],
    exit: &AtomicI32,
    (open_failed, timed_out): (bool, bool),
) {
    let saved = match crate::screenshot::save_png(path, image, guards) {
        Ok(written) => {
            let [w, h] = image.size;
            tracing::info!(path = %written.display(), w, h, "screenshot saved");
            true
        }
        Err(e) => {
            tracing::error!(error = %e, "screenshot failed");
            false
        }
    };
    exit.store(
        screenshot_exit_code(saved, open_failed, timed_out),
        Ordering::Relaxed,
    );
}

/// One row of the DID list, ready to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListRow {
    /// What the row shows.
    pub text: String,
    /// The DID the row opens; `None` for a row that opens nothing (a property name
    /// without a DID).
    pub did: Option<u32>,
    /// The archive to read the DID from (`None`: through routing).
    pub member: Option<usize>,
    /// Hover text.
    pub detail: Option<String>,
}

/// Row `row` of `list`: the DID with its size (type groups), the DID with its archive
/// (hex searches), or the matched name (name searches). `indexes` are the archive
/// indexes by member; `member_name` names an archive.
pub(crate) fn list_row_of(
    list: &ListSource,
    indexes: &[Option<Arc<ArchiveIndex>>],
    row: usize,
    member_name: impl Fn(usize) -> Option<String>,
) -> Option<ListRow> {
    match list {
        ListSource::None => None,
        ListSource::Group { member, group } => {
            let idx = indexes.get(*member)?.as_ref()?;
            let g = idx.groups.get(*group)?;
            let e = idx.entries.get(g.start + row).filter(|_| row < g.len())?;
            let c = if e.is_compressed() { "z" } else { " " };
            Some(ListRow {
                text: format!("{:08X} {c} {:>9}", e.did, human_bytes(u64::from(e.length))),
                did: Some(e.did),
                member: Some(*member),
                detail: None,
            })
        }
        ListSource::Search(r) => {
            let h = r.hit(row)?;
            let text = match (h.text, h.did) {
                (Some(t), Some(d)) => format!("{d:08X} {t}"),
                (Some(t), None) => format!("-------- {t}"),
                (None, Some(d)) => {
                    let archive = h
                        .member
                        .and_then(&member_name)
                        .unwrap_or_else(|| "routed".into());
                    format!("{d:08X} {archive}")
                }
                (None, None) => return None,
            };
            Some(ListRow {
                text,
                did: h.did,
                member: h.member,
                detail: h.detail.map(str::to_owned),
            })
        }
    }
}

/// Name of the GPU texture for a texture product (DID, level shown, channels).
pub(crate) fn texture_key(p: &TextureProduct) -> String {
    format!("tex/{:08x}/{:?}/{:x}", p.did, p.shown, p.mask.bits())
}

impl eframe::App for ExplorerApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let started = Instant::now();
        let ctx = ui.ctx().clone();
        // The previous frame's full CPU time (UI, tessellation, submission).
        if let Some(cpu) = frame.info().cpu_usage {
            let d = Duration::from_secs_f32(cpu.max(0.0));
            if self.frames.record(d) {
                tracing::warn!(
                    frame_ms = format_args!("{:.1}", cpu * 1000.0),
                    warmup = self.frames.in_warmup(),
                    did = self
                        .current
                        .as_ref()
                        .map(|c| format!("{:#010x}", c.nav.did)),
                    "slow frame (over 50 ms)"
                );
            }
        }
        if let Some(prev) = self.last_frame_start.replace(started) {
            let gap = started.duration_since(prev);
            tracing::trace!(gap_ms = gap.as_millis() as u64, "frame interval");
        }

        self.poll_background();
        self.handle_keys(&ctx);

        self.top_bar(ui);
        self.status_bar(ui);
        if self.loaded().is_some() {
            if !self.config.hide_panels {
                self.archive_tree(ui);
                self.did_list(ui);
            }
            crate::views::central(self, ui);
        } else {
            self.welcome(ui);
        }
        self.overlay(&ctx);
        if self.show_scale_preview {
            self.scale_preview.show(&ctx, &mut self.show_scale_preview);
        }
        self.screenshot_step(&ctx);
        if ctx.input(|i| i.viewport().close_requested()) {
            self.remember_window(&ctx);
            self.persist.flush();
        }

        let build = started.elapsed();
        if build > SLOW_FRAME {
            tracing::warn!(
                build_ms = build.as_millis() as u64,
                "slow UI build (over 50 ms)"
            );
        }
        if self.busy() {
            ctx.request_repaint_after(Duration::from_millis(33));
        }
    }

    fn on_exit(&mut self) {
        self.persist.flush();
        if let Some(h) = self.screenshot.as_mut().and_then(|s| s.writer.take()) {
            let _ = h.join();
        }
        if self.frames.frames() > 0 {
            let browse = self
                .screenshot
                .as_ref()
                .map(|s| (s.browsed, self.config.browse));
            tracing::info!(
                frames = self.frames.frames(),
                slow = self.frames.slow_frames(),
                slow_warmup = self.frames.slow_warmup_frames(),
                worst_ms = format_args!("{:.1}", self.frames.worst_ms()),
                browsed = browse.map(|b| b.0),
                browse_requested = browse.map(|b| b.1),
                "frame statistics"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoders::NameIndex;
    use crate::index::IndexEntry;
    use crate::search::SearchHit;

    fn missing_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "tessera-explorer-no-install-{tag}-{}",
            std::process::id()
        ))
    }

    fn app_with(config: AppConfig) -> ExplorerApp {
        ExplorerApp::new(
            egui::Context::default(),
            config,
            Persist::in_memory(tessera_settings::Settings::default()),
            Arc::new(AtomicI32::new(0)),
        )
    }

    fn app(tag: &str) -> ExplorerApp {
        app_with(AppConfig {
            game_dir: Some(missing_dir(tag)),
            ..AppConfig::default()
        })
    }

    fn screenshot_app(tag: &str, browse: u32) -> ExplorerApp {
        app_with(AppConfig {
            game_dir: Some(missing_dir(tag)),
            screenshot: Some(ScreenshotPlan {
                path: std::env::temp_dir().join("unused.png"),
                timeout: Duration::from_secs(600),
                size: None,
            }),
            browse,
            ..AppConfig::default()
        })
    }

    /// Name hits: a property name without a DID, then a label with one.
    fn name_results() -> ListSource {
        let mut names = NameIndex::default();
        names.push("Item_Name".into(), None, "property 0x10 (Int)".into());
        names.push("ITEMS".into(), Some(0x2800_0001), "root DID map".into());
        ListSource::Search(Arc::new(SearchResults {
            query: "item".into(),
            hits: vec![SearchHit::Name { name: 0 }, SearchHit::Name { name: 1 }],
            names: Some(Arc::new(names)),
            ..SearchResults::default()
        }))
    }

    /// Hex hits over one archive index.
    fn hex_results(dids: &[u32]) -> ListSource {
        let idx = ArchiveIndex::from_entries(
            0,
            "a".into(),
            dids.iter()
                .map(|&did| IndexEntry {
                    did,
                    length: 16,
                    flags: 0,
                })
                .collect(),
        );
        let hits = (0..dids.len() as u32)
            .map(|entry| SearchHit::Entry { index: 0, entry })
            .collect();
        ListSource::Search(Arc::new(SearchResults {
            query: "0x41".into(),
            hits,
            indexes: vec![Arc::new(idx)],
            ..SearchResults::default()
        }))
    }

    #[test]
    fn list_rows_keep_missing_dids_missing() {
        let list = name_results();
        let r = list_row_of(&list, &[], 0, |_| None).unwrap();
        assert_eq!(r.did, None);
        assert!(r.text.starts_with("-------- Item_Name"), "{}", r.text);
        assert_eq!(r.detail.as_deref(), Some("property 0x10 (Int)"));
        let r = list_row_of(&list, &[], 1, |_| None).unwrap();
        assert_eq!(r.did, Some(0x2800_0001));
        assert!(list_row_of(&list, &[], 2, |_| None).is_none());
        let hex = hex_results(&[0x4100_0001]);
        let r = list_row_of(&hex, &[], 0, |m| Some(format!("arch{m}"))).unwrap();
        assert_eq!((r.did, r.member), (Some(0x4100_0001), Some(0)));
        assert_eq!(r.text, "41000001 arch0");
    }

    /// Regression: clicking a name hit without a DID opened DID 0, recorded it in the
    /// history and remembered it as the last DID.
    #[test]
    fn clicking_a_row_without_a_did_opens_nothing() {
        let mut a = app("click");
        a.list = name_results();
        assert_eq!(a.list_row(0), None);
        assert!(!a.activate_row(0));
        assert!(a.current.is_none());
        assert!(a.history.current().is_none());
        assert_eq!(a.persist.settings.explorer.last_did, None);
        assert_eq!(a.list_selected, Some(0));
        assert!(a.activate_row(1));
        assert_eq!(a.current.as_ref().map(|c| c.nav.did), Some(0x2800_0001));
        assert_eq!(a.history.current().map(|n| n.did), Some(0x2800_0001));
        assert!(!a.activate_row(7));
    }

    /// Regression: the history survived a switch of install, and its archive positions
    /// were replayed against the new install's set.
    #[test]
    fn switching_installs_clears_the_history() {
        let mut a = app("switch");
        for did in [0x4100_0001, 0x4100_0002] {
            a.navigate(Nav {
                did,
                member: Some(3),
                view: ViewKind::Texture,
            });
        }
        assert!(a.history.can_back());
        a.list = hex_results(&[0x4100_0001]);
        a.list_selected = Some(0);
        a.open_install(missing_dir("switch-2"));
        assert!(a.history.current().is_none());
        assert!(!a.history.can_back() && !a.history.can_forward());
        assert!(a.current.is_none() && a.pending.is_none());
        assert!(matches!(a.list, ListSource::None));
        assert_eq!(a.list_selected, None);
        a.go_back();
        assert!(a.current.is_none());
    }

    /// `--select` and `--search` survive the first `open_install` of `new`.
    #[test]
    fn startup_selection_is_kept() {
        let a = app_with(AppConfig {
            game_dir: Some(missing_dir("startup")),
            select: Some(0x4100_0001),
            search: Some("0x41".into()),
            ..AppConfig::default()
        });
        assert_eq!(a.pending.map(|n| n.did), Some(0x4100_0001));
        assert_eq!(a.pending_search.as_deref(), Some("0x41"));
    }

    /// Regression: `--browse` used up a row per frame before there was a list.
    #[test]
    fn browsing_waits_for_a_list_and_counts_real_steps() {
        let mut a = screenshot_app("browse", 5);
        for _ in 0..10 {
            assert!(a.browse_step());
        }
        let st = a.screenshot.as_ref().unwrap();
        assert_eq!((st.browse_left, st.browsed), (5, 0));
        a.list = hex_results(&[0x4100_0001, 0x4100_0002, 0x4100_0003]);
        while a.browse_step() {}
        let st = a.screenshot.as_ref().unwrap();
        // Three rows, then the end of the list.
        assert_eq!((st.browse_left, st.browsed), (0, 3));
        assert!(!st.timed_out);
        assert_eq!(a.current.as_ref().map(|c| c.nav.did), Some(0x4100_0003));
    }

    #[test]
    fn a_failed_open_is_captured_at_once() {
        let dir = missing_dir("failed");
        let mut a = screenshot_app("failed", 3);
        // The install is opening: its directory is guarded already.
        assert!(a.install_guards().contains(&dir));
        let start = Instant::now();
        while !matches!(a.install, Install::Failed { .. }) {
            a.poll_background();
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "open did not fail"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(a.install_guards().contains(&dir));
        assert!(a.capture_ready());
        assert!(!a.browse_step());
    }

    #[test]
    fn capture_size_check() {
        assert!(capture_matches([1600, 900], [1600.0, 900.0], 1.0));
        assert!(capture_matches([3200, 1800], [1600.0, 900.0], 2.0));
        assert!(!capture_matches([1007, 691], [1600.0, 900.0], 1.0));
    }

    #[test]
    fn screenshot_exit_codes() {
        assert_eq!(screenshot_exit_code(true, false, false), 0);
        assert_eq!(screenshot_exit_code(true, false, true), EXIT_TIMEOUT);
        assert_eq!(screenshot_exit_code(true, true, true), EXIT_OPEN_FAILED);
        assert_eq!(
            screenshot_exit_code(false, true, false),
            EXIT_SCREENSHOT_FAILED
        );
    }
}
