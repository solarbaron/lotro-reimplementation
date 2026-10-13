// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the mount model (`mounts.md` §11.1, §14.10, §14.11;
//! `game-controls.md` §5.3.2; `lua-plugin-api.md` §15.5) over the in-memory install of the
//! quest tests. All DIDs, keys, labels and texts are invented.

use tessera_formats::property::PropertyEntry;
use tessera_formats::{Did, PropertyValue};

use super::*;
use crate::quest::tests::{Mem, arr, literal, si, st};

const HORSE: Did = Did(0x70F2_0001);
const STEED: Did = Did(0x70F2_0002);
const HOBBY: Did = Did(0x70F2_0003);
const STARTUP: Did = Did(0x70F2_0004);
const NPC_STARTUP: Did = Did(0x70F2_0005);
const MIXED: Did = Did(0x70F2_0006);
const HEAVY: Did = Did(0x70F2_0007);
const S_TALL: Did = Did(0x70F2_0101);
const S_SHORT: Did = Did(0x70F2_0102);
const S_STEED: Did = Did(0x70F2_0103);
const S_OTHER: Did = Did(0x70F2_0104);
const DIR: Did = Did(0x70F2_0200);
const CTRL: Did = Did(0x70F2_0201);
const ICON: Did = Did(0x41F2_0001);

#[allow(clippy::too_many_arguments)]
fn effect(
    m: &mut Mem,
    did: Did,
    name: &str,
    ty: Option<u32>,
    entity: Option<Did>,
    steed: bool,
    script: u32,
    startup: bool,
) {
    let mut e = vec![
        m.e(
            "Mounted_Player_Cosmetic_Slot_HoldingLocation",
            PropertyValue::Enum(39),
        ),
        m.e(
            "Mounted_Player_Cosmetic_Slot_Behavior",
            PropertyValue::Enum(7),
        ),
        m.e("Mounted_AnimationScriptID", PropertyValue::Enum(script)),
        m.e("Mount_Name_Initial", si(literal(name))),
        m.e("Effect_Icon", PropertyValue::DidRef(ICON)),
    ];
    if let Some(t) = ty {
        e.push(m.e("Mount_Type", PropertyValue::Bitfield32(t)));
    }
    if let Some(d) = entity {
        e.push(m.e(
            "Mounted_Player_Cosmetic_Slot_Entity",
            PropertyValue::DidRef(d),
        ));
    }
    if steed {
        e.push(m.e("Mount_ShouldUseAppearances", PropertyValue::Bool(true)));
    } else {
        e.push(m.e("Mount_DurabilityChangeEventsArray", arr(vec![])));
    }
    if startup {
        e.push(m.e("Effect_RemoveOnDefeat", PropertyValue::Bool(false)));
        e.push(m.e("Effect_EquivalenceClass", PropertyValue::Enum(9)));
    } else {
        e.push(m.e("Mod_Array", arr(vec![])));
    }
    m.put(did, e);
}

fn skill(m: &mut Mem, did: Did, effects: &[Did]) {
    let list: Vec<PropertyEntry> = effects
        .iter()
        .map(|d| {
            let f = vec![m.e("Skill_Toggle_Effect", PropertyValue::DidRef(*d))];
            PropertyEntry {
                pid: 0,
                value: st(f),
            }
        })
        .collect();
    let e = vec![m.e("Skill_Toggle_Effect_List", arr(list))];
    m.put(did, e);
}

