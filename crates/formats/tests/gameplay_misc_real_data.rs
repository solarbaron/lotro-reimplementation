// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks of the miscellaneous record decoders (`tessera_formats::gameplay::misc`)
//! against a user's own install, read-only: every record of each family is decoded and the
//! census counts and invariants of `docs/specs/misc-records.md` are checked.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. The census counts are checked
//! against the local baseline of the install version (decision 0045, `tessera-testkit`),
//! with the number the spec records for its install beside each (§2, §3.2, §4.2, §5, §7,
//! §9–§13; drift reports, `TESSERA_CENSUS_STRICT=1`). Invariants and the record rules MR1–MR11
//! stay assertions. Nothing read from the archives is printed except counts.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Debug;
use std::path::PathBuf;

use tessera_dat::ArchiveSet;
use tessera_formats::gameplay::misc::*;
use tessera_formats::imposter::ImposterTable;
use tessera_formats::{
    Did, EntityDescriptor, PropertyDictionary, PropertySetRecord, Record, RgbaImage, Surface,
    SurfaceFormat, TextureList,
};
use tessera_wstate::ObjectStore;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

type Census = tessera_testkit::Census;

/// The install and the census of one test function (decision 0045).
fn open(function: &str) -> Option<(ObjectStore, Census)> {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return None;
    };
    let census = tessera_testkit::census!(function, &dir).expect("census");
    Some((ObjectStore::open(&dir).expect("open install"), census))
}

/// One count per key of a histogram, with the spec's figure where it states one (a key on
/// one side only counts 0 on the other).
fn hist<K: Ord + Debug + Clone>(
    c: &mut Census,
    what: &str,
    observed: &BTreeMap<K, usize>,
    spec: &[(K, usize)],
) {
    let spec: BTreeMap<K, usize> = spec.iter().cloned().collect();
    let keys: BTreeSet<K> = spec.keys().chain(observed.keys()).cloned().collect();
    for k in keys {
        let name = format!("{what} {k:?}");
        let got = observed.get(&k).copied().unwrap_or(0);
        match spec.get(&k) {
            Some(&n) => c.count_vs_spec(&name, got, n),
            None => c.count(&name, got),
        }
    }
}

/// One count per position of a list, with the spec's list beside it.
fn list<T: Copy + Into<tessera_testkit::Count>>(
    c: &mut Census,
    what: &str,
    observed: &[T],
    spec: &[T],
) {
    c.count_vs_spec(&format!("{what}: length"), observed.len(), spec.len());
    for i in 0..observed.len().max(spec.len()) {
        let name = format!("{what} [{i}]");
        match (observed.get(i), spec.get(i)) {
            (Some(&o), Some(&s)) => c.count_vs_spec(&name, o, s),
            (Some(&o), None) => c.count(&name, o),
            (None, Some(&s)) => c.count_vs_spec(&name, 0u8, s),
            (None, None) => {}
        }
    }
}

/// Every record in `lo..hi` of every archive: (archive file name, DID, payload).
fn records(set: &ArchiveSet, lo: u32, hi: u32) -> Vec<(String, Did, Vec<u8>)> {
    let mut out = Vec::new();
    for m in set.members() {
        for e in m.archive.entries_range(lo..hi) {
            let e = e.expect("directory entry");
            let payload = m.archive.read_entry(&e).expect("read record");
            out.push((m.id.file_name(), Did(e.did), payload));
        }
    }
    out
}

/// DIDs only (no payload read).
fn dids(set: &ArchiveSet, lo: u32, hi: u32) -> Vec<(String, Did)> {
    let mut out = Vec::new();
    for m in set.members() {
        for e in m.archive.entries_range(lo..hi) {
            out.push((m.id.file_name(), Did(e.expect("directory entry").did)));
        }
    }
    out
}

fn dictionary(store: &ObjectStore) -> PropertyDictionary {
    store
        .game_data()
        .registry()
        .expect("dictionary")
        .dictionary()
        .clone()
}

fn unique_db(store: &ObjectStore, label: &str) -> Did {
    store
        .game_data()
        .lookup_path(&[did_map_labels::UNIQUE_DB, label])
        .expect("UNIQUEDB")
        .unwrap_or_else(|| panic!("UNIQUEDB label {label}"))
}

fn read(store: &ObjectStore, did: Did) -> Vec<u8> {
    store
        .game_data()
        .read(did)
        .expect("read")
        .unwrap_or_else(|| panic!("{did} exists"))
}

fn exists(store: &ObjectStore, did: Did) -> bool {
    store.game_data().read(did).expect("read").is_some()
}

fn histogram<K: Ord>(it: impl IntoIterator<Item = K>) -> BTreeMap<K, usize> {
    let mut h = BTreeMap::new();
    for k in it {
        *h.entry(k).or_insert(0) += 1;
    }
    h
}

fn is_archive(name: &str, key: &str) -> bool {
    name.to_ascii_lowercase()
        .starts_with(&format!("client_{key}"))
}

// ---------------------------------------------------------------------------------------------
// §3 compositors
// ---------------------------------------------------------------------------------------------

