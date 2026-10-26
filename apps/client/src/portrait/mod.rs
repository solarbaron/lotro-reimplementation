// SPDX-License-Identifier: GPL-3.0-or-later
//! The portraits of the player and target frames (decision 0024 P2; `combat-ui.md` §4.1,
//! §5.1, CU32; `ui-widgets.md` §12.1–§12.4): each `TargetViewport` (`PlayerPortrait` of P4,
//! `OpponentPortrait` of P5) gets a [`ViewportTarget`] of its render-to-texture size (62 × 62
//! with the install), the entity's figure is drawn into it through the shared figure path
//! through the viewport's scene camera ([`scene`]: `Camera_1` of scene 0x21000004 with the
//! install; below), onto the target cleared to the background colour, the opacity mask (a texture
//! list, 0x40000E4B and 0x40000E4C with the install) is applied, and the target is
//! registered as the element's external content. While no figure is drawn (no entity, a
//! figure still loading or one the client cannot draw, a scene without the viewport's
//! camera, a target that could not be created) the element is hidden with its media and
//! children (§12.4).
//!
//! Frame cost: a target is drawn again only when what it shows changes (the entity, its
//! figure as [`PortraitFigures::figure_key`] reports it, the scene and camera key, the scene
//! read, the viewport's ambient and fog values, the size, the background or the mask), never
//! every frame; the UI composes the last drawing. A
//! figure that could not be drawn is tried again every [`RETRY_FRAMES`] frames. A drawn one
//! is checked again every [`RECHECK_FRAMES`] frames until it is complete: [`STABLE_CHECKS`]
//! checks in a row drew no more of it **while** the figure reported itself settled
//! ([`PortraitFigures::settled`]; its parts and textures arrive over several frames, and a
//! texture that arrives late does not change the figure's key). Masks and scenes are read
//! and decoded on worker threads (ARCH §21); masks are uploaded once.
//!
//! Driving it: keep one [`CombatPortraits`] (built with [`CombatPortraits::for_install`])
//! beside the UI host (the client's [`crate::ui_host::client::ClientUi`] keeps one and
//! drives it from `ClientUi::render_combat_portraits` in the windowed and headless loops),
//! call [`CombatPortraits::frame`] (or [`render_combat_portraits`] with
//! the walker) once per frame **before the UI paints the frame's composition list** (in the
//! client, before `ClientUi::step8`; `render.md` §6.10 "Timing"), so that showing and hiding
//! a portrait reach the same frame's list; and call [`CombatPortraits::release`] when the UI
//! is torn down. Called after the paint instead, a show or hide is one frame late; a hidden
//! portrait's texture therefore stays registered until the next call, so a list painted
//! before the hide still draws its last drawing rather than the element's own media alone.
//!
//! The scene, its camera, its lights and the target's own viewport description are read
//! on a worker per (scene, camera key, camera property, viewport type, target) ([`scene`]):
//!
//! - §12.1 step 2: the portrait option ([`CombatPortraits::set_portrait_option`],
//!   [`portrait_option`] over a `[Render]` look-up for `EnablePortraits`): while it is off, an
//!   element with `UI_TargetViewport_DisableObjectCreationFromPreference` draws no figure and
//!   is hidden. **Not fed by the client host yet:** nothing outside the tests sets it, so it
//!   stays on in every run (the windowed host's `[Render]` reading lies outside this module);
////! - §12.1 step 3: the target's `UIViewport_Array` description of the viewport's
//!   `UIViewport_Type` (its entity entries and its lighting scene's are applied to the
//!   scene's elements by name). A description with a `UIViewport_Image` shows no figure
//!   (how the image is drawn is OPEN, §12.3: the portrait is hidden, §12.4). Its starting
//!   idle is recorded only (a still picture, below);
//! - §12.1 step 4: the camera key from the entity's property named by
//!   `UI_TargetViewport_CameraPropertyName`, else `Camera_1` (the scene is read even when the
//!   element stores no camera key of its own);
//! - camera: when an entry of the description changed the camera element (wrote one of its
//!   properties or replaced its placement; an entry that changes nothing does not count),
//!   the scene's camera itself
//!   ([`PortraitScene::figure_camera`]); otherwise the §12.2 Tessera stand-in: the scene
//!   camera's field of view and far plane from [`head_camera`]'s placement (level, in front
//!   of the figure, framing its head and shoulders from the posed figure's joints);
//! - lighting: [`PortraitJob::lighting`] holds the scene's lights in the figure's frame, the
//!   viewport's `UICore_Viewport_AmbientLight` and the fog of U3 (the scene's override, else
//!   the viewport's `UseFog` values) as a lit pass's [`tessera_render::FrameLighting`]
//!   ([`portrait_lighting`]). **Not applied by the walker's figures:**
//!   [`Walker::draw_entity_portrait`] draws the figure unlit with its fixed light (the
//!   walker has no lit draw of an entity's figure alone yet, outside this module); a
//!   [`PortraitFigures`] with a lit draw uses it;
//! - a still picture: the figure's idle animation does not redraw the target;
//! - with the walker, any entity whose character figure is built can be drawn
//!   ([`WalkerFigures`], [`Walker::draw_entity_portrait`]): the player, a placed NPC, another
//!   player. An entity the walker draws as a plain object (no character figure) shows no
//!   portrait (§12.4: nothing to draw).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use tessera_formats::did::Did;
use tessera_props::{GameData, RecordSource};
use tessera_render::{
    Camera, ColorSpace, Filter, Frame, Renderer, TextureId, TextureUpload, ViewportFinish,
    ViewportTarget, Wrap,
};
use tessera_ui::Ui;
use tessera_ui::game::combat::{CombatUi, EntityId, FrameKind, PortraitSpec};

use crate::ui_host::gpu::{ExternalTexture, ExternalTextures};
use crate::walker::Walker;
use crate::walker::characters::FigureStatus;

pub mod mask;
pub mod scene;

pub use mask::{MAX_MASK_SIDE, MaskError, MaskImage, alpha_from_red, has_alpha_channel, load_mask};
pub use scene::{
    PortraitLighting, PortraitScene, PortraitTarget, SceneCamera, SceneRequest, ViewportLook,
    ViewportPids, portrait_lighting, read_portrait_scene,
};

/// External-content id of `PlayerPortrait` (P4); `OpponentPortrait` (P5) is the next one
/// (the client's id space: the paperdoll's are 0x0050_xxxx, the creation viewport's
/// 0x0051_0000).
pub const COMBAT_PORTRAIT_CONTENT_ID: u64 = 0x0052_0000;

/// Frames between two tries of a figure that could not be drawn.
pub const RETRY_FRAMES: u32 = 10;

/// Frames between two checks of a drawn figure that may still be loading parts.
pub const RECHECK_FRAMES: u32 = 15;

/// Checks in a row that draw no more than before, with the figure settled, after which a
/// drawing counts as complete and is not drawn again until what it shows changes.
pub const STABLE_CHECKS: u8 = 3;

/// Most distinct masks kept (Tessera bound; the install has two).
pub const MAX_MASKS: usize = 16;

