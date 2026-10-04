// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data tests against a user's own install (read-only), skipped unless
//! `TESSERA_GAME_DIR` is set:
//!
//! - the class library (0x56000000) decodes and satisfies L1–L9 (`wlib.md` §11, §14);
//! - **every** object-state record (0x70…, 0x76…) decodes; the only partial decodes are
//!   unknown native classes (`wstate.md` §15). Heavy: run with `--release`;
//! - every area, territory, region and dungeon is found by property signature, every
//!   area resolves area → territory → region with rendered names, every dungeon position
//!   is null or a position, both region-number tables agree, and every rendered name is
//!   found again by name lookup (`wstate.md` §11, §15).
//!
//! Run with `TESSERA_GAME_DIR=… cargo test --release -p tessera-wstate --test real_data --
//! --nocapture --test-threads 1`. Only counts, rates, class indices and DIDs are printed;
//! no game text leaves the install.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_text::Strings;
use tessera_wstate::{
    Body, Did, Geo, GeoIndex, GeoKind, Item, LibraryWarning, ObjectStore, StateWarning,
    is_object_did, object_dids, render_name,
};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

struct Fixture {
    store: ObjectStore<Arc<ArchiveSet>>,
    set: Arc<ArchiveSet>,
    dids: Vec<Did>,
}

fn fixture() -> Option<&'static Fixture> {
    static F: OnceLock<Option<Fixture>> = OnceLock::new();
    F.get_or_init(|| {
        let dir = game_dir()?;
        let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open install"));
        let dids = object_dids(&set).expect("list object DIDs");
        let store = ObjectStore::new(Arc::clone(&set));
        Some(Fixture { store, set, dids })
    })
    .as_ref()
}

fn pct(n: usize, d: usize) -> f64 {
    if d == 0 {
        100.0
    } else {
        n as f64 * 100.0 / d as f64
    }
}

#[test]
fn library_invariants() {
    let Some(f) = fixture() else { return };
    let t = Instant::now();
    let did = f.store.library_did();
    let lib = f.store.library().expect("class library decodes (L1-L7)");
    eprintln!(
        "library {did}: version {}, {} class indices, {} defined, {} with attribute lists, \
         {} attributes, {} singletons, {} messages, bytecode {} bytes, {:?}",
        lib.version,
        lib.len(),
        lib.defined_count(),
        lib.listed_count(),
        lib.attribute_count(),
        lib.singletons().len(),
        lib.message_count.unwrap_or(0),
        lib.bytecode_len.unwrap_or(0),
        t.elapsed()
    );
    for w in lib.warnings() {
        eprintln!("library warning: {w:?}");
    }
    assert!(
        !lib.warnings()
            .iter()
            .any(|w| matches!(w, LibraryWarning::NotFlattened { .. })),
        "L8 violated"
    );
    // L9: each singleton class has its record, whose root class is the class.
    for &c in lib.singletons() {
        let sd = lib.singleton_did(c).expect("singleton DID");
        let h = f
            .store
            .header(sd)
            .expect("singleton header")
            .unwrap_or_else(|| panic!("L9: singleton record {sd} missing"));
        assert_eq!(h.root_class, c, "L9: singleton {sd} root class");
    }
    let max_depth = lib.classes().map(|c| lib.depth(c.index)).max().unwrap_or(0);
    let roots = lib
        .classes()
        .filter(|c| lib.parent(c.index).is_none())
        .count();
    eprintln!("library: max depth {max_depth}, {roots} root classes");
}

#[derive(Default)]
struct Census {
    records: usize,
    complete: usize,
    partial: BTreeMap<u32, usize>,
    failed: Vec<(Did, String)>,
    failed_count: usize,
    absent: usize,
    items: usize,
    did_items: usize,
    script: usize,
    native: BTreeMap<u32, usize>,
    max_items: usize,
    links: usize,
    available: usize,
    imports: usize,
    root_classes: HashMap<u32, usize>,
    warnings: BTreeMap<String, usize>,
    bytes: usize,
}

