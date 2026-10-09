// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data checks of the quest-family views (`tessera_formats::gameplay::quests`) against
//! a user's own install, read-only (`docs/specs/quests.md`).
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Release builds (or
//! `TESSERA_FULL=1`) walk every object record's property set, classify it (§2.1) and check
//! every record of the family; debug builds take a stride sample. Run with
//! `cargo test --release -p tessera-formats --test gameplay_quests_real_data -- --nocapture`.
//!
//! Always asserted (invariants, §19): every property set decodes; V1, V2, V6, V8, V9, V10,
//! V12, V13, V14; the numbering rule of §5.7 against the compiled quest objects (V3, V4,
//! V5, listen lists of §6.2, required counts of §6.1); the reward-level control (V7); the
//! NPC-side role-list join (§7.4) and the map-note role names (§17.1).
//!
//! The spec's census counts are observations of one install. They are printed next to
//! the counts found; `TESSERA_SPEC_CENSUS=1` (with a full walk) also asserts that they
//! match exactly.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use tessera_formats::gameplay::quests::{
    ColourTextList, EmoteDirectory, EventOrigin, NpcComment, QUEST_MAP_NOTES_DID, QuestArc,
    QuestArcDirectory, QuestControl, QuestFamilyKind, QuestItemDirectory, QuestItemRegionDirectory,
    QuestLabels, QuestMapNotes, QuestRecord, QuestSingletons, QuestTreasure, RewardLevelTable,
    RewardList, RoleAction, Scene, WorldEventList, active_quest_roles, quest_item_directory_class,
};
use tessera_formats::{
    Did, PropertyDictionary, PropertySet, PropertySetRecord, PropertyValue, StringSource,
    object_property_set_did,
};
use tessera_wstate::{Native, ObjectState, ObjectStore, Ref, object_dids};

fn game_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("TESSERA_GAME_DIR")?);
    dir.is_dir().then_some(dir)
}

fn full() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

fn strict_census() -> bool {
    std::env::var_os("TESSERA_SPEC_CENSUS").is_some_and(|v| v == "1")
}

// Attribute name hashes of the compiled quest object (§6.1, §6.2).
const EVENT_TABLE: u32 = 0x0195_52D8;
const OBJECTIVE_INDEX: u32 = 0x0E73_DC88;
const REQUIRED_COUNTS: u32 = 0x0DE1_6EF4;
const EVENT_PROPERTIES: u32 = 0x058F_0A6E;
const EVENT_ORDER: u32 = 0x06EB_4612;
const SIBLINGS: u32 = 0x0B57_13A3;
const LISTEN_LIST: u32 = 0x0C18_ECB4;

/// Census rows: (what, found, spec).
#[derive(Default)]
struct Census {
    rows: Vec<(String, u64, u64)>,
}

impl Census {
    fn row(&mut self, what: impl Into<String>, found: usize, spec: u64) {
        self.rows.push((what.into(), found as u64, spec));
    }

    fn report(&self, full: bool) {
        let mut mismatches = 0;
        eprintln!(
            "{:<58} {:>10} {:>10}",
            "census (quests.md)", "found", "spec"
        );
        for (what, found, spec) in &self.rows {
            let mark = if found == spec { "" } else { "  <- differs" };
            mismatches += usize::from(found != spec);
            eprintln!("{what:<58} {found:>10} {spec:>10}{mark}");
        }
        eprintln!("{mismatches} of {} census rows differ", self.rows.len());
        if full && strict_census() {
            assert_eq!(mismatches, 0, "census differs from the spec");
        }
    }
}

/// Every StringInfo of a set is a table reference without arguments (V8).
fn string_infos_plain(set: &PropertySet) -> (usize, usize) {
    let (mut all, mut bad) = (0usize, 0usize);
    set.walk(|v| {
        if let PropertyValue::StringInfo(s) = &v.entry.value {
            all += 1;
            if !matches!(s.source, StringSource::Table { .. }) || s.args.is_some() {
                bad += 1;
            }
        }
    });
    (all, bad)
}

fn int_of(set: &PropertySet, pid: Option<u32>) -> Option<i64> {
    match set.get(pid?)? {
        PropertyValue::Int(v) => Some(i64::from(*v)),
        PropertyValue::Enum(v) => Some(i64::from(*v)),
        _ => None,
    }
}

fn bool_of(set: &PropertySet, pid: Option<u32>) -> bool {
    pid.and_then(|p| set.get(p)) == Some(&PropertyValue::Bool(true))
}

