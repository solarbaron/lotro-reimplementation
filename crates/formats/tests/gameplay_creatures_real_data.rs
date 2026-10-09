// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks of `tessera_formats::gameplay::creatures` (`docs/specs/creatures.md`)
//! against a user's own install, read-only.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Walks every object record with
//! prefix 0x70 (templates are found by `WeenieType` label and class ancestry, never by class
//! index), every AI and generator-profile record, every encounter table (0x14) and every
//! landblock-info record, and checks the invariants CR1–CR15 of §11.
//!
//! The census counts of §2–§7 are observations of one install. They are compared and
//! printed; a mismatch fails the test only with `TESSERA_CENSUS=strict` (a content update
//! changes them). Invariants always fail the test.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tessera_dat::{ArchiveKind, ArchiveSet};
use tessera_formats::gameplay::common::{ObjectRoot, PropertyAccess, enum_mapper_of};
use tessera_formats::gameplay::creatures::*;
use tessera_formats::landblock::StaticEntityType;
use tessera_formats::{
    Did, EntityDescriptor, EntityHeader, EnumMapper, LandblockInfo, PropertyDictionary,
    PropertySet, PropertyValue, Record,
};
use tessera_wstate::{ClassLibrary, ObjectStore, object_dids};

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn strict() -> bool {
    std::env::var_os("TESSERA_CENSUS").is_some_and(|v| v == "strict")
}

/// Runs `f` over `items` on every core; results in input order.
fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<(usize, Vec<R>)>> = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    const CHUNK: usize = 64;
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(CHUNK, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    let end = (i + CHUNK).min(items.len());
                    let chunk: Vec<R> = items[i..end].iter().map(&f).collect();
                    out.lock().unwrap().push((i, chunk));
                }
            });
        }
    });
    let mut parts = out.into_inner().unwrap();
    parts.sort_by_key(|(i, _)| *i);
    parts.into_iter().flat_map(|(_, v)| v).collect()
}

/// Census rows: (what, spec value, measured value).
#[derive(Default)]
struct Census {
    rows: Vec<(String, u64, u64)>,
}

impl Census {
    fn check(&mut self, what: &str, spec: u64, measured: usize) {
        self.rows.push((what.to_owned(), spec, measured as u64));
    }

    fn finish(self, section: &str) {
        let bad: Vec<_> = self.rows.iter().filter(|(_, a, b)| a != b).collect();
        eprintln!(
            "{section}: {} census rows, {} differ from the spec",
            self.rows.len(),
            bad.len()
        );
        for (w, a, b) in &self.rows {
            let mark = if a == b { "  " } else { "!!" };
            eprintln!("  {mark} {w}: spec {a}, measured {b}");
        }
        if strict() {
            assert!(bad.is_empty(), "{section}: census differs: {bad:?}");
        }
    }
}

/// One 0x70 record after the first pass.
#[derive(Debug, Clone, Copy)]
struct Obj {
    did: Did,
    class: u32,
    kind: Option<TemplateKind>,
    record: Option<CreatureRecordKind>,
}

struct World {
    store: ObjectStore,
    library: std::sync::Arc<ClassLibrary>,
    dict: PropertyDictionary,
    labels: LabelIndex,
    pids: CreaturePids,
    objects: Vec<Obj>,
    subtree: CreatureSubtree,
    /// Non-template records of the AI and profile kinds, with their sets.
    records: BTreeMap<Did, (u32, CreatureRecordKind, PropertySet)>,
    kinds: RecordKindIndex,
}

impl World {
    fn set(&self, did: Did) -> Option<PropertySet> {
        let set_did = tessera_formats::object_property_set_did(did)?;
        let bytes = self.store.game_data().read(set_did).expect("read set")?;
        Some(
            tessera_formats::gameplay::common::decode_object_properties(did, &bytes, &self.dict)
                .unwrap_or_else(|e| panic!("{did}: {e}")),
        )
    }

    fn templates(&self) -> Vec<Obj> {
        self.objects
            .iter()
            .filter(|o| self.subtree.contains(&*self.library, o.class))
            .copied()
            .collect()
    }

    fn records_of(&self, kind: CreatureRecordKind) -> impl Iterator<Item = (Did, &PropertySet)> {
        self.records
            .iter()
            .filter(move |(_, (_, k, _))| *k == kind)
            .map(|(d, (_, _, s))| (*d, s))
    }

    fn class_of(&self, did: Did) -> Option<u32> {
        self.objects
            .binary_search_by_key(&did, |o| o.did)
            .ok()
            .map(|i| self.objects[i].class)
    }
}

fn world() -> Option<&'static World> {
    static W: OnceLock<Option<World>> = OnceLock::new();
    W.get_or_init(|| {
        let Some(dir) = game_dir() else {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            return None;
        };
        Some(build_world(dir))
    })
    .as_ref()
}

fn mapper_chain(store: &ObjectStore, start: Did) -> Vec<std::sync::Arc<EnumMapper>> {
    let data = store.game_data();
    let mut chain = Vec::new();
    let mut at = start;
    let mut seen = HashSet::new();
    while !at.is_null() && seen.insert(at) && chain.len() < 32 {
        let Some(m) = data.enum_mapper(at).expect("enum mapper") else {
            break;
        };
        at = m.base;
        chain.push(m);
    }
    chain
}