fn install() -> Mem {
    let mut m = Mem::default();
    let entity = Some(Did(0x47F2_0001));
    effect(
        &mut m,
        HORSE,
        "Bay horse",
        Some(0x1),
        entity,
        false,
        1,
        false,
    );
    effect(&mut m, STEED, "Steed", Some(0x20), entity, true, 1, false);
    effect(
        &mut m,
        HOBBY,
        "Stick",
        Some(0x400),
        entity,
        false,
        50,
        false,
    );
    effect(
        &mut m,
        STARTUP,
        "Startup",
        Some(0x20),
        entity,
        true,
        1,
        true,
    );
    effect(&mut m, NPC_STARTUP, "Npc", None, None, true, 1, true);
    effect(&mut m, MIXED, "Mixed", Some(0x9), entity, true, 1, false);
    effect(
        &mut m,
        HEAVY,
        "Heavy horse",
        Some(0x8000),
        entity,
        false,
        1,
        false,
    );
    skill(&mut m, S_TALL, &[HORSE]);
    skill(&mut m, S_SHORT, &[Did(0x70F2_0999), HEAVY]);
    skill(&mut m, S_STEED, &[STEED]);
    skill(&mut m, S_OTHER, &[Did(0x70F2_0999)]);
    let tall = m.p("Mount_SkillToGrantTall");
    let short = m.p("Mount_SkillToGrantShort");
    let e = vec![m.e(
        "Mount_SkillList",
        arr(vec![
            PropertyEntry {
                pid: short,
                value: PropertyValue::DidRef(S_SHORT),
            },
            PropertyEntry {
                pid: tall,
                value: PropertyValue::DidRef(S_TALL),
            },
            PropertyEntry {
                pid: tall,
                value: PropertyValue::DidRef(S_OTHER),
            },
        ]),
    )];
    m.put(DIR, e);
    let hash = |m: &mut Mem, tree: u32, race: u32, skill: Did| {
        let inner = vec![
            m.e("Mount_Control_Player_Race", PropertyValue::Enum(race)),
            m.e("Mount_Control_Mount_Skill", PropertyValue::DidRef(skill)),
        ];
        let f = vec![
            m.e("Mount_Control_Trait_Tree", PropertyValue::Enum(tree)),
            m.e(
                "Mount_Control_Mount_Skill_Hash",
                arr(vec![PropertyEntry {
                    pid: 0,
                    value: st(inner),
                }]),
            ),
        ];
        PropertyEntry {
            pid: 0,
            value: st(f),
        }
    };
    let trees = vec![hash(&mut m, 3, 23, S_STEED), hash(&mut m, 1, 23, S_STEED)];
    let mut e = vec![m.e("Mount_Control_Tree_To_Skill_Hash", arr(trees))];
    for (l, v) in [
        ("Mount_Control_Speed_Multiplier_Walk", 0.5),
        ("Mount_Control_Speed_Multiplier_Trot", 0.7),
        ("Mount_Control_Speed_Multiplier_Gallop", 1.0),
        ("Mount_Control_Turn_Rate_Multiplier_Walk", 1.0),
        ("Mount_Control_Turn_Rate_Multiplier_Canter", 0.55),
        ("Mount_Control_Turn_Rate_Multiplier_Gallop", 0.47),
    ] {
        e.push(m.e(l, PropertyValue::Float(v)));
    }
    m.put(CTRL, e);
    m.weenies.insert(MOUNT_DIRECTORY.into(), DIR);
    m.weenies.insert(MOUNTED_COMBAT_CONTROL.into(), CTRL);
    let ty = m.p("Mount_Type");
    for (k, l) in [
        (1, "Horse"),
        (3, "Goat"),
        (4, "WarsteedHeavy"),
        (6, "WarsteedMedium"),
        (11, "Cosmetic_Hobbyhorse"),
        (15, "Boar"),
        (16, "Clydesdale"),
    ] {
        m.enum_names.insert((ty, k), l.into());
    }
    let script = m.p("Mounted_AnimationScriptID");
    m.enum_names
        .insert((script, 50), "Emote_Holding_HobbyHorse".into());
    let class = m.p("Effect_EquivalenceClass");
    m.enum_names
        .insert((class, 9), "Mounted_Monster_Startup".into());
    let tree = m.p("Mount_Control_Trait_Tree");
    m.enum_names.insert((tree, 1), "MountedCombat_Heavy".into());
    m
}

