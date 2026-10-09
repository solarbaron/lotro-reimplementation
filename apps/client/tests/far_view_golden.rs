// SPDX-License-Identifier: GPL-3.0-or-later
//! The far view rendered headless from synthetic sectors (`docs/specs/far-view.md`):
//!
//! - `far_view_horizon.png`: a horizon view over 5 × 5 synthetic sectors of rolling hills
//!   (`LandscapeDrawDistance` Low: *F* = 2, *E* = 1,120): sector records → the far-view
//!   state of `tessera-world` (sector list, per-landblock grid, LOD and stitching, batch
//!   builds, sprite placement) → the walker's conversion helpers → the far-terrain and
//!   distant-sprite pipelines, with fog and the far-draw fade of FD1. The near field is
//!   drawn from the sectors' near batches (the tool use of FD8), so the view has no hole.
//! - pixel checks of the plain template (§6.3, the worked example's arithmetic), the
//!   far-draw fade beyond *E*, and the sprite fade-in of §9.3 (FV-I10).
//!
//! Every input is generated here; no game data is involved. Regenerate with
//! `TESSERA_BLESS=1 cargo test -p tessera-client --test far_view_golden`.

#[path = "../../../crates/render/tests/support/golden.rs"]
mod golden;
#[path = "../../../crates/render/tests/support/gpu.rs"]
mod gpu;

use std::path::Path;

use tessera_client::walker::farview::{distant_sprites, fade_params, far_terrain_data};
use tessera_formats::landblock::{
    SectorChunk, SectorMesh, SectorSample, SectorSprite, chunk_index, vertex_block,
};
use tessera_formats::{Did, ImposterEntry, ImposterTable, SectorRecord, TSize};
use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    Camera, ColorSpace, DistantSprite, FarFadeParams, FarTerrainData, FarTerrainDraw, Filter, Fog,
    FrameLighting, OffscreenFormat, Perspective, RenderError, Renderer, RgbaImage, SpriteDraw,
    SunLight, TextureId, TextureUpload, Wrap,
};
use tessera_world::LandblockKey;
use tessera_world::far_view::{
    FarViewSettings, FarViewState, NearField, SectorKey, SpriteView, Viewer,
};

const WIDTH: u32 = 384;
const HEIGHT: u32 = 216;
const CLEAR: [f32; 4] = [0.62, 0.70, 0.80, 1.0];
const VIEWER: (u8, u8) = (100, 50);
const TABLE: Did = Did(0x2701_5124);

/// Rolling hills over region-global (x, y), in units.
fn height(gx: f32, gy: f32) -> f32 {
    let a = (gx / 310.0).sin() * (gy / 270.0).cos() * 55.0;
    let b = ((gx + gy) / 140.0).sin() * 14.0;
    let ridge = (-(gy - 8_900.0).powi(2) / 60_000.0).exp() * 140.0;
    300.0 + a + b + ridge
}

fn raw_height(h: f32) -> u16 {
    (h / 2_000.0 * 65_535.0).round().clamp(0.0, 65_535.0) as u16
}

fn normal_bytes(gx: f32, gy: f32) -> [u8; 3] {
    let e = 2.5;
    let dx = (height(gx + e, gy) - height(gx - e, gy)) / (2.0 * e);
    let dy = (height(gx, gy + e) - height(gx, gy - e)) / (2.0 * e);
    let n = Vec3::new(-dx, -dy, 1.0).normalize();
    let b = |c: f32| ((c + 1.0) * 127.5).floor().clamp(0.0, 255.0) as u8;
    // Stored order: world X, world Z (up), world Y.
    [b(n.x), b(n.z), b(n.y)]
}