fn build_world(dir: PathBuf) -> World {
    let store = ObjectStore::open(&dir).expect("open install");
    let library = store.library().expect("class library");
    let data = store.game_data();
    let dict = data.registry().expect("dictionary").dictionary().clone();
    let labels = LabelIndex::from_dictionary(&dict);
    let pids = CreaturePids::resolve(&labels);
    eprintln!("creatures: unresolved labels: {:?}", pids.missing());

    let weenie_mapper =
        enum_mapper_of(&dict, pids.template.weenie_type).expect("WeenieType mapper");
    let chain = mapper_chain(&store, weenie_mapper);
    let weenie_kinds = WeenieKinds::from_mapper_chain(&chain);
    assert!(!weenie_kinds.is_empty(), "WeenieType labels found");

    let dids: Vec<Did> = object_dids(data.source())
        .expect("object DIDs")
        .into_iter()
        .filter(|d| d.prefix() == 0x70)
        .collect();
    let t0 = std::time::Instant::now();
    let walked = par_map(&dids, |&did| {
        let payload = data.read(did).expect("read").expect("listed record exists");
        let root = ObjectRoot::read_expecting(&payload, did).expect("object root");
        let set = root
            .property_set_did()
            .and_then(|s| data.read(s).expect("read set"))
            .map(|b| {
                tessera_formats::gameplay::common::decode_object_properties(did, &b, &dict)
                    .unwrap_or_else(|e| panic!("{did}: {e}"))
            })
            .unwrap_or_default();
        let kind = weenie_kinds.of_set(&set, pids.template.weenie_type);
        let record = CreatureRecordKind::of_set(&set, &pids);
        let keep = (record.is_some() || set.is_empty()).then_some(set);
        (
            Obj {
                did,
                class: root.root_class,
                kind,
                record,
            },
            keep,
        )
    });
    eprintln!(
        "creatures: first pass over {} object records in {:.1?}",
        walked.len(),
        t0.elapsed()
    );

    let mut finder = TemplateFinder::new();
    for (o, _) in &walked {
        finder.add(o.class, o.kind);
    }
    let subtree = finder
        .subtree(&*library)
        .expect("CR1: template classes share an ancestor");

    // Record kinds: by signature outside the template subtree, then by class for the rest.
    let mut kinds = RecordKindIndex::new();
    for (o, _) in &walked {
        if let Some(k) = o.record
            && !subtree.contains(&*library, o.class)
        {
            kinds.add(o.class, k);
        }
    }
    let mut records = BTreeMap::new();
    let mut objects = Vec::with_capacity(walked.len());
    for (mut o, set) in walked {
        if !subtree.contains(&*library, o.class) {
            if o.record.is_none() {
                o.record = kinds.kind_of_class(o.class);
            }
            if let (Some(k), Some(set)) = (o.record, set) {
                records.insert(o.did, (o.class, k, set));
            }
        } else {
            o.record = None;
        }
        objects.push(o);
    }
    objects.sort_by_key(|o| o.did);
    World {
        store,
        library,
        dict,
        labels,
        pids,
        objects,
        subtree,
        records,
        kinds,
    }
}

fn median(v: &mut [usize]) -> usize {
    v.sort_unstable();
    v.get(v.len() / 2).copied().unwrap_or(0)
}

// ---------------------------------------------------------------------------------------------
// §2, §3, §4: templates
// ---------------------------------------------------------------------------------------------

