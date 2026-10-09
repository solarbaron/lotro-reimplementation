// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-back tests of the UI pass (`docs/specs/render.md` §6.13 T1–T15, the renderer side of
//! T16–T18, and the renderer side of the P1 composition checks of §6.14). Every canvas is
//! synthetic. The tests skip (and pass) where no GPU adapter exists, like the other render
//! tests (`tests/support/gpu.rs`).

#[path = "support/gpu.rs"]
mod gpu;
#[path = "ui_common/mod.rs"]
mod ui_common;

use tessera_render::{
    Filter, MAX_UI_ENTRIES, OffscreenFormat, PassSpace, Quad, RenderError, TextureUpload,
    UiAddressing, UiCapCounters, UiClip, UiEntry, UiFilter, UiRect, UiRotation, UiSource,
    UiViewMode, Wrap,
};
use ui_common::{
    BG, MARKER, P1, RIM, SIZE_P1, centres, diff, disc_alpha, inside, outside, px, render_ui, rot_z,
    solid, ui_texture,
};

const SIZE: (u32, u32) = (256, 256);
const SRGB: OffscreenFormat = OffscreenFormat::Rgba8Srgb;
const UNORM: OffscreenFormat = OffscreenFormat::Rgba8Unorm;

/// Straight source-over of a canvas pixel over an opaque stored background, rounded.
fn over(c: [u8; 4], d: [u8; 4]) -> [u8; 4] {
    let a = f64::from(c[3]) / 255.0;
    let ch = |i: usize| (f64::from(c[i]) * a + f64::from(d[i]) * (1.0 - a)).round() as u8;
    [ch(0), ch(1), ch(2), 255]
}

/// A canvas pixel mixing opaque, transparent and partial alpha.
fn mixed(x: u32, y: u32) -> [u8; 4] {
    let a = match (x + y) % 4 {
        0 => 255,
        1 => 0,
        2 => (x * 37 + y * 11) as u8,
        _ => 128,
    };
    [(x * 7) as u8, (y * 5 + 3) as u8, ((x ^ y) * 9) as u8, a]
}

/// T1 and invariant 1: the 1 : 1 copy is exact where alpha is 0 or 255 and within ±1
/// elsewhere, on both target kinds.
#[test]
fn t1_one_to_one_is_exact() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let mode = r.ui_view_mode();
    eprintln!("UI view mode: {mode:?}");
    for format in [SRGB, UNORM] {
        // In the fallback (no non-sRGB view, e.g. GL) blended pixels of an sRGB target are
        // blended in linear light: a recorded known difference (render.md §6.4).
        let blended_exact = format == UNORM || mode == UiViewMode::Stored;
        let tex = ui_texture(&mut r, 64, 48, mixed)?;
        let (img, stats) = render_ui(&mut r, SIZE, format, |p| {
            p.entry(&UiEntry::new(tex, [64, 48], UiRect::new(13, 21, 64, 48)));
        })?;
        assert_eq!(stats.ui.quads, 2);
        assert_eq!(stats.ui.entries_refused, 0);
        for y in 0..48 {
            for x in 0..64 {
                let c = mixed(x, y);
                let got = px(&img, 13 + x, 21 + y);
                match c[3] {
                    255 => assert_eq!(got, [c[0], c[1], c[2], 255], "{format:?} ({x}, {y})"),
                    0 => assert_eq!(got, BG, "{format:?} ({x}, {y})"),
                    _ if blended_exact => assert!(
                        diff(got, over(c, BG)) <= 1,
                        "{format:?} ({x}, {y}): {got:?}"
                    ),
                    _ => {}
                }
            }
        }
        assert_eq!(px(&img, 12, 21), BG);
        assert_eq!(px(&img, 77, 68), BG);
        r.destroy_texture(tex);
    }
    Ok(())
}

/// T2 and invariant 5: sub-rectangle updates with a padded stride, bounds errors, zero-size
/// no-ops, and copy → update → clear issued in one frame taking effect in call order.
#[test]
fn t2_sub_rectangles_run_in_call_order() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let red = [200, 30, 30, 255];
    let green = [30, 200, 60, 255];
    let front = solid(&mut r, 16, 16, red)?;
    let back = r.create_ui_texture(16, 16, Some("back"))?;
    assert!(r.is_ui_texture(back));

    // Bounds and argument errors, each refused without effect.
    let bytes = vec![0u8; 16 * 16 * 4];
    let bad = [
        r.update_texture_rect(back, 10, 0, 7, 1, &bytes, 64),
        r.update_texture_rect(back, 0, 16, 1, 1, &bytes, 64),
        r.update_texture_rect(back, u32::MAX, 0, 2, 1, &bytes, 64),
        r.update_texture_rect(back, 0, 0, 4, 2, &bytes, 15),
        r.update_texture_rect(back, 0, 0, 16, 16, &bytes[..1023], 64),
        r.copy_texture_rect(back, 0, 0, back, 0, 0, 1, 1),
        r.copy_texture_rect(front, 8, 8, back, 0, 0, 9, 1),
        r.clear_texture_rect(back, 0, 0, 17, 1),
    ];
    for (i, e) in bad.into_iter().enumerate() {
        assert!(
            matches!(e, Err(RenderError::InvalidTexture(_))),
            "case {i}: {e:?}"
        );
    }
    let rgba8 = r.upload_texture(&TextureUpload::rgba8(4, 4, &[255; 64]))?;
    assert!(
        r.clear_texture_rect(rgba8, 0, 0, 1, 1).is_err(),
        "clear needs a UI texture"
    );
    assert!(r.copy_texture_rect(front, 0, 0, rgba8, 0, 0, 1, 1).is_err());
    // A sub-rectangle update of an RGBA8 texture is allowed.
    r.update_texture_rect(rgba8, 1, 1, 2, 2, &[7; 16], 8)?;
    // Zero-size rectangles do nothing.
    r.update_texture_rect(back, 3, 3, 0, 5, &[], 0)?;
    r.copy_texture_rect(front, 0, 0, back, 0, 0, 0, 0)?;
    r.clear_texture_rect(back, 0, 0, 5, 0)?;

    let target = r.create_offscreen_target(SIZE.0, SIZE.1, SRGB)?;
    let bg = solid(&mut r, 1, 1, BG)?;
    // An empty frame resets the per-frame counters.
    let frame = r.begin_offscreen_frame(&target)?;
    r.end_frame(frame)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    // In one frame: copy front → back, update a rectangle (stride padded by 12 bytes), clear
    // another that overlaps the update.
    r.copy_texture_rect(front, 0, 0, back, 0, 0, 16, 16)?;
    let stride = 8 * 4 + 12;
    let mut upd = vec![0xEEu8; stride * 8];
    for row in upd.chunks_mut(stride) {
        for p in row[..32].chunks_mut(4) {
            p.copy_from_slice(&ui_common::bgra(green));
        }
    }
    r.update_texture_rect(back, 0, 0, 8, 8, &upd, stride)?;
    r.clear_texture_rect(back, 4, 4, 8, 8)?;
    frame.pass("world", PassSpace::Screen).clear([0.0; 4]);
    frame
        .ui_pass()
        .entry(&UiEntry::new(bg, [1, 1], UiRect::new(0, 0, SIZE.0, SIZE.1)))
        .entry(&UiEntry::new(back, [16, 16], UiRect::new(0, 0, 16, 16)));
    let stats = r.end_frame(frame)?;
    let img = r.read_pixels(&target)?;
    assert_eq!(
        stats.ui.upload_bytes,
        8 * 8 * 4,
        "stride padding is not counted"
    );
    assert_eq!(stats.ui.upload_rects, 1);
    assert_eq!(stats.ui.gpu_copies, 1);
    assert_eq!(stats.ui.copy_bytes, 16 * 16 * 4);
    assert_eq!(stats.ui.clears, 1);
    for y in 0..16 {
        for x in 0..16 {
            let cleared = (4..12).contains(&x) && (4..12).contains(&y);
            let want = if cleared {
                BG
            } else if x < 8 && y < 8 {
                green
            } else {
                red
            };
            assert_eq!(px(&img, x, y), want, "({x}, {y})");
        }
    }
    // The counters cover only the calls since the previous frame.
    let frame = r.begin_offscreen_frame(&target)?;
    let stats = r.end_frame(frame)?;
    assert_eq!(
        (stats.ui.upload_bytes, stats.ui.gpu_copies, stats.ui.clears),
        (0, 0, 0)
    );
    Ok(())
}

