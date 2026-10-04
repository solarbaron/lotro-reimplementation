// SPDX-License-Identifier: GPL-3.0-or-later
//! Golden-image test of the renderer's own deterministic test scene.
//!
//! The scene exercises every primitive the renderer offers in one frame: a perspective
//! world pass (Z-up camera, ground grid, axis gizmo, box outlines, depth-tested opaque
//! quads with nearest and linear filtering, sRGB and linear textures, an alpha-blended
//! quad) followed by a screen-space overlay (solid rectangles, an atlas sub-rectangle,
//! a rendered-to-texture inset and a debug line).
//!
//! The golden PNG lives in `tests/golden/` and is produced by this test itself
//! (`TESSERA_BLESS=1 cargo test -p tessera-render --test golden`); all textures are
//! generated procedurally here. See `tests/support/golden.rs` for the comparison rules.

#[path = "support/golden.rs"]
mod golden;
#[path = "support/gpu.rs"]
mod gpu;

use std::f32::consts::FRAC_PI_2;
use std::path::Path;

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    Camera, ColorSpace, Filter, OffscreenFormat, PassSpace, Quad, RenderError, Renderer, RgbaImage,
    TextureUpload, Wrap,
};

/// Size of the golden image.
const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const CLEAR: [f32; 4] = [0.02, 0.03, 0.05, 1.0];

/// Two-tone checkerboard, `size × size` with `cell`-texel squares.
fn checker(size: u32, cell: u32, a: [u8; 4], b: [u8; 4]) -> Vec<u8> {
    (0..size * size)
        .flat_map(|i| {
            let (x, y) = (i % size, i / size);
            if ((x / cell) + (y / cell)).is_multiple_of(2) {
                a
            } else {
                b
            }
        })
        .collect()
}

/// Smooth RGB gradient (red along x, green along y, constant blue).
fn gradient(size: u32) -> Vec<u8> {
    let max = (size - 1).max(1);
    (0..size * size)
        .flat_map(|i| {
            let (x, y) = (i % size, i / size);
            [(x * 255 / max) as u8, (y * 255 / max) as u8, 96, 255]
        })
        .collect()
}

/// A quad standing upright (local XY rotated into the XZ plane), centred at `at`.
fn upright(at: Vec3, w: f32, h: f32) -> Quad {
    Quad::new(
        Mat4::from_translation(at)
            * Mat4::from_rotation_x(FRAC_PI_2)
            * Mat4::from_scale(Vec3::new(w, h, 1.0)),
    )
}

/// Render the test scene into a fresh `WIDTH × HEIGHT` sRGB target.
fn render_scene(r: &mut Renderer<'static>) -> Result<RgbaImage, RenderError> {
    let checker_tex = r.upload_texture(&TextureUpload {
        label: Some("golden-checker"),
        filter: Filter::Nearest,
        ..TextureUpload::rgba8(
            32,
            32,
            &checker(32, 4, [0xd8, 0xa0, 0x3c, 0xff], [0x2a, 0x4a, 0x6e, 0xff]),
        )
    })?;
    let gradient_tex = r.upload_texture(&TextureUpload {
        label: Some("golden-gradient"),
        filter: Filter::Linear,
        wrap: Wrap::Clamp,
        color_space: ColorSpace::Linear,
        ..TextureUpload::rgba8(16, 16, &gradient(16))
    })?;
    // 2x2 atlas: red, green / blue, white.
    #[rustfmt::skip]
    let atlas = r.upload_texture(&TextureUpload {
        label: Some("golden-atlas"),
        filter: Filter::Nearest,
        wrap: Wrap::Clamp,
        ..TextureUpload::rgba8(2, 2, &[
            255, 0, 0, 255,    0, 255, 0, 255,
            0, 0, 255, 255,    255, 255, 255, 255,
        ])
    })?;

    // Inset: a small render-to-texture target with its own tiny scene.
    let inset = r.create_offscreen_target(48, 48, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&inset)?;
    frame
        .pass("inset", PassSpace::Screen)
        .clear([0.15, 0.0, 0.2, 1.0])
        .quad(Quad::screen_rect(8.0, 8.0, 32.0, 32.0).with_color([1.0, 0.8, 0.0, 1.0]))
        .quad(Quad::screen_rect(20.0, 20.0, 20.0, 20.0).with_color([0.0, 0.6, 1.0, 0.5]));
    r.end_frame(frame)?;
    let inset_tex = r.register_target_texture(&inset, Filter::Nearest, Wrap::Clamp)?;

    let camera = Camera {
        eye: Vec3::new(4.5, -7.0, 4.0),
        target: Vec3::new(0.0, 0.0, 1.0),
        up: Vec3::Z,
        ..Camera::default()
    };
    let target = r.create_offscreen_target(WIDTH, HEIGHT, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("world", &camera)
        .clear(CLEAR)
        .grid(Vec3::ZERO, Vec3::X, Vec3::Y, 1.0, 6, [0.18, 0.2, 0.24, 1.0])
        .axes(Vec3::new(0.0, 0.0, 0.01), 1.5)
        .aabb(
            Vec3::new(2.0, 1.0, 0.0),
            Vec3::new(3.0, 2.0, 1.5),
            [0.3, 0.9, 0.5, 1.0],
        )
        // Opaque backing panel, then the checkerboard just in front of it.
        .quad(upright(Vec3::new(-1.0, 0.02, 1.25), 2.66, 2.66).with_color([0.09, 0.08, 0.07, 1.0]))
        .quad(upright(Vec3::new(-1.0, 0.0, 1.25), 2.5, 2.5).with_texture(checker_tex))
        // Linear-filtered gradient on the ground, partly hidden by the panel (depth test).
        .quad(
            Quad::new(
                Mat4::from_translation(Vec3::new(1.5, -1.0, 0.005))
                    * Mat4::from_scale(Vec3::new(2.0, 2.0, 1.0)),
            )
            .with_texture(gradient_tex),
        )
        // Half-transparent tinted quad in front of everything else, drawn last.
        .quad(upright(Vec3::new(0.6, -1.6, 0.9), 1.2, 1.2).with_color([0.9, 0.2, 0.3, 0.5]));
    frame
        .pass("overlay", PassSpace::Screen)
        .quad(Quad::screen_rect(8.0, 8.0, 68.0, 68.0).with_color([0.0, 0.0, 0.0, 1.0]))
        .quad(Quad::screen_rect(10.0, 10.0, 64.0, 64.0).with_texture(inset_tex))
        .quad(
            Quad::screen_rect(WIDTH as f32 - 40.0, 8.0, 32.0, 32.0)
                .with_texture(atlas)
                .with_uv([0.5, 0.0], [0.5, 0.5]),
        )
        .quad(
            Quad::screen_rect(WIDTH as f32 - 40.0, 44.0, 32.0, 32.0)
                .with_texture(atlas)
                .with_uv([0.0, 0.0], [1.0, 1.0]),
        )
        .line(
            Vec3::new(8.0, HEIGHT as f32 - 8.5, 0.0),
            Vec3::new(WIDTH as f32 - 8.0, HEIGHT as f32 - 8.5, 0.0),
            [1.0, 1.0, 1.0, 1.0],
        );
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.passes, 2);
    r.read_pixels(&target)
}

