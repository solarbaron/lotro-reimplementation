// SPDX-License-Identifier: GPL-3.0-or-later
//! The explorer's data layer over a synthetic install: archives written by the synthetic
//! archive writer of `tessera-dat`'s own tests (included by path, not copied), holding
//! records generated here from the layouts in `docs/specs/`. No game data is involved.

#[path = "../../../crates/dat/tests/common/mod.rs"]
mod datkit;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use eframe::egui::Color32;
use flate2::Compression;
use tessera_assets::{
    AssetKey, AssetState, AssetStore, Handle, JobContext, PoolConfig, Priority, StoreConfig,
    WorkerPool,
};
use tessera_dat::{ArchiveSet, SetOptions};
use tessera_explorer::data::DataContext;
use tessera_explorer::decoders::{
    AUTO_LEVEL, ContextSource, IconDecoder, NameIndexDecoder, PropsDecoder, RawDecoder,
    RecordDecoder, StringsDecoder, TextureDecoder, TextureRequest, read_raw, record_product,
};
use tessera_explorer::image_ops::ChannelMask;
use tessera_explorer::index::{Indexer, index_archive};
use tessera_explorer::query::parse_query;
use tessera_explorer::search::run_search;
use tessera_formats::name_hash;

const T: Duration = Duration::from_secs(20);

fn u32le(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

/// A render surface (`records.md` §5): DID, category, width, height, format, length, data.
fn surface(did: u32, w: u32, h: u32, format: u32, data: &[u8]) -> Vec<u8> {
    let mut r = Vec::new();
    for v in [did, 15, w, h, format, data.len() as u32] {
        r.extend_from_slice(&u32le(v));
    }
    r.extend_from_slice(data);
    r
}

/// A kind-2 texture list (`texture-list.md` §2.2).
fn texture_list(did: u32, levels: &[u32]) -> Vec<u8> {
    let mut r = Vec::new();
    r.extend_from_slice(&u32le(did));
    r.push(2);
    r.push(levels.len() as u8);
    r.push(0);
    r.extend_from_slice(&[0, 0]);
    for l in levels {
        r.extend_from_slice(&u32le(*l));
    }
    r.extend_from_slice(&u32le(0));
    r.extend_from_slice(&u32le(15));
    r
}

/// A 1-byte VLE (`records.md` §1.2).
fn vle(n: usize) -> u8 {
    assert!(n < 0x80);
    n as u8
}

/// A UTF-16 string (`records.md` §1.4).
fn utf16(s: &str) -> Vec<u8> {
    let units: Vec<u16> = s.encode_utf16().collect();
    let mut r = vec![vle(units.len())];
    for u in units {
        r.extend_from_slice(&u.to_le_bytes());
    }
    r
}

type Entry<'a> = (u32, Vec<&'a str>, Vec<u32>);

