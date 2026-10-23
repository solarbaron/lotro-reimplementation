// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI viewport target path (`docs/specs/ui-widgets.md` §12.3, `docs/specs/render.md`
//! §6.10): a synthetic figure drawn into a 62 × 62 [`ViewportTarget`] in a premultiplied
//! scene pass onto the opaque black background, a synthetic opacity mask applied, and the
//! target composed by the UI pass as external content over panel art, at 1 : 1, scaled with
//! linear filtering and, for comparison, without background and mask; and a fourth target
//! with a non-black, half-transparent background and an asymmetric mask (a disc and the
//! top-left quadrant), whose pixel positions catch a flipped mask or a wrong channel order
//! or premultiplication of the background.
//!
//! The figure is built from the renderer's own procedural shapes and the mask is generated
//! here, so the image contains no game data. Regenerate with
//! `TESSERA_BLESS=1 cargo test -p tessera-render --test viewport_golden` and review
//! `tests/golden/viewport_portrait.png`.

#[path = "support/golden.rs"]
mod golden;
#[path = "support/gpu.rs"]
mod gpu;
#[path = "ui_common/mod.rs"]
mod ui_common;

use std::path::Path;

use tessera_render::glam::{Mat4, Vec3};
use tessera_render::{
    Camera, ColorSpace, Filter, Lighting, Material, MeshData, MeshDraw, MeshId, OffscreenFormat,
    Perspective, RenderError, Renderer, RgbaImage, TextureId, TextureUpload, UiEntry, UiFilter,
    UiRect, UiSource, ViewportFinish, ViewportTarget, Wrap,
};
use ui_common::{px, render_ui, ui_texture};

const SIDE: u32 = 62;
const SCREEN: (u32, u32) = (296, 112);
const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// The synthetic figure: body, shoulders and head, standing on the Y axis.
struct Figure {
    body: MeshId,
    shoulders: MeshId,
    head: MeshId,
}

impl Figure {
    fn new(r: &mut Renderer<'static>) -> Result<Self, RenderError> {
        Ok(Self {
            body: r.upload_mesh(&MeshData::tube(0.32, 1.1, 24))?,
            shoulders: r.upload_mesh(&MeshData::cube(0.5))?,
            head: r.upload_mesh(&MeshData::uv_sphere(0.26, 20, 12))?,
        })
    }

    /// Draws the figure into a premultiplied pass of `frame`, through a camera like the
    /// portrait scene's (60° field of view, far plane 20).
    fn draw(&self, frame: &mut tessera_render::Frame) {
        let camera = Camera {
            eye: Vec3::new(0.35, 1.45, 2.2),
            target: Vec3::new(0.1, 1.25, 0.0),
            up: Vec3::Y,
            projection: Perspective {
                fov_y: 60f32.to_radians(),
                near: 0.1,
                far: 20.0,
            },
        };
        let pass = frame.world_pass("figure", &camera);
        pass.premultiplied_alpha();
        pass.lighting(Lighting {
            direction: Vec3::new(0.5, 0.7, 0.6),
            color: [0.9, 0.85, 0.75],
            ambient: [0.25, 0.25, 0.3],
        });
        pass.mesh(MeshDraw::new(
            self.body,
            Mat4::from_translation(Vec3::new(0.0, 0.55, 0.0)),
            Material::color([0.25, 0.45, 0.8, 1.0]),
        ));
        pass.mesh(MeshDraw::new(
            self.shoulders,
            Mat4::from_translation(Vec3::new(0.0, 1.12, 0.0))
                * Mat4::from_scale(Vec3::new(1.4, 0.22, 0.6)),
            Material::color([0.7, 0.3, 0.2, 1.0]),
        ));
        pass.mesh(MeshDraw::new(
            self.head,
            Mat4::from_translation(Vec3::new(0.0, 1.5, 0.0)),
            Material::color([0.9, 0.75, 0.6, 1.0]),
        ));
    }

    fn destroy(self, r: &mut Renderer<'static>) {
        for m in [self.body, self.shoulders, self.head] {
            r.destroy_mesh(m);
        }
    }
}

/// A straight half-transparent orange background (straight linear RGBA).
const TINT: [f32; 4] = [0.8, 0.2, 0.1, 0.5];

