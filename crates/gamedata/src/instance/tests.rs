// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the finder model (`travel-windows.md` §11.4 and the rules of §6.3–
//! §6.8; `instances.md` §15.1 B1–B3) over the in-memory install of the quest tests. All
//! DIDs, keys, labels and texts are invented.

use tessera_formats::{Did, PropertyValue};

use super::*;
use crate::quest::tests::{Mem, arr, literal, si, st};

const P1: Did = Did(0x70F1_0001);
const P2: Did = Did(0x70F1_0002);
const P3: Did = Did(0x70F1_0003);
const P4: Did = Did(0x70F1_0004);
const P5: Did = Did(0x70F1_0005);
const WJC: Did = Did(0x70F1_0100);
const FMC: Did = Did(0x70F1_0101);

fn enums(keys: &[u32]) -> PropertyValue {
    arr(keys
        .iter()
        .map(|k| tessera_formats::property::PropertyEntry {
            pid: 0,
            value: PropertyValue::Enum(*k),
        })
        .collect())
}

fn dids(d: &[Did]) -> PropertyValue {
    arr(d
        .iter()
        .map(|d| tessera_formats::property::PropertyEntry {
            pid: 0,
            value: PropertyValue::DidRef(*d),
        })
        .collect())
}

#[allow(clippy::too_many_arguments)]
fn template(
    m: &mut Mem,
    did: Did,
    name: &str,
    category: Option<u32>,
    encounter_type: Option<u32>,
    sizes: &[u32],
    levels: (i32, i32),
    scaling: Option<i32>,
) {
    let mut e = vec![
        m.e(
            "PrivateEncounterTemplate_ContentLayer",
            PropertyValue::Enum(1),
        ),
        m.e("PrivateEncounterTemplate_Name", si(literal(name))),
        m.e(
            "PrivateEncounterTemplate_Description",
            si(literal("Into the dark")),
        ),
        m.e("Skirmish_Template_GroupSizeArray", enums(sizes)),
        m.e("Skirmish_Template_DifficultyTierArray", enums(&[1, 2])),
        m.e(
            "Skirmish_Template_MinLevelScale",
            PropertyValue::Int(levels.0),
        ),
        m.e(
            "Skirmish_Template_MaxLevelScale",
            PropertyValue::Int(levels.1),
        ),
    ];
    if let Some(c) = category {
        e.push(m.e("WorldJoin_EncounterCategory", PropertyValue::Enum(c)));
    }
    if let Some(t) = encounter_type {
        e.push(m.e("WorldJoin_EncounterType", PropertyValue::Enum(t)));
    }
    if let Some(s) = scaling {
        e.push(m.e("Skirmish_Template_LevelScalingLevel", PropertyValue::Int(s)));
    }
    m.put(did, e);
}