#[test]
fn example_11_1_mount_type_bits() {
    let t = MountType(0x4004);
    assert_eq!(t.keys().collect::<Vec<_>>(), vec![3, 15]);
    assert!(!t.is_war_steed());
    let labels = vec!["Goat".to_owned(), "Boar".to_owned()];
    assert_eq!(
        categorise(Some(t), true, false, false, &labels),
        MountCategory::Normal
    );
}

#[test]
fn categories_of_section_14_11() {
    let m = install();
    let mp = MountPresenter::new(&m);
    let cat = |d| mp.effect(d).unwrap().category;
    assert_eq!(cat(HORSE), MountCategory::Normal);
    assert_eq!(cat(STEED), MountCategory::WarSteed);
    assert_eq!(cat(HOBBY), MountCategory::HobbyHorse);
    assert_eq!(cat(STARTUP), MountCategory::WarSteedStartup);
    assert_eq!(cat(NPC_STARTUP), MountCategory::NoEntityNoType);
    assert_eq!(cat(MIXED), MountCategory::MixedWarSteed);
    assert_eq!(cat(HEAVY), MountCategory::Clydesdale);
    assert!(!MountCategory::NoEntityNoType.is_drawable());
    let horse = mp.effect(HORSE).unwrap();
    assert_eq!(horse.name.as_deref(), Some("Bay horse"));
    assert_eq!(horse.icon, Some(ICON));
    assert_eq!(horse.type_labels, vec!["Horse".to_owned()]);
    assert_eq!(horse.class, MountClass::Basic);
    let steed = mp.effect(STEED).unwrap();
    assert!(steed.war_steed);
    assert_eq!(steed.class, MountClass::Combat);
    assert_eq!(steed.type_labels, vec!["WarsteedMedium".to_owned()]);
    assert!(mp.effect(S_TALL).is_none(), "a skill is no mount effect");
}

#[test]
fn sources_and_plugin_views() {
    let m = install();
    let mp = MountPresenter::new(&m);
    // walker.md §16.2: a skill resolves to its first toggled mount effect.
    let short = mp.resolve(S_SHORT).unwrap();
    assert_eq!((short.effect, short.skill), (HEAVY, Some(S_SHORT)));
    assert!(mp.resolve(S_OTHER).is_none());
    assert_eq!(mp.resolve(STEED).unwrap().skill, None);
    // lua-plugin-api.md §15.5: the first element only.
    assert_eq!(
        mp.plugin_class(&RecordRef::Record(S_STEED)),
        MountClass::Combat
    );
    assert_eq!(
        mp.plugin_class(&RecordRef::Record(S_SHORT)),
        MountClass::Basic
    );
    assert_eq!(
        mp.plugin_name(&RecordRef::Record(S_SHORT)),
        "",
        "first element is no mount"
    );
    assert_eq!(mp.plugin_name(&RecordRef::Record(S_STEED)), "Steed");
    assert_eq!(mp.plugin_name(&RecordRef::Literal("Pony".into())), "Pony");
    assert_eq!(
        mp.plugin_class(&RecordRef::Literal("Pony".into())),
        MountClass::Basic
    );
    assert_eq!(mp.plugin_name(&RecordRef::Record(DIR)), "");
}

#[test]
fn offered_lists_by_body() {
    let m = install();
    let mp = MountPresenter::new(&m);
    assert_eq!(mp.directory(true), vec![S_TALL, S_OTHER]);
    assert_eq!(mp.directory(false), vec![S_SHORT]);
    let trees = mp.war_steed_trees();
    assert_eq!(
        trees.iter().map(|t| t.tree).collect::<Vec<_>>(),
        vec![1, 3],
        "tree key order"
    );
    assert_eq!(trees[0].label.as_deref(), Some("MountedCombat_Heavy"));
    let tall = mp.offered(23, true);
    let shape: Vec<(usize, Did, OfferSource, bool)> = tall
        .iter()
        .map(|o| (o.index, o.skill, o.source, o.mount.is_some()))
        .collect();
    assert_eq!(
        shape,
        vec![
            (0, S_TALL, OfferSource::Directory, true),
            (1, S_OTHER, OfferSource::Directory, false),
            (2, S_STEED, OfferSource::WarSteed(1), true),
            (3, S_STEED, OfferSource::WarSteed(3), true),
        ]
    );
    assert_eq!(
        mp.offered(99, false).len(),
        1,
        "a species without war-steeds"
    );
}