/// T3: a scissored clear changes no texel outside its rectangle.
#[test]
fn t3_clear_is_scissored() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let tex = ui_texture(&mut r, 40, 30, |x, y| [x as u8 * 6, y as u8 * 8, 99, 255])?;
    r.clear_texture_rect(tex, 7, 5, 11, 13)?;
    let (img, _) = render_ui(&mut r, SIZE, SRGB, |p| {
        p.entry(&UiEntry::new(tex, [40, 30], UiRect::new(0, 0, 40, 30)));
    })?;
    for y in 0..30 {
        for x in 0..40 {
            let inside = (7..18).contains(&x) && (5..18).contains(&y);
            let want = if inside {
                BG
            } else {
                [x as u8 * 6, y as u8 * 8, 99, 255]
            };
            assert_eq!(px(&img, x, y), want, "({x}, {y})");
        }
    }
    Ok(())
}

/// T4 and invariant 4: texels outside the stored part read as transparent black,
/// `tex_offset` places the texture, and growing or shrinking by GPU copy keeps the content.
#[test]
fn t4_tile_box() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let pattern = |x: u32, y: u32| [100 + x as u8, 50 + y as u8, 200, 255];
    let small = ui_texture(&mut r, 32, 32, pattern)?;
    let dest = UiRect::new(10, 10, 64, 64);
    let check = |img: &tessera_render::RgbaImage, lo: u32, hi: u32, origin: u32| {
        for y in 0..64 {
            for x in 0..64 {
                let stored = (lo..hi).contains(&x) && (lo..hi).contains(&y);
                let want = if stored {
                    pattern(x - origin, y - origin)
                } else {
                    BG
                };
                assert_eq!(px(img, 10 + x, 10 + y), want, "({x}, {y})");
            }
        }
    };
    // 32 x 32 tile box at canvas (16, 16) of a 64 x 64 canvas, unscaled and scaled-path off.
    let (img, _) = render_ui(&mut r, SIZE, SRGB, |p| {
        p.entry(&UiEntry::new(small, [64, 64], dest).with_tex_offset([16, 16]));
    })?;
    check(&img, 16, 48, 16);
    // Grow to 48 x 48 at the same offset by a GPU copy; the content stays.
    let grown = r.create_ui_texture(48, 48, Some("grown"))?;
    r.copy_texture_rect(small, 0, 0, grown, 0, 0, 32, 32)?;
    assert!(r.destroy_texture(small), "destroy after a pending copy");
    let (img, _) = render_ui(&mut r, SIZE, SRGB, |p| {
        p.entry(&UiEntry::new(grown, [64, 64], dest).with_tex_offset([16, 16]));
    })?;
    check(&img, 16, 48, 16);
    // Shrink to the 16 x 16 box at canvas (24, 24).
    let shrunk = r.create_ui_texture(16, 16, Some("shrunk"))?;
    r.copy_texture_rect(grown, 8, 8, shrunk, 0, 0, 16, 16)?;
    r.destroy_texture(grown);
    let (img, _) = render_ui(&mut r, SIZE, SRGB, |p| {
        p.entry(&UiEntry::new(shrunk, [64, 64], dest).with_tex_offset([24, 24]));
    })?;
    check(&img, 24, 40, 16);
    // Bytes of live UI textures (the 1 x 1 backgrounds are destroyed by `render_ui`).
    assert_eq!(r.ui_texture_bytes(), 16 * 16 * 4);
    Ok(())
}

/// T5: a scaled entry with a transparent-black border does not darken its edge (premultiplied
/// filtering), and nearest reads the texel under the position.
#[test]
fn t5_scaled_filters() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let white = solid(&mut r, 1, 1, [255; 4])?;
    let framed = ui_texture(&mut r, 8, 8, |x, y| {
        if (1..7).contains(&x) && (1..7).contains(&y) {
            [255; 4]
        } else {
            [0; 4]
        }
    })?;
    for (filter, scale) in [
        (UiFilter::Linear, 4u32),
        (UiFilter::Linear, 3),
        (UiFilter::Nearest, 4),
    ] {
        let side = 8 * scale + 1;
        let (img, stats) = render_ui(&mut r, SIZE, SRGB, |p| {
            p.entry(&UiEntry::new(white, [1, 1], UiRect::new(20, 20, 60, 60)))
                .entry(
                    &UiEntry::new(framed, [8, 8], UiRect::new(24, 24, side, side))
                        .with_filter(filter),
                );
        })?;
        assert_eq!(stats.ui.quads, 3);
        for y in 20..80 {
            for x in 20..80 {
                let p = px(&img, x, y);
                assert!(
                    p.iter().all(|&c| c >= 254),
                    "{filter:?} x{scale}: darkened edge at ({x}, {y}): {p:?}"
                );
            }
        }
    }
    // Nearest at 2x: canvas texel floor(c).
    let checker = ui_texture(&mut r, 4, 4, |x, y| {
        [(x * 60) as u8, (y * 60) as u8, 30, 255]
    })?;
    let (img, _) = render_ui(&mut r, SIZE, SRGB, |p| {
        p.entry(
            &UiEntry::new(checker, [4, 4], UiRect::new(5, 6, 8, 8)).with_filter(UiFilter::Nearest),
        );
    })?;
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(
                px(&img, 5 + x, 6 + y),
                [(x / 2 * 60) as u8, (y / 2 * 60) as u8, 30, 255],
                "({x}, {y})"
            );
        }
    }
    Ok(())
}

/// T6 and invariant 2: the threshold drops the same pixels at opacity 1 and 0.5, and 0 drops
/// nothing.
#[test]
fn t6_alpha_threshold_before_opacity() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let black = solid(&mut r, 1, 1, [0, 0, 0, 255])?;
    let ramp = ui_texture(&mut r, 16, 1, |x, _| [255, 255, 255, (x * 16 + 8) as u8])?;
    let mut dropped_sets = Vec::new();
    for (opacity, threshold) in [(1.0, 0.5), (0.5, 0.5), (1.0, 0.0), (0.5, 0.0)] {
        let (img, _) = render_ui(&mut r, SIZE, UNORM, |p| {
            p.entry(&UiEntry::new(black, [1, 1], UiRect::new(0, 0, 32, 4)))
                .entry(
                    &UiEntry::new(ramp, [16, 1], UiRect::new(0, 0, 16, 1))
                        .with_opacity(opacity)
                        .with_alpha_threshold(threshold),
                );
        })?;
        let dropped: Vec<u32> = (0..16)
            .filter(|&x| px(&img, x, 0) == [0, 0, 0, 255])
            .collect();
        if threshold == 0.0 {
            assert!(dropped.is_empty(), "t = 0 dropped {dropped:?}");
        } else {
            assert_eq!(dropped, (0..8).collect::<Vec<_>>(), "opacity {opacity}");
            dropped_sets.push(dropped);
        }
    }
    assert_eq!(dropped_sets[0], dropped_sets[1]);
    Ok(())
}