fn install() -> Mem {
    let mut m = Mem::default();
    template(
        &mut m,
        P1,
        "Old mine",
        Some(13),
        Some(4),
        &[6],
        (20, 50),
        None,
    );
    template(
        &mut m,
        P2,
        "Deep vault",
        Some(0),
        Some(4),
        &[3],
        (30, 30),
        None,
    );
    template(
        &mut m,
        P3,
        "Raid",
        Some(4),
        Some(3),
        &[3, 6],
        (10, 100),
        Some(100),
    );
    template(
        &mut m,
        P4,
        "Battle (small)",
        Some(7),
        Some(4),
        &[12],
        (40, 60),
        None,
    );
    template(
        &mut m,
        P5,
        "Battle (large)",
        Some(7),
        Some(4),
        &[24],
        (40, 60),
        None,
    );
    let group = {
        let f = vec![
            m.e(
                "WorldJoinControl_Instance_Group_Label",
                PropertyValue::Enum(2),
            ),
            m.e("WorldJoinControl_Grouped_Instance_List", dids(&[P4, P5])),
        ];
        st(f)
    };
    let e = vec![
        m.e("WorldJoin_AvailablePETemplates_Array", dids(&[P1, P2, P3])),
        m.e(
            "WorldJoinControl_InstanceGroups",
            arr(vec![tessera_formats::property::PropertyEntry {
                pid: 0,
                value: group,
            }]),
        ),
        m.e("WorldJoinControl_FeaturedInstance_Array", dids(&[P3, P1])),
        m.e(
            "WorldJoin_Capped_EncounterCategory",
            PropertyValue::Enum(99),
        ),
    ];
    m.put(WJC, e);
    let e = vec![m.e(
        "FellowshipMakerControl_ValidEncounterSizeArray",
        enums(&[3, 6, 12, 24]),
    )];
    m.put(FMC, e);
    m.weenies.insert(WORLD_JOIN_CONTROL.into(), WJC);
    m.weenies.insert(FELLOWSHIP_MAKER_CONTROL.into(), FMC);
    // The configuration mapper's labels (key k is bit k − 1) and display texts.
    let config = m.p("FellowshipMaker_PlayerConfiguration");
    let ui_config = m.p("FellowshipMakerUI_Configuration");
    for (k, l) in [
        (1, "Type_Skirmish"),
        (2, "Type_Instance"),
        (3, "Type_ScalingInstance"),
        (4, "Type_NonScalingInstance"),
        (6, "Size_Size2"),
        (7, "Size_Size3"),
        (8, "Size_Size6"),
        (9, "Tier_Tier1"),
        (10, "Tier_Tier2"),
    ] {
        m.enum_names.insert((config, k), l.into());
    }
    for (k, t) in [(1, "Skirmishes"), (3, "Instances"), (4, "Other")] {
        m.enum_texts.insert((ui_config, k), t.into());
    }
    let cat = m.p("WorldJoin_EncounterCategory");
    for (k, t) in [(13, "Mines"), (4, "Raids"), (7, "Battles")] {
        m.enum_texts.insert((cat, k), t.into());
    }
    let group_label = m.p("WorldJoinControl_Instance_Group_Label");
    m.enum_texts
        .insert((group_label, 2), "Great battles".into());
    let size = m.p("Skirmish_GroupSize");
    for (k, t) in [
        (3, "Small"),
        (6, "Fellowship"),
        (12, "Raid"),
        (24, "Large raid"),
    ] {
        m.enum_texts.insert((size, k), t.into());
    }
    let tier = m.p("Skirmish_DifficultyTier");
    for (k, t) in [(1, "Tier 1"), (2, "Tier 2")] {
        m.enum_texts.insert((tier, k), t.into());
    }
    m.text(
        FELLOWSHIP_MAKER_TABLE,
        tokens::CATEGORY_NONE,
        "Uncategorised",
    );
    m
}

fn entry(n: u16, d: Did) -> FinderEntryIn {
    FinderEntryIn {
        entry: n,
        instance: RecordRef::Record(d),
        level: None,
        size: None,
        tier: None,
        locked: false,
    }
}

