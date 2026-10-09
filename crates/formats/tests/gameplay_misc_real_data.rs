// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks of the miscellaneous record decoders (`tessera_formats::gameplay::misc`)
//! against a user's own install, read-only: every record of each family is decoded and the
//! census counts and invariants of `docs/specs/misc-records.md` are checked.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. The counts are those the spec
//! records for the install it was written against (§2, §3.2, §4.2, §5, §7, §9–§13); a
//! different client build may differ, and a failure then names the count that moved.
//! Nothing read from the archives is printed except counts.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use tessera_dat::ArchiveSet;
use tessera_formats::gameplay::misc::*;
use tessera_formats::{
    Did, EntityDescriptor, PropertyDictionary, PropertySetRecord, Record, RgbaImage, Surface,
    SurfaceFormat, TextureList,
};
use tessera_wstate::ObjectStore;

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn open() -> Option<ObjectStore> {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return None;
    };
    Some(ObjectStore::open(&dir).expect("open install"))
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
    let Some(store) = open() else { return };
    let data = store.game_data();
    let recs = records(data.source(), 0x3300_0000, 0x3400_0000);
    assert_eq!(recs.len(), 24, "§2: 24 compositors");
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
    assert_eq!(
        layer_hist,
        BTreeMap::from([(2, 4), (3, 1), (7, 11), (8, 2), (9, 2), (11, 4)])
    );
    let layers: Vec<&CompositorLayer> = comps.iter().flat_map(|c| &c.layers).collect();
    assert_eq!(layers.len(), 166);
    let options: Vec<&CompositorOption> = layers.iter().flat_map(|l| &l.options).collect();
    assert_eq!(options.len(), 1811);
    assert!(layers.iter().all(|l| (1..=40).contains(&l.options.len())));
    assert_eq!(
        histogram(options.iter().map(|o| o.colours.len())),
        BTreeMap::from([(0, 307), (1, 1082), (2, 422)])
    );
    let entries: Vec<&ColourEntry> = options.iter().flat_map(|o| &o.colours).collect();
    assert_eq!(entries.len(), 1926);
    assert!(options.iter().all(|o| !o.legacy_did.is_null()));
    let zero_mask: Vec<&&ColourEntry> = entries.iter().filter(|c| c.mask_did.is_null()).collect();
    assert_eq!(zero_mask.len(), 121);
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
    assert_eq!(zero_mask_keys, BTreeSet::from([(0x1000_0003, 0x1000_0007)]));
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
    assert_eq!(flagged.len(), 3);
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
    assert_eq!(contiguous, 165);
    assert_eq!(options.iter().map(|o| o.x).max(), Some(476));
    assert_eq!(options.iter().map(|o| o.y).max(), Some(701));

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
    assert_eq!(legacy.len(), 2977);
    assert_eq!(deps.len(), 2977);
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
    assert_eq!(
        image_formats,
        BTreeMap::from([
            (SurfaceFormat::A8R8G8B8.name(), 1700),
            (SurfaceFormat::R8G8B8.name(), 111)
        ])
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
    assert_eq!(
        by_layer(true),
        BTreeMap::from([
            (SurfaceFormat::A8R8G8B8.name(), 46),
            (SurfaceFormat::R8G8B8.name(), 111)
        ])
    );
    assert_eq!(
        by_layer(false),
        BTreeMap::from([(SurfaceFormat::A8R8G8B8.name(), 1654)])
    );
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
    assert_eq!(masks, 1805);

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
    assert_eq!(off_canvas.len(), 8);
    assert_eq!(off_canvas.iter().collect::<BTreeSet<_>>().len(), 1);

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
    assert_eq!(faces.len(), 7);
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
}

/// §3.7, C8: the character-creation compositor controls on entity descriptors.
#[test]
fn compositor_controls() {
    let Some(store) = open() else { return };
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
    assert_eq!(descriptors, 40_643);
    assert_eq!(controls.len(), 218);
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
}

// ---------------------------------------------------------------------------------------------
// §4 UI scenes
// ---------------------------------------------------------------------------------------------