#[test]
fn templates_census_and_store() {
    let Some(w) = world() else { return };
    let mut c = Census::default();
    let missing = w.pids.missing();
    assert!(
        missing.is_empty(),
        "labels the dictionary does not define: {missing:?}"
    );

    // CR1 and §2.1.
    let templates = w.templates();
    let mut by_kind: BTreeMap<TemplateKind, usize> = BTreeMap::new();
    let mut classes: BTreeMap<u32, usize> = BTreeMap::new();
    let mut kind_classes: BTreeMap<TemplateKind, BTreeSet<u32>> = BTreeMap::new();
    for o in &templates {
        *classes.entry(o.class).or_default() += 1;
        if let Some(k) = o.kind {
            *by_kind.entry(k).or_default() += 1;
            kind_classes.entry(k).or_default().insert(o.class);
        }
    }
    for o in &w.objects {
        if o.kind.is_some() {
            assert!(w.subtree.contains(&*w.library, o.class), "CR1: {}", o.did);
        }
    }
    c.check("templates (§2.1 total)", 38_651, templates.len());
    c.check("root classes in the subtree", 10, classes.len());
    c.check(
        "mob templates",
        20_161,
        by_kind.get(&TemplateKind::Mob).copied().unwrap_or(0),
    );
    c.check(
        "NPC templates (RealNPC 16,621 + NPC 6)",
        16_627,
        by_kind.get(&TemplateKind::Npc).copied().unwrap_or(0),
    );
    c.check(
        "container templates",
        1_722,
        by_kind.get(&TemplateKind::Container).copied().unwrap_or(0),
    );
    c.check(
        "classes with Monster (C2)",
        1,
        kind_classes
            .get(&TemplateKind::Mob)
            .map_or(0, BTreeSet::len),
    );
    c.check(
        "classes with GameplayContainer (C2)",
        1,
        kind_classes
            .get(&TemplateKind::Container)
            .map_or(0, BTreeSet::len),
    );
    let mob_class = kind_classes
        .get(&TemplateKind::Mob)
        .and_then(|s| s.iter().next().copied());
    let container_class = kind_classes
        .get(&TemplateKind::Container)
        .and_then(|s| s.iter().next().copied());
    // The NPC class is the one holding most NPC templates.
    let mut npc_counts: BTreeMap<u32, usize> = BTreeMap::new();
    for o in &templates {
        if o.kind == Some(TemplateKind::Npc) {
            *npc_counts.entry(o.class).or_default() += 1;
        }
    }
    let npc_class = npc_counts.iter().max_by_key(|(_, n)| **n).map(|(c, _)| *c);

    // Pass 2: class constants.
    let mirrors = MirrorTable::from_labels(&w.labels);
    let mut builder = ClassConstantsBuilder::new(&mirrors);
    for chunk in templates.chunks(4096) {
        let sets = par_map(chunk, |o| w.set(o.did).unwrap_or_default());
        for (o, s) in chunk.iter().zip(&sets) {
            builder.add(o.class, s);
        }
    }
    let constants = builder.finish();
    let mut store = TemplateStore::new(mirrors.clone(), constants);

    // Pass 3: store, reconstruct, mirrors, template views, object records.
    let p = &w.pids.template;
    let speed = p.movement_speed.id();
    let fs_speed = w.labels.pid("ForwardSource_Movement_SpeedMultiplier");
    #[derive(Default)]
    struct Acc {
        mirror_occ: BTreeMap<MirrorPattern, [usize; 4]>, // total, equal, base absent, differ
        base_absent_names: BTreeSet<u32>,
        mirrors_per_set: BTreeMap<u32, BTreeSet<usize>>,
        genus_one_bit: usize,
        genus_present: usize,
        classification_checked: usize,
        classification_disagree: usize,
        level_in_mob_npc: usize,
        containers_with_level: usize,
        imports: usize,
        import_mismatch: usize,
        no_links: usize,
        no_available: usize,
        action_elements: usize,
        action_to_table: usize,
        action_null: usize,
        action_other: usize,
        min_minus_one: usize,
        max_minus_one: usize,
        attitude_mob: BTreeSet<Did>,
        attitude_npc: BTreeSet<Did>,
        speed_absent_with_mirror: usize,
        speed_lookup_wrong: usize,
        reconstructed: usize,
        varying: BTreeMap<u32, Vec<usize>>,
    }
    impl Acc {
        fn merge(&mut self, o: Acc) {
            for (k, v) in o.mirror_occ {
                let r = self.mirror_occ.entry(k).or_default();
                for (a, b) in r.iter_mut().zip(v) {
                    *a += b;
                }
            }
            self.base_absent_names.extend(o.base_absent_names);
            for (k, v) in o.mirrors_per_set {
                self.mirrors_per_set.entry(k).or_default().extend(v);
            }
            for (k, v) in o.varying {
                self.varying.entry(k).or_default().extend(v);
            }
            self.attitude_mob.extend(o.attitude_mob);
            self.attitude_npc.extend(o.attitude_npc);
            self.genus_one_bit += o.genus_one_bit;
            self.genus_present += o.genus_present;
            self.classification_checked += o.classification_checked;
            self.classification_disagree += o.classification_disagree;
            self.level_in_mob_npc += o.level_in_mob_npc;
            self.containers_with_level += o.containers_with_level;
            self.imports += o.imports;
            self.import_mismatch += o.import_mismatch;
            self.no_links += o.no_links;
            self.no_available += o.no_available;
            self.action_elements += o.action_elements;
            self.action_to_table += o.action_to_table;
            self.action_null += o.action_null;
            self.action_other += o.action_other;
            self.min_minus_one += o.min_minus_one;
            self.max_minus_one += o.max_minus_one;
            self.speed_absent_with_mirror += o.speed_absent_with_mirror;
            self.speed_lookup_wrong += o.speed_lookup_wrong;
            self.reconstructed += o.reconstructed;
        }
    }
    let mut acc = Acc::default();
    let classification_of: HashMap<Did, Classification> = w
        .records_of(CreatureRecordKind::Classification)
        .map(|(d, s)| (d, Classification::read(s, &w.pids.ai)))
        .collect();
    let action_table_classes: BTreeSet<u32> = w
        .kinds
        .classes_of(CreatureRecordKind::ActionTable)
        .into_iter()
        .map(|(c, _)| c)
        .collect();
    for chunk in templates.chunks(4096) {
        let decoded = par_map(chunk, |o| {
            let set = w.set(o.did).unwrap_or_default();
            let bytes = w
                .store
                .game_data()
                .read(o.did)
                .expect("read")
                .expect("exists");
            let state = w.store.decode(o.did, &bytes).expect("object state");
            (set, state)
        });
        for (o, (set, _)) in chunk.iter().zip(&decoded) {
            store.insert(o.did, o.class, set);
        }
        let store_ref = &store;
        let items: Vec<_> = chunk.iter().zip(&decoded).collect();
        let parts = par_map(&items, |&(o, (set, state))| {
            let mut acc = Acc::default();
            let mut want = set.entries.clone();
            want.sort_by_key(|e| e.pid);
            assert_eq!(store_ref.reconstruct(o.did), Some(want), "CR13: {}", o.did);
            acc.reconstructed += 1;

            // §3.2.1, CR4.
            let values: HashMap<u32, &PropertyValue> =
                set.entries.iter().map(|e| (e.pid, &e.value)).collect();
            let mut n_mirrors = 0;
            for e in &set.entries {
                let Some(m) = mirrors.get(e.pid) else {
                    continue;
                };
                n_mirrors += 1;
                let row = acc.mirror_occ.entry(m.pattern).or_default();
                row[0] += 1;
                match m.base.and_then(|b| values.get(&b)) {
                    Some(&b) if *b == e.value => row[1] += 1,
                    Some(_) => row[3] += 1,
                    None => {
                        row[2] += 1;
                        acc.base_absent_names.insert(e.pid);
                    }
                }
            }
            acc.mirrors_per_set
                .entry(o.class)
                .or_default()
                .insert(n_mirrors);

            // CR13 second half: a base the set lacks answers absent, never the mirror.
            if let (Some(sp), Some(fs)) = (speed, fs_speed)
                && values.contains_key(&fs)
                && !values.contains_key(&sp)
            {
                acc.speed_absent_with_mirror += 1;
                if store_ref.lookup(o.did, sp).is_some() {
                    acc.speed_lookup_wrong += 1;
                }
            }

            let t = CreatureTemplate::read(set, p);
            // CR3.
            if let Some(g) = t.identity.genus {
                acc.genus_present += 1;
                acc.genus_one_bit += usize::from(single_bit(g).is_some());
            }
            // CR2.
            if let Some(cd) = t.identity.classification {
                acc.classification_checked += 1;
                let agree = classification_of
                    .get(&cd)
                    .is_some_and(|c| c.disagreements(&t.identity).is_empty());
                acc.classification_disagree += usize::from(!agree);
            }
            // CR12, §4.8.
            let has_level = set.has(p.advancement_level);
            match o.kind {
                Some(TemplateKind::Mob | TemplateKind::Npc) => {
                    acc.level_in_mob_npc += usize::from(has_level);
                }
                Some(TemplateKind::Container) => {
                    acc.containers_with_level += usize::from(has_level)
                }
                None => {}
            }
            // CR5 for mobs and NPCs; §3.1.
            if !state.imports.is_empty() {
                acc.imports += 1;
            }
            if matches!(o.kind, Some(TemplateKind::Mob | TemplateKind::Npc)) {
                let att = t.ai.attitude_table;
                let ok = state.imports.len() == 1
                    && Some(state.imports[0].did) == att
                    && state.bundle.items().iter().any(|i| i.as_did() == att);
                acc.import_mismatch += usize::from(!ok);
                // §4 counts are of the mob class and the main NPC class.
                if let Some(a) = att {
                    if Some(o.class) == mob_class {
                        acc.attitude_mob.insert(a);
                    } else if Some(o.class) == npc_class {
                        acc.attitude_npc.insert(a);
                    }
                }
            }
            acc.no_links += usize::from(state.links.is_none());
            acc.no_available += usize::from(state.available.is_none());
            // §4.3 action-table references (mobs).
            if o.kind == Some(TemplateKind::Mob) {
                for r in &t.ai.action_tables {
                    acc.action_elements += 1;
                    acc.min_minus_one += usize::from(r.min_level == Some(-1));
                    acc.max_minus_one += usize::from(r.max_level == Some(-1));
                    if r.table.is_null() {
                        acc.action_null += 1;
                    } else if w
                        .class_of(r.table)
                        .is_some_and(|c| action_table_classes.contains(&c))
                    {
                        acc.action_to_table += 1;
                    } else {
                        acc.action_other += 1;
                    }
                }
            }
            if let Some(st) = store_ref.stored(o.did) {
                acc.varying.entry(o.class).or_default().push(st.varying());
            }
            acc
        });
        for part in parts {
            acc.merge(part);
        }
    }

    // §3.2.1 table.
    let row = |p: MirrorPattern| acc.mirror_occ.get(&p).copied().unwrap_or_default();
    for (pat, total, equal, absent, differ) in [
        (
            MirrorPattern::ForwardCachedPrefix,
            1_338_192,
            1_338_192,
            0,
            0,
        ),
        (
            MirrorPattern::CachedSuffix,
            3_848_272,
            3_737_347,
            110_925,
            0,
        ),
        (
            MirrorPattern::ForwardSourcePrefix,
            723_173,
            649_039,
            74_133,
            1,
        ),
        (MirrorPattern::ForwardSourceSuffix, 443_838, 443_838, 0, 0),
        (MirrorPattern::ShadowSuffix, 77_302, 77_302, 0, 0),
        (MirrorPattern::BaseSuffix, 77_302, 77_302, 0, 0),
    ] {
        let r = row(pat);
        c.check(&format!("{pat:?} occurrences"), total, r[0]);
        c.check(&format!("{pat:?} equal to base"), equal, r[1]);
        c.check(&format!("{pat:?} base absent"), absent, r[2]);
        c.check(&format!("{pat:?} different"), differ, r[3]);
    }
    let differ: usize = acc.mirror_occ.values().map(|r| r[3]).sum();
    assert!(differ <= 1, "CR4: {differ} mirrors differ from their base");
    c.check("base-less mirror names", 8, acc.base_absent_names.len());
    let st = store.stats();
    c.check("stored mirrors (D1)", 185_059, st.stored_mirrors);
    for (class, counts) in &acc.mirrors_per_set {
        eprintln!("  class {class}: mirrors per set {counts:?}");
    }

    // §3.2.2 table for the mob, NPC and container classes.
    for (name, class, distinct, mirrors_n, constant, varying, vmin, vmed, vmax) in [
        ("mobs", mob_class, 610, 175, 200, 235, 65, 68, 92),
        ("NPCs", npc_class, 545, 175, 190, 180, 26, 55, 70),
        ("containers", container_class, 115, 12, 16, 87, 12, 52, 59),
    ] {
        let Some(class) = class else { continue };
        let Some(info) = store.constants().class(class) else {
            continue;
        };
        c.check(
            &format!("{name}: distinct PIDs"),
            distinct,
            info.distinct_pids(),
        );
        c.check(
            &format!("{name}: mirrors"),
            mirrors_n,
            info.mirror_pids.len(),
        );
        c.check(
            &format!("{name}: class-constant"),
            constant,
            info.constants.len(),
        );
        c.check(&format!("{name}: varying"), varying, info.varying.len());
        let mut v = acc.varying.get(&class).cloned().unwrap_or_default();
        let med = median(&mut v);
        c.check(
            &format!("{name}: varying per record min"),
            vmin,
            v.first().copied().unwrap_or(0),
        );
        c.check(&format!("{name}: varying per record median"), vmed, med);
        c.check(
            &format!("{name}: varying per record max"),
            vmax,
            v.last().copied().unwrap_or(0),
        );
    }

    // Invariants.
    assert_eq!(acc.reconstructed, templates.len(), "CR13");
    assert_eq!(acc.genus_one_bit, acc.genus_present, "CR3");
    assert_eq!(acc.classification_disagree, 0, "CR2");
    assert_eq!(acc.level_in_mob_npc, 0, "CR12");
    assert_eq!(acc.import_mismatch, 0, "CR5");
    assert_eq!(acc.speed_lookup_wrong, 0, "CR13 (base never from a mirror)");
    c.check(
        "templates naming a classification (CR2)",
        36_929,
        acc.classification_checked,
    );
    c.check(
        "templates with Agent_Genus (CR3)",
        36_929,
        acc.genus_present,
    );
    c.check("templates with an import (§3.1)", 36_929, acc.imports);
    c.check("templates without a link section (§3.1)", 149, acc.no_links);
    c.check(
        "templates without an available list (§3.4)",
        163,
        acc.no_available,
    );
    c.check(
        "containers with Advancement_Level (§4.8)",
        1_385,
        acc.containers_with_level,
    );
    c.check(
        "sets with the speed mirror only (CR13)",
        36_928,
        acc.speed_absent_with_mirror,
    );
    c.check(
        "mob action-table elements (§4.3)",
        206_050,
        acc.action_elements,
    );
    c.check("… naming an action table", 50_890, acc.action_to_table);
    c.check("… null", 155_158, acc.action_null);
    c.check("… naming another kind", 2, acc.action_other);
    c.check("… min level -1", 197_911, acc.min_minus_one);
    c.check("… max level -1", 205_925, acc.max_minus_one);
    c.check("distinct mob attitude tables", 118, acc.attitude_mob.len());
    c.check("distinct NPC attitude tables", 6, acc.attitude_npc.len());
    c.finish("creatures §2–§4 templates");
}

