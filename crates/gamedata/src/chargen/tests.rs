// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the creation-screen model. Keys, DIDs and texts are invented.

use std::collections::BTreeMap;

use tessera_formats::reader::{TSize, Utf16String};
use tessera_formats::{Did, StringInfo, StringSource};
use tessera_props::{Property, PropertySet, Value};
use tessera_wstate::character::{
    NameCategory, NameGenerator, NameOption, NameText, NamingGuideline, RaceCategory,
};

use super::*;

const M: u32 = 0x1000;
const F: u32 = 0x2000;

fn text(s: &str) -> StringInfo {
    StringInfo {
        source: StringSource::Literal(Utf16String::from(s)),
        args: None,
    }
}

fn race(did: u32, species: u32, sexes: &[u32], classes: &[u32], nats: &[u32]) -> RaceChoice {
    RaceChoice::from_record(
        Did(did),
        RaceRecord {
            race: species,
            genders: sexes
                .iter()
                .map(|&sex| GenderEntry {
                    gender: sex,
                    player_avatar: Some(Did(0x7010_0000 + species * 16 + sex / 0x1000)),
                    ..GenderEntry::default()
                })
                .collect(),
            classes: classes.to_vec(),
            nationalities: nats.to_vec(),
            ..RaceRecord::default()
        },
    )
}

fn class(c: u32) -> ClassChoice {
    ClassChoice {
        class: c,
        table: Some(Did(0x7020_0000 + c)),
        icon: Some(Did(0x4100_0000 + c)),
        name: Some(text("class")),
        ..ClassChoice::default()
    }
}

fn controls(n: usize) -> AvatarControls {
    let c = AppearanceControl {
        index: 0,
        availability: 1,
        group: None,
        kind: ControlKind::FaceLayer,
    };
    AvatarControls {
        entity: Did(0x4700_0001),
        all: vec![c; n],
        creation: vec![c; n],
    }
}

/// §19.4 of `player-character.md`, plus a second race.
fn data() -> ChargenData {
    let mut d = ChargenData {
        races: vec![
            race(0x7000_0010, 0x10, &[M, F], &[0x20, 0x21, 0x22], &[0x3]),
            race(0x7000_0011, 0x11, &[M], &[0x21], &[0x5, 0x4]),
        ],
        priority: ClassPriority {
            entries: vec![(0x21, 1), (0x20, 2)],
        },
        ..ChargenData::default()
    };
    for c in [0x20, 0x21, 0x22] {
        d.classes.insert(c, class(c));
    }
    for (k, sexes) in [(0x3, vec![M, F]), (0x4, vec![M]), (0x5, vec![M])] {
        d.nationalities.insert(
            k,
            (
                Did(0x7030_0000 + k),
                NationalityRecord {
                    nationality: k,
                    naming_guidelines: sexes
                        .into_iter()
                        .map(|s| NamingGuideline {
                            sex: Some(s),
                            text: Some(text(if s == M { "m" } else { "f" })),
                        })
                        .collect(),
                    ..NationalityRecord::default()
                },
            ),
        );
    }
    let avatars: Vec<Did> = d
        .races
        .iter()
        .flat_map(|r| r.genders.iter().filter_map(|g| g.player_avatar))
        .collect();
    for a in avatars {
        d.controls.insert(a, Ok(controls(3)));
    }
    d
}

fn choice(species: u32, sex: u32, nationality: u32, class: u32) -> CreationChoice {
    CreationChoice {
        species,
        sex,
        nationality,
        class,
    }
}

#[test]
fn creation_check_example_19_4() {
    let d = data();
    assert_eq!(d.check(&choice(0x10, F, 0x3, 0x21)), ChoiceVerdict::Valid);
    assert_eq!(
        d.check(&choice(0x10, F, 0x4, 0x21)),
        ChoiceVerdict::NationalityNotAllowed
    );
    assert_eq!(
        d.check(&choice(0x10, F, 0x3, 0x23)),
        ChoiceVerdict::ClassNotAllowed
    );
    assert_eq!(
        d.check(&choice(0x12, F, 0x3, 0x21)),
        ChoiceVerdict::NoSuchRaceAndSex
    );
    assert_eq!(
        d.check(&choice(0x11, F, 0x5, 0x21)),
        ChoiceVerdict::NoSuchRaceAndSex,
        "the second race has no female entry"
    );
}