#[test]
fn ui_scenes() {
    let Some(store) = open() else { return };
    let dict = dictionary(&store);
    let recs = records(store.game_data().source(), 0x2100_0000, 0x2200_0000);
    assert_eq!(recs.len(), 20);
    let mut scenes = Vec::new();
    for (_, did, payload) in &recs {
        scenes.push(
            UiScene::parse_expecting(payload, *did, &dict).unwrap_or_else(|e| panic!("{did}: {e}")),
        );
    }
    let seds: Vec<_> = scenes.iter().flat_map(|s| &s.entities).collect();
    assert_eq!(seds.len(), 3051);
    assert!(scenes.iter().all(|s| (2..=732).contains(&s.entities.len())));
    assert!(
        scenes.iter().all(|s| s.colour == 0
            && !s.flag
            && s.value_a == 0xBF80_0000
            && s.value_b == 0xBF80_0000)
    );
    // U2: flag and type census.
    let bit = |b: u32| seds.iter().filter(|e| e.flags & b != 0).count();
    assert_eq!(
        [
            0x001, 0x002, 0x004, 0x008, 0x010, 0x020, 0x040, 0x080, 0x100, 0x200, 0x400
        ]
        .map(bit),
        // Bit 0x010: 3,026, as the flag-word census below implies (U2, corrected).
        [0, 3051, 323, 2969, 3026, 0, 0, 2099, 3041, 0, 0]
    );
    assert_eq!(
        histogram(seds.iter().map(|e| e.flags)),
        BTreeMap::from([
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
        ])
    );
    assert_eq!(
        histogram(seds.iter().filter_map(|e| e.entity_type)),
        BTreeMap::from([(2, 7), (7, 2842), (9, 120), (27, 82)])
    );
    assert_eq!(histogram(seds.iter().map(|e| e.flags)).len(), 14);
    let objects: Vec<Did> = scenes.iter().flat_map(UiScene::objects).collect();
    assert_eq!(objects.len(), 2969);
    assert!(objects.iter().all(|d| d.prefix() == 0x47));
    eprintln!(
        "UI scenes: {} records, {} descriptors",
        scenes.len(),
        seds.len()
    );
}

// ---------------------------------------------------------------------------------------------
// §5 imposter tables
// ---------------------------------------------------------------------------------------------

#[test]
fn imposter_tables() {
    let Some(store) = open() else { return };
    let data = store.game_data();
    let recs = records(data.source(), 0x2700_0000, 0x2800_0000);
    assert_eq!(recs.len(), 507);
    let mut tables = Vec::new();
    for (_, did, payload) in &recs {
        let t =
            ImposterTable::parse_expecting(payload, *did).unwrap_or_else(|e| panic!("{did}: {e}"));
        assert_eq!(t.to_bytes(), *payload);
        tables.push(t);
    }
    assert_eq!(
        histogram(tables.iter().map(|t| t.square().expect("square").region)),
        BTreeMap::from([(1, 100), (2, 94), (3, 100), (4, 100), (5, 100), (14, 13)])
    );
    assert_eq!(
        tables.iter().map(|t| t.entries.len()).sum::<usize>(),
        12_406
    );
    assert!(tables.iter().all(|t| t.entries.len() <= 128));
    // MR5 for every table.
    for t in &tables {
        assert_eq!(t.first_unusual_field(), None, "{}", t.did);
    }
    assert_eq!(tables.iter().filter(|t| t.entries.len() >= 2).count(), 289);
    let empty: Vec<&ImposterTable> = tables.iter().filter(|t| t.entries.is_empty()).collect();
    assert_eq!(empty.len(), 207);
    let all_zero = empty
        .iter()
        .filter(|t| {
            t.colour_atlas.is_null()
                && t.second_atlas.is_null()
                && t.cell_u == 0.0
                && t.cell_v == 0.0
        })
        .count();
    assert_eq!(all_zero, 204);
    assert_eq!(
        empty
            .iter()
            .filter(|t| t.dependencies().len() == 2 && t.cell_u == 0.0 && t.cell_v == 0.0)
            .count(),
        3
    );

    // I3: the atlases of the 300 non-empty tables.
    let full: Vec<&ImposterTable> = tables.iter().filter(|t| !t.entries.is_empty()).collect();
    assert_eq!(full.len(), 300);
    let mut lists = 0;
    for t in &full {
        let (w, h) = t.atlas_texels().expect("grid");
        let mut formats = Vec::new();
        for list_did in t.dependencies() {
            let bytes = read(&store, list_did);
            let list = TextureList::parse_expecting(&bytes, list_did).expect("texture list");
            let levels = list.levels().expect("mip chain");
            assert_eq!(levels.len(), 4, "{list_did}");
            let top = read(&store, levels[0]);
            let s = Surface::parse(&top).expect("surface");
            assert_eq!((s.width, s.height), (w, h), "{}", t.did);
            formats.push(s.format);
            lists += 1;
        }
        assert_eq!(formats, vec![SurfaceFormat::Dxt5, SurfaceFormat::Dxt1]);
    }
    assert_eq!(lists, 600);

    // I4: keys resolve.
    let keys: BTreeSet<u32> = tables
        .iter()
        .flat_map(|t| t.entries.iter().map(|e| e.name_key))
        .collect();
    assert_eq!(keys.len(), 816);
    let mapper = data
        .lookup_path(&["EMAPPER", enum_mapper_labels::RENDER_IMPOSTER_NAMES])
        .expect("EMAPPER")
        .expect("RenderImposterNames");
    for k in &keys {
        assert!(data.enum_label(mapper, *k).expect("mapper").name.is_some());
    }
    eprintln!(
        "imposter tables: {} records, {} entries",
        tables.len(),
        12_406
    );
}

