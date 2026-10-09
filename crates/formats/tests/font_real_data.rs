// SPDX-License-Identifier: GPL-3.0-or-later
//! `fonts.md` §3, §4, §10 and §12 against a user's own install (read-only).
//!
//! Decodes **every** record with prefix 0x42, checks FN1–FN11 (FN8–FN11 with the glyph
//! atlases) and reports the §10 census. The exact counts and per-glyph field facts of §3.3
//! and the counts of §4.3 and §10 are asserted when the install has the spec's 48 records
//! with its 28,315 glyphs; otherwise they are only printed and the invariants still apply. FN12 (font DIDs named by layouts) is checked by
//! `ui_layout_real_data.rs`.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-formats --test font_real_data -- --nocapture`.
//! Only counts, DIDs and error kinds are printed; no game data leaves the install.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::font::{AtlasIssue, QUESTION_MARK, SPACE};
use tessera_formats::{Did, Font, Record, Surface, SurfaceFormat};

const SPEC_RECORDS: usize = 48;
const SPEC_GLYPHS: usize = 28_315;
/// Category of glyph-atlas surfaces (§4.1).
const ATLAS_CATEGORY: u32 = 13;

fn open_set() -> Option<ArchiveSet> {
    let dir = match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return None;
        }
    };
    Some(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"))
}

/// An atlas: width, height and coverage bytes (rows top first).
struct Atlas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// Reads a glyph atlas and checks FN8's format and category.
fn atlas(set: &ArchiveSet, did: Did) -> Atlas {
    let payload = set
        .read(did.0)
        .unwrap_or_else(|e| panic!("FN8: atlas {did}: {e}"));
    let s = Surface::parse_expecting(&payload, did).expect("atlas surface");
    assert_eq!(s.format, SurfaceFormat::A8, "FN8: {did} format");
    assert_eq!(s.category, ATLAS_CATEGORY, "FN8: {did} category");
    Atlas {
        width: s.width,
        height: s.height,
        pixels: s.data.to_vec(),
    }
}

/// FN10: whether the glyphs were packed by the §4.2 rule into an atlas of this size.
fn packed_by_rule(f: &Font, width: u32, height: u32) -> bool {
    let (bx, by) = (f.border_x, f.border_y);
    let mut x = bx;
    let mut y = by;
    let mut row_h = 0u32;
    for (i, g) in f.glyphs().iter().enumerate() {
        let w = u32::from(g.width);
        if i > 0 && x + w + bx > width {
            y += row_h + 2 * by;
            x = bx;
            row_h = 0;
        }
        if u32::from(g.x) != x || u32::from(g.y) != y {
            return false;
        }
        row_h = row_h.max(u32::from(g.height));
        x += w + 2 * bx;
    }
    y + row_h + by == height
}

#[derive(Default)]
struct Ink {
    no_ink: BTreeMap<u16, u64>,
    tight: u64,
    outside_bordered: u64,
    outside_plain: u64,
}

