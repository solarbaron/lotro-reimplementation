// SPDX-License-Identifier: GPL-3.0-or-later
//! Helpers shared by the UI pass tests (`ui_pass.rs`, `ui_golden.rs`): synthetic canvases,
//! UI texture uploads and one-call frame rendering. Every canvas is generated here.
//!
//! Included with `#[path]` by more than one test crate, so unused items are allowed.
#![allow(dead_code)]

use tessera_render::{
    FrameStats, OffscreenFormat, PassSpace, RenderError, Renderer, RgbaImage, TextureId, UiClip,
    UiEntry, UiPass, UiRect, UiRotation,
};

/// Stored RGBA of the opaque background every scene starts from.
pub const BG: [u8; 4] = [40, 70, 110, 255];

/// Straight-alpha RGBA → the B, G, R, A bytes of a canvas pixel 0xAARRGGBB.
#[must_use]
pub fn bgra(rgba: [u8; 4]) -> [u8; 4] {
    [rgba[2], rgba[1], rgba[0], rgba[3]]
}

/// A `w × h` canvas (B, G, R, A rows, tightly packed) from a straight-RGBA function.
pub fn canvas(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    (0..w * h).flat_map(|i| bgra(f(i % w, i / w))).collect()
}

/// Create a UI texture holding `f`'s canvas.
pub fn ui_texture(
    r: &mut Renderer<'static>,
    w: u32,
    h: u32,
    f: impl Fn(u32, u32) -> [u8; 4],
) -> Result<TextureId, RenderError> {
    let id = r.create_ui_texture(w, h, Some("ui-test"))?;
    r.update_texture_rect(id, 0, 0, w, h, &canvas(w, h, f), w as usize * 4)?;
    Ok(id)
}

/// A solid UI texture.
pub fn solid(
    r: &mut Renderer<'static>,
    w: u32,
    h: u32,
    rgba: [u8; 4],
) -> Result<TextureId, RenderError> {
    ui_texture(r, w, h, |_, _| rgba)
}

/// Render one frame into a fresh `w × h` target: a screen pass, then a UI pass whose first
/// entry fills the target with [`BG`], then whatever `build` adds.
pub fn render_ui(
    r: &mut Renderer<'static>,
    (w, h): (u32, u32),
    format: OffscreenFormat,
    build: impl FnOnce(&mut UiPass),
) -> Result<(RgbaImage, FrameStats), RenderError> {
    let bg = solid(r, 1, 1, BG)?;
    let target = r.create_offscreen_target(w, h, format)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .pass("world", PassSpace::Screen)
        .clear([0.0, 0.0, 0.0, 1.0]);
    let pass = frame.ui_pass();
    pass.entry(&UiEntry::new(bg, [1, 1], UiRect::new(0, 0, w, h)));
    build(pass);
    let stats = r.end_frame(frame)?;
    let img = r.read_pixels(&target)?;
    r.destroy_texture(bg);
    Ok((img, stats))
}

/// Pixel `(x, y)` or a panic naming it.
#[must_use]
pub fn px(img: &RgbaImage, x: u32, y: u32) -> [u8; 4] {
    img.pixel(x, y)
        .unwrap_or_else(|| panic!("pixel ({x}, {y}) outside {}x{}", img.width, img.height))
}

/// Largest channel difference.
#[must_use]
pub fn diff(a: [u8; 4], b: [u8; 4]) -> u8 {
    a.iter()
        .zip(b)
        .map(|(x, y)| x.abs_diff(y))
        .max()
        .unwrap_or(0)
}

/// Rotate `p` about `c` by `z` degrees with the convention of render.md §6.6 (positive z
/// turns counter-clockwise on a y-down screen).
#[must_use]
pub fn rot_z(p: [f64; 2], c: [f64; 2], z: f64) -> [f64; 2] {
    let (s, co) = z.to_radians().sin_cos();
    let (x, y) = (p[0] - c[0], p[1] - c[1]);
    [c[0] + x * co + y * s, c[1] - x * s + y * co]
}

/// Whether `p` lies inside `r` with at least `margin` pixels to spare (`margin` < 0
/// tests "outside by at least -margin").
#[must_use]
pub fn inside(p: [f64; 2], r: UiRect, margin: f64) -> bool {
    let (x0, y0) = (f64::from(r.x), f64::from(r.y));
    let (x1, y1) = (x0 + f64::from(r.w), y0 + f64::from(r.h));
    p[0] >= x0 + margin && p[0] < x1 - margin && p[1] >= y0 + margin && p[1] < y1 - margin
}

/// Whether `p` lies outside `r` by at least `margin` pixels.
#[must_use]
pub fn outside(p: [f64; 2], r: UiRect, margin: f64) -> bool {
    let (x0, y0) = (f64::from(r.x), f64::from(r.y));
    let (x1, y1) = (x0 + f64::from(r.w), y0 + f64::from(r.h));
    p[0] < x0 - margin || p[0] >= x1 + margin || p[1] < y0 - margin || p[1] >= y1 + margin
}