// ---------------------------------------------------------------------------------------------
// §6, §7, §9, §10, §11 singletons
// ---------------------------------------------------------------------------------------------

#[test]
fn compute_shaders() {
    let Some(store) = open() else { return };
    let recs = records(store.game_data().source(), 0x2D00_0000, 0x2E00_0000);
    assert_eq!(recs.len(), 3);
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
    assert_eq!(sizes, BTreeSet::from([1664, 2532, 20_780]));
}

#[test]
fn script_channels() {
    let Some(store) = open() else { return };
    let data = store.game_data();
    let did = unique_db(&store, unique_db_labels::SCRIPT_CHANNELS);
    assert_eq!(did.prefix(), SCRIPT_CHANNELS_PREFIX);
    let all = dids(data.source(), 0x4300_0000, 0x4400_0000);
    assert_eq!(all.len(), 1);
    let payload = read(&store, did);
    assert_eq!(payload.len(), 3285);
    let sc = ScriptChannels::parse(&payload).expect("script channels");
    assert_eq!(sc.to_bytes(), payload);
    assert_eq!(sc.size.hint, 0);
    assert_eq!(sc.channels.len(), 4);
    assert_eq!(
        sc.channels
            .iter()
            .map(|c| c.scripts.len())
            .collect::<Vec<_>>(),
        vec![296, 34, 13, 2]
    );
    assert_eq!(
        sc.channels
            .iter()
            .map(|c| c.scripts_size.hint)
            .collect::<Vec<_>>(),
        vec![4, 1, 0, 0]
    );
    assert_eq!(
        sc.channels
            .iter()
            .map(|c| c.transitions.len())
            .collect::<Vec<_>>(),
        vec![0, 0, 12, 0]
    );
    assert!(sc.channels.iter().all(|c| c.exclusive));
    assert!(sc.global_transitions.is_empty());
    let scripts: Vec<&ChannelScript> = sc.channels.iter().flat_map(|c| &c.scripts).collect();
    assert_eq!(scripts.iter().filter(|s| s.interruptible).count(), 343);
    assert_eq!(scripts.iter().filter(|s| !s.interruptible).count(), 2);
    assert_eq!(
        scripts.iter().map(|s| s.priority).collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 1, 2, 10_000])
    );
    assert_eq!(
        sc.channels
            .iter()
            .map(|c| c.channel_key)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([1, 0x1000_0001, 0x1000_0003, 0x1000_0005])
    );
    // §7.3: 346 distinct script keys over scripts and transitions.
    assert_eq!(sc.script_keys().len(), 346);
    let fx = data
        .lookup_path(&["EMAPPER", enum_mapper_labels::FX])
        .expect("EMAPPER")
        .expect("FX");
    for k in sc.script_keys() {
        assert!(data.enum_label(fx, k).expect("FX").name.is_some(), "{k:#x}");
    }
}

