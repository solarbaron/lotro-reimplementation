// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data census of `docs/specs/game-controls.md` against a user's own install
//! (read-only), skipped unless `TESSERA_GAME_DIR` is set:
//!
//! - every WEENIECONTENT entry is classified by the ordered rule of §2.1 with the
//!   observed class indices, and the per-kind counts are those of the spec (GCI15);
//! - the 119 labels of §2.2 resolve and decode (GCI1); the generic forms of §3 hold
//!   (GCI2–GCI4);
//! - a sweep of **every** object record (root classes) and **every** property set
//!   (0x78–0x7F, nested values included) checks the compiled-only tables (GCI5), the mount
//!   traits (GCI6, GCI16), the vital-display unlocks (GCI10), the dictionary-only families
//!   (GCI14, §4.9, §5.10, §8.5) and the slot-material properties (§5.2);
//! - the typed views read every control of the spec's invariants (GCI6–GCI13) and the
//!   values the client uses (§5.3.2, §5.7, §8.1); every property name the views use is
//!   in the install's dictionary with the expected type.
//!
//! Install-specific counts are checked against the local baseline of the install version
//! (decision 0045, `tessera-testkit`), with the spec's figures beside them (§10, last
//! paragraph; drift reports, `TESSERA_CENSUS_STRICT=1`); the invariants and the values the
//! client uses stay assertions. Run with `TESSERA_GAME_DIR=… cargo test
//! --release -p tessera-wstate --test game_controls_real_data -- --nocapture`. Only counts,
//! class indices, keys and DIDs are printed; no game text leaves the install.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_props::{PropertyRegistry, PropertySet, Value};
use tessera_wstate::game_controls::{
    ADVANCEMENT_ATTRIBUTE, AdvancementTable, AggroPingControl, AlertsControl, ClassIndices,
    Collection, CollectionControl, CombatMusicControl, ConsiderationControl, ConsiderationTable,
    DID_SET_ATTRIBUTE, DID_SET_LABELS, DidSet, DiscoveryDistanceControl, DuelTimings,
    ENTITY_NAME_LABEL, EntryKind, GAIT_CANTER, GAIT_GALLOP, GAIT_TROT, GAIT_WALK, GuildControl,
    LoginReward, MANAGER_LABELS, MODIFIER_ATTRIBUTE, ModifierTable, MountDirectory, MountTrait,
    MountedCombatControl, OpenTappingControl, PaperItemService, PaperItemServices, Perk,
    PerkControl, RandomItemRewardControl, ResetScheduleControl, SCOPE_LABELS, SlotSchedule,
    TemplateDiff, TrackingStateControl, USAGE_PROPERTY_ARRAY, VitalDisplayOverride,
    VitalDisplayOverrideControl, WEAPON_STYLE_LABELS, WeenieIndex, entity_name, is_default_attack,
    is_empty_marker, item_vdo_unlocks, unlocked_key,
};
use tessera_wstate::testkit::GAME_CONTROL_PROPERTIES;
use tessera_wstate::{Did, ObjectHeader, ObjectStore, object_dids};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

type Census = tessera_testkit::Census;

/// The census of one test function (decision 0045).
fn census(function: &str) -> Census {
    let dir = game_dir().expect("install");
    tessera_testkit::census!(function, &dir).expect("census")
}

struct Fixture {
    store: ObjectStore<Arc<ArchiveSet>>,
    set: Arc<ArchiveSet>,
    reg: Arc<PropertyRegistry>,
}

fn fixture() -> Option<&'static Fixture> {
    static F: OnceLock<Option<Fixture>> = OnceLock::new();
    F.get_or_init(|| {
        let dir = game_dir()?;
        let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open install"));
        let store = ObjectStore::new(Arc::clone(&set));
        let reg = Arc::clone(store.game_data().registry().expect("registry"));
        store.library().expect("class library");
        Some(Fixture { store, set, reg })
    })
    .as_ref()
}

impl Fixture {
    fn weenie(&self, label: &str) -> Did {
        self.store
            .weenie(label)
            .expect("lookup")
            .unwrap_or_else(|| panic!("label {label} not in WEENIECONTENT"))
    }

    fn props(&self, did: Did) -> PropertySet {
        self.store
            .properties(did)
            .expect("property set reads")
            .unwrap_or_else(|| panic!("{did} has no property set"))
    }

    fn control(&self, label: &str) -> PropertySet {
        self.props(self.weenie(label))
    }

    fn class_of(&self, did: Did) -> u32 {
        self.store
            .header(did)
            .expect("header")
            .unwrap_or_else(|| panic!("{did} absent"))
            .root_class
    }

    fn did_of(&self, set: &PropertySet, name: &str) -> Did {
        set.get_by_name(&self.reg, name)
            .and_then(Value::as_did)
            .unwrap_or_else(|| panic!("{name} absent"))
    }

    /// The key labelled `label` in the enum mapper of property `name`.
    fn enum_key(&self, name: &str, label: &str) -> Option<u32> {
        let pid = self.reg.pid(name)?;
        let tessera_props::EnumSource::Mapper(m) = self.reg.enum_source(pid)? else {
            return None;
        };
        let mapper = self.store.game_data().enum_mapper(m).ok()??;
        mapper.key_of(label)
    }

    /// The number of non-zero keys of the enum mapper of property `name`.
    fn enum_non_zero_keys(&self, name: &str) -> Option<usize> {
        let pid = self.reg.pid(name)?;
        let tessera_props::EnumSource::Mapper(m) = self.reg.enum_source(pid)? else {
            return None;
        };
        let mapper = self.store.game_data().enum_mapper(m).ok()??;
        Some(mapper.names().iter().filter(|(k, _)| *k != 0).count())
    }
}

// --- The sweep ----------------------------------------------------------------------

#[derive(Default)]
struct Sweep {
    /// Root class of every object record.
    classes: HashMap<Did, u32>,
    /// Object records whose header failed.
    bad_headers: usize,
    /// Property sets by DID prefix.
    sets_by_prefix: BTreeMap<u8, usize>,
    /// Property sets that failed to decode.
    failed_sets: Vec<(Did, String)>,
    /// Every PID that holds a value somewhere, nested values included.
    used_nested: HashSet<u32>,
    /// Every PID that holds a value at the top level of some set.
    used_top: HashSet<u32>,
    /// (object, unlocked key or None) of every `VitalDisplayOverride_UnlockedVDOs` value,
    /// at any depth.
    vdo_unlocks: Vec<(Did, Option<u32>)>,
    /// How many of those are below the top level.
    vdo_nested: usize,
    /// How many of those are at depth 1 (GCI17: directly an element of an array).
    vdo_depth1: usize,
    /// (object, unlocked key or None) found the client's way, through
    /// `item_vdo_unlocks` (§7.1).
    vdo_via_array: Vec<(Did, Option<u32>)>,
    /// Sets holding `Usage_SetPropertyUsageData_Array` at the top level.
    usage_array_top: usize,
    /// Occurrences of `Usage_SetPropertyUsageData_Array` below the top level.
    usage_array_nested: usize,
    /// (`RaceTable_Race`, `RaceTable_IsTallRace`) of every set holding the flag at the
    /// top level, and occurrences of the flag below the top level.
    tall_flags: Vec<(Option<u32>, Option<bool>)>,
    tall_nested: usize,
}