#[derive(Default)]
struct QuestStats {
    quests: usize,
    deeds: usize,
    stored_false_deed: usize,
    objectives: usize,
    objectives_out_of_order: usize,
    bad_objective_indices: usize,
    events: usize,
    by_origin: BTreeMap<&'static str, usize>,
    compounds: usize,
    counts: usize,
    roles: usize,
    roles_by_action: BTreeMap<u32, usize>,
    role_rule_violations: usize,
    deed_with_category: usize,
    compiled_checked: usize,
    compiled_mismatches: Vec<String>,
    /// (event type, x) of the type-19 and type-29 listen lists (§6.2).
    listen_x: BTreeMap<(u32, u32), usize>,
}

/// Compares the numbering of one quest with its compiled object (§6.1, §6.2; V3–V5).
fn check_compiled(
    dict: &PropertyDictionary,
    quest: &QuestRecord<'_>,
    state: &ObjectState,
    stats: &mut QuestStats,
) -> Result<(), String> {
    let numbering = quest.number_events().map_err(|e| e.to_string())?;
    let root = state.root_object().ok_or("no root object")?;
    let table = root
        .follow(EVENT_TABLE)
        .and_then(|r| r.item())
        .and_then(|i| i.as_native())
        .ok_or("no event table")?;
    let Native::RefMapU32(_, entries) = table else {
        return Err("event table is not a refmap".into());
    };
    // V3.
    if entries.len() != numbering.len() {
        return Err(format!(
            "V3: {} compiled events, {} numbered",
            entries.len(),
            numbering.len()
        ));
    }
    let pid = |l: &str| dict.pid(l);
    let (p_event, p_obj, p_cond, p_id, p_fail, p_never) = (
        pid("QuestEvent_EventID"),
        pid("QuestEvent_ObjectiveID"),
        pid("QuestEvent_ConditionID"),
        pid("QuestEvent_ID"),
        pid("QuestEvent_IsFailureEvent"),
        pid("Quest_Condition_NeverFinish"),
    );
    let mut keys = BTreeSet::new();
    let mut event_ids_of: HashMap<Ref, i64> = HashMap::new();
    for &(key, r) in entries {
        keys.insert(key);
        let ev = state
            .resolve(r)
            .item()
            .and_then(|i| i.as_script())
            .ok_or_else(|| format!("event {key}: not a script object"))?;
        let Some(Native::PropertySet(ps)) = ev
            .follow(EVENT_PROPERTIES)
            .and_then(|r| r.item())
            .and_then(|i| i.as_native())
        else {
            return Err(format!("event {key}: no property set"));
        };
        let n = numbering
            .get(key)
            .ok_or_else(|| format!("V3: key {key} outside 1..N"))?;
        let expect = (
            Some(i64::from(n.event_id)),
            Some(i64::from(n.objective_id)),
            Some(i64::from(n.condition_id)),
            n.entry.event_type().map(i64::from),
            n.is_failure(),
            n.never_finish,
        );
        let found = (
            int_of(ps, p_event),
            int_of(ps, p_obj),
            int_of(ps, p_cond),
            int_of(ps, p_id),
            bool_of(ps, p_fail),
            bool_of(ps, p_never),
        );
        if expect != found {
            return Err(format!(
                "V4: event {key}: expected {expect:?}, found {found:?}"
            ));
        }
        if let Some(id) = found.0 {
            event_ids_of.insert(r, id);
        }
        // Order value.
        let order = ev.int(EVENT_ORDER).map(|v| v as i32);
        if order != Some(n.entry.compiled_order()) {
            return Err(format!("event {key}: order {order:?}"));
        }
        // V5.
        let siblings = match ev
            .follow(SIBLINGS)
            .and_then(|r| r.item())
            .and_then(|i| i.as_native())
        {
            Some(Native::ArrayU32(v)) => v.clone(),
            _ => Vec::new(),
        };
        if siblings != numbering.siblings(key) {
            return Err(format!(
                "V5: event {key}: siblings {siblings:?}, expected {:?}",
                numbering.siblings(key)
            ));
        }
        // Listen list (§6.2): the whole list where the rule is known; for types 19 and 29
        // the fixed parts around the element x (1 or 11, rule OPEN).
        let listen = match ev
            .follow(LISTEN_LIST)
            .and_then(|r| r.item())
            .and_then(|i| i.as_native())
        {
            Some(Native::ArrayU32(v)) => v.clone(),
            _ => Vec::new(),
        };
        if let Some(expected) = n.entry.listen_list() {
            if listen != expected {
                return Err(format!(
                    "event {key}: listen list {listen:?}, expected {expected:?}"
                ));
            }
        } else if let Some(t @ (19 | 29)) = n.entry.event_type() {
            let mut head = vec![t];
            if n.entry.waits_for_named_drama() {
                head.extend([17, 12]);
            }
            let tail: &[u32] = if t == 19 {
                &[23, 22, 28, 8]
            } else {
                &[22, 28, 23, 8]
            };
            let x = listen.get(head.len()).copied();
            let fits = listen.len() == head.len() + 1 + tail.len()
                && listen.starts_with(&head)
                && listen.ends_with(tail)
                && matches!(x, Some(1 | 11));
            if !fits {
                return Err(format!(
                    "event {key}: type-{t} listen list {listen:?} does not fit {head:?}, x, \
                     {tail:?}"
                ));
            }
            if let Some(x) = x {
                *stats.listen_x.entry((t, x)).or_default() += 1;
            }
        }
    }
    let n = u32::try_from(numbering.len()).unwrap_or(u32::MAX);
    if !keys.iter().copied().eq(1..=n) {
        return Err("V3: EventIDs are not 1..N".into());
    }
    // Objective index (§6.1): key = ObjectiveID → the event objects of that objective,
    // compared by the EventIDs of the objects referenced.
    if let Some(Native::RefMultiMapU32(_, idx)) = root
        .follow(OBJECTIVE_INDEX)
        .and_then(|r| r.item())
        .and_then(|i| i.as_native())
    {
        let mut found: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
        for (k, r) in idx {
            let id = event_ids_of.get(r).copied().ok_or_else(|| {
                format!("objective index key {k}: reference outside the event table")
            })?;
            found.entry(i64::from(*k as i32)).or_default().push(id);
        }
        for ids in found.values_mut() {
            ids.sort_unstable();
        }
        let expected: BTreeMap<i64, Vec<i64>> = numbering
            .objective_index()
            .into_iter()
            .map(|(k, v)| (i64::from(k), v.into_iter().map(i64::from).collect()))
            .collect();
        if found != expected {
            return Err(format!("objective index {found:?}, expected {expected:?}"));
        }
    } else {
        return Err("no objective index".into());
    }
    // Required counts (§6.1).
    let mut found: Vec<(u32, u32)> = match root
        .follow(REQUIRED_COUNTS)
        .and_then(|r| r.item())
        .and_then(|i| i.as_native())
    {
        Some(Native::MapU32(_, v)) => v.clone(),
        _ => Vec::new(),
    };
    let mut expected: Vec<(u32, u32)> = numbering
        .required_counts
        .iter()
        .map(|&(k, c)| (k, c as u32))
        .collect();
    found.sort_unstable();
    expected.sort_unstable();
    if found != expected {
        return Err(format!("required counts {found:?}, expected {expected:?}"));
    }
    Ok(())
}

