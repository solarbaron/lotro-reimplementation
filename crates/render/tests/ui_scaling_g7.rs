// SPDX-License-Identifier: GPL-3.0-or-later
//! Golden G7 of `docs/specs/ui-scaling.md` §9.3 (decision 0031 tier 1): a frame **in the
//! middle of a scale change**, C2 → C3, drawn by the headless renderer.
//!
//! §4.5 step 3 ("Preview, in the same frame"): after a change of *Se* every surface keeps its
//! last complete picture and is composed onto its **new** physical box, resampled by the GPU
//! with `ui_scale_filter` (here its default, linear; §6.6), until the settle step repaints it
//! at the new content scale. No surface is ever blank because of the change (invariant I9).
//!
//! The scene is synthetic (canvases generated here, no game data): two root panels of a UI
//! laid out at C2 (1920 × 1080 at *S* = 1, *F* = 1920 × 1080, *A* = *F* for 21:9) and painted
//! there at *c* = 1:
//!
//! - a 400 × 300 frame panel (`G1`'s look: 16 × 16 corners, 2-pixel tiled edges, a fitted
//!   title bar over a body), centred in the safe area: (760, 390) at C2;
//! - a 240 × 64 HUD panel docked at (8, 8).
//!
//! Then the user scale becomes 1.25 (C3: *F* = 1536 × 864, *Kx* = *Ky* = 1.25). In UI units the
//! frame is laid out again at the centre of the new safe area, (568, 282), and the HUD stays at
//! (8, 8); their physical boxes follow from the pixel-centre rule of §3.6 (computed below),
//! and the frame's **old** 400 × 300 canvas is composed onto its new 500 × 375 physical box,
//! the HUD's 240 × 64 canvas onto 300 × 80. The golden is that composite.
//!
//! **Coverage limit.** `tessera-render` does not depend on `tessera-ui`, so this test builds
//! the C3 geometry and the entry list itself (its own §3.6 edge function and centring) and
//! never runs the UI's preview path. It shows that the renderer composes a stale canvas onto a
//! differently sized box correctly; it cannot catch a regression in `tessera-ui` that blanks
//! a surface or composes it onto the wrong box mid-change. That side is covered in
//! `crates/ui/tests/ui_scaling_live.rs` (`scale_change_previews_the_old_canvas_on_the_new_box`:
//! in the frames after C2 → C3 the composition list holds the old canvas size with the new
//! §3.6 physical box as its destination). Regenerate with
//! `TESSERA_BLESS=1 cargo test -p tessera-render --test ui_scaling_g7` and check it by eye.

#[path = "support/golden.rs"]
mod golden;
#[path = "support/gpu.rs"]
mod gpu;
#[path = "ui_common/mod.rs"]
mod ui_common;

use std::path::Path;

use tessera_render::{
    OffscreenFormat, RenderError, Renderer, RgbaImage, TextureId, UiEntry, UiFilter, UiRect,
};
use ui_common::{BG, diff, px, render_ui, ui_texture};

const UNORM: OffscreenFormat = OffscreenFormat::Rgba8Unorm;

/// C2 and C3 of §9.2: the window (physical pixels) and the effective scales.
const WINDOW: (u32, u32) = (1920, 1080);
const SE_C2: f64 = 1.0;
const SE_C3: f64 = 1.25;

/// The panels' sizes in UI units (their layout boxes keep these through the change).
const FRAME: (i32, i32) = (400, 300);
const HUD: (i32, i32) = (240, 64);
const HUD_AT: (i32, i32) = (8, 8);

const CORNER: [u8; 4] = [192, 128, 64, 255];
const EDGE_A: [u8; 4] = [128, 96, 48, 255];
const EDGE_B: [u8; 4] = [96, 72, 36, 255];
const BODY: [u8; 4] = [32, 48, 64, 255];
const TITLE: [u8; 4] = [74, 58, 34, 255];
const TITLE_TEXT: [u8; 4] = [230, 220, 190, 255];
const HUD_FILL: [u8; 4] = [48, 144, 192, 255];
const HUD_MARK: [u8; 4] = [240, 240, 240, 255];