impl Census {
    fn merge(&mut self, o: Census) {
        self.records += o.records;
        self.complete += o.complete;
        for (k, v) in o.partial {
            *self.partial.entry(k).or_default() += v;
        }
        self.failed_count += o.failed_count;
        self.failed.extend(o.failed);
        self.failed.truncate(20);
        self.absent += o.absent;
        self.items += o.items;
        self.did_items += o.did_items;
        self.script += o.script;
        for (k, v) in o.native {
            *self.native.entry(k).or_default() += v;
        }
        self.max_items = self.max_items.max(o.max_items);
        self.links += o.links;
        self.available += o.available;
        self.imports += o.imports;
        for (k, v) in o.root_classes {
            *self.root_classes.entry(k).or_default() += v;
        }
        for (k, v) in o.warnings {
            *self.warnings.entry(k).or_default() += v;
        }
        self.bytes += o.bytes;
    }
}

fn warning_key(w: &StateWarning) -> String {
    match w {
        StateWarning::ReservedField { field, .. } => format!("reserved field {field}"),
        StateWarning::ImportType { dbo_type } => format!("import type {dbo_type:#x}"),
        StateWarning::LibraryMismatch { .. } => "library mismatch".into(),
        StateWarning::ClassNotInTable { .. } => "class not in table".into(),
        StateWarning::RootMismatch => "root mismatch".into(),
        StateWarning::FirstLabel { .. } => "first label".into(),
        StateWarning::RootClassRange => "root class range".into(),
    }
}

#[test]
fn decode_every_object_state() {
    let Some(f) = fixture() else { return };
    // Load shared state once before the workers start.
    f.store.library().expect("library");
    f.store.game_data().registry().expect("registry");
    let t = Instant::now();
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(16);
    let next = Mutex::new(0usize);
    let total = Mutex::new(Census::default());
    const BATCH: usize = 256;
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let mut c = Census::default();
                loop {
                    let start = {
                        let mut n = next.lock().unwrap();
                        let s = *n;
                        *n += BATCH;
                        s
                    };
                    if start >= f.dids.len() {
                        break;
                    }
                    for &did in &f.dids[start..(start + BATCH).min(f.dids.len())] {
                        census_one(f, did, &mut c);
                    }
                }
                total.lock().unwrap().merge(c);
            });
        }
    });
    let c = total.into_inner().unwrap();
    eprintln!(
        "object states: {} DIDs, {} decoded exactly ({:.4}%), {} partial, {} failed, {} absent, \
         {:.1} MiB, {:?} on {threads} threads",
        c.records,
        c.complete,
        pct(c.complete, c.records),
        c.partial.values().sum::<usize>(),
        c.failed_count,
        c.absent,
        c.bytes as f64 / (1024.0 * 1024.0),
        t.elapsed()
    );
    eprintln!(
        "items {} (script objects {}, native {}, DID items {}), largest bundle {} items, \
         {} records with links, {} with available properties, {} imports, {} root classes",
        c.items,
        c.script,
        c.native.values().sum::<usize>(),
        c.did_items,
        c.max_items,
        c.links,
        c.available,
        c.imports,
        c.root_classes.len()
    );
    eprintln!("native instances by class: {:?}", c.native);
    eprintln!("partial decodes by unknown class: {:?}", c.partial);
    eprintln!("warnings: {:?}", c.warnings);
    for (did, e) in &c.failed {
        eprintln!("FAILED {did}: {e}");
    }
    let mut top: Vec<_> = c.root_classes.iter().collect();
    top.sort_by(|a, b| b.1.cmp(a.1));
    eprintln!("top root classes: {:?}", &top[..top.len().min(10)]);
    assert_eq!(c.failed_count, 0, "every object-state record must decode");
    assert_eq!(c.absent, 0, "listed DIDs must be readable");
}