fn sector(key: SectorKey) -> SectorRecord {
    let (cx, cy) = key.corner();
    let mut terrain = SectorMesh::default();
    for k in 0..9usize {
        let (x, y) = (k / 3, k % 3);
        for s in 0..1089usize {
            let (i, j) = (s % 33, s / 33);
            let gx = (f32::from(cx) + x as f32) * 160.0 + i as f32 * 5.0;
            let gy = (f32::from(cy) + y as f32) * 160.0 + j as f32 * 5.0;
            terrain.samples.push(SectorSample {
                height: raw_height(height(gx, gy)),
                normal: normal_bytes(gx, gy),
            });
        }
    }
    for x in 0..3 {
        for y in 0..3 {
            let k = u32::try_from(vertex_block(x, y)).unwrap_or(0);
            terrain.chunks[chunk_index(x, y)] = SectorChunk::expected(k, x, y, 200.0, 600.0);
        }
    }
    // A few rows of trees in every sector: key 1 big trees, key 2 small ones, key 3 has no
    // table entry (skipped).
    let mut sprites = Vec::new();
    for n in 0..36u16 {
        let seed = u32::from(n).wrapping_mul(2_654_435_761)
            ^ u32::from(cx).wrapping_mul(40_503)
            ^ u32::from(cy).wrapping_mul(9_973);
        let east = 10.0 + (seed % 460) as f32;
        let north = 10.0 + ((seed >> 9) % 460) as f32;
        let gx = f32::from(cx) * 160.0 + east;
        let gy = f32::from(cy) * 160.0 + north;
        sprites.push(SectorSprite {
            name_key: 1 + n % 3,
            east,
            height: height(gx, gy),
            north,
            scale: 0.8 + f32::from(n % 5) * 0.1,
            distance: 140.0,
            byte: u8::try_from(n * 37 % 256).unwrap_or(0),
        });
    }
    SectorRecord {
        did: key.did(),
        terrain,
        mode: 0,
        ceiling_mask: 0,
        ceiling: None,
        block_size: 160.0,
        water: Vec::new(),
        sprites,
        entities: Vec::new(),
        colour_list: Did(0x4000_0001),
        ceiling_colour_list: Did(0),
        tile: Did(0x4100_0001),
        ceiling_tile: Did(0),
        flow_map: Did(0),
        imposter_table: TABLE,
    }
}

fn table() -> ImposterTable {
    ImposterTable {
        did: TABLE,
        size: TSize { hint: 1, count: 2 },
        entries: vec![
            ImposterEntry {
                name_key: 1,
                column: 0,
                row: 0,
                value_a: 26.0,
                value_b: -0.5,
            },
            ImposterEntry {
                name_key: 2,
                column: 1,
                row: 0,
                value_a: 14.0,
                value_b: -0.2,
            },
        ],
        colour_atlas: Did(0x4000_0002),
        second_atlas: Did(0x4000_0003),
        // Two columns, one row: 31/32 of a cell step.
        cell_u: 31.0 / 64.0,
        cell_v: 31.0 / 32.0,
    }
}

fn texels(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    (0..w * h).flat_map(|i| f(i % w, i / w)).collect()
}

fn upload(
    r: &mut Renderer<'static>,
    w: u32,
    h: u32,
    data: &[u8],
    wrap: Wrap,
) -> Result<TextureId, RenderError> {
    r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Linear,
        filter: Filter::Linear,
        wrap,
        ..TextureUpload::rgba8(w, h, data)
    })
}

/// A tree silhouette in a 128 × 128 cell: a trunk and a rounded crown; the last 4 texel
/// rows and columns are empty (`misc-records.md` §5.4).
fn tree_cell(x: u32, y: u32, crown: [u8; 3]) -> [u8; 4] {
    if x >= 124 || y >= 124 {
        return [0, 0, 0, 0];
    }
    let (fx, fy) = (x as f32 - 62.0, y as f32);
    let trunk = fx.abs() < 5.0 && fy > 90.0;
    let r = 52.0 - (fy - 50.0).abs() * 0.35;
    let crown_in = fx.abs() < r && fy > 6.0 && fy < 100.0;
    if crown_in {
        let shade = 0.8 + 0.2 * ((fx * 0.3).sin() * (fy * 0.21).cos());
        [
            (f32::from(crown[0]) * shade) as u8,
            (f32::from(crown[1]) * shade) as u8,
            (f32::from(crown[2]) * shade) as u8,
            255,
        ]
    } else if trunk {
        [90, 64, 40, 255]
    } else {
        [0, 0, 0, 0]
    }
}

struct Built {
    state: FarViewState,
}