impl Sweep {
    fn merge(&mut self, o: Sweep) {
        self.classes.extend(o.classes);
        self.bad_headers += o.bad_headers;
        for (k, v) in o.sets_by_prefix {
            *self.sets_by_prefix.entry(k).or_insert(0) += v;
        }
        self.failed_sets.extend(o.failed_sets);
        self.used_nested.extend(o.used_nested);
        self.used_top.extend(o.used_top);
        self.vdo_unlocks.extend(o.vdo_unlocks);
        self.vdo_nested += o.vdo_nested;
        self.vdo_depth1 += o.vdo_depth1;
        self.vdo_via_array.extend(o.vdo_via_array);
        self.usage_array_top += o.usage_array_top;
        self.usage_array_nested += o.usage_array_nested;
        self.tall_flags.extend(o.tall_flags);
        self.tall_nested += o.tall_nested;
    }

    fn of_class(&self, class: u32) -> BTreeSet<Did> {
        self.classes
            .iter()
            .filter(|&(_, &c)| c == class)
            .map(|(&d, _)| d)
            .collect()
    }
}

fn set_dids(set: &ArchiveSet) -> Vec<Did> {
    let mut dids = Vec::new();
    for member in set.members() {
        for entry in member.archive.entries_range(0x7800_0000..0x8000_0000) {
            dids.push(Did(entry.expect("directory entry").did));
        }
    }
    dids.sort_unstable();
    dids.dedup();
    dids
}

fn parallel<T: Send>(items: &[Did], f: impl Fn(&[Did]) -> T + Sync) -> Vec<T> {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8);
    let next = Mutex::new(0usize);
    let out = Mutex::new(Vec::new());
    const BATCH: usize = 512;
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let start = {
                        let mut n = next.lock().unwrap();
                        let s = *n;
                        *n += BATCH;
                        s
                    };
                    if start >= items.len() {
                        break;
                    }
                    let r = f(&items[start..(start + BATCH).min(items.len())]);
                    out.lock().unwrap().push(r);
                }
            });
        }
    });
    out.into_inner().unwrap()
}

fn sweep() -> Option<&'static Sweep> {
    static S: OnceLock<Option<Sweep>> = OnceLock::new();
    S.get_or_init(|| {
        let f = fixture()?;
        let t = Instant::now();
        let objects = object_dids(&f.set).expect("object DIDs");
        let sets = set_dids(&f.set);
        let data = f.store.game_data();
        let vdo = f.reg.pid("VitalDisplayOverride_UnlockedVDOs");
        let usage = f.reg.pid(USAGE_PROPERTY_ARRAY);
        let tall = f.reg.pid("RaceTable_IsTallRace");
        let race = f.reg.pid("RaceTable_Race");
        let mut total = Sweep::default();
        for part in parallel(&objects, |chunk| {
            let mut s = Sweep::default();
            for &did in chunk {
                match data
                    .read(did)
                    .expect("read")
                    .map(|b| ObjectHeader::parse(&b))
                {
                    Some(Ok(h)) => {
                        s.classes.insert(did, h.root_class);
                    }
                    _ => s.bad_headers += 1,
                }
            }
            s
        }) {
            total.merge(part);
        }
        for part in parallel(&sets, |chunk| {
            let mut s = Sweep::default();
            for &did in chunk {
                *s.sets_by_prefix.entry(did.prefix()).or_insert(0) += 1;
                let bytes = data.read(did).expect("read").expect("listed record");
                match PropertySet::parse_record(&bytes, did, &f.reg) {
                    Ok(set) => {
                        for p in set.iter() {
                            s.used_top.insert(p.pid);
                        }
                        let owner = Did(did.0.wrapping_sub(0x0900_0000));
                        set.walk(|depth, p| {
                            s.used_nested.insert(p.pid);
                            if Some(p.pid) == vdo {
                                s.vdo_unlocks.push((owner, unlocked_key(&p.value)));
                                s.vdo_nested += usize::from(depth > 0);
                                s.vdo_depth1 += usize::from(depth == 1);
                            }
                            if Some(p.pid) == usage && depth > 0 {
                                s.usage_array_nested += 1;
                            }
                            if Some(p.pid) == tall && depth > 0 {
                                s.tall_nested += 1;
                            }
                        });
                        if usage.is_some_and(|u| set.contains(u)) {
                            s.usage_array_top += 1;
                        }
                        for k in item_vdo_unlocks(&set, &f.reg).expect("usage array") {
                            s.vdo_via_array.push((owner, k));
                        }
                        if let Some(t) = tall.and_then(|t| set.get(t)) {
                            s.tall_flags.push((
                                race.and_then(|r| set.get(r)).and_then(Value::as_enum),
                                t.as_bool(),
                            ));
                        }
                    }
                    Err(e) => s.failed_sets.push((did, e.to_string())),
                }
            }
            s
        }) {
            total.merge(part);
        }
        eprintln!(
            "sweep: {} object records ({} bad headers), property sets by prefix {:?} \
             ({} failed), {} PIDs used (nested), {} at top level, {:?}",
            total.classes.len(),
            total.bad_headers,
            total.sets_by_prefix,
            total.failed_sets.len(),
            total.used_nested.len(),
            total.used_top.len(),
            t.elapsed()
        );
        Some(total)
    })
    .as_ref()
}

// --- §2.1 kinds and §2.2 scope ------------------------------------------------------