/// A 64 × 64 opacity mask: a soft-edged disc of radius 29 in the alpha channel (colour
/// white), linear, stretched over the target; with `quadrant` the top-left quadrant is
/// opaque too (an asymmetric mask).
fn mask(r: &mut Renderer<'static>, quadrant: bool) -> Result<TextureId, RenderError> {
    let n = 64u32;
    let rgba: Vec<u8> = (0..n * n)
        .flat_map(|i| {
            let (x, y) = (f64::from(i % n) + 0.5, f64::from(i / n) + 0.5);
            let d = ((x - 32.0).powi(2) + (y - 32.0).powi(2)).sqrt();
            let mut a = ((29.0 - d) * 128.0).clamp(0.0, 255.0) as u8;
            if quadrant && x < 32.0 && y < 32.0 {
                a = 255;
            }
            [255, 255, 255, a]
        })
        .collect();
    r.upload_texture(&TextureUpload {
        color_space: ColorSpace::Linear,
        filter: Filter::Linear,
        wrap: Wrap::Clamp,
        ..TextureUpload::rgba8(n, n, &rgba)
    })
}

/// Draws the figure into `target` with `finish`.
fn draw_target(
    r: &mut Renderer<'static>,
    target: &ViewportTarget,
    figure: &Figure,
    finish: &ViewportFinish,
) -> Result<(), RenderError> {
    let mut frame = r.begin_viewport_frame(target)?;
    figure.draw(&mut frame);
    r.end_viewport_frame(frame, finish)?;
    Ok(())
}

/// The targets and the composed screen.
#[derive(Debug, Clone, PartialEq)]
struct Scene {
    masked: RgbaImage,
    plain: RgbaImage,
    tinted: RgbaImage,
    screen: RgbaImage,
}

/// The scene (module docs).
fn scene(r: &mut Renderer<'static>) -> Result<Scene, RenderError> {
    let figure = Figure::new(r)?;
    let mask_disc = mask(r, false)?;
    let mask_quadrant = mask(r, true)?;
    let masked = r.create_viewport_target(SIDE, SIDE, Filter::Linear)?;
    let plain = r.create_viewport_target(SIDE, SIDE, Filter::Linear)?;
    let tinted = r.create_viewport_target(SIDE, SIDE, Filter::Linear)?;
    draw_target(
        r,
        &masked,
        &figure,
        &ViewportFinish {
            background: Some(BLACK),
            mask: Some(mask_disc),
        },
    )?;
    draw_target(r, &plain, &figure, &ViewportFinish::default())?;
    draw_target(
        r,
        &tinted,
        &figure,
        &ViewportFinish {
            background: Some(TINT),
            mask: Some(mask_quadrant),
        },
    )?;
    let masked_px = r.read_viewport_pixels(&masked)?;
    let plain_px = r.read_viewport_pixels(&plain)?;
    let tinted_px = r.read_viewport_pixels(&tinted)?;
    // Panel art: a warm frame with a lighter well, standing in for the frame's art.
    let panel = ui_texture(r, 32, 32, |x, y| {
        let edge = x < 3 || y < 3 || x > 28 || y > 28;
        if edge {
            [150, 110, 50, 255]
        } else {
            [90 + (x * 2) as u8, 80 + (y * 2) as u8, 70, 255]
        }
    })?;
    let ext = |t: &ViewportTarget, dest| {
        UiEntry::new(t.texture(), [SIDE, SIDE], dest)
            .with_source(UiSource::Premultiplied)
            .with_filter(UiFilter::Linear)
    };
    let (screen, _) = render_ui(r, SCREEN, OffscreenFormat::Rgba8Unorm, |p| {
        for x in [4, 76, 152, 224] {
            p.entry(&UiEntry::new(panel, [32, 32], UiRect::new(x, 4, 68, 104)));
        }
        p.entry(&ext(&masked, UiRect::new(7, 25, SIDE, SIDE)))
            .entry(&ext(&masked, UiRect::new(79, 17, 62, 78)))
            .entry(&ext(&plain, UiRect::new(155, 25, SIDE, SIDE)))
            .entry(&ext(&tinted, UiRect::new(227, 25, SIDE, SIDE)));
    })?;
    r.destroy_texture(panel);
    r.destroy_viewport_target(masked);
    r.destroy_viewport_target(plain);
    r.destroy_viewport_target(tinted);
    r.destroy_texture(mask_disc);
    r.destroy_texture(mask_quadrant);
    figure.destroy(r);
    Ok(Scene {
        masked: masked_px,
        plain: plain_px,
        tinted: tinted_px,
        screen,
    })
}