#[test]
fn gait_multipliers() {
    let m = install();
    let g = MountPresenter::new(&m).gaits();
    assert!((g.speed_multiplier(4) - 0.5).abs() < f32::EPSILON);
    assert!((g.speed_multiplier(5) - 0.7).abs() < f32::EPSILON);
    assert!(
        (g.speed_multiplier(6) - 1.0).abs() < f32::EPSILON,
        "absent Canter: 1.0"
    );
    assert!(
        (g.speed_multiplier(2) - 1.0).abs() < f32::EPSILON,
        "neutral: 1.0"
    );
    assert!((g.turn_multiplier(7) - 0.47).abs() < f32::EPSILON);
    assert!((g.turn_multiplier(6) - 0.55).abs() < f32::EPSILON);
    assert!(
        (g.turn_multiplier(5) - 1.0).abs() < f32::EPSILON,
        "absent Trot pair: 1.0"
    );
    assert!((g.turn_multiplier(4) - 1.0).abs() < f32::EPSILON);
    assert!(
        (g.turn_multiplier(2) - 1.0).abs() < f32::EPSILON,
        "neutral takes the Walk pair"
    );
    assert_eq!(GaitTable::default().speed_multiplier(7), 1.0);
}

#[test]
fn effect_attributes_of_the_p5_revision() {
    use std::collections::BTreeMap;

    use crate::filter::{Compiled, Pred, apply_mount_types};
    use crate::query::{AttrType, Attribute, Family, attribute};

    assert_eq!(
        attribute(Family::Effect, "mount-type"),
        Some((Attribute::MountType, AttrType::Value))
    );
    assert_eq!(
        attribute(Family::Effect, "war-steed"),
        Some((Attribute::WarSteed, AttrType::YesNo))
    );
    let types: BTreeMap<u32, Option<u32>> = [
        (10, Some(0x1)),
        (11, Some(0x20)),
        (12, Some(0x9)),
        (13, None),
    ]
    .into_iter()
    .collect();
    let effects = [10, 11, 12, 13, 14];
    let mut c = vec![
        Compiled {
            attribute: Attribute::MountType,
            pred: Pred::Values(vec![1]),
        },
        Compiled {
            attribute: Attribute::WarSteed,
            pred: Pred::Bool(true),
        },
        Compiled {
            attribute: Attribute::WarSteed,
            pred: Pred::Bool(false),
        },
        Compiled {
            attribute: Attribute::MountType,
            pred: Pred::Bool(true),
        },
        Compiled {
            attribute: Attribute::Role,
            pred: Pred::Keywords(vec!["mount".into()]),
        },
    ];
    apply_mount_types(&mut c, &types, &effects);
    let members = |p: &Pred| match p {
        Pred::Members(s) => s.iter().copied().collect::<Vec<_>>(),
        _ => panic!("members expected"),
    };
    assert_eq!(members(&c[0].pred), vec![10, 12], "key 1 is bit 0");
    assert_eq!(members(&c[1].pred), vec![11, 12], "a bit of 0x38");
    assert_eq!(members(&c[2].pred), vec![10, 13, 14], "absent = no");
    assert_eq!(c[3].pred, Pred::Never, "a value of the wrong type");
    assert_eq!(
        c[4].pred,
        Pred::Keywords(vec!["mount".into()]),
        "other attributes unchanged"
    );
}