#[test]
fn lists_in_screen_order() {
    let d = data();
    // Races in stored order; classes by ascending priority, unprioritised last.
    assert_eq!(
        d.races.iter().map(RaceChoice::species).collect::<Vec<_>>(),
        vec![0x10, 0x11]
    );
    assert_eq!(d.class_order(0x10), vec![0x21, 0x20, 0x22]);
    assert_eq!(
        d.classes_for(0x10)
            .iter()
            .map(|c| c.class)
            .collect::<Vec<_>>(),
        vec![0x21, 0x20, 0x22]
    );
    assert!(d.class_order(0x99).is_empty());
    // Nationalities in the race's stored order.
    let n: Vec<u32> = d.nationalities_for(0x11).iter().map(|(k, _)| *k).collect();
    assert_eq!(n, vec![0x5, 0x4]);
    assert!(d.nationalities_for(0x11).iter().all(|(_, r)| r.is_some()));
    assert_eq!(d.naming_guideline(0x3, F), Some(&text("f")));
    assert_eq!(d.naming_guideline(0x4, F), None);
    // Every (race, gender, class) choice is offered (§9 step 1) and accepted.
    let offered = d.offered();
    assert_eq!(offered.len(), 2 * 3 + 1);
    for (s, x, c) in offered {
        let nat = d.default_nationality(s).unwrap_or(0);
        assert_eq!(d.check(&choice(s, x, nat, c)), ChoiceVerdict::Valid);
    }
}

#[test]
fn class_order_ties_and_duplicates() {
    let mut d = data();
    d.races[0].record.classes = vec![0x22, 0x20, 0x21, 0x20, 0x23];
    d.priority.entries = vec![(0x20, 5), (0x21, 5), (0x20, 1)];
    // 0x20 takes its first entry (5); ties keep stored order; repeats listed once.
    assert_eq!(d.class_order(0x10), vec![0x20, 0x21, 0x22, 0x23]);
    let unknown = d.classes_for(0x10);
    assert_eq!(
        unknown.last().map(|c| (c.class, c.table)),
        Some((0x23, None))
    );
}

#[test]
fn racial_traits_by_level() {
    let mut d = data();
    let t = |did: u32, level: Option<i32>| RaceTrait {
        trait_did: Some(Did(did)),
        level,
        rank: Some(0),
    };
    d.races[0].record.characteristics =
        vec![t(1, Some(10)), t(2, None), t(3, Some(1)), t(4, Some(10))];
    let order: Vec<u32> = d
        .racial_traits(0x10)
        .iter()
        .filter_map(|x| x.trait_did.map(|d| d.0))
        .collect();
    assert_eq!(order, vec![3, 1, 4, 2]);
}

#[test]
fn flow_defaults_section_6() {
    let d = data();
    assert_eq!(d.default_race_and_gender(None), Some((0x10, M)));
    assert_eq!(d.default_race_and_gender(Some((0x10, F))), Some((0x10, F)));
    assert_eq!(
        d.default_race_and_gender(Some((0x11, F))),
        Some((0x10, M)),
        "an unknown pair falls back to the first race"
    );
    assert_eq!(d.class_after_race_change(Some(0x21), 0x11), Some(0x21));
    assert_eq!(d.class_after_race_change(Some(0x20), 0x11), Some(0x21));
    assert_eq!(d.class_after_race_change(None, 0x10), Some(0x21));
    assert_eq!(d.gender_after_race_change(Some(F), 0x11), Some(M));
    assert_eq!(d.gender_after_race_change(Some(F), 0x10), Some(F));
    assert_eq!(d.default_nationality(0x11), Some(0x5));
    assert!(keeps_appearance((0x10, M), (0x10, M)));
    assert!(!keeps_appearance((0x10, M), (0x10, F)));
    assert_eq!(ChargenData::default().default_race_and_gender(None), None);
}