#[test]
fn entry_kinds_and_scope() {
    let Some(f) = fixture() else { return };
    let mut census = census("entry_kinds_and_scope");
    let idx = ClassIndices::OBSERVED;
    let lib = f.store.library().expect("library");
    let problems = idx.check(&lib);
    assert!(
        problems.is_empty(),
        "class-index cross-checks: {problems:?}"
    );
    let index = WeenieIndex::build(&f.store, &idx).expect("index");
    let counts = index.counts();
    eprintln!(
        "WEENIECONTENT: {} entries; kinds {counts:?}",
        index.entries.len()
    );
    for e in index.entries.iter().filter(|e| e.kind.is_none()) {
        eprintln!(
            "UNCLASSIFIED {} {} class {:?}",
            e.label, e.did, e.root_class
        );
    }
    census.count_vs_spec("GC1 entries", index.entries.len(), 316);
    let expected = [1, 1, 6, 38, 14, 23, 12, 14, 14, 193];
    for (kind, n) in EntryKind::ALL.iter().zip(expected) {
        census.count_vs_spec(
            &format!("GCI15 entries of kind {kind:?}"),
            counts.get(&Some(*kind)).copied().unwrap_or(0),
            n,
        );
    }
    assert_eq!(counts.get(&None), None, "GCI15: every entry matches a row");
    // Every object entry descends from the common ancestors; none from both bases.
    for e in &index.entries {
        if let Some(c) = e.root_class {
            for a in idx.common_ancestors {
                assert!(lib.is_a(c, a), "{} lacks ancestor {a}", e.label);
            }
            assert!(!(lib.is_a(c, idx.plain_object_base) && lib.is_a(c, idx.entity_base)));
        }
    }
    let objects = index
        .entries
        .iter()
        .filter(|e| e.root_class.is_some())
        .count();
    census.count_vs_spec("object entries", objects, 314);

    // GCI1 and §2.2: the 119 labels resolve, decode completely, 97 root classes.
    let mut classes = BTreeSet::new();
    let mut bytes = 0usize;
    for (label, _) in SCOPE_LABELS {
        let e = index
            .get(label)
            .unwrap_or_else(|| panic!("GCI1: {label} not in WEENIECONTENT"));
        let state = f
            .store
            .state(e.did)
            .expect("state decodes")
            .unwrap_or_else(|| panic!("GCI1: {label} record missing"));
        assert!(state.is_complete(), "GCI1: {label} decodes completely");
        let set_did = Did(e.did.0 + 0x0900_0000);
        let raw = f.store.game_data().read(set_did).expect("read");
        assert!(
            f.store.properties(e.did).expect("set decodes").is_some(),
            "{label} set"
        );
        bytes += f
            .store
            .game_data()
            .read(e.did)
            .expect("read")
            .map_or(0, |b| b.len());
        bytes += raw.map_or(0, |b| b.len());
        classes.insert(state.root_class);
    }
    eprintln!(
        "scope: 119 labels, {} root classes, {bytes} bytes",
        classes.len()
    );
    census.count_vs_spec("GC2 root classes", classes.len(), 97);
    census.count_vs_spec("GC2 bytes of object state and property set", bytes, 338_903);

    // Kinds of the spec's groups.
    let kind = |l: &str| index.get(l).and_then(|e| e.kind);
    for (label, _) in SCOPE_LABELS
        .iter()
        .filter(|(l, _)| l.starts_with("Dialog_"))
    {
        assert_eq!(kind(label), Some(EntryKind::DialogMarker), "{label}");
    }
    for label in DID_SET_LABELS {
        assert_eq!(kind(label), Some(EntryKind::DidSet), "{label}");
    }
    for label in MANAGER_LABELS {
        assert_eq!(kind(label), Some(EntryKind::Manager), "{label}");
    }
    for label in WEAPON_STYLE_LABELS {
        assert_eq!(kind(label), Some(EntryKind::Skill), "{label}");
    }
    for label in ["MonsterPlayLevelDirectory", "PvPPlayLevelDirectory"] {
        assert_eq!(kind(label), Some(EntryKind::EmptyMarker), "{label}");
    }
    assert_eq!(
        kind("TravelMountSpeedBoostOverride"),
        Some(EntryKind::Effect)
    );
    assert_eq!(kind("Undef"), Some(EntryKind::Null));
    assert_eq!(
        kind("SharedUISelection_ScriptTable"),
        Some(EntryKind::ScriptTable)
    );
    for label in [
        "WorldEventServer",
        "PrivateEncounterServer",
        "RipCordPersistentUnique",
    ] {
        assert_eq!(kind(label), Some(EntryKind::Entity), "{label}");
    }

    // GCI2: the 52 empty markers.
    let markers: Vec<_> = index
        .of_kind(EntryKind::DialogMarker)
        .chain(index.of_kind(EntryKind::EmptyMarker))
        .collect();
    census.count_vs_spec("GCI2 empty markers", markers.len(), 52);
    let mut marker_classes = BTreeSet::new();
    let mut dialog_parents = BTreeSet::new();
    for e in &markers {
        let data = f.store.game_data();
        let b = data.read(e.did).expect("read").expect("present");
        let sd = Did(e.did.0 + 0x0900_0000);
        let sb = data.read(sd).expect("read").expect("set present");
        let state = f.store.decode(e.did, &b).expect("decodes");
        let set = data.property_set(sd).expect("set").expect("set");
        assert!(
            is_empty_marker(b.len(), &state, sb.len(), &set),
            "GCI2: {} ({} / {} bytes)",
            e.label,
            b.len(),
            sb.len()
        );
        marker_classes.insert(state.root_class);
        if e.kind == Some(EntryKind::DialogMarker) {
            dialog_parents.insert(lib.parent(state.root_class));
        }
    }
    assert_eq!(
        marker_classes.len(),
        markers.len(),
        "GCI2: distinct root classes"
    );
    census.count_vs_spec("GCI2 dialog parents", dialog_parents.len(), 1);
    census.finish();
}