/// The UI screen of a configuration (§3.2): `Wu = round(Wp ÷ Se)`, `Hu = round(Hp ÷ Se)`
/// (halves up) and the axis factors `Kx = Wp ÷ Wu`, `Ky = Hp ÷ Hu`.
struct Screen {
    wu: i32,
    hu: i32,
    kx: f64,
    ky: f64,
}

impl Screen {
    fn new(se: f64) -> Self {
        let (wp, hp) = (f64::from(WINDOW.0), f64::from(WINDOW.1));
        let wu = (wp / se + 0.5).floor();
        let hu = (hp / se + 0.5).floor();
        Self {
            wu: wu as i32,
            hu: hu as i32,
            kx: wp / wu,
            ky: hp / hu,
        }
    }

    /// The physical column of a UI edge (§3.6 "Edge → physical"): the smallest *X* with
    /// (*X* + ½) ÷ *K* ≥ *L*, from the closed form ⌈*L* × *K* − ½⌉ corrected by one where the
    /// comparison disagrees.
    fn edge(l: f64, k: f64) -> i32 {
        let mut x = (l * k - 0.5).ceil() as i32;
        if (f64::from(x) + 0.5) / k < l {
            x += 1;
        } else if (f64::from(x - 1) + 0.5) / k >= l {
            x -= 1;
        }
        x
    }

    /// The physical box of a UI-unit box (§3.6 "Box → physical").
    fn physical(&self, (l, t): (i32, i32), (w, h): (i32, i32)) -> UiRect {
        let x0 = Self::edge(f64::from(l), self.kx);
        let x1 = Self::edge(f64::from(l + w), self.kx);
        let y0 = Self::edge(f64::from(t), self.ky);
        let y1 = Self::edge(f64::from(t + h), self.ky);
        UiRect::new(x0, y0, (x1 - x0) as u32, (y1 - y0) as u32)
    }

    /// The top-left of a `size` box centred in the safe area. The 21:9 safe area is the whole
    /// screen at both configurations (*Wu* ≤ 2.4 × *Hu*), so it is centred in *F* (edge mode
    /// 0's centre rule, truncated like the run-time layout's halves).
    fn centred(&self, (w, h): (i32, i32)) -> (i32, i32) {
        ((self.wu - w) / 2, (self.hu - h) / 2)
    }
}

/// The frame panel's canvas at *c* = 1: corners, alternating 2-pixel edge tiles, a title bar
/// fitted between the top corners with a band of "text", and the body.
fn frame_pixel(x: u32, y: u32) -> [u8; 4] {
    let (w, h) = (FRAME.0 as u32, FRAME.1 as u32);
    let corner = (x < 16 || x >= w - 16) && (y < 16 || y >= h - 16);
    if corner {
        return CORNER;
    }
    let edge = x < 2 || x >= w - 2 || y < 2 || y >= h - 2;
    if edge {
        // A 2-pixel tile repeated along the edge.
        let along = if x < 2 || x >= w - 2 { y } else { x };
        return if (along / 2).is_multiple_of(2) {
            EDGE_A
        } else {
            EDGE_B
        };
    }
    if y < 28 && (16..w - 16).contains(&x) {
        let text = (10..18).contains(&y) && !(x / 3).is_multiple_of(3) && (150..250).contains(&x);
        return if text { TITLE_TEXT } else { TITLE };
    }
    BODY
}

/// The HUD panel's canvas at *c* = 1: a fill with a 1-pixel border and a marker square.
fn hud_pixel(x: u32, y: u32) -> [u8; 4] {
    let (w, h) = (HUD.0 as u32, HUD.1 as u32);
    if x == 0
        || y == 0
        || x == w - 1
        || y == h - 1
        || ((8..24).contains(&x) && (8..24).contains(&y))
    {
        HUD_MARK
    } else {
        HUD_FILL
    }
}

