// SPDX-License-Identifier: GPL-3.0-or-later
//! Headless golden-image tests. Each test skips (with a printed reason) when
//! no GPU adapter — hardware or software (llvmpipe / lavapipe) — is available.

#[path = "support/gpu.rs"]
mod gpu;

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    Camera, ColorSpace, Filter, OffscreenFormat, OrbitCamera, OrbitConfig, PassSpace, Quad,
    RenderError, Renderer, RgbaImage, TextureUpload, UpAxis, Wrap,
};

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// 2x2 texture: top-left red, top-right green, bottom-left blue, bottom-right white.
#[rustfmt::skip]
const QUADRANTS: [u8; 16] = [
    255, 0, 0, 255,    0, 255, 0, 255,
    0, 0, 255, 255,    255, 255, 255, 255,
];

fn near(a: Option<[u8; 4]>, b: [u8; 4], tol: u8) -> bool {
    a.is_some_and(|a| a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= tol))
}

/// Draw one full-target textured quad and read it back.
fn draw_full(
    r: &mut Renderer<'static>,
    target: &tessera_render::OffscreenTarget,
    quad: Quad,
) -> Result<RgbaImage, RenderError> {
    let (w, h) = target.size();
    let mut frame = r.begin_offscreen_frame(target)?;
    frame
        .pass("full", PassSpace::Screen)
        .clear(BLACK)
        .quad(Quad {
            transform: Quad::screen_rect(0.0, 0.0, w as f32, h as f32).transform,
            ..quad
        });
    r.end_frame(frame)?;
    r.read_pixels(target)
}

/// Shared adapter search and GPU lock (`support/gpu.rs`), default clear colour.
fn renderer() -> Option<gpu::Headless> {
    gpu::headless_renderer_with(|_| {})
}

#[test]
fn solid_quad_64x64() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let target = r.create_offscreen_target(64, 64, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .pass("overlay", PassSpace::Screen)
        .clear(BLACK)
        .quad(Quad::screen_rect(16.0, 16.0, 32.0, 32.0).with_color(RED));
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.quads, 1);
    assert_eq!(stats.draw_calls, 1);

    let img = r.read_pixels(&target)?;
    assert_eq!(
        (img.width, img.height, img.data.len()),
        (64, 64, 64 * 64 * 4)
    );
    for (x, y) in [(16, 16), (32, 32), (47, 47), (16, 47), (47, 16)] {
        assert_eq!(img.pixel(x, y), Some([255, 0, 0, 255]), "inside at {x},{y}");
    }
    for (x, y) in [(0, 0), (15, 32), (48, 32), (32, 15), (32, 48), (63, 63)] {
        assert_eq!(img.pixel(x, y), Some([0, 0, 0, 255]), "outside at {x},{y}");
    }
    // Exactly the 32x32 block is red.
    let red = img
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| **p == [255, 0, 0, 255])
        .count();
    assert_eq!(red, 32 * 32);
    Ok(())
}

#[test]
fn textured_quad_nearest_quadrants() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    // 2x2 texture: top-left red, top-right green, bottom-left blue, bottom-right white.
    #[rustfmt::skip]
    let texels = [
        255, 0, 0, 255,    0, 255, 0, 255,
        0, 0, 255, 255,    255, 255, 255, 255,
    ];
    let tex = r.upload_texture(&TextureUpload {
        filter: Filter::Nearest,
        wrap: Wrap::Clamp,
        ..TextureUpload::rgba8(2, 2, &texels)
    })?;
    assert_eq!(r.texture_size(tex), Some((2, 2)));

    let target = r.create_offscreen_target(64, 64, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .pass("tex", PassSpace::Screen)
        .clear(BLACK)
        .quad(Quad::screen_rect(0.0, 0.0, 64.0, 64.0).with_texture(tex));
    r.end_frame(frame)?;
    let img = r.read_pixels(&target)?;
    assert_eq!(img.pixel(8, 8), Some([255, 0, 0, 255]));
    assert_eq!(img.pixel(56, 8), Some([0, 255, 0, 255]));
    assert_eq!(img.pixel(8, 56), Some([0, 0, 255, 255]));
    assert_eq!(img.pixel(56, 56), Some([255, 255, 255, 255]));

    // Update in place, then destroy; stale handles are rejected.
    r.update_texture(tex, &[0u8; 16])?;
    assert!(r.update_texture(tex, &[0u8; 15]).is_err());
    assert!(r.destroy_texture(tex));
    assert!(!r.destroy_texture(tex));
    assert!(matches!(
        r.update_texture(tex, &[0u8; 16]),
        Err(RenderError::UnknownTexture(_))
    ));
    // Drawing a stale texture falls back to white instead of failing.
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .pass("stale", PassSpace::Screen)
        .quad(Quad::screen_rect(0.0, 0.0, 64.0, 64.0).with_texture(tex));
    r.end_frame(frame)?;
    let img = r.read_pixels(&target)?;
    assert_eq!(img.pixel(8, 8), Some([255, 255, 255, 255]));
    Ok(())
}