#[test]
fn compositors() {
    let Some((store, mut c)) = open("compositors") else {
        return;
    };
    let data = store.game_data();
    let recs = records(data.source(), 0x3300_0000, 0x3400_0000);
    c.count_vs_spec("§2 compositors", recs.len(), 24);
    assert!(recs.iter().all(|(a, ..)| is_archive(a, "general")));

    let mut comps = Vec::new();
    for (_, did, payload) in &recs {
        let c = ImageCompositor::parse_expecting(payload, *did)
            .unwrap_or_else(|e| panic!("{did}: {e}"));
        // MR1: exact parse, byte-exact rewrite.
        assert_eq!(c.to_bytes(), *payload, "{did}");
        assert_eq!(c.stored_len(), payload.len());
        assert_eq!(c.first_unusual_field(), None, "{did} (MR2/MR3)");
        comps.push(c);
    }
    let layer_hist = histogram(comps.iter().map(|c| c.layers.len()));
    hist(
        &mut c,
        "compositors with layer count",
        &layer_hist,
        &[(2, 4), (3, 1), (7, 11), (8, 2), (9, 2), (11, 4)],
    );
    let layers: Vec<&CompositorLayer> = comps.iter().flat_map(|c| &c.layers).collect();
    c.count_vs_spec("layers", layers.len(), 166);
    let options: Vec<&CompositorOption> = layers.iter().flat_map(|l| &l.options).collect();
    c.count_vs_spec("options", options.len(), 1811);
    assert!(layers.iter().all(|l| (1..=40).contains(&l.options.len())));
    hist(
        &mut c,
        "options with colour entries",
        &histogram(options.iter().map(|o| o.colours.len())),
        &[(0, 307), (1, 1082), (2, 422)],
    );
    let entries: Vec<&ColourEntry> = options.iter().flat_map(|o| &o.colours).collect();
    c.count_vs_spec("colour entries", entries.len(), 1926);
    assert!(options.iter().all(|o| !o.legacy_did.is_null()));
    let zero_mask: Vec<&&ColourEntry> = entries.iter().filter(|c| c.mask_did.is_null()).collect();
    c.count_vs_spec("zero-mask colour entries", zero_mask.len(), 121);
    // §3.4: all zero-mask entries share one colour key under one layer key.
    let zero_mask_keys: BTreeSet<(u32, u32)> = layers
        .iter()
        .flat_map(|l| {
            l.options
                .iter()
                .flat_map(|o| &o.colours)
                .filter(|c| c.mask_did.is_null())
                .map(|c| (l.layer_key, c.colour_key))
        })
        .collect();
    // §3.4 states that zero-mask entries exist (121 in the spec's install) and that they all
    // share one key pair, so there is exactly one.
    assert_eq!(
        zero_mask_keys.len(),
        1,
        "§3.4: zero-mask entries share one key pair"
    );
    assert!(
        entries
            .iter()
            .all(|c| c.legacy_did.is_null() == c.mask_did.is_null())
    );
    let flagged: BTreeSet<u32> = comps
        .iter()
        .filter(|c| c.compress_with_alpha)
        .map(|c| c.did.0)
        .collect();
    c.count_vs_spec("compositors compressed with alpha", flagged.len(), 3);
    // `index` is 1 … n in stored order in all layers but one.
    let contiguous = layers
        .iter()
        .filter(|l| {
            l.options
                .iter()
                .enumerate()
                .all(|(i, o)| o.index as usize == i + 1)
        })
        .count();
    c.count_vs_spec("layers with contiguous option indices", contiguous, 165);
    c.count_vs_spec(
        "largest option x",
        options.iter().map(|o| o.x).max().unwrap_or(0),
        476u32,
    );
    c.count_vs_spec(
        "largest option y",
        options.iter().map(|o| o.y).max().unwrap_or(0),
        701u32,
    );

    // §3.4: legacy DIDs exist nowhere; images and masks all exist.
    let legacy: BTreeSet<Did> = options
        .iter()
        .map(|o| o.legacy_did)
        .chain(entries.iter().map(|c| c.legacy_did))
        .filter(|d| !d.is_null())
        .collect();
    let deps: BTreeSet<Did> = comps
        .iter()
        .flat_map(ImageCompositor::dependencies)
        .collect();
    c.count_vs_spec("distinct legacy DIDs", legacy.len(), 2977);
    c.count_vs_spec("distinct surface dependencies", deps.len(), 2977);
    assert!(legacy.is_disjoint(&deps));
    assert!(
        legacy.iter().all(|&d| !exists(&store, d)),
        "a legacy DID exists"
    );

    // C4: formats and mask sizes, from the surface headers.
    let mut headers: HashMap<Did, (SurfaceFormat, u32, u32)> = HashMap::new();
    for &d in &deps {
        let bytes = read(&store, d);
        let s = Surface::parse_expecting(&bytes, d).unwrap_or_else(|e| panic!("{d}: {e}"));
        headers.insert(d, (s.format, s.width, s.height));
    }
    let image_formats = histogram(options.iter().map(|o| headers[&o.image_did].0.name()));
    hist(
        &mut c,
        "option images of format",
        &image_formats,
        &[
            (SurfaceFormat::A8R8G8B8.name(), 1700),
            (SurfaceFormat::R8G8B8.name(), 111),
        ],
    );
    // §3.4: base-layer images are R8G8B8 or A8R8G8B8, the other layers' all A8R8G8B8.
    let by_layer = |base: bool| {
        histogram(
            comps
                .iter()
                .flat_map(|c| c.layers.iter().enumerate())
                .filter(|&(i, _)| (i == 0) == base)
                .flat_map(|(_, l)| &l.options)
                .map(|o| headers[&o.image_did].0.name()),
        )
    };
    let base = by_layer(true);
    assert!(
        base.keys()
            .all(|&f| f == SurfaceFormat::A8R8G8B8.name() || f == SurfaceFormat::R8G8B8.name()),
        "§3.4: base-layer images are R8G8B8 or A8R8G8B8"
    );
    hist(
        &mut c,
        "base-layer images of format",
        &base,
        &[
            (SurfaceFormat::A8R8G8B8.name(), 46),
            (SurfaceFormat::R8G8B8.name(), 111),
        ],
    );
    let others = by_layer(false);
    assert!(
        others.keys().all(|&f| f == SurfaceFormat::A8R8G8B8.name()),
        "§3.4: the other layers' images are A8R8G8B8"
    );
    c.count_vs_spec("other-layer images", others.values().sum::<usize>(), 1654);
    let mut masks = 0;
    for o in &options {
        let (_, w, h) = headers[&o.image_did];
        for c in o.colours.iter().filter(|c| !c.mask_did.is_null()) {
            let (f, mw, mh) = headers[&c.mask_did];
            assert_eq!(
                (f, mw, mh),
                (SurfaceFormat::A8, w, h),
                "mask of {}",
                o.image_did
            );
            masks += 1;
        }
    }
    c.count_vs_spec("masks", masks, 1805);

    // MR3 and the off-canvas census of §3.6.2.
    let mut off_canvas = Vec::new();
    for c in &comps {
        let base = &c.layers[0];
        let (_, bw, bh) = headers[&base.options[0].image_did];
        for o in &base.options {
            let (_, w, h) = headers[&o.image_did];
            assert_eq!((w, h, o.x, o.y), (bw, bh, 0, 0), "{} (MR3)", c.did);
        }
        for o in c.layers.iter().skip(1).flat_map(|l| &l.options) {
            let (_, w, h) = headers[&o.image_did];
            assert!(o.x < bw && o.y < bh, "option starts outside the canvas");
            if o.x + w > bw || o.y + h > bh {
                off_canvas.push(c.did);
            }
        }
    }
    c.count_vs_spec("off-canvas options", off_canvas.len(), 8);
    c.count_vs_spec(
        "compositors with off-canvas options",
        off_canvas.iter().collect::<BTreeSet<_>>().len(),
        1,
    );

    // §3.6: the explorer preview composes for every record, with white (absent) colours
    // and with a non-white colour for every key, which runs the masked multiply on the
    // install's A8 masks.
    let tint = |k: u32| {
        let b = k.to_le_bytes();
        Some([
            f32::from(b[0] | 0x40) * BYTE_TO_UNIT,
            f32::from(b[1] | 0x20) * BYTE_TO_UNIT,
            0.5,
            1.0,
        ])
    };
    let mut tinted_differs = 0;
    for c in &comps {
        let sel = c.preview_selection();
        let mut images: HashMap<Did, RgbaImage> = HashMap::new();
        for d in c.required_images(&sel) {
            let bytes = read(&store, d);
            let s = Surface::parse(&bytes).expect("surface");
            images.insert(d, s.to_rgba8().expect("rgba"));
        }
        let out = c
            .compose(&sel, |_| None, |d| images.get(&d))
            .unwrap_or_else(|e| panic!("{}: {e:?}", c.did))
            .expect("an image");
        let (_, bw, bh) = headers[&c.layers[0].options[0].image_did];
        assert_eq!((out.width, out.height), (bw, bh));
        let tinted = c
            .compose(&sel, tint, |d| images.get(&d))
            .unwrap_or_else(|e| panic!("{} (tinted): {e:?}", c.did))
            .expect("an image");
        assert_eq!((tinted.width, tinted.height), (bw, bh));
        assert_eq!(tinted.pixels.len(), out.pixels.len());
        tinted_differs += usize::from(tinted.pixels != out.pixels);
    }
    eprintln!("compositors whose tinted preview differs from the white one: {tinted_differs}");
    assert!(tinted_differs > 0, "no colour entry changed any preview");

    // §2: the face compositors named in PRELOADRESOURCES.
    let preload = data
        .did_map_by_label(did_map_labels::PRELOAD_RESOURCES)
        .expect("map")
        .expect("PRELOADRESOURCES");
    let faces: Vec<Did> = preload
        .iter()
        .filter(|e| is_face_compositor_label(&e.label))
        .map(|e| e.value)
        .collect();
    c.count_vs_spec("face compositors in PRELOADRESOURCES", faces.len(), 7);
    assert!(faces.iter().all(|d| d.prefix() == IMAGE_COMPOSITOR_PREFIX));

    // C2: every layer and colour key resolves in its enum mapper.
    for (label, keys) in [
        (
            enum_mapper_labels::IMAGE_COMPOSITOR_LAYER_NAMES,
            layers.iter().map(|l| l.layer_key).collect::<BTreeSet<_>>(),
        ),
        (
            enum_mapper_labels::IMAGE_COMPOSITOR_COLOR_NAMES,
            entries.iter().map(|c| c.colour_key).collect(),
        ),
    ] {
        let mapper = data
            .lookup_path(&["EMAPPER", label])
            .expect("EMAPPER")
            .expect(label);
        for k in keys {
            let l = data.enum_label(mapper, k).expect("mapper");
            assert!(l.name.is_some(), "{label} key {k:#x}");
        }
    }
    eprintln!(
        "compositors: {} records, {} layers, {} options, {} colour entries, {} surfaces",
        comps.len(),
        layers.len(),
        options.len(),
        entries.len(),
        deps.len()
    );
    c.finish();
}

