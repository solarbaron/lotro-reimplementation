// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks of the explorer's data layer against the owner's install.
//!
//! Skipped unless `TESSERA_GAME_DIR` is set. The install is only read. Results are
//! printed as counts (and DIDs useful for screenshots), never as game content. Set
//! `TESSERA_EXPLORER_FULL=1` to check every record of each family instead of a sample.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};

use tessera_assets::JobContext;
use tessera_explorer::ExplorerError;
use tessera_explorer::data::DataContext;
use tessera_explorer::decoders::{
    AUTO_LEVEL, IconDecoder, NameIndexDecoder, TextureDecoder, TextureRequest, build_name_index,
    flatten_props, icon_stacks, load_property_set, read_raw, record_product, string_rows,
};
use tessera_explorer::image_ops::ChannelMask;
use tessera_explorer::index::{ArchiveIndex, index_archive};
use tessera_explorer::query::parse_query;
use tessera_explorer::search::run_search;

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("TESSERA_GAME_DIR").map(PathBuf::from);
    if dir.is_none() {
        eprintln!("TESSERA_GAME_DIR not set; skipping real-data test");
    }
    dir
}

fn full() -> bool {
    std::env::var_os("TESSERA_EXPLORER_FULL").is_some_and(|v| v == "1")
}