#[test]
fn quest_family_census_and_invariants() {
    let Some(dir) = game_dir() else {
        eprintln!("TESSERA_GAME_DIR not set; skipping");
        return;
    };
    let store = ObjectStore::open(&dir).expect("open install");
    let data = store.game_data();
    let dict: Arc<PropertyDictionary> =
        Arc::new(data.registry().expect("dictionary").dictionary().clone());
    let labels = QuestLabels::from_dictionary(&dict);
    let missing = labels.missing();
    eprintln!("labels not in the dictionary: {missing:?}");
    for must in [
        "Quest_Name",
        "Quest_ObjectiveArray",
        "Quest_RoleArray",
        "QuestEvent_Entry",
        "QuestEvent_ID",
        "Quest_ObjectiveIndex",
        "QuestDispenser_Action",
    ] {
        assert!(!missing.contains(&must), "{must} must resolve");
    }

    // §2.2, §2.3: the singletons through their WEENIECONTENT labels.
    let singletons = QuestSingletons::resolve(|label| store.weenie(label).expect("weenie map"));
    for kind in QuestFamilyKind::ALL {
        if let Some(label) = kind.weenie_content_label() {
            let did = store.weenie(label).expect("weenie map");
            assert!(did.is_some(), "§2.3: WEENIECONTENT label {label}");
            assert_eq!(
                did.and_then(|d| singletons.kind_of(d)),
                Some(kind),
                "{label}"
            );
        }
    }

    let full = full();
    let dids = object_dids(data.source()).expect("object DIDs");
    let stride = if full { 1 } else { 41 };
    let mut census = Census::default();
    let mut kinds: BTreeMap<QuestFamilyKind, Vec<Did>> = BTreeMap::new();
    let mut kept: HashMap<Did, PropertySet> = HashMap::new();
    let mut q = QuestStats::default();
    let mut quest_roles: HashMap<Did, Vec<(String, Option<Did>)>> = HashMap::new();
    let mut npc_side: Vec<(Did, Option<Did>, Option<String>)> = Vec::new();
    // §7.4 (S5): records holding `QuestDispenser_ActiveQuests`, empty lists included.
    let mut role_list_holders = 0usize;
    let (mut sets, mut strings, mut bad_strings) = (0usize, 0usize, 0usize);

    for &did in dids.iter().step_by(stride) {
        let Some(set_did) = object_property_set_did(did) else {
            continue;
        };
        let Some(payload) = data.read(set_did).expect("read") else {
            continue;
        };
        let set = PropertySetRecord::parse_expecting(&payload, set_did, &*dict)
            .unwrap_or_else(|e| panic!("{set_did}: {e}"))
            .set;
        sets += 1;
        role_list_holders += usize::from(
            labels
                .role
                .active_quests
                .id()
                .is_some_and(|p| set.get(p).is_some()),
        );
        for r in active_quest_roles(&labels, &set) {
            npc_side.push((did, r.quest, r.role_name.map(str::to_owned)));
        }
        let Some(kind) = labels.classify_record(did, &set, &singletons) else {
            continue;
        };
        kinds.entry(kind).or_default().push(did);
        let (s, b) = string_infos_plain(&set);
        strings += s;
        bad_strings += b;
        match kind {
            QuestFamilyKind::Quest => {
                let quest = QuestRecord::new(&labels, &set).expect("V1");
                check_quest(&quest, &mut q, &mut quest_roles, did);
                let payload = data.read(did).expect("read").expect("0x70 exists");
                let state = store.decode(did, &payload).expect("compiled quest");
                q.compiled_checked += 1;
                if let Err(e) = check_compiled(&dict, &quest, &state, &mut q) {
                    q.compiled_mismatches.push(format!("{did}: {e}"));
                }
            }
            QuestFamilyKind::Faction
            | QuestFamilyKind::WorldEventBool
            | QuestFamilyKind::WorldEventInt
            | QuestFamilyKind::Title
            | QuestFamilyKind::Emote => {}
            _ => {
                kept.insert(did, set);
            }
        }
    }
    assert!(sets > 0);
    let count = |k: QuestFamilyKind| kinds.get(&k).map_or(0, Vec::len);

    // --- §2.1 kinds.
    for (k, spec) in [
        (QuestFamilyKind::Quest, 20_807),
        (QuestFamilyKind::QuestTreasure, 9_479),
        (QuestFamilyKind::RewardLevelTable, 160),
        (QuestFamilyKind::QuestArc, 344),
        (QuestFamilyKind::QuestArcDirectory, 1),
        (QuestFamilyKind::AccomplishmentDirectory, 29),
        (QuestFamilyKind::AccomplishmentVersionList, 1),
        (QuestFamilyKind::QuestItemDirectory, 20),
        (QuestFamilyKind::QuestItemRegionDirectory, 1),
        (QuestFamilyKind::Title, 3_091),
        (QuestFamilyKind::Emote, 273),
        (QuestFamilyKind::EmoteDirectory, 1),
        (QuestFamilyKind::Faction, 114),
        (QuestFamilyKind::ReputationAccountModifier, 69),
        (QuestFamilyKind::ReputationControl, 1),
        (QuestFamilyKind::WorldEventBool, 1_574),
        (QuestFamilyKind::WorldEventInt, 699),
        (QuestFamilyKind::WorldEventList, 43),
        (QuestFamilyKind::NpcComment, 859),
        (QuestFamilyKind::NpcCommentList, 223),
        (QuestFamilyKind::ColourTextList, 1_206),
        (QuestFamilyKind::Scene, 1_172),
        (QuestFamilyKind::QuestControl, 1),
        (QuestFamilyKind::AccomplishmentControl, 1),
        (QuestFamilyKind::ColourTextControl, 1),
    ] {
        census.row(format!("kind {k:?}"), count(k), spec);
    }

    // --- §4, §5, §7 quests.
    census.row("quests (not deeds)", q.quests, 15_400);
    census.row("deeds", q.deeds, 5_407);
    census.row("objectives", q.objectives, 50_784);
    census.row(
        "records with objectives out of order",
        q.objectives_out_of_order,
        2_465,
    );
    census.row("event entries (numbered events)", q.events, 82_280);
    for (origin, spec) in [
        ("ObjectiveCompletion", 78_690),
        ("ObjectiveFailure", 1_250),
        ("QuestFailure", 1_997),
        ("Tracked", 343),
    ] {
        census.row(
            format!("events from {origin}"),
            q.by_origin.get(origin).copied().unwrap_or(0),
            spec,
        );
    }
    census.row("compound events", q.compounds, 6_154);
    census.row("conditions with a required count", q.counts, 196);
    census.row("roles in Quest_RoleArray", q.roles, 52_343);
    for (action, spec) in [
        (1, 10_258),
        (2, 24_354),
        (3, 904),
        (4, 221),
        (5, 959),
        (6, 14_929),
        (7, 192),
        (9, 111),
        (10, 5),
        (13, 27),
        (14, 383),
    ] {
        census.row(
            format!("roles with action {action}"),
            q.roles_by_action.get(&action).copied().unwrap_or(0),
            spec,
        );
    }
    eprintln!("StringInfos in the family's sets: {strings}");
    assert_eq!(
        q.stored_false_deed, 0,
        "V1: Quest_IsAccomplishment stored false"
    );
    assert_eq!(q.bad_objective_indices, 0, "V2");
    assert_eq!(q.role_rule_violations, 0, "V6");
    assert_eq!(q.deed_with_category, 0, "§4.1: no deed has Quest_Category");
    assert_eq!(bad_strings, 0, "V8: StringInfos with literals or arguments");
    assert!(
        q.compiled_mismatches.is_empty(),
        "§5.7 / V3–V5: {} of {} compiled quests differ, first: {:?}",
        q.compiled_mismatches.len(),
        q.compiled_checked,
        q.compiled_mismatches.first()
    );

    eprintln!(
        "§6.2 listen lists of types 19 and 29, (type, x) → events: {:?}",
        q.listen_x
    );

    // --- §12.2 the emote directory lists all emotes: property-identified emotes that it
    // does not list (§2.1 OPEN) are counted separately.
    let emote_directory: HashSet<Did> = kinds
        .get(&QuestFamilyKind::EmoteDirectory)
        .into_iter()
        .flatten()
        .flat_map(|d| EmoteDirectory::read(&labels, &kept[d]).emotes)
        .collect();
    let emotes = kinds
        .get(&QuestFamilyKind::Emote)
        .cloned()
        .unwrap_or_default();
    let listed = emotes
        .iter()
        .filter(|d| emote_directory.contains(d))
        .count();
    census.row("emotes listed in the emote directory", listed, 273);
    let mut unlisted_classes: BTreeMap<u32, usize> = BTreeMap::new();
    for d in emotes.iter().filter(|d| !emote_directory.contains(d)) {
        let class = store
            .header(*d)
            .expect("header")
            .map_or(0, |h| h.root_class);
        *unlisted_classes.entry(class).or_default() += 1;
    }
    eprintln!(
        "§2.1 emote-like sets the directory does not list, by root class: {unlisted_classes:?}"
    );

    // --- §7.4 NPC-side role lists of every object record (S5), quest records included: an
    // entry matches when its quest has a role with the same role name and the holder as NPC.
    let quest_dids: HashSet<Did> = kinds
        .get(&QuestFamilyKind::Quest)
        .into_iter()
        .flatten()
        .copied()
        .collect();
    let (mut npc_entries, mut matched) = (0usize, 0usize);
    let (mut quest_held, mut quest_held_matched) = (0usize, 0usize);
    // Root class of each holder → (entries, matching entries, holders), to place the
    // census of §7.4.
    let mut by_class: BTreeMap<u32, (usize, usize, BTreeSet<Did>)> = BTreeMap::new();
    for (holder, quest, name) in &npc_side {
        npc_entries += 1;
        let hit = quest
            .and_then(|d| quest_roles.get(&d))
            .is_some_and(|roles| {
                roles
                    .iter()
                    .any(|(n, npc)| Some(n.as_str()) == name.as_deref() && *npc == Some(*holder))
            });
        matched += usize::from(hit);
        if quest_dids.contains(holder) {
            quest_held += 1;
            quest_held_matched += usize::from(hit);
        }
        if full {
            let class = store
                .header(*holder)
                .expect("header")
                .map_or(0, |h| h.root_class);
            let slot = by_class.entry(class).or_default();
            slot.0 += 1;
            slot.1 += usize::from(hit);
            slot.2.insert(*holder);
        }
    }
    census.row(
        "NPC-side role-list entries, all holders",
        npc_entries,
        43_413,
    );
    if full {
        let mut rows: Vec<(u32, usize, usize, usize)> = by_class
            .iter()
            .map(|(c, (n, m, h))| (*c, *n, *m, h.len()))
            .collect();
        rows.sort_by_key(|&(c, n, _, _)| (std::cmp::Reverse(n), c));
        eprintln!(
            "§7.4 NPC-side entries by holder root class (class, entries, matching, holders): \
             {rows:?}"
        );
        census.row("NPC-side entries matching name and NPC", matched, 43_402);
        census.row(
            "holders of QuestDispenser_ActiveQuests",
            role_list_holders,
            10_845,
        );
        census.row("entries held by quest records", quest_held, 6);
        census.row(
            "entries held by quest records that match",
            quest_held_matched,
            0,
        );
        // §7.4: all but a handful match.
        assert!(
            matched * 1000 >= npc_entries * 999,
            "§7.4: {matched} of {npc_entries} entries match a role"
        );
    }

    // --- §15 quest-item directories: every region-directory target is a directory or the
    // one empty set (which no property identifies).
    for did in kinds
        .get(&QuestFamilyKind::QuestItemRegionDirectory)
        .into_iter()
        .flatten()
    {
        let region = QuestItemRegionDirectory::read(&labels, &kept[did]);
        census.row(
            "quest-item region directory entries",
            region.entries.len(),
            22,
        );
        if full {
            let mut empty = BTreeSet::new();
            for (_, dir) in &region.entries {
                let Some(dir) = dir else { continue };
                if kept.contains_key(dir) {
                    let d = QuestItemDirectory::read(&labels, &kept[dir]);
                    assert!(!d.entries.is_empty(), "§15: {dir}");
                } else {
                    let set_did = object_property_set_did(*dir).expect("object DID");
                    let set = data
                        .read(set_did)
                        .expect("read")
                        .map(|p| PropertySetRecord::parse(&p, &*dict).expect("set").set)
                        .unwrap_or_default();
                    assert!(
                        set.is_empty(),
                        "§15: {dir} is neither a directory nor empty"
                    );
                    empty.insert(*dir);
                }
            }
            census.row(
                "quest-item directories (with the empty one)",
                count(QuestFamilyKind::QuestItemDirectory) + empty.len(),
                21,
            );
            // §2.1 (S1): the directories are the records of one root class, the empty one
            // included.
            let class_of = |d: Did| store.header(d).expect("header").map(|h| h.root_class);
            let carriers: Vec<(u32, &PropertySet)> = kinds
                .get(&QuestFamilyKind::QuestItemDirectory)
                .into_iter()
                .flatten()
                .filter_map(|d| Some((class_of(*d)?, &kept[d])))
                .collect();
            let dir_class = quest_item_directory_class(&labels, carriers.iter().copied())
                .expect("§2.1: one root class of quest-item directories");
            for d in &empty {
                assert_eq!(class_of(*d), Some(dir_class), "§2.1: {d}");
            }
        }
    }

    // --- §9.4 treasure, V9.
    let mut treasure_items = 0usize;
    for did in kinds
        .get(&QuestFamilyKind::QuestTreasure)
        .into_iter()
        .flatten()
    {
        let t = QuestTreasure::new(&labels, &kept[did]);
        for item in t.fixed_items().into_iter().chain(t.selectable_items()) {
            treasure_items += 1;
            assert!(item.quantity.is_some(), "V9: {did}");
        }
    }
    census.row("treasure item structs", treasure_items, 14_537);

    // --- §9.2 reward-level tables, V7.
    if let Some(control_did) = store.weenie("QuestControl").expect("weenie map") {
        let set_did = object_property_set_did(control_did).expect("object DID");
        let payload = data
            .read(set_did)
            .expect("read")
            .expect("quest control set");
        let set = PropertySetRecord::parse_expecting(&payload, set_did, &*dict)
            .expect("quest control set")
            .set;
        let control = QuestControl::new(&labels, &set);
        let tables = control.reward_level_tables();
        census.row(
            "QuestControl_RewardLevelDIDArray elements",
            tables.len(),
            160,
        );
        let distinct: HashSet<Did> = tables.iter().copied().collect();
        assert_eq!(distinct.len(), tables.len(), "V7: distinct elements");
        if full {
            let reward_tables: HashSet<Did> = kinds
                .get(&QuestFamilyKind::RewardLevelTable)
                .into_iter()
                .flatten()
                .copied()
                .collect();
            assert_eq!(
                distinct, reward_tables,
                "V7: one element per reward-level record"
            );
            let mut last = f64::MIN;
            for did in &tables {
                let table = RewardLevelTable::new(&labels, &kept[did]);
                let amount = table.amount(RewardList::Exp, 4).expect("tier-4 experience");
                assert!(amount >= last, "V7: tier-4 experience decreases at {did}");
                last = amount;
            }
        }
        let thresholds = control.medal_thresholds();
        eprintln!("medal thresholds: {} entries", thresholds.len());
    }

    // --- §11 arcs, V12.
    let mut arc_quests: HashSet<Did> = HashSet::new();
    let mut arc_total = 0usize;
    for did in kinds.get(&QuestFamilyKind::QuestArc).into_iter().flatten() {
        for quest in QuestArc::new(&labels, &kept[did]).quests() {
            arc_total += 1;
            assert!(arc_quests.insert(quest), "V12: {quest} in two arcs");
        }
    }
    census.row("arc quest entries", arc_total, 1_589);
    for did in kinds
        .get(&QuestFamilyKind::QuestArcDirectory)
        .into_iter()
        .flatten()
    {
        let arcs = QuestArcDirectory::new(&labels, &kept[did]).arcs();
        census.row("arcs in the arc directory", arcs.len(), 344);
    }

    // --- §7.7 NPC comments, V13.
    for did in kinds
        .get(&QuestFamilyKind::NpcComment)
        .into_iter()
        .flatten()
    {
        let c = NpcComment::new(&labels, &kept[did]);
        let p = c.permissions().expect("V13: permission struct");
        let shape = (
            p.requirements().len(),
            p.or_requirements().len(),
            p.world_event_all().len(),
            p.world_event_any().len(),
            p.required_races().len(),
            p.required_classes().len(),
        );
        assert_eq!(shape, (2, 5, 3, 3, 3, 4), "V13: {did}");
    }

    // --- §7.8 colour text.
    let mut colour_lines = 0usize;
    for did in kinds
        .get(&QuestFamilyKind::ColourTextList)
        .into_iter()
        .flatten()
    {
        let list = ColourTextList::read(&labels, &kept[did]);
        assert!(!list.lines.is_empty() || !list.reputation_lines.is_empty());
        colour_lines += list.lines.len();
    }
    eprintln!("colour-text lines: {colour_lines}");

    // --- §7.9 scenes, V14.
    let mut keyframes = 0usize;
    for did in kinds.get(&QuestFamilyKind::Scene).into_iter().flatten() {
        let scene = Scene::read(&labels, &kept[did]);
        keyframes += scene.keyframes.len();
        let state = store.state(*did).expect("scene state").expect("exists");
        let native = (0..state.bundle.items().len())
            .filter_map(|i| state.item(i))
            .find_map(|i| match i.as_native() {
                Some(Native::SceneInfo(s)) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("V14: {did} has no scene native"));
        assert_eq!(native.entries.len(), scene.keyframes.len(), "V14: {did}");
        assert_eq!(native.owner, *did, "V14: {did}");
        assert_eq!(Some(&native.title), scene.name, "V14: {did}");
        for (entry, key) in native.entries.iter().zip(&scene.keyframes) {
            assert_eq!(Some(&entry.text), key.subtitle, "V14: {did}");
            assert_eq!(
                entry.flag,
                key.show_random_image == Some(true),
                "V14: {did}"
            );
        }
        store.evict([*did]);
    }
    census.row("scene keyframes", keyframes, 7_027);

    // --- §14.3 world-event lists.
    let mut listed = 0usize;
    for did in kinds
        .get(&QuestFamilyKind::WorldEventList)
        .into_iter()
        .flatten()
    {
        let list = WorldEventList::read(&labels, &kept[did]);
        listed += list.events.len();
    }
    eprintln!("world events listed: {listed}");

    // --- §17 map notes, V10.
    let payload = data
        .read(QUEST_MAP_NOTES_DID)
        .expect("read")
        .expect("map-note record");
    let notes = QuestMapNotes::parse_expecting(&payload, QUEST_MAP_NOTES_DID, &*dict)
        .expect("V10: map notes decode to the last byte");
    census.row("map notes", notes.notes.len(), 5_638);
    let layers_pid = dict.pid("Entity_ContentLayers");
    let mut layer_counts: BTreeMap<usize, usize> = BTreeMap::new();
    let (mut with_key, mut with_quests, mut note_roles, mut role_hits, mut role_present) =
        (0, 0, 0, 0, 0);
    let mut nameless = 0usize;
    for note in &notes.notes {
        assert!(
            matches!(note.position.flags(), 0x1B | 0x3B | 0x5B | 0x7B),
            "§17.1 flags {:#x}",
            note.position.flags()
        );
        if let Some(layers) = &note.layers {
            assert_eq!(Some(layers.pid), layers_pid, "§17.1 layers PID");
        }
        *layer_counts.entry(note.content_layers().len()).or_default() += 1;
        with_key += usize::from(note.role_constraint(&labels).is_some());
        let roles = note.quest_roles(&labels);
        with_quests += usize::from(!roles.is_empty());
        for r in roles {
            note_roles += 1;
            let Some(name) = r.role_name else {
                nameless += 1;
                continue;
            };
            if let Some(quest) = r.quest.and_then(|d| quest_roles.get(&d)) {
                role_present += 1;
                role_hits += usize::from(quest.iter().any(|(n, _)| n == name));
            }
        }
    }
    census.row(
        "notes with no content layer",
        layer_counts.get(&0).copied().unwrap_or(0),
        3_099,
    );
    census.row(
        "notes with one content layer",
        layer_counts.get(&1).copied().unwrap_or(0),
        2_520,
    );
    census.row("notes with a role key", with_key, 3_363);
    census.row("notes with quest roles", with_quests, 2_453);
    census.row("quest-role structs in notes", note_roles, 16_124);
    eprintln!(
        "map-note quest roles: {role_hits} of {role_present} named roles of present quests \
         match a role name; {nameless} carry no role name"
    );
    if full {
        // §17.1: the role names in a note are role names of the quest's Quest_RoleArray.
        assert_eq!(role_hits, role_present, "§17.1: note role names");
    }

    eprintln!(
        "quests: {} sets walked (stride {stride}), {} family records, {} compiled quests \
         checked",
        sets,
        kinds.values().map(Vec::len).sum::<usize>(),
        q.compiled_checked
    );
    if full {
        census.report(full);
    } else {
        eprintln!("sampled walk: census not compared (run a release build or TESSERA_FULL=1)");
    }
}

fn check_quest(
    quest: &QuestRecord<'_>,
    q: &mut QuestStats,
    quest_roles: &mut HashMap<Did, Vec<(String, Option<Did>)>>,
    did: Did,
) {
    if quest.is_deed() {
        q.deeds += 1;
        q.deed_with_category += usize::from(quest.category().is_some());
    } else {
        q.quests += 1;
    }
    q.stored_false_deed += usize::from(quest.is_accomplishment_stored() == Some(false));

    // V2.
    let objectives = quest.objectives();
    q.objectives += objectives.len();
    let indices: Vec<i32> = objectives.iter().filter_map(|o| o.index()).collect();
    let mut sorted = indices.clone();
    sorted.sort_unstable();
    let n = i32::try_from(objectives.len()).unwrap_or(i32::MAX);
    if indices.len() != objectives.len() || !sorted.iter().copied().eq(1..=n) {
        q.bad_objective_indices += 1;
    }
    q.objectives_out_of_order += usize::from(sorted != indices);

    // §5.7.
    if let Ok(numbering) = quest.number_events() {
        q.events += numbering.len();
        for e in &numbering.events {
            let origin = match e.origin {
                EventOrigin::ObjectiveCompletion => "ObjectiveCompletion",
                EventOrigin::ObjectiveFailure => "ObjectiveFailure",
                EventOrigin::QuestFailure => "QuestFailure",
                EventOrigin::Tracked => "Tracked",
            };
            *q.by_origin.entry(origin).or_default() += 1;
        }
        q.compounds += numbering.compounds.len();
        q.counts += numbering.required_counts.len();
    }

    // §7.1, §7.2, V6.
    let roles = quest.roles();
    q.roles += roles.len();
    let mut names = Vec::with_capacity(roles.len());
    for role in &roles {
        if let Some(a) = role.action() {
            *q.roles_by_action.entry(a.code()).or_default() += 1;
            if matches!(a, RoleAction::Other(_)) {
                eprintln!("{did}: unknown role action {}", a.code());
            }
        }
        q.role_rule_violations += usize::from(!role.objective_rule_holds());
        if let Some(name) = role.role_name() {
            names.push((name.to_owned(), role.npc()));
        }
    }
    quest_roles.insert(did, names);
}