// ---------------------------------------------------------------------------------------------
// §5 AI records
// ---------------------------------------------------------------------------------------------

#[test]
fn ai_records_census() {
    let Some(w) = world() else { return };
    let mut c = Census::default();
    let a = &w.pids.ai;
    let x = &w.pids.exceptions;
    let count = |k| w.records_of(k).count();

    // §5.1.
    let (mut actions, mut chain, mut skill_classes) = (0, 0, BTreeSet::new());
    for (did, s) in w.records_of(CreatureRecordKind::ActionTable) {
        let t = ActionTable::read(s, a).unwrap_or_else(|e| panic!("{did}: {e}"));
        actions += t.actions.len();
        for act in &t.actions {
            chain += act.chain.len();
            for e in &act.chain {
                if let Some(cl) = e.skill.and_then(|d| w.class_of(d)) {
                    skill_classes.insert(cl);
                }
            }
        }
    }
    c.check(
        "action tables",
        3_033,
        count(CreatureRecordKind::ActionTable),
    );
    c.check("actions", 14_300, actions);
    c.check("chain entries", 15_023, chain);
    c.check("skill classes", 1, skill_classes.len());

    // §5.2 through WEENIECONTENT.
    let prob = w
        .store
        .weenie(PROBABILITY_TABLE_LABEL)
        .expect("DID map")
        .expect("AIActionProbability entry");
    let (_, kind, set) = w.records.get(&prob).expect("probability table record");
    assert_eq!(*kind, CreatureRecordKind::ProbabilityTable);
    let pt = ProbabilityTable::read(set, a).expect("probability table");
    c.check("probability entries", 9, pt.entries().len());

    // §5.3.
    let (mut friendly, mut aggressive, mut both, mut empty, mut filters) = (0, 0, 0, 0, 0);
    let mut exceptions = 0;
    for (_, s) in w.records_of(CreatureRecordKind::AttitudeTable) {
        let t = AttitudeTable::read(s, a, x);
        friendly += usize::from(t.friendly.is_some());
        aggressive += usize::from(t.aggressive.is_some());
        both += usize::from(t.friendly.is_some() && t.aggressive.is_some());
        empty += usize::from(s.is_empty());
        for f in t.friendly.iter().chain(t.aggressive.iter()).flatten() {
            filters += 1;
            for e in &f.exceptions {
                exceptions += 1;
                assert!(
                    e.alignment.is_some()
                        || e.genus.is_some()
                        || e.species.is_some()
                        || e.subspecies.is_some(),
                    "a filter exception with no field read: abbreviated labels unresolved?"
                );
            }
        }
    }
    eprintln!("  attitude filters {filters}, exceptions {exceptions}");
    c.check(
        "attitude tables",
        124,
        count(CreatureRecordKind::AttitudeTable),
    );
    c.check("… friendly lists", 118, friendly);
    c.check("… aggressive lists", 114, aggressive);
    c.check("… both", 110, both);
    c.check("… empty", 2, empty);
    c.check(
        "filter-list records",
        1,
        count(CreatureRecordKind::FilterList),
    );
    for (did, s) in w.records_of(CreatureRecordKind::FilterList) {
        ClassificationFilterList::read(s, a, x).unwrap_or_else(|e| panic!("{did}: {e}"));
    }

    // §5.4.
    let mut one_bit = 0;
    for (_, s) in w.records_of(CreatureRecordKind::Classification) {
        one_bit += usize::from(
            Classification::read(s, a)
                .genus
                .and_then(single_bit)
                .is_some(),
        );
    }
    c.check(
        "classification records",
        124,
        count(CreatureRecordKind::Classification),
    );
    c.check("… with one genus bit", 124, one_bit);

    // §5.5, CR15.
    let event_mapper = enum_mapper_of(&w.dict, a.event_type).expect("event-type mapper");
    let keys: BTreeSet<u32> = mapper_chain(&w.store, event_mapper)
        .iter()
        .flat_map(|m| m.names().iter().map(|(k, _)| *k))
        .collect();
    let event_keys: Vec<u32> = keys
        .iter()
        .copied()
        .filter(|&k| (1..=22).contains(&k))
        .collect();
    let (mut events, mut lines, mut sounds, mut top) = (0, 0, 0, 0);
    for (did, s) in w.records_of(CreatureRecordKind::ColorEventTable) {
        let t = ColorEventTable::read(s, a).unwrap_or_else(|e| panic!("{did}: {e}"));
        assert!(t.covers_exactly(event_keys.iter().copied()), "CR15: {did}");
        events += t.events.len();
        for e in &t.events {
            lines += e.lines.len();
            sounds += e.lines.iter().filter(|l| l.sound.is_some()).count();
        }
        top += usize::from(t.top_level.is_some());
    }
    c.check(
        "colour-event tables",
        883,
        count(CreatureRecordKind::ColorEventTable),
    );
    c.check("event-type keys 1–22 in the mapper", 22, event_keys.len());
    c.check("array events", 19_426, events);
    c.check("data entries", 97_130, lines);
    c.check("sounds", 1_382, sounds);
    c.check("top-level events (D7)", 1, top);

    // §5.6–§5.8.
    let mut followers = 0;
    for (did, s) in w.records_of(CreatureRecordKind::Formation) {
        followers += Formation::read(s, a)
            .unwrap_or_else(|e| panic!("{did}: {e}"))
            .followers
            .len();
    }
    c.check("formations", 212, count(CreatureRecordKind::Formation));
    c.check("followers", 685, followers);
    let (mut six_roles, mut contributions, mut with_skill) = (0, 0, 0);
    for (_, s) in w.records_of(CreatureRecordKind::Conjunction) {
        let cj = Conjunction::read(s, a);
        six_roles += usize::from(cj.roles.len() == 6);
        for r in &cj.roles {
            contributions += r.contributions.len();
            with_skill += r
                .contributions
                .iter()
                .filter(|c| c.action_skill.is_some_and(|d| !d.is_null()))
                .count();
        }
    }
    c.check("conjunctions", 237, count(CreatureRecordKind::Conjunction));
    c.check("… with 6 roles", 237, six_roles);
    c.check("contributions", 9_954, contributions);
    c.check("… with an action skill", 2_016, with_skill);
    let mut queue: BTreeMap<Option<u32>, usize> = BTreeMap::new();
    for (_, s) in w.records_of(CreatureRecordKind::PuppetPackage) {
        *queue
            .entry(PuppetPackage::read(s, a).queue_type)
            .or_default() += 1;
    }
    c.check(
        "puppet packages",
        82,
        count(CreatureRecordKind::PuppetPackage),
    );
    c.check("… key 2", 72, queue.get(&Some(2)).copied().unwrap_or(0));
    c.check("… key 1", 10, queue.get(&Some(1)).copied().unwrap_or(0));
    c.finish("creatures §5 AI records");
}