/// A string table (`records.md` §3) with entries (token, pieces, argument IDs).
fn string_table(did: u32, entries: &[Entry<'_>]) -> Vec<u8> {
    let mut r = Vec::new();
    r.extend_from_slice(&u32le(did));
    r.extend_from_slice(&u32le(1));
    // TSize: hint 0 (11 buckets), count; entries in bucket order (§1.3.1).
    r.push(0);
    r.push(vle(entries.len()));
    let mut sorted: Vec<&Entry<'_>> = entries.iter().collect();
    sorted.sort_by_key(|e| e.0 % 11);
    for (token, pieces, args) in sorted {
        r.extend_from_slice(&u32le(*token));
        r.extend_from_slice(&u32le(0));
        r.extend_from_slice(&u32le(pieces.len() as u32));
        for p in pieces {
            r.extend_from_slice(&utf16(p));
        }
        r.extend_from_slice(&u32le(args.len() as u32));
        for a in args {
            r.extend_from_slice(&u32le(*a));
        }
        r.push(0);
    }
    r
}

const RGBA_SURFACE: u32 = 0x4100_0010;
const A8_SURFACE: u32 = 0x4100_0011;
const MISSING_SURFACE: u32 = 0x4100_0012;
const BAD_SURFACE: u32 = 0x4100_0013;
/// 0×0 A8R8G8B8 with no data: parses, but has nothing to show.
const EMPTY_SURFACE: u32 = 0x4100_0014;
/// 0×5000 A8R8G8B8 with no data.
const THIN_SURFACE: u32 = 0x4100_0015;
const LIST: u32 = 0x4000_0001;
const TABLE: u32 = 0x2500_0001;

struct Install {
    _dir: datkit::TempDir,
    ctx: Arc<DataContext>,
}

fn install() -> Install {
    let dir = datkit::TempDir::new("explorer");
    let mut g = datkit::Builder::new();
    // A8R8G8B8 is stored B, G, R, A: red, half-transparent green, clear blue, white.
    let bgra = [
        0, 0, 255, 255, 0, 255, 0, 128, 255, 0, 0, 0, 255, 255, 255, 255,
    ];
    g.compressed(
        RGBA_SURFACE,
        &surface(RGBA_SURFACE, 2, 2, 21, &bgra),
        Compression::fast(),
    );
    g.stored(
        A8_SURFACE,
        surface(A8_SURFACE, 2, 2, 28, &[0, 85, 170, 255]),
    );
    // Declares 4×4 A8R8G8B8 but carries 3 bytes: must fail to decode, not panic.
    g.stored(BAD_SURFACE, surface(BAD_SURFACE, 4, 4, 21, &[1, 2, 3]));
    g.stored(EMPTY_SURFACE, surface(EMPTY_SURFACE, 0, 0, 21, &[]));
    g.stored(THIN_SURFACE, surface(THIN_SURFACE, 0, 5000, 21, &[]));
    // Level 0 is missing, as without the optional high-resolution archives.
    g.stored(LIST, texture_list(LIST, &[MISSING_SURFACE, RGBA_SURFACE]));
    std::fs::write(dir.0.join("client_general.dat"), &g.build().bytes).unwrap();
    let mut l = datkit::Builder::new();
    let player = name_hash(b"PLAYER");
    l.stored(
        TABLE,
        string_table(
            TABLE,
            &[
                (5, vec!["Hello ", "!"], vec![player]),
                (6, vec!["<rgb=#FF0000>red</rgb> text"], vec![]),
            ],
        ),
    );
    std::fs::write(dir.0.join("client_local_English.dat"), &l.build().bytes).unwrap();
    let set = Arc::new(ArchiveSet::open(&dir.0, SetOptions::default()).unwrap());
    let ctx = Arc::new(DataContext::from_set(dir.0.clone(), set, None));
    Install { _dir: dir, ctx }
}

fn req(level: u8, mask: ChannelMask) -> TextureRequest {
    TextureRequest { level, mask }
}

#[test]
fn context_and_indexes() {
    let inst = install();
    let ctx = &inst.ctx;
    assert_eq!(ctx.set.members().len(), 2);
    let local = ctx.local_member.expect("language archive selected");
    assert_eq!(ctx.member_name(local), "local_English");
    assert!(ctx.strings.is_some());
    let no = AtomicBool::new(false);
    let mut indexes = Vec::new();
    for (i, m) in ctx.set.members().iter().enumerate() {
        let scanned = AtomicU64::new(0);
        let idx = index_archive(&m.archive, i, m.id.to_string(), &scanned, &no);
        assert!(idx.error.is_none());
        assert_eq!(scanned.load(Ordering::Relaxed) as usize, idx.entries.len());
        indexes.push(Arc::new(idx));
    }
    let general = indexes.iter().find(|i| i.name == "general").unwrap();
    let prefixes: Vec<u8> = general.groups.iter().map(|g| g.prefix).collect();
    assert!(prefixes.contains(&0x40) && prefixes.contains(&0x41));
    let r = run_search("0x41", &parse_query("0x41"), &indexes, None, true, &no);
    let dids: Vec<u32> = r.dids().collect();
    assert_eq!(
        dids,
        vec![
            RGBA_SURFACE,
            A8_SURFACE,
            BAD_SURFACE,
            EMPTY_SURFACE,
            THIN_SURFACE
        ]
    );
    // A cancelled walk stops early and says so.
    let yes = AtomicBool::new(true);
    let m = &ctx.set.members()[0];
    let idx = index_archive(&m.archive, 0, "x".into(), &AtomicU64::new(0), &yes);
    assert!(idx.error.is_some());
}

#[test]
fn decoders_directly() {
    let inst = install();
    let ctx = &inst.ctx;
    let job = JobContext::detached();

    let raw = read_raw(ctx, LIST, None).unwrap();
    assert_eq!(raw.archive, "general");
    assert_eq!(raw.bytes.len(), 17 + 8);
    assert!(read_raw(ctx, 0x4100_7777, None).is_err());

    // Texture list: level 0 is missing, so the effective chain starts at level 1.
    let tex = TextureDecoder::new(Arc::clone(ctx));
    let p = tex
        .texture(LIST, req(AUTO_LEVEL, ChannelMask::RGBA), &job)
        .unwrap();
    assert_eq!(p.levels.len(), 2);
    assert!(p.levels[0].header.is_none());
    assert_eq!(p.effective, vec![1]);
    assert_eq!(p.shown, Some(1));
    let img = p.image.as_ref().unwrap();
    assert_eq!(img.size, [2, 2]);
    assert_eq!(img.pixels[0], Color32::from_rgb(255, 0, 0));
    assert_eq!(img.pixels[2].a(), 0);
    assert!(!p.stats.unwrap().opaque());
    // Asking for the missing level falls back to the effective chain.
    let p = tex.texture(LIST, req(0, ChannelMask::RGBA), &job).unwrap();
    assert_eq!(p.shown, Some(1));

    // Green only: greyscale of G.
    let p = tex
        .texture(RGBA_SURFACE, req(0, ChannelMask::from_bits(2)), &job)
        .unwrap();
    let img = p.image.unwrap();
    assert_eq!(img.pixels[1], Color32::from_gray(255));
    assert_eq!(img.pixels[0], Color32::from_gray(0));

    // A8 as grey.
    let p = tex
        .texture(A8_SURFACE, req(AUTO_LEVEL, ChannelMask::ALPHA), &job)
        .unwrap();
    assert_eq!(p.image.unwrap().pixels[1], Color32::from_gray(85));

    // A broken surface and a non-texture DID are errors, not panics.
    assert!(
        tex.texture(BAD_SURFACE, req(AUTO_LEVEL, ChannelMask::RGBA), &job)
            .is_err()
    );
    assert!(tex.texture(TABLE, req(0, ChannelMask::RGBA), &job).is_err());

    // Regression: a zero-sized surface gave an empty image, which the GPU rejects
    // (the explorer aborted). It now shows its header and an error instead.
    for did in [EMPTY_SURFACE, THIN_SURFACE] {
        let p = tex
            .texture(did, req(AUTO_LEVEL, ChannelMask::RGBA), &job)
            .unwrap();
        assert!(p.image.is_none(), "{did:#010x}");
        assert!(
            p.error.as_deref().unwrap().contains("zero size"),
            "{did:#010x}"
        );
        assert!(p.levels[0].header.is_some());
    }

    // Record view of the texture list links its levels.
    let rec = record_product(ctx, LIST, &raw.bytes).unwrap();
    assert_eq!(rec.rows.len(), 2);
    assert_eq!(rec.rows[1].link, Some(RGBA_SURFACE));
    let rec = record_product(ctx, 0x0600_0001, &[1, 2, 3]).unwrap();
    assert!(rec.rows.is_empty());
    assert!(record_product(ctx, LIST, &raw.bytes[..5]).is_err());
}

fn wait<T>(h: &Handle<T>) -> AssetState<T> {
    h.wait(T)
}

#[test]
fn stores_on_workers() {
    let inst = install();
    let ctx = &inst.ctx;
    let pool = WorkerPool::new(PoolConfig {
        workers: 2,
        ..PoolConfig::default()
    });
    let cfg = StoreConfig::default();

    let strings = AssetStore::new(
        &pool,
        ContextSource,
        StringsDecoder(Arc::clone(ctx)),
        cfg.clone(),
    );
    let h = strings.request(TABLE, Priority::Immediate);
    let AssetState::Ready(p) = wait(&h) else {
        panic!("strings: {:?}", h.state())
    };
    assert_eq!(p.rows.len(), 2);
    let hello = p.rows.iter().find(|r| r.token == 5).unwrap();
    // Explorer mode: the missing value shows as a `${…}` placeholder.
    let text = hello.text.plain_text();
    assert!(
        text.starts_with("Hello ${") && text.ends_with("}!"),
        "{text}"
    );
    assert!(hello.text.spans().iter().any(|s| s.style.placeholder));
    assert_eq!(hello.slots, 1);
    let red = p.rows.iter().find(|r| r.token == 6).unwrap();
    assert_eq!(red.text.plain_text(), "red text");
    assert!(red.text.spans().iter().any(|s| s.style.color.is_some()));
    let h = strings.request(LIST, Priority::Immediate);
    assert!(matches!(wait(&h), AssetState::Failed(_)));

    // Variant n reads from member n − 1 only.
    let raw = AssetStore::new(
        &pool,
        ContextSource,
        RawDecoder(Arc::clone(ctx)),
        cfg.clone(),
    );
    let general = ctx
        .set
        .members()
        .iter()
        .position(|m| m.id.to_string() == "general")
        .unwrap();
    let h = raw.request(AssetKey::new(LIST, general as u32 + 1), Priority::Visible);
    assert!(matches!(wait(&h), AssetState::Ready(_)));
    let local = ctx.local_member.unwrap();
    let h = raw.request(AssetKey::new(LIST, local as u32 + 1), Priority::Visible);
    assert!(matches!(wait(&h), AssetState::Failed(_)));

    // Without a property dictionary, properties fail and names come back empty, without
    // panicking; surfaces still compose as single-layer icons.
    let props = AssetStore::new(
        &pool,
        ContextSource,
        PropsDecoder(Arc::clone(ctx)),
        cfg.clone(),
    );
    let h = props.request(0x7900_0001, Priority::Visible);
    assert!(matches!(wait(&h), AssetState::Failed(_)));
    let h = props.request(RGBA_SURFACE, Priority::Visible);
    assert!(matches!(wait(&h), AssetState::Failed(_)));
    let icons = AssetStore::new(
        &pool,
        ContextSource,
        IconDecoder(Arc::clone(ctx)),
        cfg.clone(),
    );
    let h = icons.request(RGBA_SURFACE, Priority::Visible);
    let AssetState::Ready(icon) = wait(&h) else {
        panic!("icon: {:?}", h.state())
    };
    assert_eq!(icon.composites.len(), 1);
    assert_eq!(icon.composites[0].size, Some((2, 2)));
    let h = icons.request(BAD_SURFACE, Priority::Visible);
    let AssetState::Ready(icon) = wait(&h) else {
        panic!("icon: {:?}", h.state())
    };
    assert!(icon.composites[0].image.is_none());
    for did in [EMPTY_SURFACE, THIN_SURFACE] {
        let h = icons.request(did, Priority::Visible);
        match wait(&h) {
            AssetState::Ready(icon) => {
                assert!(icon.composites.iter().all(|c| c.image.is_none()));
            }
            AssetState::Failed(_) => {}
            other => panic!("icon {did:#010x}: {other:?}"),
        }
    }
    let names = AssetStore::new(
        &pool,
        ContextSource,
        NameIndexDecoder(Arc::clone(ctx)),
        cfg.clone(),
    );
    let h = names.request(0x2800_0000, Priority::Visible);
    let AssetState::Ready(n) = wait(&h) else {
        panic!("names: {:?}", h.state())
    };
    assert!(n.entries.is_empty());
    assert!(!n.problems.is_empty());
    let records = AssetStore::new(&pool, ContextSource, RecordDecoder(Arc::clone(ctx)), cfg);
    let h = records.request(LIST, Priority::Visible);
    assert!(matches!(wait(&h), AssetState::Ready(_)));
}

#[test]
fn indexer_on_background_threads() {
    let inst = install();
    let set = Arc::clone(&inst.ctx.set);
    let notified = Arc::new(AtomicU64::new(0));
    let n = Arc::clone(&notified);
    let mut indexer = Indexer::spawn(Arc::clone(&set), move || {
        n.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(indexer.counts().1, 2);
    let mut got = Vec::new();
    let start = std::time::Instant::now();
    while !indexer.is_complete() {
        got.extend(indexer.poll());
        assert!(start.elapsed() < T, "indexing did not finish");
        std::thread::sleep(Duration::from_millis(5));
    }
    got.extend(indexer.poll());
    assert_eq!(got.len(), 2);
    assert_eq!(indexer.counts(), (2, 2));
    // `notify` runs right after each index is sent.
    while notified.load(Ordering::Relaxed) < 2 {
        assert!(start.elapsed() < T, "notify was not called per archive");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(notified.load(Ordering::Relaxed), 2);
    let no = AtomicBool::new(false);
    for idx in &got {
        assert!(idx.error.is_none());
        let m = set.member(idx.member);
        let direct = index_archive(
            &m.archive,
            idx.member,
            m.id.to_string(),
            &AtomicU64::new(0),
            &no,
        );
        assert_eq!(idx.entries, direct.entries);
        assert_eq!(indexer.scanned(idx.member) as usize, idx.entries.len());
    }
    assert_eq!(
        indexer.scanned_total() as usize,
        got.iter().map(|i| i.entries.len()).sum::<usize>()
    );
    // Out-of-range members read as zero.
    assert_eq!(indexer.scanned(99), 0);

    // Dropping an indexer at once cancels it without blocking or panicking.
    let started = std::time::Instant::now();
    drop(Indexer::spawn(set, || {}));
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn clip_catalogue_survives_unreadable_records() {
    // Regression: one record whose zlib stream is broken made the whole 0x05 catalogue
    // fail, and the browser spun for ever.
    let dir = datkit::TempDir::new("explorer-clips");
    let mut g = datkit::Builder::new();
    // Compressed flag, a plausible inflated size, then bytes that are no zlib stream.
    let mut broken = 64u32.to_le_bytes().to_vec();
    broken.extend_from_slice(&[0xFF; 32]);
    g.add(0x0500_0001, 3, broken, datkit::Layout::Plain);
    // Readable, but no clip envelope.
    g.stored(0x0500_0002, vec![1, 2, 3]);
    std::fs::write(dir.0.join("client_general.dat"), &g.build().bytes).unwrap();
    let set = Arc::new(ArchiveSet::open(&dir.0, SetOptions::default()).unwrap());
    let ctx = DataContext::from_set(dir.0.clone(), set, None);
    let cat =
        tessera_explorer::character::build_clip_catalog(&ctx, &JobContext::detached()).unwrap();
    assert!(cat.clips.is_empty());
    assert_eq!(cat.failed, 2, "{cat:?}");
    assert_eq!(cat.errors.len(), 1, "{:?}", cat.errors);
    assert!(cat.errors[0].starts_with("0x05000001"), "{:?}", cat.errors);
}
