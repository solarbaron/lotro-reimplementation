// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of the mount views and rules (`mounts.md` §2, §4.7, §5.1, §11).

use std::collections::HashMap;

use super::*;
use crate::gameplay::common::build::{array, did, entry, set, structure};
use crate::property_set::PropertySet;

fn labels() -> MountLabels {
    let m: HashMap<String, u32> = MountLabels::labels()
        .enumerate()
        .map(|(i, l)| (l.to_owned(), 0x3000_0000 + u32::try_from(i).unwrap()))
        .collect();
    MountLabels::resolve(&m)
}

fn pid(p: Pid) -> u32 {
    p.id().unwrap()
}

fn effect(l: &MountLabels, war_steed: bool) -> PropertySet {
    let p = &l.effect;
    let ev = |e: u32, c: i32| {
        entry(
            pid(p.durability_struct),
            structure(vec![
                entry(pid(p.event_id), PropertyValue::Enum(e)),
                entry(pid(p.durability_change), PropertyValue::Int(c)),
            ]),
        )
    };
    let mut e = vec![
        entry(pid(p.slot_entity), did(0x4700_0001)),
        entry(
            pid(p.slot_holding_location),
            PropertyValue::Enum(0x1000_0027),
        ),
        entry(pid(p.slot_yaw), PropertyValue::Float(0.0)),
        entry(pid(p.slot_behavior), PropertyValue::Enum(0x1000_0100)),
        entry(pid(p.animation_script), PropertyValue::Enum(0x1000_0200)),
        entry(pid(p.mount_type), PropertyValue::Bitfield32(0x1)),
        entry(pid(p.durability_max), PropertyValue::Int(200)),
        entry(pid(p.durability_regen), PropertyValue::Float(1.0)),
        entry(pid(p.slot_scale), PropertyValue::Vector3([0.925; 3])),
    ];
    if war_steed {
        e.push(entry(pid(p.use_appearances), PropertyValue::Bool(true)));
    } else {
        e.push(entry(
            pid(p.durability_events),
            array(vec![ev(22, -30), ev(17, -90), ev(31, -180)]),
        ));
    }
    set(e)
}

#[test]
fn mount_type_bits() {
    // §11.1: 0x4004 is keys 3 (Goat) and 15 (Boar), not a war-steed type.
    let t = MountType(0x4004);
    assert_eq!(t.keys().collect::<Vec<_>>(), vec![3, 15]);
    assert!(!t.is_war_steed());
    assert_eq!(controller_flags(t), (false, false));
    assert!(MountType(0x10).is_war_steed() && MountType(0x21).is_war_steed());
    assert_eq!(MountType::of_key(4), MountType(0x8));
    assert_eq!(MountType::of_key(0), MountType(0));
    assert_eq!(MountType::of_key(40), MountType(0));
    let labels = |k: u32| (k == 3).then_some("Goat");
    assert_eq!(
        t.labels(labels),
        vec!["Goat".to_owned(), "key 15".to_owned()]
    );
    // Areas: 0x1 refuses with B, 0x4004 allows, 0 refuses with B; type 0 refuses with A.
    let out = |a| MountPlace::Outdoors { area: Some(a) };
    assert_eq!(
        check_application(true, 0, t, out(0x1)),
        Err(MountRefusal::B)
    );
    assert_eq!(check_application(true, 0, t, out(0x4004)), Ok(()));
    assert_eq!(check_application(true, 0, t, out(0)), Err(MountRefusal::B));
    assert_eq!(
        check_application(true, 0, MountType(0), out(0x3C47D)),
        Err(MountRefusal::A)
    );
    // In water, without an area, or as a non-agent.
    assert_eq!(
        check_application(true, 2, t, out(0x4004)),
        Err(MountRefusal::A)
    );
    assert_eq!(
        check_application(true, 0, t, MountPlace::Outdoors { area: None }),
        Err(MountRefusal::A)
    );
    assert_eq!(
        check_application(false, 0, t, out(0x4004)),
        Err(MountRefusal::NotPlayerOrAgent)
    );
    // Dungeons: checked after the area; a dungeon without the property refuses with A.
    let cell = |d| MountPlace::InCell {
        area: Some(0x4004),
        dungeon: d,
    };
    assert_eq!(check_application(true, 0, t, cell(None)), Ok(()));
    assert_eq!(
        check_application(true, 0, t, cell(Some(Some(0)))),
        Err(MountRefusal::B)
    );
    assert_eq!(
        check_application(true, 0, t, cell(Some(None))),
        Err(MountRefusal::A)
    );
}

#[test]
fn effect_groups_and_rider_entries() {
    let l = labels();
    let d = effect(&l, false);
    let e = MountEffect::recognise(&l, &d).unwrap();
    assert_eq!(e.group(), MountGroup::Durability);
    assert_eq!(e.mount_type(), Some(MountType(1)));
    assert_eq!(e.durability_events().len(), 3);
    assert!(e.is_hobby_horse(|k| (k == 0x1000_0200).then_some("Emote_Holding_HobbyHorse")));
    assert!(!e.is_startup_record(|_| Some("Mounted_Monster_Startup")));
    let r = e.rider();
    assert_eq!(r.entity, Some(Did(0x4700_0001)));
    assert_eq!(r.scale, Some([0.925; 3]));
    // Decision §4.1: the rider fields plus the type and status 0.
    let entries = e.rider_entries();
    let has = |p: Pid| entries.iter().any(|x| p.is(x.pid));
    assert!(has(l.effect.slot_entity) && has(l.effect.slot_scale));
    assert!(has(l.state.current_mount_type) && has(l.state.current_mount_status));
    assert!(!has(l.effect.durability_max));
    let w = effect(&l, true);
    assert_eq!(
        MountEffect::recognise(&l, &w).unwrap().group(),
        MountGroup::WarSteed
    );
    assert!(MountEffect::recognise(&l, &set(vec![])).is_none());
    assert_eq!(MountStatus::on_removal(true), 7);
    assert_eq!(MountStatus::on_removal(false), 6);
}