/// T7: Tile repeats 1 : 1 from the box's top-left; at node scale 1.5 each repeat covers
/// 48 × 48 pixels with no seam darkening.
#[test]
fn t7_tile_addressing() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let pattern = |x: u32, y: u32| [(x * 8) as u8, (y * 8) as u8, 128, 255];
    let tile = ui_texture(&mut r, 32, 32, pattern)?;
    let dest = UiRect::new(5, 7, 200, 100);
    let tile_at = |s: f32| UiAddressing::Tile { node_scale: [s, s] };
    let (img, _) = render_ui(&mut r, SIZE, SRGB, |p| {
        p.entry(&UiEntry::new(tile, [32, 32], dest).with_addressing(tile_at(1.0)));
    })?;
    for y in 0..100 {
        for x in 0..200 {
            assert_eq!(
                px(&img, 5 + x, 7 + y),
                pattern(x % 32, y % 32),
                "({x}, {y})"
            );
        }
    }
    assert_eq!(px(&img, 205, 7), BG);
    // 1.5: the picture repeats every 48 pixels.
    let (img, _) = render_ui(&mut r, SIZE, SRGB, |p| {
        p.entry(&UiEntry::new(tile, [32, 32], dest).with_addressing(tile_at(1.5)));
    })?;
    for y in 0..100 - 48 {
        for x in 0..200 - 48 {
            let a = px(&img, 5 + x, 7 + y);
            assert!(diff(a, px(&img, 5 + x + 48, 7 + y)) <= 1, "({x}, {y})");
            assert!(diff(a, px(&img, 5 + x, 7 + y + 48)) <= 1, "({x}, {y})");
        }
    }
    // A uniform canvas tiled at 1.5 shows no seam at all.
    let flat = solid(&mut r, 32, 32, [180, 140, 60, 255])?;
    let (img, _) = render_ui(&mut r, SIZE, SRGB, |p| {
        p.entry(&UiEntry::new(flat, [32, 32], dest).with_addressing(tile_at(1.5)));
    })?;
    for y in 0..100 {
        for x in 0..200 {
            assert_eq!(px(&img, 5 + x, 7 + y), [180, 140, 60, 255], "({x}, {y})");
        }
    }
    Ok(())
}

/// The clip scene of T8: a root box and a nested entry reaching past its right edge, with the
/// root turned by `z` degrees about its centre.
fn clip_scene(z: f32) -> (UiRect, UiRect, [UiRotation; 1], [UiClip; 1]) {
    let root = UiRect::new(48, 48, 120, 120);
    let nested = UiRect::new(130, 70, 80, 40);
    (
        root,
        nested,
        [UiRotation::z([108.0, 108.0], z)],
        [UiClip::new(root, 0)],
    )
}

/// T8: a nested entry is cut at its root's edge, unrotated and with the root at z = 30.
#[test]
fn t8_clip_at_the_root() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let green = [40, 180, 70, 255];
    let red = [220, 40, 40, 255];
    let root_tex = solid(&mut r, 120, 120, green)?;
    let nested_tex = solid(&mut r, 80, 40, red)?;
    for z in [0.0f32, 30.0] {
        let (root, nested, rot, clip) = clip_scene(z);
        let (img, _) = render_ui(&mut r, SIZE, SRGB, |p| {
            p.entry(&UiEntry::new(root_tex, [120, 120], root).with_rotations(&rot))
                .entry(
                    &UiEntry::new(nested_tex, [80, 40], nested)
                        .with_rotations(&rot)
                        .with_clips(&clip),
                );
        })?;
        let (mut red_seen, mut cut_seen) = (0, 0);
        for (x, y, p) in centres(SIZE.0, SIZE.1) {
            // Back into the root's unrotated space.
            let q = rot_z(p, [108.0, 108.0], -f64::from(z));
            let got = px(&img, x, y);
            if inside(q, nested, 1.0) && inside(q, root, 1.0) {
                assert_eq!(got, red, "z {z} ({x}, {y})");
                red_seen += 1;
            } else if inside(q, nested, 1.0) && outside(q, root, 1.0) {
                assert_eq!(got, BG, "z {z}: not cut at ({x}, {y})");
                cut_seen += 1;
            } else if inside(q, root, 1.0) && outside(q, nested, 1.0) {
                assert_eq!(got, green, "z {z} ({x}, {y})");
            }
        }
        assert!(red_seen > 500 && cut_seen > 500, "{red_seen} {cut_seen}");
    }
    Ok(())
}

/// T9: the clip chain, including a mode-0 child window drawn into its parent's canvas.
#[test]
fn t9_clip_chain() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let p_tex = solid(&mut r, 160, 160, [60, 60, 160, 255])?;
    let c_tex = solid(&mut r, 100, 100, [60, 160, 60, 255])?;
    let k_tex = solid(&mut r, 90, 30, [220, 200, 40, 255])?;
    let k_col = [220, 200, 40, 255];
    let parent = UiRect::new(40, 40, 160, 160);
    let pc = [120.0f32, 120.0];
    let none = UiRotation::default();
    struct Case {
        name: &'static str,
        child: UiRect,
        child_surface: bool,
        child_z: f32,
        parent_z: f32,
    }
    let control = |child: UiRect| UiRect::new(child.x + 40, child.y + 30, 90, 30);
    let cases = [
        // (a) child window with its own surface: the control is cut at the child's edge.
        Case {
            name: "a",
            child: UiRect::new(60, 60, 100, 100),
            child_surface: true,
            child_z: 0.0,
            parent_z: 0.0,
        },
        // (b) the child reaches past its parent: both cut at the parent's edge.
        Case {
            name: "b",
            child: UiRect::new(120, 60, 100, 100),
            child_surface: true,
            child_z: 0.0,
            parent_z: 0.0,
        },
        // (c) the child window turned by z = 30: cut along its rotated edge.
        Case {
            name: "c",
            child: UiRect::new(60, 60, 100, 100),
            child_surface: true,
            child_z: 30.0,
            parent_z: 0.0,
        },
        // (d) mode-0 child window (no surface), unrotated, parent rotated, and past the parent.
        Case {
            name: "d0",
            child: UiRect::new(60, 60, 100, 100),
            child_surface: false,
            child_z: 0.0,
            parent_z: 0.0,
        },
        Case {
            name: "d30",
            child: UiRect::new(60, 60, 100, 100),
            child_surface: false,
            child_z: 0.0,
            parent_z: 30.0,
        },
        Case {
            name: "dpast",
            child: UiRect::new(120, 60, 100, 100),
            child_surface: false,
            child_z: 0.0,
            parent_z: 0.0,
        },
    ];
    for c in &cases {
        let k_box = control(c.child);
        let cc = [
            c.child.x as f32 + c.child.w as f32 / 2.0,
            c.child.y as f32 + c.child.h as f32 / 2.0,
        ];
        let p_rot = UiRotation::z(pc, c.parent_z);
        let c_rot = UiRotation::z(cc, c.child_z);
        // Rotations nearest first: the control's own surface, the child's surface (when it has
        // one), the parent's. Clip spaces count the rotations before the drawing surface's.
        let (k_rots, k_clips, c_rots, c_clips): (Vec<_>, Vec<_>, Vec<_>, Vec<_>) =
            if c.child_surface {
                (
                    vec![none, c_rot, p_rot],
                    vec![UiClip::new(c.child, 1), UiClip::new(parent, 2)],
                    vec![c_rot, p_rot],
                    vec![UiClip::new(parent, 1)],
                )
            } else {
                // The child's box is tested in the parent's surface space, like the parent's.
                (
                    vec![none, p_rot],
                    vec![UiClip::new(c.child, 1), UiClip::new(parent, 1)],
                    Vec::new(),
                    Vec::new(),
                )
            };
        let (img, stats) = render_ui(&mut r, SIZE, SRGB, |pass| {
            pass.entry(&UiEntry::new(p_tex, [160, 160], parent).with_rotations(&[p_rot]));
            if c.child_surface {
                pass.entry(
                    &UiEntry::new(c_tex, [100, 100], c.child)
                        .with_rotations(&c_rots)
                        .with_clips(&c_clips),
                );
            }
            pass.entry(
                &UiEntry::new(k_tex, [90, 30], k_box)
                    .with_rotations(&k_rots)
                    .with_clips(&k_clips),
            );
        })?;
        assert_eq!(stats.ui.entries_refused, 0, "{}", c.name);
        let (mut drawn, mut cut) = (0, 0);
        for (x, y, p) in centres(SIZE.0, SIZE.1) {
            // Screen → the parent's unrotated space → the child's unrotated space.
            let in_parent_space = rot_z(
                p,
                [f64::from(pc[0]), f64::from(pc[1])],
                -f64::from(c.parent_z),
            );
            let q = rot_z(
                in_parent_space,
                [f64::from(cc[0]), f64::from(cc[1])],
                -f64::from(c.child_z),
            );
            if !inside(q, k_box, 1.0) {
                continue;
            }
            let got = px(&img, x, y);
            let in_child = inside(q, c.child, 1.0);
            let in_parent = inside(in_parent_space, parent, 1.0);
            let out_child = outside(q, c.child, 1.0);
            let out_parent = outside(in_parent_space, parent, 1.0);
            if in_child && in_parent {
                assert_eq!(got, k_col, "{}: ({x}, {y}) not drawn", c.name);
                drawn += 1;
            } else if out_child || out_parent {
                assert_ne!(got, k_col, "{}: ({x}, {y}) not cut", c.name);
                cut += 1;
            }
        }
        assert!(
            drawn > 300 && cut > 100,
            "{}: drawn {drawn}, cut {cut}",
            c.name
        );
    }
    Ok(())
}