/// Most scene reads kept, one per (scene, camera key, camera property, viewport type,
/// target) (Tessera bound); the least recently used finished one makes room.
pub const MAX_SCENES: usize = 64;

/// The content id of a frame's portrait.
#[must_use]
pub fn content_id(kind: FrameKind) -> u64 {
    match kind {
        FrameKind::Player => COMBAT_PORTRAIT_CONTENT_ID,
        FrameKind::Target => COMBAT_PORTRAIT_CONTENT_ID + 1,
        FrameKind::SelectionTarget => COMBAT_PORTRAIT_CONTENT_ID + 2,
    }
}

#[derive(Debug)]
enum MaskState<T = TextureId> {
    Loading,
    Ready(T),
    Failed,
}

/// Stores a worker's result for mask `did`: only a `Loading` entry takes it (a result for
/// an entry released, already resolved or never asked for is not stored). Returns the
/// texture of a result that was not stored, for the caller to destroy.
fn accept_mask<T>(
    state: &mut BTreeMap<u32, MaskState<T>>,
    did: u32,
    result: MaskState<T>,
) -> Option<T> {
    match state.get_mut(&did) {
        Some(s @ MaskState::Loading) => {
            *s = result;
            None
        }
        _ => match result {
            MaskState::Ready(t) => Some(t),
            MaskState::Loading | MaskState::Failed => None,
        },
    }
}

/// The masks by DID: read on worker threads, uploaded on the frame thread.
struct Masks {
    source: Option<Arc<dyn RecordSource>>,
    state: BTreeMap<u32, MaskState>,
    tx: Sender<(u32, Result<MaskImage, MaskError>)>,
    rx: Receiver<(u32, Result<MaskImage, MaskError>)>,
}

impl std::fmt::Debug for Masks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Masks")
            .field("source", &self.source.is_some())
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

/// A mask as the frame sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaskNow {
    Pending,
    Ready(TextureId),
    None,
}

impl Masks {
    fn new(source: Option<Arc<dyn RecordSource>>) -> Self {
        let (tx, rx) = channel();
        Self {
            source,
            state: BTreeMap::new(),
            tx,
            rx,
        }
    }

    fn poll(&mut self, renderer: &mut Renderer<'_>) {
        while let Ok((did, result)) = self.rx.try_recv() {
            if !matches!(self.state.get(&did), Some(MaskState::Loading)) {
                // Released or already resolved: nothing to upload.
                continue;
            }
            let state = match result {
                Ok(m) => match renderer.upload_texture(&TextureUpload {
                    label: Some("portrait-mask"),
                    color_space: ColorSpace::Linear,
                    filter: Filter::Linear,
                    wrap: Wrap::Clamp,
                    ..TextureUpload::rgba8(m.width, m.height, &m.rgba)
                }) {
                    Ok(t) => MaskState::Ready(t),
                    Err(e) => {
                        tracing::warn!(did = format_args!("{did:#010x}"), "portrait mask: {e}");
                        MaskState::Failed
                    }
                },
                Err(e) => {
                    tracing::warn!("portrait mask: {e}; drawn without it");
                    MaskState::Failed
                }
            };
            if let Some(t) = accept_mask(&mut self.state, did, state) {
                renderer.destroy_texture(t);
            }
        }
    }

    fn get(&mut self, did: Option<Did>) -> MaskNow {
        let Some(did) = did.filter(|d| d.0 != 0) else {
            return MaskNow::None;
        };
        if let Some(s) = self.state.get(&did.0) {
            return match s {
                MaskState::Loading => MaskNow::Pending,
                MaskState::Ready(t) => MaskNow::Ready(*t),
                MaskState::Failed => MaskNow::None,
            };
        }
        let Some(source) = self.source.clone() else {
            return MaskNow::None;
        };
        if self.state.len() >= MAX_MASKS {
            return MaskNow::None;
        }
        let tx = self.tx.clone();
        let spawned = std::thread::Builder::new()
            .name("portrait-mask".into())
            .spawn(move || {
                let _ = tx.send((did.0, load_mask(&*source, did)));
            });
        match spawned {
            Ok(_) => {
                self.state.insert(did.0, MaskState::Loading);
                MaskNow::Pending
            }
            Err(e) => {
                tracing::warn!("portrait mask worker: {e}");
                self.state.insert(did.0, MaskState::Failed);
                MaskNow::None
            }
        }
    }

    fn release(&mut self, renderer: &mut Renderer<'_>) {
        for (_, s) in std::mem::take(&mut self.state) {
            if let MaskState::Ready(t) = s {
                renderer.destroy_texture(t);
            }
        }
    }
}

/// Reads a viewport's scenes on worker threads: [`InstallScenes`] over the install, or a
/// test's own.
pub trait SceneReader: Send + Sync {
    /// The PIDs of the viewport properties read on the frame thread.
    ///
    /// # Errors
    ///
    /// A text when they cannot be resolved (the §12 defaults then apply).
    fn viewport_pids(&self) -> Result<ViewportPids, String>;

    /// The scene of `request` with its target's description ([`read_portrait_scene`]).
    ///
    /// # Errors
    ///
    /// A text when the scene cannot be read (nothing is drawn, U4 rule 4).
    fn scene(&self, request: &SceneRequest) -> Result<PortraitScene, String>;
}

/// The scene reader over an install ([`read_portrait_scene`] with one shared
/// [`GameData`]).
pub struct InstallScenes {
    gd: GameData<Arc<dyn RecordSource>>,
}

impl std::fmt::Debug for InstallScenes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallScenes").finish_non_exhaustive()
    }
}

impl SceneReader for InstallScenes {
    fn viewport_pids(&self) -> Result<ViewportPids, String> {
        scene::viewport_pids(&self.gd)
    }

    fn scene(&self, request: &SceneRequest) -> Result<PortraitScene, String> {
        read_portrait_scene(&self.gd, request)
    }
}

#[derive(Debug)]
enum SceneState {
    Loading,
    Ready(Arc<PortraitScene>, u64),
    Failed,
}

/// A scene as the frame sees it.
#[derive(Debug, Clone)]
enum SceneNow {
    /// Still being read.
    Pending,
    /// Read, with the generation of the result.
    Ready(Arc<PortraitScene>, u64),
    /// No scene, or the scene could not be read: nothing is drawn (U4 rule 4).
    Failed,
    /// No scene reader (tests and tools): the job carries no scene.
    Unread,
}

/// The viewport PIDs as the frame sees them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PidsNow {
    Pending,
    Ready(ViewportPids),
}

/// What identifies one scene read ([`SceneRequest`] without the instance set itself).
type SceneKey = (u32, u32, u32, u32, Option<(tessera_net::Body, Option<u64>)>);

fn scene_key(r: &SceneRequest) -> SceneKey {
    (
        r.scene.0,
        r.camera_key,
        r.camera_property,
        r.viewport_type,
        r.target.as_ref().map(|t| (t.body, t.instance_key)),
    )
}

type SceneResult = (SceneKey, Result<PortraitScene, String>);