/// §3.7, C8: the character-creation compositor controls on entity descriptors.
#[test]
fn compositor_controls() {
    let Some((store, mut c)) = open("compositor_controls") else {
        return;
    };
    let dict = dictionary(&store);
    let pids = CompositorControlPids::resolve(&dict);
    assert!(pids.missing().is_empty(), "{:?}", pids.missing());
    let mut controls = Vec::new();
    let mut descriptors = 0usize;
    for (_, did, payload) in records(store.game_data().source(), 0x4700_0000, 0x4800_0000) {
        let ed = EntityDescriptor::parse(&payload, &dict).unwrap_or_else(|e| panic!("{did}: {e}"));
        controls.extend(CompositorControl::find_all(&ed.properties, &pids));
        descriptors += 1;
    }
    // Every 0x47 record parses (above). A rule check, not a count: an empty range would leave
    // the checks below vacuous.
    assert!(descriptors > 0, "entity descriptors");
    assert!(!controls.is_empty(), "compositor controls (§3.7)");
    c.count_vs_spec("entity descriptors", descriptors, 40_643);
    c.count_vs_spec("compositor controls", controls.len(), 218);
    assert!(controls.iter().all(|c| {
        c.file
            .is_some_and(|d| d.prefix() == IMAGE_COMPOSITOR_PREFIX)
    }));
    assert!(controls.iter().all(|c| matches!(c.min_value, Some(0 | 1))));
    // X3: the layer name is an enum key and the control property a PID.
    assert!(
        controls
            .iter()
            .all(|c| c.layer_key.is_some() && c.control_property.is_some())
    );
    eprintln!(
        "compositor controls: {} in {descriptors} descriptors",
        controls.len()
    );
    c.finish();
}

// ---------------------------------------------------------------------------------------------
// §4 UI scenes
// ---------------------------------------------------------------------------------------------

#[test]
fn ui_scenes() {
    let Some((store, mut c)) = open("ui_scenes") else {
        return;
    };
    let dict = dictionary(&store);
    let recs = records(store.game_data().source(), 0x2100_0000, 0x2200_0000);
    c.count_vs_spec("UI scenes", recs.len(), 20);
    let mut scenes = Vec::new();
    for (_, did, payload) in &recs {
        scenes.push(
            UiScene::parse_expecting(payload, *did, &dict).unwrap_or_else(|e| panic!("{did}: {e}")),
        );
    }
    let seds: Vec<_> = scenes.iter().flat_map(|s| &s.entities).collect();
    c.count_vs_spec("scene descriptors", seds.len(), 3051);
    let per_scene = scenes.iter().map(|s| s.entities.len());
    c.count_vs_spec(
        "fewest descriptors in a scene",
        per_scene.clone().min().unwrap_or(0),
        2,
    );
    c.count_vs_spec(
        "most descriptors in a scene",
        per_scene.max().unwrap_or(0),
        732,
    );
    assert!(
        scenes.iter().all(|s| s.colour == 0
            && !s.flag
            && s.value_a == 0xBF80_0000
            && s.value_b == 0xBF80_0000)
    );
    // U2: flag and type census.
    let bit = |b: u32| seds.iter().filter(|e| e.flags & b != 0).count();
    // Bit 0x010: 3,026, as the flag-word census below implies (U2, corrected).
    for (b, spec) in [
        0x001, 0x002, 0x004, 0x008, 0x010, 0x020, 0x040, 0x080, 0x100, 0x200, 0x400,
    ]
    .into_iter()
    .zip([0, 3051, 323, 2969, 3026, 0, 0, 2099, 3041, 0, 0])
    {
        c.count_vs_spec(&format!("descriptors with flag bit {b:#05x}"), bit(b), spec);
    }
    hist(
        &mut c,
        "descriptors with flag word",
        &histogram(seds.iter().map(|e| e.flags)),
        &[
            (0x19A, 1867),
            (0x11A, 761),
            (0x19E, 159),
            (0x11E, 151),
            (0x192, 41),
            (0x112, 25),
            (0x18A, 15),
            (0x9A, 10),
            (0x116, 7),
            (0x10A, 5),
            (0x196, 5),
            (0x102, 3),
            (0x182, 1),
            (0x18E, 1),
        ],
    );
    hist(
        &mut c,
        "descriptors with entity type",
        &histogram(seds.iter().filter_map(|e| e.entity_type)),
        &[(2, 7), (7, 2842), (9, 120), (27, 82)],
    );
    c.count_vs_spec(
        "distinct flag words",
        histogram(seds.iter().map(|e| e.flags)).len(),
        14,
    );
    let objects: Vec<Did> = scenes.iter().flat_map(UiScene::objects).collect();
    c.count_vs_spec("scene objects", objects.len(), 2969);
    assert!(objects.iter().all(|d| d.prefix() == 0x47));
    eprintln!(
        "UI scenes: {} records, {} descriptors",
        scenes.len(),
        seds.len()
    );
    c.finish();
}

// ---------------------------------------------------------------------------------------------
// §5 imposter tables
// ---------------------------------------------------------------------------------------------