#[test]
fn render_test_scene_matches_golden() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer(CLEAR) else {
        return Ok(());
    };
    let img = render_scene(&mut r)?;
    assert_eq!((img.width, img.height), (WIDTH, HEIGHT));

    // Rendering twice on the same device is bit-identical.
    let again = render_scene(&mut r)?;
    assert!(img.data == again.data, "scene render is not deterministic");

    golden::check(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/render_test_scene.png"),
        &img,
        golden::Tolerance::default(),
    );
    Ok(())
}

/// The comparison itself: tolerance, neighbourhood, outlier budget and the diff image.
#[test]
fn golden_comparison_rules() {
    let exact = golden::Tolerance {
        per_channel: 4,
        neighbourhood: 0,
        max_outlier_fraction: 0.0,
    };
    let base = RgbaImage {
        width: 10,
        height: 10,
        data: vec![100; 400],
    };
    let mut near = base.clone();
    near.data[0] = 104; // within ±4
    let (cmp, _) = golden::compare(&base, &near, exact);
    assert_eq!((cmp.outliers, cmp.max_diff), (0, 4));
    assert!(cmp.passes(exact));

    let mut off = base.clone();
    off.data[4 * 13 + 1] = 0; // pixel (3, 1)
    let (cmp, diff) = golden::compare(&base, &off, exact);
    assert_eq!((cmp.outliers, cmp.strict_mismatches), (1, 1));
    assert_eq!(cmp.first_outlier.map(|o| (o.0, o.1)), Some((3, 1)));
    assert_eq!(
        diff.pixel(3, 1).map(|p| p[0] >= 128 && p[1] == 0),
        Some(true)
    );
    assert!(!cmp.passes(exact));
    assert!(cmp.passes(golden::Tolerance {
        max_outlier_fraction: 0.01,
        ..exact
    }));

    // A one-pixel-wide vertical line: shifted by one column it is forgiven by the
    // neighbourhood search, removed entirely it is not.
    let line_at = |col: u32| RgbaImage {
        width: 10,
        height: 10,
        data: (0..100u32)
            .flat_map(|i| {
                if i % 10 == col {
                    [255, 255, 255, 255]
                } else {
                    [0, 0, 0, 255]
                }
            })
            .collect(),
    };
    let fuzzy = golden::Tolerance {
        neighbourhood: 1,
        ..exact
    };
    let (cmp, _) = golden::compare(&line_at(4), &line_at(5), fuzzy);
    assert_eq!((cmp.outliers, cmp.strict_mismatches), (0, 20));
    let (cmp, _) = golden::compare(&line_at(4), &line_at(99), fuzzy);
    assert_eq!(cmp.outliers, 10, "a missing line is still an outlier");
    let (cmp, _) = golden::compare(&line_at(99), &line_at(4), fuzzy);
    assert_eq!(cmp.outliers, 10, "an extra line is still an outlier");

    // PNG round trip.
    let path = golden::artifact_dir().join("selftest/roundtrip.png");
    golden::write_png(&path, &off).expect("write");
    assert_eq!(golden::read_png(&path).expect("read"), off);
}
