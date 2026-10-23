// SPDX-License-Identifier: GPL-3.0-or-later
//! Committed synthetic golden of the UI-scaling glyph ladder (decision 0031 tier 1,
//! `ui-scaling.md` §9.3 golden G2, §6.4.4).
//!
//! The spec's G2 draws "the same paragraph at the ladder's exact, downsampled and
//! distance-derived heights". The painted-paragraph form — the paragraph laid out and painted
//! through the live `tessera-ui` text layout and painter at *c* = 1, 1.25, 1.5, 2 and 3 — is
//! `ui_scaling_text_g2.rs` (`golden/uiscale_g2_c*.png`). This file keeps the golden of the
//! **derivation itself**, the heart of G2: it builds a synthetic bitmap-font glyph as five
//! records at the ladder heights 10, 12, 14, 16 and 20 (§9.3), then renders one glyph at an
//! exact ladder height, two downsampled heights (`k < 1`, the area filter) and two
//! distance-derived heights (`k > 1`, the §6.4.4 distance method) straight from the pure
//! coverage functions of `tessera_ui::scaling`, and compares the picture pixel-for-pixel against
//! `tests/golden/uiscale_glyph_ladder.png`.
//!
//! `TESSERA_BLESS=1` rewrites the image. One step per channel, no outliers, no install file
//! opened. The painted-paragraph G2 beside it (`ui_scaling_text_g2.rs`) uses the same ladder
//! heights (the content column of §9.3 is the oracle for both).

use std::path::PathBuf;

use tessera_ui::scaling::{self, SourceKind};

/// The synthetic font ladder of §9.3: the line heights the "records" exist at.
const LADDER: [i32; 5] = [10, 12, 14, 16, 20];

/// A synthetic glyph rendered analytically at line height `h`: a rectangular ring (a "□"), with
/// a margin and a border band whose thickness is a fixed fraction of the height, so its edges
/// and stems shrink visibly with the size. Returns `(coverage, w, h)`, coverage row-major, one
/// byte per texel (0 = empty, 255 = full ink), anti-aliased by 4 × 4 super-sampling.
fn record_glyph(h: i32) -> (Vec<u8>, usize, usize) {
    let hf = f64::from(h);
    let w = (hf * 0.75).round() as i32;
    let (wf, w, h) = (f64::from(w), w as usize, h as usize);
    let m = hf * 0.12; // outer margin
    let th = hf * 0.20; // border thickness
    let out_l = m;
    let out_t = m;
    let out_r = wf - m;
    let out_b = hf - m;
    let in_l = out_l + th;
    let in_t = out_t + th;
    let in_r = out_r - th;
    let in_b = out_b - th;
    let mut cov = vec![0u8; w * h];
    for py in 0..h {
        for px in 0..w {
            let mut hit = 0u32;
            for sy in 0..4 {
                for sx in 0..4 {
                    let x = px as f64 + (f64::from(sx) + 0.5) / 4.0;
                    let y = py as f64 + (f64::from(sy) + 0.5) / 4.0;
                    let in_outer = x >= out_l && x < out_r && y >= out_t && y < out_b;
                    let in_inner = x >= in_l && x < in_r && y >= in_t && y < in_b;
                    if in_outer && !in_inner {
                        hit += 1;
                    }
                }
            }
            cov[py * w + px] = (hit * 255 / 16) as u8;
        }
    }
    (cov, w, h)
}

/// The derived coverage for a target height from the ladder (§6.4.3, §6.4.4): choose the source
/// record, then downsample or distance-derive to the target box `round(src × k)` per axis.
fn derive_for(target: i32) -> (Vec<u8>, usize, usize) {
    let src = scaling::choose_glyph_source(&LADDER, target);
    let (cov, sw, sh) = record_glyph(src.source_height);
    match src.kind {
        SourceKind::Exact => (cov, sw, sh),
        SourceKind::Downsampled => {
            let dw = (sw as f64 * src.k).round().max(1.0) as usize;
            let dh = (sh as f64 * src.k).round().max(1.0) as usize;
            (scaling::downsample_coverage(&cov, sw, sh, dw, dh), dw, dh)
        }
        SourceKind::Distance => {
            let dw = (sw as f64 * src.k).round().max(sw as f64) as usize;
            let dh = (sh as f64 * src.k).round().max(sh as f64) as usize;
            (scaling::distance_coverage(&cov, sw, sh, dw, dh), dw, dh)
        }
    }
}