#[test]
fn did_sets_and_managers() {
    let Some(f) = fixture() else { return };
    let mut census = census("did_sets_and_managers");
    let objects: HashSet<Did> = object_dids(&f.set).expect("dids").into_iter().collect();
    // GCI3.
    let sizes = [1_767usize, 767, 2, 14, 2, 0];
    for (label, n) in DID_SET_LABELS.iter().zip(sizes) {
        let did = f.weenie(label);
        let state = f.store.state(did).expect("state").expect("present");
        let s = DidSet::from_state(&state).unwrap_or_else(|e| panic!("{label}: {e}"));
        assert_eq!(s.item_count, 2, "GCI3 {label}: two items");
        assert_eq!(s.attribute, DID_SET_ATTRIBUTE, "{label}");
        assert_eq!(s.duplicates, 0, "{label}");
        census.count_vs_spec(&format!("GCI3 {label}: set size"), s.len(), n);
        assert!(f.props(did).is_empty(), "GCI3 {label}: empty property set");
        let missing = s.members.iter().filter(|d| !objects.contains(d)).count();
        assert_eq!(missing, 0, "GCI3 {label}: members exist");
        eprintln!("DID set {label} ({did}): {} members", s.len());
    }

    // GCI4: the managers share one template apart from the name.
    let lib = f.store.library().expect("library");
    let idx = ClassIndices::OBSERVED;
    let sets: Vec<(&str, PropertySet)> =
        MANAGER_LABELS.iter().map(|&l| (l, f.control(l))).collect();
    let (_, reference) = sets
        .iter()
        .find(|(l, _)| *l != "RepositoryManager")
        .expect("a manager");
    census.count_vs_spec("GC8 template properties", reference.len(), 36);
    let mut differing = BTreeSet::new();
    for (label, s) in &sets {
        assert_eq!(
            lib.parent(f.class_of(f.weenie(label))),
            Some(idx.manager_base),
            "{label}"
        );
        let d = TemplateDiff::between(reference, s);
        assert!(d.only_first.is_empty(), "GCI4 {label}: {d:?}");
        if *label == "RepositoryManager" {
            census.count_vs_spec(
                "GCI4 RepositoryManager version properties",
                d.only_second.len(),
                12,
            );
            for pid in &d.only_second {
                assert!(
                    matches!(s.get(*pid), Some(Value::Int(_))),
                    "versions are ints"
                );
            }
        } else {
            assert!(d.only_second.is_empty(), "GCI4 {label}: {d:?}");
        }
        differing.extend(d.different);
    }
    // GCI4, GCI19: the one differing property is `Name`, found by label (§3.4).
    let name_pid = f
        .reg
        .pid(ENTITY_NAME_LABEL)
        .expect("Name in the dictionary");
    assert_eq!(
        differing.into_iter().collect::<Vec<_>>(),
        vec![name_pid],
        "GCI19: only `Name` differs"
    );
    let phys = f.reg.pid("PhysObj").expect("PhysObj in the dictionary");
    let mut server_parents = BTreeSet::new();
    for label in [
        "WorldEventServer",
        "PrivateEncounterServer",
        "RipCordPersistentUnique",
    ] {
        let s = f.control(label);
        let d = TemplateDiff::between(reference, &s);
        assert!(
            d.only_first.is_empty() && d.only_second.is_empty(),
            "{label}: {d:?}"
        );
        let mut expected = vec![name_pid, phys];
        expected.sort_unstable();
        assert_eq!(
            d.different, expected,
            "GCI4 {label}: name and PhysObj differ"
        );
        let parent = lib.parent(f.class_of(f.weenie(label)));
        assert_ne!(parent, Some(idx.manager_base), "GCI4 {label}");
        server_parents.insert(parent);
        let b = f
            .store
            .game_data()
            .read(f.weenie(label))
            .expect("r")
            .expect("p");
        census.count_vs_spec(&format!("GC8 {label}: object-state bytes"), b.len(), 63);
    }
    census.count_vs_spec("parents of the three servers", server_parents.len(), 1);
    // §3.4: each of the 17 entities holds exactly one textual property, `Name`.
    for label in MANAGER_LABELS.iter().copied().chain([
        "WorldEventServer",
        "PrivateEncounterServer",
        "RipCordPersistentUnique",
    ]) {
        let s = f.control(label);
        let textual: Vec<u32> = s
            .iter()
            .filter(|p| {
                matches!(
                    p.value,
                    Value::StringInfo(_) | Value::String(_) | Value::StringToken(_)
                )
            })
            .map(|p| p.pid)
            .collect();
        assert_eq!(
            textual,
            vec![name_pid],
            "§3.4 {label}: one textual property"
        );
        assert!(
            entity_name(&s, &f.reg).expect("Name").is_some(),
            "§3.4 {label}: Name"
        );
    }
    for label in MANAGER_LABELS {
        let did = f.weenie(label);
        let b = f.store.game_data().read(did).expect("r").expect("p");
        census.count_vs_spec(&format!("GC8 {label}: object-state bytes"), b.len(), 63);
        let state = f.store.state(did).expect("state").expect("present");
        let root = state.root_object().expect("root");
        assert_eq!(root.len(), 3, "{label}: three attributes");
        assert_eq!(
            root.reference_attributes()
                .filter(|(_, r)| r.is_null())
                .count(),
            3,
            "{label}: three null references"
        );
    }
    census.finish();
}

// --- The sweep's invariants ---------------------------------------------------------