#[test]
fn invariants_csi2_to_csi5() {
    let d = data();
    assert!(d.problems().is_empty(), "{:?}", d.problems());
    assert!(d.is_creation_body(0x10, F));
    assert!(!d.is_creation_body(0x11, F));
    assert!(d.controls_for(0x10, M).is_some_and(Result::is_ok));
    assert!(d.controls_for(0x11, F).is_none());

    let mut bad = data();
    bad.races.push(race(0x7000_0012, 0x12, &[], &[], &[]));
    bad.races[0].record.classes.push(0x30);
    if let Some(c) = bad.classes.get_mut(&0x22) {
        c.icon = None;
    }
    let a = bad.races[1].genders[0].player_avatar.unwrap_or_default();
    bad.controls.insert(a, Ok(controls(0)));
    let b = bad.races[0].genders[1].player_avatar.unwrap_or_default();
    bad.controls.insert(b, Err("broken".into()));
    bad.races[0].genders[0].player_avatar = None;
    let p = bad.problems();
    for expected in [
        ChargenProblem::RaceWithoutGender { species: 0x12 },
        ChargenProblem::RaceWithoutClass { species: 0x12 },
        ChargenProblem::ClassWithoutTable {
            species: 0x10,
            class: 0x30,
        },
        ChargenProblem::ClassWithoutInfo { class: 0x22 },
        ChargenProblem::GenderWithoutAvatar {
            species: 0x10,
            sex: M,
        },
        ChargenProblem::ControlsUnreadable {
            avatar: b,
            reason: "broken".into(),
        },
        ChargenProblem::NoCreationControls { avatar: a },
    ] {
        assert!(p.contains(&expected), "{expected:?} missing from {p:?}");
    }
}

#[test]
fn validate_creation_request() {
    let d = data();
    let ok = d.validate_creation(&choice(0x10, F, 0x3, 0x20), "Anna", None);
    assert_eq!(
        ok,
        Ok(CreationRequest {
            name: "Anna".into(),
            species: 0x10,
            sex: F,
            class: 0x20,
            nationality: 0x3
        })
    );
    assert_eq!(
        d.validate_creation(&choice(0x10, F, 0x9, 0x20), "Anna", None),
        Err(CreationRefusal::Choice(
            ChoiceVerdict::NationalityNotAllowed
        ))
    );
    assert_eq!(
        d.validate_creation(&choice(0x10, F, 0x3, 0x20), "", None),
        Err(CreationRefusal::Name(NameCheckFailure::Empty))
    );
    let names = NameData {
        generator: NameGenerator::default(),
        text: NameText::default(),
        policy: Some(policy(rule_set())),
    };
    assert!(matches!(
        d.validate_creation(&choice(0x10, F, 0x3, 0x20), "A1", Some(&names)),
        Err(CreationRefusal::Name(NameCheckFailure::Character {
            index: 1,
            ch: '1'
        }))
    ));
    assert!(
        d.validate_creation(&choice(0x10, F, 0x3, 0x20), "Anna", Some(&names))
            .is_ok()
    );
}

// -----------------------------------------------------------------------------------------
// Appearance controls
// -----------------------------------------------------------------------------------------

fn pids() -> AppearancePids {
    AppearancePids {
        controls: Some(1),
        availability: Some(2),
        group: Some(3),
        apr_control: Some(4),
        apr_file: Some(5),
        apr_key: Some(6),
        script_property: Some(7),
        compositor_control: Some(8),
        dyemap_control: Some(9),
        control_property: Some(10),
        dyemap_image: Some(11),
        bone_scaling_control: Some(12),
        min_float: Some(13),
        max_float: Some(14),
        bone_slider_control: Some(15),
        slider_definitions: Some(16),
        presets_control: Some(17),
        presets_file: Some(18),
    }
}

fn st(props: Vec<Property>) -> Value {
    Value::Struct(Box::new(PropertySet::from_properties(props)))
}

fn ctl(avail: u32, group: Option<u32>, inner: Property) -> Property {
    let mut v = vec![Property::new(2, Value::Bitfield32(avail)), inner];
    if let Some(g) = group {
        v.push(Property::new(3, Value::Enum(g)));
    }
    Property::new(100, st(v))
}