/// The scenes by request: read on worker threads; the oldest finished entries make room
/// past [`MAX_SCENES`].
struct Scenes {
    reader: Option<Arc<dyn SceneReader>>,
    state: BTreeMap<SceneKey, (SceneState, u64)>,
    pids: Option<Result<ViewportPids, ()>>,
    pids_rx: Option<Receiver<Result<ViewportPids, String>>>,
    /// Counter of requests and results (ages and generations).
    clock: u64,
    /// Scene workers started whose result has not arrived yet; not reset by
    /// [`Scenes::release`], so the workers of released entries still count against
    /// [`MAX_SCENES`].
    in_flight: usize,
    tx: Sender<SceneResult>,
    rx: Receiver<SceneResult>,
}

impl std::fmt::Debug for Scenes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scenes")
            .field("reader", &self.reader.is_some())
            .field("state", &self.state)
            .field("pids", &self.pids)
            .finish_non_exhaustive()
    }
}

impl Scenes {
    fn new(reader: Option<Arc<dyn SceneReader>>) -> Self {
        let (tx, rx) = channel();
        Self {
            reader,
            state: BTreeMap::new(),
            pids: None,
            pids_rx: None,
            clock: 0,
            in_flight: 0,
            tx,
            rx,
        }
    }

    fn tick(&mut self) -> u64 {
        self.clock = self.clock.wrapping_add(1);
        self.clock
    }

    fn poll(&mut self) {
        if let Some(rx) = &self.pids_rx {
            match rx.try_recv() {
                Ok(r) => {
                    self.pids = Some(r.map_err(|e| {
                        tracing::warn!("portrait viewport properties: {e}; defaults used");
                    }));
                    self.pids_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.pids = Some(Err(()));
                    self.pids_rx = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        while let Ok((key, result)) = self.rx.try_recv() {
            self.in_flight = self.in_flight.saturating_sub(1);
            let generation = self.tick();
            let Some((s @ SceneState::Loading, _)) = self.state.get_mut(&key) else {
                continue;
            };
            *s = match result {
                Ok(scene) => {
                    if scene.camera.is_none() {
                        tracing::warn!(
                            scene = format_args!("{:#010x}", key.0),
                            camera = format_args!("{:#010x}", scene.camera_key),
                            "portrait scene has no such camera; nothing is drawn"
                        );
                    }
                    SceneState::Ready(Arc::new(scene), generation)
                }
                Err(e) => {
                    tracing::warn!(
                        scene = format_args!("{:#010x}", key.0),
                        "portrait scene: {e}; nothing is drawn"
                    );
                    SceneState::Failed
                }
            };
        }
    }

    /// The viewport PIDs: asked once from the reader on a worker; without a reader, or
    /// when they cannot be resolved, none (the §12 defaults apply).
    fn pids(&mut self) -> PidsNow {
        match &self.pids {
            Some(Ok(p)) => return PidsNow::Ready(*p),
            Some(Err(())) => return PidsNow::Ready(ViewportPids::default()),
            None => {}
        }
        let Some(reader) = self.reader.clone() else {
            return PidsNow::Ready(ViewportPids::default());
        };
        if self.pids_rx.is_some() {
            return PidsNow::Pending;
        }
        let (tx, rx) = channel();
        match std::thread::Builder::new()
            .name("portrait-pids".into())
            .spawn(move || {
                let _ = tx.send(reader.viewport_pids());
            }) {
            Ok(_) => {
                self.pids_rx = Some(rx);
                PidsNow::Pending
            }
            Err(e) => {
                tracing::warn!("portrait viewport properties worker: {e}");
                self.pids = Some(Err(()));
                PidsNow::Ready(ViewportPids::default())
            }
        }
    }

    fn get(&mut self, request: Option<SceneRequest>) -> SceneNow {
        let Some(reader) = self.reader.clone() else {
            return SceneNow::Unread;
        };
        let Some(request) = request else {
            return SceneNow::Failed;
        };
        let key = scene_key(&request);
        let now = self.tick();
        if let Some((s, age)) = self.state.get_mut(&key) {
            *age = now;
            return match s {
                SceneState::Loading => SceneNow::Pending,
                SceneState::Ready(s, g) => SceneNow::Ready(Arc::clone(s), *g),
                SceneState::Failed => SceneNow::Failed,
            };
        }
        if self.state.len() >= MAX_SCENES {
            // Make room: the least recently asked-for finished entry goes.
            let oldest = self
                .state
                .iter()
                .filter(|(_, (s, _))| !matches!(s, SceneState::Loading))
                .min_by_key(|(_, (_, age))| *age)
                .map(|(k, _)| *k);
            match oldest {
                Some(k) => {
                    self.state.remove(&k);
                }
                None => return SceneNow::Pending,
            }
        }
        if self.in_flight >= MAX_SCENES {
            // Workers of released entries are still running.
            return SceneNow::Pending;
        }
        let tx = self.tx.clone();
        let job_key = key;
        let spawned = std::thread::Builder::new()
            .name("portrait-scene".into())
            .spawn(move || {
                // Every worker reports, so that it stops counting as in flight.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    reader.scene(&request)
                }))
                .unwrap_or_else(|_| Err("the scene reader panicked".into()));
                let _ = tx.send((job_key, result));
            });
        match spawned {
            Ok(_) => {
                self.in_flight += 1;
                self.state.insert(key, (SceneState::Loading, now));
                SceneNow::Pending
            }
            Err(e) => {
                tracing::warn!("portrait scene worker: {e}");
                self.state.insert(key, (SceneState::Failed, now));
                SceneNow::Failed
            }
        }
    }

    /// Forgets every entry; workers still running keep counting as in flight until their
    /// results arrive (and are dropped, or fill a new `Loading` entry of the same request).
    fn release(&mut self) {
        self.state.clear();
    }
}

/// One portrait to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct PortraitJob {
    /// The element and its viewport properties.
    pub spec: PortraitSpec,
    /// The entity whose figure is drawn.
    pub entity: EntityId,
    /// The viewport's scene camera after the target's description (module docs);
    /// `None` without a scene reader ([`CombatPortraits::new`]).
    pub camera: Option<SceneCamera>,
    /// The scene as read for this target; `None` without a scene reader.
    pub scene: Option<Arc<PortraitScene>>,
    /// The portrait's lighting (§12.2: the scene's lights, the viewport's ambient and fog,
    /// in the figure's frame); `None` without a scene reader.
    pub lighting: Option<Arc<PortraitLighting>>,
}

/// Draws the figures of portraits (the walker's figure path, or a test's synthetic one).
pub trait PortraitFigures {
    /// What identifies a figure's drawing: when it changes the target is drawn again.
    type Key: Clone + PartialEq + std::fmt::Debug;

    /// The key of `entity`'s figure when it can be drawn now; `None` while it is loading,
    /// failed or cannot be drawn (the portrait is then hidden, `ui-widgets.md` §12.4).
    fn figure_key(&self, entity: EntityId) -> Option<Self::Key>;

    /// The records of the figure `key` identifies, whose properties may hold the target's
    /// viewport description (§12.1 steps 3 and 4). The default: none (no description).
    fn target_of(&self, key: &Self::Key) -> Option<PortraitTarget> {
        let _ = key;
        None
    }