/// How a non-empty imposter table relates to the cell rule and the cell size of MR5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum AtlasLayout {
    /// §5.2 and §5.4 (MR5): the *i*-th entry in cell (*i* mod 16, ⌊*i* / 16⌋), cells of
    /// (31/32) / grid; checked in full.
    Spec,
    /// Tessera interim rule while the `misc-records.md` §5.4 OPEN (2026-10-31) is
    /// unanswered: a table that breaks only the MR5 cell rule or cell size is an OPEN
    /// exception, counted and reported, not checked against any other layout (the spec
    /// states none). Every other part of MR5 still holds for it.
    OpenException,
}

/// The note printed when a table is an [`AtlasLayout::OpenException`].
const IMPOSTER_OPEN: &str = "misc-records.md §5.4 OPEN (2026-10-31): tables breaking the MR5 \
                             cell rule or cell size are counted as exceptions, not checked";

#[test]
fn imposter_tables() {
    let Some((store, mut census)) = open("imposter_tables") else {
        return;
    };
    let data = store.game_data();
    let recs = records(data.source(), 0x2700_0000, 0x2800_0000);
    // A rule check, not a count: an empty range would leave the checks below vacuous.
    assert!(!recs.is_empty(), "imposter tables");
    census.count_vs_spec("imposter tables", recs.len(), 507);
    let mut tables = Vec::new();
    for (_, did, payload) in &recs {
        let t =
            ImposterTable::parse_expecting(payload, *did).unwrap_or_else(|e| panic!("{did}: {e}"));
        assert_eq!(t.to_bytes(), *payload);
        tables.push(t);
    }
    // §5.1: one table per 27 × 27 square, so at most 100 per region.
    let by_region = histogram(tables.iter().map(|t| t.square().expect("square").region));
    assert!(
        by_region.values().all(|&n| n <= 100),
        "at most 100 tables per region: {by_region:?}"
    );
    hist(
        &mut census,
        "tables in region",
        &by_region,
        &[(1, 100), (2, 94), (3, 100), (4, 100), (5, 100), (14, 13)],
    );
    let entries: usize = tables.iter().map(|t| t.entries.len()).sum();
    census.count_vs_spec("entries", entries, 12_406);
    census.count_vs_spec(
        "most entries in a table",
        tables.iter().map(|t| t.entries.len()).max().unwrap_or(0),
        128,
    );
    // MR5 for every table. A table that breaks only the cell rule or the cell size is an
    // OPEN exception (`misc-records.md` §5.4 OPEN; Tessera interim rule): counted, and the
    // parts of MR5 that `first_unusual_field` checks after the cell rule (both atlases) are
    // checked for it here. Any other MR5 failure fails the test.
    let mut layouts: HashMap<Did, AtlasLayout> = HashMap::new();
    let mut fifteen_per_row = 0usize;
    for t in &tables {
        let layout = match t.first_unusual_field() {
            None => AtlasLayout::Spec,
            Some(
                "entry cell is not (i mod 16, i / 16) (MR5)"
                | "cell size is not (31/32) / grid (MR5)",
            ) => {
                assert!(
                    !t.colour_atlas.is_null() && !t.second_atlas.is_null(),
                    "{}: non-empty table without both atlases (MR5)",
                    t.did
                );
                // An observation for the OPEN, counted, not a rule.
                fifteen_per_row +=
                    usize::from(t.entries.iter().enumerate().all(|(i, e)| {
                        usize::from(e.column) == i % 15 && usize::from(e.row) == i / 15
                    }));
                AtlasLayout::OpenException
            }
            Some(other) => panic!("{}: {other}", t.did),
        };
        if !t.entries.is_empty() {
            layouts.insert(t.did, layout);
        }
    }
    hist(
        &mut census,
        "non-empty tables with layout",
        &histogram(layouts.values().copied()),
        &[(AtlasLayout::Spec, 300)],
    );
    let exceptions = layouts
        .values()
        .filter(|l| **l == AtlasLayout::OpenException)
        .count();
    if exceptions > 0 {
        eprintln!("imposter tables: {exceptions} MR5 exceptions; {IMPOSTER_OPEN}");
    }
    census.count(
        "MR5 exceptions (§5.4 OPEN) with entries 15 per row",
        fifteen_per_row,
    );
    let multi = tables.iter().filter(|t| t.entries.len() >= 2).count();
    census.count_vs_spec("tables with two or more entries", multi, 289);
    let empty: Vec<&ImposterTable> = tables.iter().filter(|t| t.entries.is_empty()).collect();
    let all_zero = empty
        .iter()
        .filter(|t| {
            t.colour_atlas.is_null()
                && t.second_atlas.is_null()
                && t.cell_u == 0.0
                && t.cell_v == 0.0
        })
        .count();
    let with_lists = empty
        .iter()
        .filter(|t| t.dependencies().len() == 2 && t.cell_u == 0.0 && t.cell_v == 0.0)
        .count();
    // An empty table either names nothing at all or names two texture lists with a zero
    // cell size.
    assert_eq!(
        all_zero + with_lists,
        empty.len(),
        "empty tables: all zero or two lists with a zero cell"
    );
    census.count_vs_spec("empty tables", empty.len(), 207);
    census.count_vs_spec("empty tables naming nothing", all_zero, 204);
    census.count_vs_spec("empty tables with two lists", with_lists, 3);

    // I3: the atlases of the non-empty tables.
    let full: Vec<&ImposterTable> = tables.iter().filter(|t| !t.entries.is_empty()).collect();
    assert_eq!(empty.len() + full.len(), tables.len());
    census.count_vs_spec("non-empty tables", full.len(), 300);
    let mut lists = 0;
    let mut atlas_cell = 0usize;
    for t in &full {
        let mut formats = Vec::new();
        let mut sizes = Vec::new();
        for list_did in t.dependencies() {
            let bytes = read(&store, list_did);
            let list = TextureList::parse_expecting(&bytes, list_did).expect("texture list");
            let levels = list.levels().expect("mip chain");
            assert_eq!(levels.len(), 4, "{list_did}");
            let top = read(&store, levels[0]);
            let s = Surface::parse(&top).expect("surface");
            sizes.push((s.width, s.height));
            formats.push(s.format);
            lists += 1;
        }
        assert_eq!(formats, vec![SurfaceFormat::Dxt5, SurfaceFormat::Dxt1]);
        assert!(
            sizes.windows(2).all(|w| w[0] == w[1]),
            "{}: atlas sizes",
            t.did
        );
        let (w, h) = sizes.first().copied().expect("two lists");
        match layouts.get(&t.did) {
            Some(AtlasLayout::Spec) => {
                assert_eq!(Some((w, h)), t.atlas_texels(), "{} (§5.4)", t.did);
            }
            Some(AtlasLayout::OpenException) => {
                // The §5.4 geometry is not checked (the spec states none for these
                // tables); an observation for the OPEN is counted.
                let one =
                    |cell: f32, texels: u32| (cell * (texels / 128) as f32 - 1.0).abs() <= 1.0e-6;
                atlas_cell += usize::from(
                    w % 128 == 0 && h % 128 == 0 && one(t.cell_u, w) && one(t.cell_v, h),
                );
            }
            None => unreachable!("every non-empty table has a layout"),
        }
    }
    census.count(
        "MR5 exceptions (§5.4 OPEN) with a cell of one 128-texel atlas cell",
        atlas_cell,
    );
    assert_eq!(
        lists,
        2 * full.len(),
        "I3: two texture lists per non-empty table"
    );

    // I4: keys resolve.
    let keys: BTreeSet<u32> = tables
        .iter()
        .flat_map(|t| t.entries.iter().map(|e| e.name_key))
        .collect();
    let mapper = data
        .lookup_path(&["EMAPPER", enum_mapper_labels::RENDER_IMPOSTER_NAMES])
        .expect("EMAPPER")
        .expect("RenderImposterNames");
    for k in &keys {
        assert!(data.enum_label(mapper, *k).expect("mapper").name.is_some());
    }
    census.count_vs_spec("distinct entry keys", keys.len(), 816);
    eprintln!(
        "imposter tables: {} records ({by_region:?} by region), {entries} entries, {multi} \
         with two or more, {} empty ({all_zero} all zero, {with_lists} with two lists), {} \
         non-empty with {lists} texture lists, {} distinct keys",
        tables.len(),
        empty.len(),
        full.len(),
        keys.len()
    );
    census.finish();
}