/// T10: external content: premultiplied `Rgba8Unorm` target with a half-covered edge texel,
/// no dark fringe when scaled, stored bytes unconverted at 1 : 1, sRGB targets refused.
#[test]
fn t10_external_content() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let portrait = r.create_offscreen_target(16, 16, UNORM)?;
    let mut frame = r.begin_offscreen_frame(&portrait)?;
    frame
        .pass("portrait", PassSpace::Screen)
        .premultiplied_alpha()
        .quad(Quad::screen_rect(0.0, 0.0, 8.0, 16.0).with_color([1.0; 4]))
        // The half-covered edge column.
        .quad(Quad::screen_rect(8.0, 0.0, 1.0, 16.0).with_color([1.0, 1.0, 1.0, 0.5]));
    r.end_frame(frame)?;
    let stored = r.read_pixels(&portrait)?;
    assert_eq!(stored.pixel(3, 3), Some([255; 4]));
    let edge = stored.pixel(8, 3).expect("in bounds");
    assert!(
        edge[3].abs_diff(128) <= 1 && edge[0] == edge[3],
        "premultiplied: {edge:?}"
    );
    assert_eq!(stored.pixel(12, 3), Some([0; 4]));
    let tex = r.register_target_texture(&portrait, Filter::Nearest, Wrap::Clamp)?;
    let external = |dest| UiEntry::new(tex, [16, 16], dest).with_source(UiSource::Premultiplied);
    // Scaled x4 over white: no fringe.
    let white = solid(&mut r, 1, 1, [255; 4])?;
    let (img, _) = render_ui(&mut r, SIZE, SRGB, |p| {
        p.entry(&UiEntry::new(white, [1, 1], UiRect::new(0, 0, 80, 80)))
            .entry(&external(UiRect::new(8, 8, 64, 64)));
    })?;
    for y in 8..72 {
        for x in 8..72 {
            let p = px(&img, x, y);
            assert!(p.iter().all(|&c| c >= 254), "fringe at ({x}, {y}): {p:?}");
        }
    }
    // 1 : 1 over black: the stored bytes reach the frame unconverted (in the fallback of an
    // sRGB target only the opaque and empty ones; render.md §6.4).
    let black = solid(&mut r, 1, 1, [0, 0, 0, 255])?;
    let fallback = r.ui_view_mode() == UiViewMode::LinearFallback;
    for format in [SRGB, UNORM] {
        let (img, _) = render_ui(&mut r, SIZE, format, |p| {
            p.entry(&UiEntry::new(black, [1, 1], UiRect::new(0, 0, 40, 40)))
                .entry(&external(UiRect::new(4, 4, 16, 16)));
        })?;
        for y in 0..16 {
            for x in 0..16 {
                let s = stored.pixel(x, y).expect("in bounds");
                if fallback && format == SRGB && !matches!(s[3], 0 | 255) {
                    continue;
                }
                assert_eq!(
                    px(&img, 4 + x, 4 + y),
                    [s[0], s[1], s[2], 255],
                    "{format:?} ({x}, {y})"
                );
            }
        }
    }
    // An sRGB target is refused, and so is the frame's own target.
    let srgb = r.create_offscreen_target(16, 16, SRGB)?;
    let srgb_tex = r.register_target_texture(&srgb, Filter::Nearest, Wrap::Clamp)?;
    let (img, stats) = render_ui(&mut r, SIZE, SRGB, |p| {
        p.entry(&external(UiRect::new(0, 0, 16, 16)).with_source(UiSource::Premultiplied))
            .entry(&UiEntry::new(
                srgb_tex,
                [16, 16],
                UiRect::new(100, 100, 16, 16),
            ));
    })?;
    assert_eq!(stats.ui.entries_refused, 1);
    assert_eq!(px(&img, 105, 105), BG);
    let mut frame = r.begin_offscreen_frame(&portrait)?;
    frame.ui_pass().entry(&external(UiRect::new(0, 0, 16, 16)));
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.ui.entries_refused, 1, "feedback loop refused");
    Ok(())
}

/// T11 and P1 check 1: the round minimap.
#[test]
fn t11_round_minimap() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    // The radar's canvas: the disc written with Replace, then map pixels with the disc's alpha.
    let radar = ui_texture(&mut r, 170, 170, |x, y| {
        [(x + 40) as u8, (y + 60) as u8, 90, disc_alpha(x, y)]
    })?;
    let (img, _) = render_ui(&mut r, SIZE, SRGB, |p| {
        p.entry(&UiEntry::new(
            radar,
            [170, 170],
            UiRect::new(40, 30, 170, 170),
        ));
    })?;
    for (x, y) in [(40, 30), (209, 30), (40, 199), (209, 199), (45, 35)] {
        assert_eq!(px(&img, x, y), BG, "corner ({x}, {y})");
    }
    assert_eq!(px(&img, 125, 115), [125, 145, 90, 255], "centre");
    Ok(())
}