    /// Whether everything `entity`'s figure draws with (its parts and their textures) has
    /// arrived, or failed for good. A drawing is complete only once it was checked while
    /// this was true ([`STABLE_CHECKS`]). The default: always.
    fn settled(&self, entity: EntityId) -> bool {
        let _ = entity;
        true
    }

    /// Draws `job`'s figure into `frame` (a viewport frame: passes with the premultiplied
    /// option, `Pass::premultiplied_alpha`). Returns how much was drawn (for example the
    /// number of mesh draws), `None` or 0 when nothing was: a figure whose parts are still
    /// loading draws more on a later try, so a target is checked again until this stops
    /// growing ([`STABLE_CHECKS`]).
    fn draw(&mut self, frame: &mut Frame, job: &PortraitJob) -> Option<usize>;
}

/// The walker's figures (module docs: any entity with a built character figure, drawn only
/// through a scene camera).
#[derive(Debug)]
pub struct WalkerFigures<'w>(pub &'w mut Walker);

impl PortraitFigures for WalkerFigures<'_> {
    type Key = FigureStatus;

    fn figure_key(&self, entity: EntityId) -> Option<FigureStatus> {
        let id = tessera_net::EntityId(entity);
        // The character screens' figure is not an entity of the world.
        if id == crate::walker::characters::PREVIEW_ID {
            return None;
        }
        let f = self.0.characters().figure_of(id)?;
        matches!(f.state, crate::walker::characters::FigureState::Built { parts, .. } if parts > 0)
            .then_some(f)
    }

    fn target_of(&self, key: &FigureStatus) -> Option<PortraitTarget> {
        Some(PortraitTarget {
            body: key.request.body,
            instance: key.request.instance.clone(),
            instance_key: key.request.instance_key,
        })
    }

    /// The walker's own settled state ([`Walker::settled`]): it covers every part and
    /// texture the walker holds, the figure's among them (the walker reports no narrower
    /// one), so a portrait keeps being checked while the world around it loads.
    fn settled(&self, _entity: EntityId) -> bool {
        self.0.settled()
    }

    fn draw(&mut self, frame: &mut Frame, job: &PortraitJob) -> Option<usize> {
        // U4 rule 4: without the viewport's camera nothing is drawn.
        let scene_camera = job.camera?;
        let id = tessera_net::EntityId(job.entity);
        if id == crate::walker::characters::PREVIEW_ID {
            return None;
        }
        let first = frame.passes().len();
        if !self.0.draw_entity_portrait(frame, id, 0.0) {
            return None;
        }
        let drawn: usize = frame
            .passes()
            .get(first..)
            .unwrap_or_default()
            .iter()
            .map(|p| p.meshes().len())
            .sum();
        // A camera the target's description placed (§12.1 step 3) is used as the scene
        // stands, moved into the figure's frame (the walker draws it at the origin,
        // unturned); otherwise the head-and-shoulders stand-in (§12.2 Tessera rule) from
        // the posed figure's joints, with the scene camera's field of view and far plane.
        let described = job
            .scene
            .as_ref()
            .filter(|s| s.camera_described)
            .and_then(|s| s.figure_camera());
        let camera = match described {
            Some(c) => c,
            None => {
                let bounds = self.0.characters().posed_bounds(
                    id,
                    &|scale| tessera_render::glam::Mat4::from_scale(scale),
                    0.0,
                );
                bounds.and_then(|(lo, hi)| head_camera(lo, hi, &scene_camera))?
            }
        };
        let aspect = frame.aspect();
        for i in first..frame.passes().len() {
            if let Some(p) = frame.pass_mut(i) {
                p.set_camera(&camera, aspect);
            }
        }
        // `job.lighting` is not applied: the walker's portrait draw is unlit (module docs).
        Some(drawn)
    }
}

/// The part of the figure's height the portrait shows, from the top of its joints (Tessera
/// stand-in for the per-target framing, module docs: head and shoulders).
pub const PORTRAIT_SHOWN_HEIGHT: f32 = 0.18;
/// How far above the topmost joint the view reaches, as a part of the figure's height (the
/// head reaches above its joint).
pub const PORTRAIT_HEAD_ROOM: f32 = 0.1;

/// The portrait camera for a figure standing at the origin whose joints span `lo`–`hi`
/// (metres, Z up, facing +Y), with `scene`'s field of view and far plane (module docs):
/// level, looking along −Y at the figure's vertical axis (x = 0, or the box's middle when
/// the box does not reach the axis), from in front of the box's front face (`hi.y`, so that
/// a held item or a cloak swinging behind the figure does not move the camera into its
/// face), at the distance where the shown part fills the view's height there. `None` for
/// an empty or non-finite box or an unusable field of view.
#[must_use]
pub fn head_camera(
    lo: tessera_render::glam::Vec3,
    hi: tessera_render::glam::Vec3,
    scene: &SceneCamera,
) -> Option<Camera> {
    use tessera_render::glam::Vec3;
    if !(lo.is_finite() && hi.is_finite()) {
        return None;
    }
    let fov_degrees = scene.fov_degrees;
    if !(fov_degrees.is_finite() && (1.0..=100.0).contains(&fov_degrees)) {
        return None;
    }
    let h = hi.z - lo.z;
    if h <= 0.05 || h > 100.0 {
        return None;
    }
    let top = hi.z + h * PORTRAIT_HEAD_ROOM;
    let shown = h * (PORTRAIT_SHOWN_HEIGHT + PORTRAIT_HEAD_ROOM);
    let x = if lo.x <= 0.0 && hi.x >= 0.0 {
        0.0
    } else {
        (lo.x + hi.x) * 0.5
    };
    let target = Vec3::new(x, hi.y, top - shown * 0.5);
    let fov = fov_degrees.to_radians();
    let distance = (shown * 0.5) / (fov * 0.5).tan();
    let near = (distance * 0.5).max(scene::PORTRAIT_NEAR);
    let far = scene.far.max(near * 2.0);
    Some(Camera {
        eye: target + Vec3::Y * distance,
        target,
        up: Vec3::Z,
        projection: tessera_render::Perspective {
            fov_y: fov,
            near,
            far,
        },
    })
}

#[derive(Debug, Clone, PartialEq)]
struct DrawKey<K> {
    entity: EntityId,
    figure: K,
    scene: Option<Did>,
    camera: Option<u32>,
    size: (u32, u32),
    background: u32,
    mask: Option<TextureId>,
    /// The generation of the scene read ([`Scenes`]), `None` without a reader.
    scene_read: Option<u64>,
    /// The viewport's own values (ambient, fog, camera property).
    look: ViewportLook,
}

