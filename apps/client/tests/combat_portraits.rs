// SPDX-License-Identifier: GPL-3.0-or-later
//! [`CombatPortraits`] on a synthetic P4/P5 tree with synthetic figures and a synthetic
//! texture-list mask (`ui-widgets.md` §12.1–§12.4, `combat-ui.md` §5.1): the portraits stay
//! hidden while no figure is drawn; a drawn figure shows the element, registers its 62 × 62
//! target as external content, holds the figure on the opaque background inside the mask
//! and nothing outside it; the target is not drawn again while nothing changes, and is when
//! the figure changes; a figure that cannot be drawn hides the portrait again (its texture
//! unregistered one frame later). A picture that changes without the figure's key (a texture
//! arriving late) is drawn again while the figure is not settled, even after the stable
//! window. A sized target recorded through [`UiTextures::record`] into a larger layout box
//! is sampled whole, not mirrored, and linearly even with the `nearest` scale filter
//! (`ui-widgets.md` §12.3 step 3).
//!
//! Needs a GPU adapter (skipped, reported as passed, without one); no game data.

#[path = "../../../crates/render/tests/support/gpu.rs"]
mod gpu;

use std::sync::Arc;

use tessera_client::portrait::{
    CombatPortraits, PortraitFigures, PortraitJob, PortraitScene, PortraitTarget, SceneCamera,
    SceneReader, SceneRequest, ViewportPids, content_id,
};
use tessera_client::screens::scene::{SceneFrame, SceneLight};
use tessera_client::ui_host::gpu::UiTextures;
use tessera_formats::did::Did;
use tessera_formats::property::{PropertyEntry, PropertyValue};
use tessera_formats::surface::SurfaceFormat;
use tessera_formats::ui_layout::{ElementDesc, UiLayout};
use tessera_net::{Body, DataId};
use tessera_props::MemorySource;
use tessera_render::LightKind;
use tessera_render::glam::Vec3;
use tessera_render::{Frame, OffscreenFormat, PassSpace, Quad};
use tessera_ui::game::combat::{
    CombatLayouts, CombatRoots, CombatUi, CombatVocab, EntityId, FloatySettings, FrameKind,
    FrameView,
};
use tessera_ui::game::find_below;
use tessera_ui::paint::{
    Addressing, CapCounters, CompositionList, Entry, EntryTexture, Filter as PaintFilter, Rect,
    Source,
};
use tessera_ui::{MemoryEnv, Ui, Vocab};

const L: u32 = 0x2200_0033;
const MASK_LIST: u32 = 0x4000_0E4C;
const MASK_SURFACE: u32 = 0x4100_0E4C;

fn field(id: u32, x: i32, y: i32, w: i32, h: i32) -> ElementDesc {
    let mut d = ElementDesc {
        element_id: id,
        element_type: Vocab::default().types.field,
        ..ElementDesc::default()
    };
    d.block.rect.x = x;
    d.block.rect.y = y;
    d.block.rect.width = w;
    d.block.rect.height = h;
    d.block.has_position = true;
    d
}

fn prop(mut d: ElementDesc, pid: u32, value: PropertyValue) -> ElementDesc {
    d.block
        .properties
        .entries
        .push(PropertyEntry { pid, value });
    d.block.properties.size.count = d.block.properties.entries.len() as u32;
    d
}

fn child(mut d: ElementDesc, c: ElementDesc) -> ElementDesc {
    d.children.push(c);
    d.children_size.count = d.children.len() as u32;
    d
}