#[test]
fn world_pass_depth_and_lines() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let target = r.create_offscreen_target(64, 64, OffscreenFormat::Rgba8Srgb)?;
    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 5.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        ..Camera::default()
    };
    let mut frame = r.begin_offscreen_frame(&target)?;
    {
        let pass = frame.world_pass("world", &camera).clear(BLACK);
        // Near green quad drawn first, far red quad second: depth must keep green.
        pass.quad(
            Quad::new(
                Mat4::from_translation(Vec3::new(0.0, 0.0, 1.0))
                    * Mat4::from_scale(Vec3::splat(2.0)),
            )
            .with_color([0.0, 1.0, 0.0, 1.0]),
        );
        pass.quad(
            Quad::new(
                Mat4::from_translation(Vec3::new(0.0, 0.0, -1.0))
                    * Mat4::from_scale(Vec3::splat(10.0)),
            )
            .with_color(RED),
        );
    }
    // Screen-space horizontal blue line across row 2 (pixel centres at .5).
    frame.pass("debug", PassSpace::Screen).line(
        Vec3::new(0.0, 2.5, 0.0),
        Vec3::new(64.0, 2.5, 0.0),
        [0.0, 0.0, 1.0, 1.0],
    );
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.passes, 2);
    assert_eq!(stats.lines, 1);

    let img = r.read_pixels(&target)?;
    assert_eq!(
        img.pixel(32, 32),
        Some([0, 255, 0, 255]),
        "near quad wins depth test"
    );
    assert_eq!(
        img.pixel(32, 60),
        Some([255, 0, 0, 255]),
        "far quad visible around"
    );
    assert_eq!(img.pixel(32, 2), Some([0, 0, 255, 255]), "debug line drawn");
    Ok(())
}

#[test]
fn empty_frame_clears_and_targets_validate() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    assert!(matches!(
        r.create_offscreen_target(0, 4, OffscreenFormat::Rgba8Srgb),
        Err(RenderError::InvalidTarget(_))
    ));
    assert!(matches!(r.begin_frame(), Err(RenderError::NoSurface)));
    assert!(
        r.upload_texture(&TextureUpload::rgba8(4, 4, &[0; 3]))
            .is_err()
    );

    r.set_clear_color([0.0, 0.0, 1.0, 1.0]);
    // Different sizes exercise the depth cache and buffer growth paths.
    for size in [1u32, 17, 64, 300] {
        let target = r.create_offscreen_target(size, size, OffscreenFormat::Rgba8Srgb)?;
        let frame = r.begin_offscreen_frame(&target)?;
        r.end_frame(frame)?;
        let img = r.read_pixels(&target)?;
        assert_eq!(img.pixel(size - 1, size - 1), Some([0, 0, 255, 255]));
    }
    // Many quads force the instance buffer to grow.
    let target = r.create_offscreen_target(64, 64, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let pass = frame.world_pass(
        "many",
        &Camera {
            eye: Vec3::new(0.0, 0.0, 5.0),
            up: Vec3::Y,
            ..Camera::default()
        },
    );
    for i in 0..10_000 {
        pass.quad(Quad::new(Mat4::from_translation(Vec3::new(
            i as f32, 0.0, -100.0,
        ))));
    }
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.quads, 10_000);
    Ok(())
}