#[test]
fn target_fields_win() {
    let l = labels();
    let p = &l.effect;
    let template = set(vec![
        entry(pid(p.slot_entity), did(0x4700_0009)),
        entry(pid(p.mount_type), PropertyValue::Bitfield32(0x20)),
        entry(
            pid(p.slot_bone_offset),
            PropertyValue::Vector3([0.0, 0.0, 0.2]),
        ),
    ]);
    let t = TargetMountFields::read(&l, &template);
    let (r, ty) = t.apply(&RiderFields::default(), None);
    assert_eq!(r.entity, Some(Did(0x4700_0009)));
    assert_eq!(r.bone_offset, Some([0.0, 0.0, 0.2]));
    assert!(ty.is_war_steed());
    let (_, ty) = TargetMountFields::default().apply(&RiderFields::default(), Some(MountType(4)));
    assert_eq!(ty, MountType(4));
}

#[test]
fn normal_mount_speed() {
    // §11.5: 1.0 + 0.62 = 1.62; with a bonus of 0.75, 1.75.
    let m = forwarded_mount_speed(Some(1.0), 0.62, None);
    assert!((m - 1.62).abs() < 1e-6);
    assert!((forwarded_mount_speed(None, 0.62, Some(0.75)) - 1.75).abs() < 1e-6);
    let f = normal_mount_speed_multiplier(Some(1.0), Some(m));
    assert!((6.5 * f - 10.53).abs() < 1e-4);
    assert!((normal_mount_speed_multiplier(None, None) - 1.0).abs() < f32::EPSILON);
}

#[test]
fn durability_decision() {
    // §11.6: 200 → 170 → 80, 30 s → 110, super-critical → 0: dismounted.
    let l = labels();
    let d = effect(&l, false);
    let e = MountEffect::new(&l, &d);
    let mut s = Durability::new(&e).unwrap();
    assert!(!s.on_event(22));
    assert!((s.current - 170.0).abs() < 1e-6);
    assert!(!s.on_event(17));
    s.regenerate(30.0);
    assert!((s.current - 110.0).abs() < 1e-6);
    assert!(s.on_event(31));
    assert!(s.current.abs() < f32::EPSILON);
}

#[test]
fn templates_and_riders() {
    let l = labels();
    let st = &l.stats;
    let s = set(vec![
        entry(pid(st.max_speed), PropertyValue::Float(6.5)),
        entry(pid(st.turn_rate_cached), PropertyValue::Float(100.0)),
        entry(
            pid(st.init_effects),
            array(vec![entry(
                1,
                structure(vec![entry(pid(st.startup_effect_id), did(0x7003_3562))]),
            )]),
        ),
        entry(pid(l.state.rider_type), PropertyValue::Enum(6)),
    ]);
    let t = TemplateMountStats::read(st, &s);
    assert_eq!(t.max_speed, Some(6.5));
    assert_eq!(t.cached[2], Some(100.0));
    assert_eq!(startup_effects(st, &s), vec![Did(0x7003_3562)]);
    assert_eq!(RiderInstance::read(&l, &s).unwrap().rider_type, Some(6));
    assert!(RiderInstance::read(&l, &set(vec![])).is_none());
    let defaults: std::collections::BTreeMap<u32, PropertyValue> =
        [(pid(st.world_max_speed), PropertyValue::Float(16.0))]
            .into_iter()
            .collect();
    assert_eq!(default_world_mounted_max_speed(st, &defaults), Some(16.0));
}

/// §11.4 (revised 2026-10-09): a synthetic effect scale (0.9, 1.0, 1.1) scales the mount per
/// axis; with `UseParentScale` 1 and a rider final scale of (0.7, 0.7, 0.7) the mount is
/// drawn at (0.63, 0.7, 0.77).
#[test]
fn mount_scale_is_per_axis() {
    let mut mesh = MountMesh {
        entity: Did(0x4700_0001),
        holding_location: 1,
        bone_offset: [0.0; 3],
        scale: [0.9, 1.0, 1.1],
        scale_driven: true,
        use_parent_scale: false,
        behavior: 0,
        table: Did(0x0700_0001),
        node_id: 1,
    };
    assert_eq!(mesh.draw_scale([0.7; 3]), [0.9, 1.0, 1.1]);
    mesh.use_parent_scale = true;
    let s = mesh.draw_scale([0.7; 3]);
    for (got, want) in s.iter().zip([0.63f32, 0.7, 0.77]) {
        assert!((got - want).abs() < 1e-6, "{s:?}");
    }
    let t = mesh.local_transform([0.7; 3]);
    assert!((t[0] - 0.63).abs() < 1e-6 && (t[10] - 0.77).abs() < 1e-6);
}