#[test]
fn name_filter_and_taboo_tables() {
    let Some(store) = open() else { return };
    let did = unique_db(&store, unique_db_labels::NAME_FILTER_TABLE);
    let payload = read(&store, did);
    assert_eq!(payload.len(), 2685);
    let nf = NameFilterTable::parse_expecting(&payload, did).expect("name filter");
    assert_eq!(nf.size.hint, 1);
    assert_eq!(
        nf.rule_sets
            .iter()
            .map(|r| r.rule_key)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([1, 2, 0x1000_0003])
    );
    for r in &nf.rule_sets {
        assert_eq!(r.classes_size.hint, 1);
        assert_eq!(r.character_classes.len(), 13);
        let keys: Vec<u32> = r.character_classes.iter().map(|c| c.key).collect();
        assert!(keys.iter().all(|k| k.is_power_of_two() && *k <= 4096));
        assert_eq!(keys.iter().collect::<BTreeSet<_>>().len(), 13);
        assert!(r.character_classes.iter().all(|c| c.characters.len() <= 62));
        assert!(r.string_b.is_empty());
        assert_eq!(r.string_a.len(), 3);
        assert_eq!(r.string_list_a.len(), 4);
        assert!(!r.flag_b);
    }
    assert_eq!(
        nf.rule_sets.iter().map(|r| r.flag_a).collect::<Vec<_>>(),
        vec![true, false, true]
    );
    assert_eq!(
        nf.rule_sets
            .iter()
            .map(|r| r.u32_list.len())
            .collect::<Vec<_>>(),
        vec![4, 0, 3]
    );

    let did = unique_db(&store, unique_db_labels::TABOO_TABLE);
    let payload = read(&store, did);
    assert_eq!(payload.len(), 385_598);
    let tb = TabooTable::parse_expecting(&payload, did).expect("taboo");
    assert_eq!(tb.headers.len(), 8);
    assert_eq!(tb.lists_size.hint, 1);
    assert!(tb.keys_match_headers(), "MR8");
    assert_eq!(tb.pattern_count(), 17_612);
    let sizes: Vec<usize> = tb.lists.iter().map(|l| l.patterns.len()).collect();
    assert_eq!(sizes.iter().min(), Some(&39));
    assert_eq!(sizes.iter().max(), Some(&6708));
}

#[test]
fn compression_dictionaries() {
    let Some(store) = open() else { return };
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
    assert_eq!(targets.len(), 3);
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
    assert_eq!(
        seen.values().map(|e| e.body_len).collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 214_909, 218_203])
    );
    assert_eq!(
        seen.values().map(|e| e.id_a).collect::<BTreeSet<_>>().len(),
        1
    );
    assert_eq!(
        seen.values().map(|e| e.id_b).collect::<BTreeSet<_>>().len(),
        2
    );
}

// ---------------------------------------------------------------------------------------------
// §12 bare property sets
// ---------------------------------------------------------------------------------------------