/// T12: the non-sRGB view leaves stored bytes unconverted; in the fallback, opaque pixels
/// round-trip all 256 values exactly.
#[test]
fn t12_stored_values_and_fallback() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let ramp = ui_texture(&mut r, 256, 3, |x, y| {
        let v = x as u8;
        match y {
            0 => [v, 0, 255 - v, 255],
            1 => [0, v, 0, 255],
            _ => [v, v, v, 255],
        }
    })?;
    for force in [false, true] {
        r.set_ui_linear_fallback(force);
        if force {
            assert_eq!(r.ui_view_mode(), UiViewMode::LinearFallback);
        }
        let (img, _) = render_ui(&mut r, (256, 8), SRGB, |p| {
            p.entry(&UiEntry::new(ramp, [256, 3], UiRect::new(0, 0, 256, 3)));
        })?;
        for x in 0..256u32 {
            let v = x as u8;
            assert_eq!(px(&img, x, 0), [v, 0, 255 - v, 255], "fallback {force}");
            assert_eq!(px(&img, x, 1), [0, v, 0, 255], "fallback {force}");
            assert_eq!(px(&img, x, 2), [v, v, v, 255], "fallback {force}");
        }
    }
    r.set_ui_linear_fallback(false);
    Ok(())
}

/// §6.4 fallback: "alpha 0 gives 0". Premultiplied external content whose stored alpha is 0
/// but whose colour is not (a target cleared to (1, 0, 0, 0)) leaves the frame unchanged in
/// the linear-light fallback; opaque external texels still round-trip.
#[test]
fn fallback_alpha_zero_gives_zero() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let portrait = r.create_offscreen_target(16, 16, UNORM)?;
    let mut frame = r.begin_offscreen_frame(&portrait)?;
    frame
        .pass("portrait", PassSpace::Screen)
        .clear([1.0, 0.0, 0.0, 0.0])
        .quad(Quad::screen_rect(0.0, 0.0, 8.0, 16.0).with_color([0.0, 1.0, 0.0, 1.0]));
    r.end_frame(frame)?;
    let stored = r.read_pixels(&portrait)?;
    assert_eq!(
        stored.pixel(12, 3),
        Some([255, 0, 0, 0]),
        "colour with alpha 0"
    );
    assert_eq!(stored.pixel(3, 3), Some([0, 255, 0, 255]));
    let tex = r.register_target_texture(&portrait, Filter::Nearest, Wrap::Clamp)?;
    r.set_ui_linear_fallback(true);
    let result = render_ui(&mut r, SIZE, SRGB, |p| {
        p.entry(
            &UiEntry::new(tex, [16, 16], UiRect::new(4, 4, 16, 16))
                .with_source(UiSource::Premultiplied),
        );
    });
    r.set_ui_linear_fallback(false);
    let (img, stats) = result?;
    assert_eq!(stats.ui.quads, 2);
    for y in 0..16 {
        for x in 0..16 {
            let want = if x < 8 { [0, 255, 0, 255] } else { BG };
            assert_eq!(px(&img, 4 + x, 4 + y), want, "({x}, {y})");
        }
    }
    Ok(())
}

/// T13: walker overlay < UI pass < developer overlay < software cursor, even with the cursor
/// set before any pass is recorded.
#[test]
fn t13_screen_pass_order() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let green = solid(&mut r, 40, 40, [0, 255, 0, 255])?;
    let cursor = solid(&mut r, 32, 32, [255, 255, 255, 255])?;
    let target = r.create_offscreen_target(128, 128, SRGB)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame.software_cursor(cursor, [30, 30]);
    frame
        .pass("world", PassSpace::Screen)
        .clear([0.0, 0.0, 0.0, 1.0]);
    frame
        .pass("walker overlay", PassSpace::Screen)
        .quad(Quad::screen_rect(0.0, 0.0, 40.0, 40.0).with_color([1.0, 0.0, 0.0, 1.0]));
    frame
        .ui_pass()
        .entry(&UiEntry::new(green, [40, 40], UiRect::new(10, 10, 40, 40)));
    frame
        .pass("developer overlay", PassSpace::Screen)
        .quad(Quad::screen_rect(20.0, 20.0, 40.0, 40.0).with_color([0.0, 0.0, 1.0, 1.0]));
    let stats = r.end_frame(frame)?;
    assert!(stats.ui.cursor_drawn);
    assert_eq!(stats.ui.quads, 1, "the cursor is not an entry");
    let img = r.read_pixels(&target)?;
    assert_eq!(px(&img, 5, 5), [255, 0, 0, 255], "walker overlay");
    assert_eq!(
        px(&img, 15, 15),
        [0, 255, 0, 255],
        "UI over the walker overlay"
    );
    assert_eq!(
        px(&img, 25, 25),
        [0, 0, 255, 255],
        "developer overlay over the UI"
    );
    assert_eq!(px(&img, 35, 35), [255; 4], "cursor last");
    assert_eq!(px(&img, 70, 70), [0, 0, 0, 255]);
    Ok(())
}

/// T14: z = 90 turns a right-pointing arrow up; an x tilt of 60 halves the drawn height; a
/// tilt of 90 draws nothing.
#[test]
fn t14_rotation_convention() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let red = [230, 30, 30, 255];
    // A bar from the centre to the right edge.
    let arrow = ui_texture(&mut r, 32, 32, |x, y| {
        if x >= 16 && (14..18).contains(&y) {
            red
        } else {
            [0; 4]
        }
    })?;
    let dest = UiRect::new(100, 100, 32, 32);
    let rot = [UiRotation::z([116.0, 116.0], 90.0)];
    let (img, _) = render_ui(&mut r, SIZE, SRGB, |p| {
        p.entry(&UiEntry::new(arrow, [32, 32], dest).with_rotations(&rot));
    })?;
    assert_eq!(px(&img, 116, 105), red, "points up");
    assert_eq!(px(&img, 126, 116), BG, "no longer points right");
    assert_eq!(px(&img, 116, 126), BG, "not down");

    let block = solid(&mut r, 64, 64, [250, 250, 20, 255])?;
    let count_rows = |img: &tessera_render::RgbaImage| {
        (0..SIZE.1)
            .filter(|&y| px(img, 100, y) == [250, 250, 20, 255])
            .count()
    };
    for (angle, rows) in [(60.0, 32), (90.0, 0)] {
        let rot = [UiRotation {
            centre: [112.0, 112.0],
            angles: [angle, 0.0, 0.0],
        }];
        let (img, _) = render_ui(&mut r, SIZE, SRGB, |p| {
            p.entry(
                &UiEntry::new(block, [64, 64], UiRect::new(80, 80, 64, 64)).with_rotations(&rot),
            );
        })?;
        assert_eq!(count_rows(&img), rows, "x tilt {angle}");
    }
    Ok(())
}

