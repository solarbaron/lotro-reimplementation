// SPDX-License-Identifier: GPL-3.0-or-later
//! `--sheet out.png`: the character view rendered headless into a contact sheet, one
//! tile per (camera angle, play time), without opening a window.
//!
//! It runs exactly the character view's pipeline: the figure is assembled by
//! [`crate::character::build_character`], the clip loaded by
//! [`crate::character::load_clip`], each pose sampled by an [`AnimationScheduler`] on the
//! asset workers, and every tile drawn by the viewport's render function
//! ([`crate::viewport::render_offline`]) with the same [`DrawRequest`] the view sends.
//! Rows are angles (`--angles`), columns are times (`--times`). A report on standard
//! output names the figure, the clip and what each tile shows, so a checklist row can
//! cite the command and the sheet can be checked tile by tile.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{Color32, ColorImage};
use tessera_anim::glam::{Mat4, Vec3};
use tessera_anim::{AnimationScheduler, Animator, Playback, SchedulerConfig, Wrap};
use tessera_assets::{JobContext, PoolConfig, WorkerPool};
use tessera_render::MeshShading;

use crate::anim_view::{ClipChoice, PlayState, frame_sphere};
use crate::character::{
    CharacterRequest, Figure, bone_lines, build_catalog, build_character, load_clip,
};
use crate::charspec::{CharacterArgs, SpeciesArg};
use crate::data::DataContext;
use crate::error::ExplorerError;
use crate::viewport::{DrawRequest, OrbitView, PoseDraw, render_offline};

/// Gutter between tiles (pixels).
pub const GUTTER: usize = 4;
/// Largest sheet in pixels (64 Mpx: 256 MiB as RGBA). The tiles, the sheet and the PNG's
/// RGBA copy are held at once, so a sheet is refused above this instead of exhausting
/// memory.
pub const MAX_SHEET_PIXELS: usize = 64 * 1024 * 1024;

/// The size of a sheet of `cols` × `rows` tiles of `w` × `h` pixels with gutters.
///
/// # Errors
///
/// [`ExplorerError::NotApplicable`] when it exceeds [`MAX_SHEET_PIXELS`] (or overflows).
pub fn sheet_size(
    cols: usize,
    rows: usize,
    w: usize,
    h: usize,
) -> Result<[usize; 2], ExplorerError> {
    let side = |n: usize, t: usize| {
        n.checked_mul(t)?
            .checked_add(n.checked_add(1)?.checked_mul(GUTTER)?)
    };
    let size = side(cols, w).zip(side(rows, h));
    match size {
        Some((sw, sh)) if sw.checked_mul(sh).is_some_and(|p| p <= MAX_SHEET_PIXELS) => Ok([sw, sh]),
        _ => Err(ExplorerError::NotApplicable(format!(
            "a sheet of {rows} × {cols} tiles of {w}×{h} pixels is larger than {} Mpx; \
             use fewer times or angles or a smaller --tile",
            MAX_SHEET_PIXELS / (1024 * 1024)
        ))),
    }
}

/// What `--sheet` renders.
#[derive(Debug, Clone)]
pub struct SheetPlan {
    /// The install.
    pub game_dir: PathBuf,
    /// The output PNG.
    pub out: PathBuf,
    /// The character options (figure, items, clip, sliders, overlays). `time` is
    /// ignored: [`SheetPlan::times`] are used.
    pub args: CharacterArgs,
    /// Play times (columns).
    pub times: Vec<f32>,
    /// Camera (yaw, pitch) in degrees (rows).
    pub angles: Vec<[f32; 2]>,
    /// Tile size in pixels.
    pub tile: [u32; 2],
    /// Wireframe overlay.
    pub wireframe: bool,
    /// Frame this sphere (height on the vertical axis, radius; metres) instead of the
    /// whole figure.
    pub focus: Option<[f32; 2]>,
}

/// What a sheet showed.
#[derive(Debug, Clone, Default)]
pub struct SheetReport {
    /// The figure's title.
    pub title: String,
    /// The clip played, its duration and playback rate.
    pub clip: Option<(u32, f32, f32)>,
    /// Lines of facts for the report.
    pub lines: Vec<String>,
    /// The file written.
    pub written: Option<PathBuf>,
}

fn worker(e: impl std::fmt::Display) -> ExplorerError {
    ExplorerError::Worker(e.to_string())
}