fn avatar_set() -> PropertySet {
    let controls = vec![
        ctl(
            0b01,
            Some(2),
            Property::new(
                4,
                st(vec![
                    Property::new(5, Value::DidRef(Did(0x2000_0001))),
                    Property::new(6, Value::Enum(7)),
                    Property::new(7, Value::PropertyId(0x99)),
                ]),
            ),
        ),
        ctl(0b10, Some(2), Property::new(8, st(vec![]))),
        ctl(
            0b11,
            Some(1),
            Property::new(
                9,
                st(vec![
                    Property::new(10, Value::PropertyId(0x55)),
                    Property::new(11, Value::DidRef(Did(0x4100_0002))),
                ]),
            ),
        ),
        ctl(
            0b01,
            None,
            Property::new(
                12,
                st(vec![
                    Property::new(13, Value::Float(0.9)),
                    Property::new(14, Value::Float(1.1)),
                ]),
            ),
        ),
        ctl(
            0b01,
            Some(2),
            Property::new(
                15,
                st(vec![Property::new(16, Value::DidRef(Did(0x7800_0001)))]),
            ),
        ),
        ctl(
            0b01,
            Some(1),
            Property::new(
                17,
                st(vec![Property::new(18, Value::DidRef(Did(0x7800_0002)))]),
            ),
        ),
        ctl(0b01, None, Property::new(50, Value::Int(1))),
    ];
    PropertySet::from_properties(vec![Property::new(1, Value::Array(controls))])
}

#[test]
fn appearance_controls_section_5_2() {
    let all = read_controls(&avatar_set(), &pids()).expect("controls");
    assert_eq!(all.len(), 7);
    assert_eq!(
        all[0].kind,
        ControlKind::Style {
            file: Some(Did(0x2000_0001)),
            key: Some(7),
            script_property: Some(0x99)
        }
    );
    assert_eq!(all[1].kind, ControlKind::FaceLayer);
    assert_eq!(
        all[2].kind,
        ControlKind::Colour {
            property: Some(0x55),
            palette: Some(Did(0x4100_0002))
        }
    );
    assert_eq!(
        all[3].kind,
        ControlKind::BodyScale {
            min: Some(0.9),
            max: Some(1.1)
        }
    );
    assert_eq!(
        all[4].kind,
        ControlKind::Sliders {
            definitions: Some(Did(0x7800_0001))
        }
    );
    assert_eq!(
        all[5].kind,
        ControlKind::Presets {
            file: Some(Did(0x7800_0002))
        }
    );
    assert_eq!(all[6].kind, ControlKind::Unknown);
    // The CharGen bit (here 0b01) selects; stored order is kept.
    let creation = creation_controls(&all, Some(0b01));
    assert_eq!(
        creation.iter().map(|c| c.index).collect::<Vec<_>>(),
        vec![0, 2, 3, 4, 5, 6]
    );
    assert_eq!(
        creation_controls(&all, None).len(),
        7,
        "unknown bit keeps all"
    );
    // Groups in first-appearance order.
    let groups = group_controls(&creation);
    let shape: Vec<(Option<u32>, Vec<usize>)> = groups
        .iter()
        .map(|g| (g.group, g.controls.iter().map(|c| c.index).collect()))
        .collect();
    assert_eq!(
        shape,
        vec![
            (Some(2), vec![0, 4]),
            (Some(1), vec![2, 5]),
            (None, vec![3, 6])
        ]
    );
    assert!(read_controls(&PropertySet::new(), &pids()).is_err());
    assert!(shows_avatar_update_toggle(0x49));
    assert!(shows_avatar_update_toggle(0x51));
    assert!(!shows_avatar_update_toggle(0x17));
}

// -----------------------------------------------------------------------------------------
// Names
// -----------------------------------------------------------------------------------------

fn u(s: &str) -> Utf16String {
    Utf16String::from(s)
}

