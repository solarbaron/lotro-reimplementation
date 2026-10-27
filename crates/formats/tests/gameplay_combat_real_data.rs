// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data census of the combat families (`tessera_formats::gameplay::combat`,
//! `docs/specs/combat-data.md`) against a user's own install, read-only.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. `combat_families` walks **every**
//! record of every family of §2 and of the related objects, on both halves, and checks the
//! spec's invariants (always asserted) and its census counts. Census counts describe one
//! install version: the owner's install as patched on 2026-10-27 (`combat-data.md` still
//! states the figures of the install before that patch until it is re-measured; the labels
//! say "spec" for the pinned value); set `TESSERA_CENSUS_REPORT_ONLY=1` to report

//! differences (for example after a game patch) without failing. `modifier_entries_whole_install`
//! repeats the §5 census over every object record of the install and runs only with
//! `TESSERA_FULL=1`.
//!
//! Run in release mode; the walk reads several hundred megabytes:
//! `cargo test --release -p tessera-formats --test gameplay_combat_real_data -- --nocapture`.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tessera_formats::gameplay::combat::{
    AdvancementLine, BATTLE_STATS_MAP_LABEL, BattleStatistic, COMBAT_EFFECTS_MAP_LABEL,
    Channelling, ClassTable, CombatClasses, CombatControl, CombatFamily, CombatPids, Effect,
    EffectControl, Induction, ModEntryKind, ModOp, MonsterPlayTable, Progression, ProgressionData,
    ProgressionKind, QualityTable, Skill, SparseDidProgression, ThresholdPolicy, Trait,
    TraitControl, TraitTree, mod_arrays, mod_entries, ranged_groups, value_arrays_present,
};
use tessera_formats::gameplay::common::{
    ClassIndex, GameObjectRecord, ObjectRoot, Pid, PropertyAccess, did_references,
};
use tessera_formats::{Did, PropertyDictionary, PropertyValue};
use tessera_wstate::{ClassLibrary, ObjectState, ObjectStore, ScriptView, object_dids};

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn env_flag(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|v| v == "1")
}

// ---------------------------------------------------------------------------------------------
// Census bookkeeping
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct Census {
    rows: Vec<(String, String, String)>,
    mismatches: Vec<String>,
    broken: Vec<String>,
    notes: Vec<String>,
}

impl Census {
    /// A census count of the spec (fails unless report-only).
    fn eq(
        &mut self,
        what: impl Into<String>,
        expected: impl std::fmt::Debug + PartialEq<u64>,
        observed: u64,
    ) {
        let what = what.into();
        let ok = expected == observed;
        if !ok {
            self.mismatches
                .push(format!("{what}: spec {expected:?}, observed {observed}"));
        }
        self.rows.push((
            what,
            format!("{expected:?}"),
            format!("{observed}{}", if ok { "" } else { "  <-- differs" }),
        ));
    }

    /// A histogram census.
    fn map<K: Ord + std::fmt::Debug + Copy>(
        &mut self,
        what: &str,
        expected: &[(K, u64)],
        observed: &BTreeMap<K, u64>,
    ) {
        let exp: BTreeMap<K, u64> = expected.iter().copied().collect();
        let ok = &exp == observed;
        let show = |m: &BTreeMap<K, u64>| {
            let mut s = String::new();
            for (k, v) in m {
                let _ = write!(s, "{k:?}:{v} ");
            }
            s
        };
        if !ok {
            self.mismatches.push(format!(
                "{what}: spec {}, observed {}",
                show(&exp),
                show(observed)
            ));
        }
        self.rows.push((
            what.to_owned(),
            show(&exp),
            format!("{}{}", show(observed), if ok { "" } else { " <-- differs" }),
        ));
    }

    /// A spec statement this census does not reproduce, filed as an OPEN note in the spec.
    /// Reported, never failing. (No such statement is open after the 2026-10-09 revision.)
    #[allow(dead_code)]
    fn note(
        &mut self,
        what: impl Into<String>,
        spec: impl std::fmt::Display,
        observed: impl std::fmt::Display,
    ) {
        let what = what.into();
        self.notes
            .push(format!("{what}: spec {spec}, observed {observed}"));
        self.rows.push((
            format!("[OPEN] {what}"),
            spec.to_string(),
            observed.to_string(),
        ));
    }

    /// An invariant (always asserted): `holds` of `total`.
    fn inv(&mut self, what: impl Into<String>, holds: u64, total: u64) {
        let what = what.into();
        if holds != total {
            self.broken
                .push(format!("{what}: holds for {holds} of {total}"));
        }
        self.rows.push((
            format!("[inv] {what}"),
            format!("{total}"),
            format!("{holds}"),
        ));
    }

    fn finish(self) {
        let mut out = String::from("\ncombat-data census (what | spec | observed)\n");
        for (w, e, o) in &self.rows {
            let _ = writeln!(out, "  {w} | {e} | {o}");
        }
        eprintln!("{out}");
        if !self.notes.is_empty() {
            eprintln!(
                "spec statements not reproduced (OPEN in combat-data.md):\n  {}",
                self.notes.join("\n  ")
            );
        }
        assert!(
            self.broken.is_empty(),
            "combat-data invariants broken:\n  {}",
            self.broken.join("\n  ")
        );
        if !self.mismatches.is_empty() {
            let msg = format!(
                "{} census counts differ from combat-data.md:\n  {}",
                self.mismatches.len(),
                self.mismatches.join("\n  ")
            );
            if env_flag("TESSERA_CENSUS_REPORT_ONLY") {
                eprintln!("{msg}");
            } else {
                panic!("{msg}");
            }
        }
    }
}

fn bump<K: Ord>(m: &mut BTreeMap<K, u64>, k: K) {
    *m.entry(k).or_default() += 1;
}

// ---------------------------------------------------------------------------------------------
// Install access
// ---------------------------------------------------------------------------------------------

struct Install {
    store: ObjectStore,
    library: Arc<ClassLibrary>,
    dict: PropertyDictionary,
    pids: CombatPids,
    classes: CombatClasses,
    index: ClassIndex,
}

struct Loaded {
    record: GameObjectRecord,
    state: ObjectState,
    bytes_70: u64,
    bytes_79: u64,
}

impl Install {
    fn open(dir: &std::path::Path) -> Self {
        let store = ObjectStore::open(dir).expect("open install");
        let library = store.library().expect("class library");
        let dict = store
            .game_data()
            .registry()
            .expect("dictionary")
            .dictionary()
            .clone();
        let pids = CombatPids::resolve(&dict);
        let started = Instant::now();
        let dids = object_dids(store.game_data().source()).expect("object DIDs");
        let mut index = ClassIndex::new();
        for &did in &dids {
            let payload = store
                .game_data()
                .read(did)
                .expect("read")
                .expect("listed record exists");
            let root = ObjectRoot::read_expecting(&payload, did).expect("object root");
            index.insert_root(root);
        }
        eprintln!(
            "indexed {} object records in {:.1?}",
            index.len(),
            started.elapsed()
        );
        Self {
            store,
            library,
            dict,
            pids,
            classes: CombatClasses::observed(),
            index,
        }
    }

    fn pid(&self, label: &'static str) -> Pid {
        Pid::resolve(&self.dict, label)
    }

    /// Both halves of `did`; F1 (a 0x79 partner exists, both decode completely) asserted.
    fn load(&self, did: Did) -> Loaded {
        let data = self.store.game_data();
        let payload = data.read(did).expect("read").expect("object record");
        let set_did = ObjectRoot::read(&payload)
            .expect("root")
            .property_set_did()
            .expect("F1: property-set DID");
        let set = data
            .read(set_did)
            .expect("read set")
            .unwrap_or_else(|| panic!("F1: {did} has a property set"));
        let record = GameObjectRecord::decode(&payload, Some(&set), &self.dict)
            .unwrap_or_else(|e| panic!("F1: {did} property half: {e}"));
        let state = self
            .store
            .decode(did, &payload)
            .unwrap_or_else(|e| panic!("F1: {did} object half: {e}"));
        assert!(state.is_complete(), "F1: {did} object half is partial");
        Loaded {
            record,
            state,
            bytes_70: payload.len() as u64,
            bytes_79: set.len() as u64,
        }
    }

    fn class_of(&self, did: Did) -> Option<u32> {
        self.index.root_class(did)
    }

    fn family_of(&self, did: Did) -> Option<CombatFamily> {
        self.classes.family(&*self.library, self.class_of(did)?)
    }

    fn dids_of_family(&self, family: CombatFamily) -> Vec<Did> {
        self.index
            .classes()
            .filter(|&(c, _)| self.classes.family(&*self.library, c) == Some(family))
            .flat_map(|(c, _)| self.index.dids_of_class(c).iter().copied())
            .collect()
    }

    /// G1: every object DID the set names exists.
    fn dangling(&self, rec: &GameObjectRecord) -> u64 {
        did_references(&rec.properties)
            .iter()
            .filter(|r| matches!(r.target.prefix(), 0x70..=0x77))
            .filter(|r| self.class_of(r.target).is_none())
            .count() as u64
    }
}

fn scripts(state: &ObjectState) -> impl Iterator<Item = ScriptView<'_>> {
    (0..state.items().len()).filter_map(|i| state.item(i)?.as_script())
}

fn count_class(state: &ObjectState, class: u32) -> u64 {
    scripts(state).filter(|s| s.class() == class).count() as u64
}

// Attribute hashes of the compiled half (combat-data.md §4.7).
const ATTR_MIN_INDEX: u32 = 0x08D9_B2F5;
const ATTR_SEGMENTS_START: u32 = 0x0AA1_2EE4;
const ATTR_SEGMENT_SLOPE: u32 = 0x05BA_3155;
const ATTR_SEGMENT_END: u32 = 0x0CF2_D1F4;
const ATTR_POINT_KEY: u32 = 0x0735_E1C9;
// Compiled classes (§3.2, §4.7, §5.8).
const CLASS_SEGMENT: u32 = 3917;
const CLASS_HOOK: u32 = 822;
const CLASS_ARC: u32 = 819;
const CLASS_BOX: u32 = 820;
const CLASS_SPHERE: u32 = 821;
const CLASS_INLINE_MOD: u32 = 1339;
const CLASS_PROGRESSION_MOD: u32 = 1342;

/// (start key, end key, slope bits) of every compiled segment object.
fn compiled_segments(state: &ObjectState) -> Vec<(i32, i32, u32)> {
    let mut out: Vec<(i32, i32, u32)> = scripts(state)
        .filter(|s| s.class() == CLASS_SEGMENT)
        .filter_map(|s| {
            let key = |h| -> Option<i32> {
                Some(s.follow(h)?.item()?.as_script()?.int(ATTR_POINT_KEY)? as i32)
            };
            Some((
                key(ATTR_SEGMENTS_START)?,
                key(ATTR_SEGMENT_END)?,
                s.float(ATTR_SEGMENT_SLOPE)?.to_bits(),
            ))
        })
        .collect();
    out.sort_unstable();
    out
}

// ---------------------------------------------------------------------------------------------
// The census
// ---------------------------------------------------------------------------------------------

#[test]
fn combat_families() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let started = Instant::now();
    let inst = Install::open(&dir);
    let mut c = Census::default();

    let missing = inst.pids.missing();
    eprintln!("combat labels not in the dictionary: {missing:?}");
    c.eq(
        "labels of CombatPids not defined by the dictionary",
        0,
        missing.len() as u64,
    );

    families_census(&inst, &mut c);
    progressions(&inst, &mut c);
    effects(&inst, &mut c);
    skills(&inst, &mut c);
    traits(&inst, &mut c);
    tables(&inst, &mut c);
    related(&inst, &mut c);
    maps(&inst, &mut c);

    eprintln!("combat census finished in {:.1?}", started.elapsed());
    c.finish();
}

