// SPDX-License-Identifier: GPL-3.0-or-later
//! Mount views (`tessera_formats::gameplay::mounts`, `docs/specs/mounts.md`) and the
//! war-steed controller (`tessera_physics::controller::warsteed`, §5.3–§5.7) on arbitrary
//! bytes.
//!
//! Byte 0 selects the mode. Bit 0 clear: the rest is an embedded property set decoded with
//! a total synthetic schema (type = PID mod 24) whose labels carry the types §2 and §6 give
//! them; every view of the module reads it (effect, rider fields, durability, gear and
//! appearance lists, the rider entries of the Decision of §4.1, template stats, riders,
//! startup and toggled effects), the applicability check runs with masks from the input,
//! and the durability rule runs over the events. Bit 0 set: the rest is read as floats and
//! flags for the war-steed controller, which steps up to 64 updates, and for the mount mesh
//! of §4.4 (choice, draw scale and local transform).
//!
//! Nothing may panic. The rider entries never repeat a PID; a refused check never comes
//! from an allowed mask; durability stays within [0, maximum]; the controller's forward
//! velocity never exceeds the limit of §5.4 step 3 nor its sideways velocity −*S*1 … *S*2
//! (MI13), and every output is finite.
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run formats_mounts -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::collections::{HashMap, HashSet};

use libfuzzer_sys::fuzz_target;
use tessera_formats::gameplay::mounts::{
    Durability, MountEffect, MountLabels, MountMesh, MountPlace, MountType, RiderInstance,
    TargetMountFields, TemplateMountStats, check_application, controller_flags,
    forwarded_mount_speed, mask_check, normal_mount_speed_multiplier, startup_effects,
    toggled_effects,
};
use tessera_formats::script_table::MeshFxNode;
use tessera_formats::script_table::resolve::{MeshFxCandidate, MeshFxDriven};
use tessera_formats::{Did, PropertySet, PropertyType};
use tessera_physics::controller::MovementParams;
use tessera_physics::controller::warsteed::{
    GaitMultipliers, StepInput, WarSteedActor, WarSteedParams, speed_limit, step,
};

fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

fn spec_type(label: &str) -> u32 {
    match label {
        l if l.ends_with("Array")
            || l.ends_with("Hash")
            || l.ends_with("Appearances")
            || l.ends_with("List") =>
        {
            12
        }
        l if l.ends_with("Structure")
            || l.ends_with("Pair")
            || l.ends_with("Appearance") && l.starts_with("MeshFX") =>
        {
            11
        }
        l if l.ends_with("_Entity")
            || l.ends_with("Icon")
            || l.ends_with("EffectDID")
            || l.ends_with("AppearanceID")
            || l.ends_with("EffectID")
            || l.ends_with("Toggle_Effect") =>
        {
            20
        }
        l if l.ends_with("Scale") || l.ends_with("BoneOffset") => 6,
        l if l.ends_with("Mount_Type")
            || l.ends_with("MountType")
            || l.ends_with("Allowed_Mount_Types") =>
        {
            18
        }
        l if l.ends_with("Name_Initial") => 13,
        l if l.ends_with("Base_Max")
            || l.ends_with("DurabilityChange")
            || l.ends_with("PipMaxModifier")
            || l.ends_with("Current_Max") =>
        {
            15
        }
        l if l.contains("Yaw")
            || l.contains("Pitch")
            || l.contains("Roll")
            || l.contains("Speed")
            || l.contains("Rate")
            || l.contains("Acceleration")
            || l.contains("Modifier")
            || l.contains("AddMod")
            || l.contains("MaxLevel")
            || l.contains("Multiplier") =>
        {
            9
        }
        l if l.starts_with("Mount_Is")
            || l.ends_with("Appearances")
            || l.ends_with("Defeat")
            || l.ends_with("Update")
            || l.ends_with("Controller")
            || l.ends_with("Enabled")
            || l.ends_with("Momentum")
            || l.ends_with("Rotation")
            || l.ends_with("State")
            || l.ends_with("MeshFX")
            || l.ends_with("ShouldUseAppearances") =>
        {
            21
        }
        _ => 8,
    }
}