#[test]
fn viewport_target_with_figure_and_mask() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.2, 0.3, 0.4, 1.0]) else {
        return Ok(());
    };
    let first = scene(&mut r)?;
    let again = scene(&mut r)?;
    assert!(first == again, "the viewport scene is not deterministic");
    let Scene {
        masked,
        plain,
        tinted,
        screen,
    } = first;

    // §12.3 step 2: outside the mask's disc the target is transparent black.
    for (x, y) in [(0, 0), (61, 0), (0, 61), (61, 61)] {
        assert_eq!(px(&masked, x, y), [0, 0, 0, 0], "masked corner ({x}, {y})");
    }
    // §12.3 step 1: inside the disc where the figure draws nothing, the opaque black
    // background (row 8, left of the head).
    assert_eq!(
        px(&masked, 22, 8),
        [0, 0, 0, 255],
        "background inside the mask"
    );
    // The figure itself is opaque and coloured inside the disc.
    let body = px(&masked, 31, 50);
    assert_eq!(body[3], 255, "figure pixel opaque");
    assert!(body[2] > body[0], "the blue body: {body:?}");
    // Without background or mask, the uncovered pixels stay transparent (the frame
    // starts cleared to transparent black) and the figure is the same.
    assert_eq!(px(&plain, 22, 8), [0, 0, 0, 0], "no background");
    assert_eq!(px(&plain, 31, 50), body, "same figure without the mask");
    // Every masked pixel is the plain pixel over black times the mask's alpha.
    let n = (masked.width * masked.height) as usize;
    for i in 0..n {
        let (m, p) = (
            &masked.data[i * 4..i * 4 + 4],
            &plain.data[i * 4..i * 4 + 4],
        );
        assert!(m.iter().take(3).all(|c| *c <= m[3]), "premultiplied");
        let x = (i as u32 % SIDE) as f64 + 0.5;
        let y = (i as u32 / SIDE) as f64 + 0.5;
        let d = ((x - 31.0).powi(2) + (y - 31.0).powi(2)).sqrt();
        if d < 24.0 {
            assert_eq!(m[3], 255, "inside the disc at {x}, {y}");
            for c in 0..3 {
                assert!(m[c].abs_diff(p[c]) <= 1, "colour kept inside the disc");
            }
        } else if d > 30.5 {
            assert_eq!(m, &[0, 0, 0, 0], "outside the disc at {x}, {y}");
        }
    }
    // Composition: the panel art shows around the disc, the portrait inside it.
    assert_eq!(px(&screen, 8, 26)[3], 255);
    let around = px(&screen, 8, 26);
    assert!(around[0] > 80, "panel art at the masked corner: {around:?}");
    assert_eq!(
        px(&screen, 7 + 22, 25 + 8),
        [0, 0, 0, 255],
        "black background"
    );

    // The asymmetric mask on the half-transparent orange background: only the top-left
    // corner is inside the mask (not flipped in either axis), where the background shows
    // premultiplied, red strongest (R, G, B order).
    let bg = px(&tinted, 2, 2);
    let want = [102u8, 26, 13, 128];
    assert!(
        bg.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 2),
        "premultiplied background at the top-left corner: {bg:?}, want {want:?}"
    );
    for (x, y) in [(59, 2), (2, 59), (59, 59)] {
        assert_eq!(
            px(&tinted, x, y),
            [0, 0, 0, 0],
            "outside the mask ({x}, {y})"
        );
    }
    // The opaque figure covers the background inside the disc.
    assert_eq!(
        px(&tinted, 31, 50),
        body,
        "the figure over the tinted background"
    );
    // Composed: the half-transparent background over the panel art at the top-left
    // corner, the panel art alone at the bottom-right one.
    let over = px(&screen, 227 + 2, 25 + 2);
    let art = px(&screen, 227 + 59, 25 + 59);
    assert_ne!(over, art, "the background shows over the art: {over:?}");
    assert!(over[0] > over[2], "orange over the art: {over:?}");

    golden::check(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/viewport_portrait.png"),
        &screen,
        golden::Tolerance::default(),
    );
    Ok(())
}