/// §2.2 and F1, F2, G1 for every family.
fn families_census(inst: &Install, c: &mut Census) {
    let expected: [(CombatFamily, u64, u64, u64, u64); 8] = [
        (CombatFamily::Skill, 1, 13_250, 30_692_351, 36_127_478),
        (CombatFamily::Effect, 87, 32_977, 17_186_874, 15_031_822),
        (CombatFamily::Progression, 6, 7_558, 20_337_190, 10_009_116),
        (CombatFamily::Trait, 9, 3_965, 2_171_967, 1_608_033),
        (
            CombatFamily::MonsterPlayOrAdvancementLine,
            3,
            141,
            80_743,
            117_867,
        ),
        (CombatFamily::ClassTable, 1, 12, 312_716, 906_720),
        (CombatFamily::ThresholdPolicy, 1, 6, 6_488, 1_602),
        (CombatFamily::BattleStatistic, 2, 13, 1_274, 1_595),
    ];
    for (family, classes, records, b70, b79) in expected {
        let dids = inst.dids_of_family(family);
        let mut class_set = BTreeSet::new();
        let (mut s70, mut s79, mut sig, mut dangling) = (0u64, 0u64, 0u64, 0u64);
        for &did in &dids {
            let l = inst.load(did);
            class_set.insert(l.record.root_class());
            s70 += l.bytes_70;
            s79 += l.bytes_79;
            sig += u64::from(inst.pids.has_signature(family, &l.record.properties));
            dangling += inst.dangling(&l.record);
        }
        let n = dids.len() as u64;
        c.eq(
            format!("{family:?}: classes with records"),
            classes,
            class_set.len() as u64,
        );
        c.eq(format!("{family:?}: records"), records, n);
        c.eq(format!("{family:?}: 0x70 bytes"), b70, s70);
        c.eq(format!("{family:?}: 0x79 bytes"), b79, s79);
        c.inv(format!("F1 {family:?}: both halves decode"), n, n);
        c.inv(format!("F2 {family:?}: signature properties"), sig, n);
        c.inv(
            format!("G1 {family:?}: no dangling object DID"),
            0,
            dangling,
        );
    }
    let per_class = |f: CombatFamily| -> BTreeMap<u32, u64> {
        inst.dids_of_family(f)
            .iter()
            .filter_map(|&d| inst.class_of(d))
            .fold(BTreeMap::new(), |mut m, k| {
                bump(&mut m, k);
                m
            })
    };
    c.map(
        "§2.2 trait classes",
        &[
            (1477, 1_775),
            (3438, 946),
            (3509, 767),
            (2525, 244),
            (1483, 125),
            (3564, 45),
            (1494, 37),
            (1478, 25),
            (3712, 1),
        ],
        &per_class(CombatFamily::Trait),
    );
    c.map(
        "§2.2 progression classes",
        &[
            (1389, 4_303),
            (3918, 1_814),
            (1385, 1_338),
            (2402, 46),
            (3810, 45),
            (2519, 12),
        ],
        &per_class(CombatFamily::Progression),
    );
    c.map(
        "§2.2 monster-play and line classes",
        &[(1932, 67), (2390, 59), (2399, 15)],
        &per_class(CombatFamily::MonsterPlayOrAdvancementLine),
    );
    c.map(
        "§2.2 battle-statistic classes",
        &[(3590, 10), (3622, 3)],
        &per_class(CombatFamily::BattleStatistic),
    );
    let effects = per_class(CombatFamily::Effect);
    let top: [(u32, u64); 26] = [
        (734, 14_476),
        (755, 2_438),
        (719, 1_547),
        (708, 1_245),
        (767, 1_189),
        (725, 1_103),
        (718, 1_076),
        (763, 767),
        (2459, 724),
        (713, 666),
        (749, 631),
        (709, 617),
        (715, 549),
        (3866, 526),
        (5724, 470),
        (3686, 461),
        (2762, 453),
        (724, 429),
        (736, 356),
        (776, 328),
        (737, 310),
        (769, 239),
        (722, 238),
        (745, 222),
        (2063, 163),
        (3842, 132),
    ];
    let observed_top: BTreeMap<u32, u64> = effects
        .iter()
        .filter(|&(_, &n)| n >= 125)
        .map(|(&k, &v)| (k, v))
        .collect();
    c.map(
        "§2.2 effect classes with 125 or more records",
        &top,
        &observed_top,
    );
    c.eq(
        "§2.2 effect classes with fewer than 125 records",
        61,
        effects.values().filter(|&&n| n < 125).count() as u64,
    );
}

/// §4: every progression.
fn progressions(inst: &Install, c: &mut Census) {
    let pp = &inst.pids.progression;
    let dids = inst.dids_of_family(CombatFamily::Progression);
    let (mut p1, mut p2, mut p2_total, mut p3, mut p4_records, mut p5, mut p6, mut p6_total) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
    let mut kind_by_class = 0u64;
    let mut lengths: HashMap<ProgressionKind, BTreeMap<usize, u64>> = HashMap::new();
    let mut element_types: BTreeMap<String, u64> = BTreeMap::new();
    let mut min_values: BTreeMap<i32, u64> = BTreeMap::new();
    let (mut floats_with_min, mut type_present, mut tier_present) = (0u64, 0u64, 0u64);
    let mut type_by_kind: BTreeMap<String, u64> = BTreeMap::new();
    let (mut float_value_prop, mut linear_value_prop, mut inner_prop) = (0u64, 0u64, 0u64);
    let mut first_keys: BTreeMap<i32, u64> = BTreeMap::new();
    let (mut keys_in_range, mut last_in_range, mut monotonic, mut segments) =
        (0u64, 0u64, 0u64, 0u64);
    let mut did_targets: BTreeMap<u32, u64> = BTreeMap::new();
    let mut nested_targets: BTreeMap<u32, u64> = BTreeMap::new();
    let mut nested_min: BTreeMap<i32, u64> = BTreeMap::new();
    let (mut linear_n, mut evaluated) = (0u64, 0u64);
    for &did in &dids {
        let l = inst.load(did);
        let class = l.record.root_class();
        let set = &l.record.properties;
        p1 += u64::from(value_arrays_present(set, pp) == 1);
        let prog = Progression::read(&l.record, &inst.classes, &*inst.library, pp)
            .unwrap_or_else(|e| panic!("{did}: {e}"));
        let kind = prog.kind();
        bump(&mut kinds, format!("{kind:?}"));
        kind_by_class +=
            u64::from(inst.classes.progression_kind(&*inst.library, class) == Some(kind));
        bump(lengths.entry(kind).or_default(), prog.len());
        if let Some(m) = prog.stored_min_index {
            bump(&mut min_values, m);
            floats_with_min += u64::from(kind == ProgressionKind::FloatArray);
        }
        if set.has(pp.progression_type) {
            type_present += 1;
            bump(&mut type_by_kind, format!("{kind:?}"));
        }
        tier_present += u64::from(set.has(pp.point_tier));
        float_value_prop += u64::from(set.has(pp.float_value_property));
        linear_value_prop += u64::from(set.has(pp.linear_value_property));
        inner_prop += u64::from(set.has(pp.inner_property));

        // P5: compiled minimum index.
        let compiled_min = l
            .state
            .root_object()
            .and_then(|o| o.int(ATTR_MIN_INDEX))
            .map(|v| i64::from(v as i32));
        p5 += u64::from(compiled_min == Some(prog.min_index()));

        // Every covered index evaluates (no panic, no none).
        if let Some((lo, hi)) = prog.covered_range() {
            for x in [lo, (lo + hi) / 2, hi] {
                if kind != ProgressionKind::ProgressionOfProgressions {
                    evaluated += u64::from(prog.value_at(x).is_some());
                }
            }
        }

        match &prog.data {
            ProgressionData::Property(v) => {
                p2_total += 1;
                p2 += u64::from(prog.produced_property().is_some());
                if let Some(first) = v.first() {
                    bump(
                        &mut element_types,
                        format!("{:?}", first.value.property_type()),
                    );
                }
            }
            ProgressionData::Linear(points) => {
                linear_n += 1;
                p3 += u64::from(prog.keys_increasing());
                if let Some(first) = points.first() {
                    bump(&mut first_keys, first.key);
                }
                keys_in_range += u64::from(points.iter().all(|p| (0..=999).contains(&p.key)));
                last_in_range +=
                    u64::from(points.last().is_some_and(|p| (5..=999).contains(&p.key)));
                let up = points.windows(2).all(|w| w[0].value <= w[1].value);
                let down = points.windows(2).all(|w| w[0].value >= w[1].value);
                monotonic += u64::from(up || down);
                let ours: Vec<(i32, i32, u32)> = {
                    let mut v: Vec<_> = prog
                        .segments()
                        .iter()
                        .map(|s| (s.start.key, s.end.key, s.slope.to_bits()))
                        .collect();
                    v.sort_unstable();
                    v
                };
                segments += ours.len() as u64;
                p4_records += u64::from(ours == compiled_segments(&l.state));
            }
            ProgressionData::Did(v) => {
                for d in v {
                    if d.is_null() {
                        bump(&mut did_targets, 0);
                    } else if let Some(cl) = inst.class_of(*d) {
                        bump(&mut did_targets, cl);
                    }
                }
            }
            ProgressionData::Progressions(v) => {
                if let Some(m) = prog.stored_min_index {
                    bump(&mut nested_min, m);
                }
                for d in v {
                    p6_total += 1;
                    let target = inst.class_of(*d);
                    let is_prog =
                        !d.is_null() && inst.family_of(*d) == Some(CombatFamily::Progression);
                    p6 += u64::from(is_prog);
                    if let Some(cl) = target {
                        bump(&mut nested_targets, cl);
                    }
                }
            }
            _ => {}
        }
    }
    let n = dids.len() as u64;
    c.inv("P1 exactly one value array", p1, n);
    c.inv(
        "§4.1 kind from the value array agrees with the class",
        kind_by_class,
        n,
    );
    c.inv("P2 property arrays share one PID", p2, p2_total);
    c.inv("P3 linear keys strictly increase", p3, linear_n);
    c.inv(
        "P4 compiled segments equal ours, slopes bit-exact (records)",
        p4_records,
        linear_n,
    );
    c.inv("P5 compiled minimum index", p5, n);
    c.inv(
        "P6 progression-of-progression elements name progressions",
        p6,
        p6_total,
    );
    c.inv(
        "§4.10 value at first, middle and last covered index",
        evaluated,
        3 * (n - kinds.get("ProgressionOfProgressions").copied().unwrap_or(0)),
    );
    c.map(
        "§4.1 property-array lengths",
        &[
            (200, 3_174),
            (160, 585),
            (80, 289),
            (25, 106),
            (6, 69),
            (301, 27),
            (159, 18),
            (16, 13),
            (599, 11),
            (258, 9),
            (20, 1),
            (45, 1),
        ],
        &lengths
            .remove(&ProgressionKind::PropertyArray)
            .unwrap_or_default(),
    );
    c.map(
        "§4.1 float-array lengths",
        &[
            (200, 392),
            (500, 282),
            (80, 249),
            (150, 193),
            (160, 125),
            (100, 42),
            (120, 8),
            (15, 7),
            (2, 6),
            (19, 6),
            (5, 5),
            // Length 8 occurs once (C53).
            (8, 1),
            (10, 5),
            (101, 4),
            (16, 2),
            (301, 2),
            (401, 2),
            (700, 2),
            (7, 1),
            (20, 1),
            (36, 1),
            (45, 1),
            (250, 1),
        ],
        &lengths
            .remove(&ProgressionKind::FloatArray)
            .unwrap_or_default(),
    );
    c.map(
        "§4.1 linear point counts",
        &[
            (19, 913),
            (20, 453),
            (22, 211),
            (2, 112),
            (4, 59),
            (5, 46),
            (3, 13),
            (21, 5),
            (14, 1),
            (17, 1),
        ],
        &lengths
            .remove(&ProgressionKind::LinearInterpolating)
            .unwrap_or_default(),
    );
    c.map(
        "§4.1 text-array lengths",
        &[(150, 40), (11, 4), (7, 1), (12, 1)],
        &lengths
            .remove(&ProgressionKind::TextArray)
            .unwrap_or_default(),
    );
    c.map(
        "§4.1 DID-array lengths",
        &[(200, 12)],
        &lengths
            .remove(&ProgressionKind::DidArray)
            .unwrap_or_default(),
    );
    c.map(
        "§4.1 progression-of-progressions lengths",
        &[(5, 19), (7, 15), (10, 9), (6, 2)],
        &lengths
            .remove(&ProgressionKind::ProgressionOfProgressions)
            .unwrap_or_default(),
    );
    let element_types_ref: BTreeMap<&str, u64> = element_types
        .iter()
        .map(|(k, v)| (k.as_str(), *v))
        .collect();
    c.map(
        "§4.1 property-array element types",
        &[
            ("Float", 3_939),
            ("Int", 319),
            ("DidRef", 22),
            ("Bitfield32", 20),
            ("Enum", 3),
        ],
        &element_types_ref,
    );
    c.eq(
        "§4.2 records with Progression_MinimumIndexValue",
        5_738,
        min_values.values().sum(),
    );
    c.map(
        "§4.2 minimum index values",
        &[(1, 5_721), (0, 9), (-9, 6), (-100, 2)],
        &min_values,
    );
    c.eq(
        "§4.2 float arrays with a minimum index",
        1_332,
        floats_with_min,
    );
    c.eq("§4.2 Progression_Type present", 5_029, type_present);
    c.eq("§4.2 Progression_PointTier present", 5_029, tier_present);
    c.eq(
        "§4.2 Progression_Type in property arrays",
        3_207,
        type_by_kind.get("PropertyArray").copied().unwrap_or(0),
    );
    c.eq(
        "§4.2 Progression_Type in float arrays",
        8,
        type_by_kind.get("FloatArray").copied().unwrap_or(0),
    );
    c.eq("§4.2 FloatProgression_ValueProperty", 698, float_value_prop);
    c.eq(
        "§4.2 LinearInterpolatingProgression_ValueProperty",
        1_814,
        linear_value_prop,
    );
    c.eq(
        "§4.2 ProgressionProgression_InnerRepresentedPropertyName",
        45,
        inner_prop,
    );
    c.map(
        "§4.4 first linear keys",
        &[(1, 1_807), (0, 6), (115, 1)],
        &first_keys,
    );
    c.inv("§4.4 linear keys lie in 0–999", keys_in_range, linear_n);
    c.inv(
        "§4.4 last linear key lies in 5–999",
        last_in_range,
        linear_n,
    );
    c.eq("§4.4 monotonic linear progressions", 1_787, monotonic);
    c.eq("§4.4 compiled segments", 30_100, segments);
    let did_targets_grouped: BTreeMap<&str, u64> =
        did_targets
            .iter()
            .fold(BTreeMap::new(), |mut m, (&cl, &n)| {
                let k = match cl {
                    0 => "zero",
                    734 => "effect 734",
                    1385 => "float progression",
                    _ => "other",
                };
                *m.entry(k).or_default() += n;
                m
            });
    c.map(
        "§4.3 DID-array targets",
        &[
            ("effect 734", 1_283),
            ("float progression", 1_000),
            ("zero", 117),
        ],
        &did_targets_grouped,
    );
    c.map(
        "§4.5 progression-of-progression targets",
        &[(3918, 287), (1385, 10), (1389, 5)],
        &nested_targets,
    );
    c.map(
        "§4.5 progression-of-progression minimum index",
        &[(1, 45)],
        &nested_min,
    );
}