fn census_one(f: &Fixture, did: Did, c: &mut Census) {
    c.records += 1;
    let bytes = match f.set.locate(did.0) {
        Ok(Some(l)) => match f.set.read_located(&l) {
            Ok(b) => b,
            Err(e) => {
                c.failed_count += 1;
                if c.failed.len() < 20 {
                    c.failed.push((did, format!("read: {e}")));
                }
                return;
            }
        },
        Ok(None) => {
            c.absent += 1;
            return;
        }
        Err(e) => {
            c.failed_count += 1;
            if c.failed.len() < 20 {
                c.failed.push((did, format!("locate: {e}")));
            }
            return;
        }
    };
    c.bytes += bytes.len();
    let state = match f.store.decode(did, &bytes) {
        Ok(s) => s,
        Err(e) => {
            c.failed_count += 1;
            if c.failed.len() < 20 {
                c.failed.push((did, e.to_string()));
            }
            return;
        }
    };
    match state.partial {
        None => c.complete += 1,
        Some(p) => *c.partial.entry(p.class).or_default() += 1,
    }
    *c.root_classes.entry(state.root_class).or_default() += 1;
    c.items += state.items().len();
    c.max_items = c.max_items.max(state.item_count());
    c.links += usize::from(state.links.is_some());
    c.available += usize::from(state.available.is_some());
    c.imports += state.imports.len();
    for it in state.items() {
        match it {
            Item::Did(d) => {
                c.did_items += 1;
                assert!(is_object_did(*d), "{did}: DID item {d} outside 0x70-0x77");
            }
            Item::Object(o) => match &o.body {
                Body::Script(_) => c.script += 1,
                Body::Native(_) => *c.native.entry(o.class).or_default() += 1,
            },
            Item::InstanceId(_) => {}
        }
    }
    // Flag-16 imports correspond one to one with DID items (wstate.md §3.2).
    let flagged = state.imports.iter().filter(|i| i.flags == 16).count();
    let dids = state.did_items().count();
    if state.is_complete() && flagged != dids {
        *c.warnings
            .entry("flag-16 imports != DID items".into())
            .or_default() += 1;
    }
    for w in &state.warnings {
        *c.warnings.entry(warning_key(w)).or_default() += 1;
    }
}