// ---------------------------------------------------------------------------------------------
// §6, §7, §9, §10, §11 singletons
// ---------------------------------------------------------------------------------------------

#[test]
fn compute_shaders() {
    let Some((store, mut census)) = open("compute_shaders") else {
        return;
    };
    let recs = records(store.game_data().source(), 0x2D00_0000, 0x2E00_0000);
    census.count_vs_spec("compute shaders", recs.len(), 3);
    let by_label: BTreeSet<Did> = unique_db_labels::COMPUTE_SHADERS
        .iter()
        .map(|l| unique_db(&store, l))
        .collect();
    let mut sizes = BTreeSet::new();
    for (_, did, payload) in &recs {
        assert!(by_label.contains(did));
        let cs = ComputeShader::parse_expecting(payload, *did).expect("compute shader");
        assert_eq!(payload.len(), 8 + cs.blob.len());
        assert!(cs.is_consistent(), "MR6");
        let c = cs.container().expect("DXBC");
        let fourccs: Vec<[u8; 4]> = c.chunks.iter().map(|c| c.fourcc).collect();
        assert_eq!(
            fourccs,
            vec![*b"RDEF", *b"ISGN", *b"OSGN", *b"SHEX", *b"STAT"]
        );
        assert_eq!(c.program, Some((5, 5, 0)));
        sizes.insert(cs.blob.len());
    }
    let sizes: Vec<usize> = sizes.into_iter().collect();
    list(
        &mut census,
        "distinct blob sizes",
        &sizes,
        &[1664, 2532, 20_780],
    );
    census.finish();
}

#[test]
fn script_channels() {
    let Some((store, mut census)) = open("script_channels") else {
        return;
    };
    let data = store.game_data();
    let did = unique_db(&store, unique_db_labels::SCRIPT_CHANNELS);
    assert_eq!(did.prefix(), SCRIPT_CHANNELS_PREFIX);
    let all = dids(data.source(), 0x4300_0000, 0x4400_0000);
    census.count_vs_spec("script-channel records", all.len(), 1);
    let payload = read(&store, did);
    census.count_vs_spec("record bytes", payload.len(), 3285);
    let sc = ScriptChannels::parse(&payload).expect("script channels");
    assert_eq!(sc.to_bytes(), payload);
    census.count_vs_spec("bucket hint", sc.size.hint, 0u8);
    let per = |f: fn(&ScriptChannel) -> usize| sc.channels.iter().map(f).collect::<Vec<_>>();
    list(
        &mut census,
        "scripts per channel",
        &per(|c| c.scripts.len()),
        &[296, 34, 13, 2],
    );
    list(
        &mut census,
        "script bucket hint per channel",
        &per(|c| usize::from(c.scripts_size.hint)),
        &[4, 1, 0, 0],
    );
    list(
        &mut census,
        "transitions per channel",
        &per(|c| c.transitions.len()),
        &[0, 0, 12, 0],
    );
    assert!(sc.channels.iter().all(|c| c.exclusive));
    assert!(sc.global_transitions.is_empty());
    let scripts: Vec<&ChannelScript> = sc.channels.iter().flat_map(|c| &c.scripts).collect();
    census.count_vs_spec(
        "interruptible scripts",
        scripts.iter().filter(|s| s.interruptible).count(),
        343,
    );
    census.count_vs_spec(
        "uninterruptible scripts",
        scripts.iter().filter(|s| !s.interruptible).count(),
        2,
    );
    let priorities: Vec<u32> = scripts
        .iter()
        .map(|s| s.priority)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    list(
        &mut census,
        "distinct script priorities",
        &priorities,
        &[0, 1, 2, 10_000],
    );
    let channel_keys: Vec<u32> = sc
        .channels
        .iter()
        .map(|c| c.channel_key)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    list(
        &mut census,
        "distinct channel keys",
        &channel_keys,
        &[1, 0x1000_0001, 0x1000_0003, 0x1000_0005],
    );
    // §7.3: 346 distinct script keys over scripts and transitions.
    census.count_vs_spec("distinct script keys", sc.script_keys().len(), 346);
    let fx = data
        .lookup_path(&["EMAPPER", enum_mapper_labels::FX])
        .expect("EMAPPER")
        .expect("FX");
    for k in sc.script_keys() {
        assert!(data.enum_label(fx, k).expect("FX").name.is_some(), "{k:#x}");
    }
    census.finish();
}