/// Two world passes in one frame: the shared depth buffer is cleared by
/// `clear()` / `clear_depth()` and otherwise kept.
#[test]
fn multiple_world_passes_and_depth_clearing() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let target = r.create_offscreen_target(64, 64, OffscreenFormat::Rgba8Srgb)?;
    let camera = Camera {
        eye: Vec3::new(0.0, 0.0, 5.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        ..Camera::default()
    };
    let near_green = Quad::new(Mat4::from_translation(Vec3::new(0.0, 0.0, 2.0))).with_color(GREEN);
    let far_red = Quad::new(
        Mat4::from_translation(Vec3::new(0.0, 0.0, -5.0)) * Mat4::from_scale(Vec3::splat(10.0)),
    )
    .with_color(RED);

    // 0: second pass clears colour (+ depth): its far quad is visible.
    // 1: second pass does not clear: shared depth keeps the near quad.
    // 2: second pass clears depth only: far quad drawn, corners keep pass 1's clear.
    for mode in 0..3 {
        let mut frame = r.begin_offscreen_frame(&target)?;
        frame.world_pass("a", &camera).clear(BLACK).quad(near_green);
        let b = frame.world_pass("b", &camera);
        match mode {
            0 => {
                b.clear(BLUE);
            }
            2 => {
                b.clear_depth();
            }
            _ => {}
        }
        b.quad(far_red);
        r.end_frame(frame)?;
        let img = r.read_pixels(&target)?;
        let (centre, corner) = match mode {
            0 => ([255, 0, 0, 255], [0, 0, 255, 255]),
            1 => ([0, 255, 0, 255], [0, 0, 0, 255]),
            _ => ([255, 0, 0, 255], [0, 0, 0, 255]),
        };
        assert_eq!(img.pixel(32, 32), Some(centre), "mode {mode} centre");
        assert_eq!(img.pixel(0, 0), Some(corner), "mode {mode} corner");
    }
    Ok(())
}

#[test]
fn colour_spaces_uv_rects_wrap_and_blending() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let target = r.create_offscreen_target(64, 64, OffscreenFormat::Rgba8Unorm)?;

    // Linear data is passed through unchanged to a Unorm target; the same
    // bytes uploaded as sRGB are decoded (darker).
    let texel = [100u8, 150, 200, 255];
    let linear = r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Linear,
        filter: Filter::Nearest,
        ..TextureUpload::rgba8(1, 1, &texel)
    })?;
    let srgb = r.upload_texture(&TextureUpload {
        filter: Filter::Nearest,
        ..TextureUpload::rgba8(1, 1, &texel)
    })?;
    let img = draw_full(
        &mut r,
        &target,
        Quad::new(Mat4::IDENTITY).with_texture(linear),
    )?;
    assert!(near(img.pixel(32, 32), texel, 1), "{:?}", img.pixel(32, 32));
    let img = draw_full(
        &mut r,
        &target,
        Quad::new(Mat4::IDENTITY).with_texture(srgb),
    )?;
    // sRGB 100/255 decodes to ~0.127 linear → ~32.
    assert!(
        near(img.pixel(32, 32), [32, 78, 147, 255], 2),
        "{:?}",
        img.pixel(32, 32)
    );

    // UV sub-rectangles select one quadrant (atlas / glyph use).
    let quads = r.upload_texture(&TextureUpload {
        filter: Filter::Nearest,
        wrap: Wrap::Clamp,
        ..TextureUpload::rgba8(2, 2, &QUADRANTS)
    })?;
    for (offset, expected) in [
        ([0.0, 0.0], [255, 0, 0, 255]),
        ([0.5, 0.0], [0, 255, 0, 255]),
        ([0.0, 0.5], [0, 0, 255, 255]),
        ([0.5, 0.5], [255, 255, 255, 255]),
    ] {
        let q = Quad::new(Mat4::IDENTITY)
            .with_texture(quads)
            .with_uv(offset, [0.5, 0.5]);
        let img = draw_full(&mut r, &target, q)?;
        for (x, y) in [(1, 1), (62, 1), (1, 62), (62, 62), (32, 32)] {
            assert_eq!(img.pixel(x, y), Some(expected), "uv {offset:?} at {x},{y}");
        }
    }

    // Repeat tiles, Clamp smears the edge texel; ClampU and ClampV clamp one axis only.
    for wrap in [Wrap::Repeat, Wrap::Clamp, Wrap::ClampU, Wrap::ClampV] {
        let tex = r.upload_texture(&TextureUpload {
            filter: Filter::Nearest,
            wrap,
            ..TextureUpload::rgba8(2, 2, &QUADRANTS)
        })?;
        let q = Quad::new(Mat4::IDENTITY)
            .with_texture(tex)
            .with_uv([0.0, 0.0], [2.0, 2.0]);
        let img = draw_full(&mut r, &target, q)?;
        assert_eq!(img.pixel(8, 8), Some([255, 0, 0, 255]), "{wrap:?}");
        assert_eq!(img.pixel(24, 8), Some([0, 255, 0, 255]), "{wrap:?}");
        let (clamp_u, clamp_v) = wrap.clamps();
        let third_column = if clamp_u {
            [0, 255, 0, 255]
        } else {
            [255, 0, 0, 255]
        };
        assert_eq!(img.pixel(40, 8), Some(third_column), "{wrap:?}");
        let third_row = if clamp_v {
            [0, 0, 255, 255]
        } else {
            [255, 0, 0, 255]
        };
        assert_eq!(img.pixel(8, 40), Some(third_row), "{wrap:?}");
    }

    // 50 % white over opaque black.
    let img = draw_full(
        &mut r,
        &target,
        Quad::new(Mat4::IDENTITY).with_color([1.0, 1.0, 1.0, 0.5]),
    )?;
    assert!(
        near(img.pixel(32, 32), [128, 128, 128, 255], 1),
        "{:?}",
        img.pixel(32, 32)
    );
    Ok(())
}