#[derive(Debug)]
struct Slot<K> {
    kind: FrameKind,
    target: Option<ViewportTarget>,
    /// The key of the target's current drawing.
    drawn: Option<DrawKey<K>>,
    /// The size a target could not be created at (not tried again at that size).
    failed_size: Option<(u32, u32)>,
    /// Frames until a figure that was not drawn is tried again.
    retry_in: u32,
    /// What the drawings of this key drew at most ([`PortraitFigures::draw`]).
    measure: usize,
    /// Checks in a row that drew no more, with the figure settled ([`STABLE_CHECKS`]).
    stable: u8,
    /// Whether the drawing is complete (module docs): not drawn again until its key changes.
    complete: bool,
    /// Frames until a drawing that may not be complete is checked again.
    recheck_in: u32,
    /// The target is registered in the external table but the portrait was hidden: it is
    /// unregistered at the next frame (module docs, "Driving it").
    unregister: bool,
    /// The job of the current drawing ([`CombatPortraits::drawn_job`]).
    job: Option<PortraitJob>,
}

impl<K> Slot<K> {
    fn new(kind: FrameKind) -> Self {
        Self {
            kind,
            target: None,
            drawn: None,
            failed_size: None,
            retry_in: 0,
            measure: 0,
            stable: 0,
            complete: false,
            recheck_in: 0,
            unregister: false,
            job: None,
        }
    }
}

/// Counters of [`CombatPortraits`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PortraitStats {
    /// Drawings of a target (viewport frames submitted).
    pub renders: u64,
    /// Drawings in which the figure drew nothing.
    pub empty: u64,
    /// Frames a shown portrait kept its last drawing.
    pub kept: u64,
    /// Targets created.
    pub targets: u64,
}

/// What [`CombatPortraits::frame`] hands one portrait.
#[derive(Debug, Clone, Copy)]
struct SlotInput {
    spec: PortraitSpec,
    entity: Option<EntityId>,
    /// The viewport's own values; `None` while its properties are being resolved.
    look: Option<ViewportLook>,
    /// The portrait option (§12.1 step 2).
    option: bool,
}

/// The portraits of P4 and P5 (module docs).
#[derive(Debug)]
pub struct CombatPortraits<K> {
    slots: [Slot<K>; 2],
    masks: Masks,
    scenes: Scenes,
    stats: PortraitStats,
    /// The portrait option of §12.1 step 2 ([`CombatPortraits::set_portrait_option`]).
    portrait_option: bool,
}

/// The straight linear RGBA of a 0xAARRGGBB colour (stored values, `render.md` §6.10).
#[must_use]
pub fn argb_to_rgba(c: u32) -> [f32; 4] {
    let ch = |shift: u32| f32::from(((c >> shift) & 0xFF) as u8) / 255.0;
    [ch(16), ch(8), ch(0), ch(24)]
}

/// The scene reader over an install ([`InstallScenes`]).
#[must_use]
pub fn install_scene_reader(source: Arc<dyn RecordSource>) -> Arc<dyn SceneReader> {
    Arc::new(InstallScenes {
        gd: GameData::new(source),
    })
}

/// The portrait option of `ui-widgets.md` §12.1 step 2 from a stored `[Render]
/// EnablePortraits` value (§12.1 revision-4 answer, Tessera rule): `False`, compared
/// without regard to case (surrounding white space ignored), turns it off; any other value
/// or none leaves it on.
#[must_use]
pub fn portrait_option_value(stored: Option<&str>) -> bool {
    !stored.is_some_and(|v| v.trim().eq_ignore_ascii_case("false"))
}

/// The portrait option from a look-up of `[Render]` keys ([`portrait_option_value`] of the
/// value `render` gives for [`PORTRAIT_OPTION_KEY`]). A host passes its layered `[Render]`
/// look-up (`--render-pref KEY=VALUE` over the user's `UserPreferences.ini` in windowed
/// runs, `walker.md` §14.10), so both layers are honoured; a look-up that finds nothing
/// leaves the option on. The client host does not call it yet (module docs).
#[must_use]
pub fn portrait_option<'a>(render: impl FnOnce(&str) -> Option<&'a str>) -> bool {
    portrait_option_value(render(PORTRAIT_OPTION_KEY))
}

/// The `[Render]` key of the portrait option (§12.1 step 2, revision-4 answer).
pub const PORTRAIT_OPTION_KEY: &str = "EnablePortraits";

/// Whether a portrait may build its figure (§12.1 step 2): not while its element has
/// `UI_TargetViewport_DisableObjectCreationFromPreference` (`needs_preference`) and the
/// option is off.
#[must_use]
pub fn figure_allowed(needs_preference: bool, option: bool) -> bool {
    option || !needs_preference
}

impl<K: Clone + PartialEq + std::fmt::Debug> CombatPortraits<K> {
    /// No targets yet; masks are read from `source` (none: portraits are drawn unmasked);
    /// no scene reader, so jobs carry no camera (tests and tools; the walker's figures need
    /// one, [`CombatPortraits::for_install`]).
    #[must_use]
    pub fn new(source: Option<Arc<dyn RecordSource>>) -> Self {
        Self {
            slots: [Slot::new(FrameKind::Player), Slot::new(FrameKind::Target)],
            masks: Masks::new(source),
            scenes: Scenes::new(None),
            stats: PortraitStats::default(),
            portrait_option: true,
        }
    }

    /// The client's portraits: masks and scenes read from the install `source`.
    #[must_use]
    pub fn for_install(source: Arc<dyn RecordSource>) -> Self {
        Self::new(Some(Arc::clone(&source))).with_scene_reader(install_scene_reader(source))
    }

    /// Reads the viewports' scenes with `reader`: a portrait is then drawn only through its
    /// scene's camera, and hidden when the scene has none (U4 rule 4).
    #[must_use]
    pub fn with_scene_reader(mut self, reader: Arc<dyn SceneReader>) -> Self {
        self.scenes = Scenes::new(Some(reader));
        self
    }

    /// Sets the portrait option of §12.1 step 2 ([`portrait_option`]; on from the start; the
    /// client host does not call it yet, module docs).
    /// While it is off, a portrait whose element has
    /// `UI_TargetViewport_DisableObjectCreationFromPreference` draws no figure and is hidden
    /// (§12.4); turning it on draws the figure again in the next frame.
    pub fn set_portrait_option(&mut self, on: bool) {
        self.portrait_option = on;
    }

    /// The portrait option (§12.1 step 2).
    #[must_use]
    pub fn portrait_option(&self) -> bool {
        self.portrait_option
    }

    /// Scene reads started whose results the frames have not received yet (diagnostics
    /// and tests: at 0, every read asked for so far has reached [`CombatPortraits::frame`]).
    #[must_use]
    pub fn scene_reads_in_flight(&self) -> usize {
        self.scenes.in_flight
    }

    /// Counters.
    #[must_use]
    pub fn stats(&self) -> PortraitStats {
        self.stats
    }

    /// The target of a frame's portrait (tests and tools).
    #[must_use]
    pub fn target(&self, kind: FrameKind) -> Option<&ViewportTarget> {
        self.slots.iter().find(|s| s.kind == kind)?.target.as_ref()
    }

    /// Whether a frame's portrait shows a drawing now.
    #[must_use]
    pub fn drawn(&self, kind: FrameKind) -> bool {
        self.slots
            .iter()
            .any(|s| s.kind == kind && s.drawn.is_some())
    }