#[test]
fn resolve_areas_regions_dungeons() {
    let Some(f) = fixture() else { return };
    let dir = game_dir().expect("dir");
    let strings = Strings::open_install(&dir, Some("English")).expect("language archive");
    let geo = Geo::new(&f.store).expect("geo schema");
    let schema = geo.schema();
    assert!(schema.area_name.is_some(), "Area_Name in dictionary");
    assert!(
        schema.area_territory.is_some(),
        "Area_Territory in dictionary"
    );
    assert!(schema.dungeon_name.is_some(), "Dungeon_Name in dictionary");

    let t = Instant::now();
    let (index, report) = GeoIndex::build(&geo, Some(&strings), f.dids.iter().copied());
    eprintln!("geo index: {report:?} in {:?}", t.elapsed());
    for k in GeoKind::ALL {
        eprintln!(
            "  {k}: {} records, classes {:?}",
            index.of_kind(k).count(),
            index.classes(k)
        );
    }
    assert_eq!(report.failed, 0);
    assert_eq!(
        report.scanned,
        report.absent
            + report.failed
            + report.no_properties
            + report.skipped
            + report.properties_read,
        "every record accounted for"
    );
    assert_eq!(
        report.mismatched, 0,
        "kinds follow root classes (wstate.md §11.1)"
    );
    assert!(index.of_kind(GeoKind::Area).count() > 0, "areas found");

    // Every area: area -> territory -> region, every name renders.
    let mut areas = 0;
    let mut full_chains = 0;
    let mut names_ok = 0;
    let mut names_total = 0;
    for e in index.of_kind(GeoKind::Area) {
        areas += 1;
        let chain = geo.area_chain(e.did).expect("area chain");
        let mut ok = true;
        for info in [
            chain.area.name.as_ref(),
            chain.territory.as_ref().and_then(|t| t.name.as_ref()),
            chain.region.as_ref().and_then(|r| r.name.as_ref()),
        ] {
            names_total += 1;
            match info.and_then(|i| render_name(&strings, i)) {
                Some(_) => names_ok += 1,
                None => ok = false,
            }
        }
        if chain.territory.is_some() && chain.region.is_some() && ok {
            full_chains += 1;
        } else {
            eprintln!(
                "area {}: territory {:?} region {:?} names ok {ok}",
                e.did,
                chain.territory.as_ref().map(|t| t.did),
                chain.region.as_ref().map(|r| r.did)
            );
        }
    }
    eprintln!(
        "areas: {areas}, full chains {full_chains} ({:.2}%), names rendered {names_ok}/{names_total}",
        pct(full_chains, areas)
    );
    assert_eq!(
        full_chains, areas,
        "every area resolves to territory and region names"
    );

    // Every dungeon: name renders; position is null or a position; parent is a dungeon.
    let mut dungeons = 0;
    let mut positions = 0;
    let mut regions_seen: BTreeMap<u8, usize> = BTreeMap::new();
    let mut dungeon_names = 0;
    for e in index.of_kind(GeoKind::Dungeon) {
        dungeons += 1;
        let d = geo
            .dungeon(e.did)
            .expect("dungeon decodes, position null or position");
        if let Some(p) = d.position {
            positions += 1;
            *regions_seen.entry(p.region().unwrap_or(0)).or_default() += 1;
        }
        if d.name
            .as_ref()
            .and_then(|n| render_name(&strings, n))
            .is_some()
        {
            dungeon_names += 1;
        }
        if let Some(parent) = d.parent {
            assert_eq!(
                index.get(parent).map(|p| p.kind),
                Some(GeoKind::Dungeon),
                "{}: parent {parent} is a dungeon",
                e.did
            );
        }
    }
    eprintln!(
        "dungeons: {dungeons}, {dungeon_names} names rendered, {positions} with positions, by region {regions_seen:?}"
    );
    assert_eq!(dungeon_names, dungeons);
    // `dungeon()` fails when a dungeon root lacks the position attribute or it names
    // anything but a position, so reaching here means every root has it; most are set.
    assert!(
        positions > 0 && positions <= dungeons,
        "dungeon positions found"
    );

    // Region-number tables (§11.3): property form and object-state form agree.
    let from_props = geo.region_table().expect("region table");
    let from_state = geo
        .region_table_from_state()
        .expect("region table from state");
    let mut a: Vec<_> = from_props.iter().map(|r| (r.number, &r.name)).collect();
    let mut b: Vec<_> = from_state.iter().map(|r| (r.number, &r.name)).collect();
    a.sort_by_key(|x| x.0);
    b.sort_by_key(|x| x.0);
    eprintln!(
        "region table: {} numbers {:?}; state form {} entries",
        a.len(),
        a.iter().map(|x| x.0).collect::<Vec<_>>(),
        b.len()
    );
    assert!(!a.is_empty(), "region-control table present");
    assert_eq!(a, b, "both region tables agree");
    for (_, n) in &a {
        assert!(render_name(&strings, n).is_some(), "region name renders");
    }

    // Name lookup round trip.
    let mut found = 0;
    let mut named = 0;
    for e in index.iter() {
        let Some(name) = &e.name else { continue };
        named += 1;
        if index.find(name).iter().any(|x| x.did == e.did)
            && index
                .find(&name.to_uppercase())
                .iter()
                .any(|x| x.did == e.did)
        {
            found += 1;
        }
    }
    let ambiguous = index
        .iter()
        .filter_map(|e| e.name.as_deref())
        .filter(|n| index.find(n).len() > 1)
        .count();
    eprintln!("name lookup: {found}/{named} found by name, {ambiguous} entries share a name");
    assert_eq!(found, named);
}