/// A Z-up orbit camera renders "up" at the top and +X on the right.
#[test]
fn z_up_orbit_camera_render() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let target = r.create_offscreen_target(64, 64, OffscreenFormat::Rgba8Srgb)?;
    let cam = OrbitCamera::new(OrbitConfig {
        up: UpAxis::Z,
        pitch: 0.0,
        pivot_height: 0.0,
        ..OrbitConfig::default()
    });
    // Quads standing in the XZ plane face a camera on the -Y side.
    let upright = |at: Vec3, s: f32| {
        Quad::new(
            Mat4::from_translation(at)
                * Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2)
                * Mat4::from_scale(Vec3::splat(s)),
        )
    };
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .world_pass("z-up", &cam.camera())
        .clear(BLACK)
        .quad(upright(Vec3::ZERO, 0.5).with_color(GREEN))
        .quad(upright(Vec3::new(0.0, 0.0, 1.0), 0.5).with_color(RED))
        .quad(upright(Vec3::new(1.0, 0.0, 0.0), 0.5).with_color(BLUE));
    r.end_frame(frame)?;
    let img = r.read_pixels(&target)?;
    assert_eq!(
        img.pixel(32, 32),
        Some([0, 255, 0, 255]),
        "pivot in the centre"
    );
    // 1 unit at distance 6 with a 60 degree fov is ~9 px off-centre.
    assert_eq!(img.pixel(32, 23), Some([255, 0, 0, 255]), "+Z is up");
    assert_eq!(img.pixel(32, 41), Some([0, 0, 0, 255]));
    assert_eq!(img.pixel(41, 32), Some([0, 0, 255, 255]), "+X is right");
    assert_eq!(img.pixel(23, 32), Some([0, 0, 0, 255]));
    Ok(())
}

/// Handles from one renderer are rejected by another instead of panicking
/// inside wgpu or aliasing the other renderer's resources.
#[test]
fn handles_are_not_shared_between_renderers() -> Result<(), RenderError> {
    let (Some(mut r1), Some(mut r2)) = (renderer(), renderer()) else {
        return Ok(());
    };
    let t1 = r1.create_offscreen_target(8, 8, OffscreenFormat::Rgba8Srgb)?;
    let t2 = r2.create_offscreen_target(8, 8, OffscreenFormat::Rgba8Srgb)?;

    assert!(matches!(
        r2.begin_offscreen_frame(&t1),
        Err(RenderError::ForeignHandle(_))
    ));
    assert!(matches!(
        r2.read_pixels(&t1),
        Err(RenderError::ForeignHandle(_))
    ));
    assert!(matches!(
        r2.register_target_texture(&t1, Filter::Nearest, Wrap::Clamp),
        Err(RenderError::ForeignHandle(_))
    ));
    let frame = r1.begin_offscreen_frame(&t1)?;
    assert!(matches!(
        r2.end_frame(frame),
        Err(RenderError::ForeignHandle(_))
    ));

    let a = r1.upload_texture(&TextureUpload::rgba8(1, 1, &[1; 4]))?;
    let b = r2.upload_texture(&TextureUpload::rgba8(2, 2, &[2; 16]))?;
    assert_eq!(r2.texture_size(a), None);
    assert!(!r2.destroy_texture(a));
    assert!(matches!(
        r2.update_texture(a, &[0; 4]),
        Err(RenderError::UnknownTexture(_))
    ));
    assert_eq!(r2.texture_size(b), Some((2, 2)), "b untouched");
    assert_eq!(r1.texture_size(a), Some((1, 1)), "a untouched");

    // A foreign texture in a quad is drawn white, not as r2's texture.
    let mut frame = r2.begin_offscreen_frame(&t2)?;
    frame
        .pass("foreign", PassSpace::Screen)
        .clear(BLACK)
        .quad(Quad::screen_rect(0.0, 0.0, 8.0, 8.0).with_texture(a));
    r2.end_frame(frame)?;
    assert_eq!(r2.read_pixels(&t2)?.pixel(4, 4), Some([255, 255, 255, 255]));

    // Both renderers still work.
    let frame = r1.begin_offscreen_frame(&t1)?;
    r1.end_frame(frame)?;
    r1.read_pixels(&t1)?;
    Ok(())
}