/// T15 and invariant 6: hostile entries are refused or sanitised, never panic, and are
/// counted.
#[test]
fn t15_hostile_entries() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let tex = solid(&mut r, 4, 4, [9, 9, 9, 255])?;
    // 8,193 entries: one refused (the background entry of `render_ui` counts too).
    let (_, stats) = render_ui(&mut r, (64, 64), SRGB, |p| {
        for i in 0..8192 {
            p.entry(&UiEntry::new(tex, [4, 4], UiRect::new(i % 60, 0, 4, 4)));
        }
    })?;
    assert_eq!(stats.ui.entries_refused, 1);
    assert_eq!(stats.ui.quads, 8192);

    let stale = solid(&mut r, 4, 4, [1, 2, 3, 255])?;
    r.destroy_texture(stale);
    let rgba = r.upload_texture(&TextureUpload::rgba8(4, 4, &[255; 64]))?;
    let foreign = {
        let other = gpu::headless_renderer([0.0; 4]);
        match other {
            Some(mut o) => Some(o.create_ui_texture(4, 4, None)?),
            None => None,
        }
    };
    let ok = UiEntry::new(tex, [4, 4], UiRect::new(10, 10, 4, 4));
    let clips = [UiClip::default(); 9];
    let rots = [UiRotation::default(); 10];
    let nan_rot = [UiRotation {
        centre: [12.0, 12.0],
        angles: [f32::NAN, f32::INFINITY, f32::NAN],
    }];
    let (img, stats) = render_ui(&mut r, (64, 64), SRGB, |p| {
        p.entry(&ok.with_clips(&clips))
            .entry(&ok.with_rotations(&rots))
            .entry(&UiEntry {
                dest: UiRect::new(0, 0, 0, 4),
                ..ok
            })
            .entry(&UiEntry {
                texture: stale,
                ..ok
            })
            .entry(&UiEntry {
                texture: rgba,
                ..ok
            })
            .entry(&UiEntry {
                canvas: [0, 4],
                ..ok
            })
            .entry(&ok.with_addressing(UiAddressing::Tile {
                node_scale: [f32::NAN, 1.0],
            }));
        if let Some(f) = foreign {
            p.entry(&UiEntry { texture: f, ..ok });
        }
        // Sanitised: NaN angles draw unrotated, NaN opacity draws nothing.
        p.entry(&UiEntry::new(tex, [4, 4], UiRect::new(30, 30, 4, 4)).with_rotations(&nan_rot))
            .entry(
                &UiEntry::new(tex, [4, 4], UiRect::new(40, 40, 4, 4))
                    .with_opacity(f32::NAN)
                    .with_alpha_threshold(f32::NAN),
            );
    })?;
    let foreign_refusal = u32::from(foreign.is_some());
    assert_eq!(stats.ui.entries_refused, 7 + foreign_refusal);
    assert_eq!(stats.ui.entries_sanitised, 2);
    assert_eq!(px(&img, 31, 31), [9, 9, 9, 255], "NaN angles: unrotated");
    assert_eq!(px(&img, 41, 41), BG, "NaN opacity: nothing drawn");
    assert_eq!(px(&img, 11, 11), BG, "refused entries draw nothing");
    Ok(())
}

/// Renderer side of T16/T18 (generation hold, back-texture swap): while a back texture is
/// filled band by band over several frames, every frame shows the unchanged front texture;
/// at the swap the new content and the new box appear together.
#[test]
fn generation_hold_and_back_texture_swap() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let old = [50, 90, 220, 255];
    let new = [240, 150, 30, 255];
    let front = solid(&mut r, 64, 64, old)?;
    let old_box = UiRect::new(10, 10, 64, 64);
    let new_box = UiRect::new(30, 12, 64, 64);
    let (before, _) = render_ui(&mut r, SIZE, SRGB, |p| {
        p.entry(&UiEntry::new(front, [64, 64], old_box));
    })?;
    let back = r.create_ui_texture(64, 64, Some("back"))?;
    r.copy_texture_rect(front, 0, 0, back, 0, 0, 64, 64)?;
    let band = ui_common::canvas(64, 16, |_, _| new);
    for i in 0..4 {
        r.update_texture_rect(back, 0, 16 * i, 64, 16, &band, 256)?;
        let (img, stats) = render_ui(&mut r, SIZE, SRGB, |p| {
            p.entry(&UiEntry::new(front, [64, 64], old_box));
        })?;
        assert!(
            img == before,
            "band {i}: a partly uploaded surface was shown"
        );
        // The band, plus the 1 x 1 background `render_ui` uploads.
        assert_eq!(stats.ui.upload_bytes, 64 * 16 * 4 + 4);
    }
    // The swap frame.
    let (img, _) = render_ui(&mut r, SIZE, SRGB, |p| {
        p.entry(&UiEntry::new(back, [64, 64], new_box));
    })?;
    for (x, y, p) in centres(SIZE.0, SIZE.1) {
        let want = if inside(p, new_box, 0.0) { new } else { BG };
        assert_eq!(px(&img, x, y), want, "({x}, {y})");
    }
    Ok(())
}

/// Renderer side of T17 (group transition): during a root's resize generation the nested entry
/// and the canvas above keep their old places relative to the old root box; all three change
/// in the frame of the swap.
#[test]
fn group_transition() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let root_col = [70, 70, 90, 255];
    let nested_col = [200, 60, 60, 255];
    let above_col = [250, 250, 250, 255];
    let root_old = solid(&mut r, 100, 80, root_col)?;
    let nested = solid(&mut r, 20, 20, nested_col)?;
    // The canvas above: transparent except a 6 x 6 mark at (40, 30) of the root canvas.
    let above = ui_texture(&mut r, 100, 80, |x, y| {
        if (40..46).contains(&x) && (30..36).contains(&y) {
            above_col
        } else {
            [0; 4]
        }
    })?;
    let draw = |r: &mut tessera_render::Renderer<'static>,
                root: tessera_render::TextureId,
                root_box: UiRect,
                above_tex: tessera_render::TextureId| {
        let nested_box = UiRect::new(root_box.x + 30, root_box.y + 20, 20, 20);
        let clip = [UiClip::new(root_box, 0)];
        let root_canvas = [root_box.w, root_box.h];
        render_ui(r, SIZE, SRGB, |p| {
            p.entry(&UiEntry::new(root, root_canvas, root_box))
                .entry(&UiEntry::new(nested, [20, 20], nested_box).with_clips(&clip))
                .entry(&UiEntry::new(above_tex, root_canvas, root_box).with_clips(&clip));
        })
    };
    let old_box = UiRect::new(20, 20, 100, 80);
    let (before, _) = draw(&mut r, root_old, old_box, above)?;
    assert_eq!(px(&before, 20 + 35, 20 + 25), nested_col);
    assert_eq!(px(&before, 20 + 42, 20 + 32), above_col);
    // The generation runs over three frames; the root's new canvas (moved to x = 60 and
    // widened) is filled meanwhile, and every frame keeps the old geometry.
    let root_new = r.create_ui_texture(140, 80, Some("root-new"))?;
    let above_new = r.create_ui_texture(140, 80, Some("above-new"))?;
    for i in 0..3u32 {
        let rows = canvas_rows(140, 80, i, root_col);
        r.update_texture_rect(root_new, 0, i * 27, 140, rows.1, &rows.0, 140 * 4)?;
        let (img, _) = draw(&mut r, root_old, old_box, above)?;
        assert!(
            img == before,
            "frame {i} of the generation changed the picture"
        );
    }
    let mark = ui_common::canvas(6, 6, |_, _| above_col);
    r.update_texture_rect(above_new, 80, 30, 6, 6, &mark, 24)?;
    let new_box = UiRect::new(60, 20, 140, 80);
    let (img, _) = draw(&mut r, root_new, new_box, above_new)?;
    assert_eq!(
        px(&img, 60 + 35, 20 + 25),
        nested_col,
        "nested entry moved with the root"
    );
    assert_eq!(
        px(&img, 60 + 82, 20 + 32),
        above_col,
        "canvas above swapped"
    );
    assert_eq!(px(&img, 20 + 35, 20 + 25), BG, "old nested place empty");
    assert_eq!(px(&img, 150, 50), root_col, "root at its new box");
    Ok(())
}

/// Rows of a root canvas band (`i`-th of three) for [`group_transition`].
fn canvas_rows(w: u32, h: u32, i: u32, rgba: [u8; 4]) -> (Vec<u8>, u32) {
    let rows = (h - i * 27).min(27);
    (ui_common::canvas(w, rows, |_, _| rgba), rows)
}