/// Pixel centres of a `w × h` image.
pub fn centres(w: u32, h: u32) -> impl Iterator<Item = (u32, u32, [f64; 2])> {
    (0..h).flat_map(move |y| (0..w).map(move |x| (x, y, [f64::from(x) + 0.5, f64::from(y) + 0.5])))
}

/// A synthetic GP4-like disc: black, alpha 255 inside radius 85 with a soft edge, 0 outside.
pub fn disc_alpha(x: u32, y: u32) -> u8 {
    let d = ((f64::from(x) + 0.5 - 85.0).powi(2) + (f64::from(y) + 0.5 - 85.0).powi(2)).sqrt();
    ((85.0 - d) * 128.0).clamp(0.0, 255.0) as u8
}

pub const SIZE_P1: (u32, u32) = (272, 320);
pub const RIM: [u8; 4] = [210, 170, 60, 255];
pub const MARKER: [u8; 4] = [240, 240, 240, 255];

/// A synthetic P1-shaped tree (layout values of `game-panels.md` §8.1 and G41): root
/// 224 × 280; the radar at (0, 33) of the root; the viewport at (27, 0) of the radar; the rim
/// at (−34, −33, 237, 237) of the viewport; the marker at the viewport's centre.
pub struct P1 {
    pub root: UiRect,
    pub radar: UiRect,
    pub rim: UiRect,
    pub marker: UiRect,
    root_tex: TextureId,
    radar_tex: TextureId,
    rim_tex: TextureId,
    marker_tex: TextureId,
}

impl P1 {
    pub fn new(r: &mut Renderer<'static>) -> Result<Self, RenderError> {
        let root = UiRect::new(24, 16, 224, 280);
        let radar = UiRect::new(root.x, root.y + 33, 224, 260);
        let vp = (radar.x + 27, radar.y);
        let rim = UiRect::new(vp.0 - 34, vp.1 - 33, 237, 237);
        let marker = UiRect::new(vp.0 + 85 - 14, vp.1 + 85 - 14, 28, 28);
        // The root's own canvas is transparent (the drag box is hidden).
        let root_tex = solid(r, 224, 280, [0; 4])?;
        // The radar's canvas: the disc and the map with the disc's alpha, nothing else.
        let radar_tex = ui_texture(r, 224, 260, |x, y| {
            if (27..197).contains(&x) && y < 170 {
                let (u, v) = (x - 27, y);
                [(u + 30) as u8, (v + 50) as u8, 100, disc_alpha(u, v)]
            } else {
                [0; 4]
            }
        })?;
        // The rim: a ring around the canvas centre that spans almost the whole box.
        let rim_tex = ui_texture(r, 237, 237, |x, y| {
            let d = ((f64::from(x) + 0.5 - 118.5).powi(2) + (f64::from(y) + 0.5 - 118.5).powi(2))
                .sqrt();
            if (100.0..=118.0).contains(&d) {
                RIM
            } else {
                [0; 4]
            }
        })?;
        // The marker art points east.
        let marker_tex = ui_texture(r, 28, 28, |x, y| {
            if x >= 14 && (12..16).contains(&y) {
                MARKER
            } else {
                [0; 4]
            }
        })?;
        Ok(Self {
            root,
            radar,
            rim,
            marker,
            root_tex,
            radar_tex,
            rim_tex,
            marker_tex,
        })
    }

    pub fn marker_centre(&self) -> (u32, u32) {
        ((self.marker.x + 14) as u32, (self.marker.y + 14) as u32)
    }

    /// The composition list in drawing order at compass heading `h`.
    pub fn entries(&self, p: &mut UiPass, h: f32) {
        let none = UiRotation::default();
        let c = self.marker_centre();
        // z = 90 − h turns the east-pointing arrow clockwise on screen by h − 90.
        let marker_rot = [
            UiRotation::z([c.0 as f32, c.1 as f32], 90.0 - h),
            none,
            none,
        ];
        // Nested entries of the radar: the radar's and the root's rotations follow their own;
        // the only enclosing window is the root (space 2, its surface).
        let nested_rot = [none, none, none];
        let clip_root = [UiClip::new(self.root, 2)];
        p.entry(&UiEntry::new(self.root_tex, [224, 280], self.root))
            .entry(
                &UiEntry::new(self.radar_tex, [224, 260], self.radar)
                    .with_rotations(&nested_rot[..2])
                    .with_clips(&[UiClip::new(self.root, 1)]),
            )
            .entry(
                &UiEntry::new(self.marker_tex, [28, 28], self.marker)
                    .with_rotations(&marker_rot)
                    .with_clips(&clip_root),
            )
            .entry(
                &UiEntry::new(self.rim_tex, [237, 237], self.rim)
                    .with_rotations(&nested_rot)
                    .with_clips(&clip_root),
            );
    }
}
