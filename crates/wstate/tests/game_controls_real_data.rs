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
//! Install-specific counts are compared because the spec's install and the test install
//! are the same data (§10, last paragraph). Run with `TESSERA_GAME_DIR=… cargo test
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
    DID_SET_ATTRIBUTE, DID_SET_LABELS, DidSet, DiscoveryDistanceControl, DuelTimings, EntryKind,
    GAIT_CANTER, GAIT_GALLOP, GAIT_TROT, GAIT_WALK, GuildControl, LoginReward, MANAGER_LABELS,
    MODIFIER_ATTRIBUTE, ModifierTable, MountDirectory, MountTrait, MountedCombatControl,
    OpenTappingControl, PaperItemService, PaperItemServices, Perk, PerkControl,
    RandomItemRewardControl, ResetScheduleControl, SCOPE_LABELS, SlotSchedule, TemplateDiff,
    TrackingStateControl, VitalDisplayOverride, VitalDisplayOverrideControl, WEAPON_STYLE_LABELS,
    WeenieIndex, is_default_attack, is_empty_marker, unlocked_key,
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
    /// at any depth (the spec does not state where in an item's set it is).
    vdo_unlocks: Vec<(Did, Option<u32>)>,
    /// How many of those are below the top level.
    vdo_nested: usize,
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
                            }
                        });
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
    assert_eq!(index.entries.len(), 316, "GC1: 316 entries");
    let expected = [1, 1, 6, 38, 14, 23, 12, 14, 14, 193];
    for (kind, n) in EntryKind::ALL.iter().zip(expected) {
        assert_eq!(
            counts.get(&Some(*kind)).copied().unwrap_or(0),
            n,
            "GCI15 {kind:?}"
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
    assert_eq!(objects, 314, "314 object entries");

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
    assert_eq!(classes.len(), 97, "GC2: 97 root classes");
    assert_eq!(
        bytes, 338_903,
        "GC2: bytes of object state and property set"
    );

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
    assert_eq!(markers.len(), 52);
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
    assert_eq!(marker_classes.len(), 52, "GCI2: distinct root classes");
    assert_eq!(dialog_parents.len(), 1, "GCI2: one dialog parent");
}

#[test]
fn did_sets_and_managers() {
    let Some(f) = fixture() else { return };
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
        assert_eq!(s.len(), n, "{label}: set size");
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
    assert_eq!(reference.len(), 36, "GC8: 36-property template");
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
            assert_eq!(d.only_second.len(), 12, "GCI4: 12 version properties");
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
    assert_eq!(differing.len(), 1, "GCI4: only the literal name differs");
    let name_pid = *differing.first().expect("one");
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
        assert_eq!(b.len(), 63, "GC8: 63-byte object state");
    }
    assert_eq!(server_parents.len(), 1, "the three share one parent");
    for label in MANAGER_LABELS {
        let did = f.weenie(label);
        let b = f.store.game_data().read(did).expect("r").expect("p");
        assert_eq!(b.len(), 63, "GC8 {label}: 63-byte object state");
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
}

// --- The sweep's invariants ---------------------------------------------------------

#[test]
fn compiled_tables_traits_unlocks_and_families() {
    let Some(f) = fixture() else { return };
    let Some(s) = sweep() else { return };
    assert_eq!(s.bad_headers, 0, "every object header parses");
    assert_eq!(s.classes.len(), 383_343, "OBS: object records");
    assert!(
        s.failed_sets.is_empty(),
        "property sets fail: {:?}",
        &s.failed_sets[..s.failed_sets.len().min(5)]
    );
    assert_eq!(
        s.sets_by_prefix.get(&0x79),
        Some(&383_332),
        "OBS: object property sets"
    );
    assert_eq!(
        s.sets_by_prefix.get(&0x78),
        Some(&1_618),
        "OBS: bare property sets"
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
    assert_eq!(adv.len(), 29, "GC11: 29 advancement tables");
    assert_eq!(mods.len(), 3, "GC11: 3 modifier tables");
    let mut lens = (usize::MAX, 0usize);
    for &d in &adv {
        let st = f.store.state(d).expect("state").expect("present");
        let t = AdvancementTable::from_state(&st).unwrap_or_else(|e| panic!("{d}: {e}"));
        assert_eq!(t.attribute, ADVANCEMENT_ATTRIBUTE);
        assert!(t.is_cumulative(), "GCI5 {d}");
        assert!(f.props(d).is_empty(), "{d}: empty property set");
        lens = (lens.0.min(t.values.len()), lens.1.max(t.values.len()));
    }
    assert_eq!(lens, (11, 201), "§4.10: 11 to 201 values");
    for &d in &mods {
        let st = f.store.state(d).expect("state").expect("present");
        let t = ModifierTable::from_state(&st).unwrap_or_else(|e| panic!("{d}: {e}"));
        assert_eq!(t.attribute, MODIFIER_ATTRIBUTE);
        assert!(t.all_finite(), "GCI5 {d}");
        assert_eq!(t.entries.len(), 24, "§4.10: 24 keys");
        for &(_, v) in &t.entries {
            assert!((0.28..=24.0).contains(&v), "§4.10 range: {v}");
        }
    }

    // §4.2: slot schedules.
    let virtue = f.control("PVPVirtueControl");
    let pvp_schedule = f.did_of(&virtue, "PVPVirtueControl_PVPVirtueSlotSchedule");
    let schedules = s.of_class(f.class_of(pvp_schedule));
    assert_eq!(schedules.len(), 15, "§4.2: 15 slot schedules");
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
    assert_eq!(l, vec![0, 6, 6], "§4.2: PvP schedule lists");
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
    assert_eq!(hash_keys.len(), 7);
    let mut per_slot: BTreeMap<u32, [usize; 3]> = BTreeMap::new();
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
        assert_eq!((t.tier, t.cost_to_slot), (Some(1), Some(0)), "§5.2 {d}");
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
    assert_eq!(per_slot, expected, "GCI16 / §5.2 per-slot counts");
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
    assert_eq!(unlock_keys.len(), 111, "GC23: 111 unlocking items");
    assert_eq!(unlock_set.len(), 111, "GC23: distinct keys");
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
    assert_eq!(
        unlock_classes.len(),
        1,
        "§7.1: the unlocking items share one class"
    );

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
    let mut mismatches = Vec::new();
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
        assert_eq!(pids.len(), defined, "GC30 {prefix}: defined");
        if let Some(u) = used
            && nested != u
        {
            mismatches.push((prefix, u, nested, top));
        }
    }
    assert!(
        mismatches.is_empty(),
        "GC30 used counts (spec, nested, top): {mismatches:?}"
    );
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
                // The flank heading's stored type is not stated (§5.3.1); the view
                // accepts an int or a float.
                if found != Some(ty) && name != "Mount_Control_Flank_Heading" {
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
    let idx = ClassIndices::OBSERVED;
    // GCI6 first half.
    let dir =
        MountDirectory::from_properties(&f.control("MountDirectory"), &f.reg).expect("directory");
    assert_eq!(
        (dir.tall.len(), dir.short.len(), dir.other),
        (347, 329, 0),
        "GC12"
    );
    assert!(dir.is_disjoint(), "GCI6: disjoint");
    let all: BTreeSet<Did> = dir.skills().collect();
    assert_eq!(all.len(), 676, "GC12: distinct");
    for d in &all {
        assert_eq!(f.class_of(*d), idx.skill, "GCI6: {d} is a skill");
    }
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
    assert_eq!(c.trees.len(), 3);
    assert_eq!(c.tree_skills.len(), 3);
    let trees: BTreeSet<u32> = c.trees.iter().filter_map(|t| t.tree).collect();
    let skill_trees: BTreeSet<u32> = c.tree_skills.iter().filter_map(|t| t.tree).collect();
    eprintln!(
        "mount trees: {} keyed tree entries, {} keyed skill-hash entries",
        trees.len(),
        skill_trees.len()
    );
    assert_eq!(trees.len(), 3, "§5.3.1: one tree entry per trait tree");
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
    assert_eq!(skills.len(), 24, "§5.3.1: 24 entries");
    assert_eq!(
        skills.iter().collect::<BTreeSet<_>>().len(),
        12,
        "§5.3.1: 12 skills"
    );
    assert_eq!(c.flanks.len(), 10);
    assert_eq!(c.dye_slots.len(), 5);
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
}

#[test]
fn account_views() {
    let Some(f) = fixture() else { return };
    let Some(s) = sweep() else { return };
    // GCI8.
    let pc = PerkControl::from_properties(&f.control("PerkControl"), &f.reg).expect("perk control");
    assert_eq!(pc.channels.len(), 28);
    assert!(pc.keys_are_distinct(), "GCI8");
    assert_eq!(pc.available.len(), 2);
    let perk_class = f.class_of(*pc.available.first().expect("perk"));
    let perks = s.of_class(perk_class);
    assert_eq!(perks.len(), 47, "GC19: 47 perks");
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
    assert_eq!(cc.collections.len(), 29);
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
    assert_eq!((n, pieces.len()), (131, 131), "§6.2 pieces");
    // GCI9.
    let pi = PaperItemServices::from_properties(&f.control("PaperItemControl"), &f.reg)
        .expect("services");
    assert_eq!(pi.services.len(), 35);
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
    assert_eq!(rc.categories.len(), 2);
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
}

#[test]
fn ui_and_social_views() {
    let Some(f) = fixture() else { return };
    let Some(s) = sweep() else { return };
    // GCI10 control half.
    let vdo = VitalDisplayOverrideControl::from_properties(
        &f.control("gmVitalDisplayOverrideControl"),
        &f.reg,
    )
    .expect("control");
    assert_eq!(vdo.overrides.len(), 113);
    let class = f.class_of(*vdo.overrides.first().expect("one"));
    assert_eq!(
        s.of_class(class),
        vdo.overrides.iter().copied().collect(),
        "GCI10: every override listed once"
    );
    assert_eq!(vdo.overrides.iter().collect::<BTreeSet<_>>().len(), 113);
    // GCI11.
    let a = AlertsControl::from_properties(&f.control("AlertsControl"), &f.reg).expect("alerts");
    assert_eq!(a.max_big_alerts, Some(5));
    assert_eq!(a.overflow.len(), 64);
    assert!(a.types_are_distinct_and_non_zero(), "GCI11");
    assert_eq!(
        f.enum_non_zero_keys("Alert_Type"),
        Some(64),
        "GC24: every non-zero key"
    );
    assert_eq!(
        a.overflow.iter().filter(|(_, o)| *o == Some(true)).count(),
        59
    );
    // §7.4.
    let r = ResetScheduleControl::from_properties(&f.control("ResetScheduleControl"), &f.reg)
        .expect("reset");
    assert_eq!(r.schedules.len(), 3);
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
    assert_eq!(cc.rank_to_ui_state.len(), 9);
    assert_eq!(cc.combat_tables.len(), 6);
    let types: BTreeSet<u32> = cc.combat_tables.iter().filter_map(|(t, _)| *t).collect();
    assert_eq!(types.len(), 6, "GCI12: distinct consider types");
    let mut combat = BTreeSet::new();
    for (_, list) in &cc.combat_tables {
        assert_eq!(list.len(), 120, "GCI12");
        combat.extend(list.iter().copied());
    }
    assert_eq!(cc.quest_tables.len(), 120, "GCI12");
    let quest: BTreeSet<Did> = cc.quest_tables.iter().copied().collect();
    assert_eq!((combat.len(), quest.len()), (6, 2), "GC26: 6 + 2 tables");
    for &d in combat.iter().chain(&quest) {
        let t = ConsiderationTable::from_properties(&f.props(d), &f.reg).expect("table");
        assert!(t.is_well_formed(even), "GCI12 {d}");
    }
    // GCI13.
    let t = TrackingStateControl::from_properties(&f.control("TrackingStateControl"), &f.reg)
        .expect("tracking");
    assert_eq!(t.entries.len(), 17);
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
    assert_eq!(g.ranks.len(), 4);
    assert_eq!(
        g.player_note_max_length, None,
        "§8.1: absent in the install"
    );
    assert_eq!(g.player_note_limit(), 48, "§8.1: the client default");
}