/// Ink census of a foreground atlas (§4.3, FN11).
fn ink(f: &Font, a: &Atlas, out: &mut Ink) {
    let w = a.width as usize;
    let px = |x: usize, y: usize| a.pixels.get(y * w + x).copied().unwrap_or(0);
    let mut plain = vec![false; a.pixels.len()];
    let mut bordered = vec![false; a.pixels.len()];
    for g in f.glyphs() {
        let (gx, gy, gw, gh) = g.atlas_box();
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, 0, 0);
        for y in gy..gy + gh {
            for x in gx..gx + gw {
                if let Some(m) = plain.get_mut(y as usize * w + x as usize) {
                    *m = true;
                }
                if px(x as usize, y as usize) != 0 {
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }
        if min_x == u32::MAX {
            *out.no_ink.entry(g.code).or_default() += 1;
        } else if (min_x, min_y, max_x + 1, max_y + 1) == (gx, gy, gx + gw, gy + gh) {
            out.tight += 1;
        }
        let (bx, by, bw, bh) = g.bordered_box(f.border_x, f.border_y);
        for y in by.max(0)..(by + bh).min(i64::from(a.height)) {
            for x in bx.max(0)..(bx + bw).min(i64::from(a.width)) {
                bordered[y as usize * w + x as usize] = true;
            }
        }
    }
    let mut outside_b = false;
    let mut outside_p = false;
    for (i, &v) in a.pixels.iter().enumerate() {
        if v != 0 {
            outside_b |= !bordered[i];
            outside_p |= !plain[i];
        }
    }
    out.outside_bordered += u64::from(outside_b);
    out.outside_plain += u64::from(outside_p);
}

#[test]
fn every_font_decodes_with_the_census() {
    let Some(set) = open_set() else { return };

    // §3.1: where the records are; no DID twice.
    let mut per_archive: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    let mut dids = BTreeSet::new();
    let mut duplicates = 0;
    for m in set.members() {
        for e in m.archive.entries_range(0x4200_0000..=0x42FF_FFFF) {
            let e = e.expect("directory walk");
            per_archive.entry(m.id.file_name()).or_default().push(e.did);
            if !dids.insert(e.did) {
                duplicates += 1;
            }
        }
    }
    let counts: BTreeMap<&String, usize> = per_archive.iter().map(|(k, v)| (k, v.len())).collect();
    println!("font records per archive: {counts:?}");

    let mut fonts = Vec::new();
    let mut failures: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    let mut sizes: BTreeMap<usize, u64> = BTreeMap::new();
    for &did in &dids {
        let payload = set.read(did).expect("read font");
        match Font::parse_expecting(&payload, did) {
            Ok(f) => {
                *sizes.entry(payload.len()).or_default() += 1;
                fonts.push(f);
            }
            Err(e) => failures.entry(format!("{e}")).or_default().push(did),
        }
    }

    let mut issues: BTreeMap<&'static str, Vec<Did>> = BTreeMap::new();
    let mut atlas_problems: Vec<(Did, AtlasIssue)> = Vec::new();
    let mut not_packed = Vec::new();
    let mut ink_census = Ink::default();
    let mut families: BTreeMap<usize, u64> = BTreeMap::new();
    let mut borders: BTreeMap<u32, u64> = BTreeMap::new();
    let mut glyphs = 0usize;
    let mut no_background = Vec::new();
    let mut unwanted_codes = 0u64;
    for f in &fonts {
        for i in f.issues() {
            issues.entry(i.name()).or_default().push(f.did);
        }
        glyphs += f.len();
        *families.entry(f.len()).or_default() += 1;
        *borders.entry(f.border_x).or_default() += 1;
        assert!(
            f.glyph(SPACE).is_some() && f.glyph(QUESTION_MARK).is_some(),
            "FN4"
        );
        unwanted_codes += f
            .glyphs()
            .iter()
            .filter(|g| {
                matches!(g.code, 0xFFFD | 0x0009 | 0x3040..=0x30FF | 0x4E00..=0x9FFF | 0xAC00..=0xD7A3)
            })
            .count() as u64;
        let fg = atlas(&set, f.foreground);
        if f.has_background() {
            let bg = atlas(&set, f.background);
            assert_eq!(
                (bg.width, bg.height),
                (fg.width, fg.height),
                "FN8: {}",
                f.did
            );
        } else {
            no_background.push(f.did);
        }
        for i in f.atlas_issues(fg.width, fg.height, 8) {
            atlas_problems.push((f.did, i));
        }
        if !packed_by_rule(f, fg.width, fg.height) {
            not_packed.push(f.did);
        }
        ink(f, &fg, &mut ink_census);
    }

    println!(
        "fonts: {} records, {} decode, {glyphs} glyphs, {duplicates} duplicate DIDs",
        dids.len(),
        fonts.len()
    );
    println!("decode failures: {failures:?}");
    println!("sizes: {sizes:?}");
    println!("glyph counts (families): {families:?}");
    println!("borders: {borders:?}; without background atlas: {no_background:?}");
    println!("§3.3 / §12 issues: {issues:?}");
    println!("FN9 atlas issues: {atlas_problems:?}");
    println!("FN10 not packed by §4.2: {not_packed:?}");
    println!(
        "§4.3 ink: no ink {:?}, tight {}, fonts with ink outside bordered boxes {}, outside plain boxes {}",
        ink_census.no_ink, ink_census.tight, ink_census.outside_bordered, ink_census.outside_plain
    );
    println!("§10 unwanted codes (U+FFFD, tab, Kana, CJK, Hangul): {unwanted_codes}");

    // Invariants (any install).
    assert!(failures.is_empty(), "FN1/FN2: {failures:?}");
    assert_eq!(duplicates, 0, "§3.1: no DID twice");
    assert!(issues.is_empty(), "FN3-FN7: {issues:?}");
    assert!(atlas_problems.is_empty(), "FN9: {atlas_problems:?}");
    assert!(not_packed.is_empty(), "FN10: {not_packed:?}");
    assert_eq!(ink_census.outside_bordered, 0, "FN11");

    if fonts.len() != SPEC_RECORDS || glyphs != SPEC_GLYPHS {
        println!("install differs from the spec's census build; exact counts not asserted");
        return;
    }
    // §3.1.
    let general: Vec<u32> = per_archive
        .iter()
        .filter(|(k, _)| k.contains("general"))
        .flat_map(|(_, v)| v.iter().copied())
        .collect();
    assert_eq!(general, vec![0x4200_0027, 0x4200_0028, 0x4200_00FD], "§3.1");
    // §3.2 sizes and §10 families.
    assert_eq!(
        sizes,
        BTreeMap::from([
            (2_137, 2),
            (2_500, 1),
            (5_503, 19),
            (7_417, 17),
            (7_516, 8),
            (15_645, 1)
        ]),
        "§3.2 record sizes"
    );
    assert_eq!(
        families,
        BTreeMap::from([
            (191, 2),
            (224, 1),
            (497, 19),
            (671, 17),
            (680, 8),
            (1_419, 1)
        ]),
        "§10 families"
    );
    // §3.3 borders, §4.3.
    assert_eq!(
        borders,
        BTreeMap::from([(0, 1), (1, 4), (2, 3), (3, 8), (4, 32)]),
        "§3.3 borders"
    );
    assert_eq!(no_background, vec![Did(0x4200_002A)], "§4.3");
    assert_eq!(
        ink_census.no_ink,
        BTreeMap::from([
            (0x0020, 48),
            (0x00A0, 48),
            (0x00AF, 1),
            (0xF000, 17),
            (0xF00D, 8)
        ]),
        "§4.3 glyphs without ink"
    );
    assert_eq!(ink_census.tight, 27_563, "§4.3 tight boxes");
    assert_eq!(
        ink_census.outside_plain, 1,
        "FN11: one font inks its border ring"
    );
    assert_eq!(unwanted_codes, 0, "§10");

    // §3.3 per-glyph field facts. A decoder that swapped A, B or T would still pass FN5.
    let all = || fonts.iter().flat_map(|f| f.glyphs().iter());
    let range = |v: Vec<i32>| (*v.iter().min().unwrap(), *v.iter().max().unwrap());
    assert_eq!(
        range(all().map(|g| i32::from(g.left_bearing)).collect()),
        (-7, 12),
        "§3.3 A"
    );
    assert_eq!(
        range(all().map(|g| i32::from(g.right_bearing)).collect()),
        (-17, 11),
        "§3.3 B"
    );
    assert_eq!(
        range(all().map(|g| i32::from(g.height)).collect()),
        (1, 36),
        "§3.3 H"
    );
    assert_eq!(
        range(all().map(|g| i32::from(g.width)).collect()),
        (0, 41),
        "§3.3 W"
    );
    let (min_advance, max_advance) = range(all().map(|g| g.advance()).collect());
    assert!(min_advance >= 0 && max_advance == 37, "§3.3 advance");
    let zero_width: Vec<(Did, u16)> = fonts
        .iter()
        .flat_map(|f| {
            f.glyphs()
                .iter()
                .filter(|g| g.width == 0)
                .map(move |g| (f.did, g.code))
        })
        .collect();
    assert_eq!(zero_width.len(), 8, "§3.3 W = 0 in eight records");
    assert!(
        zero_width.iter().all(|&(_, code)| code == 0xF00D),
        "§3.3 W = 0 only for U+F00D: {zero_width:?}"
    );
    let zero_width_fonts: BTreeSet<Did> = zero_width.iter().map(|&(d, _)| d).collect();
    assert_eq!(zero_width_fonts.len(), 8, "§3.3 one per Verdana record");
    for f in &fonts {
        let space = f.glyph(SPACE).expect("FN4");
        assert_eq!(
            (
                u32::from(space.height),
                space.top,
                space.left_bearing,
                space.right_bearing
            ),
            (f.line_height, 0, 0, 1),
            "§3.3 space glyph of {}",
            f.did
        );
    }
}