/// Drives the far-view state synchronously until everything is in.
fn build(settings: FarViewSettings, viewer: (u8, u8)) -> Built {
    let mut state = FarViewState::new(settings, None);
    let v = Viewer {
        landblock: LandblockKey::new(1, viewer.0, viewer.1).unwrap_or_else(|_| unreachable!()),
        instance: 0,
    };
    let view = SpriteView {
        target_height: HEIGHT as f32,
        fov_y: 50f32.to_radians(),
        object_scale: 1.0,
    };
    for _ in 0..4 {
        let w = state.update(Some(v), NearField::Radius(1), view);
        for k in &w.load_sectors {
            state.sector_loaded(*k, Ok(Some(sector(*k))));
        }
        for d in &w.load_tables {
            state.table_loaded(*d, Ok(Some(table())));
        }
        for j in &w.build {
            let m = j.run();
            state.built(j.key, j.generation, m);
        }
        for j in &w.sprites {
            state.sprites_placed(j.key, j.epoch, j.run());
        }
    }
    assert!(state.settled(), "far view did not settle");
    Built { state }
}

/// The render origin: the viewer landblock's south-west corner.
fn origin_of(key: SectorKey) -> Vec3 {
    let (cx, cy) = key.corner();
    Vec3::new(
        (f32::from(cx) - f32::from(VIEWER.0)) * 160.0,
        (f32::from(cy) - f32::from(VIEWER.1)) * 160.0,
        0.0,
    )
}

fn horizon_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let settings = FarViewSettings {
        near_batches: true,
        ..FarViewSettings::from_level(1)
    };
    let built = build(settings, VIEWER);
    let colour = upload(
        r,
        64,
        64,
        &texels(64, 64, |x, y| {
            let t = ((x as f32 * 0.21).sin() + (y as f32 * 0.17).cos()) * 0.5;
            [
                (96.0 + 30.0 * t) as u8,
                (118.0 + 24.0 * t) as u8,
                (62.0 + 12.0 * t) as u8,
                255,
            ]
        }),
        Wrap::Clamp,
    )?;
    let detail = upload(
        r,
        32,
        32,
        &texels(32, 32, |x, y| {
            let v = 128.0 + 22.0 * ((x * 7 + y * 13) % 11) as f32 / 11.0 - 11.0;
            [v as u8, v as u8, v as u8, 255]
        }),
        Wrap::Repeat,
    )?;
    let atlas = upload(
        r,
        256,
        128,
        &texels(256, 128, |x, y| {
            if x < 128 {
                tree_cell(x, y, [52, 96, 44])
            } else {
                tree_cell(x - 128, y, [70, 110, 40])
            }
        }),
        Wrap::Repeat,
    )?;
    let normals = upload(
        r,
        256,
        128,
        &texels(256, 128, |x, _| {
            let nx = ((x % 128) as f32 - 62.0) / 62.0 * 0.5;
            let n = Vec3::new(nx, 0.0, 1.0).normalize();
            let e = |c: f32| ((c * 0.5 + 0.5) * 255.0).round() as u8;
            [e(n.x), e(n.y), e(n.z), 255]
        }),
        Wrap::Repeat,
    )?;
    let fade = fade_params(&settings);
    let mut terrain = Vec::new();
    let mut sprite_draws = Vec::new();
    let mut far_landblocks = 0;
    for k in built.state.listed() {
        let Some(slot) = built.state.slot(*k) else {
            continue;
        };
        let Some(m) = &slot.meshes else {
            continue;
        };
        far_landblocks += m.terrain.landblocks;
        let o = origin_of(*k);
        for batch in [Some(&m.terrain), m.near_terrain.as_ref()]
            .into_iter()
            .flatten()
        {
            if batch.is_empty() {
                continue;
            }
            let mesh = r.upload_far_terrain(&far_terrain_data("far", batch))?;
            let mut d = FarTerrainDraw::new(mesh, Mat4::from_translation(o));
            d.colour = Some(colour);
            d.detail = Some(detail);
            d.fade = fade;
            terrain.push(d);
        }
        if let Some((_, sprites)) = &slot.sprites {
            let batch = r.upload_sprite_batch("trees", &distant_sprites(sprites))?;
            let mut d = SpriteDraw::new(batch, Mat4::IDENTITY, Some(atlas));
            d.normal_atlas = Some(normals);
            d.fade = fade;
            let centre = o + Vec3::new(240.0, 240.0, 0.0);
            sprite_draws.push((centre.truncate().length(), d));
        }
    }
    assert_eq!(far_landblocks, 15 * 15 - 9, "the far field of F = 2");
    // FD4: far to near.
    sprite_draws.sort_by(|a, b| b.0.total_cmp(&a.0));
    let eye = Vec3::new(
        80.0,
        40.0,
        height(100.0 * 160.0 + 80.0, 50.0 * 160.0 + 40.0) + 70.0,
    );
    let camera = Camera {
        eye,
        target: eye + Vec3::new(0.25, 1.0, -0.1),
        up: Vec3::Z,
        projection: Perspective {
            fov_y: 50f32.to_radians(),
            near: 0.5,
            far: 4_000.0,
        },
    };
    let lighting = FrameLighting {
        ambient: [0.34, 0.36, 0.40],
        hemispheric: [0.06, 0.06, 0.04],
        sun: Some(SunLight {
            direction: Vec3::new(-0.4, 0.5, 0.75).normalize(),
            color: [0.28, 0.27, 0.24],
            specular: [0.0; 3],
        }),
        fog: Some(Fog {
            near: 250.0,
            far: 1_300.0,
            color: [CLEAR[0], CLEAR[1], CLEAR[2]],
        }),
        ..FrameLighting::default()
    };
    let target = r.create_offscreen_target(WIDTH, HEIGHT, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let pass = frame
        .world_pass("far-view", &camera)
        .clear(CLEAR)
        .frame_lighting(lighting);
    for d in terrain {
        pass.far_terrain(d);
    }
    for (_, d) in sprite_draws {
        pass.distant_sprites(d);
    }
    r.end_frame(frame)?;
    r.read_pixels(&target)
}