/// Render-to-texture: an offscreen target drawn as a quad texture.
#[test]
fn offscreen_target_as_texture() -> Result<(), RenderError> {
    let Some(mut r) = renderer() else {
        return Ok(());
    };
    let inner = r.create_offscreen_target(8, 8, OffscreenFormat::Rgba8Srgb)?;
    let outer = r.create_offscreen_target(32, 32, OffscreenFormat::Rgba8Srgb)?;
    let mut frame = r.begin_offscreen_frame(&inner)?;
    frame.pass("inner", PassSpace::Screen).clear(RED);
    r.end_frame(frame)?;

    let tex = r.register_target_texture(&inner, Filter::Nearest, Wrap::Clamp)?;
    assert_eq!(r.texture_size(tex), Some((8, 8)));
    assert!(matches!(
        r.update_texture(tex, &[0; 8 * 8 * 4]),
        Err(RenderError::InvalidTexture(_))
    ));
    let img = draw_full(&mut r, &outer, Quad::new(Mat4::IDENTITY).with_texture(tex))?;
    assert_eq!(img.pixel(16, 16), Some([255, 0, 0, 255]));

    // Sampling the frame's own target would be a feedback loop: drawn white.
    let mut frame = r.begin_offscreen_frame(&inner)?;
    frame
        .pass("loop", PassSpace::Screen)
        .clear(BLACK)
        .quad(Quad::screen_rect(0.0, 0.0, 8.0, 8.0).with_texture(tex));
    r.end_frame(frame)?;
    assert_eq!(
        r.read_pixels(&inner)?.pixel(4, 4),
        Some([255, 255, 255, 255])
    );

    // Dropping the registration leaves the target itself usable.
    assert!(r.destroy_texture(tex));
    assert_eq!(r.texture_size(tex), None);
    let mut frame = r.begin_offscreen_frame(&inner)?;
    frame.pass("after", PassSpace::Screen).clear(GREEN);
    r.end_frame(frame)?;
    assert_eq!(r.read_pixels(&inner)?.pixel(0, 0), Some([0, 255, 0, 255]));
    Ok(())
}

/// An adapter that matches but cannot open a device is not reported as "no
/// adapter": the GPU test harness skips on `NoAdapter` and must fail instead.
#[test]
fn device_failures_are_not_reported_as_no_adapter() {
    // Only meaningful where an adapter exists at all.
    let Some(r) = renderer() else {
        return;
    };
    let config = tessera_render::RendererConfig {
        context: tessera_render::ContextOptions {
            backends: Some(tessera_render::wgpu::Backends::all()),
            allow_software: true,
            // No device can satisfy this, so every candidate's request fails.
            required_limits: Some(tessera_render::wgpu::Limits {
                max_texture_dimension_2d: u32::MAX,
                ..tessera_render::wgpu::Limits::default()
            }),
            ..tessera_render::ContextOptions::default()
        },
        ..tessera_render::RendererConfig::default()
    };
    match Renderer::new_headless(&config) {
        Ok(_) => panic!("a device with impossible limits was created"),
        Err(RenderError::NoDevice { reason, .. }) => assert!(!reason.is_empty()),
        Err(RenderError::RequestDevice(_)) => {}
        Err(e) => panic!("wrong error for failed device requests: {e}"),
    }
    drop(r);
}