#[test]
fn bare_property_sets() {
    let Some(store) = open() else { return };
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
    assert_eq!(
        per_archive,
        BTreeMap::from([("gamelogic", 1616), ("local", 2)])
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
    assert_eq!(empty, 9);
    assert_eq!(
        families,
        BTreeMap::from([
            (BareSetFamily::Empty, 8),
            (BareSetFamily::GameMap, 1300),
            (BareSetFamily::LocalSky, 147),
            (BareSetFamily::FootstepSounds, 38),
            (BareSetFamily::AmbientMusic, 25),
            (BareSetFamily::ContentLayerDefaults, 13),
            (BareSetFamily::Volume, 9),
            (BareSetFamily::Other, 76),
        ])
    );
    assert_eq!(light_maps, 21);
    assert_eq!(fog, 7);
}

// ---------------------------------------------------------------------------------------------
// §13 metadata objects, §14 WEENIECONTENT
// ---------------------------------------------------------------------------------------------

#[test]
fn metadata_objects() {
    use tessera_formats::gameplay::common::PropertyAccess;
    let Some(store) = open() else { return };
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
    assert_eq!(weenie.len(), 316);
    let targets = histogram(weenie.iter().map(|e| e.value.prefix()));
    assert_eq!(targets.get(&0x70), Some(&314));
    assert_eq!(targets.get(&0x07), Some(&1));
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
    assert_eq!(listed.len(), 5211);
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
    assert_eq!(index.len(), 5211);
    assert_eq!(percent, 2094);
    assert_eq!(forwarded, (364, 352));
    assert_eq!(operators, 4498 + 4415 + 4048 + 2474 + 539 + 44 + 12);

    // §13.2.
    assert_eq!(count(MetadataFamily::ErrorCategory), 675);
    let cats: Vec<ErrorCategoryView<'_>> = members[&MetadataFamily::ErrorCategory]
        .iter()
        .map(|d| ErrorCategoryView::from_set(&sets[d], p))
        .collect();
    assert!(cats.iter().all(|c| c.brief.is_some()));
    assert_eq!(cats.iter().filter(|c| c.sound.is_some()).count(), 673);
    assert_eq!(cats.iter().filter(|c| c.script.is_some()).count(), 2);
    assert_eq!(cats.iter().filter(|c| c.icon.is_some()).count(), 1);
    let ec = ErrorControlView::from_set(&control(controls.error), p);
    assert_eq!(ec.categories.len(), 675);
    let keys: Vec<u32> = ec
        .categories
        .iter()
        .flat_map(|(k, _)| k.iter().copied())
        .collect();
    assert_eq!(keys.len(), 950);
    assert_eq!(keys.iter().collect::<BTreeSet<_>>().len(), 950);
    let named: BTreeSet<Did> = ec.categories.iter().filter_map(|&(_, d)| d).collect();
    assert_eq!(named.len(), 674);
    let default = ec.default_category.expect("default category");
    assert!(!named.contains(&default));
    let all_cats: BTreeSet<Did> = members[&MetadataFamily::ErrorCategory]
        .iter()
        .copied()
        .collect();
    assert!(all_cats.contains(&default) && named.is_subset(&all_cats));

    // §13.3.
    assert_eq!(count(MetadataFamily::TutorialHint), 337);
    let hints: Vec<TutorialHintView<'_>> = members[&MetadataFamily::TutorialHint]
        .iter()
        .map(|d| TutorialHintView::from_set(&sets[d], p))
        .collect();
    assert_eq!(
        hints
            .iter()
            .filter_map(|h| h.hint_type)
            .collect::<BTreeSet<_>>()
            .len(),
        337
    );
    assert!(
        hints
            .iter()
            .all(|h| h.sections.len() == 3 && h.mp_sections.len() == 3)
    );
    assert_eq!(
        hints
            .iter()
            .filter(|h| h.use_mp_overrides == Some(true))
            .count(),
        16
    );
    assert_eq!(
        hints
            .iter()
            .filter(|h| h.min_server_max_level.is_some())
            .count(),
        98
    );
    assert_eq!(
        hints.iter().filter(|h| h.world_property.is_some()).count(),
        4
    );
    let tc_set = control(controls.tutorial_hint);
    let tc = TutorialHintControlView::from_set(&tc_set, p);
    assert_eq!(
        (
            tc.hints.len(),
            tc.mp_hints.len(),
            tc.session_hints.len(),
            tc.skill_hints.len()
        ),
        (331, 50, 4, 0)
    );
    assert_eq!(tc.level_hints.len(), 20);
    assert_eq!(
        tc.all_hints(),
        members[&MetadataFamily::TutorialHint]
            .iter()
            .copied()
            .collect()
    );

    // §13.4.
    assert_eq!(count(MetadataFamily::MovieLibrary), 85);
    let movies: Vec<MovieView<'_>> = members[&MetadataFamily::MovieLibrary]
        .iter()
        .map(|d| MovieView::from_set(&sets[d], p))
        .collect();
    assert_eq!(movies.iter().filter(|m| m.movie_id.is_some()).count(), 65);
    assert_eq!(movies.iter().filter(|m| m.quest.is_some()).count(), 65);
    assert_eq!(
        movies
            .iter()
            .filter_map(|m| m.icon)
            .collect::<BTreeSet<_>>()
            .len(),
        1
    );
    let mc = MovieLibraryControlView::from_set(&control(controls.movie_library), p);
    assert_eq!((mc.categories.len(), mc.movies.len()), (48, 85));
    assert_eq!(
        mc.movies.iter().copied().collect::<BTreeSet<_>>(),
        members[&MetadataFamily::MovieLibrary]
            .iter()
            .copied()
            .collect()
    );

    assert_eq!(movies.iter().filter(|m| m.icon.is_some()).count(), 85);

    // §13.5: members are the sets with a non-referrer `PropertyResponse*` property. The
    // five characteristic arrays give 237 + 120 + 21 + 9; 32 members carry only callback
    // settings (kind Other).
    assert_eq!(count(MetadataFamily::PropertyResponse), 419);
    let kinds = histogram(
        members[&MetadataFamily::PropertyResponse]
            .iter()
            .map(|d| classifier.response_kind(&sets[d]).expect("member kind")),
    );
    eprintln!("property responses by kind: {kinds:?}");
    assert_eq!(
        kinds,
        BTreeMap::from([
            (PropertyResponseKind::AppliedEffects, 237),
            (PropertyResponseKind::WorldEvents, 120),
            (PropertyResponseKind::MapProperties, 21),
            (PropertyResponseKind::CalculateProperties, 9),
            (PropertyResponseKind::Other, 32),
        ])
    );
    // The four monster objects that refer to a response map are not members.
    let only_referrers: Vec<&Found> = referrers
        .iter()
        .filter(|(_, _, set)| classifier.response_kind(set).is_none())
        .collect();
    assert_eq!(only_referrers.len(), 4);
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
    assert_eq!(count(MetadataFamily::GameCallback), 110);
    let returns = histogram(
        members[&MetadataFamily::GameCallback]
            .iter()
            .filter_map(|d| game_callback_return_value(&sets[d], p)),
    );
    let mut return_counts: Vec<usize> = returns.values().copied().collect();
    return_counts.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(return_counts, vec![82, 19, 5, 1, 1, 1, 1]);
    // Exactly one property each, except one object with a second.
    let sizes = histogram(
        members[&MetadataFamily::GameCallback]
            .iter()
            .map(|d| sets[d].iter().count()),
    );
    assert_eq!(sizes, BTreeMap::from([(1, 109), (2, 1)]));
    // §13.7 (X2): 124 objects carry `Version_*` properties: 123 members and the
    // VersionSystem control, which the classifier leaves out.
    assert_eq!(count(MetadataFamily::Version) + 1, 124);
    let vs_set = control(controls.version_system);
    assert!(vs_set.iter().any(|e| classifier.version.contains(&e.pid)));
    assert_eq!(classifier.classify(&vs_set), None);
    assert!(classifier.is_version_control(&vs_set));
    let vs = VersionSystemView::from_set(&vs_set, p);
    assert_eq!(vs.updates.len(), 93);
    assert_eq!(vs.always_run_on_players.len(), 2);
    assert!(vs.current.is_some());
    // 92 structs carry `Version_SingletonList`, one the account list (one DID).
    assert_eq!(vs.account_updates.len(), 1);
    assert_eq!(vs.account_updates[0].1.len(), 1);
    // The candidates (155 + 2 distinct DIDs) cover every member; 34 are not members.
    let version_members: BTreeSet<Did> =
        members[&MetadataFamily::Version].iter().copied().collect();
    let candidates = vs.member_candidates();
    assert_eq!(candidates.len(), 157);
    let candidate_set: BTreeSet<Did> = candidates.iter().copied().collect();
    assert!(
        version_members.is_subset(&candidate_set),
        "§13.7: the control names every member"
    );
    assert_eq!(candidate_set.len() - version_members.len(), 34);
}
