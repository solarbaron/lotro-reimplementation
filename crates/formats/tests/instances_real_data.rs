// SPDX-License-Identifier: GPL-3.0-or-later
//! `instances.md` census against a user's own install (read-only).
//!
//! Reads the root class (offset 4, `wstate.md` §3.1) of **every** object record (prefix
//! 0x70), then decodes the property set (DID + 0x09000000) of every record of the family's
//! classes with [`decode_instance_set`] and checks:
//!
//! - the labels the views read exist in the dictionary with the spec's types;
//! - the census of §2.1 (records and bytes per class) and N-I1's property-set part;
//! - §3.4: each class classifies as its kind;
//! - N-I2 (no dangling 0x45 DID, plus the classes of the main reference fields and the
//!   `IntroVO` prefix of §3.2), N-I3 (with the `ContentLayer` DID map), N-I4, N-I5, N-I6,
//!   N-I7, N-I9 (property-form part), N-I10, N-I12;
//! - the counts of §4.1, §4.3, §5.1, §6.1–§6.3, §7.1, §8.1–§8.5, §8.7, §9, §10.1 and the
//!   kind-bit census of §10.2.
//!
//! The object-state forms (N-I1's object part, N-I8, N-I11, §13) are not checked here: they
//! need `tessera-wstate`.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. A debug build reads a 1-in-23
//! sample of the object records and skips the census assertions unless `TESSERA_FULL=1`.
//! Run with `cargo test --release -p tessera-formats --test instances_real_data -- --nocapture`.
//! Only counts, DIDs, class numbers and labels are printed; no game data leaves the install.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tessera_dat::{ArchiveSet, Entry, SetOptions};
use tessera_formats::instance::finder::{
    FlagBits, MatchingIndex, MatchingRules, MatchingTemplate, kind_bit,
};
use tessera_formats::instance::{
    Deviation, InstanceKind, InstanceRecord, InstanceVocab, MatchParams, Template, TemplateKind,
    decode_instance_set, labels,
};
use tessera_formats::{
    DataIdMap, Did, PropertyDictionary, PropertySetRecord, PropertyValue, Record,
    object_property_set_did,
};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;
const THREADS: usize = 4;

/// §2.1: (root class, records, 0x70 bytes, 0x79 bytes, kind by §3.4).
const CENSUS: &[(u32, usize, u64, u64, Option<InstanceKind>)] = &[
    (1368, 959, 641_099, 317_319, Some(InstanceKind::Template)),
    (2651, 552, 506_245, 539_505, Some(InstanceKind::Template)),
    (2727, 183, 6_588, 14_274, Some(InstanceKind::MatchParams)),
    (
        3725,
        96,
        14_396,
        765_590,
        Some(InstanceKind::DistantBattleUnit),
    ),
    (
        3808,
        14,
        1_166,
        12_576,
        Some(InstanceKind::DistantBattleScript),
    ),
    (2669, 3, 725, 1_150, Some(InstanceKind::SkillTrainerTraits)),
    (2573, 2, 144, 100, Some(InstanceKind::EntryFilter)),
    (2576, 2, 126, 76, Some(InstanceKind::EntryFilter)),
    (3894, 2, 72, 60, Some(InstanceKind::WorldJoinTicket)),
    (1366, 1, 429, 776, Some(InstanceKind::EncounterControl)),
    (2515, 1, 11_268, 14_078, Some(InstanceKind::SkirmishControl)),
    (2606, 1, 2_092, 1_742, Some(InstanceKind::WorldJoinControl)),
    (3787, 1, 1_877, 7_176, Some(InstanceKind::BigBattleControl)),
    (3745, 1, 113, 240, Some(InstanceKind::FormationControl)),
    (1925, 1, 753, 2_708, Some(InstanceKind::SessionPlayControl)),
    (
        3276,
        1,
        25_539,
        374,
        Some(InstanceKind::FellowshipMakerControl),
    ),
    (3369, 1, 109, 432, Some(InstanceKind::PhasingBox)),
    (3423, 1, 36, 6, None),
    (3576, 1, 312, 6, None),
    (1367, 1, 63, 509, None),
    (2619, 1, 63, 495, None),
    (3136, 1, 63, 507, None),
    (1766, 1, 36, 6, None),
    (1769, 1, 36, 6, None),
    (1772, 1, 36, 6, None),
    (2753, 1, 36, 6, None),
    (2757, 1, 36, 6, None),
    (2759, 1, 36, 6, None),
    (3060, 1, 36, 6, None),
];

