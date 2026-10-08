// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data check of the UI pass (`docs/specs/render.md` §6.15 invariant 1): render surfaces
//! decoded from the user's install are uploaded as UI canvases (B, G, R, A, straight alpha)
//! and composed at 1 : 1 over an opaque background. Every composed pixel must equal the
//! canvas where its alpha is 255, the background where it is 0, and straight source-over
//! within ±1 elsewhere. Skipped unless `TESSERA_GAME_DIR` is set; the install is opened
//! read-only and nothing derived from it is written anywhere. Only counts are printed.

#[path = "support/gpu.rs"]
mod gpu;
#[path = "ui_common/mod.rs"]
mod ui_common;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::{Record, Surface};
use tessera_render::{OffscreenFormat, RenderError, UiEntry, UiRect, UiViewMode};
use ui_common::{BG, render_ui};

const SURFACES: std::ops::Range<u32> = 0x4100_0000..0x4200_0000;
/// Distinct surfaces to compose.
const WANT: usize = 32;
/// Directory entries examined at most.
const MAX_ATTEMPTS: usize = 6000;
/// Largest side composed (keeps the test quick).
const MAX_SIDE: u32 = 512;

#[test]
fn real_surfaces_compose_exactly_at_one_to_one() -> Result<(), RenderError> {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR") else {
        eprintln!("skipping: TESSERA_GAME_DIR is not set");
        return Ok(());
    };
    let Some(mut r) = gpu::headless_renderer([0.0; 4]) else {
        return Ok(());
    };
    let set = ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set");
    let stored_view = r.ui_view_mode() == UiViewMode::Stored;
    let routed = set.candidates(SURFACES.start);
    let order: Vec<usize> = routed
        .iter()
        .copied()
        .chain((0..set.members().len()).filter(|i| !routed.contains(i)))
        .collect();
    let (mut attempts, mut composed, mut partial_alpha, mut checked) = (0usize, 0, 0usize, 0u64);
    'outer: for i in order {
        let Some(m) = set.get(i) else { continue };
        for entry in m.archive.entries_range(SURFACES) {
            let Ok(entry) = entry else { break };
            attempts += 1;
            if attempts > MAX_ATTEMPTS || composed >= WANT {
                break 'outer;
            }
            let Ok(bytes) = m.archive.read_entry(&entry) else {
                continue;
            };
            let Ok(image) = Surface::parse_expecting(&bytes, entry.did).and_then(|s| s.to_rgba8())
            else {
                continue;
            };
            let (w, h) = (image.width, image.height);
            if w > MAX_SIDE || h > MAX_SIDE {
                continue;
            }
            // Prefer canvases with transparency (UI art); take a few opaque ones too.
            let has_alpha = image.pixels.as_chunks::<4>().0.iter().any(|p| p[3] != 255);
            if !has_alpha && composed % 4 != 3 {
                continue;
            }
            let canvas: Vec<u8> = image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| [p[2], p[1], p[0], p[3]])
                .collect();
            let tex = r.create_ui_texture(w, h, None)?;
            r.update_texture_rect(tex, 0, 0, w, h, &canvas, w as usize * 4)?;
            for format in [OffscreenFormat::Rgba8Unorm, OffscreenFormat::Rgba8Srgb] {
                let blended_exact = format == OffscreenFormat::Rgba8Unorm || stored_view;
                let (img, stats) = render_ui(&mut r, (MAX_SIDE + 8, MAX_SIDE + 8), format, |p| {
                    p.entry(&UiEntry::new(tex, [w, h], UiRect::new(3, 5, w, h)));
                })?;
                assert_eq!(stats.ui.entries_refused, 0);
                for (k, c) in image.pixels.as_chunks::<4>().0.iter().enumerate() {
                    let (x, y) = (k as u32 % w, k as u32 / w);
                    let got = img.pixel(3 + x, 5 + y).expect("in bounds");
                    let ok = match c[3] {
                        255 => got == [c[0], c[1], c[2], 255],
                        0 => got == BG,
                        _ if blended_exact => {
                            partial_alpha += 1;
                            let a = f64::from(c[3]) / 255.0;
                            (0..3).all(|i| {
                                let want =
                                    (f64::from(c[i]) * a + f64::from(BG[i]) * (1.0 - a)).round();
                                (f64::from(got[i]) - want).abs() <= 1.0
                            })
                        }
                        _ => true,
                    };
                    assert!(
                        ok,
                        "surface #{composed} {format:?}: pixel ({x}, {y}) differs"
                    );
                    checked += 1;
                }
            }
            r.destroy_texture(tex);
            composed += 1;
        }
    }
    eprintln!(
        "composed {composed} real surfaces at 1:1 ({checked} pixels checked, {partial_alpha} \
         with partial alpha) after {attempts} directory entries"
    );
    assert!(composed > 0, "no decodable surface found");
    Ok(())
}