const CELL_W: usize = 36;
const CELL_H: usize = 40;
const COLS: usize = 5;
const ROWS: usize = 2;

/// Blits a coverage glyph as black ink over white into cell `(col, row)` of the RGBA image.
fn blit(img: &mut [u8], img_w: usize, col: usize, row: usize, cov: &[u8], gw: usize, gh: usize) {
    let ox = col * CELL_W + 2;
    let oy = row * CELL_H + 2;
    for y in 0..gh.min(CELL_H - 2) {
        for x in 0..gw.min(CELL_W - 2) {
            let c = cov.get(y * gw + x).copied().unwrap_or(0);
            let v = 255 - c; // black ink over a white cell
            let p = ((oy + y) * img_w + (ox + x)) * 4;
            img[p] = v;
            img[p + 1] = v;
            img[p + 2] = v;
            img[p + 3] = 255;
        }
    }
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("uiscale_glyph_ladder.png")
}

fn write_png(path: &std::path::Path, data: &[u8], w: u32, h: u32) {
    let f = std::fs::File::create(path).expect("create png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    let mut writer = e.write_header().expect("png header");
    writer.write_image_data(data).expect("png data");
}

fn read_png(path: &std::path::Path) -> Option<(Vec<u8>, u32, u32)> {
    let f = std::fs::File::open(path).ok()?;
    let d = png::Decoder::new(std::io::BufReader::new(f));
    let mut r = d.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()?];
    let info = r.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    buf.truncate(info.buffer_size());
    Some((buf, info.width, info.height))
}

#[test]
fn glyph_ladder_golden() {
    let img_w = COLS * CELL_W;
    let img_h = ROWS * CELL_H;
    let mut img = vec![255u8; img_w * img_h * 4]; // white

    // Row 0: the five ladder records at their native heights (10, 12, 14, 16, 20).
    for (col, &h) in LADDER.iter().enumerate() {
        let (cov, gw, gh) = record_glyph(h);
        blit(&mut img, img_w, col, 0, &cov, gw, gh);
    }
    // Row 1: one exact height and two each of the downsampled and distance-derived heights.
    // 16 is an exact ladder hit; 8 and 13 downsample (from 10 and 14); 24 and 30 derive by the
    // distance method (from 20). Guard the classification so the golden stays a true oracle.
    assert_eq!(
        scaling::choose_glyph_source(&LADDER, 16).kind,
        SourceKind::Exact
    );
    assert_eq!(
        scaling::choose_glyph_source(&LADDER, 8).kind,
        SourceKind::Downsampled
    );
    assert_eq!(
        scaling::choose_glyph_source(&LADDER, 13).kind,
        SourceKind::Downsampled
    );
    assert_eq!(
        scaling::choose_glyph_source(&LADDER, 24).kind,
        SourceKind::Distance
    );
    assert_eq!(
        scaling::choose_glyph_source(&LADDER, 30).kind,
        SourceKind::Distance
    );
    for (col, &target) in [16, 8, 13, 24, 30].iter().enumerate() {
        let (cov, gw, gh) = derive_for(target);
        assert!(
            !cov.is_empty(),
            "derivation for {target} produced no coverage"
        );
        blit(&mut img, img_w, col, 1, &cov, gw, gh);
    }

    let path = golden_path();
    if std::env::var_os("TESSERA_BLESS").is_some_and(|v| v == "1") {
        std::fs::create_dir_all(path.parent().expect("dir")).expect("golden dir");
        write_png(&path, &img, img_w as u32, img_h as u32);
        eprintln!("golden: blessed {}", path.display());
        return;
    }
    let Some((want, ww, wh)) = read_png(&path) else {
        panic!(
            "golden {} missing; run with TESSERA_BLESS=1 and check it by eye",
            path.display()
        );
    };
    assert_eq!((ww, wh), (img_w as u32, img_h as u32), "golden size");
    let actual = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("uiscale_glyph_ladder.actual.png");
    let mut bad = 0usize;
    let mut first = None;
    for (i, (a, b)) in img.chunks(4).zip(want.chunks(4)).enumerate() {
        let d = a
            .iter()
            .zip(b)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0);
        if d > 1 {
            bad += 1;
            first.get_or_insert((i % img_w, i / img_w, d));
        }
    }
    if bad > 0 {
        write_png(&actual, &img, img_w as u32, img_h as u32);
        panic!(
            "glyph ladder golden: {bad} pixels differ (first {first:?}); actual at {}",
            actual.display()
        );
    }
}