#[test]
fn compiled_tables_traits_unlocks_and_families() {
    let Some(f) = fixture() else { return };
    let Some(s) = sweep() else { return };
    let mut census = census("compiled_tables_traits_unlocks_and_families");
    assert_eq!(s.bad_headers, 0, "every object header parses");
    // Every 0x70 object has its 0x79 set (object prefix + 9, `property-system.md` §6.2).
    let objects_0x70 = s.classes.keys().filter(|d| d.prefix() == 0x70).count();
    eprintln!(
        "sweep: {} object records ({objects_0x70} of prefix 0x70); property sets by prefix {:?}",
        s.classes.len(),
        s.sets_by_prefix
    );
    census.count_vs_spec("object records", s.classes.len(), 383_343);
    assert!(
        s.failed_sets.is_empty(),
        "property sets fail: {:?}",
        &s.failed_sets[..s.failed_sets.len().min(5)]
    );
    let sets_0x79 = s.sets_by_prefix.get(&0x79).copied().unwrap_or(0);
    census.count_vs_spec("object property sets", sets_0x79, 383_332);
    assert_eq!(sets_0x79, objects_0x70, "one 0x79 set per 0x70 object");

    census.count_vs_spec(
        "bare property sets",
        s.sets_by_prefix.get(&0x78).copied().unwrap_or(0),
        1_618,
    );
    assert_eq!(s.sets_by_prefix.len(), 2, "no other prefix of 0x78-0x7F");

    // GCI5: class 849 and 852, found from the controls that name them.
    let adv_class = f.class_of(f.did_of(
        &f.control("PVPControl"),
        "PVPControl_Hero_EarnedRankXP_Table",
    ));
    let mod_class = f.class_of(f.did_of(
        &f.control("GloryControl"),
        "GloryControl_GloryModifierTable",
    ));
    let adv = s.of_class(adv_class);
    let mods = s.of_class(mod_class);
    eprintln!(
        "advancement tables: class {adv_class}, {} objects; modifier tables: class {mod_class}, {} objects",
        adv.len(),
        mods.len()
    );
    census.count_vs_spec("GC11 advancement tables", adv.len(), 29);
    census.count_vs_spec("GC11 modifier tables", mods.len(), 3);
    let mut lens = (usize::MAX, 0usize);
    for &d in &adv {
        let st = f.store.state(d).expect("state").expect("present");
        let t = AdvancementTable::from_state(&st).unwrap_or_else(|e| panic!("{d}: {e}"));
        assert_eq!(t.attribute, ADVANCEMENT_ATTRIBUTE);
        assert!(t.is_cumulative(), "GCI5 {d}");
        assert!(f.props(d).is_empty(), "{d}: empty property set");
        lens = (lens.0.min(t.values.len()), lens.1.max(t.values.len()));
    }
    census.count_vs_spec("§4.10 shortest advancement table", lens.0, 11);
    census.count_vs_spec("§4.10 longest advancement table", lens.1, 201);
    let (mut keys, mut lowest, mut highest) = ((usize::MAX, 0usize), f64::MAX, f64::MIN);
    for &d in &mods {
        let st = f.store.state(d).expect("state").expect("present");
        let t = ModifierTable::from_state(&st).unwrap_or_else(|e| panic!("{d}: {e}"));
        assert_eq!(t.attribute, MODIFIER_ATTRIBUTE);
        assert!(t.all_finite(), "GCI5 {d}");
        keys = (keys.0.min(t.entries.len()), keys.1.max(t.entries.len()));
        for &(_, v) in &t.entries {
            lowest = lowest.min(f64::from(v));
            highest = highest.max(f64::from(v));
        }
    }
    census.count_vs_spec("§4.10 fewest modifier keys", keys.0, 24);
    census.count_vs_spec("§4.10 most modifier keys", keys.1, 24);
    // The §4.10 range 0.28–24.0, in thousandths.
    #[allow(clippy::cast_possible_truncation)]
    let milli = |v: f64| (v * 1000.0).round() as i64;
    census.count_vs_spec("§4.10 lowest modifier (1/1000)", milli(lowest), 280i64);
    census.count_vs_spec("§4.10 highest modifier (1/1000)", milli(highest), 24_000i64);

    // §4.2: slot schedules.
    let virtue = f.control("PVPVirtueControl");
    let pvp_schedule = f.did_of(&virtue, "PVPVirtueControl_PVPVirtueSlotSchedule");
    let schedules = s.of_class(f.class_of(pvp_schedule));
    census.count_vs_spec("§4.2 slot schedules", schedules.len(), 15);
    for &d in &schedules {
        let st = f.store.state(d).expect("state").expect("present");
        SlotSchedule::from_state(&st).unwrap_or_else(|e| panic!("{d}: {e}"));
        assert!(f.props(d).is_empty(), "{d}: empty property set");
    }
    let st = f
        .store
        .state(pvp_schedule)
        .expect("state")
        .expect("present");
    let pvp = SlotSchedule::from_state(&st).expect("PvP schedule");
    let mut l = pvp.lengths();
    l.sort_unstable();
    census.count_vs_spec("§4.2 PvP schedule lists", l.len(), 3);
    for (i, &n) in l.iter().enumerate() {
        let name = format!("§4.2 PvP schedule list {i} (by length)");
        match [0, 6, 6].get(i) {
            Some(&spec) => census.count_vs_spec(&name, n, spec),
            None => census.count(&name, n),
        }
    }
    assert!(pvp.lists.iter().all(|(_, v)| v.iter().all(|&x| x == 1)));

    // GCI6 second half and GCI16: the mount traits.
    let traits_did = f.weenie("MountAppearanceTraits");
    let traits =
        DidSet::from_state(&f.store.state(traits_did).expect("s").expect("p")).expect("set");
    let trait_classes: BTreeSet<u32> = traits
        .members
        .iter()
        .map(|d| s.classes.get(d).copied().expect("member exists"))
        .collect();
    assert_eq!(trait_classes.len(), 1, "one trait class");
    let trait_class = *trait_classes.first().expect("one");
    let all: BTreeSet<Did> = s.of_class(trait_class);
    let members: BTreeSet<Did> = traits.members.iter().copied().collect();
    assert_eq!(
        members, all,
        "GCI6: MountAppearanceTraits is every object of class {trait_class}"
    );
    let control = MountedCombatControl::from_properties(&f.control("MountedCombatControl"), &f.reg)
        .expect("control");
    let hash_keys: BTreeSet<u32> = control
        .material_properties
        .iter()
        .filter_map(|(k, _)| *k)
        .collect();
    census.count_vs_spec("§5.2 slot-material hash keys", hash_keys.len(), 7);
    let mut per_slot: BTreeMap<u32, [usize; 3]> = BTreeMap::new();
    let mut other_tier_or_cost = 0usize;
    for &d in &traits.members {
        let t =
            MountTrait::from_properties(&f.props(d), &f.reg).unwrap_or_else(|e| panic!("{d}: {e}"));
        assert_eq!(
            t.top_level_group, None,
            "GCI16 {d}: no top-level Trait_Group"
        );
        let slot = t
            .slot()
            .unwrap_or_else(|| panic!("GCI16 {d}: one nested Trait_Group"));
        assert!(
            hash_keys.contains(&slot),
            "GCI16 {d}: slot {slot} in the hash"
        );
        assert_eq!(t.appearance_sets.len(), 2, "GCI16 {d}");
        assert!(t.appearance_sets.iter().all(|s| s.len() == 1), "GCI16 {d}");
        for a in t.appearance_sets.iter().flatten() {
            assert!(
                a.apr_key.is_some() && a.color_apr_key.is_some() && a.apr_value.is_some(),
                "§5.2 {d}"
            );
            assert!(
                a.apr.is_none_or(|x| x.prefix() == 0x20),
                "§5.2 {d}: APR prefix"
            );
        }
        other_tier_or_cost += usize::from((t.tier, t.cost_to_slot) != (Some(1), Some(0)));
        let e = per_slot.entry(slot).or_default();
        e[0] += 1;
        e[1] += usize::from(t.is_dyable == Some(true));
        e[2] += usize::from(t.material.is_some_and(|m| m != 0));
    }
    eprintln!("mount traits per slot (count, dyable, material): {per_slot:?}");
    let expected: BTreeMap<u32, [usize; 3]> = [
        (8, [152, 129, 30]),
        (9, [175, 166, 70]),
        (10, [105, 82, 0]),
        (11, [54, 54, 0]),
        (12, [153, 10, 0]),
        (13, [118, 5, 0]),
        (14, [10, 10, 0]),
    ]
    .into_iter()
    .collect();
    census.count_vs_spec(
        "§5.2 traits not of tier 1 and cost 0",
        other_tier_or_cost,
        0,
    );
    let slots: BTreeSet<u32> = expected.keys().chain(per_slot.keys()).copied().collect();
    for slot in slots {
        let got = per_slot.get(&slot).copied().unwrap_or_default();
        let spec = expected.get(&slot);
        for (i, what) in ["traits", "dyable", "with a material"]
            .into_iter()
            .enumerate()
        {
            let name = format!("GCI16 slot {slot}: {what}");
            match spec {
                Some(e) => census.count_vs_spec(&name, got[i], e[i]),
                None => census.count(&name, got[i]),
            }
        }
    }
    // §5.2: the slot-material properties named by the hash occur in no set.
    for (_, pid) in &control.material_properties {
        let pid = pid.expect("material PID");
        assert!(
            !s.used_nested.contains(&pid),
            "§5.2: slot-material property {pid:#x} holds a value"
        );
    }

    // GCI10: the unlocking items.
    let vdo = VitalDisplayOverrideControl::from_properties(
        &f.control("gmVitalDisplayOverrideControl"),
        &f.reg,
    )
    .expect("control");
    let mut keys = BTreeSet::new();
    for &o in &vdo.overrides {
        let v = VitalDisplayOverride::from_properties(&f.props(o), &f.reg).expect("override");
        let k = v.key.expect("RequiredEnum");
        assert!(
            k != 0 && keys.insert(k),
            "GCI10: override keys distinct and non-zero"
        );
    }
    eprintln!(
        "VDO unlock values: {} ({} nested)",
        s.vdo_unlocks.len(),
        s.vdo_nested
    );
    let owners: BTreeSet<Did> = s.vdo_unlocks.iter().map(|&(d, _)| d).collect();
    assert_eq!(
        owners.len(),
        s.vdo_unlocks.len(),
        "§7.1: one unlock value per item"
    );
    let unlock_keys: Vec<u32> = s
        .vdo_unlocks
        .iter()
        .map(|&(d, k)| k.unwrap_or_else(|| panic!("GCI10 {d}: one bit")))
        .collect();
    let unlock_set: BTreeSet<u32> = unlock_keys.iter().copied().collect();
    census.count_vs_spec("GC23 unlocking items", unlock_keys.len(), 111);
    assert_eq!(unlock_set.len(), unlock_keys.len(), "GC23: distinct keys");
    assert!(
        unlock_set.is_subset(&keys),
        "GCI10: every unlock names an override"
    );
    assert!(
        !unlock_set.contains(&1) && !unlock_set.contains(&2),
        "§7.1: keys 1 and 2 not unlocked"
    );
    let unlock_classes: BTreeSet<u32> = s
        .vdo_unlocks
        .iter()
        .filter_map(|(d, _)| s.classes.get(d).copied())
        .collect();
    eprintln!(
        "VDO unlocks: {} items of classes {unlock_classes:?}",
        unlock_keys.len()
    );
    census.count_vs_spec(
        "§7.1 classes of the unlocking items",
        unlock_classes.len(),
        1,
    );
    // GCI17: each unlock is the single element of the item's top-level usage array; the
    // client's way of finding it (`item_vdo_unlocks`) finds every value of the install.
    eprintln!(
        "usage arrays: {} top-level, {} nested; VDO values at depth 1: {}, via the array: {}",
        s.usage_array_top,
        s.usage_array_nested,
        s.vdo_depth1,
        s.vdo_via_array.len()
    );
    assert_eq!(
        s.vdo_nested,
        s.vdo_unlocks.len(),
        "GCI17: none at the top level"
    );
    assert_eq!(
        s.vdo_depth1,
        s.vdo_unlocks.len(),
        "GCI17: none inside a struct"
    );
    let mut all = s.vdo_unlocks.clone();
    let mut via = s.vdo_via_array.clone();
    all.sort_unstable();
    via.sort_unstable();
    assert_eq!(
        via, all,
        "GCI17: every unlock is an element of the usage array"
    );
    assert_eq!(
        s.usage_array_nested, 0,
        "§7.1: the usage array is top-level only"
    );
    census.count_vs_spec("§7.1 sets holding the usage array", s.usage_array_top, 112);

    // GCI14 and the family counts of §4.9, §5.10, §8.5.
    let families: [(&str, usize, Option<usize>); 12] = [
        ("LFG_", 17, Some(0)),
        ("Antispam_", 65, Some(0)),
        ("Mail_", 40, Some(12)),
        ("Auction_", 117, Some(47)),
        ("Guild_", 58, Some(20)),
        ("MonsterPlay_", 108, Some(73)),
        ("PVP_", 37, Some(15)),
        ("PVPCombat_", 70, Some(62)),
        ("PvP_", 3, Some(1)),
        ("Mount_", 149, Some(96)),
        ("MountedCombat_", 18, Some(13)),
        ("MountPower_", 7, None),
    ];
    for (prefix, defined, used) in families {
        let pids: Vec<u32> = f
            .reg
            .iter()
            .filter(|(_, n)| n.is_some_and(|n| n.starts_with(prefix)))
            .map(|(d, _)| d.pid)
            .collect();
        let nested = pids.iter().filter(|p| s.used_nested.contains(p)).count();
        let top = pids.iter().filter(|p| s.used_top.contains(p)).count();
        eprintln!(
            "family {prefix}: {} defined, {nested} used (nested), {top} at top level",
            pids.len()
        );
        census.count_vs_spec(&format!("GC30 {prefix}: defined"), pids.len(), defined);
        match used {
            Some(u) => census.count_vs_spec(&format!("GC30 {prefix}: used (nested)"), nested, u),
            None => census.count(&format!("GC30 {prefix}: used (nested)"), nested),
        }
        census.count(&format!("GC30 {prefix}: used at the top level"), top);
    }
    census.finish();
}