/// A rule set in the shape of `misc-records.md` §9 with invented values: letters in
/// contexts 1 and 4, digits in context 2, printable in 8; repeat 2, vowel run 3, vowel
/// within 4, consonant run 4; one letter group "th"; extra vowel "y"; no forbidden first.
fn rule_set() -> NameRuleSet {
    NameRuleSet {
        rule_key: 1,
        numbers: [0b0101, 0b0010, 0b1000, 2, 3, 4, 4],
        string_list_a: vec![u("th"), u("")],
        string_a: u("y"),
        string_b: u(""),
        classes_size: TSize::default(),
        character_classes: vec![
            CharacterClass {
                key: 1,
                characters: u("'-"),
                strings: vec![u("xz")],
            },
            CharacterClass {
                key: 4,
                characters: u(""),
                strings: vec![],
            },
        ],
        flag_a: false,
        u32_list: vec![],
        flag_b: false,
    }
}

fn policy(rule: NameRuleSet) -> NamePolicy {
    NamePolicy {
        rule: Some(rule),
        context: 1,
    }
}

#[test]
fn character_check_9_1() {
    let r = rule_set();
    assert!(char_allowed(&r, 1, 'a'));
    assert!(!char_allowed(&r, 1, 'y'), "inside the excluded range x–z");
    assert!(char_allowed(&r, 4, 'y'), "class 4 has no range");
    assert!(char_allowed(&r, 1, '\''), "allowed string");
    assert!(
        !char_allowed(&r, 4, '\''),
        "not in class 4's allowed string"
    );
    assert!(!char_allowed(&r, 1, '['));
    assert!(!char_allowed(&r, 1, ']'));
    assert!(!char_allowed(&r, 1, '7'));
    assert!(char_allowed(&r, 2, '7'), "digits in context 2");
    assert!(char_allowed(&r, 8, '!'), "printable in context 8");
    assert!(!char_allowed(&r, 8, '\u{7}'));
    assert!(!char_allowed(&r, 16, 'a'), "no type allowed in context 16");
    assert!(char_allowed(&r, 1, 'é'));
    assert_eq!(
        check_characters(Some(&r), 1, ""),
        Err(NameCheckFailure::Empty)
    );
    assert_eq!(
        check_characters(None, 1, "anna"),
        Err(NameCheckFailure::Character { index: 0, ch: 'a' }),
        "an unknown rule set allows no character"
    );
    assert_eq!(check_characters(Some(&r), 1, "o'neal"), Ok(()));
}

#[test]
fn name_check_9_2() {
    let r = Some(rule_set());
    let r = r.as_ref();
    assert_eq!(
        check_name_rules(None, "zzzzzz"),
        Ok(()),
        "unknown rule set passes"
    );
    assert_eq!(check_name_rules(r, "Anna"), Ok(()));
    // Step 1: at most 2 identical in a row (case-insensitive after lower-casing).
    assert_eq!(check_name_rules(r, "Annna"), Err(NameCheckFailure::Repeats));
    assert_eq!(check_name_rules(r, "AAa"), Err(NameCheckFailure::Repeats));
    // Step 3: a vowel (or extra vowel) among the first 4 collapsed letters; "th" counts
    // as one letter.
    assert_eq!(
        check_name_rules(r, "brdka"),
        Err(NameCheckFailure::NoEarlyVowel)
    );
    assert_eq!(check_name_rules(r, "thrsa"), Ok(()), "collapsed: t r s a");
    assert_eq!(check_name_rules(r, "brdy"), Ok(()), "y is an extra vowel");
    // Step 4: at most 3 vowels (not extra vowels) in a row on the plain name.
    assert_eq!(
        check_name_rules(r, "baeiou"),
        Err(NameCheckFailure::VowelRun)
    );
    assert_eq!(check_name_rules(r, "baeiyou"), Ok(()));
    // Step 5: every 4 collapsed letters hold a vowel; a short name needs one vowel.
    assert_eq!(
        check_name_rules(r, "abrdk"),
        Err(NameCheckFailure::ConsonantRun)
    );
    assert_eq!(
        check_name_rules(r, "abrthd"),
        Err(NameCheckFailure::ConsonantRun)
    );
    assert_eq!(
        check_name_rules(r, "abrthad"),
        Ok(()),
        "collapsed: a b r t a d"
    );
}

