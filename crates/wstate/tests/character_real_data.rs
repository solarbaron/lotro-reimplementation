// SPDX-License-Identifier: GPL-3.0-or-later
//! Census of the player-character records against a user's own install (read-only),
//! skipped unless `TESSERA_GAME_DIR` is set (`docs/specs/player-character.md` §2, §18).
//!
//! The records are found without class indices (§2.3): the controls through their
//! WEENIECONTENT labels, the rest by following references; the class of each found record
//! then names its family, and a header scan of every 0x70 record counts **all** records of
//! those classes. Every record of the families is decoded on both halves (PC1) and the
//! invariants PC2 (race-table pairs), PC3–PC16 are checked. The counts asserted are the
//! spec's observations of the install it was written against (§2.1).
//!
//! Heavy (reads every object record once): run with
//! `TESSERA_GAME_DIR=… cargo test --release -p tessera-wstate --test character_real_data
//! -- --nocapture`. Only counts and DIDs are printed; no game text leaves the install.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::{Did, StringInfo};
use tessera_props::{PropertyRegistry, PropertySet, Value};
use tessera_text::{RenderOptions, Strings, Values};
use tessera_wstate::character::{
    Allegiance, AllegianceControl, BankRepository, BankUpgradeSchedule, CONTROL_LABELS,
    CharacterRecords, ClassCreationInfo, ExperienceTable, NameGenerator, NameRuleControl,
    NationalityRecord, Pip, PipControl, RaceDirectory, RaceRecord, Role, ShiftTable, XpGlobals,
    allegiance_problems, mirror_base,
};
use tessera_wstate::{ObjectStore, object_dids};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

type Store = ObjectStore<Arc<ArchiveSet>>;

fn props(store: &Store, did: Did) -> PropertySet {
    store
        .properties(did)
        .unwrap_or_else(|e| panic!("{did}: property half: {e}"))
        .unwrap_or_else(|| panic!("{did}: no property half"))
}

fn text(strings: &Strings, info: &StringInfo) -> Option<String> {
    let r = strings.resolve(info, &Values::new(), &RenderOptions::default());
    r.is_clean().then(|| r.plain_text())
}