fn portrait(id: u32, x: i32, y: i32, use_player: bool) -> ElementDesc {
    let p = CombatVocab::default().pids;
    let mut d = field(id, x, y, 62, 62);
    d = prop(d, p.viewport_scene, PropertyValue::DidRef(Did(0x2100_0004)));
    d = prop(d, p.viewport_width, PropertyValue::Int(62));
    d = prop(d, p.viewport_height, PropertyValue::Int(62));
    d = prop(
        d,
        p.viewport_background,
        PropertyValue::Colour([0, 0, 0, 255]),
    );
    d = prop(d, p.viewport_mask, PropertyValue::DidRef(Did(MASK_LIST)));
    // As in data: the `Camera_1` key, the portrait option gate, the viewport's type, the
    // ambient and the fog values of the synthetic reader's PIDs.
    d = prop(d, p.viewport_camera, PropertyValue::Enum(CAMERA_1));
    d = prop(d, p.viewport_from_preference, PropertyValue::Bool(true));
    d = prop(d, p.viewport_type, PropertyValue::Enum(VIEWPORT_TYPE));
    d = prop(
        d,
        p.viewport_ambient,
        PropertyValue::Colour([0x19, 0x1E, 0x28, 0xFF]),
    );
    d = prop(d, USE_FOG, PropertyValue::Bool(true));
    d = prop(d, FOG_NEAR, PropertyValue::Float(3.0));
    d = prop(d, FOG_FAR, PropertyValue::Float(40.0));
    d = prop(
        d,
        FOG_COLOUR,
        PropertyValue::Colour([0x00, 0x00, 0xFF, 0xFF]),
    );
    prop(d, p.viewport_use_player, PropertyValue::Bool(use_player))
}

const CAMERA_1: u32 = 0x1000_001E;
const VIEWPORT_TYPE: u32 = 0x1000_0002;
const USE_FOG: u32 = 0xFFFD_3001;
const FOG_NEAR: u32 = 0xFFFD_3002;
const FOG_FAR: u32 = 0xFFFD_3003;
const FOG_COLOUR: u32 = 0xFFFD_3004;
/// The figure key whose synthetic description gives an image in place of the figure.
const IMAGE_FIGURE: u32 = 0x0099_0099;

fn ui_and_combat() -> (Ui, CombatUi) {
    let cv = CombatVocab::default();
    let i = cv.ids;
    let mut avatar = field(i.avatar_panel, 0, 0, 280, 300);
    avatar = child(avatar, field(i.player_name, 87, 3, 191, 36));
    avatar = child(avatar, portrait(i.player_portrait, 21, 21, true));
    let mut opp = field(i.opponent_panel, 300, 0, 305, 300);
    opp = child(opp, field(i.opponent_name, 2, 3, 216, 36));
    opp = child(
        opp,
        child(
            portrait(i.opponent_portrait, 220, 19, false),
            field(0x1000_0E01, 0, 0, 62, 62),
        ),
    );
    let mut l = UiLayout {
        did: Did(L),
        design_width: 1024,
        design_height: 768,
        ..UiLayout::default()
    };
    l.elements = vec![avatar, opp];
    l.elements_size.count = 2;
    let mut env = MemoryEnv::default();
    env.add_layout(l);
    let mut ui = Ui::new(Box::new(env), Vocab::default(), (1024, 768));
    let a = ui.open_panel(Did(L), i.avatar_panel).expect("P4");
    let o = ui.open_panel(Did(L), i.opponent_panel).expect("P5");
    let combat = CombatUi::new(
        &mut ui,
        cv,
        CombatLayouts::default(),
        CombatRoots {
            avatar_panel: Some(a),
            opponent_panel: Some(o),
            ..CombatRoots::default()
        },
        FloatySettings::default(),
    );
    (ui, combat)
}

/// A texture list with one 32 × 32 A8 surface: a disc of radius 15.
fn mask_source() -> MemorySource {
    let le = |w: &[u32]| -> Vec<u8> { w.iter().flat_map(|v| v.to_le_bytes()).collect() };
    let mut list = le(&[MASK_LIST]);
    list.extend_from_slice(&[2, 1, 0, 0, 0]);
    list.extend(le(&[MASK_SURFACE, 0, 15]));
    let n = 32u32;
    let px: Vec<u8> = (0..n * n)
        .map(|i| {
            let (x, y) = (f64::from(i % n) + 0.5, f64::from(i / n) + 0.5);
            if ((x - 16.0).powi(2) + (y - 16.0).powi(2)).sqrt() < 15.0 {
                255
            } else {
                0
            }
        })
        .collect();
    let mut surface = le(&[MASK_SURFACE, 0, n, n, SurfaceFormat::A8.code(), n * n]);
    surface.extend_from_slice(&px);
    let mut s = MemorySource::new();
    s.insert(Did(MASK_LIST), list);
    s.insert(Did(MASK_SURFACE), surface);
    s
}