#[test]
fn name_check_short_names_and_first_character() {
    let mut rs = rule_set();
    rs.numbers[5] = 0;
    let r = Some(&rs);
    assert_eq!(
        check_name_rules(r, "brd"),
        Err(NameCheckFailure::ConsonantRun)
    );
    assert_eq!(check_name_rules(r, "bra"), Ok(()));
    rs.string_b = u("q");
    assert_eq!(
        check_name_rules(Some(&rs), "Qua"),
        Err(NameCheckFailure::FirstCharacter),
        "checked on the lower-cased name"
    );
    // Zero limits switch the steps off.
    let mut off = rule_set();
    off.numbers = [0b1, 0, 0, 0, 0, 0, 0];
    assert_eq!(check_name_rules(Some(&off), "bbbbbbrrrrdk"), Ok(()));
}

#[test]
fn name_policy_default_and_check() {
    let mut other = rule_set();
    other.rule_key = 2;
    other.numbers[0] = 0;
    let table = NameFilterTable {
        did: Did(0x0E20_0000),
        size: TSize::default(),
        rule_sets: vec![other, rule_set()],
    };
    // Key 2 has no letter context; key 1 is the first usable listed key.
    let p = NamePolicy::tessera_default(&table, &[3, 1, 2]).expect("policy");
    assert_eq!(p.rule.as_ref().map(|r| r.rule_key), Some(1));
    assert_eq!(p.context, 1);
    assert!(NamePolicy::tessera_default(&table, &[2]).is_none());
    assert!(NamePolicy::tessera_default(&table, &[9]).is_none());
    assert_eq!(p.check("Anna"), Ok(()));
    assert_eq!(
        p.check("Anya"),
        Err(NameCheckFailure::Character { index: 2, ch: 'y' })
    );
    assert_eq!(p.check("Annna"), Err(NameCheckFailure::Repeats));
}

fn generator() -> (NameGenerator, NameText) {
    let g = NameGenerator {
        race_categories: vec![RaceCategory {
            species: 0x10,
            sex: M,
            category: 5,
        }],
        categories: vec![NameCategory {
            key: 5,
            options: vec![NameOption {
                weight: 1,
                syllable_tables: vec![1, 2],
                shift_tables: vec![9],
            }],
        }],
        ..NameGenerator::default()
    };
    let mut syllables = BTreeMap::new();
    syllables.insert(1, vec!["Dor".to_owned(), "Bel".to_owned()]);
    syllables.insert(2, vec!["dor".to_owned(), "in".to_owned(), "nnn".to_owned()]);
    (
        g,
        NameText {
            syllables,
            shifts: BTreeMap::new(),
        },
    )
}

#[test]
fn random_names_section_8_5() {
    let (generator, text) = generator();
    let mut names = NameData {
        generator,
        text,
        policy: None,
    };
    let allowed = ["Dorin", "Belin", "Beldor", "Dornnn", "Belnnn"];
    for seed in 0..64 {
        let n = names.random_name(0x10, M, seed);
        assert!(n.accepted);
        assert!(allowed.contains(&n.name.as_str()), "{}", n.name);
        assert_eq!(names.random_name(0x10, M, seed), n, "seeded: deterministic");
    }
    // With the policy, names with three n in a row never pass.
    names.policy = Some(policy(rule_set()));
    for seed in 0..64 {
        let n = names.random_name(0x10, M, seed);
        assert!(n.accepted);
        assert!(!n.name.contains("nnn"), "{}", n.name);
    }
    // No category for the pair: generation fails, the name is empty.
    let n = names.random_name(0x11, F, 1);
    assert!(!n.accepted);
    assert!(n.name.is_empty());
    assert_eq!(names.check(""), Err(NameCheckFailure::Empty));
    names.policy = None;
    assert_eq!(names.check("x"), Ok(()));
}

// -----------------------------------------------------------------------------------------
// Review regressions (2026-10-10)
// -----------------------------------------------------------------------------------------