/// Root class of every 0x70 record, read with four threads.
fn scan_classes(store: &Store, dids: &[Did]) -> HashMap<Did, u32> {
    let chunk = dids.len().div_ceil(4).max(1);
    let mut out = HashMap::with_capacity(dids.len());
    std::thread::scope(|s| {
        let handles: Vec<_> = dids
            .chunks(chunk)
            .map(|part| {
                s.spawn(move || {
                    part.iter()
                        .filter_map(|&d| match store.header(d) {
                            Ok(Some(h)) => Some((d, h.root_class)),
                            Ok(None) => None,
                            Err(e) => panic!("{d}: header: {e}"),
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for h in handles {
            out.extend(h.join().expect("scan thread"));
        }
    });
    out
}

/// Names of the properties whose presence or value differs between two sets.
fn differing(a: &PropertySet, b: &PropertySet, reg: &PropertyRegistry) -> BTreeSet<String> {
    let mut pids: BTreeSet<u32> = a.iter().map(|p| p.pid).collect();
    pids.extend(b.iter().map(|p| p.pid));
    pids.into_iter()
        .filter(|&p| a.get(p) != b.get(p))
        .map(|p| reg.name(p).map_or_else(|| format!("{p:#x}"), str::to_owned))
        .collect()
}

/// The base of a name after removing every mirror affix.
fn root_name(name: &str) -> &str {
    let mut n = name;
    while let Some(b) = mirror_base(n) {
        n = b;
    }
    n
}

#[test]
fn player_character_census() {
    let Some(dir) = game_dir() else { return };
    let t0 = Instant::now();
    let set = Arc::new(ArchiveSet::open(&dir, SetOptions::default()).expect("open install"));
    let store: Store = ObjectStore::new(Arc::clone(&set));
    let reg = store.game_data().registry().expect("registry").clone();
    let strings = Strings::open_install(&dir, Some("English")).expect("language archive");

    // §2.2, §2.3: labels and references.
    let recs = CharacterRecords::collect(&store).expect("collect");
    assert_eq!(
        recs.controls.len(),
        CONTROL_LABELS.len(),
        "every §2.2 label present"
    );

    // Header scan: root class of every 0x70 record.
    let dids: Vec<Did> = object_dids(&set)
        .expect("object DIDs")
        .into_iter()
        .filter(|d| d.prefix() == 0x70)
        .collect();
    let classes = scan_classes(&store, &dids);
    let mut by_class: HashMap<u32, Vec<Did>> = HashMap::new();
    for (&d, &c) in &classes {
        by_class.entry(c).or_default().push(d);
    }
    eprintln!(
        "scanned {} object records in {:?}",
        classes.len(),
        t0.elapsed()
    );
    let class_of = |d: Did| {
        *classes
            .get(&d)
            .unwrap_or_else(|| panic!("{d} not an object record"))
    };
    let all_of = |c: u32| -> Vec<Did> {
        let mut v = by_class.get(&c).cloned().unwrap_or_default();
        v.sort_unstable();
        v
    };
    let role_class = |role: Role| -> BTreeSet<u32> { recs.with_role(role).map(class_of).collect() };

    // §2.1 families: one class per role (stat objects: seven classes).
    let mut family: BTreeMap<u32, &str> = BTreeMap::new();
    for (&label, &did) in &recs.controls {
        if label == "TraitControl" {
            continue; // combat-data.md §8.6, not a §2.1 class
        }
        let c = class_of(did);
        assert_eq!(
            all_of(c).len(),
            1,
            "{label}: control class {c} has one record"
        );
        family.insert(c, label);
    }
    let expected_roles: [(Role, &str, usize); 13] = [
        (Role::RaceTable, "race tables", 8),
        (Role::PlayerAvatar, "player avatar templates", 69),
        (Role::AdminAvatar, "administrator avatar templates", 69),
        (Role::NationalityTable, "nationality tables", 29),
        (Role::ExperienceTable, "experience tables", 29),
        (Role::AdvancedStartKit, "advanced-start kits", 9),
        (Role::Pip, "pips", 25),
        (Role::TraitConfiguration, "trait configurations", 5),
        (Role::Allegiance, "allegiances", 7),
        (Role::BankSchedule, "bank upgrade schedules", 3),
        (Role::KinshipTheme, "kinship themes", 9),
        (Role::GroupModifierTable, "group modifier tables", 3),
        (Role::StatObject, "stat objects", 7),
    ];
    for (role, what, want) in expected_roles {
        let cs = role_class(role);
        let n: usize = cs.iter().map(|&c| all_of(c).len()).sum();
        eprintln!("{what}: classes {cs:?}, {n} records");
        assert_eq!(n, want, "{what}: records of the family (§2.1)");
        if role == Role::StatObject {
            assert_eq!(cs.len(), 7, "seven stat classes");
        } else {
            assert_eq!(cs.len(), 1, "{what}: one class");
        }
        for c in cs {
            family.insert(c, what);
        }
    }
    let class_tables: Vec<Did> = recs.with_role(Role::ClassTable).collect();
    assert_eq!(class_tables.len(), 12, "class tables (§7)");

    // PC1: every record of the 52 classes decodes completely on both halves.
    assert_eq!(family.len(), 52, "§2.1: 33 + 19 classes");
    let (mut n, mut b70, mut b79) = (0usize, 0usize, 0usize);
    for &c in family.keys() {
        for d in all_of(c) {
            let state = store
                .state(d)
                .unwrap_or_else(|e| panic!("{d}: {e}"))
                .expect("present");
            assert!(
                state.is_complete(),
                "PC1: {d} (class {c}) decodes completely"
            );
            let _ = props(&store, d);
            b70 += store
                .game_data()
                .read(d)
                .expect("read")
                .map_or(0, |v| v.len());
            let sd = Did(d.0 + 0x0900_0000);
            b79 += store
                .game_data()
                .read(sd)
                .expect("read")
                .map_or(0, |v| v.len());
            n += 1;
            store.clear_cache();
        }
    }
    eprintln!("PC1: {n} records, {b70} bytes (0x70), {b79} bytes (0x79)");
    assert_eq!(n, 224 + 81, "§2.1 record total");
    assert_eq!(b70, 784_978 + 71_649, "§2.1 0x70 bytes");
    assert_eq!(b79, 1_560_674 + 36_773, "§2.1 0x79 bytes");

    // §5: race directory and tables.
    let dir_set = props(&store, recs.control("RaceTableDirectory").expect("label"));
    let race_dids = RaceDirectory::from_properties(&dir_set, &reg)
        .expect("directory")
        .tables;
    let races: Vec<RaceRecord> = race_dids
        .iter()
        .map(|&d| RaceRecord::from_properties(&props(&store, d), &reg).expect("race table"))
        .collect();
    let race_keys: BTreeSet<u32> = races.iter().map(|r| r.race).collect();
    assert_eq!(race_keys.len(), races.len(), "PC6: RaceTable_Race unique");
    let entries: Vec<(u32, &tessera_wstate::character::GenderEntry)> = races
        .iter()
        .flat_map(|r| r.genders.iter().map(move |g| (r.race, g)))
        .collect();
    assert_eq!(entries.len(), 15, "§5.3: 15 gender entries");
    let mut per_sex: BTreeMap<u32, usize> = BTreeMap::new();
    for (_, g) in &entries {
        *per_sex.entry(g.gender).or_default() += 1;
    }
    let mut sex_counts: Vec<usize> = per_sex.values().copied().collect();
    sex_counts.sort_unstable();
    assert_eq!(sex_counts, vec![7, 8], "§5.3: 8 and 7 entries per sex");
    assert_eq!(
        races.iter().map(|r| r.classes.len()).sum::<usize>(),
        69,
        "§5.2 class lists"
    );
    assert_eq!(
        races.iter().map(|r| r.nationalities.len()).sum::<usize>(),
        28,
        "§5.2 nationality lists"
    );
    assert_eq!(
        races.iter().map(|r| r.characteristics.len()).sum::<usize>(),
        66,
        "§5.2 characteristics"
    );

    // PC5 and PC2 (race-table pairs), PC4.
    let sex_pid = reg.pid("Sex").expect("Sex");
    let species_pid = reg.pid("Agent_Species").expect("Agent_Species");
    let pair_names: BTreeSet<&str> = [
        "WeenieType",
        "Name",
        "Inscription_Text",
        "Usage_Permission",
        "BaseRotTime",
        "GOP_ProfanityFilterEnabled",
        "Death_NeverSayDie",
        "CloakingAppearanceID",
        "Stance_CurrentStance",
        "Admin_PlayerTestMode",
        "Render_AdjustableScale",
        "Combat_PartialBlock_Mitigation_Mod",
        "Combat_PartialEvade_Mitigation_Mod",
        "Combat_PartialParry_Mitigation_Mod",
        "Physics_EtherealityType",
        "Physics_EtherealToType",
        "Entity_AdditionalScriptTables",
    ]
    .into_iter()
    .collect();
    let mut sex_matches = 0;
    let mut player_avatars: BTreeMap<Did, PropertySet> = BTreeMap::new();
    let mut admin_avatars = BTreeSet::new();
    for (race, g) in &entries {
        let pa = g.player_avatar.expect("player avatar");
        let aa = g.admin_avatar.expect("admin avatar");
        let ps = props(&store, pa);
        let as_ = props(&store, aa);
        for (who, s) in [("player", &ps), ("admin", &as_)] {
            assert_eq!(
                s.get(species_pid).and_then(Value::as_enum),
                Some(*race),
                "PC5: {who} avatar species"
            );
        }
        let sexes = (
            ps.get(sex_pid).and_then(Value::as_enum),
            as_.get(sex_pid).and_then(Value::as_enum),
        );
        if sexes == (Some(g.gender), Some(g.gender)) {
            sex_matches += 1;
        }
        for name in differing(&ps, &as_, &reg) {
            assert!(
                pair_names.contains(root_name(&name)),
                "PC2: {pa}/{aa} differ in {name}, not a §4.2 property"
            );
        }
        player_avatars.insert(pa, ps);
        admin_avatars.insert(aa);
    }
    assert_eq!(
        sex_matches, 14,
        "PC5: Sex equals the gender in 14 of 15 entries"
    );
    assert_eq!(player_avatars.len(), 14, "§5.3: 14 player avatars");
    assert_eq!(admin_avatars.len(), 14, "§5.3: 14 administrator avatars");
    let six: BTreeSet<&str> = [
        "Agent_Species",
        "Sex",
        "Agent_Genus",
        "Agent_Classification",
        "PhysObj",
        "Character_SpeciesNameOverride",
    ]
    .into_iter()
    .collect();
    let avatar_sets: Vec<&PropertySet> = player_avatars.values().collect();
    let mut varying = BTreeSet::new();
    for w in avatar_sets.windows(2) {
        if let [a, b] = w {
            varying.extend(differing(a, b, &reg));
        }
    }
    for name in &varying {
        assert!(
            six.contains(name.as_str()),
            "PC4: race avatars differ in {name}"
        );
    }
    let mut sizes: Vec<usize> = avatar_sets.iter().map(|s| s.len()).collect();
    sizes.sort_unstable();
    sizes.dedup();
    eprintln!(
        "PC4: race avatars vary in {} properties; sizes {sizes:?}",
        varying.len()
    );

    // PC3: mirrors in all 138 avatar templates.
    let mut mirror_names: Option<BTreeSet<u32>> = None;
    let (mut equal, mut no_base) = (0usize, 0usize);
    let avatar_classes: BTreeSet<u32> = role_class(Role::PlayerAvatar)
        .into_iter()
        .chain(role_class(Role::AdminAvatar))
        .collect();
    let mut avatar_count = 0;
    for &c in &avatar_classes {
        for d in all_of(c) {
            avatar_count += 1;
            let s = props(&store, d);
            let mut names = BTreeSet::new();
            for p in s.iter() {
                let Some(name) = reg.name(p.pid) else {
                    continue;
                };
                let Some(base) = mirror_base(name) else {
                    continue;
                };
                names.insert(p.pid);
                match reg.pid(base).and_then(|b| s.get(b)) {
                    None => no_base += 1,
                    Some(v) => {
                        assert_eq!(v, &p.value, "PC3: {d}: mirror {name} differs from its base");
                        equal += 1;
                    }
                }
            }
            match &mirror_names {
                None => mirror_names = Some(names),
                Some(first) => assert_eq!(first, &names, "PC3: {d}: mirror names differ"),
            }
        }
    }
    assert_eq!(avatar_count, 138);
    assert_eq!(
        mirror_names.map(|m| m.len()),
        Some(355),
        "PC3: 355 mirror names"
    );
    assert_eq!((equal, no_base), (47_886, 1_104), "§4.3 mirror values");

    // §6, PC6, PC7: nationalities.
    let nat_dir = props(
        &store,
        recs.control("NationalityTableDirectory").expect("label"),
    );
    let nat_dids = tessera_wstate::character::NationalityDirectory::from_properties(&nat_dir, &reg)
        .expect("directory")
        .tables;
    let nats: Vec<NationalityRecord> = nat_dids
        .iter()
        .map(|&d| NationalityRecord::from_properties(&props(&store, d), &reg).expect("nationality"))
        .collect();
    assert_eq!(nats.len(), 29);
    let nat_keys: BTreeSet<u32> = nats.iter().map(|n| n.nationality).collect();
    assert_eq!(nat_keys.len(), 29, "nationality keys unique");
    let mut listed: BTreeMap<u32, usize> = BTreeMap::new();
    for r in &races {
        let genders: BTreeSet<u32> = r.genders.iter().map(|g| g.gender).collect();
        for &k in &r.nationalities {
            *listed.entry(k).or_default() += 1;
            let n = nats
                .iter()
                .find(|n| n.nationality == k)
                .expect("PC6: nationality has a record");
            let sexes: BTreeSet<u32> = n.naming_sexes().into_iter().collect();
            assert_eq!(sexes, genders, "PC7: nationality {k:#x} sexes");
        }
    }
    assert!(
        listed.values().all(|&c| c == 1),
        "PC6: no nationality listed by two races"
    );
    assert_eq!(
        nat_keys.len() - listed.len(),
        1,
        "§6.2: one nationality named by no race"
    );
    assert_eq!(
        nats.iter()
            .map(|n| n.naming_guidelines.len())
            .sum::<usize>(),
        53,
        "§6.2 naming guidelines"
    );

    // §7, PC6: class tables.
    let infos: Vec<ClassCreationInfo> = class_tables
        .iter()
        .map(|&d| ClassCreationInfo::from_properties(&props(&store, d), &reg).expect("class"))
        .collect();
    let class_keys: BTreeSet<u32> = infos.iter().filter_map(|i| i.class).collect();
    assert_eq!(class_keys.len(), 12, "class keys");
    for r in &races {
        for k in &r.classes {
            assert!(
                class_keys.contains(k),
                "PC6: race class {k:#x} has a class table"
            );
        }
    }
    assert_eq!(
        infos
            .iter()
            .map(|i| i.starting_inventory.len())
            .sum::<usize>(),
        88,
        "§7.1 starting entries"
    );
    let kits: BTreeSet<Did> = infos
        .iter()
        .flat_map(|i| i.advanced_starts.iter().filter_map(|a| a.kit))
        .collect();
    assert_eq!(kits.len(), 9, "§7.2 kits");

    // §8, PC8–PC11: names.
    let rules = NameRuleControl::from_properties(
        &props(
            &store,
            recs.control("NameRulePropertyControl").expect("label"),
        ),
        &reg,
    )
    .expect("name rules");
    assert_eq!(rules.rules.len(), 4, "§8.1 rule keys");
    let gen_did = recs
        .control("RandomNameGenerationDirectory")
        .expect("label");
    let generator =
        NameGenerator::from_properties(&props(&store, gen_did), &reg).expect("generator");
    let pairs: BTreeSet<(u32, u32)> = generator
        .race_categories
        .iter()
        .map(|e| (e.species, e.sex))
        .collect();
    let race_pairs: BTreeSet<(u32, u32)> = entries.iter().map(|(r, g)| (*r, g.gender)).collect();
    assert_eq!(generator.race_categories.len(), 15);
    assert_eq!(
        pairs, race_pairs,
        "PC8: generator pairs equal the gender entries"
    );
    for e in &generator.race_categories {
        assert!(
            generator.category(e.category).is_some(),
            "PC8: category has rules"
        );
    }
    assert_eq!(generator.categories.len(), 21);
    assert_eq!(
        generator
            .categories
            .iter()
            .map(|c| c.options.len())
            .sum::<usize>(),
        64
    );
    assert_eq!(generator.syllable_tables.len(), 56);
    let used: BTreeSet<u32> = generator
        .categories
        .iter()
        .flat_map(|c| &c.options)
        .flat_map(|o| o.syllable_tables.iter().copied())
        .collect();
    let defined: BTreeSet<u32> = generator.syllable_tables.iter().map(|t| t.key).collect();
    assert_eq!(
        used, defined,
        "PC8: every syllable key used is defined and vice versa"
    );
    assert_eq!(
        generator
            .syllable_tables
            .iter()
            .map(|t| t.strings.len())
            .sum::<usize>(),
        8_797
    );
    assert_eq!(generator.shift_tables.len(), 13);
    assert_eq!(
        generator
            .shift_tables
            .iter()
            .map(|t| t.entries.len())
            .sum::<usize>(),
        1_289
    );
    assert_eq!(generator.valid_rules.len(), 3);
    for r in &generator.valid_rules {
        assert!(
            rules.rules.contains(r),
            "§8.1: valid rule {r:#x} is a name rule"
        );
    }

    let names_text = generator
        .resolve(|i| text(&strings, i))
        .expect("PC11: shift tables resolve");
    for c in &generator.categories {
        for o in &c.options {
            for (pos, key) in o.syllable_tables.iter().enumerate() {
                let strs = names_text.syllables.get(key).expect("resolved table");
                let table = generator.syllable_table(*key).expect("defined");
                assert_eq!(
                    strs.len(),
                    table.strings.len(),
                    "PC9: all syllables resolve"
                );
                for s in strs {
                    assert!(
                        (1..=7).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphabetic()),
                        "PC9: syllable of table {key} is 1-7 ASCII letters"
                    );
                    let first_upper = s.bytes().next().is_some_and(|b| b.is_ascii_uppercase());
                    assert_eq!(
                        first_upper,
                        pos == 0,
                        "PC9: case of table {key} at position {pos}"
                    );
                }
            }
        }
    }
    for t in names_text.shifts.values() {
        let mut seen = BTreeSet::new();
        for (_, e, _, s, _, _) in t.canonical_entries() {
            assert!(seen.insert((e, s.to_lowercase())), "PC10: (E, S) unique");
        }
    }
    let state = store.state(gen_did).expect("state").expect("present");
    let compiled: Vec<ShiftTable> = state
        .items()
        .iter()
        .filter_map(|i| i.as_native().and_then(|n| n.as_nested_string_table()))
        .map(ShiftTable::from_native)
        .collect();
    assert_eq!(compiled.len(), 13, "§8.2: 13 compiled shift tables");
    let mut unmatched: Vec<Vec<_>> = names_text
        .shifts
        .values()
        .map(ShiftTable::canonical_entries)
        .collect();
    for c in &compiled {
        assert!(c.is_well_formed(), "PC11: compiled table well formed");
        let canon = c.canonical_entries();
        let at = unmatched
            .iter()
            .position(|u| *u == canon)
            .expect("PC11: compiled table equals a source definition");
        unmatched.remove(at);
    }
    let mut draws = 0x1234_5678u32;
    let mut rnd = |n: u32| {
        draws = draws.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        draws % n.max(1)
    };
    // §8.5: the client's procedure under Decision D2; PC20 on every generated name.
    let mut generated = 0;
    let mut produced = 0usize;
    for &(species, sex) in &race_pairs {
        let g = tessera_wstate::character::generate_name(
            &generator,
            &names_text,
            species,
            sex,
            &mut rnd,
            |_| true,
        );
        if g.accepted {
            generated += 1;
        }
        for _ in 0..50 {
            let name = tessera_wstate::character::generate_name_once(
                &generator,
                &names_text,
                species,
                sex,
                &mut rnd,
            );
            assert!(!name.is_empty(), "§8.5: generation succeeds here");
            let mut chars = name.chars();
            let first = chars.next().expect("not empty");
            assert!(
                first.is_ascii_uppercase() && chars.all(|c| c.is_ascii_lowercase()),
                "PC20: case form of a generated name"
            );
            produced += 1;
        }
    }
    eprintln!("§8.5: {produced} names generated, all in the PC20 case form");
    assert_eq!(
        generated,
        race_pairs.len(),
        "D2: a name for every (species, sex)"
    );

    // PC19: every resolvable shift text is ASCII (all syllables were checked under PC9).
    for t in names_text.shifts.values() {
        for (_, e, _, s, l, r) in t.canonical_entries() {
            assert!(
                [e, s, l, r].iter().all(|x| x.is_ascii()),
                "PC19: shift texts are ASCII"
            );
        }
    }

    // PC18 (partly): the compiled half holds one category object per category and one
    // option object per option, and its weighted tables carry the categories' weights in
    // option order. Which attributes link them is not stated (§8.5 implementer OPEN), so
    // the tables are compared as a multiset.
    let mut compiled_weights: Vec<Vec<u32>> = state
        .items()
        .iter()
        .filter_map(|i| match i.as_native() {
            Some(tessera_wstate::Native::WeightedTable(v, _)) => {
                Some(v.iter().map(|e| e.weight).collect())
            }
            _ => None,
        })
        .collect();
    let mut property_weights: Vec<Vec<u32>> = generator
        .categories
        .iter()
        .map(|c| {
            c.options
                .iter()
                .map(|o| u32::try_from(o.weight).expect("weights are positive"))
                .collect()
        })
        .collect();
    compiled_weights.sort();
    property_weights.sort();
    assert_eq!(
        compiled_weights, property_weights,
        "PC18: weighted tables of the compiled half"
    );
    let option_count: usize = generator.categories.iter().map(|c| c.options.len()).sum();
    let mut by_class: BTreeMap<u32, usize> = BTreeMap::new();
    for i in state.items() {
        if let Some(c) = i.class() {
            *by_class.entry(c).or_insert(0) += 1;
        }
    }
    eprintln!(
        "PC18: {} categories, {option_count} options; compiled objects of classes 3047 / 3048 here: {:?} / {:?}",
        generator.categories.len(),
        by_class.get(&3047),
        by_class.get(&3048)
    );
    assert_eq!(
        by_class.get(&3047),
        Some(&generator.categories.len()),
        "PC18: categories"
    );
    assert_eq!(by_class.get(&3048), Some(&option_count), "PC18: options");

    // §9, PC12.
    let xp = XpGlobals::from_properties(
        &props(&store, recs.control("XPGlobals").expect("label")),
        &reg,
    )
    .expect("xp globals");
    let xp_class = role_class(Role::ExperienceTable);
    for &c in &xp_class {
        for d in all_of(c) {
            let s = store.state(d).expect("state").expect("present");
            let t = ExperienceTable::from_state(&s)
                .expect("ok")
                .expect("PC12: has an array");
            assert!(t.is_valid(), "PC12: {d} starts at 0 and never decreases");
        }
    }
    let class_xp: BTreeSet<Did> = infos.iter().filter_map(|i| i.experience_table).collect();
    assert_eq!(
        class_xp.len(),
        1,
        "§7.4: one experience table for all classes"
    );
    let class_xp = class_xp.into_iter().next().expect("one");
    let table =
        ExperienceTable::from_state(&store.state(class_xp).expect("state").expect("present"))
            .expect("ok")
            .expect("table");
    let max_level = usize::try_from(xp.absolute_max_level.expect("max level")).expect("positive");
    assert!(
        max_level < table.totals.len(),
        "§9.1: max level below the table length"
    );
    assert_eq!(
        table.level_for(0),
        Some(1),
        "§9.2: a new character is level 1"
    );

    // §10, PC13, PC14.
    let pip_ctl = PipControl::from_properties(
        &props(&store, recs.control("PipControl").expect("label")),
        &reg,
    )
    .expect("pip control");
    let pip_set: BTreeSet<Did> = pip_ctl.pips.iter().copied().collect();
    assert_eq!(pip_set.len(), pip_ctl.pips.len(), "PC14: each pip once");
    assert_eq!(pip_set.len(), 25);
    let pips: Vec<Pip> = pip_ctl
        .pips
        .iter()
        .map(|&d| Pip::from_properties(&props(&store, d), &reg).expect("pip"))
        .collect();
    let types: BTreeSet<u32> = pips.iter().map(|p| p.pip_type).collect();
    let values: BTreeSet<u32> = pips.iter().filter_map(|p| p.value_property).collect();
    assert_eq!(
        (types.len(), values.len()),
        (25, 25),
        "PC13: types and value properties unique"
    );
    let mut policies = 0;
    for p in &pips {
        let (min, home, max) = (
            p.min.expect("min"),
            p.home.expect("home"),
            p.max.expect("max"),
        );
        assert!(min <= home && home <= max, "PC13: Min <= Home <= Max");
        if let Some(t) = &p.threshold {
            policies += 1;
            assert!(
                t.bands_are_consistent(max),
                "PC13: bands of pip type {}",
                p.pip_type
            );
        }
    }
    assert_eq!(policies, 18, "§10.3: 18 threshold policies");
    for i in &infos {
        for k in i.pips.iter().chain(&i.own_pip) {
            assert!(types.contains(k), "PC14: class pip {k:#x} has a record");
        }
    }

    // §11.3, PC15.
    let character = BankRepository::from_properties(
        &props(&store, recs.control("Character").expect("label")),
        &reg,
    )
    .expect("character bank");
    let mut step_counts = Vec::new();
    for d in recs.with_role(Role::BankSchedule) {
        let s = BankUpgradeSchedule::from_properties(&props(&store, d), &reg).expect("schedule");
        assert!(s.is_consistent(), "PC15: {d}");
        step_counts.push(s.steps.len());
        if Some(d) == character.schedule {
            assert_eq!(
                s.steps.first().and_then(|x| x.required_capacity),
                character.base_capacity,
                "§11.3: character schedule starts at the base capacity"
            );
        }
    }
    step_counts.sort_unstable();
    assert_eq!(step_counts, vec![13, 17, 23], "§11.3 schedule lengths");

    // §12.4, PC16.
    let control = AllegianceControl::from_properties(
        &props(&store, recs.control("AllegianceControl").expect("label")),
        &reg,
    )
    .expect("allegiance control");
    let allegiances: Vec<(Did, Allegiance)> = recs
        .with_role(Role::Allegiance)
        .map(|d| {
            (
                d,
                Allegiance::from_properties(&props(&store, d), &reg).expect("allegiance"),
            )
        })
        .collect();
    assert_eq!(allegiances.len(), 7);
    assert_eq!(control.groups.len(), 3);
    let problems = allegiance_problems(&control, &allegiances);
    assert!(problems.is_empty(), "PC16: {problems:?}");

    eprintln!("player-character census passed in {:?}", t0.elapsed());
}
