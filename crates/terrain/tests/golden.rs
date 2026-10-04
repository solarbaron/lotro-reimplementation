// SPDX-License-Identifier: GPL-3.0-or-later
//! Golden images of a generated terrain patch (roadmap M3 exit criterion 6).
//!
//! A 3 × 3-landblock patch is generated here from integer-hashed value noise and a
//! rational "mountain" bump (only `+ − × ÷` and `floor`/`round` in f64, so the heights are
//! identical on every platform), meshed with a different LOD per block (stitched edges),
//! and rendered through `tessera-render` headless:
//!
//! - `terrain_patch.png`: oblique view, lit, height/slope vertex colours;
//! - `terrain_lod_wireframe.png`: top-down view, unlit vertex colours with the triangle
//!   wireframe, showing the levels and the stitch strips between them.
//!
//! No game data is involved. Regenerate with
//! `TESSERA_BLESS=1 cargo test -p tessera-terrain --test golden` and review the PNGs in
//! `tests/golden/` (see `tests/golden/README.md`). The comparison rules and the
//! llvmpipe/lavapipe notes are those of `crates/render/tests/golden/README.md`.

#[path = "../../render/tests/support/golden.rs"]
mod golden;
#[path = "../../render/tests/support/gpu.rs"]
mod gpu;

use std::path::Path;

use tessera_formats::landblock::{GRID_SIDE, HeightGrid, VertexFlags};
use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    Camera, Indices, Lighting, Material, MeshData, MeshDebug, MeshDraw, MeshShading,
    OffscreenFormat, Perspective, RenderError, Renderer, RgbaImage,
};
use tessera_terrain::{
    EdgeLods, HeightSlopeShading, Lod, Neighbours, Side, TerrainBlock, TerrainMesh, VertexNormals,
};

const CLEAR: [f32; 4] = [0.55, 0.68, 0.82, 1.0];
/// Blocks per side of the patch.
const N: usize = 3;
/// LOD of block (x, y), indexed `[y][x]` (south row first).
const LODS: [[u8; N]; N] = [[2, 1, 3], [1, 0, 2], [3, 2, 4]];

fn hash01(x: i64, y: i64, seed: u64) -> f64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ seed;
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 29;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// Value noise in [0, 1) on a lattice of `cell` samples.
fn value_noise(gi: i64, gj: i64, cell: i64, seed: u64) -> f64 {
    let (cx, cy) = (gi.div_euclid(cell), gj.div_euclid(cell));
    let fx = smooth(gi.rem_euclid(cell) as f64 / cell as f64);
    let fy = smooth(gj.rem_euclid(cell) as f64 / cell as f64);
    let h = |dx, dy| hash01(cx + dx, cy + dy, seed);
    let a = h(0, 0) + (h(1, 0) - h(0, 0)) * fx;
    let b = h(0, 1) + (h(1, 1) - h(0, 1)) * fx;
    a + (b - a) * fy
}

/// Stored height (u16) of patch-global sample (gi, gj).
fn stored_height(gi: i64, gj: i64) -> u16 {
    let span = (N * 32) as f64;
    let (x, y) = (gi as f64 / span, gj as f64 / span);
    let d2 = (x - 0.85) * (x - 0.85) + (y - 0.8) * (y - 0.8);
    let mountain = 1.0 / (1.0 + d2 * 6.0);
    let lake = 1.0 / (1.0 + ((x - 0.2) * (x - 0.2) + (y - 0.25) * (y - 0.25)) * 30.0);
    let units = 24.0 + 300.0 * mountain * mountain - 30.0 * lake
        + 40.0 * value_noise(gi, gj, 16, 1)
        + 14.0 * value_noise(gi, gj, 5, 2)
        + 3.0 * value_noise(gi, gj, 2, 3);
    (units * 32.768).round().clamp(0.0, 65_535.0) as u16
}

fn block_at(bx: usize, by: usize) -> TerrainBlock {
    let (ox, oy) = ((bx * 32) as i64, (by * 32) as i64);
    let heights = HeightGrid::from_fn(|i, j| stored_height(ox + i as i64, oy + j as i64));
    let mut flags = VertexFlags::default();
    for i in 0..GRID_SIDE {
        for j in 0..GRID_SIDE {
            if hash01(ox + i as i64, oy + j as i64, 7) < 0.5 {
                flags.raw_mut()[GRID_SIDE * i + j] = VertexFlags::SPLIT_SW_NE;
            }
        }
    }
    TerrainBlock::new(&heights, &flags)
}

