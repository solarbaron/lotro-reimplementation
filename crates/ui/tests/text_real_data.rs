// SPDX-License-Identifier: GPL-3.0-or-later
//! Glyph atlases and text layout on a user's own install (read-only), skipped unless
//! `TESSERA_GAME_DIR` is set: every font record (prefix 0x42) is built into a
//! [`FontFace`] with its atlases (`fonts.md` §4), a synthetic string is looked up (§5),
//! laid out with wrapping (§7), drawn with the outline and an underline (§8) and painted
//! into a canvas. Checks: every font has `?` (FN4), every drawn box reads inside its atlas
//! (FN9), lines stay within the available width unless a single glyph is wider (§7.3), and
//! the text height follows FN16.
//!
//! Run with `TESSERA_GAME_DIR=… cargo test -p tessera-ui --release --test text_real_data
//! -- --nocapture`. Only counts, DIDs and timings are printed; nothing is written.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tessera_formats::{Font, Record};
use tessera_props::{Did, GameData};
use tessera_ui::paint::{CanvasSet, DisplayList, Rect, paint_rect};
use tessera_ui::text::{CharListBuilder, DrawParams, FontFace, LayoutParams, TextOp, draw, layout};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

/// A synthetic test string (our own words): Latin, Latin-1, Greek, Cyrillic, CJK, a
/// surrogate pair, a tab, a hyphenated word and a newline.
const SAMPLE: &str = "The quick brown fox jumps over 13 lazy dogs - well-known, isn't it? \
Ünïcödé çà ŝ αβγ жзи \u{4E00}\u{4E01}\u{3002} \u{1F600}\tend\nsecond line (x) A";

#[test]
fn every_font_builds_and_lays_out() {
    let Some(dir) = game_dir() else { return };
    let gd = GameData::open(&dir).expect("open install");
    let mut dids = BTreeSet::new();
    for member in gd.source().members() {
        for entry in member.archive.entries_range(0x4200_0000..0x4300_0000) {
            dids.insert(entry.expect("directory walk").did);
        }
    }
    assert!(!dids.is_empty(), "no font records");
    let read = |did: u32| gd.read(Did(did)).expect("read").expect("listed DID");

    let mut build_time = std::time::Duration::ZERO;
    let mut layout_time = std::time::Duration::ZERO;
    let mut atlas_bytes = 0usize;
    let mut glyph_boxes = 0usize;
    for &did in &dids {
        let payload = read(did);
        let font = Font::parse(&payload).unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        let fg = read(font.foreground.0);
        let bg = font.has_background().then(|| read(font.background.0));
        let t = Instant::now();
        let face = FontFace::from_payloads(&payload, &fg, bg.as_deref())
            .unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        build_time += t.elapsed();
        atlas_bytes += face.memory_bytes();
        assert!(!face.lookup(0x3F).substituted, "FN4: {did:#010x} has `?`");
        assert!(face.lookup(0x4E00).substituted, "no font has CJK (§10)");
        let face = Arc::new(face);
        let fonts = [Some(Arc::clone(&face)), Some(Arc::clone(&face))];

        let mut b = CharListBuilder::new();
        b.push_str(SAMPLE)
            .underline(Some(0xFFFF_FF00))
            .push_str(" underlined");
        let cs = b.finish();
        let p = LayoutParams {
            width: 200,
            height: 400,
            leading: 0.1,
            ..LayoutParams::default()
        };
        let t = Instant::now();
        let l = layout(&cs, &fonts, &p, None).expect("layout");
        let d = draw(
            &l,
            &cs,
            &fonts,
            &DrawParams {
                outline: true,
                ..DrawParams::default()
            },
        );
        layout_time += t.elapsed();

        // §7.3: a line wider than the available width holds a single glyph (no re-test).
        for line in &l.lines {
            let drawn = cs[line.start..line.end]
                .iter()
                .filter(|c| !c.is_newline())
                .count();
            assert!(line.width <= 200 || drawn <= 1, "{did:#010x}: {line:?}");
        }
        // FN16.
        let sum: i32 = l.lines.iter().map(|x| x.height + x.leading).sum();
        assert_eq!(l.text_height, sum);
        // FN9: every box reads inside its atlas.
        for op in &d.ops {
            if let TextOp::Glyphs(r) = op {
                let (aw, ah) = (r.coverage.width() as i32, r.coverage.height() as i32);
                for g in &r.glyphs {
                    glyph_boxes += 1;
                    assert!(g.src[0] >= 0 && g.src[1] >= 0, "{did:#010x}: {g:?}");
                    assert!(g.src[0] + g.size[0] as i32 <= aw, "{did:#010x}: {g:?}");
                    assert!(g.src[1] + g.size[1] as i32 <= ah, "{did:#010x}: {g:?}");
                }
            }
        }
        let mut list = DisplayList::new();
        d.push_to(&mut list, None).expect("display list");
        let mut set = CanvasSet::new(256, 512).unwrap();
        paint_rect(&list, &mut set, Rect::new(0, 0, 256, 512));
        let ink = set
            .below()
            .to_vec()
            .iter()
            .filter(|&&p| p >> 24 != 0)
            .count();
        assert!(ink > 0, "{did:#010x} painted nothing");
    }
    eprintln!(
        "{} fonts, {} KiB of faces, {} glyph boxes; build {:?}, layout+draw {:?} (indicative)",
        dids.len(),
        atlas_bytes / 1024,
        glyph_boxes,
        build_time,
        layout_time
    );
}