/// The renderer side of the P1 composition checks (render.md §6.14): the rim in its own
/// canvas shows outside the viewport box and is cut only by the root (7 columns left, 6
/// right, no row); the marker points up at heading 0 and right at 90; turning the marker
/// uploads nothing.
#[test]
fn p1_composition_checks() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let scene = P1::new(&mut r)?;
    let (img, stats) = render_ui(&mut r, SIZE_P1, SRGB, |p| scene.entries(p, 0.0))?;
    assert_eq!(stats.ui.entries_refused, 0);
    let root = scene.root;
    let viewport = UiRect::new(root.x + 27, root.y + 33, 170, 170);
    let mut rim_outside_viewport = 0;
    for (x, y, p) in centres(SIZE_P1.0, SIZE_P1.1) {
        let is_rim = px(&img, x, y) == RIM;
        if is_rim && outside(p, viewport, 0.0) {
            rim_outside_viewport += 1;
        }
        if outside(p, root, 0.0) {
            assert!(!is_rim, "rim pixel outside the root box at ({x}, {y})");
        }
    }
    assert!(rim_outside_viewport > 1000, "{rim_outside_viewport}");
    // The rim's ring reaches the root's left and right edges, so the root cut it there.
    let ring_row = (scene.rim.y + 118) as u32;
    assert_eq!(
        px(&img, root.x as u32, ring_row),
        RIM,
        "cut exactly at the left edge"
    );
    assert_eq!(px(&img, (root.x - 1) as u32, ring_row), BG);
    assert_eq!(px(&img, (root.x + 223) as u32, ring_row), RIM);
    assert_eq!(px(&img, (root.x + 224) as u32, ring_row), BG);
    // The minimap is round: near the viewport's corner, outside the disc and inside the
    // ring, the transparent root canvas shows.
    assert_eq!(px(&img, viewport.x as u32 + 20, viewport.y as u32 + 20), BG);

    // The marker: up at heading 0, right at 90.
    let c = scene.marker_centre();
    for (h, probe, away) in [
        (0.0f32, (c.0, c.1 - 9), (c.0 + 9, c.1)),
        (90.0, (c.0 + 9, c.1), (c.0, c.1 - 9)),
    ] {
        let (img, _) = render_ui(&mut r, SIZE_P1, SRGB, |p| scene.entries(p, h))?;
        assert_eq!(px(&img, probe.0, probe.1), MARKER, "heading {h}: arrow tip");
        assert_ne!(px(&img, away.0, away.1), MARKER, "heading {h}");
    }
    // Turning through 360 degrees in 1-degree steps changes only the entry.
    let target = r.create_offscreen_target(SIZE_P1.0, SIZE_P1.1, SRGB)?;
    for h in 0..360 {
        let mut frame = r.begin_offscreen_frame(&target)?;
        scene.entries(frame.ui_pass(), h as f32);
        let stats = r.end_frame(frame)?;
        assert_eq!(
            (stats.ui.upload_rects, stats.ui.gpu_copies, stats.ui.clears),
            (0, 0, 0),
            "heading {h}"
        );
    }
    Ok(())
}

/// A canvas texel that names its position: R, G = the low and high byte of `i`, B = the low
/// byte of `j` (opaque).
fn index_texel(i: u32, j: u32) -> [u8; 4] {
    [(i & 255) as u8, (i >> 8) as u8, (j & 255) as u8, 255]
}

/// T5 (Nearest where `c` lands exactly on an integer): render.md §6.7 step 3 reads texel ⌊c⌋.
/// A 2 : 1 reduction puts `c` = 2(X − x) + 1 on an integer at every pixel, UI scale 1.5 and
/// 0.75 at every third; Tile node scales 0.5, 1.5 and 0.75 likewise. The interpolated
/// position used before could fall just below the integer and read the previous texel.
#[test]
fn t5_nearest_reads_floor_c_on_integers() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let (x0, y0) = (3, 5);
    // Fit: (canvas side, box side); texel floor((2k + 1) cw / 2w).
    for (cw, w) in [(400u32, 200u32), (160, 240), (240, 180), (7, 3)] {
        let tex = ui_texture(&mut r, cw, cw, index_texel)?;
        let (img, _) = render_ui(&mut r, SIZE, UNORM, |p| {
            p.entry(
                &UiEntry::new(tex, [cw, cw], UiRect::new(x0, y0, w, w))
                    .with_filter(UiFilter::Nearest),
            );
        })?;
        let mut wrong = 0;
        for y in 0..w {
            for x in 0..w {
                let i = (2 * x + 1) * cw / (2 * w);
                let j = (2 * y + 1) * cw / (2 * w);
                if px(&img, x0 as u32 + x, y0 as u32 + y) != index_texel(i, j) {
                    wrong += 1;
                }
            }
        }
        assert_eq!(wrong, 0, "Fit {cw} on {w}: pixels off floor(c)");
        r.destroy_texture(tex);
    }
    // Tile: node scale num/den; texel floor((k + 1/2) den / num) mod 32.
    let tex = ui_texture(&mut r, 32, 32, index_texel)?;
    for (num, den) in [(1u32, 2u32), (3, 2), (3, 4)] {
        let s = num as f32 / den as f32;
        let (img, _) = render_ui(&mut r, SIZE, UNORM, |p| {
            p.entry(
                &UiEntry::new(tex, [32, 32], UiRect::new(x0, y0, 250, 240))
                    .with_addressing(UiAddressing::Tile { node_scale: [s, s] })
                    .with_filter(UiFilter::Nearest),
            );
        })?;
        let mut wrong = 0;
        for y in 0..240 {
            for x in 0..250 {
                let i = ((2 * x + 1) * den / (2 * num)) % 32;
                let j = ((2 * y + 1) * den / (2 * num)) % 32;
                if px(&img, x0 as u32 + x, y0 as u32 + y) != index_texel(i, j) {
                    wrong += 1;
                }
            }
        }
        assert_eq!(wrong, 0, "Tile at node scale {s}: pixels off floor(c)");
    }
    Ok(())
}

/// T2 / invariant 5 through the older whole-texture call: `update_texture` refuses UI textures
/// (they take B, G, R, A rectangles in call order), and on an RGBA8 texture a later
/// `update_texture` wins over an earlier, still pending `update_texture_rect`.
#[test]
fn t2_whole_texture_update_keeps_call_order() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let ui = solid(&mut r, 4, 4, [255, 0, 0, 255])?;
    assert!(matches!(
        r.update_texture(ui, &[0, 255, 0, 255].repeat(16)),
        Err(RenderError::InvalidTexture(_))
    ));
    let (img, _) = render_ui(&mut r, SIZE, UNORM, |p| {
        p.entry(&UiEntry::new(ui, [4, 4], UiRect::new(0, 0, 4, 4)));
    })?;
    assert_eq!(
        px(&img, 1, 1),
        [255, 0, 0, 255],
        "the refused call wrote nothing"
    );

    let rgba8 = r.upload_texture(&TextureUpload::rgba8(4, 4, &[0; 64]))?;
    r.update_texture_rect(rgba8, 0, 0, 4, 4, &[255; 64], 16)?;
    assert!(r.ui_pending_ops() > 0);
    r.update_texture(rgba8, &[0, 0, 255, 255].repeat(16))?;
    let target = r.create_offscreen_target(16, 16, UNORM)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .pass("quad", PassSpace::Screen)
        .clear([0.0, 0.0, 0.0, 1.0])
        .quad(Quad::screen_rect(0.0, 0.0, 16.0, 16.0).with_texture(rgba8));
    r.end_frame(frame)?;
    let img = r.read_pixels(&target)?;
    assert_eq!(px(&img, 8, 8), [0, 0, 255, 255], "the later update wins");
    Ok(())
}