#[test]
fn gender_list_is_built_from_the_record() {
    let d = data();
    let records: Vec<RaceRecord> = d.races.iter().map(|r| r.record.clone()).collect();
    for r in &d.races {
        let from_record: Vec<u32> = r.record.genders.iter().map(|g| g.gender).collect();
        let model: Vec<u32> = r.genders.iter().map(|g| g.sex).collect();
        assert_eq!(model, from_record);
        assert_eq!(
            r.genders
                .iter()
                .map(|g| g.player_avatar)
                .collect::<Vec<_>>(),
            r.record
                .genders
                .iter()
                .map(|g| g.player_avatar)
                .collect::<Vec<_>>()
        );
    }
    // `check` agrees with `check_choice` over the records for every combination.
    for species in [0x10, 0x11, 0x12] {
        for sex in [M, F, 0x3000] {
            for nationality in [0x3, 0x4, 0x5, 0x6] {
                for class in [0x20, 0x21, 0x22, 0x23] {
                    let c = choice(species, sex, nationality, class);
                    assert_eq!(
                        d.check(&c),
                        tessera_wstate::character::check_choice(&records, &c),
                        "{c:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn offered_uses_one_priority_map() {
    let d = data();
    let expected: Vec<(u32, u32, u32)> = d
        .races
        .iter()
        .flat_map(|r| {
            let order = d.class_order(r.species());
            r.genders
                .iter()
                .flat_map(move |g| {
                    order
                        .clone()
                        .into_iter()
                        .map(move |c| (r.species(), g.sex, c))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(d.offered(), expected);
    assert_eq!(
        &d.offered()[..3],
        &[(0x10, M, 0x21), (0x10, M, 0x20), (0x10, M, 0x22)]
    );
}

#[test]
fn skipped_records_are_problems() {
    let mut d = data();
    d.unreadable.push((Did(0x7000_00FF), "bad".into()));
    assert_eq!(
        d.problems(),
        vec![ChargenProblem::RecordUnreadable {
            did: Did(0x7000_00FF),
            reason: "bad".into()
        }]
    );
}

#[test]
fn nationality_dye_override_replaces_the_palette() {
    let mut d = data();
    let colour = |property| AppearanceControl {
        index: 0,
        availability: 1,
        group: None,
        kind: ControlKind::Colour {
            property,
            palette: Some(Did(0x4100_0001)),
        },
    };
    if let Some((_, n)) = d.nationalities.get_mut(&0x3) {
        n.dye_overrides = vec![
            DyeOverride {
                control_property: Some(0x55),
                image: None,
            },
            DyeOverride {
                control_property: Some(0x55),
                image: Some(Did(0x4100_0099)),
            },
            DyeOverride {
                control_property: Some(0x56),
                image: Some(Did(0x4100_0098)),
            },
        ];
    }
    assert_eq!(
        d.palette_for(&colour(Some(0x55)), 0x3),
        Some(Did(0x4100_0099)),
        "the matching override with an image replaces the palette"
    );
    assert_eq!(
        d.palette_for(&colour(Some(0x57)), 0x3),
        Some(Did(0x4100_0001)),
        "no override for this property"
    );
    assert_eq!(
        d.palette_for(&colour(Some(0x55)), 0x4),
        Some(Did(0x4100_0001)),
        "another nationality has no override"
    );
    assert_eq!(
        d.palette_for(&colour(None), 0x3),
        Some(Did(0x4100_0001)),
        "a control without a property keeps its palette"
    );
    let style = AppearanceControl {
        kind: ControlKind::FaceLayer,
        ..colour(Some(0x55))
    };
    assert_eq!(d.palette_for(&style, 0x3), None, "not a colour control");
}

#[test]
fn simple_lower_case_of_dotted_capital_i() {
    assert_eq!(super::names::lower('\u{130}'), 'i');
    assert_eq!(super::names::lower('A'), 'a');
    assert_eq!(super::names::lower('\u{1E9E}'), '\u{DF}', "capital sharp s");
    // U+0130 is the only scalar whose full lower case has more than one character.
    let multi: Vec<char> = (0..=u32::from(char::MAX))
        .filter_map(char::from_u32)
        .filter(|c| c.to_lowercase().count() > 1)
        .collect();
    assert_eq!(multi, vec!['\u{130}']);
    let mut r = rule_set();
    r.numbers = [0b1, 0, 0, 0, 0, 1, 0];
    assert_eq!(
        check_name_rules(Some(&r), "\u{130}bb"),
        Ok(()),
        "starts with the vowel i"
    );
    assert_eq!(
        check_name_rules(Some(&r), "bbb"),
        Err(NameCheckFailure::NoEarlyVowel)
    );
}