fn present(set: &impl PropertyAccess, p: Pid) -> u64 {
    u64::from(set.has(p))
}

/// §6: every effect.
fn effects(inst: &Install, c: &mut Census) {
    let pids = &inst.pids;
    let ef = &pids.effect;
    let x = &pids.effect_extra;
    let m = &pids.modifier;
    let dids = inst.dids_of_family(CombatFamily::Effect);
    let mut presence: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut groups: BTreeMap<String, u64> = BTreeMap::new();
    let mut stored_false_by_group: BTreeMap<(String, u32), u64> = BTreeMap::new();
    let (mut e2, mut e2_total) = (0u64, 0u64);
    let (mut additive_nonzero, mut prob_prog_nonzero, mut instant_true, mut eqv_zero) =
        (0u64, 0u64, 0u64, 0u64);
    let mut prog_pulse_groups: BTreeMap<String, u64> = BTreeMap::new();
    let mut mod_holders: BTreeMap<u32, u64> = BTreeMap::new();
    let mut entry_kinds: BTreeMap<String, u64> = BTreeMap::new();
    let (mut mod_prog_fields, mut ranged) = (0u64, 0u64);
    let mut mod_prog_targets: BTreeMap<u32, u64> = BTreeMap::new();
    let mut vital_refs: BTreeMap<(&'static str, u32), u64> = BTreeMap::new();
    let mut combos_725: BTreeMap<String, u64> = BTreeMap::new();
    let mut combos_755: BTreeMap<String, u64> = BTreeMap::new();
    // C57: class-725 sources with the variance left out, variance presence, and the
    // per-stem modifier lists and critical multipliers by full label.
    let mut combos_725_nv: BTreeMap<String, u64> = BTreeMap::new();
    let mut variances: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut stem_lists: BTreeMap<String, u64> = BTreeMap::new();
    let mut class_props: BTreeMap<(&'static str, u32), u64> = BTreeMap::new();
    let mut aura_types: BTreeMap<u32, u64> = BTreeMap::new();
    let mut struct_elems: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut refs: BTreeMap<String, u64> = BTreeMap::new();
    let (mut inline_mismatch, mut prog_mismatch) = (0u64, 0u64);
    let props: [(&'static str, Pid); 26] = [
        ("Effect_Icon", ef.icon),
        ("Effect_Harmful", ef.harmful),
        ("Effect_Debuff", ef.debuff),
        (
            "Effect_ConstantApplicationProbability",
            ef.application_probability,
        ),
        (
            "Effect_ApplicationProbabilityVariance",
            ef.probability_variance,
        ),
        (
            "Effect_ApplicationProbability_AdditiveModProp",
            ef.probability_additive_property,
        ),
        (
            "Effect_ApplicationProbabilityProgression",
            ef.probability_progression,
        ),
        ("Effect_Resist_Category", ef.resist_category),
        ("Effect_Resist_Level", ef.resist_level),
        ("Effect_Resist_Chance_Modifier", ef.resist_chance_modifier),
        ("Effect_Duration_ConstantInterval", ef.duration_interval),
        (
            "Effect_Duration_ConstantPulseCount",
            ef.duration_pulse_count,
        ),
        ("Effect_Duration_Permanent", ef.duration_permanent),
        ("Effect_Instantaneous", ef.instantaneous),
        (
            "Effect_Duration_ProgressionPulseCount",
            ef.duration_progression_pulse_count,
        ),
        (
            "Effect_Duration_ProgressionInterval",
            ef.duration_progression_interval,
        ),
        ("Effect_Duration_ExpiresInRealTime", ef.expires_in_real_time),
        ("Effect_Duration_CombatOnly", ef.combat_only),
        (
            "Effect_Duration_ConstantInterval_ModifierList",
            ef.interval_modifier_list,
        ),
        ("Effect_EquivalenceClass", ef.equivalence_class),
        ("Effect_ClassPriority", ef.class_priority),
        ("Effect_Cap", ef.cap),
        ("Effect_Critical_Chance", ef.critical_chance),
        ("Effect_Name", ef.name),
        ("Effect_Definition_Description", ef.definition_description),
        ("Effect_Applied_Description", ef.applied_description),
    ];
    let class_specific: [(&'static str, u32, Pid); 22] = [
        ("Effect_Aura_Type", 709, x.aura_type),
        ("Effect_Aura_Radius", 709, x.aura_radius),
        ("Effect_Aura_Applied_Effect_Array", 709, x.aura_applied),
        (
            "Effect_Aura_Spellcraft_Override",
            709,
            x.aura_spellcraft_override,
        ),
        ("Aura_ShouldAffectCaster", 709, x.aura_affects_caster),
        ("Effect_Area_MaxRange", 2762, x.area_max_range),
        ("Effect_Area_MaxTargets", 2762, x.area_max_targets),
        ("Effect_Area_Applied_Effect_Array", 2762, x.area_applied),
        (
            "Effect_ApplyOverTime_Applied_Effect_Array",
            708,
            x.apply_over_time,
        ),
        (
            "Effect_ApplyOverTime_Initial_Applied_Effect_Array",
            708,
            x.apply_over_time_initial,
        ),
        ("Effect_SkillProc_SkillTypes", 3686, x.proc_skill_types),
        (
            "Effect_SkillProc_RequiredCombatResult",
            3686,
            x.proc_required_result,
        ),
        ("Effect_SkillProc_ProcProbability", 3686, x.proc_probability),
        ("Effect_MinTimeBetweenProcs", 3686, x.proc_min_interval),
        ("Effect_Bubble_VitalType", 3222, x.bubble_vital_type),
        ("Effect_Bubble_Value", 3222, x.bubble_value),
        (
            "Effect_Bubble_Value_Progression",
            3222,
            x.bubble_value_progression,
        ),
        ("Effect_Bubble_Percentage", 3222, x.bubble_percentage),
        ("Effect_Pip_Type", 731, x.pip_type),
        ("Effect_Pip_AdjustmentAmount", 731, x.pip_amount),
        ("Effect_Pip_AdjustmentType", 731, x.pip_adjustment_type),
        ("Effect_Pip_Reset", 731, x.pip_reset),
    ];
    for &did in &dids {
        let l = inst.load(did);
        let set = &l.record.properties;
        let class = l.record.root_class();
        for (label, p) in props {
            *presence.entry(label).or_default() += present(set, p);
        }
        for (label, cl, p) in class_specific {
            if cl == class {
                *class_props.entry((label, cl)).or_default() += present(set, p);
            }
        }
        let eff = Effect::read(&l.record, pids).unwrap_or_else(|e| panic!("{did}: {e}"));
        let group = format!("{:?}", eff.duration.group());
        bump(&mut groups, group.clone());
        if eff.duration.instantaneous == Some(false) {
            bump(&mut stored_false_by_group, (group.clone(), class));
        }
        if let Some(p) = eff.duration.permanent {
            e2_total += 1;
            e2 += u64::from(p);
        }
        if eff.duration.pulse_count_progression.is_some() {
            bump(&mut prog_pulse_groups, group.clone());
        }
        if eff.duration.interval_progression.is_some() {
            bump(
                &mut prog_pulse_groups,
                format!("interval progression in {group}"),
            );
        }
        additive_nonzero += u64::from(eff.probability_additive_property.is_some());
        prob_prog_nonzero += u64::from(eff.probability_progression.is_some());
        instant_true += u64::from(eff.duration.instantaneous == Some(true));
        eqv_zero += u64::from(eff.equivalence_class == Some(0));
        if eff.aura.is_some()
            && let Some(t) = eff.aura.and_then(|a| a.aura_type)
        {
            bump(&mut aura_types, t);
        }
        for (label, p) in [
            ("aura applied", x.aura_applied),
            ("area applied", x.area_applied),
            ("apply over time", x.apply_over_time),
            ("apply over time initial", x.apply_over_time_initial),
        ] {
            *struct_elems.entry(label).or_default() += set.elements(p).len() as u64;
        }

        // §5 in effects.
        if set.has(m.array) {
            bump(&mut mod_holders, class);
        }
        for s in set.structs(m.array) {
            mod_prog_fields += present(s, m.progression);
        }
        let entries = mod_entries(set, m);
        for en in &entries {
            bump(&mut entry_kinds, format!("{:?}", en.kind));
            ranged += u64::from(en.range.is_ranged());
            if let Some(t) = en.progression.and_then(|d| inst.class_of(d)) {
                bump(&mut mod_prog_targets, t);
            }
        }
        let inline = entries
            .iter()
            .filter(|e| e.kind == ModEntryKind::Inline)
            .count() as u64;
        let driven = entries
            .iter()
            .filter(|e| e.kind == ModEntryKind::ProgressionDriven)
            .count() as u64;
        inline_mismatch += u64::from(count_class(&l.state, CLASS_INLINE_MOD) != inline);
        prog_mismatch += u64::from(count_class(&l.state, CLASS_PROGRESSION_MOD) != driven);

        // §6.4.
        for (label, amount) in [
            ("instant", eff.instant_vital),
            ("initial", eff.over_time_initial),
            ("per interval", eff.over_time_per_interval),
        ] {
            if let Some(t) = amount
                .and_then(|a| a.progression)
                .and_then(|d| inst.class_of(d))
            {
                *vital_refs.entry((label, t)).or_default() += 1;
            }
        }
        let parts = |a: Option<tessera_formats::gameplay::combat::VitalAmount>| -> String {
            let Some(a) = a else {
                return "none".into();
            };
            let mut v = Vec::new();
            if a.constant.is_some() {
                v.push("constant");
            }
            if a.progression.is_some() {
                v.push("progression");
            }
            if a.random.is_some() {
                v.push("random");
            }
            if a.variable.is_some() {
                v.push("variable");
            }
            if a.variance.is_some() {
                v.push("variance");
            }
            if v.is_empty() {
                "none".into()
            } else {
                v.join("+")
            }
        };
        if class == 725 {
            bump(&mut combos_725, parts(eff.instant_vital));
            bump(
                &mut combos_725_nv,
                parts(eff.instant_vital.map(|mut a| {
                    a.variance = None;
                    a
                })),
            );
        }
        for (label, amount) in [
            ("725 initial", eff.instant_vital.filter(|_| class == 725)),
            (
                "755 initial",
                eff.over_time_initial.filter(|_| class == 755),
            ),
            (
                "755 per interval",
                eff.over_time_per_interval.filter(|_| class == 755),
            ),
        ] {
            if amount.is_some_and(|a| a.variance.is_some()) {
                bump(&mut variances, label);
            }
        }
        // The counts of C57 are records of the stem's class (725 or 755).
        for (stem, m, stem_class) in [
            ("instant", &eff.instant_vital_modifiers, 725),
            ("initial", &eff.over_time_initial_modifiers, 755),
            ("per interval", &eff.over_time_per_interval_modifiers, 755),
        ] {
            if class != stem_class {
                continue;
            }
            for (part, present) in [
                ("modifier list", !m.modifier_list.is_empty()),
                ("additive list", !m.additive_modifier_list.is_empty()),
                ("critical multiplier", m.critical_multiplier.is_some()),
                (
                    "critical multiplier list",
                    !m.critical_multiplier_mod_list.is_empty(),
                ),
            ] {
                if present {
                    bump(&mut stem_lists, format!("{stem} {part}"));
                }
            }
        }
        if class == 755 {
            // The §6.4 grouping does not count the variance (it reproduces with it left out).
            let no_variance = |a: Option<tessera_formats::gameplay::combat::VitalAmount>| {
                parts(a.map(|mut a| {
                    a.variance = None;
                    a
                }))
            };
            let key = format!(
                "initial {} / per interval {}",
                no_variance(eff.over_time_initial),
                no_variance(eff.over_time_per_interval)
            );
            bump(&mut combos_755, key);
        }

        // §6.3.
        for r in &eff.references {
            let container = inst.dict.name(r.container).unwrap_or("?");
            let key = format!(
                "{:?} in {container}{}",
                r.kind,
                if r.is_placeholder() {
                    " (placeholder)"
                } else {
                    ""
                }
            );
            bump(&mut refs, key);
        }
    }
    let n = dids.len() as u64;
    c.inv(
        "E1 every effect in one observed duration group",
        n - groups.get("Other").copied().unwrap_or(0),
        n,
    );
    c.inv(
        "E2 Effect_Duration_Permanent is true when present",
        e2,
        e2_total,
    );
    let obs = |k: &str| presence.get(k).copied().unwrap_or(0);
    for (label, expected) in [
        ("Effect_Name", 32_977),
        ("Effect_Definition_Description", 32_977),
        ("Effect_Applied_Description", 32_977),
        ("Effect_Icon", 22_807),
        ("Effect_Harmful", 32_971),
        ("Effect_Debuff", 32_958),
        ("Effect_ConstantApplicationProbability", 32_628),
        ("Effect_ApplicationProbabilityVariance", 25_904),
        ("Effect_ApplicationProbability_AdditiveModProp", 22_361),
        ("Effect_ApplicationProbabilityProgression", 14_147),
        ("Effect_Resist_Category", 27_137),
        ("Effect_Resist_Level", 26_496),
        ("Effect_Resist_Chance_Modifier", 25_933),
        ("Effect_Duration_ConstantInterval", 10_114),
        ("Effect_Duration_ConstantPulseCount", 3_206),
        ("Effect_Duration_Permanent", 14_222),
        ("Effect_Instantaneous", 4_321),
        ("Effect_Duration_ProgressionPulseCount", 23),
        ("Effect_Duration_ProgressionInterval", 1),
        ("Effect_Duration_ExpiresInRealTime", 14_380),
        ("Effect_Duration_CombatOnly", 18_375),
        ("Effect_Duration_ConstantInterval_ModifierList", 486),
        ("Effect_EquivalenceClass", 22_641),
        ("Effect_ClassPriority", 22_679),
        ("Effect_Cap", 2_709),
        ("Effect_Critical_Chance", 3_612),
    ] {
        c.eq(format!("§6.2 {label} present"), expected, obs(label));
    }
    c.eq("§6.2 additive mod prop non-zero", 275, additive_nonzero);
    c.eq(
        "§6.2 application probability progression non-zero",
        1,
        prob_prog_nonzero,
    );
    c.eq("§6.2 Effect_Instantaneous stored true", 3_596, instant_true);
    c.eq("§6.2 Effect_EquivalenceClass 0", 12_480, eqv_zero);
    let g: Vec<(String, u64)> = [
        ("Unspecified", 6_703),
        ("Instantaneous", 3_596),
        ("Permanent", 12_564),
        ("Interval", 5_250),
        ("Pulsed", 3_206),
        ("IntervalPermanent", 1_658),
    ]
    .iter()
    .map(|&(k, v)| (k.to_owned(), v))
    .collect();
    let gref: Vec<(&str, u64)> = g.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    let groups_ref: BTreeMap<&str, u64> = groups.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    c.map("§6.2 duration groups", &gref, &groups_ref);
    let false_ref: BTreeMap<(&str, u32), u64> = stored_false_by_group
        .iter()
        .map(|((g, cl), v)| ((g.as_str(), *cl), *v))
        .collect();
    c.map(
        "§6.2 Effect_Instantaneous stored false, by group and class",
        &[(("Permanent", 2459), 724), (("Interval", 3943), 1)],
        &false_ref,
    );
    let pp_ref: BTreeMap<&str, u64> = prog_pulse_groups
        .iter()
        .map(|(k, v)| (k.as_str(), *v))
        .collect();
    c.map(
        "§6.2 duration progressions by group",
        &[("Interval", 23), ("interval progression in Unspecified", 1)],
        &pp_ref,
    );
    c.map(
        "§6.5 Mod_Array holders by effect class",
        &[
            (734, 14_476),
            (708, 1_245),
            (2459, 721),
            (713, 190),
            (736, 125),
            (3686, 62),
            (3222, 49),
            (3218, 40),
            (780, 29),
            (3842, 5),
            (752, 4),
            (748, 2),
            (764, 1),
            (3690, 1),
        ],
        &mod_holders,
    );
    c.eq(
        "§5.2 empty entries in effects",
        12_449,
        entry_kinds.get("Empty").copied().unwrap_or(0),
    );
    c.inv(
        "M1 effects: no entry with both or neither value source",
        0,
        entry_kinds.get("Conflicting").copied().unwrap_or(0)
            + entry_kinds.get("Valueless").copied().unwrap_or(0),
    );
    c.inv("§5.5 no ranged entry in effects", 0, ranged);
    c.eq(
        "§4.9 Mod_Progression fields in effects",
        17_918,
        mod_prog_fields,
    );
    c.map(
        "§4.9 non-zero Mod_Progression targets in effects",
        &[(1389, 3_444), (3918, 1_607), (1385, 417)],
        &mod_prog_targets,
    );
    c.eq(
        "§5.8 effects whose inline-entry count differs from compiled objects",
        1,
        inline_mismatch,
    );
    c.eq(
        "§5.8 effects whose progression-entry count differs",
        0,
        prog_mismatch,
    );
    c.map(
        "§4.9 vital-amount progression targets",
        &[
            (("initial", 1385), 142),
            (("initial", 3918), 1_538),
            (("instant", 1385), 333),
            (("instant", 3918), 479),
            (("per interval", 1385), 318),
            (("per interval", 3918), 2_085),
        ],
        &vital_refs,
    );
    let c725: BTreeMap<&str, u64> = combos_725.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    c.map(
        "§6.4 instant-vital sources (class 725)",
        &[
            ("progression", 529),
            ("progression+variance", 259),
            ("constant", 226),
            ("variable", 52),
            ("constant+progression", 19),
            ("constant+variable", 5),
            ("progression+variable", 4),
            ("constant+variance", 3),
            ("random", 1),
            ("progression+random", 1),
            ("none", 4),
        ],
        &c725,
    );
    let main_755: u64 = [
        "initial progression / per interval progression",
        "initial none / per interval progression",
        "initial constant / per interval constant",
        "initial progression / per interval constant+progression",
    ]
    .iter()
    .map(|k| combos_755.get(*k).copied().unwrap_or(0))
    .sum();
    for (k, expected) in [
        ("initial progression / per interval progression", 1_651),
        ("initial none / per interval progression", 716),
        ("initial constant / per interval constant", 32),
        (
            "initial progression / per interval constant+progression",
            29,
        ),
    ] {
        c.eq(
            format!("§6.4 vital over time (class 755): {k}"),
            expected,
            combos_755.get(k).copied().unwrap_or(0),
        );
    }
    c.eq(
        "§6.4 vital over time (class 755): others",
        10,
        combos_755.values().sum::<u64>() - main_755,
    );
    eprintln!("vital-over-time source combinations: {combos_755:?}");
    let c725nv: BTreeMap<&str, u64> = combos_725_nv
        .iter()
        .map(|(k, v)| (k.as_str(), *v))
        .collect();
    c.map(
        "§6.4 instant-vital sources without the variance (class 725, C57)",
        &[
            ("progression", 788),
            ("constant", 229),
            ("variable", 52),
            ("constant+progression", 19),
            ("constant+variable", 5),
            ("progression+variable", 4),
            ("none", 4),
            ("random", 1),
            ("progression+random", 1),
        ],
        &c725nv,
    );
    c.map(
        "§6.4 variance present (C57)",
        &[
            ("725 initial", 262),
            ("755 initial", 143),
            ("755 per interval", 281),
        ],
        &variances,
    );
    // Non-empty lists after zero padding is removed; a stored but all-zero list would
    // read as absent here.
    let stems: BTreeMap<&str, u64> = stem_lists.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    c.map(
        "§6.4 per-stem modifier lists and critical multipliers (C57)",
        &[
            ("instant modifier list", 526),
            ("instant additive list", 22),
            ("instant critical multiplier", 1_103),
            ("instant critical multiplier list", 304),
            ("initial modifier list", 1_117),
            ("initial additive list", 1),
            ("initial critical multiplier", 2_438),
            ("initial critical multiplier list", 185),
            ("per interval modifier list", 2_140),
            ("per interval additive list", 12),
            ("per interval critical multiplier", 2_438),
            ("per interval critical multiplier list", 357),
        ],
        &stems,
    );
    for ((label, cl), expected) in [
        (("Effect_Aura_Type", 709), 617),
        (("Effect_Aura_Radius", 709), 602),
        (("Effect_Aura_Applied_Effect_Array", 709), 617),
        (("Effect_Aura_Spellcraft_Override", 709), 596),
        (("Aura_ShouldAffectCaster", 709), 617),
        (("Effect_Area_MaxRange", 2762), 453),
        (("Effect_Area_MaxTargets", 2762), 452),
        (("Effect_Area_Applied_Effect_Array", 2762), 453),
        (("Effect_ApplyOverTime_Applied_Effect_Array", 708), 1_245),
        (
            ("Effect_ApplyOverTime_Initial_Applied_Effect_Array", 708),
            819,
        ),
        (("Effect_SkillProc_SkillTypes", 3686), 434),
        (("Effect_SkillProc_RequiredCombatResult", 3686), 317),
        (("Effect_SkillProc_ProcProbability", 3686), 259),
        (("Effect_MinTimeBetweenProcs", 3686), 269),
        (("Effect_Bubble_VitalType", 3222), 88),
        (("Effect_Bubble_Value", 3222), 42),
        (("Effect_Bubble_Value_Progression", 3222), 8),
        (("Effect_Bubble_Percentage", 3222), 38),
        (("Effect_Pip_Type", 731), 86),
        (("Effect_Pip_AdjustmentAmount", 731), 84),
        (("Effect_Pip_AdjustmentType", 731), 84),
        (("Effect_Pip_Reset", 731), 2),
    ] {
        c.eq(
            format!("§6.5 {label} in class {cl}"),
            expected,
            class_props.get(&(label, cl)).copied().unwrap_or(0),
        );
    }
    c.map(
        "§6.5 Effect_Aura_Type values",
        &[(1, 596), (2, 21)],
        &aura_types,
    );
    for (label, expected) in [
        ("aura applied", 734),
        ("area applied", 578),
        ("apply over time", 1_365),
        ("apply over time initial", 1_070),
    ] {
        c.eq(
            format!("§6.5 {label} structs"),
            expected,
            struct_elems.get(label).copied().unwrap_or(0),
        );
    }
    let r = |k: &str| refs.get(k).copied().unwrap_or(0);
    c.eq(
        "§6.3 EffectGenerator_EffectStruct in effects",
        6_604,
        r("Generator in EffectGenerator_EffectStruct"),
    );
    c.eq(
        "§6.3 EffectGenerator_EffectID in effects outside the struct (on removal)",
        740,
        refs.iter()
            .filter(|(k, _)| {
                k.starts_with("Generator in ") && !k.contains("EffectGenerator_EffectStruct")
            })
            .map(|(_, v)| v)
            .sum(),
    );
    c.eq(
        "§6.3 Skill_EffectData in effects, non-zero",
        779,
        r("SkillEffectData in Skill_EffectData"),
    );
    c.eq(
        "§6.3 Skill_EffectData in effects, placeholders",
        2_549,
        r("SkillEffectData in Skill_EffectData (placeholder)"),
    );
    c.eq(
        "§6.3 Effect_MonsterStartupEffect_Struct in effects",
        532,
        r("Startup in Effect_MonsterStartupEffect_Struct"),
    );
    c.eq(
        "§6.3 bare Effect_WSLEffect in effects (zero values included)",
        4_954,
        r("Bare in Effect_WSLEffect") + r("Bare in Effect_WSLEffect (placeholder)"),
    );
    c.eq(
        "§6.3 Effect_WSLEffect zero values (C56)",
        2_649,
        r("Bare in Effect_WSLEffect (placeholder)"),
    );
    // §6.5, D6: the applied-effect structs are effect references (C58).
    for (container, expected) in [
        ("Effect_Aura_Applied_Effect_Data", 734),
        ("Effect_Area_Applied_Effect_Data", 578),
        // 1,365 + 1,070 in class 708 and 20 in the class-3124 combination arrays.
        ("Effect_ApplyOverTime_Applied_Effect_Data", 2_455),
    ] {
        c.eq(
            format!("§6.5 applied-effect references in {container}"),
            expected,
            r(&format!("Applied in {container}")),
        );
    }
    eprintln!("effect references by kind and container: {refs:?}");

    // §6.6 effect control object.
    let control: Vec<Did> = inst
        .index
        .dids_of_class(inst.classes.effect_control)
        .to_vec();
    c.eq("§6.6 effect control objects", 1, control.len() as u64);
    if let Some(&did) = control.first() {
        let l = inst.load(did);
        let ctl = EffectControl::from_properties(&l.record.properties, pids);
        let count =
            |f: &dyn Fn(&tessera_formats::gameplay::combat::EquivalenceClassData) -> bool| {
                ctl.classes.iter().filter(|e| f(e)).count() as u64
            };
        c.eq(
            "§6.6 Effect_EqvClassData structs",
            728,
            ctl.classes.len() as u64,
        );
        c.eq(
            "§6.6 MaxDepth present",
            459,
            count(&|e| e.max_depth.is_some()),
        );
        c.eq(
            "§6.6 MaxExpressedEffs present",
            144,
            count(&|e| e.max_expressed.is_some()),
        );
        c.eq(
            "§6.6 MaxExpressedEffs = 1",
            102,
            count(&|e| e.max_expressed == Some(1)),
        );
        c.eq(
            "§6.6 IsPerCaster present",
            293,
            count(&|e| e.per_caster.is_some()),
        );
        c.eq(
            "§6.6 IsPerCaster true",
            210,
            count(&|e| e.per_caster == Some(true)),
        );
        c.eq(
            "§6.6 SorterOverride",
            43,
            count(&|e| e.sorter_override.is_some()),
        );
        c.eq(
            "§6.6 DurationModifier source / target",
            8,
            count(&|e| e.duration_modifier_source.is_some())
                + count(&|e| e.duration_modifier_target.is_some()),
        );
        let mut depth: BTreeMap<i64, u64> = BTreeMap::new();
        for e in &ctl.classes {
            if let Some(d) = e.max_depth {
                bump(&mut depth, d);
            }
        }
        let common: BTreeMap<i64, u64> = depth
            .iter()
            .filter(|&(k, _)| [24, 3, 2, 4, 12, 1, 5].contains(k))
            .map(|(&k, &v)| (k, v))
            .collect();
        c.map(
            "§6.6 MaxDepth most common values",
            &[
                (24, 280),
                (3, 55),
                (2, 36),
                (4, 29),
                (12, 14),
                (1, 11),
                (5, 10),
            ],
            &common,
        );
        c.inv(
            "§6.6 MaxDepth values lie in 1–201",
            depth
                .keys()
                .filter(|k| (1..=201).contains(*k))
                .map(|k| depth[k])
                .sum(),
            depth.values().sum(),
        );
    }
}

/// §7: every skill.
fn skills(inst: &Install, c: &mut Census) {
    let pids = &inst.pids;
    let s = &pids.skill;
    let h = &pids.hook;
    let dids = inst.dids_of_family(CombatFamily::Skill);
    let mut pid_counts: HashMap<u32, u64> = HashMap::new();
    let (mut s1, mut s2) = (0u64, 0u64);
    let mut volumes: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut compiled_volumes: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut presence: BTreeMap<&'static str, u64> = BTreeMap::new();
    let (mut hooks, mut hook_prog_nonzero, mut hook_lists) = (0u64, 0u64, 0u64);
    let mut hook_fields: BTreeMap<&'static str, u64> = BTreeMap::new();
    let (mut cost_entries, mut toggle_entries, mut vital_prog_fields, mut vital_prog_nonzero) =
        (0u64, 0u64, 0u64, 0u64);
    let mut refs: BTreeMap<String, u64> = BTreeMap::new();
    let mut targets: BTreeMap<(&'static str, u32), u64> = BTreeMap::new();
    let (mut ignores_true, mut immediate_true, mut toggle_hook_1) = (0u64, 0u64, 0u64);
    let props: [(&'static str, Pid); 21] = [
        ("Skill_VitalCostList", s.vital_cost_list),
        ("Skill_Toggle_VitalCostPerSecondList", s.toggle_cost_list),
        ("Skill_VitalMaxList", s.vital_max_list),
        ("Skill_AttackHookList", h.list),
        ("Skill_AEDetectionVolume_HeadingOffset", s.heading_offset),
        ("Skill_RecoveryChannel", s.recovery_channel),
        ("Skill_IgnoresResetTime", s.ignores_reset_time),
        ("Skill_Immediate", s.immediate),
        ("Skill_InductionAction", s.induction),
        ("Skill_Toggle_ChannelingState", s.channelling_state),
        (
            "Skill_Toggle_ChannelingDurationOverride",
            s.channelling_duration_override,
        ),
        ("Skill_Toggle_Hook_Number", s.toggle_hook_number),
        (
            "Skill_SkillRecoveryTime_Mod_Array",
            s.recovery_time_modifiers,
        ),
        ("Skill_RequiredTrait", s.required_trait),
        ("Skill_RequiredEffectList", s.required_effect_list),
        ("Skill_BarringEffectList", s.barring_effect_list),
        ("Skill_UserEffectList", s.user_effects),
        ("Skill_UserEffectList_Override", s.user_effects_override),
        ("Skill_CriticalEffectList", s.critical_effects),
        ("Skill_Toggle_Effect_List", s.toggle_effects),
        ("Skill_Toggle_User_Effect_List", s.toggle_user_effects),
    ];
    for &did in &dids {
        let l = inst.load(did);
        let set = &l.record.properties;
        for e in &set.entries {
            *pid_counts.entry(e.pid).or_default() += 1;
        }
        for (label, p) in props {
            *presence.entry(label).or_default() += present(set, p);
        }
        let skill = Skill::read(&l.record, pids).unwrap_or_else(|e| panic!("{did}: {e}"));
        s2 += u64::from(skill.detection_volumes.len() <= 1);
        for v in &skill.detection_volumes {
            let k = match v {
                tessera_formats::gameplay::combat::DetectionVolume::Sphere { .. } => "sphere",
                tessera_formats::gameplay::combat::DetectionVolume::Arc { .. } => "arc",
                tessera_formats::gameplay::combat::DetectionVolume::Box { .. } => "box",
            };
            *volumes.entry(k).or_default() += 1;
        }
        for (k, cl) in [
            ("sphere", CLASS_SPHERE),
            ("arc", CLASS_ARC),
            ("box", CLASS_BOX),
        ] {
            *compiled_volumes.entry(k).or_default() += u64::from(count_class(&l.state, cl) > 0);
        }
        hook_lists += present(set, h.list);
        hooks += skill.attack_hooks.len() as u64;
        s1 += u64::from(count_class(&l.state, CLASS_HOOK) == skill.attack_hooks.len() as u64);
        hook_prog_nonzero += skill
            .attack_hooks
            .iter()
            .filter(|k| k.damage_max_progression.is_some())
            .count() as u64;
        for hs in set.structs(h.list) {
            for (label, p) in [
                ("HookDamageMax", h.damage_max),
                ("HookDamageMax_Progression", h.damage_max_progression),
                ("DPSAddMod_Progression", h.dps_add_progression),
                ("SuperCriticalDamageMultiplier", h.super_critical_multiplier),
                ("AdditionalThreatValue", h.threat_value),
                ("UsesTactical", h.uses_tactical),
                ("DamageTypeOverride", h.damage_type_override),
            ] {
                *hook_fields.entry(label).or_default() += present(hs, p);
            }
        }
        cost_entries += set.structs(s.vital_cost_list).count() as u64;
        toggle_entries += set.structs(s.toggle_cost_list).count() as u64;
        for list in [s.vital_cost_list, s.toggle_cost_list, s.vital_max_list] {
            for vs in set.structs(list) {
                vital_prog_fields += present(vs, s.vital_points_progression);
                vital_prog_nonzero +=
                    u64::from(vs.did_nonnull(s.vital_points_progression).is_some());
            }
        }
        ignores_true += u64::from(skill.timing.ignores_reset_time == Some(true));
        immediate_true += u64::from(skill.timing.immediate == Some(true));
        toggle_hook_1 += u64::from(skill.timing.toggle_hook_number == Some(1));
        for (label, d) in [
            ("induction", skill.timing.induction),
            ("channelling", skill.timing.channelling_state),
            ("required trait", skill.required_trait),
        ] {
            if let Some(cl) = d.and_then(|d| inst.class_of(d)) {
                let k = match inst.classes.family(&*inst.library, cl) {
                    Some(CombatFamily::Trait) => 1479,
                    _ => cl,
                };
                *targets.entry((label, k)).or_default() += 1;
            }
        }
        for r in &skill.references {
            let container = inst.dict.name(r.container).unwrap_or("?");
            let key = format!(
                "{:?} in {container}{}",
                r.kind,
                if r.is_placeholder() {
                    " (placeholder)"
                } else {
                    ""
                }
            );
            bump(&mut refs, key);
        }
    }
    let n = dids.len() as u64;
    let in_all = pid_counts.values().filter(|&&v| v == n).count() as u64;
    let frequent = pid_counts.values().filter(|&&v| v > 5_000 && v < n).count() as u64;
    c.eq("§7.1 properties present in every skill", 56, in_all);
    c.eq(
        "§7.1 further properties in more than 5,000 skills",
        42,
        frequent,
    );
    c.inv("S1 one compiled hook object per attack hook", s1, n);
    c.inv("S2 at most one detection volume", s2, n);
    let vol_ref: BTreeMap<&str, u64> = volumes.clone();
    c.map(
        "§7.4 detection volumes",
        &[("sphere", 1_882), ("arc", 1_121), ("box", 132)],
        &vol_ref,
    );
    c.eq(
        "§3.2 skills with compiled sphere objects = sphere skills",
        volumes.get("sphere").copied().unwrap_or(0),
        compiled_volumes.get("sphere").copied().unwrap_or(0),
    );
    c.eq(
        "§3.2 skills with compiled arc objects = arc skills",
        volumes.get("arc").copied().unwrap_or(0),
        compiled_volumes.get("arc").copied().unwrap_or(0),
    );
    c.eq("§7.7 skills with Skill_AttackHookList", 13_246, hook_lists);
    c.eq("§7.7 attack hooks", 13_736, hooks);
    c.eq(
        "§4.9 non-zero hook damage progressions",
        6_365,
        hook_prog_nonzero,
    );
    for (label, expected) in [
        ("HookDamageMax", 13_736),
        ("HookDamageMax_Progression", 13_736),
        ("DPSAddMod_Progression", 4_840),
        ("SuperCriticalDamageMultiplier", 13_717),
        ("AdditionalThreatValue", 13_717),
        ("UsesTactical", 13_350),
        ("DamageTypeOverride", 13_111),
    ] {
        c.eq(
            format!("§7.7 hooks with {label}"),
            expected,
            hook_fields.get(label).copied().unwrap_or(0),
        );
    }
    let p = |k: &str| presence.get(k).copied().unwrap_or(0);
    for (label, expected) in [
        ("Skill_VitalCostList", 12_628),
        ("Skill_Toggle_VitalCostPerSecondList", 12_628),
        ("Skill_VitalMaxList", 4_645),
        ("Skill_AEDetectionVolume_HeadingOffset", 60),
        ("Skill_RecoveryChannel", 1_247),
        ("Skill_IgnoresResetTime", 4_645),
        ("Skill_Immediate", 4_645),
        ("Skill_InductionAction", 3_496),
        ("Skill_Toggle_ChannelingState", 171),
        ("Skill_Toggle_ChannelingDurationOverride", 45),
        ("Skill_Toggle_Hook_Number", 1_581),
        ("Skill_SkillRecoveryTime_Mod_Array", 1_124),
        ("Skill_RequiredTrait", 817),
        ("Skill_RequiredEffectList", 1_544),
        ("Skill_BarringEffectList", 3_641),
        ("Skill_UserEffectList", 2_739),
        ("Skill_UserEffectList_Override", 4_789),
        ("Skill_CriticalEffectList", 159),
        ("Skill_Toggle_Effect_List", 1_591),
        ("Skill_Toggle_User_Effect_List", 43),
    ] {
        c.eq(format!("§7 {label} present"), expected, p(label));
    }
    c.eq(
        "§7.3 Skill_VitalInfo entries in cost lists",
        26_123,
        cost_entries,
    );
    eprintln!("Skill_VitalInfo entries in toggle cost lists: {toggle_entries}");
    c.eq(
        "§4.9 Skill_Vital_Points_Progression values",
        52_248,
        vital_prog_fields,
    );
    c.eq(
        "§4.9 Skill_Vital_Points_Progression non-zero",
        4_513,
        vital_prog_nonzero,
    );
    c.eq("§7.9 Skill_IgnoresResetTime true", 1_995, ignores_true);
    c.eq("§7.9 Skill_Immediate true", 146, immediate_true);
    c.inv(
        "§7.9 Skill_Toggle_Hook_Number is 1",
        toggle_hook_1,
        p("Skill_Toggle_Hook_Number"),
    );
    c.map(
        "§7.9 timing and requirement targets",
        &[
            (("channelling", inst.classes.channelling), 171),
            (("induction", inst.classes.induction), 3_496),
            (("required trait", 1479), 817),
        ],
        &targets,
    );
    let r = |k: &str| refs.get(k).copied().unwrap_or(0);
    c.eq(
        "§7.8 Skill_EffectData in skills, non-zero",
        15_187,
        r("SkillEffectData in Skill_EffectData"),
    );
    c.eq(
        "§7.8 Skill_EffectData in skills, placeholders",
        97_304,
        r("SkillEffectData in Skill_EffectData (placeholder)"),
    );
    c.eq(
        "§7.8 bare Skill_Effect in skill lists, non-zero",
        7_382,
        r("Bare in Skill_Effect"),
    );
    c.eq(
        "§6.3 Skill_Toggle_Effect_Data in skills",
        1_865,
        refs.iter()
            .filter(|(k, _)| k.starts_with("Toggle in Skill_Toggle_Effect_Data"))
            .map(|(_, v)| v)
            .sum(),
    );
    eprintln!("skill effect references by kind and container: {refs:?}");
}

/// §8: every trait, trait trees, sparse DID progressions, the trait control object.
fn traits(inst: &Install, c: &mut Census) {
    let pids = &inst.pids;
    let t = &pids.traits;
    let m = &pids.modifier;
    let dids = inst.dids_of_family(CombatFamily::Trait);
    let mut presence: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut overlay_nonzero = 0u64;
    let mut mod_targets: BTreeMap<u32, u64> = BTreeMap::new();
    let mut nested_in: BTreeMap<u32, u64> = BTreeMap::new();
    let mut point_cost: BTreeMap<u32, u64> = BTreeMap::new();
    let mut overlay: BTreeMap<u32, u64> = BTreeMap::new();
    let mut refs: BTreeMap<String, u64> = BTreeMap::new();
    let (mut m1_bad, mut ranged) = (0u64, 0u64);
    let (mut inline_mismatch, mut prog_mismatch) = (0u64, 0u64);
    let props: [(&'static str, Pid); 14] = [
        ("Trait_Name", t.name),
        ("Trait_Description", t.description),
        ("Trait_Tooltip", t.tooltip),
        ("Trait_Nature", t.nature),
        ("Trait_Category", t.category),
        ("Trait_Tier", t.tier),
        ("Trait_Cost_To_Slot", t.cost_to_slot),
        ("Trait_Icon", t.icon),
        ("Trait_Minimum_Level", t.minimum_level),
        ("Mod_Array", m.array),
        ("Trait_Virtue_Maximum_Rank", t.maximum_rank),
        ("Trait_Virtue_Rank_PropertyName", t.rank_property),
        ("Trait_Rank_Overlay_Progression", t.rank_overlay_progression),
        ("EffectGenerator_TraitEffectList", t.effect_list),
    ];
    let mut granted_skills = 0u64;
    for &did in &dids {
        let l = inst.load(did);
        let set = &l.record.properties;
        for (label, p) in props {
            *presence.entry(label).or_default() += present(set, p);
        }
        let tr = Trait::read(&l.record, pids).unwrap_or_else(|e| panic!("{did}: {e}"));
        granted_skills += tr.skills.len() as u64;
        overlay_nonzero += u64::from(tr.rank_overlay_progression.is_some());
        if let Some(cl) = tr.rank_overlay_progression.and_then(|d| inst.class_of(d)) {
            bump(&mut overlay, cl);
        }
        if let Some(cl) = tr.point_cost_progression.and_then(|d| inst.class_of(d)) {
            bump(&mut point_cost, cl);
        }
        for en in &tr.modifiers {
            m1_bad += u64::from(matches!(
                en.kind,
                ModEntryKind::Conflicting | ModEntryKind::Valueless
            ));
            ranged += u64::from(en.range.is_ranged());
            if let Some(cl) = en.progression.and_then(|d| inst.class_of(d)) {
                bump(&mut mod_targets, cl);
                if cl == inst.classes.progression_of_progressions {
                    bump(&mut nested_in, tr.class);
                }
            }
        }
        let inline = tr
            .modifiers
            .iter()
            .filter(|e| e.kind == ModEntryKind::Inline)
            .count() as u64;
        let driven = tr
            .modifiers
            .iter()
            .filter(|e| e.kind == ModEntryKind::ProgressionDriven)
            .count() as u64;
        inline_mismatch += u64::from(count_class(&l.state, CLASS_INLINE_MOD) != inline);
        prog_mismatch += u64::from(count_class(&l.state, CLASS_PROGRESSION_MOD) != driven);
        for r in &tr.references {
            let container = inst.dict.name(r.container).unwrap_or("?");
            bump(&mut refs, format!("{:?} in {container}", r.kind));
        }
    }
    let n = dids.len() as u64;
    // §8.1 (C58): `Trait_Skill_Array` holds bare `Trait_Skill` DIDs.
    c.eq("§8.1 Trait_Skill_Array DIDs", 135, granted_skills);
    let p = |k: &str| presence.get(k).copied().unwrap_or(0);
    for label in [
        "Trait_Name",
        "Trait_Description",
        "Trait_Tooltip",
        "Trait_Nature",
        "Trait_Category",
        "Trait_Tier",
        "Trait_Cost_To_Slot",
    ] {
        c.eq(format!("§8.2 {label} in every trait"), n, p(label));
    }
    for (label, expected) in [
        ("Trait_Icon", 3_952),
        ("Trait_Minimum_Level", 3_198),
        ("Mod_Array", 2_262),
        ("Trait_Virtue_Maximum_Rank", 1_273),
        ("Trait_Virtue_Rank_PropertyName", 1_271),
        ("Trait_Rank_Overlay_Progression", 1_260),
        ("EffectGenerator_TraitEffectList", 555),
    ] {
        c.eq(format!("§8 {label} present"), expected, p(label));
    }
    c.eq(
        "§4.9 Trait_Rank_Overlay_Progression non-zero",
        1_089,
        overlay_nonzero,
    );
    c.map("§4.9 rank overlay targets", &[(1389, 1_089)], &overlay);
    c.map(
        "§4.9 point-cost progression targets",
        &[(1385, 946)],
        &point_cost,
    );
    c.map(
        "§4.9 trait Mod_Progression targets",
        &[(1389, 776), (3918, 399), (3810, 50), (1385, 14)],
        &mod_targets,
    );
    c.map(
        "P7 progressions of progressions in traits, by class",
        &[(3438, 50)],
        &nested_in,
    );
    c.inv("M1 traits: one value source per entry", 0, m1_bad);
    c.inv("§5.5 no ranged entry in traits", 0, ranged);
    c.eq(
        "§5.8 traits whose inline-entry count differs",
        0,
        inline_mismatch,
    );
    c.eq(
        "§5.8 traits whose progression-entry count differs",
        0,
        prog_mismatch,
    );
    let r = |k: &str| refs.get(k).copied().unwrap_or(0);
    c.eq(
        "§6.3 EffectGenerator_EffectStruct in traits",
        697,
        r("Generator in EffectGenerator_EffectStruct"),
    );
    c.eq(
        "§6.3 Effect_MonsterStartupEffect_Struct in traits",
        9,
        r("Startup in Effect_MonsterStartupEffect_Struct"),
    );

    // §8.5 trait trees.
    let trees: Vec<Did> = inst.index.dids_of_class(inst.classes.trait_tree).to_vec();
    c.eq("§8.5 trait trees", 17, trees.len() as u64);
    let (mut entries, mut t1, mut t2, mut branch, mut deps) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut spec_trees, mut specs, mut spec_targets) = (0u64, 0u64, 0u64);
    let mut cells = BTreeSet::new();
    for &did in &trees {
        let l = inst.load(did);
        c.inv(format!("G1 trait tree {did}"), 0, inst.dangling(&l.record));
        let tree = TraitTree::from_properties(&l.record.properties, t);
        for e in &tree.entries {
            entries += 1;
            t1 += u64::from(inst.class_of(e.trait_did) == Some(inst.classes.point_based_trait));
            t2 += u64::from(e.cell().is_some_and(|k| k.has_observed_form()));
            if let Some(k) = e.location {
                cells.insert(k);
            }
            branch += u64::from(e.branch.is_some());
            deps += e.dependencies.len() as u64;
        }
        spec_trees += u64::from(!tree.specializations.is_empty());
        for &(_, d) in &tree.specializations {
            specs += 1;
            spec_targets +=
                u64::from(inst.class_of(d) == Some(inst.classes.sparse_did_progression));
        }
    }
    c.eq("§8.5 trait-tree entries", 1_216, entries);
    c.inv("T1 tree entries name point-based traits", t1, entries);
    c.inv("T2 cell keys have the observed form", t2, entries);
    c.eq("§8.5 entries with a branch", 1_201, branch);
    c.eq("§8.5 dependencies", 511, deps);
    c.eq("§8.5 trees with specialisations", 13, spec_trees);
    c.eq("§8.5 specialisations", 39, specs);
    c.inv(
        "§8.5 specialisations name sparse DID progressions",
        spec_targets,
        specs,
    );
    eprintln!("distinct trait-tree cell keys used: {}", cells.len());

    // §4.6 sparse DID progressions.
    let sparse: Vec<Did> = inst
        .index
        .dids_of_class(inst.classes.sparse_did_progression)
        .to_vec();
    c.eq("§4.6 sparse DID progressions", 39, sparse.len() as u64);
    let (mut pairs, mut to_traits, mut keys_ok) = (0u64, 0u64, 0u64);
    let mut sparse_targets: BTreeMap<Option<u32>, u64> = BTreeMap::new();
    for &did in &sparse {
        let l = inst.load(did);
        let sp = SparseDidProgression::from_properties(&l.record.properties, &pids.progression);
        for &(k, d) in &sp.entries {
            pairs += 1;
            to_traits += u64::from(inst.family_of(d) == Some(CombatFamily::Trait));
            bump(&mut sparse_targets, inst.class_of(d));
            keys_ok += u64::from((5.0..=120.0).contains(&k));
        }
    }
    c.eq("§4.6 sparse entries", 270, pairs);
    c.inv("§4.6 sparse entries name traits", to_traits, pairs);
    // C54: the 270 targets are traits of two classes (1477 and 1483 here), none
    // point-based.
    c.eq(
        "§4.6 sparse entries naming point-based traits",
        0,
        sparse_targets
            .get(&Some(inst.classes.point_based_trait))
            .copied()
            .unwrap_or(0),
    );
    c.eq(
        "§4.6 trait classes named by sparse entries",
        2,
        sparse_targets.keys().filter(|k| k.is_some()).count() as u64,
    );
    eprintln!("sparse DID progression targets by class: {sparse_targets:?}");
    c.inv("§4.6 sparse keys lie in 5–120", keys_ok, pairs);

    // §8.6 trait control object.
    let ctl: Vec<Did> = inst
        .index
        .dids_of_class(inst.classes.trait_control)
        .to_vec();
    c.eq("§8.6 trait control objects", 1, ctl.len() as u64);
    if let Some(&did) = ctl.first() {
        let l = inst.load(did);
        let tc = TraitControl::from_properties(&l.record.properties, t);
        c.eq("§8.6 progression DIDs", 6, tc.progressions.len() as u64);
        c.inv(
            "§8.6 progression DIDs name progressions",
            tc.progressions
                .iter()
                .filter(|(_, d)| inst.family_of(*d) == Some(CombatFamily::Progression))
                .count() as u64,
            tc.progressions.len() as u64,
        );
        for (label, list, expected) in [
            ("player", &tc.player_traits, 22),
            ("monster", &tc.monster_traits, 14),
            ("skirmish", &tc.skirmish_traits, 236),
        ] {
            c.eq(
                format!("§8.6 {label} trait list"),
                expected,
                list.len() as u64,
            );
            c.inv(
                format!("§8.6 {label} trait list names traits"),
                list.iter()
                    .filter(|d| inst.family_of(**d) == Some(CombatFamily::Trait))
                    .count() as u64,
                list.len() as u64,
            );
        }
        c.eq(
            "§8.6 virtue experience caps present",
            2,
            u64::from(tc.virtue_xp_soft_cap.is_some()) + u64::from(tc.virtue_xp_hard_cap.is_some()),
        );
    }
}

/// §9 and §10.3, §10.4: class tables, monster play, lines, thresholds, battle statistics.
fn tables(inst: &Install, c: &mut Census) {
    let pids = &inst.pids;
    let ct = &pids.class_table;
    let x = &pids.tables;

    let (mut levels_ok, mut formulas_ok, mut terms_ok) = (0u64, 0u64, 0u64);
    let (mut skill_entries, mut mount_entries, mut characteristics) = (0u64, 0u64, 0u64);
    let (mut typed_entries, mut complete_entries) = (0u64, 0u64);
    let tables: Vec<Did> = inst.index.dids_of_class(inst.classes.class_table).to_vec();
    for &did in &tables {
        let l = inst.load(did);
        let set = &l.record.properties;
        let t = ClassTable::from_properties(set, ct);
        levels_ok += u64::from(
            t.levels.len() == 160
                && t.levels
                    .iter()
                    .all(|lv| lv.base_vitals.len() == 4 && lv.max_stats.len() == 7),
        );
        formulas_ok += u64::from(t.derived_stats.len() == 37);
        terms_ok +=
            u64::from(t.derived_stats.iter().all(|f| {
                f.terms.len() == 7 && f.terms.iter().all(|&(_, v)| (0.0..=4.5).contains(&v))
            }));
        skill_entries += set.elements(ct.available_skills).len() as u64;
        mount_entries += set.elements(ct.available_mount_skills).len() as u64;
        for e in t.skill_entries.iter().chain(&t.mount_skill_entries) {
            typed_entries += 1;
            complete_entries += u64::from(
                e.skill.is_some()
                    && e.level.is_some()
                    && e.prerequisite.is_some()
                    && e.rank.is_some()
                    && e.training_cost.is_some()
                    && e.shortcut_index.is_some(),
            );
        }
        characteristics += t.characteristic_traits.len() as u64;
    }
    let n = tables.len() as u64;
    c.inv("§9.1 160 levels with 4 vitals and 7 stats", levels_ok, n);
    c.inv("§9.1 37 derived-stat formulas", formulas_ok, n);
    c.inv("§9.1 7 terms per formula, values 0–4.5", terms_ok, n);
    c.eq("§9.1 available skill entries", 551, skill_entries);
    c.eq("§9.1 available mount skill entries", 36, mount_entries);
    c.inv(
        "§9.1 skill entries with all six fields (C58)",
        complete_entries,
        typed_entries,
    );
    c.eq("§9.1 typed skill entries", 587, typed_entries);
    c.eq(
        "§9.1 class characteristics naming a trait",
        297,
        characteristics,
    );

    // §9.2 monster play, §9.3 lines.
    let virtue_list = inst.pid("MonsterPlay_VirtueList");
    let (mut levels_ok, mut skills, mut traits, mut virtues) = (0u64, 0u64, 0u64, 0u64);
    let (mut mp_typed, mut mp_complete) = (0u64, 0u64);
    let mp: Vec<Did> = inst.index.dids_of_class(inst.classes.monster_play).to_vec();
    for &did in &mp {
        let l = inst.load(did);
        let set = &l.record.properties;
        let t = MonsterPlayTable::from_properties(set, ct);
        levels_ok += u64::from(t.level.is_some_and(|v| (1..=160).contains(&v)));
        skills += set.elements(ct.monster_skill_list).len() as u64;
        traits += set.elements(ct.monster_trait_list).len() as u64;
        virtues += set.elements(virtue_list).len() as u64;
        mp_typed += (t.skill_entries.len() + t.trait_entries.len() + t.virtue_entries.len()) as u64;
        mp_complete += t
            .skill_entries
            .iter()
            .filter(|e| {
                e.skill.is_some()
                    && e.cost.is_some()
                    && e.min_level.is_some()
                    && e.min_rank.is_some()
            })
            .count() as u64
            + t.trait_entries
                .iter()
                .filter(|e| {
                    e.trait_did.is_some()
                        && e.min_level.is_some()
                        && e.min_rank.is_some()
                        && e.trainable.is_some()
                })
                .count() as u64
            + t.virtue_entries
                .iter()
                .filter(|e| e.trait_did.is_some() && e.start_rank.is_some() && e.max_rank.is_some())
                .count() as u64;
    }
    c.inv("§9.2 monster level 1–160", levels_ok, mp.len() as u64);
    c.eq("§9.2 monster-play skill entries", 525, skills);
    c.eq("§9.2 monster-play trait entries", 687, traits);
    c.eq("§9.2 monster-play virtue entries", 84, virtues);
    c.inv(
        "§9.2 monster-play entries with every field (C58)",
        mp_complete,
        mp_typed,
    );
    for (class, expected, family) in [
        (2390u32, 120u64, CombatFamily::Skill),
        (2399, 154, CombatFamily::Trait),
    ] {
        let (mut entries, mut named) = (0u64, 0u64);
        for &did in inst.index.dids_of_class(class) {
            let l = inst.load(did);
            let line = AdvancementLine::from_properties(&l.record.properties, ct);
            entries += line.entries.len() as u64;
            named += line
                .entries
                .iter()
                .filter(|d| inst.family_of(**d) == Some(family))
                .count() as u64;
        }
        c.eq(
            format!("§9.3 line entries (class {class})"),
            expected,
            entries,
        );
        c.inv(
            format!("§9.3 line entries (class {class}) name {family:?}"),
            named,
            entries,
        );
    }

    // §10.3 thresholds.
    let thresholds = inst.dids_of_family(CombatFamily::ThresholdPolicy);
    let (mut states_ok, mut bands_ok, mut effects_ok) = (0u64, 0u64, 0u64);
    let mut index_props = BTreeSet::new();
    for &did in &thresholds {
        let l = inst.load(did);
        let t = ThresholdPolicy::from_properties(&l.record.properties, x);
        states_ok += u64::from((2..=10).contains(&t.states.len()));
        bands_ok += u64::from(t.states.iter().all(|s| {
            [s.min, s.max]
                .iter()
                .flatten()
                .all(|v| (0.1..=0.9).contains(&((v * 1000.0).round() / 1000.0)))
        }));
        effects_ok += u64::from(t.states.iter().all(|s| {
            s.effects
                .iter()
                .all(|d| inst.family_of(*d) == Some(CombatFamily::Effect))
        }));
        if let Some(p) = t.index_property {
            index_props.insert(p);
        }
    }
    let n = thresholds.len() as u64;
    c.inv("§10.3 2–10 states per policy", states_ok, n);
    c.inv("§10.3 bands within 0.1–0.9", bands_ok, n);
    c.inv("§10.3 state effect lists name effects", effects_ok, n);
    c.eq(
        "§10.3 distinct index properties",
        1,
        index_props.len() as u64,
    );

    // §10.4 battle statistics.
    let stats = inst.dids_of_family(CombatFamily::BattleStatistic);
    let mut scope: BTreeMap<u32, u64> = BTreeMap::new();
    let mut policy: BTreeMap<u32, u64> = BTreeMap::new();
    let (mut replicate, mut forward, mut forward_ok) = (0u64, 0u64, 0u64);
    for &did in &stats {
        let l = inst.load(did);
        let b = BattleStatistic::from_properties(&l.record.properties, x);
        if let Some(s) = b.scope {
            bump(&mut scope, s);
        }
        if let Some(p) = b.update_policy {
            bump(&mut policy, p);
        }
        replicate += u64::from(b.replicate_to_clients == Some(true));
        if let Some(d) = b.forward_to {
            forward += 1;
            forward_ok += u64::from(inst.family_of(d) == Some(CombatFamily::BattleStatistic));
        }
    }
    c.map("§10.4 BattleStat_Scope", &[(3, 13)], &scope);
    c.inv(
        "§10.4 BattleStat_UpdatePolicy is 1, 2 or 4",
        policy
            .iter()
            .filter(|(k, _)| [1, 2, 4].contains(*k))
            .map(|(_, v)| v)
            .sum(),
        stats.len() as u64,
    );
    c.eq("§10.4 BattleStat_ReplicateToClients true", 9, replicate);
    c.eq("§10.4 BattleStat_ForwardUpdatesToStat", 4, forward);
    c.inv("§10.4 forwards name battle statistics", forward_ok, forward);
}

/// Related objects of §2.1: combat control, DPS and value tables, induction and channelling.
fn related(inst: &Install, c: &mut Census) {
    let pids = &inst.pids;
    let x = &pids.tables;

    let control: Vec<Did> = inst
        .index
        .dids_of_class(inst.classes.combat_control)
        .to_vec();
    c.eq("§10.1 combat control objects", 1, control.len() as u64);
    if let Some(&did) = control.first() {
        let l = inst.load(did);
        let ctl = CombatControl::from_properties(&l.record.properties, x);
        c.eq(
            "§10.1 calculation entries",
            30,
            ctl.calc_controls.len() as u64,
        );
        let keys: BTreeSet<u32> = ctl.calc_controls.iter().map(|k| k.calc_type).collect();
        c.inv(
            "§10.1 calculation keys are 1–30, once each",
            keys.iter().filter(|k| (1..=30).contains(*k)).count() as u64,
            30,
        );
        let floats = ctl
            .calc_controls
            .iter()
            .flat_map(|k| [k.rating, k.target_cap, k.hard_cap])
            .filter(|d| d.and_then(|d| inst.class_of(d)) == Some(1385))
            .count() as u64;
        c.eq("§4.9 combat control float progressions", 90, floats);
    }

    for (class, records, length, label) in [
        (inst.classes.dps_table, 8u64, 701usize, "DPS"),
        (inst.classes.value_table, 99, 601, "value"),
    ] {
        let dids: Vec<Did> = inst.index.dids_of_class(class).to_vec();
        c.eq(format!("§10.2 {label} tables"), records, dids.len() as u64);
        let (mut len_ok, mut order_ok, mut two_props) = (0u64, 0u64, 0u64);
        for &did in &dids {
            let l = inst.load(did);
            let set = &l.record.properties;
            let t = if class == inst.classes.dps_table {
                QualityTable::dps_from_properties(set, x)
            } else {
                QualityTable::value_from_properties(set, x)
            };
            len_ok += u64::from(t.base.len() == length);
            let order: Vec<u32> = t.qualities.iter().map(|&(k, _)| k).collect();
            order_ok += u64::from(order == [5, 4, 2, 3, 1]);
            two_props += u64::from(set.len() == 2);
        }
        let n = dids.len() as u64;
        c.inv(
            format!("§10.2 {label} tables hold {length} base values"),
            len_ok,
            n,
        );
        c.inv(
            format!("§10.2 {label} qualities stored as 5, 4, 2, 3, 1"),
            order_ok,
            n,
        );
        c.inv(
            format!("§10.2 {label} tables hold exactly two properties"),
            two_props,
            n,
        );
    }

    let t = &pids.timing;
    let induction: Vec<Did> = inst.index.dids_of_class(inst.classes.induction).to_vec();
    c.eq("§7.9 induction objects", 1_779, induction.len() as u64);
    let (mut ok, mut add, mut mult) = (0u64, 0u64, 0u64);
    for &did in &induction {
        let l = inst.load(did);
        let set = &l.record.properties;
        let i = Induction::from_properties(set, t).unwrap_or_else(|e| panic!("{did}: {e}"));
        ok += u64::from(
            (0.1..=60.0).contains(&i.duration)
                && i.interruptible.is_some()
                && i.allowed_while_moving.is_some()
                && i.allowed_in_combat.is_some(),
        );
        add += present(set, t.induction_additive_modifiers);
        mult += present(set, t.induction_multiplier_modifiers);
    }
    c.inv(
        "§7.9 induction duration 0.1–60 and three flags",
        ok,
        induction.len() as u64,
    );
    c.eq("§7.9 induction additive modifier lists", 54, add);
    c.eq("§7.9 induction multiplier modifier lists", 509, mult);
    let channelling: Vec<Did> = inst.index.dids_of_class(inst.classes.channelling).to_vec();
    c.eq("§7.9 channelling objects", 401, channelling.len() as u64);
    let mut ok = 0u64;
    for &did in &channelling {
        let l = inst.load(did);
        let set = &l.record.properties;
        let ch = Channelling::from_properties(set, t).unwrap_or_else(|e| panic!("{did}: {e}"));
        ok += u64::from(
            (1.0..=99_999.0).contains(&ch.duration) && set.has(t.channelling_multiplier_property),
        );
    }
    c.inv(
        "§7.9 channelling duration 1–99,999 and multiplier property",
        ok,
        channelling.len() as u64,
    );
}

/// §6.7, §10.4: the two DID maps.
fn maps(inst: &Install, c: &mut Census) {
    let data = inst.store.game_data();
    if let Some(map) = data
        .did_map_by_label(COMBAT_EFFECTS_MAP_LABEL)
        .expect("DID map")
    {
        c.eq("§6.7 COMBATEFFECTS entries", 27, map.len() as u64);
        let mut targets: BTreeMap<u32, u64> = BTreeMap::new();
        for e in map.iter() {
            bump(
                &mut targets,
                if e.value.is_null() {
                    0
                } else {
                    inst.class_of(e.value).unwrap_or(u32::MAX)
                },
            );
        }
        c.map(
            "§6.7 COMBATEFFECTS targets",
            &[(0, 1), (734, 10), (718, 16)],
            &targets,
        );
        c.eq("§6.7 COMBATEFFECTS groups", 2, map.groups().len() as u64);
    } else {
        c.eq("§6.7 COMBATEFFECTS map present", 1, 0);
    }
    if let Some(map) = data
        .did_map_by_label(BATTLE_STATS_MAP_LABEL)
        .expect("DID map")
    {
        c.eq("§10.4 BATTLESTATS entries", 14, map.len() as u64);
        let named = map
            .iter()
            .filter(|e| inst.family_of(e.value) == Some(CombatFamily::BattleStatistic))
            .count() as u64;
        c.eq(
            "§10.4 BATTLESTATS entries naming battle statistics",
            13,
            named,
        );
        c.eq("§10.4 BATTLESTATS groups", 2, map.groups().len() as u64);
    } else {
        c.eq("§10.4 BATTLESTATS map present", 1, 0);
    }
}

// ---------------------------------------------------------------------------------------------
// §5 over the whole install
// ---------------------------------------------------------------------------------------------

#[test]
fn modifier_entries_whole_install() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    if !env_flag("TESSERA_FULL") {
        eprintln!("TESSERA_FULL not set; skipping the whole-install modifier census");
        return;
    }
    let started = Instant::now();
    let inst = Install::open(&dir);
    let mut c = Census::default();
    let m = &inst.pids.modifier;
    let data = inst.store.game_data();
    let mut arrays_by: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
    let mut ops: BTreeMap<(u32, &'static str), u64> = BTreeMap::new();
    let (mut entries, mut m2, mut m2_total, mut unrelated) = (0u64, 0u64, 0u64, 0u64);
    let (mut m4_bad, mut list_entries) = (0u64, 0u64);
    let mut list_ops: BTreeMap<u32, u64> = BTreeMap::new();
    let mut list_lens: BTreeMap<usize, u64> = BTreeMap::new();
    let (mut groups, mut ranged_entries, mut disjoint, mut not_ascending, mut not_contiguous) =
        (0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut mixed, mut combat_ranged) = (0u64, 0u64);
    let mut sizes: BTreeMap<usize, u64> = BTreeMap::new();
    let mut targets: BTreeMap<u32, u64> = BTreeMap::new();
    let mut nested_holders: BTreeMap<u32, u64> = BTreeMap::new();
    let (mut both_ends, mut nested_arrays) = (0u64, 0u64);
    let op_mapper = data
        .registry()
        .ok()
        .and_then(|_| tessera_formats::gameplay::common::enum_mapper_of(&inst.dict, m.op))
        .and_then(|d| data.enum_mapper(d).ok().flatten());
    // Every 0x79 set (by its object's class), then every stand-alone 0x78 set.
    let mut sources: Vec<(Did, Option<u32>)> = Vec::new();
    for (class, _) in inst.index.classes() {
        for &did in inst.index.dids_of_class(class) {
            sources.push((did, Some(class)));
        }
    }
    let mut standalone: BTreeSet<u32> = BTreeSet::new();
    for member in data.source().members() {
        for entry in member.archive.entries_range(0x7800_0000..0x7900_0000) {
            standalone.insert(entry.expect("directory entry").did);
        }
    }
    let standalone_n = standalone.len() as u64;
    sources.extend(standalone.into_iter().map(|d| (Did(d), None)));
    for (did, class) in sources {
        let family = class.and_then(|cl| inst.classes.family(&*inst.library, cl));
        let holder: &'static str = match (class, family) {
            (None, _) => "0x78 sets",
            (_, Some(CombatFamily::Effect)) => "effects",
            (_, Some(CombatFamily::Trait)) => "traits",
            (_, Some(_)) => "other combat",
            (_, None) => "non-combat",
        };
        let set = match class {
            Some(_) => {
                let Some(set_did) = tessera_formats::property_set::object_property_set_did(did)
                else {
                    continue;
                };
                let Some(payload) = data.read(set_did).expect("read") else {
                    continue;
                };
                tessera_formats::gameplay::common::decode_object_properties(
                    did, &payload, &inst.dict,
                )
                .unwrap_or_else(|e| panic!("{did}: {e}"))
            }
            None => {
                let Some(payload) = data.read(did).expect("read") else {
                    continue;
                };
                tessera_formats::PropertySetRecord::parse_expecting(&payload, did, &inst.dict)
                    .unwrap_or_else(|e| panic!("{did}: {e}"))
                    .set
            }
        };
        {
            for array in mod_arrays(&set, m) {
                *arrays_by.entry(holder).or_default() += 1;
                nested_arrays += u64::from(array.depth > 0);
                let list = array.entries;
                for en in &list {
                    entries += 1;
                    bump(&mut kinds, format!("{holder}: {:?}", en.kind));
                    unrelated += u64::from(!en.unrelated_fields.is_empty());
                    if en.is_skipped() {
                        continue;
                    }
                    if let Some(v) = &en.value {
                        m2_total += 1;
                        m2 += u64::from(
                            inst.dict.definition(en.modified).map(|d| d.ty)
                                == Some(v.property_type()),
                        );
                    }
                    let op = en.operator(op_mapper.as_deref());
                    let src = if en.value.is_some() {
                        "inline"
                    } else {
                        "progression"
                    };
                    if let Some(op) = op {
                        *ops.entry((op.observed_key(), src)).or_default() += 1;
                        if let Some(v) = &en.value {
                            use PropertyValue as V;
                            let ok = match op {
                                ModOp::Or => matches!(
                                    v,
                                    V::Bool(_)
                                        | V::Bitfield32(_)
                                        | V::Bitfield64(_)
                                        | V::BitfieldAny(_)
                                ),
                                ModOp::Subtract | ModOp::Add | ModOp::Multiply => {
                                    matches!(v, V::Int(_) | V::Float(_))
                                }
                                ModOp::ArrayAdd => matches!(v, V::Array(_)),
                                _ => true,
                            };
                            m4_bad += u64::from(!ok);
                        }
                    }
                    if !en.modifier_list.is_empty() {
                        list_entries += 1;
                        bump(&mut list_lens, en.modifier_list.len());
                        if let Some(k) = en.modifier_op {
                            bump(&mut list_ops, k);
                        }
                    }
                    if let Some(t) = en.progression.and_then(|d| inst.class_of(d)) {
                        bump(&mut targets, t);
                        if t == inst.classes.progression_of_progressions {
                            bump(&mut nested_holders, class.unwrap_or(0));
                        }
                    }
                    if en.range.is_ranged() {
                        ranged_entries += 1;
                        both_ends +=
                            u64::from(en.range.floor.is_some() && en.range.ceiling.is_some());
                        combat_ranged += u64::from(family.is_some());
                    }
                }
                for g in ranged_groups(&list) {
                    groups += 1;
                    bump(&mut sizes, g.members.len());
                    disjoint += u64::from(g.is_disjoint());
                    not_ascending += u64::from(!g.stored_ascending);
                    not_contiguous += u64::from(!g.is_contiguous());
                    mixed += u64::from(g.unranged > 0);
                }
            }
        }
    }
    eprintln!("stand-alone 0x78 sets: {standalone_n}; nested Mod_Array values: {nested_arrays}");
    let total_arrays: u64 = arrays_by.values().sum();
    c.eq("§5.2 Mod_Array values", 116_435, total_arrays);
    c.eq(
        "§5.2 Mod_Array values in effects",
        16_947,
        arrays_by.get("effects").copied().unwrap_or(0),
    );
    c.eq(
        "§5.2 Mod_Array values in traits",
        2_262,
        arrays_by.get("traits").copied().unwrap_or(0),
    );
    c.eq("§5.2 entries", 372_834, entries);
    let k = |s: &str| -> u64 {
        kinds
            .iter()
            .filter(|(key, _)| key.ends_with(s))
            .map(|(_, v)| v)
            .sum()
    };
    c.eq("§5.2 empty entries", 12_449, k(": Empty"));
    c.eq(
        "§5.2 empty entries in effects",
        12_449,
        kinds.get("effects: Empty").copied().unwrap_or(0),
    );
    c.eq("§5.2 field-less entries", 947, k(": FieldLess"));
    c.eq(
        "§5.2 field-less entries in non-combat objects",
        947,
        kinds.get("non-combat: FieldLess").copied().unwrap_or(0),
    );
    c.eq("§5.2 inline entries", 60_295, k(": Inline"));
    c.eq(
        "§5.2 progression-driven entries",
        299_143,
        k(": ProgressionDriven"),
    );
    c.inv(
        "M1 one value source per non-empty entry",
        0,
        k(": Conflicting") + k(": Valueless"),
    );
    c.eq("§5.2 entries with an unrelated field", 12, unrelated);
    c.inv("M2 inline value type = declared type", m2, m2_total);
    c.inv("M4 operator / value-type co-occurrence", 0, m4_bad);
    let ops_ref: BTreeMap<(u32, &str), u64> = ops.clone();
    c.map(
        "§5.3 operators (key, source)",
        &[
            ((3, "inline"), 2_227),
            ((3, "progression"), 20),
            ((5, "inline"), 8_365),
            ((5, "progression"), 28),
            ((6, "inline"), 3_762),
            ((6, "progression"), 357),
            ((7, "inline"), 34_821),
            ((7, "progression"), 298_697),
            ((8, "inline"), 10_939),
            ((8, "progression"), 41),
            ((10, "inline"), 181),
        ],
        &ops_ref,
    );
    c.eq(
        "§5.4 entries with a non-empty modifier list",
        1_946,
        list_entries,
    );
    c.map(
        "§5.4 modifier-list lengths",
        &[
            (1, list_lens.get(&1).copied().unwrap_or(0)),
            (2, list_lens.get(&2).copied().unwrap_or(0)),
        ],
        &list_lens,
    );
    c.map(
        "§5.4 Mod_ModifierOp of those entries",
        &[(5, 668), (6, 108), (7, 887), (8, 283)],
        &list_ops,
    );
    c.eq("§5.5 ranged groups", 14_798, groups);
    c.eq("§5.5 ranged entries", 19_533, ranged_entries);
    c.eq("§5.5 ranged entries with both ends", 19_334, both_ends);
    c.inv("M3 ranged groups are disjoint", disjoint, groups);
    c.inv(
        "M3 ranged groups never mixed with unranged entries",
        0,
        mixed,
    );
    c.inv("§5.5 no ranged entry in a combat holder", 0, combat_ranged);
    c.eq("§5.5 groups not ascending as stored", 100, not_ascending);
    c.eq("§5.5 groups with a gap once sorted", 29, not_contiguous);
    c.map(
        "§5.5 group sizes",
        &[(1, 10_548), (2, 3_789), (3, 437), (4, 24)],
        &sizes,
    );
    c.map(
        "§4.9 Mod_Progression targets over the install",
        &[(3918, 291_927), (1389, 6_735), (1385, 431), (3810, 50)],
        &targets,
    );
    c.map(
        "P7 holders of progressions of progressions",
        &[(3438, 50)],
        &nested_holders,
    );
    eprintln!("whole-install modifier census in {:.1?}", started.elapsed());
    c.finish();
}