#[test]
#[allow(clippy::type_complexity)]
fn example_11_4_finder_tree() {
    let m = install();
    let fp = FinderPresenter::new(&m);
    assert_eq!(fp.kind_of(P1), 0x4, "classic");
    assert_eq!(fp.kind_of(P3), 0x1, "skirmish");
    assert_eq!(
        fp.kind_of(P4),
        NO_KIND,
        "size 12 has no Size_ label: no index entry"
    );
    let entries: Vec<FinderEntryIn> = [P1, P2, P3, P4, P5]
        .iter()
        .enumerate()
        .map(|(i, d)| entry(u16::try_from(i + 1).unwrap(), *d))
        .collect();
    let v = fp.view(&entries, Some(&RecordRef::Record(P5)), 60);
    let texts: Vec<Option<&str>> = v.rows.iter().map(|r| r.text.as_deref()).collect();
    assert_eq!(
        texts,
        vec![
            Some("Old mine"),
            Some("Deep vault"),
            Some("Raid"),
            Some("Great battles")
        ]
    );
    assert_eq!(v.rows[3].entries, vec![4, 5], "one group row for P4 and P5");
    let shape: Vec<(u32, Option<&str>, Vec<(Option<&str>, Vec<usize>)>)> = v
        .headers
        .iter()
        .map(|h| {
            (
                h.kind,
                h.text.as_deref(),
                h.categories
                    .iter()
                    .map(|c| (c.text.as_deref(), c.rows.clone()))
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        shape,
        vec![
            (
                0x4,
                Some("Instances"),
                vec![(Some("Mines"), vec![0]), (Some("Uncategorised"), vec![1])]
            ),
            (0x1, Some("Skirmishes"), vec![(Some("Raids"), vec![2])]),
            (NO_KIND, Some("Other"), vec![(Some("Battles"), vec![3])]),
        ]
    );
    // The group's size menu is the union over its templates; size 24 resolves P5 (B3).
    let sizes: Vec<u32> = v.rows[3].sizes.iter().map(|m| m.key).collect();
    assert_eq!(sizes, vec![12, 24]);
    assert_eq!(v.rows[3].sizes[1].text.as_deref(), Some("Large raid"));
    assert_eq!(fp.group_template(2, 24), Some(P5));
    assert_eq!(fp.group_template(2, 12), Some(P4));
    assert_eq!(fp.group_template(2, 6), None);
    assert_eq!(fp.group_template(9, 24), None);
    assert_eq!(v.featured_row, Some(3), "the featured template's group row");
    assert_eq!(v.featured_template, Some(P5));
}

#[test]
fn menus_levels_and_feed_values() {
    let m = install();
    let fp = FinderPresenter::new(&m);
    let mut e2 = entry(2, P2);
    e2.level = Some(33);
    e2.size = Some(6);
    e2.locked = true;
    let lit = FinderEntryIn {
        entry: 3,
        instance: RecordRef::Literal("Cellar".into()),
        level: Some(5),
        size: None,
        tier: None,
        locked: false,
    };
    let v = fp.view(&[entry(1, P1), e2, lit], None, 60);
    let r1 = &v.rows[0];
    assert_eq!(r1.level, Some(50), "the player's 60 clamped into 20 … 50");
    assert!(r1.level_editable);
    assert_eq!((r1.size, r1.tier), (Some(6), Some(1)), "the first items");
    assert_eq!(r1.tiers[1].text.as_deref(), Some("Tier 2"));
    let r2 = &v.rows[1];
    assert_eq!(r2.level, Some(33), "the feed's level");
    assert!(!r2.level_editable, "no scaling range");
    assert_eq!(r2.size, Some(6), "the feed's size");
    assert!(r2.error_look);
    let r3 = &v.rows[2];
    assert_eq!(r3.subject, RowSubject::Literal("Cellar".into()));
    assert_eq!(r3.kind, NO_KIND);
    assert!(v.featured_row.is_none());
}

#[test]
fn b1_featured_and_b2_level_scaling() {
    let m = install();
    let fp = FinderPresenter::new(&m);
    assert_eq!(fp.featured_instance(None), None);
    assert_eq!(fp.featured_instance(Some(&PropertyValue::Int(0))), None);
    assert_eq!(fp.featured_instance(Some(&PropertyValue::Int(1))), Some(P3));
    assert_eq!(fp.featured_instance(Some(&PropertyValue::Int(2))), Some(P1));
    assert_eq!(fp.featured_instance(Some(&PropertyValue::Int(3))), None);
    assert_eq!(
        fp.featured_instance(Some(&PropertyValue::Int64(1))),
        None,
        "64-bit is not an integer"
    );
    assert_eq!(fp.level_scaling_level(P3, None), 100);
    assert_eq!(
        fp.level_scaling_level(P3, Some(&PropertyValue::Int(90))),
        90
    );
    assert_eq!(
        fp.level_scaling_level(P3, Some(&PropertyValue::Int64(90))),
        100
    );
    assert_eq!(
        fp.level_scaling_level(P1, Some(&PropertyValue::Int(90))),
        0,
        "below 1: itself"
    );
}

#[test]
fn join_queue_and_travel_window() {
    assert_eq!(
        join_button(false, false),
        JoinButton {
            normal: false,
            label: tokens::JOIN_NO_TEMPLATE
        }
    );
    assert_eq!(
        join_button(true, true),
        JoinButton {
            normal: false,
            label: tokens::JOIN_ERROR
        }
    );
    assert_eq!(
        join_button(true, false),
        JoinButton {
            normal: true,
            label: tokens::JOIN_NORMAL
        }
    );
    let q = queue_display(2, Some(30));
    assert!(q.waiting_field && q.waiting_timer && !q.confirm_timer);
    assert_eq!(q.alert_token, Some(tokens::QUEUE_STATES[1]));
    assert!(
        !queue_display(7, Some(0)).waiting_timer,
        "hidden when the time is 0"
    );
    assert!(queue_display(1, None).confirm_timer);
    assert_eq!(queue_display(0, None).alert_token, None);
    assert_eq!(queue_display(9, None).alert_token, None);
    let m = install();
    let fp = FinderPresenter::new(&m);
    let w = fp.instance_travel(P1);
    assert_eq!(
        (w.title.as_deref(), w.intro.as_deref()),
        (Some("Old mine"), Some("Into the dark"))
    );
    assert!(fp.template(WJC).is_none(), "a control is no template");
}

#[test]
fn row_cap() {
    let m = install();
    let fp = FinderPresenter::new(&m);
    let entries: Vec<FinderEntryIn> = (0..2_000u16)
        .map(|i| FinderEntryIn {
            entry: i,
            instance: RecordRef::Literal(format!("L{i}")),
            level: None,
            size: None,
            tier: None,
            locked: false,
        })
        .collect();
    assert_eq!(fp.view(&entries, None, 1).rows.len(), MAX_FINDER_ROWS);
}