/// The preview frame: the old canvases composed onto the C3 physical boxes, linear filter.
fn preview(
    r: &mut Renderer<'static>,
    frame: TextureId,
    hud: TextureId,
) -> Result<(RgbaImage, UiRect, UiRect), RenderError> {
    let c3 = Screen::new(SE_C3);
    let frame_dest = c3.physical(c3.centred(FRAME), FRAME);
    let hud_dest = c3.physical(HUD_AT, HUD);
    let (img, _) = render_ui(r, WINDOW, UNORM, |pass| {
        for (tex, size, dest) in [(hud, HUD, hud_dest), (frame, FRAME, frame_dest)] {
            let mut e = UiEntry::new(tex, [size.0 as u32, size.1 as u32], dest);
            e.filter = UiFilter::Linear;
            pass.entry(&e);
        }
    })?;
    Ok((img, frame_dest, hud_dest))
}

#[test]
fn configurations_are_c2_and_c3() {
    let c2 = Screen::new(SE_C2);
    assert_eq!((c2.wu, c2.hu), (1920, 1080));
    assert_eq!(c2.centred(FRAME), (760, 390));
    // At C2 every physical box is its UI box (Kx = Ky = 1): the canvases are 1 : 1 there.
    assert_eq!(
        c2.physical(c2.centred(FRAME), FRAME),
        UiRect::new(760, 390, 400, 300)
    );
    let c3 = Screen::new(SE_C3);
    assert_eq!((c3.wu, c3.hu), (1536, 864), "§9.2 C3");
    assert_eq!((c3.kx, c3.ky), (1.25, 1.25));
    assert_eq!(c3.centred(FRAME), (568, 282));
    // §3.6: Ex(568) = 710, Ex(968) = 1210, Ey(282) = 352, Ey(582) = 727.
    assert_eq!(
        c3.physical(c3.centred(FRAME), FRAME),
        UiRect::new(710, 352, 500, 375)
    );
    assert_eq!(c3.physical(HUD_AT, HUD), UiRect::new(10, 10, 300, 80));
}

#[test]
fn g7_preview_c2_to_c3() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let frame = ui_texture(&mut r, FRAME.0 as u32, FRAME.1 as u32, frame_pixel)?;
    let hud = ui_texture(&mut r, HUD.0 as u32, HUD.1 as u32, hud_pixel)?;
    let (img, frame_dest, hud_dest) = preview(&mut r, frame, hud)?;
    let (again, _, _) = preview(&mut r, frame, hud)?;
    assert!(img == again, "G7: render is not deterministic");

    // Invariant I9: neither surface is blank; each covers its whole new physical box.
    for (name, d) in [("frame", frame_dest), ("hud", hud_dest)] {
        let (x0, y0) = (d.x as u32, d.y as u32);
        for (x, y) in [
            (x0, y0),
            (x0 + d.w - 1, y0),
            (x0, y0 + d.h - 1),
            (x0 + d.w - 1, y0 + d.h - 1),
            (x0 + d.w / 2, y0 + d.h / 2),
        ] {
            assert!(
                diff(px(&img, x, y), BG) > 8,
                "{name}: ({x}, {y}) shows the background"
            );
        }
        // Just outside the box the background shows (the box is exact, §6.6).
        assert!(
            diff(px(&img, x0 + d.w, y0 + d.h / 2), BG) <= 1,
            "{name}: right"
        );
        assert!(
            diff(px(&img, x0 + d.w / 2, y0 + d.h), BG) <= 1,
            "{name}: bottom"
        );
    }
    // The old 16 × 16 corner is magnified by 1.25 (about 20 physical pixels), not repainted.
    let (fx, fy) = (frame_dest.x as u32, frame_dest.y as u32);
    assert!(
        diff(px(&img, fx + 17, fy + 17), CORNER) <= 4,
        "corner scaled up"
    );
    assert!(
        diff(px(&img, fx + 23, fy + 40), BODY) <= 4,
        "body beyond the corner"
    );

    golden::check(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/uiscale_g7_preview_c2_c3.png"),
        &img,
        golden::Tolerance::default(),
    );
    r.destroy_texture(frame);
    r.destroy_texture(hud);
    Ok(())
}