    /// What a frame's portrait drew its current drawing with (its scene read and lighting;
    /// tests and tools), `None` while it shows none.
    #[must_use]
    pub fn drawn_job(&self, kind: FrameKind) -> Option<&PortraitJob> {
        let s = self.slots.iter().find(|s| s.kind == kind)?;
        s.drawn.as_ref().and(s.job.as_ref())
    }

    /// Whether a frame's portrait shows a complete drawing (module docs; tests).
    #[must_use]
    pub fn complete(&self, kind: FrameKind) -> bool {
        self.slots
            .iter()
            .any(|s| s.kind == kind && s.drawn.is_some() && s.complete)
    }

    /// One frame (module docs): for each portrait of `combat`, draws its target when what
    /// it shows changed, registers it in `externals` and gives the element its external
    /// content, or hides the element while no figure is drawn. Call before the UI paints
    /// the frame's composition list (module docs, "Driving it").
    pub fn frame<F: PortraitFigures<Key = K>>(
        &mut self,
        ui: &mut Ui,
        combat: &CombatUi,
        externals: &ExternalTextures,
        renderer: &mut Renderer<'_>,
        figures: &mut F,
    ) {
        self.masks.poll(renderer);
        self.scenes.poll();
        let pids = self.scenes.pids();
        let views = combat.portraits(ui);
        for slot in &mut self.slots {
            let id = content_id(slot.kind);
            if std::mem::take(&mut slot.unregister) {
                externals.set(id, None);
            }
            let Some(v) = views.iter().find(|v| v.spec.kind == slot.kind) else {
                continue;
            };
            if !v.displayed {
                // A hidden panel or vital field: nothing to draw; the element keeps its
                // state.
                continue;
            }
            let look = match pids {
                PidsNow::Ready(p) => {
                    Some(ViewportLook::read(ui, v.spec.element, &p, v.spec.ambient))
                }
                PidsNow::Pending => None,
            };
            let shown = Self::slot_frame(
                slot,
                &mut self.masks,
                &mut self.scenes,
                &mut self.stats,
                SlotInput {
                    spec: v.spec,
                    entity: v.entity,
                    look,
                    option: self.portrait_option,
                },
                renderer,
                figures,
            );
            match shown.then_some(slot.target.as_ref()).flatten() {
                Some(t) => {
                    externals.set(
                        id,
                        Some(ExternalTexture {
                            texture: t.texture(),
                            size: Some(t.size()),
                        }),
                    );
                    combat.set_portrait_content(ui, slot.kind, Some(id));
                }
                None => {
                    // Unregistered at the next frame (module docs).
                    slot.unregister = externals.get(id).is_some();
                    combat.set_portrait_content(ui, slot.kind, None);
                }
            }
        }
    }

    /// One portrait; returns whether its target shows a drawing.
    fn slot_frame<F: PortraitFigures<Key = K>>(
        slot: &mut Slot<K>,
        masks: &mut Masks,
        scenes: &mut Scenes,
        stats: &mut PortraitStats,
        input: SlotInput,
        renderer: &mut Renderer<'_>,
        figures: &mut F,
    ) -> bool {
        let SlotInput {
            spec,
            entity,
            look,
            option,
        } = input;
        // §12.1 step 1 (no entity), step 2 (the portrait option) and §12.4 (no figure that
        // can be drawn).
        let Some(entity) = entity.filter(|_| figure_allowed(spec.needs_preference, option)) else {
            slot.drawn = None;
            return false;
        };
        let Some(figure) = figures.figure_key(entity) else {
            slot.drawn = None;
            return false;
        };
        // While the mask, the viewport's properties or the scene load, a drawing of the same
        // entity is kept (no unmasked square, no other entity's figure).
        let keep = slot.drawn.as_ref().is_some_and(|d| d.entity == entity);
        let mask = match masks.get(spec.mask) {
            MaskNow::Pending => return keep,
            MaskNow::Ready(t) => Some(t),
            MaskNow::None => None,
        };
        let Some(look) = look else {
            return keep;
        };
        // §12.1 step 4 sets the camera key on every update (the entity's camera property,
        // else `Camera_1`), so the scene is read even when the element stores no key of its
        // own (0); the reader resolves the key, and U4 rule 4 hides the portrait when the
        // resulting key names no camera.
        let request = spec.scene.filter(|d| d.0 != 0).map(|scene| SceneRequest {
            scene,
            camera_key: spec.camera.unwrap_or(0),
            camera_property: look.camera_property,
            viewport_type: spec.viewport_type,
            target: figures.target_of(&figure),
        });
        let (scene, scene_read) = match scenes.get(request) {
            SceneNow::Pending => return keep,
            SceneNow::Ready(s, generation) => {
                if s.camera.is_none() {
                    // U4 rule 4: nothing is drawn.
                    slot.drawn = None;
                    return false;
                }
                if s.image.is_some() {
                    // §12.1 step 3: an image in place of the figure; how it is drawn is
                    // OPEN (§12.3), so no figure is drawn (§12.4).
                    slot.drawn = None;
                    return false;
                }
                (Some(s), Some(generation))
            }
            SceneNow::Failed => {
                slot.drawn = None;
                return false;
            }
            SceneNow::Unread => (None, None),
        };
        let key = DrawKey {
            entity,
            figure,
            scene: spec.scene,
            camera: spec.camera,
            size: spec.target_size,
            background: spec.background,
            mask,
            scene_read,
            look,
        };
        let same = slot.drawn.as_ref() == Some(&key);
        if same {
            if slot.complete {
                stats.kept = stats.kept.saturating_add(1);
                return true;
            }
            if slot.recheck_in > 0 {
                slot.recheck_in -= 1;
                stats.kept = stats.kept.saturating_add(1);
                return true;
            }
        } else {
            slot.stable = 0;
            slot.measure = 0;
            slot.complete = false;
        }
        if slot.drawn.is_none() && slot.retry_in > 0 {
            slot.retry_in -= 1;
            return false;
        }
        if slot
            .target
            .as_ref()
            .is_some_and(|t| t.size() != spec.target_size)
            && let Some(old) = slot.target.take()
        {
            renderer.destroy_viewport_target(old);
        }
        if slot.target.is_none() {
            if slot.failed_size == Some(spec.target_size) {
                return false;
            }
            let (w, h) = spec.target_size;
            match renderer.create_viewport_target(w, h, Filter::Linear) {
                Ok(t) => {
                    slot.target = Some(t);
                    slot.failed_size = None;
                    stats.targets = stats.targets.saturating_add(1);
                }
                Err(e) => {
                    tracing::warn!(size = ?spec.target_size, "portrait target: {e}");
                    slot.failed_size = Some(spec.target_size);
                    return false;
                }
            }
        }
        let Some(target) = slot.target.as_ref() else {
            return false;
        };
        // Asked before the drawing: what arrives while it is recorded is seen next time.
        let settled = figures.settled(entity);
        let job = PortraitJob {
            spec,
            entity,
            camera: scene.as_ref().and_then(|s| s.camera),
            lighting: scene
                .as_ref()
                .map(|s| Arc::new(portrait_lighting(s, &look, 0.0))),
            scene,
        };
        let drawn = match renderer.begin_viewport_frame(target) {
            Ok(mut frame) => {
                let drawn = figures.draw(&mut frame, &job).filter(|n| *n > 0);
                let finish = ViewportFinish {
                    background: Some(argb_to_rgba(spec.background)),
                    mask,
                };
                match renderer.end_viewport_frame(frame, &finish) {
                    Ok(_) => drawn,
                    Err(e) => {
                        tracing::debug!("portrait: {e}");
                        None
                    }
                }
            }
            Err(e) => {
                tracing::debug!("portrait: {e}");
                None
            }
        };
        stats.renders = stats.renders.saturating_add(1);
        if let Some(n) = drawn {
            if n > slot.measure || !settled {
                slot.measure = slot.measure.max(n);
                slot.stable = 0;
            } else {
                slot.stable = slot.stable.saturating_add(1);
            }
            slot.complete = slot.stable >= STABLE_CHECKS;
            slot.recheck_in = RECHECK_FRAMES;
            slot.drawn = Some(key);
            slot.job = Some(job);
            true
        } else {
            stats.empty = stats.empty.saturating_add(1);
            slot.drawn = None;
            slot.retry_in = RETRY_FRAMES;
            false
        }
    }