// --- Typed views ----------------------------------------------------------------------

#[test]
fn view_names_exist_with_their_types() {
    let Some(f) = fixture() else { return };
    let mut missing = Vec::new();
    let mut retyped = Vec::new();
    for &(name, ty) in GAME_CONTROL_PROPERTIES {
        if name.starts_with("Test_") {
            continue;
        }
        match f.reg.pid(name) {
            None => missing.push(name),
            Some(pid) => {
                let found = f.reg.property_type(pid).map(|t| t.code());
                // GCI18: the flank heading and arcs are floats (§5.3.1).
                if found != Some(ty) {
                    retyped.push((name, ty, found));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "names not in the dictionary: {missing:?}"
    );
    assert!(
        retyped.is_empty(),
        "type codes differ (name, synthetic, install): {retyped:?}"
    );
}

#[test]
fn mount_and_combat_views() {
    let Some(f) = fixture() else { return };
    let mut census = census("mount_and_combat_views");
    let idx = ClassIndices::OBSERVED;
    // GCI6 first half.
    let dir =
        MountDirectory::from_properties(&f.control("MountDirectory"), &f.reg).expect("directory");
    eprintln!(
        "mount skills: {} tall, {} short, {} other",
        dir.tall.len(),
        dir.short.len(),
        dir.other
    );
    census.count_vs_spec("GC12 tall mount skills", dir.tall.len(), 347);
    census.count_vs_spec("GC12 short mount skills", dir.short.len(), 329);
    assert_eq!(dir.other, 0, "GC12: no other size");
    assert!(dir.is_disjoint(), "GCI6: disjoint");
    let all: BTreeSet<Did> = dir.skills().collect();
    assert_eq!(
        all.len(),
        dir.tall.len() + dir.short.len(),
        "GC12: distinct"
    );
    for d in &all {
        assert_eq!(f.class_of(*d), idx.skill, "GCI6: {d} is a skill");
    }
    // §5.1, GC12: the tall flag sits at the top level of the eight species objects of the
    // race table, nowhere else; four species are tall.
    let s = sweep().expect("sweep");
    let races: BTreeSet<u32> = s.tall_flags.iter().filter_map(|t| t.0).collect();
    let tall = s.tall_flags.iter().filter(|t| t.1 == Some(true)).count();
    eprintln!(
        "tall flags: {} sets, {} species, {tall} tall, {} nested",
        s.tall_flags.len(),
        races.len(),
        s.tall_nested
    );
    census.count_vs_spec(
        "GC12 species objects with the tall flag",
        s.tall_flags.len(),
        8,
    );
    assert_eq!(races.len(), s.tall_flags.len(), "GC12: distinct species");
    census.count_vs_spec("GC12 tall species", tall, 4);
    assert_eq!(s.tall_nested, 0, "GC12: top level only");
    assert_eq!(dir.skills_for_size(true), dir.tall.as_slice());
    assert_eq!(dir.skills_for_size(false), dir.short.as_slice());
    // §5.3.1 values and the §5.3.2 rules on them.
    let c = MountedCombatControl::from_properties(&f.control("MountedCombatControl"), &f.reg)
        .expect("control");
    assert_eq!(
        c.speed,
        [Some(0.5), Some(0.7), Some(0.8), Some(1.0)],
        "§5.3.1 speed"
    );
    assert_eq!(
        c.turn_rate,
        [Some(1.0), Some(1.0), Some(0.55), Some(0.47)],
        "§5.3.1 turn"
    );
    assert!(
        c.turn_add_mod.iter().all(|p| p.is_some_and(|p| p != 0)),
        "four add-mod PIDs"
    );
    assert_eq!(c.non_mounted_combat_max_speed, Some(0.75));
    assert_eq!(
        (c.leash_distance, c.max_leash_distance),
        (Some(250.0), Some(500.0))
    );
    assert_eq!(
        (c.within_leash_timeout, c.outside_leash_timeout),
        (Some(120.0), Some(60.0))
    );
    assert_eq!(
        (c.steed_xp_mounted, c.steed_xp_unmounted),
        (Some(1.0), Some(0.3))
    );
    census.count_vs_spec("§5.3.1 trees", c.trees.len(), 3);
    census.count_vs_spec("§5.3.1 tree skill hashes", c.tree_skills.len(), 3);
    let trees: BTreeSet<u32> = c.trees.iter().filter_map(|t| t.tree).collect();
    let skill_trees: BTreeSet<u32> = c.tree_skills.iter().filter_map(|t| t.tree).collect();
    eprintln!(
        "mount trees: {} keyed tree entries, {} keyed skill-hash entries",
        trees.len(),
        skill_trees.len()
    );
    assert_eq!(
        trees.len(),
        c.trees.len(),
        "§5.3.1: one tree entry per trait tree"
    );
    assert_eq!(skill_trees, trees, "§5.3.1: one skill hash per trait tree");
    assert!(
        c.trees.iter().all(|t| t.effect.is_some()),
        "§5.3.1: tree effects"
    );
    assert!(
        c.tree_skills.iter().all(|t| t.skills.len() == 8),
        "§5.3.1: 8 races per tree"
    );
    let skills: Vec<_> = c
        .tree_skills
        .iter()
        .flat_map(|t| t.skills.iter().filter_map(|s| s.1))
        .collect();
    census.count_vs_spec("§5.3.1 tree skill entries", skills.len(), 24);
    census.count_vs_spec(
        "§5.3.1 distinct tree skills",
        skills.iter().collect::<BTreeSet<_>>().len(),
        12,
    );
    census.count_vs_spec("§5.3.1 flanks", c.flanks.len(), 10);
    // GCI18: the view reads the flank angles only as floats, so all thirty being present
    // means all are floats; the tree keys agree between the two arrays.
    assert!(
        c.flanks.iter().all(|fl| fl.heading.is_some()
            && fl.left_arc.is_some()
            && fl.right_arc.is_some()
            && fl.position.is_some()),
        "GCI18: flank fields present as floats"
    );
    assert!(c.tree_keys_consistent(), "GCI18: tree keys");
    census.count_vs_spec("§5.2 dye slots", c.dye_slots.len(), 5);
    let dye: BTreeSet<u32> = c.dye_slots.iter().filter_map(|d| d.slot).collect();
    assert_eq!(
        dye,
        [8, 9, 10, 11, 14].into_iter().collect(),
        "§5.2 Q29: dye slots"
    );
    assert_eq!(c.speed_multiplier(GAIT_WALK), 0.5);
    assert_eq!(c.speed_multiplier(GAIT_GALLOP), 1.0);
    assert_eq!(c.turn_rate(GAIT_CANTER, |_| None), 0.55);
    assert_eq!(c.turn_rate(GAIT_TROT, |_| None), 1.0);
    let steed = c.steed_xp_table.expect("steed table");
    assert_eq!(
        f.class_of(steed),
        f.class_of(f.did_of(
            &f.control("PVPControl"),
            "PVPControl_Hero_EarnedRankXP_Table"
        )),
        "steed table is class 849"
    );
    // GCI7.
    for label in WEAPON_STYLE_LABELS {
        let d = f.weenie(label);
        assert_eq!(f.class_of(d), idx.skill, "{label}");
        assert!(is_default_attack(&f.props(d), &f.reg), "GCI7 {label}");
    }
    // §5.7.
    let duel = DuelTimings::from_properties(&f.control("CombatControl"), &f.reg);
    assert_eq!(
        (duel.begin_delay, duel.length),
        (Some(10.0), Some(600.0)),
        "§5.7"
    );
    census.finish();
}

#[test]
fn account_views() {
    let Some(f) = fixture() else { return };
    let Some(s) = sweep() else { return };
    let mut census = census("account_views");
    // GCI8.
    let pc = PerkControl::from_properties(&f.control("PerkControl"), &f.reg).expect("perk control");
    census.count_vs_spec("GCI8 perk channels", pc.channels.len(), 28);
    assert!(pc.keys_are_distinct(), "GCI8");
    census.count_vs_spec("GCI8 available perks", pc.available.len(), 2);
    let perk_class = f.class_of(*pc.available.first().expect("perk"));
    let perks = s.of_class(perk_class);
    census.count_vs_spec("GC19 perks", perks.len(), 47);
    for &d in &perks {
        let p = Perk::from_properties(&f.props(d), &f.reg).expect("perk");
        assert!(pc.admits(&p), "GCI8 {d}");
        assert!(
            p.name.is_some() && p.icon.is_some() && p.effect.is_some(),
            "{d}"
        );
    }
    // §6.2.
    let cc = CollectionControl::from_properties(&f.control("CollectionControl"), &f.reg)
        .expect("collections");
    census.count_vs_spec("§6.2 collections", cc.collections.len(), 29);
    let coll_class = f.class_of(*cc.collections.first().expect("one"));
    assert_eq!(
        s.of_class(coll_class),
        cc.collections.iter().copied().collect(),
        "every class member listed"
    );
    let mut pieces = BTreeSet::new();
    let mut n = 0;
    for &d in &cc.collections {
        let c = Collection::from_properties(&f.props(d), &f.reg).expect("collection");
        n += c.pieces.len();
        pieces.extend(c.pieces);
    }
    census.count_vs_spec("§6.2 pieces", n, 131);
    census.count_vs_spec("§6.2 distinct pieces", pieces.len(), 131);
    // GCI9.
    let pi = PaperItemServices::from_properties(&f.control("PaperItemControl"), &f.reg)
        .expect("services");
    census.count_vs_spec("GCI9 paper-item services", pi.services.len(), 35);
    let mut types = BTreeSet::new();
    for &d in &pi.services {
        let e = PaperItemService::from_properties(&f.props(d), &f.reg).expect("service");
        assert!(
            types.insert(e.service_type.expect("type")),
            "GCI9: distinct types"
        );
    }
    // §6.6, §6.7.
    let rc =
        RandomItemRewardControl::from_properties(&f.control("RandomItemRewardControl"), &f.reg)
            .expect("rewards");
    census.count_vs_spec("§6.6 reward categories", rc.categories.len(), 2);
    assert!(
        rc.categories.iter().all(|(_, t)| t.len() == 5),
        "§6.6: 5 tiers each"
    );
    assert_eq!(rc.chance_to_show_rarest, Some(0.1));
    let logins = DidSet::from_state(
        &f.store
            .state(f.weenie("LoginRewards"))
            .expect("s")
            .expect("p"),
    )
    .expect("set");
    for &d in &logins.members {
        let l = LoginReward::from_properties(&f.props(d), &f.reg).expect("login reward");
        let cat = l.control_category.expect("category PID");
        assert!(
            rc.tiers(cat).is_some(),
            "§6.6: login reward {d} names a category"
        );
        assert_eq!(l.treasure_tables.len(), 3, "§6.7");
    }
    census.count("§6.7 login rewards", logins.members.len());
    census.finish();
}

#[test]
fn ui_and_social_views() {
    let Some(f) = fixture() else { return };
    let Some(s) = sweep() else { return };
    let mut census = census("ui_and_social_views");
    // GCI10 control half.
    let vdo = VitalDisplayOverrideControl::from_properties(
        &f.control("gmVitalDisplayOverrideControl"),
        &f.reg,
    )
    .expect("control");
    census.count_vs_spec("GCI10 overrides", vdo.overrides.len(), 113);
    let class = f.class_of(*vdo.overrides.first().expect("one"));
    assert_eq!(
        s.of_class(class),
        vdo.overrides.iter().copied().collect(),
        "GCI10: every override listed once"
    );
    assert_eq!(
        vdo.overrides.iter().collect::<BTreeSet<_>>().len(),
        vdo.overrides.len(),
        "GCI10: overrides distinct"
    );
    // GCI11.
    let a = AlertsControl::from_properties(&f.control("AlertsControl"), &f.reg).expect("alerts");
    assert_eq!(a.max_big_alerts, Some(5));
    census.count_vs_spec("GCI11 alert types", a.overflow.len(), 64);
    assert!(a.types_are_distinct_and_non_zero(), "GCI11");
    assert_eq!(
        f.enum_non_zero_keys("Alert_Type"),
        Some(a.overflow.len()),
        "GC24: every non-zero key"
    );
    census.count_vs_spec(
        "GCI11 alert types that overflow",
        a.overflow.iter().filter(|(_, o)| *o == Some(true)).count(),
        59,
    );
    // §7.4.
    let r = ResetScheduleControl::from_properties(&f.control("ResetScheduleControl"), &f.reg)
        .expect("reset");
    census.count_vs_spec("§7.4 reset schedules", r.schedules.len(), 3);
    for (label, days) in [
        ("Daily", vec![1, 2, 3, 4, 5, 6, 7]),
        ("BiWeekly", vec![2, 5]),
        ("Weekly", vec![5]),
    ] {
        let key = f
            .enum_key("ResetScheduleControl_ResetSchedule", label)
            .expect("schedule key");
        assert_eq!(r.get(key).map(|s| s.day_keys()), Some(days), "§7.4 {label}");
    }
    // GCI12.
    let cc = ConsiderationControl::from_properties(&f.control("ConsiderationControl"), &f.reg)
        .expect("consideration");
    let even = f
        .enum_key("ConsiderationTable_ConsiderRank", "Even")
        .expect("Even key");
    census.count_vs_spec("GCI12 rank-to-UI states", cc.rank_to_ui_state.len(), 9);
    census.count_vs_spec("GCI12 combat consider types", cc.combat_tables.len(), 6);
    let types: BTreeSet<u32> = cc.combat_tables.iter().filter_map(|(t, _)| *t).collect();
    assert_eq!(
        types.len(),
        cc.combat_tables.len(),
        "GCI12: distinct consider types"
    );
    let mut combat = BTreeSet::new();
    for (_, list) in &cc.combat_tables {
        assert_eq!(
            list.len(),
            cc.quest_tables.len(),
            "GCI12: one table per level"
        );
        combat.extend(list.iter().copied());
    }
    census.count_vs_spec("GCI12 levels", cc.quest_tables.len(), 120);
    let quest: BTreeSet<Did> = cc.quest_tables.iter().copied().collect();
    census.count_vs_spec("GC26 distinct combat tables", combat.len(), 6);
    census.count_vs_spec("GC26 distinct quest tables", quest.len(), 2);
    for &d in combat.iter().chain(&quest) {
        let t = ConsiderationTable::from_properties(&f.props(d), &f.reg).expect("table");
        assert!(t.is_well_formed(even), "GCI12 {d}");
    }
    // GCI13.
    let t = TrackingStateControl::from_properties(&f.control("TrackingStateControl"), &f.reg)
        .expect("tracking");
    census.count_vs_spec("GCI13 tracking states", t.entries.len(), 17);
    assert!(t.states_are_distinct(), "GCI13");
    // §7.7–§7.9, §7.15.
    let dd =
        DiscoveryDistanceControl::from_properties(&f.control("DiscoveryDistanceControl"), &f.reg)
            .expect("discovery");
    let mut th: Vec<f32> = dd.thresholds.iter().filter_map(|t| t.1).collect();
    th.sort_by(f32::total_cmp);
    assert_eq!(th, vec![20.0, 40.0, 80.0, 120.0], "§7.9");
    let m = CombatMusicControl::from_properties(&f.control("CombatMusicControl"), &f.reg)
        .expect("music");
    assert_eq!(
        (m.floor, m.ceiling, m.modifiers.len()),
        (Some(-5), Some(15), 7),
        "§7.8"
    );
    let ap =
        AggroPingControl::from_properties(&f.control("AggroPingControl"), &f.reg).expect("aggro");
    assert_eq!(
        (ap.frequency, ap.challenge_level_window),
        (Some(17.0), Some(15)),
        "§7.7"
    );
    let ot = OpenTappingControl::from_properties(&f.control("OpenTappingControl"), &f.reg)
        .expect("tapping");
    assert_eq!(ot.max_credit_distance, Some(120.0), "§7.15");
    // §8.1.
    let g = GuildControl::from_properties(&f.control("GuildControl"), &f.reg).expect("guild");
    assert_eq!(g.max_members, Some(1_000));
    assert_eq!(
        (g.name_max_length, g.rank_name_max_length, g.motd_max_length),
        (Some(32), Some(32), Some(250))
    );
    census.count_vs_spec("§8.1 guild ranks", g.ranks.len(), 4);
    assert_eq!(
        g.player_note_max_length, None,
        "§8.1: absent in the install"
    );
    assert_eq!(g.player_note_limit(), 48, "§8.1: the client default");
    census.finish();
}