// ---------------------------------------------------------------------------------------------
// §6.5–§6.7, §6.10 profiles
// ---------------------------------------------------------------------------------------------

fn profiles(w: &World) -> BTreeMap<Did, GeneratorProfile> {
    w.records_of(CreatureRecordKind::GeneratorProfile)
        .map(|(d, s)| {
            (
                d,
                GeneratorProfile::read(s, &w.pids).unwrap_or_else(|e| panic!("{d}: {e}")),
            )
        })
        .collect()
}

#[test]
fn generator_profiles_census() {
    let Some(w) = world() else { return };
    let mut c = Census::default();
    let src = profiles(w);
    c.check("profiles", 4_168, src.len());

    let (mut regen, mut top_entries, mut top_entry_profiles) = (0, 0, 0);
    let (mut one_ofs, mut one_of_profiles, mut pses, mut pse_profiles) = (0, 0, 0, 0);
    let (mut wsl, mut defs, mut repeats, mut randoms) = (0, 0, 0, 0);
    let (mut entry_choice, mut set_choice, mut set_choice_elems) = (0, 0, 0);
    let mut modifiers: BTreeMap<&str, usize> = BTreeMap::new();
    let mut pse_entry = 0;
    for (did, p) in &src {
        assert!(
            p.issues().is_empty(),
            "CR9/CR10/CR14: {did}: {:?}",
            p.issues()
        );
        let (mut te, mut oo, mut ps) = (0, 0, 0);
        for el in &p.elements {
            match el {
                ProfileElement::RegenType(_) => regen += 1,
                ProfileElement::Entry(_) => te += 1,
                ProfileElement::OneOf(o) => {
                    oo += 1;
                    let sets = o
                        .choices
                        .iter()
                        .filter(|c| matches!(c, Choice::PositionSet(_)))
                        .count();
                    if sets > 0 {
                        set_choice += 1;
                        set_choice_elems += sets;
                    } else {
                        entry_choice += 1;
                        let m = match o.modifier {
                            Some(Modifier::Repeat(_)) => "Repeat",
                            Some(Modifier::Placement(Placement::Exact { .. })) => "ExactPlacement",
                            Some(Modifier::Placement(Placement::Random { .. })) => {
                                "RandomPlacement"
                            }
                            None => "none",
                        };
                        *modifiers.entry(m).or_default() += 1;
                    }
                }
                ProfileElement::PositionSet(s) => {
                    ps += 1;
                    pse_entry += usize::from(s.entry.is_some());
                }
                ProfileElement::Other { .. } => panic!("{did}: unknown profile element"),
            }
        }
        top_entries += te;
        top_entry_profiles += usize::from(te > 0);
        one_ofs += oo;
        one_of_profiles += usize::from(oo > 0);
        pses += ps;
        pse_profiles += usize::from(ps > 0);
        for (e, _) in p.entries() {
            wsl += usize::from(e.object.is_some());
            defs += usize::from(e.profile.is_some());
            repeats += usize::from(e.repeat.is_some());
            randoms += usize::from(matches!(e.placement, Some(Placement::Random { .. })));
        }
        // CR6: every ProfileDefinition names a profile; expansion terminates.
        let r = resolve_candidates(&src, *did, None);
        assert!(
            r.missing_profiles.is_empty(),
            "CR6: {did}: {:?}",
            r.missing_profiles
        );
        assert_eq!(
            (r.cycles, r.depth_limited, r.truncated),
            (0, 0, false),
            "CR6: {did}"
        );
    }
    c.check("RegenType elements", 3_621, regen);
    c.check("top-level entries", 75, top_entries);
    c.check("… in profiles", 20, top_entry_profiles);
    c.check("OneOfs in a profile", 3_328, one_ofs);
    c.check("… in profiles", 2_736, one_of_profiles);
    c.check("PositionSetEntry elements", 6_010, pses);
    c.check("… in profiles", 1_759, pse_profiles);
    c.check("… with an Entry", 420, pse_entry);
    c.check("entries with WSLEntity", 23_270, wsl);
    c.check("entries with ProfileDefinition", 3_352, defs);
    c.check("OneOfs choosing entries", 2_440, entry_choice);
    c.check(
        "… Repeat modifier",
        16,
        modifiers.get("Repeat").copied().unwrap_or(0),
    );
    c.check(
        "… ExactPlacement modifier",
        26,
        modifiers.get("ExactPlacement").copied().unwrap_or(0),
    );
    c.check(
        "… RandomPlacement modifier",
        35,
        modifiers.get("RandomPlacement").copied().unwrap_or(0),
    );
    c.check(
        "… no modifier",
        2_363,
        modifiers.get("none").copied().unwrap_or(0),
    );
    c.check("OneOfs choosing position sets", 888, set_choice);
    c.check("… their PositionSetEntry choices", 1_952, set_choice_elems);
    c.check("entries with their own Repeat", 8, repeats);
    c.check("entries with their own RandomPlacement", 8, randoms);
    c.finish("creatures §6.5 profiles");
}