#[test]
fn far_view_horizon_golden() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let img = horizon_scene(&mut r)?;
    let again = horizon_scene(&mut r)?;
    assert!(img.data == again.data, "render is not deterministic");
    // The sky stays clear at the top, the terrain fills the bottom.
    let top = img.pixel(WIDTH / 2, 2).unwrap_or_default();
    let bottom = img.pixel(WIDTH / 2, HEIGHT - 3).unwrap_or_default();
    assert_ne!(top, bottom);
    golden::check(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/far_view_horizon.png"),
        &img,
        golden::Tolerance::default(),
    );
    Ok(())
}

/// One flat far batch: a 160 × 160 square at height 0 in two triangles.
fn flat_square(r: &mut Renderer<'static>) -> Result<tessera_render::FarTerrainMesh, RenderError> {
    let positions = [
        [0.0, 0.0, 0.0],
        [160.0, 0.0, 0.0],
        [0.0, 160.0, 0.0],
        [160.0, 160.0, 0.0],
    ];
    let normals = [[0.0, 0.0, 1.0]; 4];
    let uvs = [
        [0.0, 1.0],
        [1.0 / 3.0, 1.0],
        [0.0, 2.0 / 3.0],
        [1.0 / 3.0, 2.0 / 3.0],
    ];
    let indices = [2, 0, 3, 3, 0, 1];
    r.upload_far_terrain(&FarTerrainData {
        label: "flat",
        positions: &positions,
        normals: &normals,
        uvs: &uvs,
        indices: &indices,
        stitch_start: 6,
    })
}