/// Renders the sheet of `plan` (see the module documentation).
///
/// # Errors
///
/// When the install does not open, the figure cannot be assembled, no GPU renderer is
/// available, or the PNG cannot be written (never inside the install).
pub fn run_sheet(plan: &SheetPlan) -> Result<SheetReport, ExplorerError> {
    // Refuse an oversized sheet before any work.
    sheet_size(
        plan.times.len().max(1),
        plan.angles.len().max(1),
        plan.tile[0] as usize,
        plan.tile[1] as usize,
    )?;
    let ctx = DataContext::open(&plan.game_dir, None)?;
    let job = JobContext::detached();
    let catalog = build_catalog(&ctx)?;
    let figure_arg = match plan.args.figure.clone() {
        Some(f) => f,
        None => {
            let b = catalog
                .default_body()
                .ok_or_else(|| ExplorerError::NotApplicable(catalog.no_body_reason()))?;
            crate::charspec::FigureArg::Playable {
                species: SpeciesArg::Key(b.species),
                sex: b.sex,
            }
        }
    };
    let figure = catalog.resolve_figure(&figure_arg).map_err(|e| {
        ExplorerError::NotApplicable(if catalog.problems.is_empty() {
            e
        } else {
            format!("{e} (catalogue: {})", catalog.problems.join("; "))
        })
    })?;
    let mut req = CharacterRequest::new(figure);
    req.items.clone_from(&plan.args.items);
    req.sliders.clone_from(&plan.args.sliders);
    let product = Arc::new(build_character(&ctx, &catalog, &req, &job)?);
    let mut report = SheetReport {
        title: product.title.clone(),
        ..SheetReport::default()
    };
    for p in &catalog.problems {
        report.lines.push(format!("catalogue problem: {p}"));
    }
    report.lines.push(format!(
        "figure {} ({}): {} parts, {} skinned, {} held, {} bones, scale {:.4}, {} diagnostics",
        product.title,
        match figure {
            Figure::Playable { .. } => product
                .facts
                .iter()
                .find(|(k, _)| k == "entity")
                .map_or_else(
                    || format!("record {:#010x}", product.did()),
                    |(_, v)| format!("avatar entity {v}"),
                ),
            Figure::Object(d) | Figure::Setup(d) => d.to_string(),
        },
        product.scene.parts.len(),
        product.scene.parts.iter().filter(|p| p.skinned).count(),
        product.held.len(),
        product.bones(),
        product.model_scale,
        product.diagnostics.len()
    ));
    for (k, v) in &product.facts {
        report.lines.push(format!("  {k}: {v}"));
    }
    if let Some((_, hi)) = product.scene.aabb {
        report.lines.push(format!(
            "  bind-pose top {:.3} m × scale {:.4} = standing height {:.3} m",
            hi[2],
            product.model_scale,
            hi[2] * product.model_scale
        ));
    }
    for h in &product.held {
        report.lines.push(format!(
            "  held {} at {} ({}) on {}",
            h.what,
            h.location_label,
            if h.sheathed { "sheathed" } else { "wielded" },
            h.bone
                .as_ref()
                .map_or_else(|| "the origin".into(), ToString::to_string)
        ));
    }
    for d in &product.diagnostics {
        report.lines.push(format!("  diagnostic {d}"));
    }
    for n in &product.scene.notes {
        report.lines.push(format!("  note {n}"));
    }
    for (p, info) in product.scene.parts.iter().zip(&product.parts) {
        let what = match info {
            crate::character::FigurePart::Body {
                priority, origin, ..
            } => format!(
                "{} · {origin}",
                priority.map_or_else(|| "default".into(), |v| format!("priority {v}"))
            ),
            crate::character::FigurePart::Held { held } => product.held.get(*held).map_or_else(
                || "held".into(),
                |h| format!("held at {}", h.location_label),
            ),
        };
        let mut line = format!(
            "  part {}{} · {what}",
            p.caption,
            if p.skinned { " (skinned)" } else { "" }
        );
        if let Some(e) = &p.error {
            line.push_str(&format!(" ERROR {e}"));
        }
        for i in &p.issues {
            line.push_str(&format!(" [{i}]"));
        }
        report.lines.push(line);
    }

    // The clip (the state's choice from the script tables, or a DID).
    let choice: ClipChoice = plan.args.clip.map_or(
        ClipChoice::State(crate::charspec::StateArg::Idle),
        Into::into,
    );
    let (clip_did, rate) = match choice {
        ClipChoice::None => (None, 1.0),
        ClipChoice::Did(d) => (Some(d), 1.0),
        ClipChoice::State(s) => match product.clips.chosen(s) {
            Some(r) => (
                Some(r.clip.0),
                Some(r.rate)
                    .filter(|r| r.is_finite() && *r > 0.0)
                    .unwrap_or(1.0),
            ),
            None => {
                report.lines.push(format!(
                    "no clip selected for {} ({:?})",
                    s.name(),
                    product.clips.error
                ));
                (None, 1.0)
            }
        },
    };
    let clip = match (clip_did, &product.skeleton) {
        (Some(d), Some(sk)) => Some(load_clip(&ctx, d, Some(Arc::clone(sk)), &job)?),
        _ => None,
    };
    if let Some(c) = &clip {
        report.lines.push(format!(
            "clip {:#010x}: {:.3} s, {} frames, {} tracks, {}, rate {rate:.2}{}",
            c.info.did,
            c.info.duration,
            c.info.frames,
            c.info.tracks,
            c.info.kind,
            c.bind_error
                .as_ref()
                .map_or_else(String::new, |e| format!(", NOT BOUND: {e}"))
        ));
        report.clip = Some((c.info.did, c.info.duration, rate));
    }
    let bound = clip.as_ref().and_then(|c| c.bound.clone());

    // Poses on the workers.
    let pool = WorkerPool::new(PoolConfig {
        thread_name: "sheet-worker".into(),
        ..PoolConfig::default()
    });
    let mut poses: Vec<PoseDraw> = Vec::new();
    let mut play = PlayState::default();
    for (k, &t) in plan.times.iter().enumerate() {
        play.time = t;
        let placement = play.placement_matrix(product.model_scale);
        let mut draw = PoseDraw {
            placement,
            ..PoseDraw::default()
        };
        if let Some(sk) = &product.skeleton {
            let mut sched = AnimationScheduler::new(&pool, SchedulerConfig::default());
            let id = sched
                .add(Arc::clone(sk), product.attach.clone())
                .map_err(worker)?;
            let bytes = plan
                .args
                .sliders
                .clone()
                .unwrap_or_else(|| product.sliders.values.clone());
            let (sliders, problems) = product.sliders.build(sk, &bytes);
            for p in problems {
                report.lines.push(format!("  slider problem {p}"));
            }
            if !sliders.is_empty() {
                sched
                    .set_bone_sliders(id, Some(Arc::new(sliders)))
                    .map_err(worker)?;
            }
            let mut anim = Animator::new(Arc::clone(sk));
            if let Some(b) = &bound {
                let mut p = Playback::new(Arc::clone(b), Wrap::Loop);
                p.time = t;
                p.rate = rate;
                anim.play(0, p, 0.0, false).map_err(worker)?;
            }
            sched.submit(id, &anim, 1).map_err(worker)?;
            sched.collect_all(Duration::from_secs(30));
            let pose = sched
                .pose(id)
                .filter(|p| p.frame == 1)
                .ok_or_else(|| worker("the pose did not come back from the workers"))?;
            if let Some(e) = &pose.error {
                report.lines.push(format!("  t={t}: pose error {e}"));
            }
            if pose.report.replaced > 0 {
                report.lines.push(format!(
                    "  t={t}: {} tracks replaced by the reference pose",
                    pose.report.replaced
                ));
            }
            draw.palette.clone_from(&pose.skin);
            draw.attach.clone_from(&pose.attach);
            if plan.args.bones {
                for (a, b) in bone_lines(sk, &pose.model, placement) {
                    draw.lines.push((a, b, [0.2, 0.95, 0.4, 1.0]));
                }
            }
            if plan.args.trail
                && let Some(c) = &clip
            {
                for (pts, color) in [
                    (&c.root_trail, [0.3, 0.75, 1.0, 1.0]),
                    (&c.motion_trail, [1.0, 0.8, 0.2, 1.0]),
                ] {
                    for w in pts.windows(2) {
                        let a = placement.transform_point3(Vec3::from_array(w[0]));
                        let b = placement.transform_point3(Vec3::from_array(w[1]));
                        draw.lines.push((a.to_array(), b.to_array(), color));
                    }
                }
            }
            // The feet: the lowest skinned bone position, to check "on the ground".
            let lowest = pose
                .model
                .iter()
                .map(|m| (placement * *m).w_axis.z)
                .filter(|z| z.is_finite())
                .fold(f32::INFINITY, f32::min);
            report.lines.push(format!(
                "  column {k}: play time {t} s → clip time {:.3} s, lowest bone at z = {lowest:.3} m",
                bound
                    .as_ref()
                    .map_or(0.0, |b| tessera_anim::clip_time(t, b.duration(), Wrap::Loop))
            ));
        }
        poses.push(draw);
    }

    // Tiles.
    let (c, r) = match plan.focus {
        Some([z, r]) => ([0.0, 0.0, z], r),
        None => frame_sphere(&product.scene, product.model_scale),
    };
    let [tw, th] = plan.tile;
    let aspect = tw as f32 / th as f32;
    let mut requests = Vec::new();
    for angle in &plan.angles {
        let mut orbit = OrbitView::new();
        orbit.orbit.yaw = angle[0].to_radians();
        orbit.orbit.pitch = angle[1].to_radians();
        orbit.frame(c, r, aspect);
        for pose in &poses {
            requests.push(DrawRequest {
                scene: 1,
                width: tw,
                height: th,
                camera: orbit.camera(),
                shading: MeshShading::Lit,
                wireframe: plan.wireframe,
                normals: false,
                grid: true,
                highlight: false,
                isolate: false,
                hidden_parts: Vec::new(),
                hidden: Vec::new(),
                selected: None,
                dye: None,
                diffuse_tint: false,
                explode: 0.0,
                pose: Some(Arc::new(pose.clone())),
            });
        }
    }
    let (images, problems) =
        render_offline(Arc::clone(&product.scene), &requests).map_err(ExplorerError::Worker)?;
    for p in problems {
        report.lines.push(format!("  upload problem {p}"));
    }
    let cols = plan.times.len().max(1);
    let rows = plan.angles.len().max(1);
    let sheet = tile(&images, cols, rows, tw as usize, th as usize);
    drop(images);
    let guards = vec![plan.game_dir.clone()];
    let written = crate::screenshot::save_png(&plan.out, &sheet, &guards)?;
    report.written = Some(written);
    report.lines.push(format!(
        "sheet: {rows} rows (angles {:?}) × {cols} columns (times {:?}), tiles {tw}×{th}",
        plan.angles, plan.times
    ));
    Ok(report)
}