#[test]
fn name_filter_and_taboo_tables() {
    let Some((store, mut census)) = open("name_filter_and_taboo_tables") else {
        return;
    };
    let did = unique_db(&store, unique_db_labels::NAME_FILTER_TABLE);
    let payload = read(&store, did);
    census.count_vs_spec("name-filter bytes", payload.len(), 2685);
    let nf = NameFilterTable::parse_expecting(&payload, did).expect("name filter");
    census.count_vs_spec("name-filter bucket hint", nf.size.hint, 1u8);
    let rule_keys: Vec<u32> = nf
        .rule_sets
        .iter()
        .map(|r| r.rule_key)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    list(
        &mut census,
        "distinct rule keys",
        &rule_keys,
        &[1, 2, 0x1000_0003],
    );
    for (i, r) in nf.rule_sets.iter().enumerate() {
        // MR14: the class keys are the 13 single bits 1 … 4096.
        let keys: BTreeSet<u32> = r.character_classes.iter().map(|c| c.key).collect();
        assert_eq!(keys, (0..13).map(|b| 1u32 << b).collect(), "MR14");
        assert_eq!(r.character_classes.len(), 13, "MR14");
        let set = |what: &str| format!("rule set {i}: {what}");
        census.count_vs_spec(&set("class bucket hint"), r.classes_size.hint, 1u8);
        census.count_vs_spec(&set("string_b length"), r.string_b.len(), 0);
        census.count_vs_spec(&set("string_a length"), r.string_a.len(), 3);
        census.count_vs_spec(&set("string_list_a length"), r.string_list_a.len(), 4);
        census.count_vs_spec(&set("flag_b"), u8::from(r.flag_b), 0u8);
    }
    census.count_vs_spec(
        "longest class character set",
        nf.rule_sets
            .iter()
            .flat_map(|r| &r.character_classes)
            .map(|c| c.characters.len())
            .max()
            .unwrap_or(0),
        62,
    );
    list(
        &mut census,
        "flag_a per rule set",
        &nf.rule_sets
            .iter()
            .map(|r| u8::from(r.flag_a))
            .collect::<Vec<_>>(),
        &[1, 0, 1],
    );
    list(
        &mut census,
        "u32_list length per rule set",
        &nf.rule_sets
            .iter()
            .map(|r| r.u32_list.len())
            .collect::<Vec<_>>(),
        &[4, 0, 3],
    );

    let did = unique_db(&store, unique_db_labels::TABOO_TABLE);
    let payload = read(&store, did);
    census.count_vs_spec("taboo bytes", payload.len(), 385_598);
    let tb = TabooTable::parse_expecting(&payload, did).expect("taboo");
    census.count_vs_spec("taboo headers", tb.headers.len(), 8);
    census.count_vs_spec("taboo bucket hint", tb.lists_size.hint, 1u8);
    assert!(tb.keys_match_headers(), "MR8");
    census.count_vs_spec("taboo patterns", tb.pattern_count(), 17_612);
    let sizes: Vec<usize> = tb.lists.iter().map(|l| l.patterns.len()).collect();
    census.count_vs_spec(
        "shortest taboo list",
        sizes.iter().min().copied().unwrap_or(0),
        39,
    );
    census.count_vs_spec(
        "longest taboo list",
        sizes.iter().max().copied().unwrap_or(0),
        6708,
    );
    census.finish();
}

#[test]
fn compression_dictionaries() {
    let Some((store, mut census)) = open("compression_dictionaries") else {
        return;
    };
    let data = store.game_data();
    let map = data
        .did_map_by_label(did_map_labels::COMPRESSION_DICTIONARY)
        .expect("map")
        .expect("COMPRESSIONDICTIONARY");
    let targets: Vec<Did> = map
        .iter()
        .map(|e| e.value)
        .filter(|d| !d.is_null())
        .collect();
    census.count_vs_spec("dictionaries", targets.len(), 3);
    let mut seen = BTreeMap::new();
    for did in targets {
        let payload = read(&store, did);
        let env = CompressionDictionaryEnvelope::parse(&payload).expect("MR9");
        assert_eq!(
            payload.len(),
            COMPRESSION_DICTIONARY_HEADER_LEN + env.body_len as usize
        );
        seen.insert(did, env);
    }
    let lens: Vec<u32> = seen
        .values()
        .map(|e| e.body_len)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    list(
        &mut census,
        "distinct body lengths",
        &lens,
        &[0, 214_909, 218_203],
    );
    census.count_vs_spec(
        "distinct id_a",
        seen.values().map(|e| e.id_a).collect::<BTreeSet<_>>().len(),
        1,
    );
    census.count_vs_spec(
        "distinct id_b",
        seen.values().map(|e| e.id_b).collect::<BTreeSet<_>>().len(),
        2,
    );
    census.finish();
}

// ---------------------------------------------------------------------------------------------
// §12 bare property sets
// ---------------------------------------------------------------------------------------------

#[test]
fn bare_property_sets() {
    let Some((store, mut census)) = open("bare_property_sets") else {
        return;
    };
    let dict = dictionary(&store);
    let classifier = BareSetClassifier::new(&dict);
    let recs = records(store.game_data().source(), 0x7800_0000, 0x7900_0000);
    let per_archive = histogram(recs.iter().map(|(a, ..)| {
        if is_archive(a, "gamelogic") {
            "gamelogic"
        } else if is_archive(a, "local_") {
            "local"
        } else {
            "other"
        }
    }));
    hist(
        &mut census,
        "0x78 sets in archive",
        &per_archive,
        &[("gamelogic", 1616), ("local", 2)],
    );
    let mut families = BTreeMap::new();
    let (mut empty, mut light_maps, mut fog) = (0, 0, 0);
    for (archive, did, payload) in &recs {
        let rec = PropertySetRecord::parse_expecting(payload, *did, &dict)
            .unwrap_or_else(|e| panic!("{did}: {e} (MR10)"));
        if rec.set.is_empty() {
            empty += 1;
        }
        if !is_archive(archive, "gamelogic") {
            continue;
        }
        let family = classifier.classify(&rec.set);
        *families.entry(family).or_insert(0) += 1;
        match family {
            BareSetFamily::GameMap => {
                let v = GameMapView::from_set(&rec.set, &classifier.pids).expect("game map");
                light_maps += usize::from(v.has_light_map);
                // X3: the fields have their dictionary types.
                assert!(
                    v.image.is_some()
                        && v.scale.is_some()
                        && v.pixel_offset.iter().all(Option::is_some)
                        && v.block_offset.iter().all(Option::is_some),
                    "§12.2: typed game-map fields"
                );
            }
            BareSetFamily::LocalSky => {
                use tessera_formats::gameplay::common::PropertyAccess;
                fog += usize::from(rec.set.has(classifier.pids.interior_fog_near));
            }
            _ => {}
        }
    }
    census.count_vs_spec("empty sets", empty, 9);
    hist(
        &mut census,
        "gamelogic sets of family",
        &families,
        &[
            (BareSetFamily::Empty, 8),
            (BareSetFamily::GameMap, 1300),
            (BareSetFamily::LocalSky, 147),
            (BareSetFamily::FootstepSounds, 38),
            (BareSetFamily::AmbientMusic, 25),
            (BareSetFamily::ContentLayerDefaults, 13),
            (BareSetFamily::Volume, 9),
            (BareSetFamily::Other, 76),
        ],
    );
    census.count_vs_spec("game maps with a light map", light_maps, 21);
    census.count_vs_spec("local skies with interior fog", fog, 7);
    census.finish();
}

// ---------------------------------------------------------------------------------------------
// §13 metadata objects, §14 WEENIECONTENT
// ---------------------------------------------------------------------------------------------