/// Figures: a coloured square per entity, its colour the figure's key (or `tint`, a picture
/// change the key does not show), with a white marker square near the top-left corner.
#[derive(Default)]
struct Squares {
    figures: Vec<(EntityId, u32)>,
    draws: u32,
    tint: Option<u32>,
    unsettled: bool,
    last_job: Option<PortraitJob>,
    /// The entity of every figure drawn, in order.
    drawn: Vec<EntityId>,
}

impl PortraitFigures for Squares {
    type Key = u32;

    fn figure_key(&self, entity: EntityId) -> Option<u32> {
        self.figures
            .iter()
            .find(|(e, _)| *e == entity)
            .map(|(_, k)| *k)
    }

    fn settled(&self, _entity: EntityId) -> bool {
        !self.unsettled
    }

    fn target_of(&self, key: &u32) -> Option<PortraitTarget> {
        Some(PortraitTarget {
            body: Body::Object(DataId(*key)),
            instance: None,
            instance_key: None,
        })
    }

    fn draw(&mut self, frame: &mut Frame, job: &PortraitJob) -> Option<usize> {
        let key = self.figure_key(job.entity)?;
        let key = self.tint.unwrap_or(key);
        self.draws += 1;
        self.drawn.push(job.entity);
        self.last_job = Some(job.clone());
        let c = [
            f32::from(((key >> 16) & 0xFF) as u8) / 255.0,
            f32::from(((key >> 8) & 0xFF) as u8) / 255.0,
            f32::from((key & 0xFF) as u8) / 255.0,
            1.0,
        ];
        frame
            .pass("square", PassSpace::Screen)
            .premultiplied_alpha()
            .quad(Quad::screen_rect(21.0, 21.0, 20.0, 20.0).with_color(c))
            .quad(Quad::screen_rect(12.0, 12.0, 8.0, 8.0).with_color([1.0; 4]));
        Some(1)
    }
}

fn view(entity: EntityId) -> FrameView {
    FrameView {
        entity,
        name: "x".into(),
        has_vitals: true,
        ..FrameView::default()
    }
}