fn to_mesh_data(m: &TerrainMesh, label: String) -> MeshData {
    MeshData {
        label: Some(label),
        positions: m.positions.clone(),
        normals: Some(m.normals.clone()),
        uvs: Some(m.uvs.clone()),
        colors: Some(m.colors.clone()),
        indices: Indices::U16(m.indices.clone()),
        ..MeshData::default()
    }
}

/// Meshes of the patch with their block transforms (patch centred on the origin).
fn patch_meshes() -> Vec<(MeshData, Mat4)> {
    let blocks: Vec<Vec<TerrainBlock>> = (0..N)
        .map(|y| (0..N).map(|x| block_at(x, y)).collect())
        .collect();
    let lod = |x: usize, y: usize| Lod::new(LODS[y][x]).expect("valid LOD");
    let neighbour = |x: usize, y: usize, side: Side| {
        let (dx, dy) = side.offset();
        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
        ((0..N as i32).contains(&nx) && (0..N as i32).contains(&ny))
            .then_some((nx as usize, ny as usize))
    };
    let shading = HeightSlopeShading::default();
    let half = (N as f32) * 160.0 / 2.0;
    let mut out = Vec::new();
    for y in 0..N {
        for x in 0..N {
            let nb =
                Neighbours(Side::ALL.map(|s| neighbour(x, y, s).map(|(nx, ny)| &blocks[ny][nx])));
            let edges = EdgeLods(Side::ALL.map(|s| neighbour(x, y, s).map(|(nx, ny)| lod(nx, ny))));
            let normals = VertexNormals::compute(&blocks[y][x], &nb);
            let mesh = TerrainMesh::build(&blocks[y][x], &normals, &shading, lod(x, y), edges);
            let at = Mat4::from_translation(Vec3::new(
                x as f32 * 160.0 - half,
                y as f32 * 160.0 - half,
                0.0,
            ));
            out.push((to_mesh_data(&mesh, format!("terrain {x},{y}")), at));
        }
    }
    out
}

fn render(
    r: &mut Renderer<'static>,
    width: u32,
    height: u32,
    camera: &Camera,
    debug: MeshDebug,
) -> Result<RgbaImage, RenderError> {
    let mut draws = Vec::new();
    for (data, at) in patch_meshes() {
        let id = r.upload_mesh(&data)?;
        draws.push(MeshDraw::new(id, at, Material::default()));
    }
    let target = r.create_offscreen_target(width, height, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    {
        let pass = frame
            .world_pass("terrain", camera)
            .clear(CLEAR)
            .lighting(Lighting {
                direction: Vec3::new(-0.5, -0.35, 0.8),
                ..Lighting::default()
            })
            .mesh_debug(debug);
        for d in &draws {
            pass.mesh(*d);
        }
    }
    r.end_frame(frame)?;
    let img = r.read_pixels(&target)?;
    for d in draws {
        r.destroy_mesh(d.mesh);
    }
    Ok(img)
}

/// Render twice (must be bit-identical) and compare against the golden PNG.
fn check_golden(
    name: &str,
    scene: impl Fn(&mut Renderer<'static>) -> Result<RgbaImage, RenderError>,
) -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let img = scene(&mut r)?;
    let again = scene(&mut r)?;
    assert!(
        img.data == again.data,
        "{name}: render is not deterministic"
    );
    golden::check(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/golden/{name}.png")),
        &img,
        golden::Tolerance::default(),
    );
    Ok(())
}

#[test]
fn terrain_patch_matches_golden() -> Result<(), RenderError> {
    check_golden("terrain_patch", |r| {
        let camera = Camera {
            eye: Vec3::new(-300.0, -520.0, 420.0),
            target: Vec3::new(10.0, 20.0, 60.0),
            up: Vec3::Z,
            projection: Perspective {
                fov_y: 50f32.to_radians(),
                near: 1.0,
                far: 4_000.0,
            },
        };
        render(r, 320, 200, &camera, MeshDebug::default())
    })
}

#[test]
fn terrain_lod_wireframe_matches_golden() -> Result<(), RenderError> {
    check_golden("terrain_lod_wireframe", |r| {
        // Straight down from high above (north up in the image), narrow field of view.
        let camera = Camera {
            eye: Vec3::new(0.0, 0.0, 3_000.0),
            target: Vec3::new(0.0, 0.0, 0.0),
            up: Vec3::Y,
            projection: Perspective {
                fov_y: 2.0 * (250.0f32 / 2_600.0).atan(),
                near: 100.0,
                far: 4_000.0,
            },
        };
        render(
            r,
            448,
            448,
            &camera,
            MeshDebug {
                shading: MeshShading::BaseColor,
                wireframe: Some([0.08, 0.08, 0.1, 1.0]),
                normals: None,
            },
        )
    })
}