/// Every `step`-th DID of `prefix` across the indexes (all of them in full mode).
fn sample(indexes: &[Arc<ArchiveIndex>], prefix: u8, step: usize) -> Vec<u32> {
    let lo = u32::from(prefix) << 24;
    let hi = lo | 0x00FF_FFFF;
    let step = if full() { 1 } else { step.max(1) };
    let mut out: Vec<u32> = indexes
        .iter()
        .flat_map(|i| {
            let r = i.range_of(lo..=hi);
            i.entries[r].iter().map(|e| e.did).collect::<Vec<_>>()
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out.into_iter().step_by(step).collect()
}

#[test]
fn explorer_over_the_real_install() {
    let Some(dir) = game_dir() else { return };
    let ctx = Arc::new(DataContext::open(&dir, None).expect("open install"));
    let job = JobContext::detached();
    let no = AtomicBool::new(false);

    // Archive tree.
    let started = std::time::Instant::now();
    let mut indexes = Vec::new();
    for (i, m) in ctx.set.members().iter().enumerate() {
        let idx = index_archive(&m.archive, i, m.id.to_string(), &AtomicU64::new(0), &no);
        assert!(idx.error.is_none(), "{}: {:?}", idx.name, idx.error);
        assert!(!idx.truncated);
        assert!(idx.entries.windows(2).all(|w| w[0].did < w[1].did));
        indexes.push(Arc::new(idx));
    }
    let total: usize = indexes.iter().map(|i| i.entries.len()).sum();
    let groups: usize = indexes.iter().map(|i| i.groups.len()).sum();
    eprintln!(
        "index: {} archives, {total} entries, {groups} type groups, {} ms",
        indexes.len(),
        started.elapsed().as_millis()
    );
    assert!(total > 0);

    // Search.
    let r = run_search("0x25", &parse_query("0x25"), &indexes, None, true, &no);
    eprintln!("search 0x25: {} hits", r.hits.len());
    assert!(!r.hits.is_empty());

    // Names (DID-map labels and property names).
    let names = build_name_index(&ctx, &job);
    eprintln!(
        "name index: {} names, {} problems",
        names.entries.len(),
        names.problems.len()
    );
    assert!(!names.entries.is_empty());
    let _ = NameIndexDecoder(Arc::clone(&ctx));

    // Root DID map as a record table.
    let raw = read_raw(&ctx, 0x2800_0000, None).expect("root DID map");
    let rec = record_product(&ctx, 0x2800_0000, &raw.bytes).expect("root DID map decodes");
    eprintln!("root DID map: {} rows", rec.rows.len());
    assert!(!rec.rows.is_empty());

    // String tables: every entry renders in explorer mode.
    let strings = ctx.strings.as_ref().expect("language archive");
    let tables = sample(&indexes, 0x25, 100);
    let (mut rows, mut placeholders, mut notes, mut failed) = (0usize, 0usize, 0usize, 0usize);
    let mut with_vars = None;
    for &did in &tables {
        match strings.table(did) {
            Ok(Some(t)) => {
                let r = string_rows(&t, &job).expect("rows");
                rows += r.len();
                for row in &r {
                    if row.text.spans().iter().any(|s| s.style.placeholder) {
                        placeholders += 1;
                        with_vars.get_or_insert(did);
                    }
                    notes += usize::from(!row.notes.is_empty());
                }
            }
            Ok(None) => {}
            Err(e) => {
                failed += 1;
                eprintln!("table {did:#010x}: {e}");
            }
        }
    }
    eprintln!(
        "strings: {} tables, {rows} entries rendered, {placeholders} with placeholders, \
         {notes} with notes, {failed} failed; e.g. {:?}",
        tables.len(),
        with_vars.map(|d| format!("{d:#010x}"))
    );
    assert_eq!(failed, 0);

    // Textures: lists and surfaces.
    let tex = TextureDecoder::new(Arc::clone(&ctx));
    let auto = TextureRequest {
        level: AUTO_LEVEL,
        mask: ChannelMask::RGBA,
    };
    let lists = sample(&indexes, 0x40, 400);
    let (mut shown, mut unavailable, mut errors) = (0usize, 0usize, 0usize);
    for &did in &lists {
        match tex.texture(did, auto, &job) {
            Ok(p) if p.image.is_some() => shown += 1,
            Ok(_) => unavailable += 1,
            Err(e) => {
                errors += 1;
                eprintln!("list {did:#010x}: {e}");
            }
        }
    }
    eprintln!(
        "texture lists: {} sampled, {shown} shown, {unavailable} unavailable, {errors} errors",
        lists.len()
    );
    assert_eq!(errors, 0);
    let surfaces = sample(&indexes, 0x41, 2000);
    let mut by_format = std::collections::BTreeMap::<String, (usize, usize)>::new();
    for &did in &surfaces {
        match tex.texture(did, auto, &job) {
            Ok(p) => {
                let f = p
                    .shown
                    .and_then(|s| p.levels[s].header)
                    .map_or_else(|| "?".to_owned(), |h| h.format.name().to_owned());
                let e = by_format.entry(f).or_default();
                if p.image.is_some() {
                    e.0 += 1;
                } else {
                    e.1 += 1;
                }
            }
            Err(e) => {
                by_format.entry("error".into()).or_default().1 += 1;
                eprintln!("surface {did:#010x}: {e}");
            }
        }
    }
    eprintln!(
        "surfaces: {} sampled, by format (ok, failed): {by_format:?}",
        surfaces.len()
    );
    assert!(!by_format.contains_key("error"));

    // Property sets (every property-set prefix, `texture-list.md` §6.1): decode,
    // resolve, flatten; icons. M1 exit criterion 3: at least 99.5% decode.
    let registry = ctx.game.registry().expect("property registry");
    let mut icon_sets = Vec::new();
    let (mut all_sets, mut all_failed) = (0usize, 0usize);
    for prefix in 0x78..=0x7Fu8 {
        let sets = sample(&indexes, prefix, 400);
        if sets.is_empty() {
            continue;
        }
        let (mut ok, mut trailing, mut perr, mut prow) = (0usize, 0usize, 0usize, 0usize);
        for &did in &sets {
            match load_property_set(&ctx, did) {
                Ok((_, set, t)) => {
                    let resolved = ctx.game.resolve(&set).expect("resolve");
                    let (rows, _) = flatten_props(&resolved, ctx.strings.as_ref(), 250_000);
                    prow += rows.len();
                    ok += 1;
                    trailing += usize::from(t > 0);
                    if icon_sets.len() < 3
                        && icon_stacks(&set, registry)
                            .iter()
                            .any(|(f, _)| *f == "item")
                    {
                        icon_sets.push(did);
                    }
                }
                Err(e) => {
                    perr += 1;
                    eprintln!("set {did:#010x}: {e}");
                }
            }
        }
        eprintln!(
            "property sets {prefix:02X}: {} sampled, {ok} decoded ({prow} rows), \
             {trailing} with trailing bytes, {perr} failed",
            sets.len()
        );
        all_sets += sets.len();
        all_failed += perr;
    }
    eprintln!(
        "property sets: {all_sets} sampled, {all_failed} failed; item icons e.g. {:?}",
        icon_sets
            .iter()
            .map(|d| format!("{d:#010x}"))
            .collect::<Vec<_>>()
    );
    assert!(all_sets > 0);
    assert!(
        all_failed * 1000 <= all_sets * 5,
        "{all_failed} of {all_sets} property sets failed (over 0.5%)"
    );

    // Objects (0x70–0x77) open their property set at +0x09000000.
    let (mut objects, mut with_set, mut without, mut obj_failed) = (0usize, 0, 0, 0);
    for prefix in 0x70..=0x77u8 {
        for did in sample(&indexes, prefix, 400) {
            objects += 1;
            match load_property_set(&ctx, did) {
                Ok((set_did, _, _)) => {
                    assert_eq!(set_did, did.wrapping_add(0x0900_0000));
                    with_set += 1;
                }
                Err(ExplorerError::NotFound(_) | ExplorerError::NotApplicable(_)) => {
                    without += 1;
                }
                Err(e) => {
                    obj_failed += 1;
                    eprintln!("object {did:#010x}: {e}");
                }
            }
        }
    }
    eprintln!(
        "objects: {objects} sampled, {with_set} with a property set, {without} without, \
         {obj_failed} failed"
    );
    assert!(obj_failed * 1000 <= objects.max(1) * 5);
    let icons = IconDecoder(Arc::clone(&ctx));
    for &did in &icon_sets {
        let p =
            tessera_assets::Decoder::decode(&icons, did.into(), Vec::new(), &job).expect("icon");
        let drawn = p.composites.iter().filter(|c| c.image.is_some()).count();
        eprintln!(
            "icon {did:#010x}: {} families, {drawn} drawn",
            p.composites.len()
        );
    }
}