#[test]
fn portraits_draw_once_show_and_hide() {
    let Some(mut r) = gpu::headless_renderer([0.2, 0.3, 0.4, 1.0]) else {
        return;
    };
    let (mut ui, mut combat) = ui_and_combat();
    let cv = CombatVocab::default();
    let textures = UiTextures::new();
    let externals = textures.external_textures();
    let mut portraits: CombatPortraits<u32> = CombatPortraits::new(Some(Arc::new(mask_source())));
    let mut figures = Squares::default();
    let frames = |n: usize,
                  ui: &mut Ui,
                  combat: &CombatUi,
                  portraits: &mut CombatPortraits<u32>,
                  figures: &mut Squares,
                  r: &mut tessera_render::Renderer<'static>| {
        for _ in 0..n {
            portraits.frame(ui, combat, &externals, r, figures);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    };
    let p4 = {
        let root = combat.player.as_ref().unwrap().root();
        find_below(&ui, root, cv.ids.player_portrait).unwrap()
    };

    // No player entity yet: hidden, nothing registered (§12.4).
    frames(3, &mut ui, &combat, &mut portraits, &mut figures, &mut r);
    assert!(!ui.is_displayed(p4));
    assert!(externals.is_empty());

    // The player with a figure: drawn once the mask has loaded.
    combat.set_player(&mut ui, &view(7));
    figures.figures.push((7, 0x00C0_4020));
    for _ in 0..500 {
        frames(1, &mut ui, &combat, &mut portraits, &mut figures, &mut r);
        if portraits.drawn(FrameKind::Player) {
            break;
        }
    }
    assert!(portraits.drawn(FrameKind::Player), "P4 drawn");
    assert!(ui.is_displayed(p4), "P4 shown with its scene");
    let ext = externals
        .get(content_id(FrameKind::Player))
        .expect("registered");
    assert_eq!(ext.size, Some((62, 62)));
    let img = r
        .read_viewport_pixels(portraits.target(FrameKind::Player).unwrap())
        .unwrap();
    let px = |x, y| img.pixel(x, y).unwrap();
    assert_eq!(px(0, 0), [0, 0, 0, 0], "outside the mask");
    assert_eq!(px(30, 30), [0xC0, 0x40, 0x20, 255], "the figure");
    assert_eq!(px(31, 50), [0, 0, 0, 255], "the background inside the mask");
    assert_eq!(px(15, 15), [255, 255, 255, 255], "the marker, top left");
    assert_eq!(px(46, 46), [0, 0, 0, 255], "not mirrored");

    // Checked a few times while it might still load, then kept.
    frames(200, &mut ui, &combat, &mut portraits, &mut figures, &mut r);
    let settled = figures.draws;
    frames(100, &mut ui, &combat, &mut portraits, &mut figures, &mut r);
    assert_eq!(
        figures.draws, settled,
        "not drawn again while nothing changes"
    );
    assert!(settled <= 1 + u32::from(tessera_client::portrait::STABLE_CHECKS));

    // The figure changes: drawn again.
    figures.figures[0].1 = 0x0020_40C0;
    frames(1, &mut ui, &combat, &mut portraits, &mut figures, &mut r);
    assert_eq!(figures.draws, settled + 1);
    let img = r
        .read_viewport_pixels(portraits.target(FrameKind::Player).unwrap())
        .unwrap();
    assert_eq!(img.pixel(30, 30).unwrap(), [0x20, 0x40, 0xC0, 255]);

    // P5: a selected entity with a figure.
    combat.set_target(&mut ui, Some(&view(9)), None);
    figures.figures.push((9, 0x0010_C010));
    frames(2, &mut ui, &combat, &mut portraits, &mut figures, &mut r);
    assert!(portraits.drawn(FrameKind::Target), "P5 drawn");
    let p5 = combat.target.as_ref().unwrap().portrait().unwrap();
    let child = find_below(&ui, p5, 0x1000_0E01).unwrap();
    assert!(ui.is_displayed(p5) && ui.is_displayed(child));

    // A figure that cannot be drawn hides the portrait with its child again; its texture
    // stays registered for a list painted before the hide, until the next frame.
    figures.figures.retain(|(e, _)| *e != 9);
    frames(1, &mut ui, &combat, &mut portraits, &mut figures, &mut r);
    assert!(!ui.is_displayed(p5) && !ui.is_displayed(child));
    assert!(externals.get(content_id(FrameKind::Target)).is_some());
    frames(1, &mut ui, &combat, &mut portraits, &mut figures, &mut r);
    assert!(externals.get(content_id(FrameKind::Target)).is_none());
    assert!(ui.is_displayed(p4), "P4 unaffected");

    portraits.release(&mut r, &externals);
    assert!(externals.is_empty());
}