// ---------------------------------------------------------------------------------------------
// §7 encounter tables
// ---------------------------------------------------------------------------------------------

#[test]
fn encounter_tables() {
    let Some(w) = world() else { return };
    let mut c = Census::default();
    let set: &ArchiveSet = w.store.game_data().source();
    let (mut records, mut slots, mut sizes) = (0, 0, Vec::new());
    for m in set.members() {
        for e in m.archive.entries_range(0x1400_0000..=0x14FF_FFFF) {
            let e = e.expect("directory");
            let bytes = m.archive.read_entry(&e).expect("read");
            let t = EncounterTable::parse_expecting(&bytes, Did(e.did))
                .unwrap_or_else(|err| panic!("CR11: {:#010x}: {err}", e.did));
            assert!(t.keys_are_indices(), "CR11: {:#010x}", e.did);
            assert_eq!(t.to_bytes(), bytes, "re-encode {:#010x}", e.did);
            records += 1;
            slots += t.zones.iter().map(|z| z.slots.len()).sum::<usize>();
            sizes.push(bytes.len());
        }
    }
    sizes.sort_unstable();
    eprintln!("  encounter-table sizes {sizes:?}");
    c.check("encounter tables", 5, records);
    c.check("slots", 70, slots);
    c.finish("creatures §7 encounter tables");
}