fn f(bytes: &[u8], i: usize) -> f32 {
    let mut b = [0u8; 4];
    for (k, x) in b.iter_mut().enumerate() {
        *x = bytes.get(i * 4 + k).copied().unwrap_or(0);
    }
    f32::from_le_bytes(b)
}

fn opt(bytes: &[u8], i: usize) -> Option<f32> {
    let v = f(bytes, i);
    (bytes.get(i * 4).copied().unwrap_or(0) % 3 != 0).then_some(v)
}

fn controller(rest: &[u8]) {
    let layered = MovementParams {
        forward_speed: f(rest, 0),
        backward_speed: f(rest, 1).abs(),
        strafe_left_speed: f(rest, 2).abs(),
        strafe_right_speed: f(rest, 3).abs(),
        control_acceleration: [f(rest, 4), f(rest, 5), f(rest, 6)],
        ..MovementParams::default()
    };
    let flag = |i: usize| rest.get(i).map(|b| b & 1 != 0);
    let actor = WarSteedActor {
        max_speed_cached: opt(rest, 7),
        acceleration_cached: opt(rest, 8),
        turn_rate_cached: opt(rest, 9),
        gear_change_rate_cached: opt(rest, 10),
        speed_multiplier: opt(rest, 11),
        acceleration_multiplier: opt(rest, 12),
        gait: rest.get(52).map(|b| u32::from(*b % 9)),
        locked_gear: rest
            .get(53)
            .filter(|b| **b % 2 == 0)
            .map(|b| u32::from(*b % 9)),
        lose_momentum: flag(54),
        freeze_rotation: flag(55),
        active_movement_frozen: flag(56),
        player_movement_frozen: flag(57),
    };
    let p = WarSteedParams::build(&layered, &actor);
    let g = GaitMultipliers {
        gallop_speed: f(rest, 15),
        locked_gear_speed: opt(rest, 16),
        turn: f(rest, 17),
    };
    let world = f(rest, 18);
    let limit = speed_limit(&p, &g, world);
    let mut v = [f(rest, 19), f(rest, 20)];
    for k in 0..64usize {
        let input = [f(rest, 21 + k % 4), f(rest, 25 + k % 4)];
        let s = step(
            &p,
            &actor,
            &g,
            world,
            StepInput {
                input,
                turn_input: f(rest, 29),
                velocity: v,
                seconds: f(rest, 30).abs().min(1.0),
            },
        );
        assert!(s.velocity.iter().all(|c| c.is_finite()));
        assert!(s.mean.iter().all(|c| c.is_finite()));
        assert!(s.turn_degrees.is_finite());
        let sane = [p.forward, p.backward, p.strafe_1, p.strafe_2, limit]
            .iter()
            .all(|x| x.is_finite());
        if sane && -p.backward <= limit && -p.strafe_1 <= p.strafe_2 {
            assert!(s.velocity[1] <= limit && s.velocity[1] >= -p.backward);
            assert!(s.velocity[0] <= p.strafe_2 && s.velocity[0] >= -p.strafe_1);
        }
        v = s.velocity;
    }
}