/// Places `images` (row-major, each `w` × `h`) in a grid with gutters; missing or
/// mis-sized images leave their cell dark.
///
/// A grid larger than [`MAX_SHEET_PIXELS`] gives an empty image.
#[must_use]
pub fn tile(images: &[ColorImage], cols: usize, rows: usize, w: usize, h: usize) -> ColorImage {
    let Ok([width, height]) = sheet_size(cols, rows, w, h) else {
        return ColorImage::new([0, 0], Vec::new());
    };
    let mut out = ColorImage::new([width, height], vec![Color32::from_gray(8); width * height]);
    for (i, img) in images.iter().enumerate().take(cols * rows) {
        if img.size != [w, h] || img.pixels.len() != w * h {
            continue;
        }
        let (cx, cy) = (i % cols, i / cols);
        let x0 = GUTTER + cx * (w + GUTTER);
        let y0 = GUTTER + cy * (h + GUTTER);
        for y in 0..h {
            let src = &img.pixels[y * w..(y + 1) * w];
            let start = (y0 + y) * width + x0;
            if let Some(dst) = out.pixels.get_mut(start..start + w) {
                dst.copy_from_slice(src);
            }
        }
    }
    out
}

/// The figure placement of a sheet tile (exposed for tests).
#[must_use]
pub fn placement(scale: f32) -> Mat4 {
    PlayState::default().placement_matrix(scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_are_placed_row_major_with_gutters() {
        let red = ColorImage::new([2, 2], vec![Color32::RED; 4]);
        let blue = ColorImage::new([2, 2], vec![Color32::BLUE; 4]);
        let wrong = ColorImage::new([3, 2], vec![Color32::GREEN; 6]);
        let s = tile(&[red, blue, wrong], 2, 2, 2, 2);
        assert_eq!(s.size, [2 * 2 + 3 * GUTTER, 2 * 2 + 3 * GUTTER]);
        let px = |x: usize, y: usize| s.pixels[y * s.size[0] + x];
        assert_eq!(px(GUTTER, GUTTER), Color32::RED);
        assert_eq!(px(2 * GUTTER + 2, GUTTER), Color32::BLUE);
        // The mis-sized third image is left out.
        assert_eq!(px(GUTTER, 2 * GUTTER + 2), Color32::from_gray(8));
        assert_eq!(placement(2.0).x_axis.x, 2.0);
    }

    #[test]
    fn oversized_sheets_are_refused() {
        // The largest accepted options (16 times, 8 angles, 2048×2048 tiles) would need
        // about 2 GiB of tiles plus 2 × 2.16 GB for the sheet and its PNG copy.
        assert!(sheet_size(16, 8, 2048, 2048).is_err());
        assert!(sheet_size(usize::MAX, 2, 2048, 2048).is_err());
        // The default tile at the most times and angles fits.
        assert_eq!(
            sheet_size(16, 8, 480, 640).unwrap(),
            [16 * 480 + 17 * GUTTER, 8 * 640 + 9 * GUTTER]
        );
        let t = tile(&[], 16, 8, 2048, 2048);
        assert_eq!(t.size, [0, 0]);
    }
}