/// Battle statistics (§2.1 second table, §8.7), decoded too: (class, records).
const BATTLE_STATS: &[(u32, usize)] = &[(3590, 10), (3622, 3)];

fn open_set() -> Option<&'static ArchiveSet> {
    static SET: OnceLock<Option<ArchiveSet>> = OnceLock::new();
    SET.get_or_init(|| {
        let dir = match std::env::var_os("TESSERA_GAME_DIR") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => {
                eprintln!("TESSERA_GAME_DIR not set; skipping");
                return None;
            }
        };
        Some(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"))
    })
    .as_ref()
}

fn exhaustive() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

fn par_each<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out = Mutex::new(Vec::with_capacity(items.len()));
    std::thread::scope(|s| {
        for _ in 0..THREADS {
            s.spawn(|| {
                let mut local = Vec::new();
                loop {
                    let i = next.fetch_add(64, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    for item in &items[i..(i + 64).min(items.len())] {
                        local.push(f(item));
                    }
                }
                out.lock().unwrap().extend(local);
            });
        }
    });
    out.into_inner().unwrap()
}

/// (did, root class, payload length) of every object record.
fn object_classes(set: &ArchiveSet) -> Vec<(u32, u32, usize)> {
    let lo = 0x70u32 << 24;
    let mut work: Vec<(usize, Entry)> = Vec::new();
    for (i, m) in set.members().iter().enumerate() {
        work.extend(
            m.archive
                .entries_range(lo..=lo | 0x00FF_FFFF)
                .map(|e| (i, e.expect("directory walk"))),
        );
    }
    let full = exhaustive();
    let work: Vec<_> = work
        .into_iter()
        .enumerate()
        .filter(|(n, _)| full || n % 23 == 0)
        .map(|(_, w)| w)
        .collect();
    par_each(&work, |(i, e)| {
        let payload = set
            .member(*i)
            .archive
            .read_entry(e)
            .expect("read 0x70 record");
        let class = payload
            .get(4..8)
            .map_or(u32::MAX, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        (e.did, class, payload.len())
    })
}

fn report(name: &str, devs: &[Deviation]) {
    for d in devs.iter().take(10) {
        eprintln!("  {name}: {} {}", d.invariant, d.detail);
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn instances_census() {
    let Some(set) = open_set() else { return };
    let dict_payload = set.read(PROPERTY_DICTIONARY).expect("read the dictionary");
    let dict = PropertyDictionary::parse_expecting(&dict_payload, PROPERTY_DICTIONARY)
        .expect("dictionary");
    let vocab = InstanceVocab::from_dictionary(&dict);

    // Labels and types.
    let missing = vocab.missing();
    let mismatched = vocab.type_mismatches(&dict);
    eprintln!(
        "labels: {} read, {} missing, {} with another type",
        labels::ALL.len(),
        missing.len(),
        mismatched.len()
    );
    for m in &missing {
        eprintln!("  missing label {m}");
    }
    for (label, want, found) in &mismatched {
        eprintln!("  {label}: spec type {want:?}, dictionary {found:?}");
    }
    assert!(missing.is_empty(), "labels missing from the dictionary");
    assert!(mismatched.is_empty(), "label types differ from the spec");

    // Root classes of all object records.
    let objects = object_classes(set);
    let all_objects: HashMap<u32, u32> = objects.iter().map(|(d, c, _)| (*d, *c)).collect();
    eprintln!("object records read: {}", objects.len());
    let wanted: BTreeSet<u32> = CENSUS
        .iter()
        .map(|c| c.0)
        .chain(BATTLE_STATS.iter().map(|c| c.0))
        .collect();
    let family: Vec<(u32, u32, usize)> = objects
        .iter()
        .copied()
        .filter(|(_, c, _)| wanted.contains(c))
        .collect();

    // Property sets of the family.
    let decoded = par_each(&family, |&(did, class, len)| {
        let set_did = object_property_set_did(did).expect("0x70 maps to 0x79");
        let bytes = set.read(set_did.0).map_err(|e| format!("read: {e}"));
        let r = bytes.and_then(|b| {
            let n = b.len();
            decode_instance_set(&b, Some(set_did), &dict, &vocab)
                .map(|(rec, view)| (rec, view, n))
                .map_err(|e| format!("{e}"))
        });
        (did, class, len, r)
    });
    let mut per_class: BTreeMap<u32, (usize, u64, u64)> = BTreeMap::new();
    let mut failures = Vec::new();
    let mut kinds_wrong = Vec::new();
    let mut views: BTreeMap<u32, (u32, InstanceRecord)> = BTreeMap::new();
    let mut sets: Vec<(u32, PropertySetRecord)> = Vec::new();
    let expected_kind: HashMap<u32, Option<InstanceKind>> = CENSUS
        .iter()
        .map(|c| (c.0, c.4))
        .chain(
            BATTLE_STATS
                .iter()
                .map(|c| (c.0, Some(InstanceKind::BattleStat))),
        )
        .collect();
    for (did, class, len, r) in decoded {
        let e = per_class.entry(class).or_default();
        e.0 += 1;
        e.1 += len as u64;
        match r {
            Ok((rec, view, n)) => {
                e.2 += n as u64;
                let kind = view.as_ref().map(InstanceRecord::kind);
                if kind != expected_kind[&class] {
                    kinds_wrong.push((did, class, kind));
                }
                if let Some(v) = view {
                    views.insert(did, (class, v));
                }
                sets.push((did, rec));
            }
            Err(msg) => failures.push((did, class, msg)),
        }
    }
    eprintln!(
        "family records: {} decoded, {} failed, {} misclassified",
        sets.len(),
        failures.len(),
        kinds_wrong.len()
    );
    for f in failures.iter().take(10) {
        eprintln!("  failed {:#010x} (class {}): {}", f.0, f.1, f.2);
    }
    for k in kinds_wrong.iter().take(10) {
        eprintln!("  {:#010x} class {} classified {:?}", k.0, k.1, k.2);
    }
    assert!(
        failures.is_empty(),
        "N-I1: property sets that do not decode"
    );
    assert!(kinds_wrong.is_empty(), "§3.4 classification");
    if !exhaustive() {
        eprintln!("debug sample: census assertions skipped (TESSERA_FULL=1 for all)");
        return;
    }

    // §2.1 census.
    let mut census_ok = true;
    let (mut n, mut b70, mut b79) = (0usize, 0u64, 0u64);
    for &(class, records, bytes70, bytes79, _) in CENSUS {
        let got = per_class.get(&class).copied().unwrap_or_default();
        n += got.0;
        b70 += got.1;
        b79 += got.2;
        if got != (records, bytes70, bytes79) {
            census_ok = false;
            eprintln!("  class {class}: {got:?}, spec ({records}, {bytes70}, {bytes79})");
        }
    }
    for &(class, records) in BATTLE_STATS {
        let got = per_class.get(&class).map_or(0, |c| c.0);
        if got != records {
            census_ok = false;
            eprintln!("  class {class}: {got} records, spec {records}");
        }
    }
    eprintln!("census: {n} records, {b70} bytes of 0x70, {b79} bytes of 0x79");
    assert!(census_ok, "§2.1 per-class census");
    assert_eq!((n, b70, b79), (1_833, 1_213_530, 1_679_741), "§2.1 totals");

    // Typed views by kind.
    let templates: BTreeMap<u32, (u32, &Template)> = views
        .iter()
        .filter_map(|(d, (c, v))| match v {
            InstanceRecord::Template(t) => Some((*d, (*c, t.as_ref()))),
            _ => None,
        })
        .collect();
    let one = |kind: InstanceKind| -> Vec<(u32, &InstanceRecord)> {
        views
            .iter()
            .filter(|(_, (_, v))| v.kind() == kind)
            .map(|(d, (_, v))| (*d, v))
            .collect()
    };
    let class_of = |d: Did| all_objects.get(&d.0).copied();
    let mut problems: Vec<String> = Vec::new();

    // N-I2: every non-zero 0x45 DID names an existing object record.
    let (mut did45, mut dangling, mut intro, mut intro_nonzero, mut intro_bad) = (0, 0, 0, 0, 0);
    let intro_pid = vocab.pid(labels::TPL_INTRO_VO);
    let census_classes: BTreeSet<u32> = CENSUS.iter().map(|c| c.0).collect();
    for (did, rec) in &sets {
        let in_census = all_objects
            .get(did)
            .is_some_and(|c| census_classes.contains(c));
        rec.set.walk(|v| {
            let PropertyValue::DidRef(d) = &v.entry.value else {
                return;
            };
            if Some(v.entry.pid) == intro_pid {
                intro += 1;
                if d.0 != 0 {
                    intro_nonzero += 1;
                    if d.prefix() != 0x2A {
                        intro_bad += 1;
                    }
                }
            }
            if dict.definition(v.entry.pid).and_then(|x| x.dbo_type()) == Some(0x45) {
                did45 += usize::from(in_census);
                if d.0 != 0 && !all_objects.contains_key(&d.0) {
                    dangling += 1;
                }
            }
        });
    }
    eprintln!(
        "N-I2: {did45} type-0x45 DID values in the §2.1 records, {dangling} dangling (battle statistics included); IntroVO {intro} values, {intro_nonzero} non-zero, {intro_bad} not 0x2A"
    );
    assert_eq!(did45, 38_545, "§3.2: type-0x45 values in the §2.1 records");
    if dangling != 0 {
        problems.push(format!("N-I2: {dangling} dangling"));
    }
    assert_eq!(
        (intro, intro_nonzero, intro_bad),
        (942, 908, 0),
        "§3.2 IntroVO"
    );

    // §4: templates.
    let by_kind = templates.values().fold(BTreeMap::new(), |mut m, (_, t)| {
        *m.entry(t.kind()).or_insert(0usize) += 1;
        m
    });
    eprintln!("templates: {} {by_kind:?}", templates.len());
    assert_eq!(templates.len(), 1_511);
    assert_eq!(by_kind.get(&TemplateKind::Ordinary), Some(&959));
    assert_eq!(by_kind.get(&TemplateKind::Mission), Some(&384));
    assert_eq!(by_kind.get(&TemplateKind::WorldJoinEncounter), Some(&166));
    assert_eq!(by_kind.get(&TemplateKind::OtherSkirmish), Some(&2));
    let skirmish_class_ok = templates
        .values()
        .all(|(c, t)| t.skirmish_class == (*c == 2651));
    assert!(skirmish_class_ok, "§3.4: TierMin list tells 2651 from 1368");

    let layer_map_did = vocab
        .pid(labels::TPL_CONTENT_LAYER)
        .and_then(|p| dict.definition(p))
        .map(|d| d.data)
        .expect("content-layer definition");
    let layer_payload = set
        .read(layer_map_did.0)
        .expect("read the ContentLayer DID map");
    let layer_map = DataIdMap::parse_expecting(&layer_payload, layer_map_did).expect("DID map");
    let mut local = 0usize;
    let mut unknown_layer = 0usize;
    let (mut values, mut aligned, mut with_list, mut dup, mut no_list) = (0, 0, 0, 0, 0);
    let mut lock_size = BTreeSet::new();
    let mut scripts: BTreeMap<Did, Vec<u32>> = BTreeMap::new();
    for (did, (_, t)) in &templates {
        let devs = t.deviations();
        if !devs.is_empty() {
            local += devs.len();
            report(&format!("{did:#010x}"), &devs);
        }
        if !t.content_layer_known(|k| layer_map.get(k).is_some()) {
            unknown_layer += 1;
        }
        values += t.landblocks.len() + t.bad_landblocks.len();
        aligned += t.landblocks.iter().filter(|b| b.is_aligned()).count();
        if t.has_landblock_list {
            with_list += 1;
        } else {
            no_list += 1;
        }
        if t.has_duplicate_landblock() {
            dup += 1;
        }
        if t.lock_size.is_some() {
            lock_size.insert(Did(*did));
        }
        if let Some(s) = t.distant_battle_script.filter(|d| d.0 != 0) {
            scripts.entry(s).or_default().push(*did);
            if class_of(s) != Some(3808) {
                problems.push(format!("N-I2: {did:#010x} script class {:?}", class_of(s)));
            }
        }
        if let Some(tk) = t.world_join_ticket.filter(|d| d.0 != 0)
            && class_of(tk) != Some(3894)
        {
            problems.push(format!("N-I2: {did:#010x} ticket class {:?}", class_of(tk)));
        }
    }
    eprintln!(
        "landblocks: {values} values ({aligned} aligned), {with_list} lists, {no_list} without, {dup} with a duplicate; {unknown_layer} unknown layers; {local} local deviations"
    );
    assert_eq!(local, 0, "N-I3, N-I4, N-I5");
    assert_eq!(unknown_layer, 0, "N-I3: content layer in the DID map");
    assert_eq!(
        (values, aligned, with_list, no_list, dup),
        (4_628, 4_410, 1_487, 24, 25),
        "§4.3"
    );

    // §5.1 control and N-I6.
    let template_dids: BTreeSet<Did> = templates.keys().map(|d| Did(*d)).collect();
    let controls = one(InstanceKind::EncounterControl);
    assert_eq!(controls.len(), 1);
    if let InstanceRecord::EncounterControl(c) = controls[0].1 {
        let devs = c.check_lockable(&lock_size);
        report("N-I6", &devs);
        assert!(devs.is_empty(), "N-I6");
        assert_eq!(c.lockable.len(), 43);
        assert_eq!(c.locks.len(), 5);
        let sizes: BTreeSet<u32> = c.locks.iter().filter_map(|r| r.size).collect();
        assert_eq!(sizes, (1..=5).collect());
        assert_eq!(c.lock(5).map(|r| r.reset_day_keys().len()), Some(7));
        assert_eq!(c.lock(1).map(|r| r.reset_day_keys().len()), Some(1));
        assert_eq!(c.entry_filters.len(), 4);
        let filter_classes: BTreeSet<_> = c.entry_filters.iter().map(|d| class_of(*d)).collect();
        assert_eq!(filter_classes, [Some(2573), Some(2576)].into());
        assert_eq!(c.filter_ignored.len(), 12);
        for d in c
            .lockable
            .iter()
            .chain(&c.filter_ignored)
            .chain(&c.pvmp_challenge)
        {
            if !template_dids.contains(d) {
                problems.push(format!("N-I2: control names non-template {:#010x}", d.0));
            }
        }
    }

    // §6.1 match parameters.
    let params: Vec<MatchParams> = one(InstanceKind::MatchParams)
        .into_iter()
        .filter_map(|(_, v)| match v {
            InstanceRecord::MatchParams(m) => Some(*m),
            _ => None,
        })
        .collect();
    let distinct: BTreeSet<_> = params.iter().copied().collect();
    let (mut same_bands_overlap, mut triple_overlap) = (0, 0);
    for (i, a) in params.iter().enumerate() {
        for b in &params[i + 1..] {
            if a.group_size == b.group_size && a.tier == b.tier && a.level_overlaps(b) {
                same_bands_overlap += 1;
            }
            if a.level_overlaps(b) && a.group_overlaps(b) && a.tier_overlaps(b) {
                triple_overlap += 1;
            }
        }
    }
    eprintln!(
        "match parameters: {} ({} distinct), {same_bands_overlap} same-band overlaps, {triple_overlap} triple overlaps",
        params.len(),
        distinct.len()
    );
    assert_eq!((params.len(), distinct.len()), (183, 183));
    assert_eq!(
        (same_bands_overlap, triple_overlap),
        (0, 144),
        "§6.1 band overlaps"
    );

    // §6.2 trait lists.
    let mut trait_sizes: Vec<usize> = one(InstanceKind::SkillTrainerTraits)
        .into_iter()
        .filter_map(|(_, v)| match v {
            InstanceRecord::SkillTrainerTraits(t) => Some(t.traits.len()),
            _ => None,
        })
        .collect();
    trait_sizes.sort_unstable();
    assert_eq!(trait_sizes, vec![1, 47, 89], "§6.2");

    // §6.3 skirmish control, N-I12.
    let sc = one(InstanceKind::SkirmishControl);
    assert_eq!(sc.len(), 1);
    if let InstanceRecord::SkirmishControl(c) = sc[0].1 {
        let devs = c.check_conversion();
        report("N-I12", &devs);
        assert!(devs.is_empty(), "N-I12");
        assert_eq!(c.difficulty_multipliers.len(), 3);
        assert_eq!(c.group_size_multipliers.len(), 6);
        assert_eq!(c.trait_gates.len(), 6);
        assert_eq!(c.blocked_stats.len(), 6);
        assert_eq!((c.grs_properties.len(), c.stat_to_grs.len()), (19, 19));
        assert_eq!(c.derived_stats.len(), 5);
        // §3.5: fields read by their full labels.
        let tiers: Vec<(Option<u32>, Option<u32>)> =
            [(1, 1), (2, 3), (3, 3), (6, 6), (12, 12), (24, 12)]
                .into_iter()
                .map(|(a, b)| (Some(a), Some(b)))
                .collect();
        assert_eq!(c.group_tiers, tiers, "§6.3 group tiers");
        assert!(
            c.trait_gates
                .iter()
                .all(|(r, d)| r.is_some() && d.is_some())
        );
        assert!(
            c.stat_to_grs
                .iter()
                .all(|(s, p)| s.is_some() && p.is_some())
        );
        assert!(c.derived_stats.iter().all(|d| d.derived.is_some()
            && d.primary.is_some()
            && (1..=6).contains(&d.operations.len())));
        assert_eq!(
            c.derived_stats.iter().filter(|d| d.scale == 3600).count(),
            3,
            "§6.3: scale 3,600 in three statistics"
        );
        assert_eq!(c.fastest_possible_time, Some(0.5));
        assert_eq!(c.max_skirmishes_per_week, Some(-1));
    }

    // §7.1 world join, N-I7.
    let wj = one(InstanceKind::WorldJoinControl);
    assert_eq!(wj.len(), 1);
    let InstanceRecord::WorldJoinControl(w) = wj[0].1 else {
        unreachable!()
    };
    let devs = w.deviations();
    report("N-I7", &devs);
    assert!(devs.is_empty(), "N-I7");
    let grouped: BTreeSet<Did> = w.grouped().collect();
    let labels_sorted: Vec<u32> = {
        let mut l: Vec<u32> = w.groups.iter().filter_map(|g| g.label).collect();
        l.sort_unstable();
        l
    };
    eprintln!(
        "world join: {} available, {} groups ({} templates), {} featured, {} seasonal",
        w.available.len(),
        w.groups.len(),
        grouped.len(),
        w.featured.len(),
        w.seasonal.len()
    );
    assert_eq!(
        (w.available.len(), w.groups.len(), grouped.len()),
        (150, 8, 14)
    );
    assert_eq!(labels_sorted, (1..=8).collect::<Vec<_>>());
    assert_eq!((w.featured.len(), w.seasonal.len()), (6, 5));
    assert_eq!(w.min_level, Some(10));
    assert_eq!(w.finder_order().len(), 164);
    for d in w.available.iter().chain(&grouped) {
        if !template_dids.contains(d) {
            problems.push(format!("N-I2: finder names non-template {:#010x}", d.0));
        }
    }
    let tickets = one(InstanceKind::WorldJoinTicket);
    assert_eq!(tickets.len(), 2, "§7.2");

    // §8.1, §8.3, N-I9.
    let big_battles: BTreeSet<Did> = templates
        .iter()
        .filter(|(_, (_, t))| t.is_big_battle())
        .map(|(d, _)| Did(*d))
        .collect();
    let script_records = one(InstanceKind::DistantBattleScript);
    assert_eq!(script_records.len(), 14);
    for (d, _) in &script_records {
        let n = scripts.get(&Did(*d)).map_or(0, Vec::len);
        if n != 1 {
            problems.push(format!("N-I9: script {d:#010x} named by {n} templates"));
        }
    }
    assert_eq!(
        big_battles, grouped,
        "N-I9 (property form): big battles = grouped templates"
    );
    let (mut waves, mut objectives, mut units_named) = (0, 0, BTreeSet::new());
    for (_, v) in &script_records {
        if let InstanceRecord::DistantBattleScript(s) = v {
            assert!(
                s.weighted_units
                    .iter()
                    .chain(&s.weighted_siege_units)
                    .all(|(u, w)| u.is_some() && w.is_some()),
                "§3.5: DistantBattleUnit_Entry and DistantBattleUnit_Weight"
            );
            waves += s.waves.len();
            objectives += s.objectives.len();
            for u in s.units() {
                if class_of(u) != Some(3725) {
                    problems.push(format!("N-I2: script unit class {:?}", class_of(u)));
                }
                units_named.insert(u);
            }
        }
    }
    eprintln!(
        "distant battles: {waves} waves, {objectives} objectives, {} units named",
        units_named.len()
    );
    assert_eq!(
        (waves, objectives, units_named.len()),
        (41, 85, 60),
        "§8.3, §14.2"
    );
    let bb = one(InstanceKind::BigBattleControl);
    assert_eq!(bb.len(), 1);
    if let InstanceRecord::BigBattleControl(b) = bb[0].1 {
        assert_eq!(b.medals.len(), 5);
        assert_eq!(b.quest_points.len(), 107);
        assert_eq!(b.quest_encounters.len(), 99);
        let targets: BTreeSet<Did> = b.quest_encounters.iter().filter_map(|(_, e)| *e).collect();
        assert_eq!(targets, big_battles, "§8.1 quest → encounter targets");
        assert_eq!(b.location_events.len(), 3);
        assert!(
            b.quest_points
                .iter()
                .all(|(q, p)| q.is_some() && p.is_some())
        );
        assert!(
            b.location_events
                .iter()
                .all(|(l, e)| l.is_some() && e.is_some())
        );
    }

    // §8.4, §8.5, N-I10.
    let fc = one(InstanceKind::FormationControl);
    assert_eq!(fc.len(), 1);
    let InstanceRecord::FormationControl(form) = fc[0].1 else {
        unreachable!()
    };
    assert_eq!(form.widths.len(), 7);
    assert!(form.widths.iter().all(|(c, w)| c.is_some() && w.is_some()));
    let (mut units, mut groups, mut offsets, mut unit_devs) = (0, 0, 0, 0);
    for (d, v) in one(InstanceKind::DistantBattleUnit) {
        let InstanceRecord::DistantBattleUnit(u) = v else {
            continue;
        };
        units += 1;
        groups += u.groups.len();
        offsets += u.figure_count();
        let width = u.width_class.and_then(|c| form.actual_width(c));
        let devs = u.deviations(width);
        if !devs.is_empty() {
            unit_devs += devs.len();
            report(&format!("{d:#010x}"), &devs);
        }
    }
    eprintln!(
        "distant-battle units: {units}, {groups} groups, {offsets} offsets, {unit_devs} deviations"
    );
    assert_eq!((units, groups, offsets), (96, 376, 26_581), "§8.4");
    assert_eq!(unit_devs, 0, "N-I10");

    // §8.7, §9, §10.1.
    let stats = one(InstanceKind::BattleStat);
    let player_scope = stats
        .iter()
        .filter(|(_, v)| matches!(v, InstanceRecord::BattleStat(s) if s.scope == Some(3)))
        .count();
    assert_eq!((stats.len(), player_scope), (13, 13), "§8.7");
    let sp = one(InstanceKind::SessionPlayControl);
    if let [(_, InstanceRecord::SessionPlayControl(s))] = sp.as_slice() {
        assert_eq!(s.slots.len(), 87, "§9");
    } else {
        panic!("one session-play control expected");
    }
    let fm = one(InstanceKind::FellowshipMakerControl);
    let [(_, InstanceRecord::FellowshipMakerControl(f))] = fm.as_slice() else {
        panic!("one fellowship-maker control expected");
    };
    assert_eq!(f.valid_sizes, vec![2, 3, 6]);
    assert_eq!(
        (
            f.match_time.len(),
            f.bonus_thresholds.len(),
            f.excluded.len()
        ),
        (3, 5, 1)
    );
    assert!(f.match_time.iter().all(|(a, b)| a.is_some() && b.is_some()));
    assert!(
        f.bonus_thresholds
            .iter()
            .all(|(a, b)| a.is_some() && b.is_some())
    );
    assert_eq!(f.min_level, Some(10));

    // §10.2 item 2, I29: the flag bits come from the mapper named by
    // `FellowshipMaker_PlayerConfiguration`, by key label.
    let mapper_did = FlagBits::mapper_did(&dict).expect("configuration mapper");
    let mapper = tessera_formats::decode_enum_mapper(&set.read(mapper_did.0).expect("mapper"))
        .expect("mapper decodes");
    let bits = FlagBits::from_mapper(&mapper);
    eprintln!(
        "configuration mapper {:#010x}: {} keys, {} bits, kind mask {:#x}, sizes {:?}, tiers {:?}",
        mapper_did.0,
        mapper.names().len(),
        bits.keys.len(),
        bits.kind_mask(),
        bits.sizes,
        bits.tiers
    );
    assert_eq!(
        mapper.names().len(),
        21,
        "§10.2: 21 keys (0 and 1–21 except 15)"
    );
    assert_eq!(bits.keys.len(), 20, "§10.2 item 3: 20 non-zero keys");
    assert!(!bits.keys.contains(&0x4000));
    assert_eq!(bits.kind_mask(), 0x18_000F, "§15.1 B8: the six Type_ bits");
    assert_eq!(
        bits.sizes.keys().copied().collect::<Vec<_>>(),
        vec![1, 2, 3, 6]
    );
    assert_eq!(bits.tiers.len(), 10);

    // §10.2 kind bits over the 166 templates with a category.
    let rules = MatchingRules {
        bits,
        valid_sizes: f.valid_sizes.clone(),
        capped: w.capped,
        seasonal: w.seasonal.iter().copied().collect(),
    };
    let mut kind_counts: BTreeMap<Option<u32>, usize> = BTreeMap::new();
    for (d, (_, t)) in &templates {
        if t.world_join_category.is_some() {
            let m = MatchingTemplate::from_template(Did(*d), t);
            *kind_counts.entry(kind_bit(&m, &rules)).or_default() += 1;
        }
    }
    eprintln!("kind bits of the category templates: {kind_counts:?}");
    let expected: BTreeMap<Option<u32>, usize> = [
        (Some(0x1), 22),
        (Some(0x2), 5),
        (Some(0x4), 118),
        (Some(0x8_0000), 14),
        (Some(0x10_0000), 7),
    ]
    .into();
    assert_eq!(
        kind_counts, expected,
        "§10.2: kind bits disjoint, per-kind counts"
    );

    // The index rebuilt over every finder template with a valid size (which templates get
    // entries is OPEN, so the count is reported, not checked).
    let excluded: BTreeSet<Did> = f.excluded.iter().copied().collect();
    let finder: Vec<MatchingTemplate> = w
        .finder_order()
        .into_iter()
        .filter(|d| !excluded.contains(d))
        .filter_map(|d| {
            templates
                .get(&d.0)
                .map(|(_, t)| MatchingTemplate::from_template(d, t))
        })
        .collect();
    let idx = MatchingIndex::build(&finder, &rules);
    eprintln!(
        "matching index over {} finder templates: {} entries for {} templates (the object form holds 557 for 107; selection OPEN)",
        finder.len(),
        idx.entries.len(),
        idx.encounters.len()
    );
    assert!(idx.kind_conflicts.is_empty());
    assert!(idx.unlabelled_kinds.is_empty() && idx.unencodable.is_empty());
    assert_eq!(
        idx.by_flag.len(),
        20,
        "N-I11: one key per non-zero mapper key"
    );
    // N-I11: one kind, one size and one tier bit per entry.
    let kinds = rules.bits.kind_mask();
    let sizes = rules.bits.sizes.values().fold(0, |a, b| a | b);
    let tiers = rules.bits.tiers.values().fold(0, |a, b| a | b);
    assert!(
        idx.entries
            .iter()
            .all(|e| (e.flags & kinds).count_ones() == 1
                && (e.flags & sizes).count_ones() == 1
                && (e.flags & tiers).count_ones() == 1
                && e.flags & !(kinds | sizes | tiers) == 0)
    );

    for p in problems.iter().take(20) {
        eprintln!("  {p}");
    }
    assert!(problems.is_empty(), "{} reference problems", problems.len());
}