/// The mount mesh of §4.4 from MeshFX candidates built of the input's floats: the chosen
/// node is one started under the mesh script with a non-null entity, and its draw scale and
/// local transform are finite (the scale positive) whatever the stored values.
fn mount_mesh(rest: &[u8]) {
    let f = |i: usize| {
        rest.get(i * 4..i * 4 + 4)
            .map_or(0.0, |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let flag = |i: usize| rest.get(i).is_some_and(|b| b & 1 != 0);
    let candidates: Vec<MeshFxCandidate> = (0..3)
        .map(|k| {
            let base = k * 8;
            let mut node = MeshFxNode {
                entity: Did(u32::from(flag(base)) * 0x4700_0001),
                attach_to_parent: true,
                allow_physics: false,
                fade_in_time: 0.25,
                life_time: 0.25,
                fade_out_time: 0.25,
                destroy_time: 0.25,
                holding_location: k as u32,
                bone_offset: [f(base), f(base + 1), f(base + 2)],
                offset_rotation_yaw: 0.0,
                offset_rotation_pitch: 0.0,
                offset_rotation_roll: 0.0,
                remove_rotation: false,
                bone_type: 0,
                behavior: 0,
                persistent: true,
                on_water: false,
                use_parent_scale: flag(base + 1),
                use_particle_scale: false,
                ignore_child_scale: true,
                scale: [f(base + 3), f(base + 4), f(base + 5)],
                appearances: Vec::new(),
                mesh_iid_target_property: 0,
            };
            if flag(base + 2) {
                node.entity = Did(0);
            }
            MeshFxCandidate {
                node,
                driven: MeshFxDriven::default(),
                table: Did(0x0700_0001),
                scriptlet: Did(0),
                node_id: k as u32,
                scripts: if flag(base + 3) { vec![7] } else { Vec::new() },
            }
        })
        .collect();
    if let Some(m) = MountMesh::choose(&candidates, 7) {
        assert!(!m.entity.is_null());
        let c = &candidates[m.node_id as usize];
        assert!(c.scripts.contains(&7));
        let parent = [f(30), f(31), f(32)];
        assert!(
            m.draw_scale(parent)
                .iter()
                .all(|v| v.is_finite() && *v > 0.0)
        );
        assert!(m.local_transform(parent).iter().all(|v| v.is_finite()));
    }
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    if mode & 1 != 0 {
        controller(rest);
        mount_mesh(rest);
        return;
    }
    let Ok(set) = PropertySet::parse(rest, &total) else {
        return;
    };
    let map: HashMap<String, u32> = MountLabels::labels()
        .enumerate()
        .map(|(i, l)| {
            let i = u32::try_from(i).unwrap_or(0);
            (l.to_owned(), (i + 1) * 24 + spec_type(l))
        })
        .collect();
    let labels = MountLabels::resolve(&map);
    let e = MountEffect::new(&labels, &set);
    let _ = (
        MountEffect::recognise(&labels, &set).is_some(),
        e.group(),
        e.name(),
    );
    let _ = (
        e.rider(),
        e.mount_type(),
        e.durability_events(),
        e.gear_modifiers(),
        e.appearances(),
    );
    let _ = (
        e.is_hobby_horse(|_| Some("Emote_Holding_HobbyHorse")),
        e.is_startup_record(|_| None),
    );
    let entries = e.rider_entries();
    let pids: HashSet<u32> = entries.iter().map(|x| x.pid).collect();
    assert_eq!(pids.len(), entries.len());
    if let Some(mut d) = Durability::new(&e) {
        for ev in [22, 17, 31, u32::from(*mode)] {
            let _ = d.on_event(ev);
            d.regenerate(f32::from(*mode));
            assert!(d.current >= 0.0 && d.current <= d.max);
        }
    }
    let target = TargetMountFields::read(&labels, &set);
    let (_, ty) = target.apply(&e.rider(), e.mount_type());
    let _ = controller_flags(ty);
    let mask = u32::from_le_bytes([*mode, rest.first().copied().unwrap_or(0), 0, 0]);
    let checked = check_application(
        true,
        u32::from(*mode % 5),
        ty,
        MountPlace::InCell {
            area: Some(mask),
            dungeon: Some(Some(mask)),
        },
    );
    if mask & ty.0 != 0 && !matches!(*mode % 5, 1..=3) {
        assert!(checked.is_ok());
    }
    let _ = mask_check(mask, MountType(u32::from(*mode)));
    let _ = (
        TemplateMountStats::read(&labels.stats, &set),
        RiderInstance::read(&labels, &set),
    );
    let _ = (
        startup_effects(&labels.stats, &set),
        toggled_effects(&labels.stats, &set),
    );
    let _ = forwarded_mount_speed(None, f32::from(*mode), None);
    assert!(normal_mount_speed_multiplier(Some(f32::NAN), None).is_finite());
});