fn flat_pixel(
    r: &mut Renderer<'static>,
    eye_height: f32,
    fade: FarFadeParams,
    sprite: Option<(f32, f32)>,
) -> Result<[u8; 4], RenderError> {
    let mesh = flat_square(r)?;
    let grey = |v: u8| [v, v, v, 255];
    let colour = upload(r, 4, 4, &texels(4, 4, |_, _| grey(102)), Wrap::Clamp)?;
    let detail = upload(r, 4, 4, &texels(4, 4, |_, _| grey(128)), Wrap::Repeat)?;
    let white = upload(
        r,
        4,
        4,
        &texels(4, 4, |_, _| [200, 40, 40, 255]),
        Wrap::Repeat,
    )?;
    let eye = Vec3::new(80.0, 80.0, eye_height);
    let camera = Camera {
        eye,
        target: Vec3::new(80.0, 80.0001, 0.0),
        up: Vec3::Z,
        projection: Perspective {
            fov_y: 30f32.to_radians(),
            near: 0.5,
            far: 10_000.0,
        },
    };
    let lighting = FrameLighting {
        ambient: [0.3; 3],
        hemispheric: [0.0; 3],
        sun: None,
        fog: None,
        ..FrameLighting::default()
    };
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let pass = frame
        .world_pass("flat", &camera)
        .clear([0.0, 0.0, 1.0, 1.0])
        .frame_lighting(lighting);
    let mut d = FarTerrainDraw::new(mesh, Mat4::IDENTITY);
    d.colour = Some(colour);
    d.detail = Some(detail);
    d.fade = fade;
    pass.far_terrain(d);
    if let Some((distance, below)) = sprite {
        // A large quad under the camera, facing it, filling the view.
        let batch = r.upload_sprite_batch(
            "probe",
            &[DistantSprite {
                position: [80.0, 80.0, below],
                size: 400.0,
                base: -200.0,
                u: [0.0, 1.0],
                v: [0.0, 1.0],
                distance,
                mirror: false,
            }],
        )?;
        pass.distant_sprites(SpriteDraw::new(batch, Mat4::IDENTITY, Some(white)));
    }
    r.end_frame(frame)?;
    Ok(r.read_pixels(&target)?.pixel(8, 8).unwrap_or_default())
}

#[test]
fn plain_template_fade_and_sprite_fade_in() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    // §6.3: c = (0.4 + 0.502 − 0.5) × 0.3; output 2c = 0.241 (stored value) → 61.5.
    let px = flat_pixel(&mut r, 100.0, FarFadeParams::default(), None)?;
    for c in &px[..3] {
        assert!((i32::from(*c) - 61).abs() <= 2, "{px:?}");
    }
    // FD1-style fade: fully faded (a < 0.1) beyond the end → the clear colour.
    let fade = FarFadeParams {
        start: 20.0,
        inv_range: 1.0 / 40.0,
    };
    let px = flat_pixel(&mut r, 100.0, fade, None)?;
    assert_eq!(px, [0, 0, 255, 255]);
    // §9.3 (FV-I10): a sprite whose anchor is 110 units away with D = 140, c = 1 is
    // invisible (fade-in starts at 115); at 130 it is fully opaque.
    let px = flat_pixel(&mut r, 120.0, FarFadeParams::default(), Some((140.0, 10.0)))?;
    for c in &px[..3] {
        assert!(
            (i32::from(*c) - 61).abs() <= 2,
            "sprite drawn too early: {px:?}"
        );
    }
    let px = flat_pixel(&mut r, 140.0, FarFadeParams::default(), Some((140.0, 10.0)))?;
    assert!(px[0] > px[1] + 40, "sprite missing: {px:?}");
    Ok(())
}

/// The centre pixel of a sprite quad filling the view whose atlas texels have coverage
/// `alpha` (0–255), drawn with alpha-test reference `reference` (`None`: the default)
/// over a blue clear colour.
fn sprite_coverage_pixel(
    r: &mut Renderer<'static>,
    alpha: u8,
    reference: Option<f32>,
) -> Result<[u8; 4], RenderError> {
    let atlas = upload(
        r,
        4,
        4,
        &texels(4, 4, |_, _| [200, 40, 40, alpha]),
        Wrap::Repeat,
    )?;
    let camera = Camera {
        eye: Vec3::new(80.0, 80.0, 140.0),
        target: Vec3::new(80.0, 80.0001, 0.0),
        up: Vec3::Z,
        projection: Perspective {
            fov_y: 30f32.to_radians(),
            near: 0.5,
            far: 10_000.0,
        },
    };
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let pass = frame
        .world_pass("sprite coverage", &camera)
        .clear([0.0, 0.0, 1.0, 1.0])
        .frame_lighting(FrameLighting {
            ambient: [0.3; 3],
            hemispheric: [0.0; 3],
            sun: None,
            fog: None,
            ..FrameLighting::default()
        });
    let batch = r.upload_sprite_batch(
        "coverage",
        &[DistantSprite {
            position: [80.0, 80.0, 10.0],
            size: 400.0,
            base: -200.0,
            u: [0.0, 1.0],
            v: [0.0, 1.0],
            distance: 1.0,
            mirror: false,
        }],
    )?;
    let mut d = SpriteDraw::new(batch, Mat4::IDENTITY, Some(atlas));
    if let Some(a) = reference {
        d.alpha_test = a;
    }
    pass.distant_sprites(d);
    r.end_frame(frame)?;
    Ok(r.read_pixels(&target)?.pixel(8, 8).unwrap_or_default())
}