// ---------------------------------------------------------------------------------------------
// §6.2–§6.4, §6.9, §6.10 landblocks
// ---------------------------------------------------------------------------------------------

/// A 0x47 marker: header kind, own `Generator_PositionSetID`, parent 0x47.
type Marker = (u32, Option<u32>, Option<Did>);

#[derive(Default)]
struct LbStats {
    generators_with_profile: usize,
    generator_points: usize,
    patrol_points: usize,
    type7: usize,
    type7_cross: usize,
    type7_bad_target: usize,
    points_without_set: usize,
    entity_kind_checked: usize,
    entity_kind_mismatch: usize,
    markers: HashSet<Did>,
    patrol_ends: BTreeMap<(u32, &'static str), usize>,
    profiles: BTreeSet<Did>,
    depths: BTreeMap<usize, usize>,
    missing_profile: usize,
    generators_with_type8: usize,
}

#[test]
fn landblock_spawn_chain() {
    let Some(w) = world() else { return };
    let mut c = Census::default();
    let set: &ArchiveSet = w.store.game_data().source();
    let sp = &w.pids.spawn;

    // Every landblock-info record (0x802R…) of every cell archive.
    let mut work = Vec::new();
    for (i, m) in set.members().iter().enumerate() {
        if !matches!(m.id.kind, ArchiveKind::Cell(_)) {
            continue;
        }
        for e in m.archive.entries_range(0x8000_0000..=0x8FFF_FFFF) {
            let e = e.expect("directory");
            if (e.did >> 20) & 0xF == 2 {
                work.push((i, e));
            }
        }
    }

    // 0x47 markers: header kind and own Generator_PositionSetID, cached.
    let marker_cache: Mutex<HashMap<Did, Option<Marker>>> = Mutex::new(HashMap::new());
    let marker = |d: Did| -> Option<Marker> {
        if let Some(v) = marker_cache.lock().unwrap().get(&d) {
            return *v;
        }
        let v = set.read(d.0).ok().and_then(|bytes| {
            let header = EntityHeader::parse(&bytes).ok()?;
            let full = EntityDescriptor::parse(&bytes, &w.dict).ok();
            let ps = full
                .as_ref()
                .and_then(|f| f.properties.enum_key(sp.position_set_id));
            let parent = (header.template.prefix() == 0x47).then_some(header.template);
            Some((header.kind, ps, parent))
        });
        marker_cache.lock().unwrap().insert(d, v);
        v
    };
    let src = profiles(w);
    let t0 = std::time::Instant::now();
    let parts = par_map(&work, |(i, e)| {
        let bytes = set.member(*i).archive.read_entry(e).expect("read");
        let info = LandblockInfo::parse_expecting(&bytes, Did(e.did), Some(e.stamp), &w.dict)
            .unwrap_or_else(|err| panic!("{:#010x}: {err}", e.did));
        let spawns = LandblockSpawns::build(&info, sp, |d| {
            marker_position_set(d, |m| marker(m).map(|(_, s, p)| (s, p)))
        });
        let mut s = LbStats::default();
        let points: HashSet<_> = spawns.points.iter().map(|p| p.iid).collect();
        for g in &spawns.generators {
            s.generators_with_type8 +=
                usize::from(!g.spawned.is_empty() || g.cross_spawn_links > 0);
            s.type7 += g.points.len();
            s.type7_cross += g.cross_point_links;
            // CR7: targets are GeneratorPoints of this landblock.
            s.type7_bad_target += g.points.iter().filter(|p| !points.contains(p)).count();
            if let Some(p) = g.profile {
                s.generators_with_profile += 1;
                s.profiles.insert(p);
                let r = resolve_candidates(&src, p, None);
                s.missing_profile += usize::from(!r.missing_profiles.is_empty());
                *s.depths.entry(r.max_depth).or_default() += 1;
            }
        }
        s.generator_points = spawns.points.len();
        s.points_without_set = spawns
            .points
            .iter()
            .filter(|p| p.position_set.is_none())
            .count();
        // CR8: the 0x47 kind equals the static-entity type.
        for ent in &info.entities {
            if let (Some(o), Some(t)) = (ent.object, ent.entity_type)
                && o.prefix() == 0x47
            {
                s.entity_kind_checked += 1;
                if marker(o).is_none_or(|(k, _, _)| k != t) {
                    s.entity_kind_mismatch += 1;
                }
                if ent.kind() == Some(StaticEntityType::GeneratorPoint) {
                    s.markers.insert(o);
                }
            }
            s.patrol_points += usize::from(ent.kind() == Some(StaticEntityType::PatrolPoint));
        }
        // §6.9 chains.
        for st in &spawns.patrol_starts {
            if st.start.0 == 0 {
                continue;
            }
            let end = match spawns.paths.follow(st.start).end {
                PatrolEnd::NoNext => "ends",
                PatrolEnd::ReturnsToStart => "returns",
                PatrolEnd::EntersLoop => "loop",
                PatrolEnd::LeavesLandblock => "leaves",
            };
            *s.patrol_ends.entry((st.link_type, end)).or_default() += 1;
        }
        s
    });
    eprintln!(
        "  {} landblock-info records in {:.1?}",
        work.len(),
        t0.elapsed()
    );
    let mut t = LbStats::default();
    for s in parts {
        t.generators_with_profile += s.generators_with_profile;
        t.generator_points += s.generator_points;
        t.patrol_points += s.patrol_points;
        t.type7 += s.type7;
        t.type7_cross += s.type7_cross;
        t.type7_bad_target += s.type7_bad_target;
        t.points_without_set += s.points_without_set;
        t.entity_kind_checked += s.entity_kind_checked;
        t.entity_kind_mismatch += s.entity_kind_mismatch;
        t.markers.extend(s.markers);
        for (k, v) in s.patrol_ends {
            *t.patrol_ends.entry(k).or_default() += v;
        }
        t.profiles.extend(s.profiles);
        for (k, v) in s.depths {
            *t.depths.entry(k).or_default() += v;
        }
        t.missing_profile += s.missing_profile;
        t.generators_with_type8 += s.generators_with_type8;
    }
    // CR8: every GeneratorPoint marker carries Generator_PositionSetID itself.
    let markers_without_set = t
        .markers
        .iter()
        .filter(|&&m| marker(m).is_none_or(|(_, s, _)| s.is_none()))
        .count();
    assert_eq!(markers_without_set, 0, "CR8 markers");
    assert_eq!(t.entity_kind_mismatch, 0, "CR8 kinds");
    assert_eq!(t.type7_bad_target, 0, "CR7");
    assert_eq!(t.missing_profile, 0, "CR6");

    // §6.10: templates reachable from the generators' profiles.
    let mut reach: BTreeSet<Did> = BTreeSet::new();
    for &p in &t.profiles {
        reach.extend(resolve_candidates(&src, p, None).objects());
    }
    let mut by_kind: BTreeMap<Option<TemplateKind>, usize> = BTreeMap::new();
    for d in &reach {
        let k = w
            .objects
            .binary_search_by_key(d, |o| o.did)
            .ok()
            .and_then(|i| w.objects[i].kind);
        *by_kind.entry(k).or_default() += 1;
    }

    c.check("landblock-info records", 49_636, work.len());
    c.check(
        "generators with a profile",
        34_296,
        t.generators_with_profile,
    );
    c.check("distinct generator profiles", 1_413, t.profiles.len());
    c.check("GeneratorPoints", 415_238, t.generator_points);
    c.check("GeneratorPoint markers", 239, t.markers.len());
    c.check("PatrolPoints", 268_896, t.patrol_points);
    c.check("type-7 links (same landblock)", 418_967, t.type7);
    c.check(
        "type-7 cross-landblock halves (target side 0)",
        0,
        t.type7_cross,
    );
    c.check("points without a position set", 0, t.points_without_set);
    c.check(
        "0x47 entities checked (CR8)",
        3_210_410,
        t.entity_kind_checked,
    );
    for (depth, n) in [(0, 625), (1, 22_808), (2, 9_856), (3, 997), (4, 10)] {
        c.check(
            &format!("generators with nesting depth {depth}"),
            n,
            t.depths.get(&depth).copied().unwrap_or(0),
        );
    }
    c.check(
        "reachable mob templates",
        4_742,
        by_kind.get(&Some(TemplateKind::Mob)).copied().unwrap_or(0),
    );
    c.check(
        "reachable NPC templates",
        17,
        by_kind.get(&Some(TemplateKind::Npc)).copied().unwrap_or(0),
    );
    c.check(
        "reachable containers",
        318,
        by_kind
            .get(&Some(TemplateKind::Container))
            .copied()
            .unwrap_or(0),
    );
    for ((ty, end), want) in [
        ((2, "ends"), 15_047),
        ((2, "returns"), 6_895),
        ((2, "loop"), 278),
        ((2, "leaves"), 5_554),
        ((1, "ends"), 3_713),
        ((1, "returns"), 1_230),
        ((1, "loop"), 68),
        ((1, "leaves"), 1_592),
        ((11, "ends"), 173),
        ((11, "returns"), 11),
        ((11, "loop"), 0),
        ((11, "leaves"), 59),
    ] {
        c.check(
            &format!("patrol type {ty} chains that {end}"),
            want,
            t.patrol_ends.get(&(ty, end)).copied().unwrap_or(0),
        );
    }
    c.finish("creatures §6 landblocks");
}