/// Hostile input to the UI texture operations: a caller that keeps clearing while it ends no
/// frame never has more than 4,096 operations pending, and consecutive clears of a texture
/// still take effect exactly (they share one render pass per flush).
#[test]
fn ui_op_queue_is_bounded() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let tex = solid(&mut r, 128, 128, [200, 100, 50, 255])?;
    let start = std::time::Instant::now();
    let mut most = 0;
    // Clear every other texel of the left 100 columns: 6,400 one-texel clears.
    for y in 0..128 {
        for x in (0..100).step_by(2) {
            r.clear_texture_rect(tex, x + y % 2, y, 1, 1)?;
            most = most.max(r.ui_pending_ops());
        }
    }
    assert!(most <= 4096, "{most} operations pending");
    r.flush_ui_ops()?;
    assert_eq!(r.ui_pending_ops(), 0);
    eprintln!(
        "6,400 one-texel clears recorded and submitted in {:?} (indicative)",
        start.elapsed()
    );
    let (img, stats) = render_ui(&mut r, SIZE, UNORM, |p| {
        p.entry(&UiEntry::new(tex, [128, 128], UiRect::new(0, 0, 128, 128)));
    })?;
    assert_eq!(stats.ui.clears, 6400, "every call is counted");
    for y in 0..128 {
        for x in 0..128 {
            let cleared = x < 100 && x % 2 == y % 2;
            let want = if cleared { BG } else { [200, 100, 50, 255] };
            assert_eq!(px(&img, x, y), want, "({x}, {y})");
        }
    }
    Ok(())
}

/// The software cursor takes UI textures only (render.md §6.11): a registered (premultiplied)
/// target, a destroyed texture and a cleared cursor draw nothing, are not counted as refused
/// entries, and never panic.
#[test]
fn software_cursor_refusals() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let portrait = r.create_offscreen_target(8, 8, UNORM)?;
    let mut frame = r.begin_offscreen_frame(&portrait)?;
    frame
        .pass("portrait", PassSpace::Screen)
        .premultiplied_alpha()
        .quad(Quad::screen_rect(0.0, 0.0, 8.0, 8.0).with_color([1.0, 1.0, 1.0, 0.5]));
    r.end_frame(frame)?;
    let target_tex = r.register_target_texture(&portrait, Filter::Nearest, Wrap::Clamp)?;
    let stale = solid(&mut r, 8, 8, [255; 4])?;
    r.destroy_texture(stale);
    let good = solid(&mut r, 8, 8, [255; 4])?;
    let target = r.create_offscreen_target(32, 32, UNORM)?;
    for (case, cursor, clear) in [
        ("registered target", target_tex, false),
        ("destroyed texture", stale, false),
        ("cleared cursor", good, true),
        ("UI texture", good, false),
    ] {
        let mut frame = r.begin_offscreen_frame(&target)?;
        frame.software_cursor(cursor, [4, 4]);
        if clear {
            frame.clear_software_cursor();
        }
        frame
            .pass("world", PassSpace::Screen)
            .clear([0.0, 0.0, 0.0, 1.0]);
        let stats = r.end_frame(frame)?;
        let img = r.read_pixels(&target)?;
        let drawn = case == "UI texture";
        assert_eq!(stats.ui.cursor_drawn, drawn, "{case}");
        assert_eq!(
            stats.ui.entries_refused, 0,
            "{case}: the cursor is not an entry"
        );
        let want = if drawn { [255; 4] } else { [0, 0, 0, 255] };
        assert_eq!(px(&img, 6, 6), want, "{case}");
    }
    Ok(())
}

/// The frame-wide entry cap across several UI passes (decision 0015 §7): entries past it are
/// refused when recorded, never stored; the cap counters are copied unchanged, the last call
/// of the frame winning (render.md §6.5).
#[test]
fn entry_cap_spans_ui_passes_and_caps_last_call_wins() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let tex = solid(&mut r, 4, 4, [9, 9, 9, 255])?;
    let target = r.create_offscreen_target(64, 64, SRGB)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    let entry = UiEntry::new(tex, [4, 4], UiRect::new(0, 0, 4, 4));
    let first = UiCapCounters {
        total_cap: 3,
        ..UiCapCounters::default()
    };
    let last = UiCapCounters {
        apartment_cap: 1,
        memory_cap: 2,
        entry_cap: 7,
        ..UiCapCounters::default()
    };
    let a = frame.ui_pass();
    a.cap_counters(first);
    for _ in 0..4097 {
        a.entry(&entry);
    }
    let b = frame.ui_pass();
    for _ in 0..4096 {
        b.entry(&entry);
    }
    b.cap_counters(last);
    assert_eq!((b.len(), b.refused()), (4095, 1), "refused while recording");
    // A third pass has no room left at all.
    let c = frame.ui_pass();
    c.entry(&entry);
    assert_eq!((c.len(), c.refused()), (0, 1));
    let stored: usize = frame.ui_passes().map(|p| p.len()).sum();
    assert_eq!(stored, MAX_UI_ENTRIES);
    let stats = r.end_frame(frame)?;
    assert_eq!(stats.ui.quads, 8192);
    assert_eq!(stats.ui.entries_refused, 2);
    assert_eq!(stats.ui.caps, last);
    Ok(())
}

/// A rectangle larger than one 32 MiB staging band (4096 × 2100 with a padded stride: two
/// bands) arrives texel-exact, reaching the frame through a `tex_offset` window across the
/// band boundary; `flush_ui_ops` submits it, and the staging memory kept afterwards is
/// bounded.
#[test]
fn banded_upload_and_flush() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let (w, h) = (4096u32, 2100u32);
    let max = r.context().device.limits().max_texture_dimension_2d;
    if max < w {
        return Ok(());
    }
    let texel = |x: u32, y: u32| [(x * 7 + y) as u8, (y * 3) as u8, (x ^ y) as u8, 255];
    let stride = w as usize * 4 + 36;
    let mut bytes = vec![0xAAu8; stride * h as usize];
    for (y, row) in bytes.chunks_mut(stride).enumerate() {
        for (x, p) in row[..w as usize * 4].chunks_mut(4).enumerate() {
            p.copy_from_slice(&ui_common::bgra(texel(x as u32, y as u32)));
        }
    }
    let tex = r.create_ui_texture(w, h, Some("banded"))?;
    r.update_texture_rect(tex, 0, 0, w, h, &bytes, stride)?;
    drop(bytes);
    assert!(r.ui_pending_ops() > 0);
    r.flush_ui_ops()?;
    assert_eq!(r.ui_pending_ops(), 0);
    assert!(
        r.ui_staging_bytes() <= 16 << 20,
        "{} staging bytes kept",
        r.ui_staging_bytes()
    );
    // Band boundary at row 2048: show rows 1950..2100 of columns 3900..4096.
    let (ox, oy) = (3900u32, 1950u32);
    let (img, stats) = render_ui(&mut r, SIZE, UNORM, |p| {
        p.entry(
            &UiEntry::new(tex, [196, 150], UiRect::new(0, 0, 196, 150))
                .with_tex_offset([-(ox as i32), -(oy as i32)]),
        );
    })?;
    assert_eq!(
        stats.ui.upload_rects, 2,
        "the background and the banded rectangle"
    );
    for y in 0..150 {
        for x in 0..196 {
            assert_eq!(px(&img, x, y), texel(ox + x, oy + y), "({x}, {y})");
        }
    }
    Ok(())
}