#[test]
fn metadata_objects() {
    use tessera_formats::gameplay::common::PropertyAccess;
    let Some((store, mut census)) = open("metadata_objects") else {
        return;
    };
    let data = store.game_data();
    let dict = dictionary(&store);
    let classifier = MetadataClassifier::new(&dict);
    let p = &classifier.pids;
    assert!(p.missing().is_empty(), "{:?}", p.missing());

    // §14.
    let weenie = data
        .did_map_by_label(did_map_labels::WEENIE_CONTENT)
        .expect("map")
        .expect("WEENIECONTENT");
    census.count_vs_spec("WEENIECONTENT entries", weenie.len(), 316);
    let targets = histogram(weenie.iter().map(|e| e.value.prefix()));
    census.count_vs_spec(
        "WEENIECONTENT targets 0x70",
        targets.get(&0x70).copied().unwrap_or(0),
        314,
    );
    census.count_vs_spec(
        "WEENIECONTENT targets 0x07",
        targets.get(&0x07).copied().unwrap_or(0),
        1,
    );
    let controls = MetadataControls::from_weenie_content(&weenie);
    let control = |d: Option<Did>| {
        let d = d.expect("control label");
        assert_eq!(d.prefix(), 0x70);
        let set_did = Did(d.0 + 0x0900_0000);
        PropertySetRecord::parse_expecting(&read(&store, set_did), set_did, &dict)
            .expect("control set")
            .set
    };

    // Every object set of the gamelogic archive, classified.
    // Decoded on a few threads: there are several hundred thousand sets.
    let set_dids: Vec<Did> = dids(data.source(), 0x7900_0000, 0x7A00_0000)
        .into_iter()
        .map(|(_, d)| d)
        .collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(8));
    let chunk = set_dids.len().div_ceil(threads).max(1);
    // Sets carrying `PropertyResponse_EntityPropertyResponseMap` (the §13.5 referrer) are
    // kept with their family too, members or not.
    type Found = (Option<MetadataFamily>, Did, tessera_formats::PropertySet);
    let entity_map_ref = p.response_entity_map_ref;
    let (found, referrers): (Vec<Found>, Vec<Found>) = std::thread::scope(|scope| {
        let handles: Vec<_> = set_dids
            .chunks(chunk)
            .map(|part| {
                let (dict, classifier) = (&dict, &classifier);
                scope.spawn(move || {
                    let mut out = Vec::new();
                    for &set_did in part {
                        let payload = data.read(set_did).expect("read").expect("listed");
                        let rec = PropertySetRecord::parse_expecting(&payload, set_did, dict)
                            .unwrap_or_else(|e| panic!("{set_did}: {e}"));
                        let f = classifier.classify(&rec.set);
                        let referrer = rec.set.has(entity_map_ref);
                        if f.is_some() || referrer {
                            out.push((f, Did(set_did.0 - 0x0900_0000), rec.set, referrer));
                        }
                    }
                    out
                })
            })
            .collect();
        let mut found = Vec::new();
        let mut referrers = Vec::new();
        for (f, d, set, referrer) in handles
            .into_iter()
            .flat_map(|h| h.join().expect("scan thread"))
        {
            if referrer {
                referrers.push((f, d, set.clone()));
            }
            if f.is_some() {
                found.push((f, d, set));
            }
        }
        (found, referrers)
    });
    let mut members: BTreeMap<MetadataFamily, Vec<Did>> = BTreeMap::new();
    let mut sets = HashMap::new();
    for (f, object, set) in found {
        let Some(f) = f else { continue };
        members.entry(f).or_default().push(object);
        sets.insert(object, set);
    }
    let count = |f| members.get(&f).map_or(0, Vec::len);
    eprintln!(
        "metadata families: {:?}",
        members
            .iter()
            .map(|(f, v)| (f, v.len()))
            .collect::<Vec<_>>()
    );

    // §13.1, MR11.
    let pmd = control(controls.property_metadata);
    let listed = property_metadata_members(&pmd, p);
    census.count_vs_spec("§13.1 listed property metadata", listed.len(), 5211);
    let family: BTreeSet<Did> = members[&MetadataFamily::PropertyMetaData]
        .iter()
        .copied()
        .collect();
    assert_eq!(
        listed.iter().copied().collect::<BTreeSet<_>>(),
        family,
        "MR11"
    );
    let mut index = PropertyMetaDataIndex::new();
    let mut percent = 0;
    let mut forwarded = (0, 0);
    let mut operators = 0;
    for d in &listed {
        let v = PropertyMetaDataView::from_set(&sets[d], p).expect("metadata");
        assert!(dict.definition(v.property).is_some());
        assert!(v.name.is_some());
        assert!((1..=5).contains(&v.allowed_operators.len()));
        operators += v.allowed_operators.len();
        percent += usize::from(v.display_as_percentage);
        forwarded.0 += usize::from(v.forwarded_value_target.is_some());
        forwarded.1 += usize::from(v.forwarded_required_weenie_type.is_some());
        index.insert(v.property, *d);
    }
    assert!(index.duplicates.is_empty(), "MR11");
    assert_eq!(index.len(), listed.len(), "MR11: one entry per property");
    census.count_vs_spec("§13.1 shown as percentage", percent, 2094);
    census.count_vs_spec("§13.1 forwarded value targets", forwarded.0, 364);
    census.count_vs_spec("§13.1 forwarded weenie types", forwarded.1, 352);
    census.count_vs_spec(
        "§13.1 allowed operators",
        operators,
        4498 + 4415 + 4048 + 2474 + 539 + 44 + 12,
    );

    // §13.2.
    census.count_vs_spec(
        "§13.2 error categories",
        count(MetadataFamily::ErrorCategory),
        675,
    );
    let cats: Vec<ErrorCategoryView<'_>> = members[&MetadataFamily::ErrorCategory]
        .iter()
        .map(|d| ErrorCategoryView::from_set(&sets[d], p))
        .collect();
    assert!(cats.iter().all(|c| c.brief.is_some()));
    census.count_vs_spec(
        "§13.2 with a sound",
        cats.iter().filter(|c| c.sound.is_some()).count(),
        673,
    );
    census.count_vs_spec(
        "§13.2 with a script",
        cats.iter().filter(|c| c.script.is_some()).count(),
        2,
    );
    census.count_vs_spec(
        "§13.2 with an icon",
        cats.iter().filter(|c| c.icon.is_some()).count(),
        1,
    );
    let ec = ErrorControlView::from_set(&control(controls.error), p);
    census.count_vs_spec("§13.2 control categories", ec.categories.len(), 675);
    let keys: Vec<u32> = ec
        .categories
        .iter()
        .flat_map(|(k, _)| k.iter().copied())
        .collect();
    census.count_vs_spec("§13.2 error keys", keys.len(), 950);
    census.count_vs_spec(
        "§13.2 distinct error keys",
        keys.iter().collect::<BTreeSet<_>>().len(),
        950,
    );
    let named: BTreeSet<Did> = ec.categories.iter().filter_map(|&(_, d)| d).collect();
    census.count_vs_spec("§13.2 named categories", named.len(), 674);
    let default = ec.default_category.expect("default category");
    assert!(!named.contains(&default));
    let all_cats: BTreeSet<Did> = members[&MetadataFamily::ErrorCategory]
        .iter()
        .copied()
        .collect();
    assert!(all_cats.contains(&default) && named.is_subset(&all_cats));

    // §13.3.
    census.count_vs_spec(
        "§13.3 tutorial hints",
        count(MetadataFamily::TutorialHint),
        337,
    );
    let hints: Vec<TutorialHintView<'_>> = members[&MetadataFamily::TutorialHint]
        .iter()
        .map(|d| TutorialHintView::from_set(&sets[d], p))
        .collect();
    census.count_vs_spec(
        "§13.3 distinct hint types",
        hints
            .iter()
            .filter_map(|h| h.hint_type)
            .collect::<BTreeSet<_>>()
            .len(),
        337,
    );
    assert!(
        hints
            .iter()
            .all(|h| h.sections.len() == 3 && h.mp_sections.len() == 3)
    );
    census.count_vs_spec(
        "§13.3 with MP overrides",
        hints
            .iter()
            .filter(|h| h.use_mp_overrides == Some(true))
            .count(),
        16,
    );
    census.count_vs_spec(
        "§13.3 with a minimum server level",
        hints
            .iter()
            .filter(|h| h.min_server_max_level.is_some())
            .count(),
        98,
    );
    census.count_vs_spec(
        "§13.3 with a world property",
        hints.iter().filter(|h| h.world_property.is_some()).count(),
        4,
    );
    let tc_set = control(controls.tutorial_hint);
    let tc = TutorialHintControlView::from_set(&tc_set, p);
    census.count_vs_spec("§13.3 control hints", tc.hints.len(), 331);
    census.count_vs_spec("§13.3 control MP hints", tc.mp_hints.len(), 50);
    census.count_vs_spec("§13.3 control session hints", tc.session_hints.len(), 4);
    census.count_vs_spec("§13.3 control skill hints", tc.skill_hints.len(), 0);
    census.count_vs_spec("§13.3 control level hints", tc.level_hints.len(), 20);
    assert_eq!(
        tc.all_hints(),
        members[&MetadataFamily::TutorialHint]
            .iter()
            .copied()
            .collect()
    );

    // §13.4.
    census.count_vs_spec("§13.4 movies", count(MetadataFamily::MovieLibrary), 85);
    let movies: Vec<MovieView<'_>> = members[&MetadataFamily::MovieLibrary]
        .iter()
        .map(|d| MovieView::from_set(&sets[d], p))
        .collect();
    census.count_vs_spec(
        "§13.4 with a movie ID",
        movies.iter().filter(|m| m.movie_id.is_some()).count(),
        65,
    );
    census.count_vs_spec(
        "§13.4 with a quest",
        movies.iter().filter(|m| m.quest.is_some()).count(),
        65,
    );
    census.count_vs_spec(
        "§13.4 distinct icons",
        movies
            .iter()
            .filter_map(|m| m.icon)
            .collect::<BTreeSet<_>>()
            .len(),
        1,
    );
    let mc = MovieLibraryControlView::from_set(&control(controls.movie_library), p);
    census.count_vs_spec("§13.4 control categories", mc.categories.len(), 48);
    census.count_vs_spec("§13.4 control movies", mc.movies.len(), 85);
    assert_eq!(
        mc.movies.iter().copied().collect::<BTreeSet<_>>(),
        members[&MetadataFamily::MovieLibrary]
            .iter()
            .copied()
            .collect()
    );

    census.count_vs_spec(
        "§13.4 with an icon",
        movies.iter().filter(|m| m.icon.is_some()).count(),
        85,
    );

    // §13.5: members are the sets with a non-referrer `PropertyResponse*` property. The
    // five characteristic arrays give 237 + 120 + 21 + 9; 32 members carry only callback
    // settings (kind Other).
    census.count_vs_spec(
        "§13.5 property responses",
        count(MetadataFamily::PropertyResponse),
        419,
    );
    let kinds = histogram(
        members[&MetadataFamily::PropertyResponse]
            .iter()
            .map(|d| classifier.response_kind(&sets[d]).expect("member kind")),
    );
    eprintln!("property responses by kind: {kinds:?}");
    hist(
        &mut census,
        "§13.5 responses of kind",
        &kinds,
        &[
            (PropertyResponseKind::AppliedEffects, 237),
            (PropertyResponseKind::WorldEvents, 120),
            (PropertyResponseKind::MapProperties, 21),
            (PropertyResponseKind::CalculateProperties, 9),
            (PropertyResponseKind::Other, 32),
        ],
    );
    // The four monster objects that refer to a response map are not members.
    let only_referrers: Vec<&Found> = referrers
        .iter()
        .filter(|(_, _, set)| classifier.response_kind(set).is_none())
        .collect();
    census.count_vs_spec("§13.5 referrers only", only_referrers.len(), 4);
    assert!(
        only_referrers
            .iter()
            .all(|(f, ..)| *f != Some(MetadataFamily::PropertyResponse))
    );
    assert!(
        referrers
            .iter()
            .all(|(f, ..)| *f != Some(MetadataFamily::PropertyResponse)),
        "a response member also refers to a response map"
    );

    // §13.6.
    census.count_vs_spec(
        "§13.6 game callbacks",
        count(MetadataFamily::GameCallback),
        110,
    );
    let returns = histogram(
        members[&MetadataFamily::GameCallback]
            .iter()
            .filter_map(|d| game_callback_return_value(&sets[d], p)),
    );
    let mut return_counts: Vec<usize> = returns.values().copied().collect();
    return_counts.sort_unstable_by(|a, b| b.cmp(a));
    list(
        &mut census,
        "§13.6 return values by frequency",
        &return_counts,
        &[82, 19, 5, 1, 1, 1, 1],
    );
    // Exactly one property each, except one object with a second.
    let sizes = histogram(
        members[&MetadataFamily::GameCallback]
            .iter()
            .map(|d| sets[d].iter().count()),
    );
    hist(
        &mut census,
        "§13.6 callbacks with property count",
        &sizes,
        &[(1, 109), (2, 1)],
    );
    // §13.7 (X2): 124 objects carry `Version_*` properties: 123 members and the
    // VersionSystem control, which the classifier leaves out.
    census.count_vs_spec(
        "§13.7 objects with Version properties",
        count(MetadataFamily::Version) + 1,
        124,
    );
    let vs_set = control(controls.version_system);
    assert!(vs_set.iter().any(|e| classifier.version.contains(&e.pid)));
    assert_eq!(classifier.classify(&vs_set), None);
    assert!(classifier.is_version_control(&vs_set));
    let vs = VersionSystemView::from_set(&vs_set, p);
    census.count_vs_spec("§13.7 updates", vs.updates.len(), 93);
    census.count_vs_spec(
        "§13.7 always run on players",
        vs.always_run_on_players.len(),
        2,
    );
    assert!(vs.current.is_some());
    // 92 structs carry `Version_SingletonList`, one the account list (one DID).
    census.count_vs_spec("§13.7 account updates", vs.account_updates.len(), 1);
    census.count_vs_spec(
        "§13.7 account update DIDs",
        vs.account_updates.iter().map(|u| u.1.len()).sum::<usize>(),
        1,
    );
    // The candidates (155 + 2 distinct DIDs) cover every member; 34 are not members.
    let version_members: BTreeSet<Did> =
        members[&MetadataFamily::Version].iter().copied().collect();
    let candidates = vs.member_candidates();
    census.count_vs_spec("§13.7 member candidates", candidates.len(), 157);
    let candidate_set: BTreeSet<Did> = candidates.iter().copied().collect();
    assert!(
        version_members.is_subset(&candidate_set),
        "§13.7: the control names every member"
    );
    census.count_vs_spec(
        "§13.7 candidates that are not members",
        candidate_set.len() - version_members.len(),
        34,
    );
    census.finish();
}