/// Draws P4 until its first drawing (the mask loads on a worker).
fn first_drawing(
    ui: &mut Ui,
    combat: &mut CombatUi,
    portraits: &mut CombatPortraits<u32>,
    figures: &mut Squares,
    externals: &tessera_client::ui_host::gpu::ExternalTextures,
    r: &mut tessera_render::Renderer<'static>,
) {
    combat.set_player(ui, &view(7));
    figures.figures.push((7, 0x00C0_4020));
    for _ in 0..500 {
        portraits.frame(ui, combat, externals, r, figures);
        if portraits.drawn(FrameKind::Player) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("P4 not drawn");
}

#[test]
fn a_texture_that_arrives_late_is_drawn() {
    let Some(mut r) = gpu::headless_renderer([0.2, 0.3, 0.4, 1.0]) else {
        return;
    };
    let (mut ui, mut combat) = ui_and_combat();
    let textures = UiTextures::new();
    let externals = textures.external_textures();
    let mut portraits: CombatPortraits<u32> = CombatPortraits::new(Some(Arc::new(mask_source())));
    let mut figures = Squares {
        unsettled: true,
        ..Squares::default()
    };
    first_drawing(
        &mut ui,
        &mut combat,
        &mut portraits,
        &mut figures,
        &externals,
        &mut r,
    );
    // Far longer than the stable window: still checked, never complete, while unsettled.
    let window = (u32::from(tessera_client::portrait::STABLE_CHECKS) + 2)
        * (tessera_client::portrait::RECHECK_FRAMES + 1);
    let before = figures.draws;
    for _ in 0..window * 2 {
        portraits.frame(&mut ui, &combat, &externals, &mut r, &mut figures);
    }
    assert!(!portraits.complete(FrameKind::Player));
    assert!(figures.draws > before + 2, "rechecked while unsettled");
    // The texture arrives (the picture changes, the key does not) and the figure settles:
    // the next check draws it.
    figures.tint = Some(0x0030_C030);
    figures.unsettled = false;
    for _ in 0..=tessera_client::portrait::RECHECK_FRAMES {
        portraits.frame(&mut ui, &combat, &externals, &mut r, &mut figures);
    }
    let img = r
        .read_viewport_pixels(portraits.target(FrameKind::Player).unwrap())
        .unwrap();
    assert_eq!(img.pixel(30, 30).unwrap(), [0x30, 0xC0, 0x30, 255]);
    // Then complete after the stable window, and not drawn again.
    for _ in 0..window {
        portraits.frame(&mut ui, &combat, &externals, &mut r, &mut figures);
    }
    assert!(portraits.complete(FrameKind::Player));
    let settled = figures.draws;
    for _ in 0..window {
        portraits.frame(&mut ui, &combat, &externals, &mut r, &mut figures);
    }
    assert_eq!(figures.draws, settled);
    portraits.release(&mut r, &externals);
}

#[test]
fn a_scaled_portrait_is_recorded_whole_and_linear() {
    let Some(mut r) = gpu::headless_renderer([0.2, 0.3, 0.4, 1.0]) else {
        return;
    };
    let (mut ui, mut combat) = ui_and_combat();
    let mut textures = UiTextures::new();
    let externals = textures.external_textures();
    let mut portraits: CombatPortraits<u32> = CombatPortraits::new(Some(Arc::new(mask_source())));
    let mut figures = Squares::default();
    first_drawing(
        &mut ui,
        &mut combat,
        &mut portraits,
        &mut figures,
        &externals,
        &mut r,
    );
    // The layout box at UI scale 1.5 (93 × 93), the list's canvas the box's, with the
    // nearest scale filter.
    textures.set_filter_override(Some(PaintFilter::Nearest));
    let list = CompositionList {
        entries: vec![Entry {
            texture: EntryTexture::External(content_id(FrameKind::Player)),
            source: Source::Premultiplied,
            canvas: [93, 93],
            tex_offset: [0, 0],
            dest: Rect::from_xywh(10, 10, 93, 93),
            addressing: Addressing::Fit,
            filter: PaintFilter::Nearest,
            opacity: 1.0,
            alpha_threshold: 0.0,
            rotations: 0..0,
            clips: 0..0,
        }],
        ..CompositionList::default()
    };
    let target = r
        .create_offscreen_target(113, 113, OffscreenFormat::Rgba8Unorm)
        .unwrap();
    let mut frame = r.begin_offscreen_frame(&target).unwrap();
    frame
        .pass("clear", PassSpace::Screen)
        .clear([0.0, 0.0, 1.0, 1.0]);
    assert_eq!(
        textures.record(&mut frame, &list, &CapCounters::default(), 0),
        1
    );
    r.end_frame(frame).unwrap();
    let img = r.read_pixels(&target).unwrap();
    let px = |x, y| img.pixel(x, y).unwrap();
    // The whole target fills the box: the square (21–41 of 62) spans 41.5–71.5, the marker
    // (12–20) 28–40, top left; the mask cuts the box's corners to the blue clear colour.
    assert_eq!(px(56, 56), [0xC0, 0x40, 0x20, 255], "the square");
    assert_eq!(px(34, 34), [255, 255, 255, 255], "the marker, top left");
    assert_eq!(px(79, 79), [0, 0, 0, 255], "not mirrored");
    assert_eq!(px(11, 11), [0, 0, 255, 255], "outside the mask");
    assert_eq!(px(101, 101), [0, 0, 255, 255], "outside the mask");
    // Linear: the pixel on the square's left edge (texel 21.0) mixes it with the
    // background; nearest would give one of them.
    let edge = px(41, 56);
    assert!(
        edge[0] > 20 && edge[0] < 0xC0 - 20,
        "linear filtering at the edge: {edge:?}"
    );
    portraits.release(&mut r, &externals);
}

/// A synthetic scene reader: `Camera_1` 2 m in front of the figure, one light, the scene's
/// fog override off; the figure [`IMAGE_FIGURE`] has a description with an image (its
/// reads are counted, so that a test knows the read finished).
#[derive(Default)]
struct Scenes {
    image_reads: std::sync::atomic::AtomicUsize,
}

impl SceneReader for Scenes {
    fn viewport_pids(&self) -> Result<ViewportPids, String> {
        Ok(ViewportPids {
            use_fog: Some(USE_FOG),
            fog_near: Some(FOG_NEAR),
            fog_far: Some(FOG_FAR),
            fog_colour: Some(FOG_COLOUR),
            camera_property: None,
        })
    }

    fn scene(&self, request: &SceneRequest) -> Result<PortraitScene, String> {
        let camera = SceneCamera {
            frame: SceneFrame {
                position: Vec3::new(0.0, 2.0, 1.5),
                rotation: tessera_render::glam::Quat::from_rotation_z(std::f32::consts::PI),
            },
            fov_degrees: 60.0,
            far: 20.0,
        };
        let mut s = PortraitScene::with_camera(request.scene, request.camera_key, Some(camera));
        s.lights.push(SceneLight {
            position: Vec3::new(1.0, 2.0, 2.0),
            params: tessera_world::LightParams::default(),
        });
        let image = matches!(
            request.target.as_ref().map(|t| t.body),
            Some(Body::Object(DataId(IMAGE_FIGURE)))
        );
        if image {
            self.image_reads
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            s.described = true;
            s.image = Some(Did(0x4100_0001));
        }
        Ok(s)
    }
}

#[test]
fn the_portrait_option_and_the_scene_decide_what_is_drawn() {
    let Some(mut r) = gpu::headless_renderer([0.2, 0.3, 0.4, 1.0]) else {
        return;
    };
    let (mut ui, mut combat) = ui_and_combat();
    let cv = CombatVocab::default();
    let textures = UiTextures::new();
    let externals = textures.external_textures();
    let reader = Arc::new(Scenes::default());
    let mut portraits: CombatPortraits<u32> = CombatPortraits::new(Some(Arc::new(mask_source())))
        .with_scene_reader(Arc::clone(&reader) as Arc<dyn SceneReader>);
    let mut figures = Squares::default();
    let p4 = {
        let root = combat.player.as_ref().unwrap().root();
        find_below(&ui, root, cv.ids.player_portrait).unwrap()
    };
    let externals = &externals;
    let until = |done: &dyn Fn(&CombatPortraits<u32>) -> bool,
                 ui: &mut Ui,
                 combat: &CombatUi,
                 portraits: &mut CombatPortraits<u32>,
                 figures: &mut Squares,
                 r: &mut tessera_render::Renderer<'static>| {
        for _ in 0..2000 {
            portraits.frame(ui, combat, externals, r, figures);
            if done(portraits) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        false
    };

    // The option off (§12.1 step 2): no figure for an element with the preference gate.
    portraits.set_portrait_option(false);
    assert!(!portraits.portrait_option());
    combat.set_player(&mut ui, &view(7));
    figures.figures.push((7, 0x00C0_4020));
    assert!(!until(
        &|p| p.drawn(FrameKind::Player),
        &mut ui,
        &combat,
        &mut portraits,
        &mut figures,
        &mut r
    ));
    assert!(!ui.is_displayed(p4), "hidden while the option is off");
    assert_eq!(figures.draws, 0);

    // On again: drawn through the scene, with its lighting in the job.
    portraits.set_portrait_option(true);
    assert!(until(
        &|p| p.drawn(FrameKind::Player),
        &mut ui,
        &combat,
        &mut portraits,
        &mut figures,
        &mut r
    ));
    assert!(ui.is_displayed(p4));
    let job = figures.last_job.clone().expect("drawn");
    assert_eq!(job.camera.map(|c| c.fov_degrees), Some(60.0));
    let scene = job.scene.as_ref().expect("the scene read");
    assert_eq!(scene.camera_key, CAMERA_1);
    let lit = job.lighting.as_ref().expect("lighting");
    let l = &lit.lighting;
    assert_eq!(
        l.ambient,
        [
            0x28 as f32 / 255.0,
            0x1E as f32 / 255.0,
            0x19 as f32 / 255.0
        ]
    );
    let fog = l.fog.expect("the viewport's fog");
    assert_eq!((fog.near, fog.far, fog.color), (3.0, 40.0, [1.0, 0.0, 0.0]));
    assert_eq!(l.lights.len(), 1);
    assert!(l.sun.is_none());
    let LightKind::Point { position, .. } = l.lights[0].kind else {
        panic!("a point light");
    };
    assert!((position - Vec3::new(1.0, 2.0, 2.0)).length() < 1e-5);

    // Turning the option off hides the drawn portrait at once.
    portraits.set_portrait_option(false);
    portraits.frame(&mut ui, &combat, externals, &mut r, &mut figures);
    assert!(!portraits.drawn(FrameKind::Player) && !ui.is_displayed(p4));
    portraits.set_portrait_option(true);

    // A target whose description gives an image (§12.1 step 3) draws no figure: wait until
    // the reader has served that read and its result has reached the frames (no read in
    // flight), then check over a few more frames that P5 stays hidden and entity 9 was
    // never drawn.
    combat.set_target(&mut ui, Some(&view(9)), None);
    figures.figures.push((9, IMAGE_FIGURE));
    let served = |p: &CombatPortraits<u32>| {
        reader.image_reads.load(std::sync::atomic::Ordering::SeqCst) >= 1
            && p.scene_reads_in_flight() == 0
    };
    for _ in 0..5000 {
        portraits.frame(&mut ui, &combat, externals, &mut r, &mut figures);
        if served(&portraits) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(served(&portraits), "the image description was read");
    for _ in 0..5 {
        portraits.frame(&mut ui, &combat, externals, &mut r, &mut figures);
    }
    assert!(!portraits.drawn(FrameKind::Target), "an image, no figure");
    let p5 = combat.target.as_ref().unwrap().portrait().unwrap();
    assert!(!ui.is_displayed(p5));
    assert!(!figures.drawn.contains(&9), "entity 9 never drawn");
    // The same target with a figure whose description has no image is drawn under the same
    // setup: the image alone hid it.
    figures.figures.retain(|(e, _)| *e != 9);
    figures.figures.push((9, 0x00C0_4021));
    assert!(until(
        &|p| p.drawn(FrameKind::Target),
        &mut ui,
        &combat,
        &mut portraits,
        &mut figures,
        &mut r
    ));
    assert!(ui.is_displayed(p5));
    assert!(figures.drawn.contains(&9));
    assert_eq!(
        reader.image_reads.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the image description was read once"
    );
    portraits.release(&mut r, externals);
}