    /// Releases the targets, masks and scenes and unregisters the contents (UI teardown).
    /// Results of workers still running are dropped when they arrive; the portraits can be
    /// driven again afterwards.
    pub fn release(&mut self, renderer: &mut Renderer<'_>, externals: &ExternalTextures) {
        for slot in &mut self.slots {
            externals.set(content_id(slot.kind), None);
            if let Some(t) = slot.target.take() {
                renderer.destroy_viewport_target(t);
            }
            slot.drawn = None;
            slot.complete = false;
            slot.unregister = false;
        }
        self.masks.release(renderer);
        self.scenes.release();
    }
}

/// [`CombatPortraits::frame`] with the UI host's combat widgets and the walker's figures:
/// does nothing before the combat widgets are attached. Call before the UI paints the
/// frame (module docs, "Driving it").
pub fn render_combat_portraits(
    portraits: &mut CombatPortraits<FigureStatus>,
    host: &crate::ui_host::UiHost,
    walker: &mut Walker,
    renderer: &mut Renderer<'_>,
) {
    let Some(hud) = host.combat_hud() else { return };
    let externals = host.textures().external_textures();
    let mut ui = host.ui_mut();
    portraits.frame(
        &mut ui,
        hud.widgets(),
        &externals,
        renderer,
        &mut WalkerFigures(walker),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn camera_1() -> SceneCamera {
        SceneCamera {
            frame: crate::screens::scene::SceneFrame::ORIGIN,
            fov_degrees: 60.0,
            far: 20.0,
        }
    }

    #[test]
    fn the_head_camera_frames_the_top_of_the_figure_with_the_scene_lens() {
        use tessera_render::glam::Vec3;
        let c = head_camera(
            Vec3::new(-0.3, -0.2, 0.0),
            Vec3::new(0.3, 0.2, 1.8),
            &camera_1(),
        )
        .unwrap();
        // Level, in front of the box's front face (+Y), on the figure's axis, aimed into
        // the upper part of the figure.
        assert!((c.eye.z - c.target.z).abs() < 1e-5);
        assert!(c.eye.y > 0.2 && c.target.z > 1.3 && c.target.z < 1.8);
        assert!(c.eye.x.abs() < 1e-6 && (c.target.y - 0.2).abs() < 1e-6);
        // Something swinging far behind the figure does not move the camera.
        let behind = head_camera(
            Vec3::new(-0.6, -0.9, 0.0),
            Vec3::new(0.3, 0.2, 1.8),
            &camera_1(),
        )
        .unwrap();
        assert!((behind.eye - c.eye).length() < 1e-5);
        assert!((c.projection.fov_y - 60f32.to_radians()).abs() < 1e-6);
        assert_eq!(c.projection.far, 20.0);
        assert!(c.projection.near > 0.0 && c.projection.far > c.projection.near);
        assert!(head_camera(Vec3::ZERO, Vec3::ZERO, &camera_1()).is_none());
        assert!(head_camera(Vec3::splat(f32::NAN), Vec3::ONE, &camera_1()).is_none());
        let narrow = SceneCamera {
            fov_degrees: 0.5,
            ..camera_1()
        };
        assert!(head_camera(Vec3::ZERO, Vec3::new(0.3, 0.2, 1.8), &narrow).is_none());
    }

    #[test]
    fn colours_are_straight_rgba() {
        assert_eq!(argb_to_rgba(0xFF00_0000), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(argb_to_rgba(0x80FF_0000), [1.0, 0.0, 0.0, 128.0 / 255.0]);
    }

    #[test]
    fn content_ids_are_distinct() {
        let ids = [
            FrameKind::Player,
            FrameKind::Target,
            FrameKind::SelectionTarget,
        ]
        .map(content_id);
        assert_eq!(ids[0], COMBAT_PORTRAIT_CONTENT_ID);
        assert!(ids[0] != ids[1] && ids[1] != ids[2]);
        assert_ne!(ids[0], crate::screens::ui::VIEWPORT_CONTENT_ID);
        assert_ne!(ids[0], crate::character::panel::PORTRAIT_CONTENT_ID);
    }

    /// A mask result is stored only in a `Loading` entry; any other result's texture is
    /// handed back to be destroyed (a release while the worker ran, a second result).
    #[test]
    fn mask_results_never_overwrite_a_texture() {
        let (t1, t2) = (1u32, 2u32);
        let mut state: BTreeMap<u32, MaskState<u32>> = BTreeMap::new();
        // Never asked for (or released): handed back.
        assert_eq!(accept_mask(&mut state, 7, MaskState::Ready(t1)), Some(t1));
        assert!(state.is_empty());
        state.insert(7, MaskState::Loading);
        assert_eq!(accept_mask(&mut state, 7, MaskState::Ready(t1)), None);
        assert!(matches!(state.get(&7), Some(MaskState::Ready(t)) if *t == t1));
        // A second result for a resolved entry: the new texture goes, the old one stays.
        assert_eq!(accept_mask(&mut state, 7, MaskState::Ready(t2)), Some(t2));
        assert!(matches!(state.get(&7), Some(MaskState::Ready(t)) if *t == t1));
        assert_eq!(accept_mask(&mut state, 7, MaskState::Failed), None);
        assert!(matches!(state.get(&7), Some(MaskState::Ready(t)) if *t == t1));
    }

    #[test]
    fn the_portrait_option_is_off_only_for_false() {
        assert!(portrait_option_value(None));
        assert!(portrait_option_value(Some("True")));
        assert!(portrait_option_value(Some("")));
        assert!(portrait_option_value(Some("0")), "only the word False");
        for off in ["False", "false", "FALSE", " False "] {
            assert!(!portrait_option_value(Some(off)), "{off:?}");
        }
        let ini = tessera_install::Ini::parse("[Render]\nenableportraits = false\n");
        assert!(
            !portrait_option(|k| ini.get("Render", k)),
            "the key is read without regard to case"
        );
        let ini = tessera_install::Ini::parse("[Sound]\nEnablePortraits=False\n");
        assert!(portrait_option(|k| ini.get("Render", k)), "another section");
        assert!(portrait_option(|_| None));
        // A layer above the file (a `--render-pref` value) decides when it holds the key.
        let file = tessera_install::Ini::parse("[Render]\nEnablePortraits=True\n");
        let layered = |k: &str| (k == PORTRAIT_OPTION_KEY).then_some("False");
        assert!(!portrait_option(
            |k| layered(k).or_else(|| file.get("Render", k))
        ));
        assert!(figure_allowed(true, true) && figure_allowed(false, false));
        assert!(!figure_allowed(true, false));
    }

    struct FakeReader {
        calls: std::sync::atomic::AtomicUsize,
    }

    impl SceneReader for FakeReader {
        fn viewport_pids(&self) -> Result<ViewportPids, String> {
            Ok(ViewportPids {
                use_fog: Some(1),
                ..ViewportPids::default()
            })
        }

        fn scene(&self, request: &SceneRequest) -> Result<PortraitScene, String> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if request.camera_key == 0xBAD {
                return Err("unreadable".into());
            }
            Ok(PortraitScene::with_camera(
                request.scene,
                request.camera_key,
                Some(camera_1()),
            ))
        }
    }

    /// A reader whose scene reads wait while `blocked` is set.
    struct BlockingReader {
        blocked: std::sync::atomic::AtomicBool,
        calls: std::sync::atomic::AtomicUsize,
    }

    impl SceneReader for BlockingReader {
        fn viewport_pids(&self) -> Result<ViewportPids, String> {
            Ok(ViewportPids::default())
        }

        fn scene(&self, request: &SceneRequest) -> Result<PortraitScene, String> {
            use std::sync::atomic::Ordering::SeqCst;
            self.calls.fetch_add(1, SeqCst);
            for _ in 0..20_000 {
                if !self.blocked.load(SeqCst) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Ok(PortraitScene::with_camera(
                request.scene,
                request.camera_key,
                Some(camera_1()),
            ))
        }
    }

    /// A release while scene workers run does not let more than [`MAX_SCENES`] of them run
    /// at once: the released workers count until their results arrive.
    #[test]
    fn released_scene_workers_still_bound_new_ones() {
        use std::sync::atomic::Ordering::SeqCst;
        let reader = Arc::new(BlockingReader {
            blocked: std::sync::atomic::AtomicBool::new(true),
            calls: std::sync::atomic::AtomicUsize::new(0),
        });
        let mut scenes = Scenes::new(Some(Arc::clone(&reader) as Arc<dyn SceneReader>));
        for i in 0..MAX_SCENES as u32 {
            assert!(matches!(
                scenes.get(Some(request(0x1E, 0x4900_0000 + i))),
                SceneNow::Pending
            ));
        }
        assert_eq!(scenes.in_flight, MAX_SCENES);
        scenes.release();
        assert!(scenes.state.is_empty());
        // A new request waits: no worker is started while the old ones run.
        let fresh = request(0x1E, 0x4A00_0000);
        for _ in 0..3 {
            scenes.poll();
            assert!(matches!(scenes.get(Some(fresh.clone())), SceneNow::Pending));
        }
        assert!(scenes.state.is_empty(), "not even a Loading entry");
        assert_eq!(scenes.in_flight, MAX_SCENES);
        // The old workers finish (their results dropped): the new request is read.
        reader.blocked.store(false, SeqCst);
        let SceneNow::Ready(..) = settle(&mut scenes, &fresh) else {
            panic!("read after the old workers ended");
        };
        for _ in 0..5000 {
            scenes.poll();
            if scenes.in_flight == 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(scenes.in_flight, 0);
        // Every worker has reported: the old ones and the one new read.
        assert_eq!(reader.calls.load(SeqCst), MAX_SCENES + 1);
        assert_eq!(scenes.state.len(), 1, "released results are not stored");
    }

    fn request(camera_key: u32, body: u32) -> SceneRequest {
        SceneRequest {
            scene: Did(0x2100_0004),
            camera_key,
            camera_property: 0,
            viewport_type: 2,
            target: Some(PortraitTarget {
                body: tessera_net::Body::Object(tessera_net::DataId(body)),
                instance: None,
                instance_key: None,
            }),
        }
    }

    fn settle(scenes: &mut Scenes, r: &SceneRequest) -> SceneNow {
        for _ in 0..2000 {
            scenes.poll();
            match scenes.get(Some(r.clone())) {
                SceneNow::Pending => std::thread::sleep(std::time::Duration::from_millis(1)),
                other => return other,
            }
        }
        panic!("the scene read did not finish");
    }

    #[test]
    fn scenes_are_read_once_per_request_and_make_room() {
        let reader = Arc::new(FakeReader {
            calls: std::sync::atomic::AtomicUsize::new(0),
        });
        let mut scenes = Scenes::new(Some(Arc::clone(&reader) as Arc<dyn SceneReader>));
        // The viewport PIDs arrive from a worker.
        let mut pids = scenes.pids();
        for _ in 0..2000 {
            if pids != PidsNow::Pending {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
            scenes.poll();
            pids = scenes.pids();
        }
        assert_eq!(
            pids,
            PidsNow::Ready(ViewportPids {
                use_fog: Some(1),
                ..ViewportPids::default()
            })
        );
        let a = request(0x1E, 0x4700_0001);
        let SceneNow::Ready(s, g) = settle(&mut scenes, &a) else {
            panic!("read");
        };
        assert_eq!(s.camera_key, 0x1E);
        // Asked again: the same read, no second call.
        let SceneNow::Ready(_, g2) = settle(&mut scenes, &a) else {
            panic!("kept");
        };
        assert_eq!(g, g2);
        assert_eq!(reader.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        // Another target is another read; an unreadable scene draws nothing.
        assert!(matches!(
            settle(&mut scenes, &request(0x1E, 0x4700_0002)),
            SceneNow::Ready(..)
        ));
        assert!(matches!(
            settle(&mut scenes, &request(0xBAD, 0x4700_0002)),
            SceneNow::Failed
        ));
        assert!(matches!(scenes.get(None), SceneNow::Failed));
        // Past the bound the least recently used read goes; `a` was asked for most
        // recently among the first ones and is kept until it is the oldest.
        for i in 0..(MAX_SCENES as u32 + 4) {
            let r = request(0x1E, 0x4800_0000 + i);
            assert!(matches!(settle(&mut scenes, &r), SceneNow::Ready(..)));
            assert!(scenes.state.len() <= MAX_SCENES);
        }
        // Without a reader nothing is read.
        assert!(matches!(Scenes::new(None).get(Some(a)), SceneNow::Unread));
        assert_eq!(scenes.in_flight, 0);
        assert_eq!(
            Scenes::new(None).pids(),
            PidsNow::Ready(ViewportPids::default())
        );
    }
}
