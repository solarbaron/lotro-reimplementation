// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-back tests of the UI pass's untextured fill entries ([`UiFill`]), the drawing of the
//! letterbox bars of `ui-scaling.md` §5.4 step 6: a fill draws exactly its rectangle in its
//! place among the entries (below what follows it, over what precedes it), with its colour
//! premultiplied and the clip chain applied; hostile fills are refused and counted, never a
//! panic. Every canvas is synthetic. The tests skip (and pass) where no GPU adapter exists.

#[path = "support/gpu.rs"]
mod gpu;
#[path = "ui_common/mod.rs"]
mod ui_common;

use tessera_render::{
    MAX_UI_CLIPS, OffscreenFormat, RenderError, UiClip, UiEntry, UiFill, UiRect, UiViewMode,
};
use ui_common::{BG, diff, px, render_ui, solid};

const SIZE: (u32, u32) = (128, 96);

/// Opaque black bars on both sides of a picture, drawn below it: the bars cover exactly their
/// rectangles, the picture drawn after them covers its own box, and the background shows
/// elsewhere. Counted as quads but not as surfaces.
#[test]
fn letterbox_fills_draw_exactly_their_rectangles_below_the_picture() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    for format in [OffscreenFormat::Rgba8Srgb, OffscreenFormat::Rgba8Unorm] {
        let green = [30, 200, 60, 255];
        let pic = solid(&mut r, 64, 96, green)?;
        let (img, stats) = render_ui(&mut r, SIZE, format, |p| {
            // Bars at 0..32 and 96..128, the picture at 32..96, and a fill under part of the
            // picture that the picture must hide.
            p.fill(&UiFill::new(UiRect::new(0, 0, 32, 96), UiFill::BLACK));
            p.fill(&UiFill::new(UiRect::new(96, 0, 32, 96), UiFill::BLACK));
            p.fill(&UiFill::new(
                UiRect::new(40, 10, 8, 8),
                [1.0, 0.0, 0.0, 1.0],
            ));
            p.entry(&UiEntry::new(pic, [64, 96], UiRect::new(32, 0, 64, 96)));
        })?;
        assert_eq!(stats.ui.entries_refused, 0, "{format:?}");
        assert_eq!(
            stats.ui.quads, 5,
            "{format:?}: background, three fills, picture"
        );
        assert_eq!(stats.ui.surfaces, 2, "{format:?}: fills read no texture");
        for y in [0, 10, 47, 95] {
            for x in [0, 31, 96, 127] {
                assert_eq!(px(&img, x, y), [0, 0, 0, 255], "{format:?} bar ({x}, {y})");
            }
            for x in [32, 41, 64, 95] {
                assert_eq!(px(&img, x, y), green, "{format:?} picture ({x}, {y})");
            }
        }
        r.destroy_texture(pic);
    }
    Ok(())
}

/// A fill over a picture is drawn over it (entry order), a translucent fill blends
/// premultiplied source-over, and a screen clip cuts the fill.
#[test]
fn fills_keep_entry_order_blend_and_clip() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let exact = r.ui_view_mode() == UiViewMode::Stored;
    let format = OffscreenFormat::Rgba8Unorm;
    let clip = [UiClip::new(UiRect::new(0, 0, 20, 20), 0)];
    let (img, stats) = render_ui(&mut r, SIZE, format, |p| {
        p.fill(&UiFill::new(UiRect::new(10, 10, 40, 40), [1.0, 1.0, 1.0, 1.0]).with_clips(&clip));
        p.fill(&UiFill::new(
            UiRect::new(60, 10, 20, 20),
            [1.0, 0.0, 0.0, 0.5],
        ));
    })?;
    assert_eq!(stats.ui.entries_refused, 0);
    // Clipped to [0, 20) on both axes.
    assert_eq!(px(&img, 10, 10), [255, 255, 255, 255]);
    assert_eq!(px(&img, 19, 19), [255, 255, 255, 255]);
    assert_eq!(px(&img, 20, 19), BG);
    assert_eq!(px(&img, 19, 20), BG);
    if exact {
        // Half red over the background.
        let want = [
            ((255.0 + f64::from(BG[0])) / 2.0).round() as u8,
            (f64::from(BG[1]) / 2.0).round() as u8,
            (f64::from(BG[2]) / 2.0).round() as u8,
            255,
        ];
        assert!(diff(px(&img, 65, 15), want) <= 1, "{:?}", px(&img, 65, 15));
    }
    Ok(())
}

/// Hostile fills: an empty or out-of-range rectangle, too many clips or a clip space other
/// than 0 are refused and counted; NaN colours draw as 0; nothing panics.
#[test]
fn hostile_fills_are_refused_and_counted() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let many = [UiClip::new(UiRect::new(0, 0, 10, 10), 0); MAX_UI_CLIPS + 1];
    let space = [UiClip::new(UiRect::new(0, 0, 10, 10), 1)];
    let (img, stats) = render_ui(&mut r, SIZE, OffscreenFormat::Rgba8Unorm, |p| {
        p.fill(&UiFill::new(UiRect::new(0, 0, 0, 10), UiFill::BLACK));
        p.fill(&UiFill::new(
            UiRect::new(i32::MAX, 0, 10, 10),
            UiFill::BLACK,
        ));
        p.fill(&UiFill::new(UiRect::new(0, 0, 10, 10), UiFill::BLACK).with_clips(&many));
        p.fill(&UiFill::new(UiRect::new(0, 0, 10, 10), UiFill::BLACK).with_clips(&space));
        p.fill(&UiFill::new(
            UiRect::new(30, 30, 10, 10),
            [f32::NAN, f32::INFINITY, -1.0, f32::NAN],
        ));
        p.fill(&UiFill::new(
            UiRect::new(50, 50, 10, 10),
            [f32::NAN, 0.0, 0.0, 1.0],
        ));
    })?;
    assert_eq!(stats.ui.entries_refused, 4);
    // T15: both fills with a NaN colour channel are accepted after sanitising, and counted.
    assert_eq!(stats.ui.entries_sanitised, 2);
    assert_eq!(px(&img, 5, 5), BG);
    assert_eq!(px(&img, 35, 35), BG, "a NaN alpha draws nothing");
    assert_eq!(
        px(&img, 55, 55),
        [0, 0, 0, 255],
        "a NaN red channel draws as 0"
    );
    Ok(())
}