/// FV24, `far-view.md` §9.3: the sprite material's alpha test uses the records' reference
/// 0.1 (handed as 25 / 255), so a texel of coverage 0.3 is drawn and one of 0.08 is not;
/// the former Tessera threshold 0.5 would have cut the 0.3 texel.
#[test]
fn sprite_alpha_test_uses_the_records_reference() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let kept = sprite_coverage_pixel(&mut r, 77, None)?;
    assert!(kept[0] > 0, "coverage 0.3 must be drawn: {kept:?}");
    let cut = sprite_coverage_pixel(&mut r, 20, None)?;
    assert_eq!(cut, [0, 0, 255, 255], "coverage 0.08 must be discarded");
    let old = sprite_coverage_pixel(&mut r, 77, Some(0.5))?;
    assert_eq!(
        old,
        [0, 0, 255, 255],
        "an explicit 0.5 reference still applies"
    );
    Ok(())
}

/// FD14: the far view draws after the sky in the same pass, so a half-faded far-terrain
/// pixel blends with the sky behind it, not with the clear colour (a far view drawn before
/// the sky would write depth there and keep the sky out).
#[test]
fn far_view_fades_over_the_sky() -> Result<(), RenderError> {
    use tessera_render::{MeshData, SkyDraw, SkyObject, WorldSurface};
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let mesh = flat_square(&mut r)?;
    let grey = |v: u8| [v, v, v, 255];
    let colour = upload(&mut r, 4, 4, &texels(4, 4, |_, _| grey(102)), Wrap::Clamp)?;
    let detail = upload(&mut r, 4, 4, &texels(4, 4, |_, _| grey(128)), Wrap::Repeat)?;
    // A red sky card below the eye, so it lies behind the terrain in the view.
    let h = 500.0;
    let card = MeshData::new(
        vec![
            [-h, -h, -40.0],
            [h, -h, -40.0],
            [h, h, -40.0],
            [-h, h, -40.0],
        ],
        vec![0, 1, 2, 0, 2, 3],
    );
    let sky = r.upload_world_model(
        &card,
        &[WorldSurface {
            unlit: true,
            two_sided: true,
            ..WorldSurface {
                base_color: [1.0, 0.0, 0.0, 1.0],
                ..WorldSurface::default()
            }
        }],
    )?;
    let camera = Camera {
        eye: Vec3::new(80.0, 80.0, 100.0),
        target: Vec3::new(80.0, 80.0001, 0.0),
        up: Vec3::Z,
        projection: Perspective {
            fov_y: 30f32.to_radians(),
            near: 0.5,
            far: 10_000.0,
        },
    };
    let target = r.create_offscreen_target(16, 16, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let pass = frame
        .world_pass("far over sky", &camera)
        .clear([0.0, 0.0, 1.0, 1.0])
        .frame_lighting(FrameLighting {
            ambient: [0.3; 3],
            hemispheric: [0.0; 3],
            sun: None,
            fog: None,
            ..FrameLighting::default()
        });
    let mut d = FarTerrainDraw::new(mesh, Mat4::IDENTITY);
    d.colour = Some(colour);
    d.detail = Some(detail);
    // About half faded at the centre pixel (100 units away).
    d.fade = FarFadeParams {
        start: 80.0,
        inv_range: 1.0 / 40.0,
    };
    // Queued before the sky: the renderer's order decides, not the queue.
    pass.far_terrain(d);
    pass.sky(SkyDraw {
        objects: vec![SkyObject::new(sky, 0.0, 0.0)],
    });
    r.end_frame(frame)?;
    let px = r.read_pixels(&target)?.pixel(8, 8).unwrap_or_default();
    assert!(
        i32::from(px[0]) > i32::from(px[2]) + 60,
        "the faded far terrain must blend with the sky: {px:?}"
    );
    Ok(())
}
