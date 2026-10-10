// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests for script tables (0x07), scriptlets (0x0C) and clip selection
//! (`docs/specs/script-table.md`). Every buffer is built here; values are invented.

mod common;

use std::collections::HashMap;
use std::convert::Infallible;

use common::B;
use tessera_formats::script_table::resolve::{
    AnimationFieldPids, ClipResolver, EntityScripts, ImpulseFieldPids, MotionInputs, MotionState,
    MovementStyle, PropertyState, ResolveIssue, ResolveLimits, ScriptVocabulary,
};
use tessera_formats::script_table::{
    MAX_NODE_DEPTH, NodeData, NodeKind, PortPayload, PortType, TargetData, TargetValue,
};
use tessera_formats::{
    AnyRecord, Did, FormatError, NoSchema, PropertyType, PropertyValue, ScriptTable, Scriptlet,
    decode_script_table, decode_scriptlet,
};

/// The worked example of `script-table.md` §13 (165 bytes).
const SPEC_TABLE: &str = "
02 00 00 00 00 00 01 05 00 00 00 01 24 34 12 00
05 01 01 00 00 00 cd cc 4c 3e cd cc 4c 3e 0a 00
00 00 01 00 00 00 00 00 00 00 00 00 00 00 80 3f
00 00 00 80 3f 00 00 80 3f 00 00 00 00 00 00 00
40 00 00 00 00 00 01 00 01 00 01 00 00 01 00 00
80 3f 00 00 00 00 00 00 80 3f 0c 00 00 00 01 00
00 00 06 00 00 00 3f 06 00 00 00 01 02 01 00 01
00 00 01 00 00 80 3f 00 00 00 00 00 00 80 3f 0d
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
01 05 00 00 00 00 00 00 01 05 00 00 00 01 00 00
00 34 12 00 05";

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

#[test]
fn spec_example_table_decodes() {
    let data = hex(SPEC_TABLE);
    assert_eq!(data.len(), 165);
    let t = decode_script_table(&data, &NoSchema).unwrap();
    assert_eq!(t.kind, 2);
    assert!(t.includes.is_empty());
    assert_eq!(t.scripts.len(), 1);
    let s = t.script(5).unwrap();
    assert_eq!(s.roots.len(), 1);
    let n = &s.roots[0];
    assert_eq!(n.kind, NodeKind::ANIMATION);
    let a = n.animation().unwrap();
    assert_eq!(a.asset, Did(0x0500_1234));
    assert!(a.loop_forever);
    assert_eq!(a.loop_count, 1);
    assert!((a.ramp_in - 0.2).abs() < 1e-6 && (a.ramp_out - 0.2).abs() < 1e-6);
    assert_eq!(a.layer, 10);
    assert!(a.use_bone_opacity && !a.manual_update && !a.randomize_start_time);
    assert_eq!((a.start_time, a.end_time), (0.0, 1.0));
    assert_eq!((a.speed, a.speed_multiplier), (1.0, 1.0));
    assert_eq!(a.displacement, [0.0, 2.0, 0.0]);
    assert!(!a.detach_on_ramp_out && a.synchronize_timing && !a.requires_updating_when_culled);
    assert_eq!(n.base.node_id, 12);
    assert!(n.base.use_parent_intensity);
    assert_eq!(n.base.probability, 1.0);
    assert_eq!(n.ports.len(), 1);
    assert_eq!(n.ports[0].port_type, PortType::DEFAULT);
    assert_eq!(n.ports[0].payload, PortPayload::F32(0.5));
    assert_eq!(n.event_times().collect::<Vec<_>>(), vec![0.5]);
    let child = &n.ports[0].children[0];
    assert_eq!(child.kind, NodeKind::COLLECTION);
    assert_eq!(child.base.node_id, 13);
    assert!(n.targets.is_empty());
    assert!(t.script_properties(5).unwrap().is_empty());
    assert_eq!(t.script_dids(5), Some(&[Did(0x0500_1234)][..]));
    assert_eq!(t.first_unusual_field(), None);
    let c = t.census();
    assert_eq!((c.nodes, c.animations, c.ports, c.max_depth), (2, 1, 1, 2));

    // Through the router, the DID is kept.
    let any = AnyRecord::decode_with_schema(0x0700_0042u32, &data, &NoSchema)
        .unwrap()
        .unwrap();
    assert_eq!(any.did(), Did(0x0700_0042));
    assert_eq!(any.type_name(), "script table");
    assert!(AnyRecord::is_decodable_with_schema(0x0C00_0001u32));
    assert!(!AnyRecord::is_decodable(0x0700_0001u32));
    assert!(AnyRecord::decode(0x0700_0042u32, &data).unwrap().is_none());
}

#[test]
fn spec_example_port_echo_mismatch_is_malformed() {
    let mut data = hex(SPEC_TABLE);
    // §13 says byte 0x77; the echo of the animation node's port is at 0x67 (OPEN noted in
    // the spec): 0x5E port count, 0x62 port type, 0x63 payload, 0x67 echo.
    assert_eq!(data[0x67], 0x06);
    data[0x67] = 0x07;
    let e = decode_script_table(&data, &NoSchema).unwrap_err();
    assert!(matches!(e, FormatError::Malformed { .. }), "{e:?}");
}

#[test]
fn spec_example_scriptlet_decodes() {
    let table = hex(SPEC_TABLE);
    // 0x01 roots, then the kind byte, the 59 data bytes and the 22 base bytes of the
    // animation node (offsets 12..94 of the table), no ports, no targets, one dependency.
    let mut data = vec![0x01];
    data.extend_from_slice(&table[12..12 + 1 + 59 + 22]);
    data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0x34, 0x12, 0x00, 0x05]);
    assert_eq!(data.len(), 99);
    let s = decode_scriptlet(&data, &NoSchema).unwrap();
    assert_eq!(s.roots.len(), 1);
    assert_eq!(s.roots[0].animation().unwrap().asset, Did(0x0500_1234));
    assert_eq!(s.dependencies, vec![Did(0x0500_1234)]);
    assert_eq!(s.first_unusual_field(), None);
    let s = Scriptlet::parse_expecting(&data, 0x0C00_0007u32, &NoSchema).unwrap();
    assert_eq!(s.did, Did(0x0C00_0007));
    assert!(matches!(
        Scriptlet::parse_expecting(&data, 0x0700_0007u32, &NoSchema),
        Err(FormatError::WrongRecordType { .. })
    ));
    // Truncations never panic.
    for len in 0..data.len() {
        assert!(decode_scriptlet(&data[..len], &NoSchema).is_err(), "{len}");
    }
    let mut longer = data.clone();
    longer.push(0);
    assert!(matches!(
        decode_scriptlet(&longer, &NoSchema),
        Err(FormatError::TrailingBytes { .. })
    ));
}

// --- builders ---

const PID_STYLE: u32 = 0x1000_0101;
const PID_PROPEL: u32 = 0x1000_0102;
const PID_STRAFE: u32 = 0x1000_0103;
const PID_TURN: u32 = 0x1000_0104;
const PID_ELEMENT: u32 = 0x1000_0105;
const PID_SPEEDMULT: u32 = 0x1000_0106;
const PID_CLASS_TABLE: u32 = 0x1000_0110;
const PID_DB_TABLE: u32 = 0x1000_0111;
const PID_STARTUP_NAMES: u32 = 0x1000_0112;
const PID_SCRIPT_ID: u32 = 0x1000_0113;
const PID_ANIM_SPEED: u32 = 0x1000_0120;
const PID_ANIM_DISP: u32 = 0x1000_0121;
const PID_MOOD: u32 = 0x1000_0130;
const PID_ADDITIONAL: u32 = 0x1000_0114;
const PID_PHYS_OBJ: u32 = 0x1000_0115;
/// An Int property with a dictionary default (7) that no entity sets.
const PID_DEFAULTED: u32 = 0x1000_0140;
/// An Int-array property (SetProperty with a large value).
const PID_BIG: u32 = 0x1000_0141;
const PID_BIG_ELEMENT: u32 = 0x1000_0142;
/// `Script_CombinationType` (an enum; `Overriding` = 1, `Additive` = 2; §8.2).
const PID_COMBINATION: u32 = 0x1000_0143;
const PID_FINAL_ELEMENT: u32 = 0x1000_0160;
const PID_AIR_TIME: u32 = 0x1000_0161;
const PID_JUMP_MOD: u32 = 0x1000_0162;
const PID_IS_JUMPING: u32 = 0x1000_0163;
// Mount mesh (`mounts.md` §4.4, §11.4): the rider's fields and the MeshFX fields they drive.
const PID_MOUNT_TYPE: u32 = 0x1000_0150;
const PID_SPECIES: u32 = 0x1000_0151;
const PID_SLOT_ENTITY: u32 = 0x1000_0152;
const PID_SLOT_SCALE: u32 = 0x1000_0153;
const PID_FX_ENTITY: u32 = 0x1000_0154;
const PID_FX_SCALE: u32 = 0x1000_0155;
// Face compositing and colour edits (`appearance.md` §3.5).
const PID_LOOK_OPTION: u32 = 0x1000_0170;
const PID_LOOK_SKIN: u32 = 0x1000_0171;
const PID_LOOK_TEXTURE: u32 = 0x1000_0172;
const PID_EDIT_COLOR: u32 = 0x1000_0173;
// The figure's placement and the start-up flag (`appearance.md` §3.5).
const PID_IN_WORLD: u32 = 0x1000_0174;
const PID_SERVER_FLAG: u32 = 0x1000_0175;
// Compositor field PIDs (`appearance.md` §3.5 "Field names").
const PID_FIELD_IMAGE: u32 = 0x1000_0999;
const PID_FIELD_COLOUR: u32 = 0x1000_0998;
const STARTUP: u32 = 5;
const WALKING: u32 = 1;
const RUNNING: u32 = 2;

fn schema(pid: u32) -> Option<PropertyType> {
    Some(match pid {
        PID_STYLE | PID_ELEMENT | PID_SCRIPT_ID | PID_MOOD => PropertyType::Enum,
        PID_DEFAULTED | PID_BIG_ELEMENT => PropertyType::Int,
        PID_BIG | PID_ADDITIONAL => PropertyType::Array,
        PID_PHYS_OBJ => PropertyType::DidRef,
        PID_PROPEL | PID_STRAFE | PID_TURN | PID_SPEEDMULT | PID_ANIM_SPEED => PropertyType::Float,
        PID_ANIM_DISP => PropertyType::Vector3,
        PID_CLASS_TABLE => PropertyType::Struct,
        PID_DB_TABLE => PropertyType::DidRef,
        PID_STARTUP_NAMES => PropertyType::Array,
        PID_COMBINATION => PropertyType::Enum,
        PID_FINAL_ELEMENT => PropertyType::Enum,
        PID_JUMP_MOD | PID_AIR_TIME => PropertyType::Float,
        PID_IS_JUMPING => PropertyType::Bool,
        PID_MOUNT_TYPE | PID_SPECIES => PropertyType::Enum,
        PID_SLOT_ENTITY => PropertyType::DidRef,
        PID_SLOT_SCALE => PropertyType::Vector3,
        PID_LOOK_OPTION | PID_IN_WORLD | PID_SERVER_FLAG => PropertyType::Enum,
        PID_LOOK_SKIN => PropertyType::Colour,
        PID_LOOK_TEXTURE => PropertyType::Int,
        _ => return None,
    })
}

fn vocab() -> ScriptVocabulary {
    ScriptVocabulary {
        class_script_table: PID_CLASS_TABLE,
        db_script_table: PID_DB_TABLE,
        startup_script_names: Some(PID_STARTUP_NAMES),
        additional_script_tables: Some(PID_ADDITIONAL),
        phys_obj: Some(PID_PHYS_OBJ),
        startup_key: STARTUP,
        movement_style: PID_STYLE,
        style_walking: WALKING,
        style_running: RUNNING,
        slider_propel: PID_PROPEL,
        slider_strafe: PID_STRAFE,
        slider_turn: PID_TURN,
        entity_element: Some(PID_ELEMENT),
        element_ground: Some(5),
        element_no_surface: Some(4),
        speed_multiplier: Some(PID_SPEEDMULT),
        animation: AnimationFieldPids {
            speed: Some(PID_ANIM_SPEED),
            displacement: Some(PID_ANIM_DISP),
            ..AnimationFieldPids::default()
        },
        ..ScriptVocabulary::default()
    }
}

/// A base block with node ID `id`, probability `p` and driven pairs.
fn base_full(id: u32, p: f32, driven: &[(u32, u32)]) -> B {
    let mut b = B::new().tsize(1, driven.len() as u32);
    for &(f, s) in driven {
        b = b.u32(f).u32(s);
    }
    b.tsize(1, 0).u8(0).u8(1).f32(1.0).f32(0.0).f32(p).u32(id)
}

fn base(id: u32) -> B {
    base_full(id, 1.0, &[])
}

/// A port: type, optional payload (followed by its echo) and children.
fn port(ty: u8, payload: Option<B>, children: &[Vec<u8>]) -> Vec<u8> {
    let mut b = B::new().u8(ty);
    if let Some(p) = payload {
        b = b.bytes(&p.build()).u32(u32::from(ty));
    }
    b = b.u8(children.len() as u8);
    for c in children {
        b = b.bytes(c);
    }
    b.build()
}

fn node(kind: u8, data: B, base: B, ports: &[Vec<u8>]) -> Vec<u8> {
    let mut b = B::new()
        .u8(kind)
        .bytes(&data.build())
        .bytes(&base.build())
        .u32(ports.len() as u32);
    for p in ports {
        b = b.bytes(p);
    }
    b.u32(0).build()
}

fn anim_data(clip: u32, layer: u32, disp_y: f32) -> B {
    B::new()
        .u32(clip)
        .u8(1) // loop forever
        .u32(1)
        .f32(0.2)
        .f32(0.2)
        .u32(layer)
        .u8(1)
        .u8(0)
        .f32(0.0)
        .f32(0.0)
        .f32(1.0)
        .u8(0)
        .f32(1.0)
        .f32(1.0)
        .f32(0.0)
        .f32(disp_y)
        .f32(0.0)
        .u8(0)
        .u8(1)
        .u8(0)
}

fn anim(id: u32, clip: u32, layer: u32, disp_y: f32, driven: &[(u32, u32)]) -> Vec<u8> {
    node(
        36,
        anim_data(clip, layer, disp_y),
        base_full(id, 1.0, driven),
        &[],
    )
}

fn collection(id: u32, children: &[Vec<u8>]) -> Vec<u8> {
    node(
        2,
        B::new(),
        base(id),
        &[port(6, Some(B::new().u32(1)), children)],
    )
}

/// An empty embedded property set, or one with entries (PID, echo, value bytes).
fn pset(entries: &[(u32, B)]) -> B {
    let mut b = B::new().tsize(1, entries.len() as u32);
    for (pid, v) in entries {
        b = b.u32(*pid).u32(*pid).bytes(&v.0);
    }
    b
}

fn scriptlet_node(id: u32, file: u32, params: B, bindings: &[(&str, Vec<Vec<u8>>)]) -> Vec<u8> {
    let ports: Vec<Vec<u8>> = bindings
        .iter()
        .map(|(name, children)| port(6, Some(B::new().pascal(name)), children))
        .collect();
    node(
        19,
        B::new().u32(file).bytes(&params.build()),
        base(id),
        &ports,
    )
}

fn param_node(id: u32, name: &str) -> Vec<u8> {
    // Default port holding a placeholder Scriptlet node with file 0 (§7.8).
    let placeholder = scriptlet_node(id + 1000, 0, pset(&[]), &[]);
    node(
        22,
        B::new().pascal(name),
        base(id),
        &[port(6, None, &[placeholder])],
    )
}

fn switch_case(values: &[(u32, u32)], children: &[Vec<u8>]) -> Vec<u8> {
    let mut p = B::new().u8(values.len() as u8);
    for &(pid, v) in values {
        p = p.u32(pid).u32(v);
    }
    let p = p.u8(0).u32(0).u32(3).f32(0.2).f32(0.0);
    port(5, Some(p), children)
}

fn switch(id: u32, property: u32, ports: &[Vec<u8>]) -> Vec<u8> {
    node(
        12,
        B::new().u32(property).u8(1).u8(0).u8(0),
        base(id),
        ports,
    )
}

fn default_port_pair(children: &[Vec<u8>]) -> Vec<u8> {
    port(6, Some(B::new().f32(0.0).f32(0.0)), children)
}

fn scalar_blend(id: u32, property: u32, anchors: &[(f32, Vec<Vec<u8>>)]) -> Vec<u8> {
    let ports: Vec<Vec<u8>> = anchors
        .iter()
        .map(|(a, c)| port(6, Some(B::new().f32(*a)), c))
        .collect();
    node(57, B::new().u32(property).u8(1), base(id), &ports)
}

fn random(id: u32, weighted: &[(u32, Vec<Vec<u8>>)]) -> Vec<u8> {
    let ports: Vec<Vec<u8>> = weighted
        .iter()
        .map(|(w, c)| port(6, Some(B::new().u32(*w)), c))
        .collect();
    node(17, B::new().u8(1).u8(0), base(id), &ports)
}

fn table(kind: u32, includes: &[u32], scripts: &[(u32, Vec<Vec<u8>>)]) -> Vec<u8> {
    let mut b = B::new().u32(kind).vle(includes.len() as u32);
    for i in includes {
        b = b.u32(*i);
    }
    b = b.tsize(1, scripts.len() as u32);
    for (key, roots) in scripts {
        b = b.u32(*key).vle(roots.len() as u32);
        for r in roots {
            b = b.bytes(r);
        }
    }
    b.tsize(1, 0).tsize(1, 0).build()
}

fn scriptlet(roots: &[Vec<u8>], deps: &[u32]) -> Vec<u8> {
    let mut b = B::new().vle(roots.len() as u32);
    for r in roots {
        b = b.bytes(r);
    }
    b = b.u32(deps.len() as u32);
    for d in deps {
        b = b.u32(*d);
    }
    b.build()
}

// --- decoder tests ---

#[test]
fn node_kinds_and_payloads_round_out() {
    // A switch with a case (PV with a schema-typed PID), a default pair and a transition.
    let transition = port(
        28,
        Some(
            B::new()
                .u32(PID_STYLE)
                .u32(1)
                .u32(PID_STYLE)
                .u32(2)
                .u8(0)
                .u8(0)
                .f32(0.0)
                .u8(1)
                .f32(0.0)
                .f32(-1.0)
                .f32(0.0),
        ),
        &[],
    );
    let sw = switch(
        1,
        PID_STYLE,
        &[
            switch_case(&[(PID_STYLE, WALKING)], &[collection(2, &[])]),
            default_port_pair(&[]),
            transition,
        ],
    );
    let when_test = port(
        24,
        Some(
            B::new()
                .u32(PID_SPEEDMULT)
                .f32(0.75)
                .u32(0)
                .u32(1)
                .u32(0)
                .u32(0),
        ),
        &[],
    );
    let when = node(
        63,
        B::new().u32(PID_SPEEDMULT).u8(1).u8(0).u8(0),
        base(3),
        &[when_test],
    );
    // A Sound node (typed, `sound.md` §9.1) and a Particle node with its extra byte.
    let sound = node(
        48,
        B::new()
            .u32(0x2A00_0001)
            .u8(1)
            .u8(1)
            .u8(0)
            .f32(0.0)
            .f32(1.0)
            .f32(0.0)
            .f32(0.0)
            .f32(0.0)
            .u32(0)
            .f32(0.1)
            .f32(1.0)
            .f32(0.0),
        base(4),
        &[],
    );
    let particle = {
        let data = B::new()
            .u32(0x3900_0001)
            .u32(0)
            .u8(0)
            .f32(0.0)
            .f32(0.0)
            .f32(0.0)
            .f32(0.0)
            .u32(0)
            .u32(0)
            .f32(0.0)
            .f32(0.0)
            .f32(0.0)
            .u8(0)
            .u8(0)
            .u8(0)
            .u8(0)
            .u32(0);
        B::new()
            .u8(59)
            .bytes(&data.build())
            .bytes(&base(5).build())
            .u8(1)
            .u32(0)
            .u32(0)
            .build()
    };
    // A Run node with a top-level target (port type 1 value form) and a sub-target.
    let run = B::new()
        .u8(5)
        .u8(1)
        .u8(0)
        .u32(0x1000_0042)
        .u8(0)
        .u8(0)
        .u8(0)
        .u8(0)
        .u32(0)
        .bytes(&base(6).build())
        .u32(0)
        .u32(1)
        // target: port 1, value u32 + echo, marker, type 13 (This), one sub-target
        .u8(1)
        .u32(77)
        .u32(1)
        .u8(1)
        .u8(13)
        .u32(1)
        // sub-target: port 6 carries an index
        .u8(6)
        .u32(3)
        .u32(6)
        .u8(1)
        .u8(3)
        .u32(PID_MOOD)
        .u32(0)
        .build();
    let data = table(
        1,
        &[0x0700_0002],
        &[(STARTUP, vec![sw, when, sound, particle, run])],
    );
    let t = ScriptTable::parse_expecting(&data, 0x0700_0001u32, &schema).unwrap();
    let roots = &t.script(STARTUP).unwrap().roots;
    assert_eq!(roots.len(), 5);
    match &roots[0].ports[0].payload {
        PortPayload::SwitchCase(c) => {
            assert_eq!(c.operator, 3);
            assert_eq!(c.values.len(), 1);
            assert_eq!(
                c.values[0].as_ref().unwrap().value,
                PropertyValue::Enum(WALKING)
            );
            assert!((c.transition_time - 0.2).abs() < 1e-6);
        }
        p => panic!("{p:?}"),
    }
    assert_eq!(roots[0].ports[1].payload, PortPayload::F32Pair(0.0, 0.0));
    match &roots[0].ports[2].payload {
        PortPayload::SwitchTransition(t) => {
            assert_eq!(t.flags, [false, false, true]);
            assert_eq!(t.floats, [0.0, 0.0, -1.0, 0.0]);
        }
        p => panic!("{p:?}"),
    }
    match &roots[1].ports[0].payload {
        PortPayload::WhenTest(w) => {
            assert_eq!(w.operator, 1);
            assert_eq!(
                w.constant.as_ref().unwrap().value,
                PropertyValue::Float(0.75)
            );
        }
        p => panic!("{p:?}"),
    }
    match &roots[2].data {
        NodeData::Sound(s) => assert_eq!(s.sound.0, 0x2A00_0001),
        d => panic!("{d:?}"),
    }
    assert_eq!(roots[3].particle_flag, Some(true));
    let p = roots[3].particle().expect("Particle node data");
    assert_eq!(p.particle_id, Did(0x3900_0001));
    assert_eq!(p.effect_did(), Some(Did(0x3900_0001)));
    assert!(!p.drop_effect && !p.should_fade && p.redirect_property == 0);
    match &roots[4].data {
        NodeData::Run(r) => assert_eq!(r.script, 0x1000_0042),
        d => panic!("{d:?}"),
    }
    let target = &roots[4].targets[0];
    assert_eq!(target.value, TargetValue::U32(77));
    assert_eq!(target.target_type, 13);
    assert_eq!(target.sub_targets[0].value, TargetValue::U32(3));
    assert_eq!(target.sub_targets[0].data, TargetData::U32(PID_MOOD));
    assert_eq!(t.first_unusual_field(), None);

    // Without the schema, the switch case's PV cannot be decoded.
    assert!(matches!(
        decode_script_table(&data, &NoSchema)
            .unwrap_err()
            .root_cause(),
        FormatError::UnknownProperty { .. }
    ));
}

/// The data of a Light node (§5.3) with the light-glow stored values of
/// `instance-scripts.md` §6.3: range `range`, white diffuse and specular, constant
/// intensity 0.5, falloff 1, fade time 0.25, baked shadows allowed.
fn light_data(range: f32) -> B {
    B::new()
        .f32(0.0)
        .f32(0.0)
        .f32(0.0) // Direction
        .f32(range)
        .bytes(&[255, 255, 255, 255]) // LightDiffuse
        .u32(1)
        .f32(0.5) // LightDiffuseIntensity: subtype 1, one value
        .bytes(&[250, 251, 252, 253]) // LightSpecular
        .f32(1.0) // LightFalloff
        .u8(0) // ProjectorEnabled
        .u32(0) // ProjectorTexture
        .u32(0)
        .u32(0)
        .u32(0) // ProjectorYaw, Pitch, Roll: subtype 0
        .u8(0) // CoronaEnabled
        .u32(0x4100_0007) // CoronaTexture
        .f32(2.0)
        .f32(3.0)
        .f32(0.75) // CoronaFadeSpeed, Size, Opacity
        .u8(1) // CoronaOcclusion
        .f32(0.5) // CoronaOcclusionSize
        .u8(0) // KeyLight
        .u8(1) // ShadowCaster
        .u8(1) // Light_AllowBakedShadows
        .f32(0.0) // Lifespan
        .f32(0.25) // FadeTime
        .f32(0.0)
        .f32(0.0)
        .f32(1.5) // Offset
        .u8(0)
        .u8(0)
        .u8(0)
        .u8(0)
        .u8(1) // the five flags
        .u32(9) // HoldingLocation
}

/// The data of a MeshFX node (§5.2): entity, holding location, bone offset, scale,
/// `UseParentScale`, one appearance, target property.
fn mesh_fx_data(entity: u32, location: u32, offset: [f32; 3], scale: f32, parent: bool) -> B {
    B::new()
        .u32(entity)
        .u8(1) // AttachToParent
        .u8(0) // AllowPhysics
        .f32(0.25)
        .f32(0.25)
        .f32(0.25)
        .f32(0.25) // the four times
        .u32(location)
        .f32(offset[0])
        .f32(offset[1])
        .f32(offset[2])
        .f32(0.0)
        .f32(0.0)
        .f32(0.0) // yaw, pitch, roll
        .u8(0) // RemoveRotation
        .u32(0) // BoneType
        .u32(0x1000_0077) // Behavior
        .u8(1) // Persistent
        .u8(0) // OnWater
        .u8(u8::from(parent)) // UseParentScale
        .u8(0) // UseParticleScale
        .u8(1) // IgnoreChildScale
        .f32(scale)
        .f32(scale)
        .f32(scale)
        .vle(1)
        .u32(0x2000_0001)
        .u32(4)
        .f32(1.0)
        .u32(0x1000_5000) // MeshIIDTargetProperty
}

#[test]
fn light_and_mesh_fx_nodes_decode_every_field() {
    let light = node(13, light_data(10.0), base(1), &[]);
    let fx = node(
        41,
        mesh_fx_data(0x4700_0001, 7, [0.0, 0.0, -0.3], 1.05, true),
        base(2),
        &[],
    );
    let data = table(1, &[], &[(STARTUP, vec![light, fx])]);
    let t = ScriptTable::parse_expecting(&data, 0x0700_0001u32, &schema).unwrap();
    let roots = &t.script(STARTUP).unwrap().roots;
    let NodeData::Light(l) = &roots[0].data else {
        panic!("{:?}", roots[0].data)
    };
    assert_eq!(l.range, 10.0);
    assert_eq!(l.diffuse, [255; 4]);
    assert_eq!(l.specular, [250, 251, 252, 253]);
    assert_eq!(l.diffuse_intensity.subtype, 1);
    assert_eq!(l.diffuse_intensity.values, vec![0.5]);
    assert_eq!(l.falloff, 1.0);
    assert_eq!(l.projector_yaw.subtype, 0);
    assert_eq!(l.corona_texture, Did(0x4100_0007));
    assert_eq!(
        (l.corona_fade_speed, l.corona_size, l.corona_opacity),
        (2.0, 3.0, 0.75)
    );
    assert!(l.corona_occlusion && !l.key_light && l.shadow_caster && l.allow_baked_shadows);
    assert_eq!(l.fade_time, 0.25);
    assert_eq!(l.offset, [0.0, 0.0, 1.5]);
    assert!(l.global_light && !l.run_on_local_player_only);
    assert_eq!(l.holding_location, 9);
    assert_eq!(roots[0].base.node_id, 1, "the base block follows the data");

    let NodeData::MeshFx(m) = &roots[1].data else {
        panic!("{:?}", roots[1].data)
    };
    assert_eq!(m.entity, Did(0x4700_0001));
    assert!(m.attach_to_parent && !m.allow_physics && m.persistent);
    assert_eq!(m.holding_location, 7);
    assert_eq!(m.bone_offset, [0.0, 0.0, -0.3]);
    assert_eq!(m.behavior, 0x1000_0077);
    assert!(m.use_parent_scale && !m.use_particle_scale && m.ignore_child_scale);
    assert_eq!(m.scale, [1.05; 3]);
    assert_eq!(m.appearances.len(), 1);
    assert_eq!(m.appearances[0].appearance, Did(0x2000_0001));
    assert_eq!(m.appearances[0].key, 4);
    assert_eq!(m.mesh_iid_target_property, 0x1000_5000);
    assert_eq!(roots[1].base.node_id, 2);

    // Truncated data fails instead of panicking.
    for cut in [1, 20, 60] {
        let short = &data[..data.len().saturating_sub(cut)];
        assert!(ScriptTable::parse_expecting(short, 0x0700_0001u32, &schema).is_err());
    }
}

#[test]
fn malformed_and_unsupported_nodes_are_rejected() {
    let unknown_kind = table(2, &[], &[(STARTUP, vec![node(8, B::new(), base(1), &[])])]);
    assert!(matches!(
        decode_script_table(&unknown_kind, &NoSchema),
        Err(FormatError::Malformed { .. })
    ));
    let camera = table(2, &[], &[(STARTUP, vec![node(7, B::new(), base(1), &[])])]);
    assert!(matches!(
        decode_script_table(&camera, &NoSchema),
        Err(FormatError::Unsupported { value: 7, .. })
    ));
    // Bad bool8 in the animation data.
    let mut bad = anim(1, 0x0500_0001, 10, 0.0, &[]);
    bad[5] = 2;
    let bad = table(2, &[], &[(STARTUP, vec![bad])]);
    assert!(matches!(
        decode_script_table(&bad, &NoSchema),
        Err(FormatError::InvalidBool { .. })
    ));
    // Switch-case reserved field must be 0.
    let p = B::new().u8(0).u8(0).u32(9).u32(3).f32(0.0).f32(0.0);
    let sw = switch(1, PID_STYLE, &[port(5, Some(p), &[])]);
    assert!(matches!(
        decode_script_table(&table(2, &[], &[(STARTUP, vec![sw])]), &schema),
        Err(FormatError::Malformed { .. })
    ));
    // Target marker 0.
    let n = B::new()
        .u8(2)
        .bytes(&base(1).build())
        .u32(0)
        .u32(1)
        .u8(3)
        .u8(0)
        .u8(4)
        .u32(0)
        .build();
    assert!(matches!(
        decode_script_table(&table(2, &[], &[(STARTUP, vec![n])]), &NoSchema),
        Err(FormatError::Malformed { .. })
    ));
    // A negative DID-list count.
    let t = B::new()
        .u32(4)
        .u8(0)
        .tsize(0, 0)
        .tsize(0, 0)
        .tsize(0, 1)
        .u32(5)
        .u32(u32::MAX)
        .build();
    assert!(matches!(
        decode_script_table(&t, &NoSchema),
        Err(FormatError::Malformed { .. })
    ));
}

#[test]
fn hostile_counts_and_depth_are_bounded() {
    // A huge root count is rejected before allocating.
    let t = B::new()
        .u32(2)
        .u8(0)
        .tsize(1, 1)
        .u32(5)
        .vle(0x3FFF_FFFF)
        .build();
    assert!(matches!(
        decode_script_table(&t, &NoSchema),
        Err(FormatError::Truncated { .. })
    ));
    let s = B::new().vle(0x3FFF_FFFF).build();
    assert!(matches!(
        decode_scriptlet(&s, &NoSchema),
        Err(FormatError::Truncated { .. })
    ));
    // Port count larger than the rest of the input.
    let n = B::new().u8(2).bytes(&base(1).build()).u32(u32::MAX).build();
    assert!(decode_scriptlet(&B::new().vle(1).bytes(&n).u32(0).build(), &NoSchema).is_err());

    // Nesting one past the limit fails with DepthLimit; at the limit it decodes. Run on a
    // thread with a normal 2 MiB stack to show the recursion fits.
    let nested = |depth: usize| {
        let mut n = collection(1, &[]);
        for i in 1..depth {
            n = collection(i as u32 + 1, &[n]);
        }
        scriptlet(&[n], &[])
    };
    let ok = nested(MAX_NODE_DEPTH);
    let too_deep = nested(MAX_NODE_DEPTH + 1);
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(move || {
            let s = decode_scriptlet(&ok, &NoSchema).unwrap();
            assert_eq!(s.census().max_depth, MAX_NODE_DEPTH);
            assert!(matches!(
                decode_scriptlet(&too_deep, &NoSchema),
                Err(FormatError::DepthLimit { .. })
            ));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn unusual_fields_are_reported() {
    let t = table(3, &[], &[]);
    assert_eq!(
        decode_script_table(&t, &NoSchema)
            .unwrap()
            .first_unusual_field(),
        Some("table kind other than 1, 2 or 4")
    );
    let t = table(2, &[0x0700_0009], &[]);
    let t = ScriptTable::parse_expecting(&t, 0x0700_0009u32, &NoSchema).unwrap();
    assert_eq!(t.first_unusual_field(), Some("table includes itself"));
    let t = table(2, &[], &[(STARTUP, vec![param_node(1, "x")])]);
    assert_eq!(
        decode_script_table(&t, &NoSchema)
            .unwrap()
            .first_unusual_field(),
        Some("scriptlet parameter node in a table")
    );
    let s = scriptlet(&[collection(1, &[collection(1, &[])])], &[]);
    assert_eq!(
        decode_scriptlet(&s, &NoSchema)
            .unwrap()
            .first_unusual_field(),
        Some("duplicate node ID in a scriptlet")
    );
}

// --- resolver tests ---

const CLASS: u32 = 0x0700_0001;
const BINDING: u32 = 0x0700_0002;
const SHARED: u32 = 0x0700_0003;
const TEMPLATE: u32 = 0x0C00_0010;
const IDLE_SCRIPTLET: u32 = 0x0C00_0020;
const SELF_LOOP: u32 = 0x0C00_0030;
const CLIP_IDLE: u32 = 0x0500_0001;
const CLIP_WALK: u32 = 0x0500_0002;
const CLIP_RUN: u32 = 0x0500_0003;
const CLIP_FIDGET_A: u32 = 0x0500_0004;
const CLIP_FIDGET_B: u32 = 0x0500_0005;
const CLIP_BACK: u32 = 0x0500_0006;
const CLIP_EMOTE: u32 = 0x0500_0007;

/// A small world shaped like §8.3–§8.4: a class table including a binding table whose
/// Startup invokes a locomotion template with named ports.
fn world() -> HashMap<Did, Vec<u8>> {
    let mut m = HashMap::new();
    let speed = &[(PID_ANIM_SPEED, PID_SPEEDMULT)][..];
    // Template: idle parameter (lower-case name, matched ignoring case), then a switch on
    // the movement style whose walking case and default each blend on propel.
    let blend = |id: u32, fwd: &str, back: &str| {
        scalar_blend(
            id,
            PID_PROPEL,
            &[
                (-1.0, vec![param_node(id + 1, back)]),
                (0.0, vec![collection(id + 2, &[])]),
                (1.0, vec![param_node(id + 3, fwd)]),
            ],
        )
    };
    let template = scriptlet(
        &[
            param_node(1, "idle"),
            switch(
                2,
                PID_STYLE,
                &[
                    switch_case(
                        &[(PID_STYLE, WALKING)],
                        &[blend(10, "WalkForward", "WalkBack")],
                    ),
                    default_port_pair(&[blend(20, "RunForward", "RunBack")]),
                ],
            ),
            param_node(30, "Unbound"),
        ],
        &[],
    );
    m.insert(Did(TEMPLATE), template);
    m.insert(
        Did(IDLE_SCRIPTLET),
        scriptlet(&[anim(1, CLIP_IDLE, 10, 0.0, &[])], &[CLIP_IDLE]),
    );
    // The run binding overrides the displacement through the Scriptlet node's parameters,
    // which reach the clip through its scriptlet-driven pair (`appearance.md` §7.4.1).
    let run_clip = scriptlet(
        &[node(
            36,
            anim_data(CLIP_RUN, 20, 9.0),
            base_pairs(1, speed, &[(PID_ANIM_DISP, PID_ANIM_DISP)]),
            &[],
        )],
        &[CLIP_RUN],
    );
    m.insert(Did(0x0C00_0021), run_clip);
    let disp = B::new().f32(0.0).f32(6.5).f32(0.0);
    let binding_startup = scriptlet_node(
        1,
        TEMPLATE,
        pset(&[]),
        &[
            (
                "Idle",
                vec![scriptlet_node(2, IDLE_SCRIPTLET, pset(&[]), &[])],
            ),
            ("WalkForward", vec![anim(3, CLIP_WALK, 20, 1.9, speed)]),
            (
                "RunForward",
                vec![scriptlet_node(
                    4,
                    0x0C00_0021,
                    pset(&[(PID_ANIM_DISP, disp)]),
                    &[],
                )],
            ),
            ("WalkBack", vec![anim(5, CLIP_BACK, 20, -2.3, speed)]),
            ("Unused", vec![anim(6, 0x0500_0099, 20, 0.0, &[])]),
        ],
    );
    // A second Startup root: a switch on a mood whose default starts an idle emote, and a
    // random fidget with weights 1 and 3.
    let mood = switch(
        7,
        PID_MOOD,
        &[
            switch_case(&[(PID_MOOD, 9)], &[]),
            default_port_pair(&[anim(8, CLIP_EMOTE, 15, 0.0, &[])]),
        ],
    );
    let fidget = random(
        9,
        &[
            (1, vec![anim(10, CLIP_FIDGET_A, 30, 0.0, &[])]),
            (3, vec![anim(11, CLIP_FIDGET_B, 30, 0.0, &[])]),
            (0, vec![anim(12, 0x0500_0098, 30, 0.0, &[])]),
        ],
    );
    m.insert(
        Did(BINDING),
        table(
            2,
            &[SHARED],
            &[(STARTUP, vec![binding_startup, mood, fidget])],
        ),
    );
    m.insert(
        Did(CLASS),
        table(1, &[BINDING, SHARED], &[(0x1000_0042, vec![])]),
    );
    // The shared table defines a script the class table's start-up names list, and one
    // that only stops a native script.
    m.insert(Did(SHARED), table(2, &[], &[(0x1000_0039, vec![])]));
    m.insert(
        Did(SELF_LOOP),
        scriptlet(&[scriptlet_node(1, SELF_LOOP, pset(&[]), &[])], &[]),
    );
    m
}

fn fetch(m: &HashMap<Did, Vec<u8>>) -> impl FnMut(Did) -> Result<Option<Vec<u8>>, Infallible> + '_ {
    |d| Ok(m.get(&d).cloned())
}

fn entity() -> EntityScripts {
    EntityScripts {
        class_table: Some(Did(CLASS)),
        startup_scripts: vec![0x1000_0039, 0x1000_0837],
        ..EntityScripts::default()
    }
}

#[test]
fn closure_is_pre_order_and_deduplicated() {
    let m = world();
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m));
    let c = r.closure(&[Did(CLASS)]).unwrap();
    assert_eq!(c.order, vec![Did(CLASS), Did(BINDING), Did(SHARED)]);
    assert!(c.issues.is_empty());
    let c = r.closure(&[Did(CLASS), Did(0x0700_0099)]).unwrap();
    assert_eq!(c.issues, vec![ResolveIssue::Missing(Did(0x0700_0099))]);
}

#[test]
fn idle_walk_and_run_resolve_through_the_template() {
    let m = world();
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m));
    let e = entity();

    let idle = r.resolve(&e, MotionState::Idle).unwrap();
    assert_eq!(
        idle.issues,
        vec![ResolveIssue::StartupScriptNotFound(0x1000_0837)],
        "only the native start-up script is unknown"
    );
    let base_layer = idle.primary(10).unwrap();
    assert_eq!(base_layer.clip, Did(CLIP_IDLE));
    assert!(base_layer.loop_forever);
    assert_eq!(base_layer.table, Did(BINDING));
    assert_eq!(base_layer.scriptlet, Did(IDLE_SCRIPTLET));
    assert_eq!(idle.primary(15).unwrap().clip, Did(CLIP_EMOTE));
    assert_eq!(idle.on_layer(20).count(), 0, "no locomotion at zero propel");
    // The random fidget: two candidates sharing the probability 1:3; weight 0 never runs.
    let fidgets: Vec<(Did, f32)> = idle.on_layer(30).map(|c| (c.clip, c.chance)).collect();
    assert_eq!(
        fidgets,
        vec![(Did(CLIP_FIDGET_A), 0.25), (Did(CLIP_FIDGET_B), 0.75)]
    );
    assert_eq!(idle.primary(30).unwrap().clip, Did(CLIP_FIDGET_B));

    let walk = r.resolve(&e, MotionState::Walk).unwrap();
    let w = walk.primary(20).unwrap();
    assert_eq!(w.clip, Did(CLIP_WALK));
    assert_eq!(w.speed_source, Some(PID_SPEEDMULT));
    assert!((w.natural_speed(1.0).unwrap() - 1.9).abs() < 1e-6);
    assert_eq!(walk.primary(10).unwrap().clip, Did(CLIP_IDLE));

    let run = r.resolve(&e, MotionState::Run).unwrap();
    let c = run.primary(20).unwrap();
    assert_eq!(c.clip, Did(CLIP_RUN));
    assert_eq!(
        c.displacement,
        [0.0, 6.5, 0.0],
        "the binding's parameter overrides 9.0"
    );
    assert_eq!(c.playback_rate(), 1.0);
    assert_eq!(c.blend_weight, 1.0);

    let back = r.resolve(&e, MotionState::WalkBack).unwrap();
    assert_eq!(back.primary(20).unwrap().clip, Did(CLIP_BACK));
    // Running back has no binding: the template's placeholder starts nothing.
    assert_eq!(
        r.resolve(&e, MotionState::RunBack)
            .unwrap()
            .on_layer(20)
            .count(),
        0
    );

    // A slowed gait: the driven speed follows Movement_SpeedMultiplier.
    let slow = MotionInputs {
        speed_multiplier: 0.5,
        ..MotionState::Run.inputs()
    };
    assert_eq!(r.resolve(&e, slow).unwrap().primary(20).unwrap().speed, 0.5);
    // Half propel at a walk blends idle-locomotion (empty) and walk 50/50.
    let half = MotionInputs {
        style: MovementStyle::Walking,
        propel: 0.5,
        ..MotionInputs::default()
    };
    assert_eq!(
        r.resolve(&e, half)
            .unwrap()
            .primary(20)
            .unwrap()
            .blend_weight,
        0.5
    );
    assert!(r.cached_records() >= 5);
}

#[test]
fn cycles_missing_records_and_limits_are_reported() {
    let mut m = world();
    // The binding table's Startup now also invokes a self-recursive scriptlet and a missing
    // one.
    m.insert(
        Did(SHARED),
        table(
            2,
            &[],
            &[(
                STARTUP,
                vec![
                    scriptlet_node(1, SELF_LOOP, pset(&[]), &[]),
                    scriptlet_node(2, 0x0C00_0777, pset(&[]), &[]),
                ],
            )],
        ),
    );
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m));
    let res = r.resolve(&entity(), MotionState::Idle).unwrap();
    assert!(res.issues.contains(&ResolveIssue::Cycle(Did(SELF_LOOP))));
    assert!(
        res.issues
            .contains(&ResolveIssue::Missing(Did(0x0C00_0777)))
    );
    assert_eq!(res.primary(10).unwrap().clip, Did(CLIP_IDLE));

    // A tiny visit budget stops the evaluation cleanly.
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m)).with_limits(ResolveLimits {
        max_node_visits: 3,
        ..ResolveLimits::default()
    });
    let res = r.resolve(&entity(), MotionState::Idle).unwrap();
    assert!(res.issues.contains(&ResolveIssue::Limit("max_node_visits")));
    assert!(res.nodes_visited <= 4);

    // A fetch failure aborts.
    let mut r = ClipResolver::new(&schema, vocab(), |d: Did| {
        if d == Did(BINDING) {
            Err("disk on fire")
        } else {
            Ok(m.get(&d).cloned())
        }
    });
    assert!(r.resolve(&entity(), MotionState::Idle).is_err());

    // An undecodable table is reported, not fatal.
    let mut broken = world();
    broken.insert(Did(BINDING), vec![1, 2, 3]);
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&broken));
    let res = r.resolve(&entity(), MotionState::Idle).unwrap();
    assert!(
        res.closure
            .issues
            .iter()
            .any(|i| matches!(i, ResolveIssue::Undecodable(d, _) if *d == Did(BINDING)))
    );
    assert!(res.candidates.is_empty());
}

#[test]
fn entity_scripts_read_property_sets() {
    use tessera_formats::{PropertyArray, PropertyEntry, PropertySet, PropertyStruct, TSize};
    let table_struct = PropertyValue::Struct(PropertyStruct {
        size: TSize { hint: 0, count: 2 },
        fields: vec![
            PropertyEntry {
                pid: PID_DB_TABLE,
                value: PropertyValue::DidRef(Did(CLASS)),
            },
            PropertyEntry {
                pid: PID_STARTUP_NAMES,
                value: PropertyValue::Array(PropertyArray {
                    count: 1,
                    elements: vec![PropertyEntry {
                        pid: PID_SCRIPT_ID,
                        value: PropertyValue::Enum(0x1000_0039),
                    }],
                }),
            },
        ],
    });
    let set = PropertySet {
        size: TSize { hint: 0, count: 2 },
        entries: vec![
            PropertyEntry {
                pid: PID_CLASS_TABLE,
                value: table_struct,
            },
            PropertyEntry {
                pid: PID_MOOD,
                value: PropertyValue::Enum(9),
            },
        ],
    };
    let mut e = EntityScripts::new();
    e.add_property_set(&set, &vocab());
    assert_eq!(e.class_table, Some(Did(CLASS)));
    assert_eq!(e.startup_scripts, vec![0x1000_0039]);
    assert_eq!(e.properties.get(PID_MOOD), Some(&PropertyValue::Enum(9)));
    assert_eq!(e.tables(), vec![Did(CLASS)]);

    // The mood value now selects the empty case: no idle emote.
    let m = world();
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m));
    let res = r.resolve(&e, MotionState::Idle).unwrap();
    assert_eq!(res.on_layer(15).count(), 0);
    assert!(res.issues.is_empty());

    // A state without motion inputs: the style is unset, so the switch takes its default.
    let mut state = PropertyState::new();
    state.set(PID_PROPEL, PropertyValue::Float(1.0));
    let res = r.resolve_with_state(&e, state).unwrap();
    assert_eq!(res.primary(20).unwrap().clip, Did(CLIP_RUN));
}

#[test]
fn deep_graphs_evaluate_within_a_small_stack() {
    // A Startup root nested to the decoder's limit with an animation at the bottom, and a
    // chain of 60 scriptlets each invoking the next: both evaluate on a 2 MiB stack, and
    // the evaluation-depth limit cuts the nest cleanly.
    let mut n = anim(1, CLIP_IDLE, 10, 0.0, &[]);
    for i in 1..MAX_NODE_DEPTH - 1 {
        n = collection(i as u32 + 1, &[n]);
    }
    let mut m = HashMap::new();
    m.insert(Did(CLASS), table(2, &[], &[(STARTUP, vec![n])]));
    for i in 0..60u32 {
        let next = scriptlet_node(1, 0x0C00_1000 + i + 1, pset(&[]), &[]);
        m.insert(Did(0x0C00_1000 + i), scriptlet(&[next], &[]));
    }
    m.insert(
        Did(0x0C00_1000 + 60),
        scriptlet(&[anim(1, CLIP_WALK, 20, 0.0, &[])], &[]),
    );
    m.insert(
        Did(BINDING),
        table(
            2,
            &[],
            &[(
                STARTUP,
                vec![scriptlet_node(1, 0x0C00_1000, pset(&[]), &[])],
            )],
        ),
    );
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(move || {
            let e = EntityScripts {
                class_table: Some(Did(CLASS)),
                additional_tables: vec![Did(BINDING)],
                ..EntityScripts::default()
            };
            let mut r = ClipResolver::new(&schema, vocab(), fetch(&m));
            let res = r.resolve(&e, MotionState::Idle).unwrap();
            assert_eq!(res.clips(), vec![Did(CLIP_IDLE), Did(CLIP_WALK)]);
            assert!(res.issues.is_empty(), "{:?}", res.issues);
            let mut r = ClipResolver::new(&schema, vocab(), fetch(&m)).with_limits(ResolveLimits {
                max_eval_depth: 100,
                max_call_depth: 10,
                ..ResolveLimits::default()
            });
            let res = r.resolve(&e, MotionState::Idle).unwrap();
            assert!(res.candidates.is_empty());
            assert!(res.issues.contains(&ResolveIssue::Limit("max_eval_depth")));
            assert!(res.issues.contains(&ResolveIssue::Limit("max_call_depth")));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn run_nodes_follow_the_lookup_and_stop_on_cycles() {
    // Startup runs script 0x10000100, which starts a clip and runs itself again.
    let run = |id: u32, key: u32, stop: bool| {
        node(
            5,
            B::new()
                .u8(1)
                .u8(0)
                .u32(key)
                .u8(0)
                .u8(0)
                .u8(u8::from(stop))
                .u8(0)
                .u32(0),
            base(id),
            &[],
        )
    };
    let mut m = HashMap::new();
    m.insert(
        Did(CLASS),
        table(
            2,
            &[SHARED],
            &[(
                STARTUP,
                vec![run(1, 0x1000_0100, false), run(2, 0x1000_0999, true)],
            )],
        ),
    );
    m.insert(
        Did(SHARED),
        table(
            2,
            &[],
            &[(
                0x1000_0100,
                vec![anim(1, CLIP_WALK, 20, 0.0, &[]), run(2, 0x1000_0100, false)],
            )],
        ),
    );
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m));
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let res = r.resolve(&e, MotionState::Idle).unwrap();
    assert_eq!(res.clips(), vec![Did(CLIP_WALK)]);
    let c = &res.candidates[0];
    assert_eq!(
        c.table,
        Did(SHARED),
        "attributed to the table defining the script"
    );
    assert_eq!(c.scripts, vec![0x1000_0100]);
    assert_eq!(res.issues, vec![ResolveIssue::ScriptCycle(0x1000_0100)]);
}

// --- review regressions ---

/// A Run node (kind 5) for `key`.
fn run_node(id: u32, key: u32, stop: bool) -> Vec<u8> {
    node(
        5,
        B::new()
            .u8(1)
            .u8(0)
            .u32(key)
            .u8(0)
            .u8(0)
            .u8(u8::from(stop))
            .u8(0)
            .u32(0),
        base(id),
        &[],
    )
}

/// A SetProperty node (kind 40) whose value PV is `value` (bytes after the PID).
fn set_property_node(id: u32, property: u32, value: B) -> Vec<u8> {
    node(
        40,
        B::new().u32(property).u32(property).bytes(&value.0).u8(0),
        base(id),
        &[],
    )
}

fn struct_value(fields: Vec<tessera_formats::PropertyEntry>) -> PropertyValue {
    PropertyValue::Struct(tessera_formats::PropertyStruct {
        size: tessera_formats::TSize {
            hint: 0,
            count: fields.len() as u32,
        },
        fields,
    })
}

fn array_value(elements: Vec<tessera_formats::PropertyEntry>) -> PropertyValue {
    PropertyValue::Array(tessera_formats::PropertyArray {
        count: elements.len() as u32,
        elements,
    })
}

fn pe(pid: u32, value: PropertyValue) -> tessera_formats::PropertyEntry {
    tessera_formats::PropertyEntry { pid, value }
}

#[test]
fn additional_tables_are_read_from_bare_dids_and_structs() {
    // `Entity_AdditionalScriptTables` elements are bare `ScriptTable_DBScriptTable` DIDs in
    // the reference install; table structs are read too; null DIDs are skipped.
    let set = tessera_formats::PropertySet {
        size: tessera_formats::TSize { hint: 0, count: 1 },
        entries: vec![pe(
            PID_ADDITIONAL,
            array_value(vec![
                pe(PID_DB_TABLE, PropertyValue::DidRef(Did(BINDING))),
                pe(PID_DB_TABLE, PropertyValue::DidRef(Did(0))),
                pe(
                    0x1000_0199,
                    struct_value(vec![
                        pe(PID_DB_TABLE, PropertyValue::DidRef(Did(SHARED))),
                        pe(
                            PID_STARTUP_NAMES,
                            array_value(vec![pe(PID_SCRIPT_ID, PropertyValue::Enum(0x1000_0039))]),
                        ),
                    ]),
                ),
                pe(PID_DB_TABLE, PropertyValue::DidRef(Did(BINDING))),
            ]),
        )],
    };
    let mut e = EntityScripts::new();
    e.add_property_set(&set, &vocab());
    assert_eq!(e.additional_tables, vec![Did(BINDING), Did(SHARED)]);
    assert_eq!(e.startup_scripts, vec![0x1000_0039]);
    assert_eq!(e.tables(), vec![Did(BINDING), Did(SHARED)]);
    // An entity whose only tables are additional ones resolves through them.
    let m = world();
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m));
    let res = r.resolve(&e, MotionState::Idle).unwrap();
    assert_eq!(res.primary(10).unwrap().clip, Did(CLIP_IDLE));
}

#[test]
fn repeated_problems_are_reported_once_and_cheaply() {
    // Startup runs K1 and K4 and K5. K1 has 300 Run(K2) roots and K2 300 Scriptlet nodes
    // naming a missing scriptlet (90,000 failed invocations); K4 has 100 Run(K3) where K3
    // nests below the evaluation-depth limit with 50 leaves; K5 runs itself 500 times.
    const K1: u32 = 0x1000_0201;
    const K2: u32 = 0x1000_0202;
    const K3: u32 = 0x1000_0203;
    const K4: u32 = 0x1000_0204;
    const K5: u32 = 0x1000_0205;
    const MISSING: u32 = 0x0C00_0BAD;
    let k1: Vec<Vec<u8>> = (0..300).map(|i| run_node(i + 1, K2, false)).collect();
    let k2: Vec<Vec<u8>> = (0..300)
        .map(|i| scriptlet_node(i + 1, MISSING, pset(&[]), &[]))
        .collect();
    let leaves: Vec<Vec<u8>> = (0..50)
        .map(|i| anim(100 + i, CLIP_IDLE, 10, 0.0, &[]))
        .collect();
    let mut k3 = collection(1, &leaves);
    for i in 0..10 {
        k3 = collection(2 + i, &[k3]);
    }
    let k4: Vec<Vec<u8>> = (0..100).map(|i| run_node(i + 1, K3, false)).collect();
    let k5: Vec<Vec<u8>> = (0..500).map(|i| run_node(i + 1, K5, false)).collect();
    let mut m = HashMap::new();
    m.insert(
        Did(CLASS),
        table(
            2,
            &[],
            &[
                (
                    STARTUP,
                    vec![
                        run_node(1, K1, false),
                        run_node(2, K4, false),
                        run_node(3, K5, false),
                    ],
                ),
                (K1, k1),
                (K2, k2),
                (K3, vec![k3]),
                (K4, k4),
                (K5, k5),
            ],
        ),
    );
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m)).with_limits(ResolveLimits {
        max_eval_depth: 8,
        ..ResolveLimits::default()
    });
    let started = std::time::Instant::now();
    let res = r.resolve(&e, MotionState::Idle).unwrap();
    let took = started.elapsed();
    assert!(res.nodes_visited > 90_000, "{}", res.nodes_visited);
    assert_eq!(
        res.issues,
        vec![
            ResolveIssue::Missing(Did(MISSING)),
            ResolveIssue::Limit("max_eval_depth"),
            ResolveIssue::ScriptCycle(K5),
        ]
    );
    assert!(res.candidates.is_empty());
    // Generous bound (debug build): the quadratic issue scan took minutes at this size.
    assert!(took.as_secs() < 30, "{took:?}");

    // An undecodable scriptlet is reported once too, with its error.
    drop(r);
    m.insert(Did(MISSING), vec![0xFF; 3]);
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m));
    let res = r.resolve(&e, MotionState::Idle).unwrap();
    let undecodable = res
        .issues
        .iter()
        .filter(|i| matches!(i, ResolveIssue::Undecodable(d, _) if *d == Did(MISSING)))
        .count();
    assert_eq!(undecodable, 1, "{:?}", res.issues);
}

#[test]
fn work_inside_visits_is_bounded() {
    // Startup: 200 copies of a Switch on the movement style with 2,000 cases that never
    // match. Few visits, but every visit scans every case.
    let cases: Vec<Vec<u8>> = (0..2000)
        .map(|i| switch_case(&[(PID_STYLE, 1000 + i)], &[]))
        .collect();
    let wide = switch(1, PID_STYLE, &cases);
    let roots: Vec<Vec<u8>> = (0..200).map(|_| wide.clone()).collect();
    let mut m = HashMap::new();
    m.insert(Did(CLASS), table(2, &[], &[(STARTUP, roots)]));
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let walking = MotionInputs {
        style: MovementStyle::Walking,
        ..MotionInputs::default()
    };

    // The default budget covers it.
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m));
    let res = r.resolve(&e, walking).unwrap();
    assert!(res.issues.is_empty(), "{:?}", res.issues);
    assert_eq!(res.nodes_visited, 200);

    // A small work budget stops the evaluation long before the visit budget does.
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m)).with_limits(ResolveLimits {
        max_work: 50_000,
        ..ResolveLimits::default()
    });
    let res = r.resolve(&e, walking).unwrap();
    assert_eq!(res.issues, vec![ResolveIssue::Limit("max_work")]);
    assert!(res.nodes_visited < 20, "{}", res.nodes_visited);

    // Parameter bindings, random and blend ports are charged too: a scriptlet invoked with
    // 2,000 bindings whose roots look one up 200 times.
    const WIDE: u32 = 0x0C00_0040;
    let bindings: Vec<(String, Vec<Vec<u8>>)> =
        (0..2000).map(|i| (format!("P{i}"), vec![])).collect();
    let bindings: Vec<(&str, Vec<Vec<u8>>)> = bindings
        .iter()
        .map(|(n, c)| (n.as_str(), c.clone()))
        .collect();
    let lookups: Vec<Vec<u8>> = (0..200).map(|i| param_node(i + 1, "Missing")).collect();
    let weighted: Vec<(u32, Vec<Vec<u8>>)> = (0..2000).map(|_| (0, vec![])).collect();
    let anchors: Vec<(f32, Vec<Vec<u8>>)> = (0..2000).map(|i| (i as f32, vec![])).collect();
    let mut m = HashMap::new();
    m.insert(Did(WIDE), scriptlet(&lookups, &[]));
    m.insert(
        Did(CLASS),
        table(
            2,
            &[],
            &[(
                STARTUP,
                vec![
                    random(1, &weighted),
                    scalar_blend(2, PID_PROPEL, &anchors),
                    scriptlet_node(3, WIDE, pset(&[]), &bindings),
                ],
            )],
        ),
    );
    for (max_work, limited) in [(1_000, true), (10_000, true), (u64::MAX, false)] {
        let mut r = ClipResolver::new(&schema, vocab(), fetch(&m)).with_limits(ResolveLimits {
            max_work,
            ..ResolveLimits::default()
        });
        let res = r.resolve(&e, walking).unwrap();
        assert_eq!(
            res.issues.contains(&ResolveIssue::Limit("max_work")),
            limited,
            "{max_work}: {:?}",
            res.issues
        );
    }
}

#[test]
fn animation_node_scans_are_charged() {
    // A scriptlet whose one root is an animation node with 5,000 Default (event-time) ports
    // and 2,000 driven pairs, invoked 200 times. Few visits, but every emit scans every
    // port and driven pair and collects the event times.
    const WIDE_ANIM: u32 = 0x0C00_0041;
    let events: Vec<Vec<u8>> = (0..5000)
        .map(|i| port(6, Some(B::new().f32(i as f32 * 0.001)), &[]))
        .collect();
    let driven: Vec<(u32, u32)> = (0..2000)
        .map(|i| (0x2000_0000 + i, 0x3000_0000 + i))
        .collect();
    let wide = node(
        36,
        anim_data(CLIP_IDLE, 10, 0.0),
        base_full(1, 1.0, &driven),
        &events,
    );
    let invocations: Vec<Vec<u8>> = (0..200)
        .map(|i| scriptlet_node(i + 1, WIDE_ANIM, pset(&[]), &[]))
        .collect();
    let mut m = HashMap::new();
    m.insert(Did(WIDE_ANIM), scriptlet(&[wide], &[]));
    m.insert(Did(CLASS), table(2, &[], &[(STARTUP, invocations)]));
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let walking = MotionInputs {
        style: MovementStyle::Walking,
        ..MotionInputs::default()
    };

    // A small budget stops it after a few emits, well within the visit budget.
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m)).with_limits(ResolveLimits {
        max_work: 100_000,
        ..ResolveLimits::default()
    });
    let res = r.resolve(&e, walking).unwrap();
    assert_eq!(res.issues, vec![ResolveIssue::Limit("max_work")]);
    assert!(res.candidates.len() < 20, "{}", res.candidates.len());
    assert!(res.nodes_visited < 50, "{}", res.nodes_visited);

    // The default and an unlimited budget cover it and keep every event time.
    for max_work in [ResolveLimits::default().max_work, u64::MAX] {
        let mut r = ClipResolver::new(&schema, vocab(), fetch(&m)).with_limits(ResolveLimits {
            max_work,
            ..ResolveLimits::default()
        });
        let res = r.resolve(&e, walking).unwrap();
        assert!(res.issues.is_empty(), "{max_work}: {:?}", res.issues);
        assert_eq!(res.candidates.len(), 200);
        assert!(res.candidates.iter().all(|c| c.event_times.len() == 5000));
    }
}

#[test]
fn set_property_keeps_only_comparable_values() {
    // Startup: SetProperty(mood = 9) and SetProperty(big = 2,000-element array), then a
    // Switch on the mood (case 9 → emote) and a Switch on the big property (case 0 → walk;
    // default → run).
    let mut big = B::new().u32(2000);
    for i in 0..2000 {
        big = big.u32(PID_BIG_ELEMENT).u32(i);
    }
    let mood_switch = switch(
        3,
        PID_MOOD,
        &[
            switch_case(&[(PID_MOOD, 9)], &[anim(4, CLIP_EMOTE, 15, 0.0, &[])]),
            default_port_pair(&[]),
        ],
    );
    let big_switch = switch(
        5,
        PID_BIG,
        &[
            switch_case(&[(PID_DEFAULTED, 0)], &[anim(6, CLIP_WALK, 20, 0.0, &[])]),
            default_port_pair(&[anim(7, CLIP_RUN, 20, 0.0, &[])]),
        ],
    );
    let mut m = HashMap::new();
    m.insert(
        Did(CLASS),
        table(
            2,
            &[],
            &[(
                STARTUP,
                vec![
                    set_property_node(1, PID_MOOD, B::new().u32(9)),
                    set_property_node(2, PID_BIG, big),
                    mood_switch,
                    big_switch,
                ],
            )],
        ),
    );
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    // The big property reads its default (0) when unset: the array was not stored, so the
    // Switch on it selects the case for 0.
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m))
        .with_defaults(|pid| (pid == PID_BIG).then_some(PropertyValue::Int(0)));
    let res = r.resolve(&e, MotionState::Idle).unwrap();
    assert_eq!(res.issues, vec![]);
    assert_eq!(res.clips(), vec![Did(CLIP_EMOTE), Did(CLIP_WALK)]);
    assert!(tessera_formats::script_table::resolve::is_state_value(
        &PropertyValue::Enum(1)
    ));
    assert!(!tessera_formats::script_table::resolve::is_state_value(
        &array_value(vec![])
    ));
}

/// A table whose Startup holds a Switch on `PID_DEFAULTED`: case 7 → walk clip, case 0 →
/// back clip, default → run clip.
fn defaulted_switch_world() -> HashMap<Did, Vec<u8>> {
    let sw = switch(
        1,
        PID_DEFAULTED,
        &[
            switch_case(&[(PID_DEFAULTED, 7)], &[anim(2, CLIP_WALK, 20, 0.0, &[])]),
            switch_case(&[(PID_DEFAULTED, 0)], &[anim(3, CLIP_BACK, 20, 0.0, &[])]),
            default_port_pair(&[anim(4, CLIP_RUN, 20, 0.0, &[])]),
        ],
    );
    let mut m = HashMap::new();
    m.insert(Did(CLASS), table(2, &[], &[(STARTUP, vec![sw])]));
    m
}

/// Writes one dictionary definition (layout of `property-system.md` §2.5).
fn dict_def(b: B, pid: u32, ty: u32, data: u32, default: Option<B>) -> B {
    let flags = if default.is_some() {
        tessera_formats::property::FLAG_HAS_DEFAULT
    } else {
        0
    };
    let mut b = b
        .u32(pid)
        .u32(pid)
        .u32(ty)
        .u32(0x1000_0001)
        .u32(4)
        .u32(data)
        .u32(0)
        .u32(flags);
    if let Some(d) = default {
        b = b.bytes(&d.0);
    }
    b.u32(0x3FC0_0000)
        .bytes(&[2, 2, 4, 0, 1])
        .vle(0)
        .u32(0)
        .u32(0)
}

/// A small dictionary: the names `ScriptVocabulary::from_data` needs, with the enum
/// mappers 0x23000010 (script IDs), 0x23000020 (movement styles), 0x23000030 (elements),
/// and `Test_Defaulted` (Int, default 7).
fn small_dictionary() -> tessera_formats::PropertyDictionary {
    use tessera_formats::Record;
    // (pid, name, type code, data, default)
    let defs: Vec<(u32, &str, u32, u32, Option<B>)> = vec![
        (PID_SCRIPT_ID, "ScriptTable_ScriptID", 8, 0x2300_0010, None),
        (PID_STYLE, "Movement_MovementStyle", 8, 0x2300_0020, None),
        (PID_CLASS_TABLE, "Entity_ClassScriptTable", 11, 0, None),
        (PID_DB_TABLE, "ScriptTable_DBScriptTable", 20, 0, None),
        (PID_ADDITIONAL, "Entity_AdditionalScriptTables", 12, 0, None),
        (PID_PROPEL, "Slider_Propel", 9, 0, None),
        (PID_STRAFE, "Slider_Strafe", 9, 0, None),
        (PID_TURN, "Slider_Turn", 9, 0, None),
        (PID_ELEMENT, "Entity_Element", 8, 0x2300_0030, None),
        (PID_ANIM_SPEED, "AnimationScript_Speed", 9, 0, None),
        (
            PID_DEFAULTED,
            "Test_Defaulted",
            15,
            0,
            Some(B::new().u32(7)),
        ),
    ];
    let mut b = B::new()
        .u32(0x3400_0000)
        .bytes(&[0; 8])
        .tsize(0, defs.len() as u32);
    for (pid, name, ..) in &defs {
        b = b.u32(*pid).pascal(name);
    }
    b = b.u16(0).tsize(0, defs.len() as u32);
    for (pid, _, ty, data, default) in defs {
        b = dict_def(b, pid, ty, data, default);
    }
    tessera_formats::PropertyDictionary::parse(&b.build()).unwrap()
}

fn enum_mapper(did: u32, base: u32, names: &[(u32, &str)]) -> tessera_formats::EnumMapper {
    use tessera_formats::Record;
    let mut b = B::new().u32(did).u32(base).tsize(0, names.len() as u32);
    for (k, n) in names {
        b = b.u32(*k).pascal(n);
    }
    tessera_formats::EnumMapper::parse(&b.tsize(0, 0).build()).unwrap()
}

#[test]
fn vocabulary_from_data_follows_base_mappers() {
    let dict = small_dictionary();
    let mappers = [
        // `Startup` only in the base mapper.
        enum_mapper(0x2300_0010, 0x2300_0011, &[(0x1000_0001, "Other")]),
        enum_mapper(0x2300_0011, 0, &[(STARTUP, "Startup")]),
        enum_mapper(
            0x2300_0020,
            0,
            &[(WALKING, "Walking"), (RUNNING, "Running")],
        ),
        enum_mapper(0x2300_0030, 0, &[(5, "Earth_ControlledSurface")]),
    ];
    let fetch_mapper = |d: Did| mappers.iter().find(|m| m.did == d).cloned();
    let v = ScriptVocabulary::from_data(&dict, fetch_mapper).unwrap();
    assert_eq!(v.startup_key, STARTUP);
    assert_eq!((v.style_walking, v.style_running), (WALKING, RUNNING));
    assert_eq!(v.class_script_table, PID_CLASS_TABLE);
    assert_eq!(v.db_script_table, PID_DB_TABLE);
    assert_eq!(v.additional_script_tables, Some(PID_ADDITIONAL));
    assert_eq!(v.instance_script_table, None);
    assert_eq!(v.entity_element, Some(PID_ELEMENT));
    assert_eq!(v.element_ground, Some(5));
    assert_eq!(v.element_no_surface, None, "label missing: optional");
    assert_eq!(v.animation.speed, Some(PID_ANIM_SPEED));
    assert_eq!(v.animation.layer, None);

    // A required label missing everywhere in the chain is an error.
    let broken = |d: Did| {
        if d == Did(0x2300_0011) {
            None
        } else {
            fetch_mapper(d)
        }
    };
    assert!(matches!(
        ScriptVocabulary::from_data(&dict, broken),
        Err(FormatError::UnknownName {
            name: "Startup",
            ..
        })
    ));
}

#[test]
fn unset_properties_read_defaults_or_zero_when_asked() {
    let m = defaulted_switch_world();
    let dict = small_dictionary();
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    // Unset and no default source: the Switch takes its Default port.
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m));
    let clips = r.resolve(&e, MotionState::Idle).unwrap().clips();
    assert_eq!(clips, vec![Did(CLIP_RUN)]);
    // The dictionary default (7) selects its case (§16 Decision).
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m)).with_dictionary_defaults(&dict);
    let clips = r.resolve(&e, MotionState::Idle).unwrap().clips();
    assert_eq!(clips, vec![Did(CLIP_WALK)]);
    // Without a default, the option reads an unset scalar as 0.
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m)).unset_reads_as_zero(true);
    let clips = r.resolve(&e, MotionState::Idle).unwrap().clips();
    assert_eq!(clips, vec![Did(CLIP_BACK)]);
    // A value in the entity's sets wins over the default.
    let mut set_value = e.clone();
    set_value
        .properties
        .set(PID_DEFAULTED, PropertyValue::Int(0));
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m)).with_dictionary_defaults(&dict);
    let clips = r.resolve(&set_value, MotionState::Idle).unwrap().clips();
    assert_eq!(clips, vec![Did(CLIP_BACK)]);
    // Every branch whatever the value.
    let all = r.reachable(&e).unwrap();
    assert_eq!(
        all.clips(),
        vec![Did(CLIP_WALK), Did(CLIP_BACK), Did(CLIP_RUN)]
    );
}

#[test]
fn resolver_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    let m = defaulted_switch_world();
    let dict = small_dictionary();
    let mut r = ClipResolver::new(&dict, vocab(), fetch(&m)).with_dictionary_defaults(&dict);
    assert_send_sync(&r);
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let clips = std::thread::scope(|s| {
        s.spawn(move || r.resolve(&e, MotionState::Idle).unwrap().clips())
            .join()
            .unwrap()
    });
    assert_eq!(clips, vec![Did(CLIP_WALK)]);
}

#[test]
fn natural_speed_needs_a_displacement() {
    let m = world();
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m));
    let idle = r.resolve(&entity(), MotionState::Idle).unwrap();
    let c = idle.primary(10).unwrap();
    assert_eq!(c.displacement, [0.0; 3]);
    assert_eq!(
        c.natural_speed(1.0),
        None,
        "no displacement: no natural speed"
    );
    let walk = r.resolve(&entity(), MotionState::Walk).unwrap();
    let w = walk.primary(20).unwrap();
    assert!((w.natural_speed(2.0).unwrap() - 0.95).abs() < 1e-6);
    assert_eq!(w.natural_speed(0.0), None);
    assert_eq!(w.natural_speed(f32::NAN), None);
}

// --- entity chains (`collect_entity_scripts`) ---

/// An entity descriptor (flags 0x8B) linking to `template` with an embedded property set.
fn entity_record(did: u32, template: u32, entries: &[B]) -> Vec<u8> {
    let mut b = B::new()
        .u32(0x8B)
        .u32(did)
        .u32(7)
        .u32(template)
        .tsize(0, entries.len() as u32);
    for e in entries {
        b = b.bytes(&e.0);
    }
    b.build()
}

fn property_set_record(did: u32, entries: &[B]) -> Vec<u8> {
    let mut b = B::new().u32(did).tsize(0, entries.len() as u32);
    for e in entries {
        b = b.bytes(&e.0);
    }
    b.build()
}

/// A top-level entry with PID echo.
fn echo(pid: u32, value: B) -> B {
    B::new().u32(pid).u32(pid).bytes(&value.0)
}

/// An `Entity_ClassScriptTable` entry naming `table` and start-up `names`.
fn class_table_entry(table: u32, names: &[u32]) -> B {
    let mut list = B::new().u32(names.len() as u32);
    for n in names {
        list = list.u32(PID_SCRIPT_ID).u32(*n);
    }
    let fields = B::new()
        .tsize(0, 2)
        .bytes(&echo(PID_DB_TABLE, B::new().u32(table)).0)
        .bytes(&echo(PID_STARTUP_NAMES, list).0);
    echo(PID_CLASS_TABLE, fields)
}

#[test]
fn entity_chains_collect_parents_first_then_the_object() {
    use tessera_formats::script_table::resolve::collect_entity_scripts;
    let mut m: HashMap<Did, Vec<u8>> = HashMap::new();
    // Parent 0x47000001 (→ template): class table SHARED, start-up 0x10000039, mood 9.
    m.insert(
        Did(0x4700_0001),
        entity_record(
            0x4700_0001,
            0x1F00_0001,
            &[
                class_table_entry(SHARED, &[0x1000_0039]),
                echo(PID_MOOD, B::new().u32(9)),
            ],
        ),
    );
    // Child 0x47000002 (→ parent): class table CLASS replaces SHARED, start-up 0x10000040,
    // mood 3 replaces 9.
    m.insert(
        Did(0x4700_0002),
        entity_record(
            0x4700_0002,
            0x4700_0001,
            &[
                class_table_entry(CLASS, &[0x1000_0040]),
                echo(PID_MOOD, B::new().u32(3)),
            ],
        ),
    );
    // Object 0x70000005: PhysObj → child, an additional table, mood 4 (wins).
    m.insert(
        Did(0x7900_0005),
        property_set_record(
            0x7900_0005,
            &[
                echo(PID_PHYS_OBJ, B::new().u32(0x4700_0002)),
                echo(
                    PID_ADDITIONAL,
                    B::new().u32(1).u32(PID_DB_TABLE).u32(BINDING),
                ),
                echo(PID_MOOD, B::new().u32(4)),
            ],
        ),
    );
    // Object 0x70000006 without PhysObj: only its own set.
    m.insert(
        Did(0x7900_0006),
        property_set_record(0x7900_0006, &[class_table_entry(SHARED, &[])]),
    );
    // A parent cycle 0x47000003 ↔ 0x47000004.
    m.insert(
        Did(0x4700_0003),
        entity_record(0x4700_0003, 0x4700_0004, &[]),
    );
    m.insert(
        Did(0x4700_0004),
        entity_record(0x4700_0004, 0x4700_0003, &[]),
    );
    // A dangling parent.
    m.insert(
        Did(0x4700_0007),
        entity_record(0x4700_0007, 0x4700_0099, &[]),
    );
    let v = vocab();

    let chain = collect_entity_scripts(Did(0x7000_0005), &schema, &v, fetch(&m)).unwrap();
    assert_eq!(chain.object, Some(Did(0x7000_0005)));
    assert_eq!(chain.entities, vec![Did(0x4700_0002), Did(0x4700_0001)]);
    let s = &chain.scripts;
    assert_eq!(
        s.class_table,
        Some(Did(CLASS)),
        "the child overrides its parent"
    );
    assert_eq!(s.startup_scripts, vec![0x1000_0039, 0x1000_0040]);
    assert_eq!(s.additional_tables, vec![Did(BINDING)]);
    assert_eq!(s.properties.get(PID_MOOD), Some(&PropertyValue::Enum(4)));
    assert_eq!(s.tables(), vec![Did(CLASS), Did(BINDING)]);

    // Starting from the entity: no object, the child's values.
    let chain = collect_entity_scripts(Did(0x4700_0002), &schema, &v, fetch(&m)).unwrap();
    assert_eq!(chain.object, None);
    assert_eq!(
        chain.scripts.properties.get(PID_MOOD),
        Some(&PropertyValue::Enum(3))
    );
    assert!(chain.scripts.additional_tables.is_empty());

    // An object without a physics entity.
    let chain = collect_entity_scripts(Did(0x7000_0006), &schema, &v, fetch(&m)).unwrap();
    assert!(chain.entities.is_empty());
    assert_eq!(chain.scripts.class_table, Some(Did(SHARED)));

    // A parent cycle, a dangling parent, and a start that is neither kind.
    let err = collect_entity_scripts(Did(0x4700_0003), &schema, &v, fetch(&m)).unwrap_err();
    assert!(
        matches!(
            err,
            tessera_formats::ResolveError::Format(FormatError::ReferenceChain { did, .. })
                if did == 0x4700_0003
        ),
        "{err:?}"
    );
    let err = collect_entity_scripts(Did(0x4700_0007), &schema, &v, fetch(&m)).unwrap_err();
    assert!(
        matches!(
            err,
            tessera_formats::ResolveError::Format(FormatError::DanglingReference { .. })
        ),
        "{err:?}"
    );
    let err = collect_entity_scripts(Did(0x0700_0001), &schema, &v, fetch(&m)).unwrap_err();
    assert!(
        matches!(
            err,
            tessera_formats::ResolveError::Format(FormatError::ReferenceChain { .. })
        ),
        "{err:?}"
    );
}

#[test]
fn unusual_field_check_is_linear_in_the_record_size() {
    // 100,000 scripts and as many property sets in reverse key order (a few MB as a hostile
    // record): the "property-set key without a script" check is a set lookup, not a scan.
    use tessera_formats::script_table::Script;
    use tessera_formats::{PropertySet, TSize};
    const N: u32 = 100_000;
    let t = ScriptTable {
        did: Did(CLASS),
        kind: 2,
        scripts_size: TSize { hint: 13, count: N },
        scripts: (0..N)
            .map(|k| Script {
                key: k,
                roots: vec![],
            })
            .collect(),
        script_properties: (0..N).rev().map(|k| (k, PropertySet::default())).collect(),
        ..ScriptTable::default()
    };
    let started = std::time::Instant::now();
    assert_eq!(t.first_unusual_field(), None);
    assert!(started.elapsed().as_secs() < 10, "{:?}", started.elapsed());
    let mut t = t;
    t.script_properties.push((N + 1, PropertySet::default()));
    assert_eq!(
        t.first_unusual_field(),
        Some("property-set key without a script")
    );
}

// --- held clips and deformers (§6.7–§6.9) ---

const PID_MANUAL_UPDATE: u32 = 0x1000_0300;
const PID_MANUAL_TIME: u32 = 0x1000_0301;
const PID_HEIGHT: u32 = 0x1000_0302;
const PID_BONE_SCALE: u32 = 0x1000_0303;
const PID_DEFORM_TIME: u32 = 0x1000_0304;
const PID_SLIDER_VALUES: u32 = 0x1000_0305;
const PID_SLIDER_ARRAY: u32 = 0x1000_0306;
const PID_INTENSITY: u32 = 0x1000_0307;
const PID_OTHER_FLOAT: u32 = 0x1000_0308;

fn shape_schema(pid: u32) -> Option<PropertyType> {
    match pid {
        PID_MANUAL_TIME | PID_HEIGHT | PID_BONE_SCALE | PID_DEFORM_TIME | PID_INTENSITY
        | PID_OTHER_FLOAT => Some(PropertyType::Float),
        PID_MANUAL_UPDATE => Some(PropertyType::Bool),
        PID_SLIDER_VALUES | PID_SLIDER_ARRAY => Some(PropertyType::String),
        p => schema(p),
    }
}

/// A held animation node (`ManualUpdate` 1) on layer 110, `LoopForever` as given.
fn held_anim(id: u32, clip: u32, loop_forever: bool, manual_time: f32) -> Vec<u8> {
    let data = B::new()
        .u32(clip)
        .u8(u8::from(loop_forever))
        .u32(1)
        .f32(0.1)
        .f32(0.1)
        .u32(110)
        .u8(1)
        .u8(1) // ManualUpdate
        .f32(manual_time)
        .f32(0.0)
        .f32(1.0)
        .u8(0)
        .f32(1.0)
        .f32(1.0)
        .f32(0.0)
        .f32(0.0)
        .f32(0.0)
        .u8(0)
        .u8(1)
        .u8(0);
    node(
        36,
        data,
        base_full(id, 1.0, &[(PID_MANUAL_TIME, PID_HEIGHT)]),
        &[],
    )
}

fn shape_world() -> HashMap<Did, Vec<u8>> {
    let mesh_deformer = node(
        18,
        B::new().u32(0x0500_0E43).f32(0.0).u8(1),
        base_full(20, 1.0, &[(PID_DEFORM_TIME, PID_BONE_SCALE)]),
        &[],
    );
    let slider_deformer = node(
        81,
        B::new().pascal("8000").u32(0x7800_0001),
        base_full(21, 1.0, &[(PID_SLIDER_VALUES, PID_SLIDER_ARRAY)]),
        &[],
    );
    let mut m = HashMap::new();
    m.insert(
        Did(CLASS),
        table(
            2,
            &[],
            &[(
                STARTUP,
                vec![
                    anim(1, CLIP_IDLE, 10, 0.0, &[]),
                    held_anim(2, 0x0500_8E76, true, 0.0),
                    mesh_deformer,
                    slider_deformer,
                ],
            )],
        ),
    );
    m
}

fn shape_vocab() -> ScriptVocabulary {
    let mut v = vocab();
    v.animation.manual_update = Some(PID_MANUAL_UPDATE);
    v.animation.manual_time = Some(PID_MANUAL_TIME);
    v.slider_values = Some(PID_SLIDER_VALUES);
    v.mesh_deformer.time = Some(PID_DEFORM_TIME);
    v
}

#[test]
fn held_clips_and_deformers_take_driven_values() {
    use tessera_formats::script_table::resolve::DeformerCandidate;
    let m = shape_world();
    let mut r = ClipResolver::new(&shape_schema, shape_vocab(), fetch(&m));
    let mut e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    e.properties.set(PID_HEIGHT, PropertyValue::Float(0.3));
    e.properties.set(PID_BONE_SCALE, PropertyValue::Float(0.85));
    e.properties
        .set(PID_SLIDER_ARRAY, PropertyValue::String("80ff".into()));
    let res = r.resolve(&e, MotionState::Idle).unwrap();
    let held = res.held();
    assert_eq!(held.len(), 1);
    let h = held[0];
    assert_eq!((h.clip, h.layer), (Did(0x0500_8E76), 110));
    assert!(h.is_held() && h.loops());
    assert!((h.manual_time - 0.3).abs() < 1e-6);
    assert_eq!(h.manual_time_source, Some(PID_HEIGHT));
    assert!((h.held_fraction() - 0.3).abs() < 1e-6);
    // The idle clip is not held.
    assert!(
        res.candidates
            .iter()
            .any(|c| c.clip == Did(CLIP_IDLE) && !c.is_held())
    );
    assert_eq!(res.deformers.len(), 2);
    match &res.deformers[0] {
        DeformerCandidate::Mesh {
            clip,
            time,
            time_source,
            use_bone_weights,
            ..
        } => {
            assert_eq!(*clip, Did(0x0500_0E43));
            assert!((time - 0.85).abs() < 1e-6);
            assert_eq!(*time_source, Some(PID_BONE_SCALE));
            assert!(use_bone_weights);
        }
        d => panic!("{d:?}"),
    }
    match &res.deformers[1] {
        DeformerCandidate::Slider {
            definitions,
            values,
            values_source,
            ..
        } => {
            assert_eq!(*definitions, Did(0x7800_0001));
            assert_eq!(values.as_deref(), Some("80ff"));
            assert_eq!(*values_source, Some(PID_SLIDER_ARRAY));
        }
        d => panic!("{d:?}"),
    }
    // Without the driving properties: the node's own time, no value string (the caller's
    // Decision for an absent property applies, appearance.md §6.3).
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let res = r.resolve(&e, MotionState::Walk).unwrap();
    assert_eq!(res.held()[0].held_fraction(), 0.0);
    assert!(matches!(
        &res.deformers[1],
        DeformerCandidate::Slider { values: None, .. }
    ));
}

/// `script-table.md` §4.2, §6.9: a MeshDeformer's driven pair is bound by its field PID
/// (`MeshDeformerScriptNode_AnimationTime`), not by the field's type: a pair on another float
/// field (an `IntensityMultiplier`) does not set the time.
#[test]
fn mesh_deformer_pairs_are_bound_by_field_pid() {
    use tessera_formats::script_table::resolve::DeformerCandidate;
    let deformer = |driven: &[(u32, u32)]| {
        node(
            18,
            B::new().u32(0x0500_0E43).f32(0.25).u8(1),
            base_full(20, 1.0, driven),
            &[],
        )
    };
    let world = |n: Vec<u8>| {
        let mut m = HashMap::new();
        m.insert(Did(CLASS), table(2, &[], &[(STARTUP, vec![n])]));
        m
    };
    let mut e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    e.properties.set(PID_BONE_SCALE, PropertyValue::Float(0.85));
    e.properties.set(PID_OTHER_FLOAT, PropertyValue::Float(0.6));
    let time_of = |m: &HashMap<Did, Vec<u8>>| {
        let mut r = ClipResolver::new(&shape_schema, shape_vocab(), fetch(m));
        match r.resolve(&e, MotionState::Idle).unwrap().deformers.first() {
            Some(DeformerCandidate::Mesh {
                time, time_source, ..
            }) => (*time, *time_source),
            d => panic!("{d:?}"),
        }
    };
    // A pair on the intensity field (a float) leaves the stored time.
    let m = world(deformer(&[(PID_INTENSITY, PID_OTHER_FLOAT)]));
    assert_eq!(time_of(&m), (0.25, None));
    // Both pairs: only the AnimationTime pair sets it, whatever the order.
    let m = world(deformer(&[
        (PID_DEFORM_TIME, PID_BONE_SCALE),
        (PID_INTENSITY, PID_OTHER_FLOAT),
    ]));
    assert_eq!(time_of(&m), (0.85, Some(PID_BONE_SCALE)));
    // Without the field name in the dictionary nothing is driven.
    let mut v = shape_vocab();
    v.mesh_deformer.time = None;
    let mut r = ClipResolver::new(&shape_schema, v, fetch(&m));
    assert!(matches!(
        r.resolve(&e, MotionState::Idle).unwrap().deformers.first(),
        Some(DeformerCandidate::Mesh { time, time_source: None, .. }) if *time == 0.25
    ));
}

/// `script-table.md` §6.2: starting a clip never stops another clip of the same layer, so
/// two held nodes on one layer are both held clips of the figure, in evaluation order.
#[test]
fn every_held_clip_of_a_layer_is_kept() {
    let mut m = HashMap::new();
    m.insert(
        Did(CLASS),
        table(
            2,
            &[],
            &[(
                STARTUP,
                vec![
                    held_anim(2, 0x0500_8E76, true, 0.25),
                    held_anim(3, 0x0500_8E75, true, 0.5),
                ],
            )],
        ),
    );
    let mut r = ClipResolver::new(&shape_schema, shape_vocab(), fetch(&m));
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let res = r.resolve(&e, MotionState::Idle).unwrap();
    let held: Vec<(Did, u32)> = res.held().iter().map(|c| (c.clip, c.layer)).collect();
    assert_eq!(held, vec![(Did(0x0500_8E76), 110), (Did(0x0500_8E75), 110)]);
}

/// `script-table.md` §6.7 step 2: the clip time is the fraction × the played length, which
/// is the duration minus the 1/30 s end crop for `LoopForever` 0 with `LoopCount` 1 and
/// the full duration otherwise; `StartTime` and `EndTime` do not enter.
#[test]
fn held_time_uses_the_played_length() {
    use tessera_formats::script_table::resolve::{held_clip_time, held_end_crop};
    assert_eq!(held_end_crop(true, 1), 0.0);
    assert_eq!(held_end_crop(false, 2), 0.0);
    assert_eq!(held_end_crop(false, 0), 0.0);
    assert_eq!(held_end_crop(false, 1), 0.033_333_335);
    assert_eq!(held_clip_time(0.25, 2.0, 0.0), 0.5);
    // A time equal to the played length stays there.
    assert_eq!(held_clip_time(1.0, 2.0, 1.0 / 30.0), 2.0 - 1.0 / 30.0);
    assert_eq!(held_clip_time(7.0, 2.0, 0.0), 2.0);
    assert_eq!(held_clip_time(-1.0, 2.0, 0.0), 0.0);
    // Tessera guards: NaN, a clip shorter than the crop, a broken duration.
    assert_eq!(held_clip_time(f32::NAN, 2.0, 0.0), 0.0);
    assert_eq!(held_clip_time(0.5, 0.016, 1.0 / 30.0), 0.0);
    assert_eq!(held_clip_time(0.5, f32::INFINITY, 0.0), 0.0);
}

/// `script-table.md` §6.7 worked example (revised 2026-10-07, BIN5): a clip with a tagfile
/// `duration` of 2.0 s; played length 2.0 s for `LoopForever` 1 and for `LoopCount` 2,
/// 2.0 − 1/30 s for `LoopForever` 0 with `LoopCount` 1.
#[test]
fn held_fraction_example_6_7() {
    use tessera_formats::script_table::resolve::{held_clip_time, held_end_crop, held_fraction};
    let rows: [(f32, f32, f32, f32); 9] = [
        (0.25, 0.5, 0.491_666_7, 0.5),
        (0.95, 1.9, 1.868_333_3, 1.9),
        (1.0, 0.0, 1.966_666_7, 0.0),
        (1.3, 0.6, 1.966_666_7, 0.6),
        (-0.2, 1.6, 0.0, 0.0),
        (-1e-9, 2.0, 0.0, 0.0),
        (3e9, 0.0, 0.0, 0.0),
        (f32::INFINITY, 0.0, 0.0, 0.0),
        (f32::NAN, 0.0, 0.0, 0.0),
    ];
    for (v, looping, once, twice) in rows {
        let t = |lf: bool, count: i32| {
            held_clip_time(held_fraction(v, lf, count), 2.0, held_end_crop(lf, count))
        };
        assert!(
            (t(true, 1) - looping).abs() < 1e-5,
            "{v} looping: {}",
            t(true, 1)
        );
        assert!(
            (t(false, 1) - once).abs() < 1e-5,
            "{v} once: {}",
            t(false, 1)
        );
        assert!(
            (t(false, 2) - twice).abs() < 1e-5,
            "{v} twice: {}",
            t(false, 2)
        );
    }
    // §6.7 step 1 (T36): a looping fraction that rounds up to 1 stays 1.
    assert_eq!(held_fraction(-1e-9, true, 1), 1.0);
    // Values that are not finite or of magnitude 2³¹ or more give 0, on either kind.
    for v in [f32::NEG_INFINITY, -3e9, 2_147_483_648.0, -2_147_483_648.0] {
        assert_eq!(held_fraction(v, false, 1), 0.0, "{v}");
        assert_eq!(held_fraction(v, true, 1), 0.0, "{v}");
    }
    // The largest integer part that converts: n ≥ LoopCount, so 1.
    assert_eq!(held_fraction(2_147_483_520.0, false, 1), 1.0);
}

/// Hostile input: many SliderMeshDeformer nodes with long value strings. Every candidate
/// keeps its own copy of the string, so the copies are charged to the work budget
/// ([`ResolveLimits::max_work`]): what the candidates hold stays within it.
#[test]
fn slider_strings_are_charged_to_the_budget() {
    use tessera_formats::script_table::resolve::DeformerCandidate;
    let long = "8".repeat(20_000);
    let stored = |id: u32| {
        node(
            81,
            B::new().pascal(&long).u32(0x7800_0001),
            base_full(id, 1.0, &[]),
            &[],
        )
    };
    let driven = |id: u32| {
        node(
            81,
            B::new().pascal("80").u32(0x7800_0001),
            base_full(id, 1.0, &[(PID_SLIDER_VALUES, PID_SLIDER_ARRAY)]),
            &[],
        )
    };
    let held_bytes = |res: &tessera_formats::script_table::resolve::ClipResolution| -> usize {
        res.deformers
            .iter()
            .map(|d| match d {
                DeformerCandidate::Slider { values, .. } => values.as_ref().map_or(0, String::len),
                _ => 0,
            })
            .sum()
    };
    let budget = 1_000_000u64;
    for nodes in [
        (0..200).map(stored).collect::<Vec<_>>(),
        (0..200).map(driven).collect::<Vec<_>>(),
    ] {
        let mut m = HashMap::new();
        m.insert(Did(CLASS), table(2, &[], &[(STARTUP, nodes)]));
        let mut e = EntityScripts {
            class_table: Some(Did(CLASS)),
            ..EntityScripts::default()
        };
        e.properties
            .set(PID_SLIDER_ARRAY, PropertyValue::String(long.clone()));
        let mut r =
            ClipResolver::new(&shape_schema, shape_vocab(), fetch(&m)).with_limits(ResolveLimits {
                max_work: budget,
                ..ResolveLimits::default()
            });
        let res = r.resolve(&e, MotionState::Idle).unwrap();
        assert!(res.issues.contains(&ResolveIssue::Limit("max_work")));
        let held = held_bytes(&res);
        assert!(held as u64 <= budget, "{held}");
        assert!(res.deformers.len() < 50, "{}", res.deformers.len());
        // An unlimited budget registers them all.
        let mut r =
            ClipResolver::new(&shape_schema, shape_vocab(), fetch(&m)).with_limits(ResolveLimits {
                max_work: u64::MAX,
                ..ResolveLimits::default()
            });
        let res = r.resolve(&e, MotionState::Idle).unwrap();
        assert_eq!(res.deformers.len(), 200);
        assert_eq!(held_bytes(&res), 200 * 20_000);
    }
}

// --- start order of one script run (§8.2 step 3, §7.5, §7.6; T41, T42) ---

/// A table like [`table`] with per-script property sets (§2.4).
fn table_with_properties(
    kind: u32,
    includes: &[u32],
    scripts: &[(u32, Vec<Vec<u8>>)],
    properties: &[(u32, B)],
) -> Vec<u8> {
    let mut b = B::new().u32(kind).vle(includes.len() as u32);
    for i in includes {
        b = b.u32(*i);
    }
    b = b.tsize(1, scripts.len() as u32);
    for (key, roots) in scripts {
        b = b.u32(*key).vle(roots.len() as u32);
        for r in roots {
            b = b.bytes(r);
        }
    }
    b = b.tsize(1, properties.len() as u32);
    for (key, set) in properties {
        b = b.u32(*key).bytes(&set.0);
    }
    b.tsize(1, 0).build()
}

/// A per-script property set with `Script_CombinationType` = `value`.
fn combination(value: u32) -> B {
    pset(&[(PID_COMBINATION, B::new().u32(value))])
}

fn order_vocab() -> ScriptVocabulary {
    ScriptVocabulary {
        combination_type: Some(PID_COMBINATION),
        ..vocab()
    }
}

/// A Startup root that starts clip `0x0500_0000 + n` on layer 10.
fn marker(n: u32) -> Vec<Vec<u8>> {
    vec![anim(n, 0x0500_0000 + n, 10, 0.0, &[])]
}

fn clip_numbers(res: &tessera_formats::script_table::resolve::ClipResolution) -> Vec<u32> {
    res.candidates
        .iter()
        .map(|c| c.clip.0 - 0x0500_0000)
        .collect()
}

fn resolve_class(
    m: &HashMap<Did, Vec<u8>>,
    vocab: ScriptVocabulary,
) -> tessera_formats::script_table::resolve::ClipResolution {
    let mut r = ClipResolver::new(&schema, vocab, fetch(m));
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    r.resolve(&e, MotionState::Idle).unwrap()
}

/// T41: kind-1 definitions start first, then kind 2, then kind 4; within a list in
/// gathering order (pre-order, includes in stored order). Kinds are bit masks: a kind-3
/// table joins both lists, a kind-0 table neither, and a table that does not match is
/// still descended into. Null and self-naming includes are skipped without a report.
#[test]
fn run_order_is_kind_1_then_kind_2_then_kind_4() {
    const A: u32 = 0x0700_0011; // kind 1
    const B2: u32 = 0x0700_0012; // kind 2
    const C4: u32 = 0x0700_0013; // kind 4 (holds a script here, to show it runs last)
    const D: u32 = 0x0700_0014; // kind 1, below the kind-4 table
    const E3: u32 = 0x0700_0015; // kind 3: both lists
    const F0: u32 = 0x0700_0016; // kind 0: no list, still descended into
    const G: u32 = 0x0700_0017; // kind 1, below the kind-0 table
    let mut m = HashMap::new();
    m.insert(
        Did(CLASS),
        table(2, &[0, CLASS, A, B2, C4, F0], &[(STARTUP, marker(1))]),
    );
    m.insert(Did(A), table(1, &[], &[(STARTUP, marker(2))]));
    m.insert(Did(B2), table(2, &[E3], &[(STARTUP, marker(3))]));
    m.insert(Did(C4), table(4, &[D], &[(STARTUP, marker(4))]));
    m.insert(Did(D), table(1, &[], &[(STARTUP, marker(5))]));
    m.insert(Did(E3), table(3, &[], &[(STARTUP, marker(6))]));
    m.insert(Did(F0), table(0, &[G], &[(STARTUP, marker(7))]));
    m.insert(Did(G), table(1, &[], &[(STARTUP, marker(8))]));
    let res = resolve_class(&m, order_vocab());
    // List 1: A, E3, D, G; list 2: CLASS, B2, E3; list 4: C4. F0 never runs.
    assert_eq!(clip_numbers(&res), vec![2, 6, 5, 8, 1, 3, 6, 4]);
    assert!(res.closure.issues.is_empty(), "{:?}", res.closure.issues);
    assert!(!res.has_issues(), "{:?}", res.issues);
    // Each candidate is attributed to the table whose definition started it.
    assert_eq!(res.candidates[0].table, Did(A));
    assert_eq!(res.candidates[4].table, Did(CLASS));
    // The every-branch mode starts in the same order.
    let mut r = ClipResolver::new(&schema, order_vocab(), fetch(&m));
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let all = r.reachable(&e).unwrap();
    assert_eq!(clip_numbers(&all), vec![2, 6, 5, 8, 1, 3, 6, 4]);
}

/// §8.2 step 2: an overriding definition stops the descent into its table's includes, in
/// the lists the table contributes to; a table reached twice contributes once; every
/// definition of a key run by a Run node or a start-up name runs.
#[test]
fn overriding_definitions_and_runs_gather_every_definition() {
    const X1: u32 = 0x0700_0021; // kind 1 below the overriding kind-2 class table
    const X2: u32 = 0x0700_0022; // kind 2 below it: cut off
    const Y: u32 = 0x0700_0023; // an additional table, also included by X1
    const RUN_KEY: u32 = 0x1000_0500;
    const NAMED: u32 = 0x1000_0501;
    let mut m = HashMap::new();
    m.insert(
        Did(CLASS),
        table_with_properties(
            2,
            &[X1, X2],
            &[
                (STARTUP, vec![anim(1, 0x0500_0001, 10, 0.0, &[])]),
                (RUN_KEY, marker(10)),
            ],
            &[(STARTUP, combination(1)), (RUN_KEY, combination(2))],
        ),
    );
    m.insert(
        Did(X1),
        table(
            1,
            &[Y],
            &[
                (STARTUP, vec![run_node(2, RUN_KEY, false)]),
                (RUN_KEY, marker(11)),
            ],
        ),
    );
    m.insert(
        Did(X2),
        table(2, &[], &[(STARTUP, marker(3)), (RUN_KEY, marker(12))]),
    );
    m.insert(
        Did(Y),
        table(1, &[], &[(RUN_KEY, marker(13)), (NAMED, marker(20))]),
    );
    let mut r = ClipResolver::new(&schema, order_vocab(), fetch(&m));
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        additional_tables: vec![Did(Y)],
        startup_scripts: vec![NAMED],
        ..EntityScripts::default()
    };
    let res = r.resolve(&e, MotionState::Idle).unwrap();
    // Startup: list 1 = X1 (its Run of RUN_KEY: list 1 = X1, Y; list 2 = CLASS, X2,
    // because CLASS's RUN_KEY definition is additive), list 2 = CLASS only (overriding:
    // X2 is not reached). Then the start-up name NAMED (Y).
    assert_eq!(clip_numbers(&res), vec![11, 13, 10, 12, 1, 20]);
    let run = &res.candidates[0];
    assert_eq!((run.table, run.scripts.clone()), (Did(X1), vec![RUN_KEY]));
    assert!(!res.has_issues(), "{:?}", res.issues);

    // Without the combination property in the vocabulary every definition is additive.
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m));
    let res = r.resolve(&e, MotionState::Idle).unwrap();
    assert_eq!(clip_numbers(&res), vec![11, 13, 10, 12, 1, 3, 20]);
}

/// A Collection node with (ordinal, children) ports in stored order.
fn ordered_collection(id: u32, ports: &[(u32, Vec<Vec<u8>>)]) -> Vec<u8> {
    let ports: Vec<Vec<u8>> = ports
        .iter()
        .map(|(o, c)| port(6, Some(B::new().u32(*o)), c))
        .collect();
    node(2, B::new(), base(id), &ports)
}

/// T42: a Collection starts its ports by ascending ordinal (unsigned), equal ordinals in
/// stored order, and each port's children in stored order.
#[test]
fn collection_ports_start_by_ascending_ordinal() {
    let c = ordered_collection(
        1,
        &[
            (3, marker(1)),
            (1, marker(2)),
            (0x8000_0000, marker(3)),
            (
                2,
                vec![
                    anim(4, 0x0500_0004, 10, 0.0, &[]),
                    anim(5, 0x0500_0005, 10, 0.0, &[]),
                ],
            ),
            (1, marker(6)),
            (0, marker(7)),
        ],
    );
    let mut m = HashMap::new();
    m.insert(Did(CLASS), table(2, &[], &[(STARTUP, vec![c])]));
    let res = resolve_class(&m, order_vocab());
    assert_eq!(clip_numbers(&res), vec![7, 2, 6, 4, 5, 1, 3]);
}

/// §7.5 step 4: a Collection whose script run is stopped by one of its own children starts
/// no further ports; a stop of another script does not cut it, and the every-branch mode
/// ignores stops.
#[test]
fn a_collection_stopped_during_its_start_starts_no_further_ports() {
    const KEY: u32 = 0x1000_0600;
    const OTHER: u32 = 0x1000_0601;
    let c = ordered_collection(
        1,
        &[
            (3, marker(3)),
            (1, vec![run_node(2, KEY, false)]),
            (2, marker(2)),
        ],
    );
    let stopping = ordered_collection(
        10,
        &[
            (1, marker(11)),
            (2, vec![run_node(12, OTHER, true)]),
            (3, marker(13)),
            (4, vec![run_node(14, KEY, true)]),
            (5, marker(15)),
        ],
    );
    let mut m = HashMap::new();
    m.insert(
        Did(CLASS),
        table(2, &[], &[(STARTUP, vec![c]), (KEY, vec![stopping])]),
    );
    let res = resolve_class(&m, order_vocab());
    // KEY's Collection stops after its fourth port; the Startup Collection goes on.
    assert_eq!(clip_numbers(&res), vec![11, 13, 2, 3]);
    let mut r = ClipResolver::new(&schema, order_vocab(), fetch(&m));
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let all = r.reachable(&e).unwrap();
    assert_eq!(clip_numbers(&all), vec![11, 13, 15, 2, 3]);
}

/// §7.5 step 4 only cuts a Collection stopped while it is still starting: a Collection that
/// starts after its run was already stopped (a later child of the same port, a later root)
/// starts its ports, and a second stop during its own start cuts it in turn.
#[test]
fn a_collection_starting_after_its_run_was_stopped_starts_its_ports() {
    const KEY: u32 = 0x1000_0610;
    // Starts after the first stop; the second stop (port 2) cuts it before port 3.
    let later_child = ordered_collection(
        20,
        &[
            (1, marker(4)),
            (2, vec![run_node(21, KEY, true)]),
            (3, marker(6)),
        ],
    );
    let stopping = ordered_collection(
        10,
        &[
            (1, vec![run_node(11, KEY, true), later_child]),
            (2, marker(2)),
        ],
    );
    let later_root = ordered_collection(30, &[(2, marker(5)), (1, marker(3))]);
    let mut m = HashMap::new();
    m.insert(
        Did(CLASS),
        table(
            2,
            &[],
            &[
                (STARTUP, vec![run_node(1, KEY, false)]),
                (
                    KEY,
                    vec![stopping, later_root, anim(7, 0x0500_0007, 10, 0.0, &[])],
                ),
            ],
        ),
    );
    let res = resolve_class(&m, order_vocab());
    assert_eq!(clip_numbers(&res), vec![4, 3, 5, 7]);
    let mut r = ClipResolver::new(&schema, order_vocab(), fetch(&m));
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let all = r.reachable(&e).unwrap();
    assert_eq!(clip_numbers(&all), vec![4, 6, 2, 3, 5, 7]);
}

/// §7.6, `appearance.md` §6.4: Sequence steps start by ascending step number and Timer
/// ports by ascending time (a NaN time last, −0 with +0 in stored order).
#[test]
fn sequence_and_timer_ports_start_in_firing_order() {
    let ports_u32 = |ports: &[(u32, u32)]| -> Vec<Vec<u8>> {
        ports
            .iter()
            .map(|&(step, n)| port(6, Some(B::new().u32(step)), &marker(n)))
            .collect()
    };
    let sequence = node(
        6,
        B::new().u8(0).u32(1),
        base(1),
        &ports_u32(&[(3, 1), (1, 2), (2, 3), (1, 4)]),
    );
    let timer_ports: Vec<Vec<u8>> = [
        (f32::NAN, 10),
        (2.5, 11),
        (0.0, 12),
        (-1.0, 13),
        (-0.0, 14),
        (f32::INFINITY, 15),
    ]
    .iter()
    .map(|&(t, n)| port(6, Some(B::new().f32(t)), &marker(n)))
    .collect();
    let timer = node(
        65,
        B::new()
            .u8(0)
            .f32(3.0)
            .u32(1)
            .u8(0)
            .f32(1.0)
            .f32(1.0)
            .u32(0),
        base(2),
        &timer_ports,
    );
    let mut m = HashMap::new();
    m.insert(
        Did(CLASS),
        table(2, &[], &[(STARTUP, vec![sequence, timer])]),
    );
    let res = resolve_class(&m, order_vocab());
    assert_eq!(clip_numbers(&res), vec![2, 4, 3, 1, 13, 12, 14, 11, 15, 10]);
}

/// `appearance.md` §6.4 (A33): of two SliderMeshDeformer nodes, the most recently started
/// one is the last deformer of the resolution: kind-1 definitions start before kind-2
/// ones, and a Collection's ports by ascending ordinal (usually the reverse of their
/// stored order).
#[test]
fn the_most_recently_started_slider_node_is_the_last_deformer() {
    use tessera_formats::script_table::resolve::DeformerCandidate;
    const KIND1: u32 = 0x0700_0031;
    let slider = |id: u32, record: u32| node(81, B::new().pascal("80").u32(record), base(id), &[]);
    let definitions = |res: &tessera_formats::script_table::resolve::ClipResolution| {
        res.deformers
            .iter()
            .filter_map(|d| match d {
                DeformerCandidate::Slider { definitions, .. } => Some(definitions.0),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    // A Collection stored with descending ordinals: the stored-first node starts last.
    let mut m = HashMap::new();
    m.insert(
        Did(CLASS),
        table(
            2,
            &[],
            &[(
                STARTUP,
                vec![ordered_collection(
                    1,
                    &[
                        (2, vec![slider(2, 0x7800_0002)]),
                        (1, vec![slider(3, 0x7800_0001)]),
                    ],
                )],
            )],
        ),
    );
    let res = resolve_class(&m, order_vocab());
    assert_eq!(definitions(&res), vec![0x7800_0001, 0x7800_0002]);
    // The class table (kind 2) lists a kind-1 table first, yet the kind-1 definition
    // starts first, so the class table's slider node is the most recent one.
    let mut m = HashMap::new();
    m.insert(
        Did(CLASS),
        table(2, &[KIND1], &[(STARTUP, vec![slider(1, 0x7800_0002)])]),
    );
    m.insert(
        Did(KIND1),
        table(1, &[], &[(STARTUP, vec![slider(2, 0x7800_0001)])]),
    );
    let res = resolve_class(&m, order_vocab());
    assert_eq!(definitions(&res), vec![0x7800_0001, 0x7800_0002]);
}

// --- PhysicsImpulse nodes and one-script runs (`collision.md` §11.10) ---

const JUMP_KEY: u32 = 0x1000_0250;
const JUMP_TABLE: u32 = 0x0700_0700;

fn impulse_node(
    id: u32,
    is_jump: bool,
    impulse: [f32; 3],
    air: f32,
    g: f32,
    driven: &[(u32, u32)],
) -> Vec<u8> {
    let data = B::new()
        .u8(u8::from(is_jump))
        .u32(2)
        .u8(0)
        .f32(impulse[0])
        .f32(impulse[1])
        .f32(impulse[2])
        .f32(air)
        .f32(g);
    node(9, data, base_full(id, 1.0, driven), &[])
}

/// A table whose jump script switches on the final element: land (5, 6) and water (1–3)
/// jump nodes; its `Startup` holds a plain impulse that a one-script run must not start.
fn jump_world() -> HashMap<Did, Vec<u8>> {
    let land = impulse_node(
        11,
        true,
        [0.0, 0.0, 9.0],
        0.2,
        15.0,
        &[(PID_AIR_TIME, PID_JUMP_MOD)],
    );
    let water = impulse_node(12, true, [0.0; 3], 0.2, 7.5, &[]);
    let sw = switch(
        10,
        PID_FINAL_ELEMENT,
        &[
            switch_case(&[(PID_FINAL_ELEMENT, 5), (PID_FINAL_ELEMENT, 6)], &[land]),
            switch_case(
                &[
                    (PID_FINAL_ELEMENT, 1),
                    (PID_FINAL_ELEMENT, 2),
                    (PID_FINAL_ELEMENT, 3),
                ],
                &[water],
            ),
        ],
    );
    let push = impulse_node(20, false, [1.0, 2.0, 3.0], 0.0, 0.0, &[]);
    // The same jump under a While node (kind 50: property, no values, no names, not
    // inverted), whose behaviour the evaluator has no rule for.
    let inner = impulse_node(31, true, [0.0; 3], 0.3, 10.0, &[]);
    let wrapped = node(
        50,
        B::new().u32(PID_FINAL_ELEMENT).u8(0).u8(0).u8(0),
        base(30),
        &[port(6, None, &[inner])],
    );
    let mut m = HashMap::new();
    m.insert(
        Did(JUMP_TABLE),
        table(
            2,
            &[],
            &[
                (STARTUP, vec![push]),
                (JUMP_KEY, vec![sw]),
                (JUMP_KEY + 1, vec![wrapped]),
            ],
        ),
    );
    m
}

fn jump_vocab() -> ScriptVocabulary {
    ScriptVocabulary {
        jump_script: Some(JUMP_KEY),
        final_element: Some(PID_FINAL_ELEMENT),
        impulse: ImpulseFieldPids {
            air_time: Some(PID_AIR_TIME),
            ..ImpulseFieldPids::default()
        },
        ..vocab()
    }
}

#[test]
fn physics_impulse_nodes_decode() {
    let m = jump_world();
    let t = ScriptTable::parse(&m[&Did(JUMP_TABLE)], &schema).unwrap();
    let push = &t.scripts[0].roots[0];
    assert_eq!(push.kind, NodeKind::PHYSICS_IMPULSE);
    let p = push.physics_impulse().expect("typed data");
    assert!(!p.is_jump && !p.global_space);
    assert_eq!((p.impulse_type, p.impulse), (2, [1.0, 2.0, 3.0]));
    let NodeData::Switch(_) = &t.scripts[1].roots[0].data else {
        panic!("switch");
    };
    let land = &t.scripts[1].roots[0].ports[0].children[0];
    let p = land.physics_impulse().unwrap();
    assert!(p.is_jump);
    assert_eq!((p.air_time, p.expected_gravity), (0.2, 15.0));
}

#[test]
fn one_script_run_lists_the_jump_node_of_each_state() {
    let m = jump_world();
    let mut r = ClipResolver::new(&schema, jump_vocab(), fetch(&m));
    let e = EntityScripts {
        class_table: Some(Did(JUMP_TABLE)),
        ..EntityScripts::default()
    };
    let state = |element: u32, jump_mod: Option<f32>| {
        let mut st = PropertyState::new();
        st.set(PID_FINAL_ELEMENT, PropertyValue::Enum(element));
        if let Some(v) = jump_mod {
            st.set(PID_JUMP_MOD, PropertyValue::Float(v));
        }
        st
    };
    let land = r
        .resolve_script(&e, JUMP_KEY, state(5, None))
        .unwrap()
        .impulses;
    assert_eq!(land.len(), 1, "only the jump script runs: {land:?}");
    assert!(land[0].node.is_jump);
    assert_eq!(
        (land[0].node.air_time, land[0].node.expected_gravity),
        (0.2, 15.0)
    );
    assert_eq!((land[0].chance, land[0].table), (1.0, Did(JUMP_TABLE)));
    let g = |r: &mut ClipResolver<'_, _, _, _, _>, element: u32| {
        r.resolve_script(&e, JUMP_KEY, state(element, None))
            .unwrap()
            .impulses
            .first()
            .map(|c| c.node.expected_gravity)
    };
    assert_eq!(g(&mut r, 6), Some(15.0));
    for w in [1, 2, 3] {
        assert_eq!(g(&mut r, w), Some(7.5));
    }
    assert_eq!(g(&mut r, 4), None, "no jump in the air");
    // A driven field wins over the stored value.
    let driven = r
        .resolve_script(&e, JUMP_KEY, state(5, Some(0.285)))
        .unwrap();
    assert_eq!(driven.impulses[0].node.air_time, 0.285);
    // An unknown key runs nothing; the normal resolution sees the Startup impulse.
    let none = r
        .resolve_script(&e, 0x1000_0999, PropertyState::new())
        .unwrap();
    assert!(none.impulses.is_empty() && none.candidates.is_empty());
    let all = r.resolve(&e, MotionState::Idle).unwrap();
    assert_eq!(all.impulses.len(), 1);
    assert_eq!(all.impulses[0].node.impulse, [1.0, 2.0, 3.0]);
}

/// A While node (kind 50, `script-table.md` §5.2): property, `Values` (bool PVs), no
/// names, `Inverse`; one Default port.
fn while_node(
    id: u32,
    property: u32,
    values: &[bool],
    inverse: bool,
    children: &[Vec<u8>],
) -> Vec<u8> {
    let mut data = B::new().u32(property).u8(values.len() as u8);
    for &v in values {
        data = data.u32(property).u8(u8::from(v));
    }
    node(
        50,
        data.u8(0).u8(u8::from(inverse)),
        base(id),
        &[port(6, None, children)],
    )
}

/// The shape of the avatar's `Loc_Jumping` (`script-runtime.md` §30.7 item 3): a Collection
/// whose ordinal 1 sets the flag and whose ordinal 4 is a While on it; inside, a one-pass
/// Sequence: step 1 the jump impulse, step 2 the clear of the flag, step 3 an impulse that
/// must not start (the While stops its branch at the clear, §30.4).
fn while_world(inverse: bool) -> HashMap<Did, Vec<u8>> {
    let jump = impulse_node(41, true, [0.0; 3], 0.2, 15.0, &[]);
    let late = impulse_node(42, true, [0.0; 3], 0.5, 1.0, &[]);
    let clear = set_property_node(43, PID_IS_JUMPING, B::new().u8(0));
    let step = |n: u32, c: Vec<u8>| port(6, Some(B::new().u32(n)), &[c]);
    let seq = node(
        6,
        B::new().u8(0).u32(1),
        base(44),
        &[step(1, jump), step(2, clear), step(3, late)],
    );
    let w = while_node(45, PID_IS_JUMPING, &[true], inverse, &[seq]);
    let set = set_property_node(46, PID_IS_JUMPING, B::new().u8(1));
    let coll = node(
        2,
        B::new(),
        base(47),
        &[
            port(6, Some(B::new().u32(4)), &[w]),
            port(6, Some(B::new().u32(1)), &[set]),
        ],
    );
    let mut m = HashMap::new();
    m.insert(Did(JUMP_TABLE), table(2, &[], &[(JUMP_KEY, vec![coll])]));
    m
}

#[test]
fn while_nodes_decode_their_fields() {
    let m = while_world(false);
    let t = ScriptTable::parse(&m[&Did(JUMP_TABLE)], &schema).unwrap();
    let w = &t.scripts[0].roots[0].ports[0].children[0];
    assert_eq!(w.kind, NodeKind::WHILE);
    let NodeData::While(d) = &w.data else {
        panic!("typed While data: {:?}", w.data);
    };
    assert_eq!(d.property, PID_IS_JUMPING);
    assert_eq!(d.values.len(), 1);
    assert_eq!(
        d.values[0].as_ref().map(|e| (e.pid, e.value.clone())),
        Some((PID_IS_JUMPING, PropertyValue::Bool(true)))
    );
    assert!(d.names.is_empty() && !d.inverse);
    assert_eq!(d.name_pids().count(), 0);
}

#[test]
fn while_runs_its_branch_while_the_test_holds() {
    let e = EntityScripts {
        class_table: Some(Did(JUMP_TABLE)),
        ..EntityScripts::default()
    };
    // The Collection sets the flag first (ordinal 1), so the While's test holds and its
    // branch starts; the clear in step 2 stops the branch: step 3 never starts (§30.4).
    let m = while_world(false);
    let mut r = ClipResolver::new(&schema, jump_vocab(), fetch(&m)).unset_reads_as_zero(true);
    let res = r
        .resolve_script(&e, JUMP_KEY, PropertyState::new())
        .unwrap();
    assert_eq!(res.impulses.len(), 1, "{:?}", res.impulses);
    assert_eq!(res.impulses[0].node.air_time, 0.2);
    // Inverted, the test fails once the flag is set: nothing starts (§30.3 step 4).
    let m = while_world(true);
    let mut r = ClipResolver::new(&schema, jump_vocab(), fetch(&m)).unset_reads_as_zero(true);
    let res = r
        .resolve_script(&e, JUMP_KEY, PropertyState::new())
        .unwrap();
    assert!(res.impulses.is_empty(), "{:?}", res.impulses);
    // The every-branch scan reaches both impulses.
    let m = while_world(true);
    let mut r = ClipResolver::new(&schema, jump_vocab(), fetch(&m));
    assert_eq!(
        r.reachable(&EntityScripts {
            class_table: Some(Did(JUMP_TABLE)),
            startup_scripts: vec![JUMP_KEY],
            ..EntityScripts::default()
        })
        .unwrap()
        .impulses
        .len(),
        2
    );
}

#[test]
fn a_random_node_with_one_port_always_picks_it() {
    // `script-table.md` §7.4 rule 1: even at weight 0.
    let m = {
        let mut m = HashMap::new();
        let r = random(
            1,
            &[(0, vec![impulse_node(2, true, [0.0; 3], 0.2, 15.0, &[])])],
        );
        m.insert(Did(JUMP_TABLE), table(2, &[], &[(JUMP_KEY, vec![r])]));
        m
    };
    let e = EntityScripts {
        class_table: Some(Did(JUMP_TABLE)),
        ..EntityScripts::default()
    };
    let mut r = ClipResolver::new(&schema, jump_vocab(), fetch(&m)).with_random_seed(Some(7));
    let res = r
        .resolve_script(&e, JUMP_KEY, PropertyState::new())
        .unwrap();
    assert_eq!(res.impulses.len(), 1);
    assert_eq!(res.impulses[0].chance, 1.0);
}

/// An Appearance node (kind 58, §5.1): record, key, modifier, persistent.
fn appearance_node(id: u32, record: u32, key: u32, modifier: f32, p: f32) -> Vec<u8> {
    node(
        58,
        B::new().u32(record).u32(key).f32(modifier).u8(1),
        base_full(id, p, &[]),
        &[],
    )
}

/// `appearance.md` §7.4: a Switch on a random-appearance type (here the mood property)
/// whose case is a Collection of two piece groups (Random nodes over Appearance nodes),
/// plus an optional piece with probability 0.25 and a plain Appearance node outside any
/// Random. Seeded picks choose one piece per group; the candidates keep the client's start
/// order (a Collection's ports by ascending ordinal, `script-table.md` §7.5).
#[test]
fn appearance_nodes_are_listed_and_picked_by_seed() {
    const REC: u32 = 0x2000_0001;
    const KEY: u32 = 0x1000_003A;
    let shirts = random(
        40,
        &[
            (1, vec![appearance_node(41, REC, KEY, 0.01, 1.0)]),
            (1, vec![appearance_node(42, REC, KEY, 0.02, 1.0)]),
        ],
    );
    let hair = random(
        50,
        &[
            (1, vec![appearance_node(51, REC + 1, KEY, 0.0, 1.0)]),
            (3, vec![appearance_node(52, REC + 1, KEY, 0.01, 1.0)]),
            (0, vec![appearance_node(53, REC + 1, KEY, 0.99, 1.0)]),
        ],
    );
    // Stored with descending ordinals: the hair group (ordinal 1) starts first.
    let case = ordered_collection(
        30,
        &[
            (3, vec![appearance_node(31, REC + 2, KEY, 1.0, 0.25)]),
            (2, vec![shirts]),
            (1, vec![hair]),
        ],
    );
    let root = switch(
        20,
        PID_MOOD,
        &[
            switch_case(&[(PID_MOOD, 9)], &[case]),
            default_port_pair(&[appearance_node(21, REC + 3, KEY, 1.0, 1.0)]),
        ],
    );
    let plain = appearance_node(10, REC + 4, KEY, 1.0, 1.0);
    let mut m = HashMap::new();
    m.insert(Did(CLASS), table(1, &[], &[(STARTUP, vec![plain, root])]));
    let mut e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    e.properties.set(PID_MOOD, PropertyValue::Enum(9));

    // Without a seed: every node of the case is listed (static analysis), only the node
    // outside Random and probability is picked.
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m));
    let res = r.resolve(&e, MotionState::Idle).unwrap();
    let ids: Vec<u32> = res.appearances.iter().map(|a| a.node_id).collect();
    assert_eq!(
        ids,
        vec![10, 51, 52, 41, 42, 31],
        "start order; weight 0 never runs"
    );
    let picked: Vec<u32> = res
        .appearances
        .iter()
        .filter(|a| a.picked)
        .map(|a| a.node_id)
        .collect();
    assert_eq!(picked, vec![10]);
    let a52 = res.appearances.iter().find(|a| a.node_id == 52).unwrap();
    assert!((a52.chance - 0.75).abs() < 1e-6);
    assert_eq!(
        (a52.appearance, a52.key, a52.modifier),
        (Did(REC + 1), KEY, 0.01)
    );

    // With a seed: one piece per group, the same picks for the same seed, and over many
    // seeds every alternative with a weight occurs, the optional piece sometimes.
    let picks = |seed: u64| {
        let mut r = ClipResolver::new(&schema, vocab(), fetch(&m)).with_random_seed(Some(seed));
        let res = r.resolve(&e, MotionState::Idle).unwrap();
        res.appearances
            .iter()
            .filter(|a| a.picked)
            .map(|a| a.node_id)
            .collect::<Vec<u32>>()
    };
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..64u64 {
        let p = picks(seed);
        assert_eq!(p, picks(seed), "seed {seed}: deterministic");
        assert_eq!(p.first(), Some(&10));
        assert_eq!(
            p.iter().filter(|&&n| n == 51 || n == 52).count(),
            1,
            "{p:?}"
        );
        assert_eq!(
            p.iter().filter(|&&n| n == 41 || n == 42).count(),
            1,
            "{p:?}"
        );
        assert!(!p.contains(&53));
        seen.extend(p);
    }
    for n in [41, 42, 51, 52, 31] {
        assert!(seen.contains(&n), "node {n} never picked: {seen:?}");
    }
    // Another type: the Default port's piece.
    e.properties.set(PID_MOOD, PropertyValue::Enum(1));
    let mut r = ClipResolver::new(&schema, vocab(), fetch(&m)).with_random_seed(Some(1));
    let res = r.resolve(&e, MotionState::Idle).unwrap();
    let ids: Vec<u32> = res.appearances.iter().map(|a| a.node_id).collect();
    assert_eq!(ids, vec![10, 21]);
    assert!(res.appearances.iter().all(|a| a.picked));
}

// --- MeshFX candidates and the mount mesh (`mounts.md` §4.4, §11.4) ---

const MESH_SCRIPT: u32 = 0x1000_0500;
const CHANNELLING: u32 = 0x1000_0501;
const SPECIES_A: u32 = 7;
const SPECIES_B: u32 = 8;
const GOAT: u32 = 0x4;
const MOUNT_ENTITY: u32 = 0x4700_0099;

/// The synthetic table of §11.4: `Mounted_Player_Cosmetic_Mesh` is a Switch on the mount
/// type with a goat case, a When on the slot entity (not the null DID), then a species
/// Switch: species A gets a node of stored scale 1.05 that does not drive `Scale`, the
/// default a node that drives it. `Startup` holds a WhenRunning on the channelling script
/// whose Running port starts a clip.
fn mount_world() -> HashMap<Did, Vec<u8>> {
    let fx_a = node(
        41,
        mesh_fx_data(0, 0, [0.0; 3], 1.05, false),
        base_full(11, 1.0, &[(PID_FX_ENTITY, PID_SLOT_ENTITY)]),
        &[],
    );
    let fx_default = node(
        41,
        mesh_fx_data(0, 0, [0.0; 3], 1.0, false),
        base_full(
            12,
            1.0,
            &[
                (PID_FX_ENTITY, PID_SLOT_ENTITY),
                (PID_FX_SCALE, PID_SLOT_SCALE),
            ],
        ),
        &[],
    );
    let species = switch(
        10,
        PID_SPECIES,
        &[
            switch_case(&[(PID_SPECIES, SPECIES_A)], &[fx_a]),
            default_port_pair(&[fx_default]),
        ],
    );
    let not_null = port(
        24,
        Some(
            B::new()
                .u32(PID_SLOT_ENTITY)
                .u32(0)
                .u32(0)
                .u32(3) // NotEqual
                .u32(0)
                .u32(0),
        ),
        &[species],
    );
    let when = node(
        63,
        B::new().u32(PID_SLOT_ENTITY).u8(1).u8(0).u8(0),
        base(9),
        &[not_null],
    );
    let by_type = switch(
        8,
        PID_MOUNT_TYPE,
        &[switch_case(&[(PID_MOUNT_TYPE, GOAT)], &[when])],
    );
    let running = node(
        14,
        B::new().u8(1).u32(CHANNELLING).u8(0).u8(1),
        base(20),
        &[
            port(11, None, &[anim(21, CLIP_WALK, 10, 0.0, &[])]), // Running
            port(10, None, &[anim(22, CLIP_IDLE, 10, 0.0, &[])]), // NotRunning
        ],
    );
    let mut m = HashMap::new();
    m.insert(
        Did(CLASS),
        table(
            1,
            &[],
            &[(STARTUP, vec![running]), (MESH_SCRIPT, vec![by_type])],
        ),
    );
    m
}

fn mount_vocab() -> ScriptVocabulary {
    ScriptVocabulary {
        mesh_fx: tessera_formats::script_table::resolve::MeshFxFieldPids {
            entity: Some(PID_FX_ENTITY),
            scale: Some(PID_FX_SCALE),
            ..Default::default()
        },
        ..vocab()
    }
}

fn rider(species: u32, entity: u32) -> EntityScripts {
    let mut properties = PropertyState::new();
    properties.set(PID_MOUNT_TYPE, PropertyValue::Enum(GOAT));
    properties.set(PID_SPECIES, PropertyValue::Enum(species));
    properties.set(PID_SLOT_ENTITY, PropertyValue::DidRef(Did(entity)));
    properties.set(PID_SLOT_SCALE, PropertyValue::Vector3([0.925; 3]));
    EntityScripts {
        class_table: Some(Did(CLASS)),
        startup_scripts: vec![MESH_SCRIPT],
        properties,
        ..EntityScripts::default()
    }
}

#[test]
fn worked_example_mount_scale() {
    use tessera_formats::gameplay::mounts::MountMesh;
    let m = mount_world();
    let mut r = ClipResolver::new(&schema, mount_vocab(), fetch(&m));
    // Species A on the goat: the stored 1.05; the effect's 0.925 is ignored.
    let res = r
        .resolve_with_state(
            &rider(SPECIES_A, MOUNT_ENTITY),
            rider(SPECIES_A, MOUNT_ENTITY).properties,
        )
        .unwrap();
    let mesh = MountMesh::choose(&res.mesh_fx, MESH_SCRIPT).unwrap();
    assert_eq!(mesh.entity, Did(MOUNT_ENTITY));
    assert_eq!(mesh.scale, [1.05; 3]);
    assert!(!mesh.scale_driven);
    assert_eq!(mesh.node_id, 11);
    assert_eq!(res.mesh_fx[0].scripts, vec![MESH_SCRIPT]);
    // Another species: the default node drives the scale: 0.925.
    let e = rider(SPECIES_B, MOUNT_ENTITY);
    let res = r.resolve_with_state(&e, e.properties.clone()).unwrap();
    let mesh = MountMesh::choose(&res.mesh_fx, MESH_SCRIPT).unwrap();
    assert_eq!(mesh.scale, [0.925; 3]);
    assert!(mesh.scale_driven);
    // `UseParentScale` is off in these nodes: the rider's model scale does not apply.
    assert_eq!(mesh.draw_scale([1.2; 3]), [0.925; 3]);
    let t = mesh.local_transform([1.0; 3]);
    assert_eq!((t[0], t[5], t[10], t[15]), (0.925, 0.925, 0.925, 1.0));
    // A rider without an entity (the null DID): the When node stops the branch.
    let e = rider(SPECIES_A, 0);
    let res = r.resolve_with_state(&e, e.properties.clone()).unwrap();
    assert!(res.mesh_fx.is_empty());
    assert_eq!(MountMesh::choose(&res.mesh_fx, MESH_SCRIPT), None);
}

/// `appearance.md` §5.7 step 3 (A36): a driven `Scale` whose source property the figure
/// lacks takes the property's dictionary default, (1, 1, 1) for the vector properties.
#[test]
fn absent_driven_mesh_fx_sources_read_their_default() {
    use tessera_formats::gameplay::mounts::MountMesh;
    let m = mount_world();
    let mut e = rider(SPECIES_B, MOUNT_ENTITY);
    let mut props = PropertyState::new();
    props.set(PID_MOUNT_TYPE, PropertyValue::Enum(GOAT));
    props.set(PID_SPECIES, PropertyValue::Enum(SPECIES_B));
    props.set(PID_SLOT_ENTITY, PropertyValue::DidRef(Did(MOUNT_ENTITY)));
    e.properties = props;
    let mut r = ClipResolver::new(&schema, mount_vocab(), fetch(&m))
        .with_defaults(|pid| (pid == PID_SLOT_SCALE).then_some(PropertyValue::Vector3([1.0; 3])));
    let res = r.resolve_with_state(&e, e.properties.clone()).unwrap();
    let mesh = MountMesh::choose(&res.mesh_fx, MESH_SCRIPT).unwrap();
    assert_eq!(mesh.scale, [1.0; 3]);
    assert!(mesh.scale_driven);
}

#[test]
fn running_keys_select_when_running_branches() {
    let m = mount_world();
    let mut r = ClipResolver::new(&schema, mount_vocab(), fetch(&m));
    let mut e = rider(SPECIES_A, MOUNT_ENTITY);
    e.startup_scripts.clear();
    let res = r.resolve_with_state(&e, e.properties.clone()).unwrap();
    assert_eq!(res.clips(), vec![Did(CLIP_IDLE)], "not running");
    e.running = vec![CHANNELLING];
    let res = r.resolve_with_state(&e, e.properties.clone()).unwrap();
    assert_eq!(res.clips(), vec![Did(CLIP_WALK)], "running before Startup");
}

// --- face compositing and colour edits (appearance.md §3.5) ---

fn attach_node(id: u32, name: u32, file: u32, mesh: u32, material: u32) -> Vec<u8> {
    node(
        53,
        B::new().u32(name).u32(file).u32(mesh).u32(material),
        base(id),
        &[],
    )
}

fn layer_node(id: u32, name: u32, layer: u32, image: i32, driven: &[(u32, u32)]) -> Vec<u8> {
    node(
        44,
        B::new()
            .u32(name)
            .u32(layer)
            .bytes(&image.to_le_bytes())
            .f32(1.0),
        base_full(id, 1.0, driven),
        &[],
    )
}

fn colour_node(id: u32, name: u32, colour: u32, bgra: [u8; 4], driven: &[(u32, u32)]) -> Vec<u8> {
    node(
        33,
        B::new().u32(name).u32(colour).bytes(&bgra),
        base_full(id, 1.0, driven),
        &[],
    )
}

fn edit_node(id: u32, mesh: u32, material: u32, priority: i32, driven: &[(u32, u32)]) -> Vec<u8> {
    node(
        60,
        B::new()
            .u32(2003)
            .u32(mesh)
            .u32(material)
            .bytes(&priority.to_le_bytes())
            .u32(1) // waveform subtype 1: one float
            .f32(0.5)
            .bytes(&[0xFF; 4])
            .u32(0)
            .u8(0),
        base_full(id, 1.0, driven),
        &[],
    )
}

#[test]
fn compositor_and_edit_nodes_decode_every_field() {
    let t = table(
        2,
        &[],
        &[(
            STARTUP,
            vec![
                attach_node(1, 1, 0x3300_0001, 0x1000_0002, 0x1000_006C),
                layer_node(2, 1, 0x1000_0004, -3, &[]),
                colour_node(3, 1, 0x1000_000A, [1, 2, 3, 4], &[]),
                edit_node(4, 2, 0x1000_0073, 5, &[]),
            ],
        )],
    );
    let t = ScriptTable::parse(&t, &schema).unwrap();
    let roots = &t.scripts[0].roots;
    match &roots[0].data {
        NodeData::TextureCompositor(a) => {
            assert_eq!(a.compositor, 1);
            assert_eq!(a.file, Did(0x3300_0001));
            assert_eq!((a.mesh_type, a.material_type), (0x1000_0002, 0x1000_006C));
        }
        other => panic!("{other:?}"),
    }
    match &roots[1].data {
        NodeData::CompositorLayer(l) => {
            assert_eq!(
                (l.compositor, l.layer, l.image_number),
                (1, 0x1000_0004, -3)
            );
            assert_eq!(l.opacity, 1.0);
        }
        other => panic!("{other:?}"),
    }
    match &roots[2].data {
        NodeData::CompositorColour(c) => {
            assert_eq!(
                (c.compositor, c.colour, c.color),
                (1, 0x1000_000A, [1, 2, 3, 4])
            );
        }
        other => panic!("{other:?}"),
    }
    match &roots[3].data {
        NodeData::EditMaterial(e) => {
            assert_eq!(e.material_property, 2003);
            assert_eq!(
                (e.mesh_type, e.material_type, e.priority),
                (2, 0x1000_0073, 5)
            );
            assert_eq!(e.waveform_value.subtype, 1);
            assert_eq!(e.color_value, [0xFF; 4]);
            assert!(!e.bool_value);
        }
        other => panic!("{other:?}"),
    }
    // Truncated compositor data is malformed, never a panic.
    let mut bytes = table(2, &[], &[(STARTUP, vec![edit_node(4, 2, 3, 5, &[])])]);
    bytes.truncate(bytes.len() - 20);
    assert!(ScriptTable::parse(&bytes, &schema).is_err());
}

const K_NEW_FACE: u32 = 0x1000_2ACD;
const K_OLD_FACE: u32 = 0x1000_2ACE;

/// `Startup` runs the older face, then a switch on the option runs the newer one and
/// stops the older; the newer face's nodes are driven by the figure's properties.
fn face_world() -> HashMap<Did, Vec<u8>> {
    let mut m = HashMap::new();
    let startup = vec![
        run_node(1, K_OLD_FACE, false),
        switch(
            2,
            PID_LOOK_OPTION,
            &[
                switch_case(
                    &[(PID_LOOK_OPTION, 1)],
                    &[
                        run_node(3, K_NEW_FACE, false),
                        run_node(4, K_OLD_FACE, true),
                    ],
                ),
                default_port_pair(&[]),
            ],
        ),
    ];
    m.insert(
        Did(CLASS),
        table(
            2,
            &[],
            &[
                (STARTUP, startup),
                (
                    K_OLD_FACE,
                    vec![
                        attach_node(10, 0x0A, 0x3300_0002, 0x1000_0002, 0x1000_006C),
                        colour_node(11, 0x0A, 0x1000_000B, [0xFF; 4], &[]),
                    ],
                ),
                (
                    K_NEW_FACE,
                    vec![
                        attach_node(20, 1, 0x3300_0001, 0x1000_0002, 0x1000_006C),
                        layer_node(
                            21,
                            1,
                            0x1000_0004,
                            0,
                            &[(PID_FIELD_IMAGE, PID_LOOK_TEXTURE)],
                        ),
                        colour_node(
                            22,
                            1,
                            0x1000_000B,
                            [0xFF; 4],
                            &[(PID_FIELD_COLOUR, PID_LOOK_SKIN)],
                        ),
                        edit_node(23, 2, 0x1000_006C, 5, &[(PID_EDIT_COLOR, PID_LOOK_SKIN)]),
                    ],
                ),
            ],
        ),
    );
    m
}

fn face_vocab() -> ScriptVocabulary {
    use tessera_formats::script_table::resolve::{CompositorFieldPids, EditMaterialFieldPids};
    ScriptVocabulary {
        edit_material: EditMaterialFieldPids {
            color_value: Some(PID_EDIT_COLOR),
            ..EditMaterialFieldPids::default()
        },
        compositor: CompositorFieldPids {
            image_number: Some(PID_FIELD_IMAGE),
            colour: Some(PID_FIELD_COLOUR),
            ..CompositorFieldPids::default()
        },
        ..vocab()
    }
}

/// A driven pair sets the compositor field its PID names, and nothing when the PID names
/// no field of the node's kind (`appearance.md` §3.5 "Field names").
#[test]
fn compositor_fields_are_driven_by_pid() {
    use tessera_formats::script_table::resolve::LookEffect;
    let mut m = HashMap::new();
    m.insert(
        Did(CLASS),
        table(
            2,
            &[],
            &[(
                STARTUP,
                vec![
                    // The image number by its field; the colour PID on a layer node is
                    // not a field of that kind.
                    layer_node(1, 1, 4, 0, &[(PID_FIELD_IMAGE, PID_LOOK_TEXTURE)]),
                    layer_node(2, 1, 5, 0, &[(PID_FIELD_COLOUR, PID_LOOK_TEXTURE)]),
                ],
            )],
        ),
    );
    let mut r = ClipResolver::new(&schema, face_vocab(), fetch(&m));
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let mut state = PropertyState::new();
    state.set(PID_LOOK_TEXTURE, PropertyValue::Int(3));
    let res = r.resolve_with_state(&e, state).unwrap();
    let numbers: Vec<i32> = res
        .look
        .iter()
        .map(|l| match &l.effect {
            LookEffect::CompositorLayer(l) => l.image_number,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(numbers, [3, 0]);
    assert!(res.look[0].driven && !res.look[1].driven);
}

/// A Switch node that does not keep watching (`…_Persistent` false).
fn switch_once(id: u32, property: u32, ports: &[Vec<u8>]) -> Vec<u8> {
    node(
        12,
        B::new().u32(property).u8(0).u8(0).u8(0),
        base(id),
        ports,
    )
}

/// The elf pattern of `appearance.md` §3.5: a persistent Switch on the placement flag
/// whose case runs a one-time check of a flag that a later root of the start-up clears.
fn placement_world() -> HashMap<Did, Vec<u8>> {
    let mut m = HashMap::new();
    let check = switch_once(
        11,
        PID_SERVER_FLAG,
        &[switch_case(
            &[(PID_SERVER_FLAG, 0)],
            &[run_node(12, K_NEW_FACE, false)],
        )],
    );
    let startup = vec![
        switch(
            10,
            PID_IN_WORLD,
            &[
                switch_case(&[(PID_IN_WORLD, 1)], &[check]),
                default_port_pair(&[colour_node(13, 9, 9, [1, 2, 3, 4], &[])]),
            ],
        ),
        set_property_node(14, PID_SERVER_FLAG, B::new().u32(0)),
    ];
    m.insert(
        Did(CLASS),
        table(
            2,
            &[],
            &[
                (STARTUP, startup),
                (
                    K_NEW_FACE,
                    vec![attach_node(20, 1, 0x3300_0001, 0x1000_0002, 0x1000_006C)],
                ),
            ],
        ),
    );
    m
}

#[test]
fn persistent_switch_re_evaluates_after_the_start_up() {
    use tessera_formats::script_table::resolve::LookEffect;
    let m = placement_world();
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let start = || {
        let mut state = PropertyState::new();
        state.set(PID_SERVER_FLAG, PropertyValue::Enum(1));
        state
    };
    // Without a placement only the Default branch runs.
    let mut r = ClipResolver::new(&schema, face_vocab(), fetch(&m));
    let res = r.resolve_with_state(&e, start()).unwrap();
    assert_eq!(res.look.len(), 1);
    assert!(res.look[0].active());
    // Placed before the start-up: the check runs before the flag is cleared.
    let mut placed = start();
    placed.set(PID_IN_WORLD, PropertyValue::Enum(1));
    let res = r.resolve_with_state(&e, placed).unwrap();
    assert!(res.look.is_empty(), "{:?}", res.look);
    // Placed after the start-up: the Switch re-evaluates in the end state, the check sees
    // the cleared flag, the face attaches and the Default branch stops.
    let mut r = ClipResolver::new(&schema, face_vocab(), fetch(&m))
        .with_after_startup(vec![(PID_IN_WORLD, PropertyValue::Enum(1))]);
    let res = r.resolve_with_state(&e, start()).unwrap();
    assert_eq!(res.look.len(), 2, "{:?}", res.look);
    assert!(res.look[0].stopped);
    assert_eq!(res.look[0].stopped_at, Some(1));
    assert!(res.look[1].active());
    assert!(matches!(
        res.look[1].effect,
        LookEffect::AttachCompositor(_)
    ));
    assert_eq!(res.look[1].scripts, [K_NEW_FACE]);
    // Nothing watches another property: one pass, same result as without.
    let mut r = ClipResolver::new(&schema, face_vocab(), fetch(&m))
        .with_after_startup(vec![(PID_LOOK_OPTION, PropertyValue::Enum(1))]);
    let res = r.resolve_with_state(&e, start()).unwrap();
    assert_eq!(res.look.len(), 1);
    assert!(res.look[0].active());
}

/// A Run node (kind 5) with its `Persistent` and `UnbindOnDeath` flags.
fn run_node_flags(id: u32, key: u32, persistent: bool, unbind: bool) -> Vec<u8> {
    node(
        5,
        B::new()
            .u8(1)
            .u8(0)
            .u32(key)
            .u8(u8::from(persistent))
            .u8(u8::from(unbind))
            .u8(0)
            .u8(0)
            .u32(3),
        base(id),
        &[],
    )
}

/// `script-runtime.md` §6.6 R56: when a persistent Switch changes case after the start-up,
/// the whole left branch stops, and with it the runs of its `Persistent` 1,
/// `UnbindOnDeath` 0 Run nodes; the runs that `Persistent` 0 or `UnbindOnDeath` 1 Runs of
/// the branch started keep running, at any depth (a binding Run inside a kept run is kept
/// with it; a non-binding Run inside a stopped run keeps its own run).
#[test]
fn a_case_change_stops_only_the_runs_bound_to_the_left_branch() {
    const K_P0: u32 = 0x1000_2B01;
    const K_P1: u32 = 0x1000_2B02;
    const K_UNBIND: u32 = 0x1000_2B03;
    const K_P1_OUTER: u32 = 0x1000_2B04;
    const K_P0_INNER: u32 = 0x1000_2B05;
    const K_P0_OUTER: u32 = 0x1000_2B06;
    const K_P1_INNER: u32 = 0x1000_2B07;
    let mut m = HashMap::new();
    let left = vec![
        colour_node(11, 9, 9, [1, 2, 3, 4], &[]),
        run_node_flags(12, K_P0, false, false),
        run_node_flags(13, K_P1, true, false),
        run_node_flags(14, K_UNBIND, true, true),
        run_node_flags(15, K_P1_OUTER, true, false),
        run_node_flags(16, K_P0_OUTER, false, false),
    ];
    let startup = vec![switch(
        10,
        PID_IN_WORLD,
        &[
            switch_case(&[(PID_IN_WORLD, 1)], &[]),
            default_port_pair(&left),
        ],
    )];
    m.insert(
        Did(CLASS),
        table(
            2,
            &[],
            &[
                (STARTUP, startup),
                (
                    K_P0,
                    vec![attach_node(20, 1, 0x3300_0001, 0x1000_0002, 0x1000_006C)],
                ),
                (K_P1, vec![layer_node(21, 1, 4, 0, &[])]),
                (K_UNBIND, vec![layer_node(22, 1, 5, 0, &[])]),
                (
                    K_P1_OUTER,
                    vec![
                        colour_node(23, 1, 9, [5, 6, 7, 8], &[]),
                        run_node_flags(24, K_P0_INNER, false, false),
                    ],
                ),
                (K_P0_INNER, vec![layer_node(25, 1, 6, 0, &[])]),
                (
                    K_P0_OUTER,
                    vec![run_node_flags(26, K_P1_INNER, true, false)],
                ),
                (K_P1_INNER, vec![layer_node(27, 1, 7, 0, &[])]),
            ],
        ),
    );
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let state = |res: &tessera_formats::script_table::resolve::ClipResolution| {
        res.look
            .iter()
            .map(|l| (l.node_id, l.stopped, l.stopped_at))
            .collect::<Vec<_>>()
    };
    // Before the placement every node of the Default branch runs.
    let mut r = ClipResolver::new(&schema, face_vocab(), fetch(&m));
    let res = r.resolve_with_state(&e, PropertyState::new()).unwrap();
    assert!(res.look.iter().all(|l| l.active()), "{:?}", res.look);
    assert_eq!(res.look.len(), 7);
    // Placed after the start-up: the case changes to the empty one.
    let mut r = ClipResolver::new(&schema, face_vocab(), fetch(&m))
        .with_after_startup(vec![(PID_IN_WORLD, PropertyValue::Enum(1))]);
    let res = r.resolve_with_state(&e, PropertyState::new()).unwrap();
    let stop = Some(7);
    assert_eq!(
        state(&res),
        [
            // The branch's own node stops.
            (11, true, stop),
            // `Persistent` 0: the face stays attached.
            (20, false, None),
            // `Persistent` 1, `UnbindOnDeath` 0: stopped with the branch.
            (21, true, stop),
            // `UnbindOnDeath` 1: kept.
            (22, false, None),
            // A binding run stops, but the `Persistent` 0 run it started does not.
            (23, true, stop),
            (25, false, None),
            // A binding Run inside a kept run is not in the branch any more.
            (27, false, None),
        ]
    );
}

#[test]
fn face_nodes_follow_driven_values_and_stops() {
    use tessera_formats::script_table::resolve::LookEffect;
    let m = face_world();
    let mut r = ClipResolver::new(&schema, face_vocab(), fetch(&m));
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let mut state = PropertyState::new();
    state.set(PID_LOOK_OPTION, PropertyValue::Enum(1));
    state.set(PID_LOOK_SKIN, PropertyValue::Colour([10, 20, 30, 255]));
    state.set(PID_LOOK_TEXTURE, PropertyValue::Int(3));
    let res = r.resolve_with_state(&e, state).unwrap();
    assert_eq!(res.look.len(), 6, "{:?}", res.look);
    // The older face started first and was stopped by the switch.
    assert!(
        res.look[..2]
            .iter()
            .all(|l| l.stopped && l.scripts == [K_OLD_FACE])
    );
    let active: Vec<_> = res.look.iter().filter(|l| l.active()).collect();
    assert_eq!(active.len(), 4);
    match &active[0].effect {
        LookEffect::AttachCompositor(a) => assert_eq!(a.file, Did(0x3300_0001)),
        other => panic!("{other:?}"),
    }
    match &active[1].effect {
        LookEffect::CompositorLayer(l) => assert_eq!(l.image_number, 3),
        other => panic!("{other:?}"),
    }
    assert!(active[1].driven);
    match &active[2].effect {
        LookEffect::CompositorColour(c) => assert_eq!(c.color, [10, 20, 30, 255]),
        other => panic!("{other:?}"),
    }
    match &active[3].effect {
        LookEffect::EditMaterial(ed) => {
            assert_eq!(ed.color_value, [10, 20, 30, 255]);
            assert_eq!(ed.priority, 5);
        }
        other => panic!("{other:?}"),
    }
    // With the option off, only the older face runs and nothing stops it.
    let mut state = PropertyState::new();
    state.set(PID_LOOK_OPTION, PropertyValue::Enum(0));
    let res = r.resolve_with_state(&e, state).unwrap();
    assert_eq!(res.look.len(), 2);
    assert!(res.look.iter().all(|l| l.active()));
    // An absent source keeps the stored values.
    match &res.look[1].effect {
        LookEffect::CompositorColour(c) => assert_eq!(c.color, [0xFF; 4]),
        other => panic!("{other:?}"),
    }
}

// --- random dye chains (appearance.md §7.4 "Random dye colours"; script-runtime.md §5.2,
// §15, §17) ---

/// The figure property a dye chain's SetProperty nodes set (a colour).
const PID_DYE_VALUE: u32 = 0x1000_0180;
/// A scriptlet parameter that chooses the mesh type, passed down by scriptlet-driven pairs.
const PID_PARAM_MESH: u32 = 0x1000_0181;
/// `EditMaterialScript_MeshType`.
const PID_EDIT_MESH: u32 = 0x1000_0182;
/// A property no Switch case matches.
const PID_NEVER: u32 = 0x1000_0183;
const DYE_OUTER: u32 = 0x0C00_0050;
const DYE_INNER: u32 = 0x0C00_0051;
/// The stored placeholder colour of the edits, as stored (B, G, R, A): R 153, G 21, B 113.
const PLACEHOLDER: [u8; 4] = [113, 21, 153, 255];
/// The colours the chain's Random chooses from, as stored (B, G, R, A).
const DYE_COLOURS: [[u8; 4]; 4] = [
    [20, 60, 30, 255],
    [25, 70, 40, 255],
    [30, 80, 50, 255],
    [35, 90, 60, 255],
];

fn dye_schema(pid: u32) -> Option<PropertyType> {
    match pid {
        PID_DYE_VALUE => Some(PropertyType::Colour),
        PID_PARAM_MESH | PID_NEVER => Some(PropertyType::Enum),
        _ => schema(pid),
    }
}

fn dye_vocab() -> ScriptVocabulary {
    use tessera_formats::script_table::resolve::EditMaterialFieldPids;
    ScriptVocabulary {
        edit_material: EditMaterialFieldPids {
            color_value: Some(PID_EDIT_COLOR),
            mesh_type: Some(PID_EDIT_MESH),
            ..EditMaterialFieldPids::default()
        },
        ..vocab()
    }
}

/// A base block with driven and scriptlet-driven pairs.
fn base_pairs(id: u32, driven: &[(u32, u32)], from_params: &[(u32, u32)]) -> B {
    let mut b = B::new().tsize(1, driven.len() as u32);
    for &(f, s) in driven {
        b = b.u32(f).u32(s);
    }
    b = b.tsize(1, from_params.len() as u32);
    for &(f, p) in from_params {
        b = b.u32(f).u32(p);
    }
    b.u8(0).u8(1).f32(1.0).f32(0.0).f32(1.0).u32(id)
}

/// An EditMaterial node of material parameter `property` with the placeholder colour,
/// whose fields come from scriptlet parameters by `from_params`.
fn dye_edit(id: u32, property: u32, from_params: &[(u32, u32)]) -> Vec<u8> {
    node(
        60,
        B::new()
            .u32(property)
            .u32(2)
            .u32(0x1000_0073)
            .bytes(&20i32.to_le_bytes())
            .u32(1)
            .f32(0.0)
            .bytes(&PLACEHOLDER)
            .u32(0)
            .u8(0),
        base_pairs(id, &[], from_params),
        &[],
    )
}

/// A Scriptlet node with pairs (no bound ports).
fn scriptlet_pairs(
    id: u32,
    file: u32,
    params: B,
    driven: &[(u32, u32)],
    from_params: &[(u32, u32)],
) -> Vec<u8> {
    node(
        19,
        B::new().u32(file).bytes(&params.build()),
        base_pairs(id, driven, from_params),
        &[],
    )
}

/// The chain of `appearance.md` §7.4: a Sequence whose step 1 is a Random over SetProperty
/// nodes of the dye value and whose step 2 is a Scriptlet node holding the dye value
/// (stored white), driven by the figure's value, and passing the mesh parameter down.
fn dye_chain(id: u32, with_colours: bool) -> Vec<u8> {
    let setters: Vec<(u32, Vec<Vec<u8>>)> = DYE_COLOURS
        .iter()
        .enumerate()
        .map(|(i, c)| {
            (
                1,
                vec![set_property_node(
                    id + 10 + i as u32,
                    PID_DYE_VALUE,
                    B::new().bytes(c),
                )],
            )
        })
        .collect();
    // The node holds both parameters its pairs write (`appearance.md` §7.4.1 rule 2).
    let invoke = scriptlet_pairs(
        id + 2,
        DYE_INNER,
        pset(&[
            (PID_DYE_VALUE, B::new().bytes(&[0xFF; 4])),
            (PID_PARAM_MESH, B::new().u32(2)),
        ]),
        &[(PID_DYE_VALUE, PID_DYE_VALUE)],
        &[(PID_PARAM_MESH, PID_PARAM_MESH)],
    );
    let mut steps = Vec::new();
    if with_colours {
        steps.push(port(6, Some(B::new().u32(1)), &[random(id + 1, &setters)]));
    }
    steps.push(port(6, Some(B::new().u32(2)), &[invoke]));
    node(6, B::new().u8(0).u32(1), base(id), &steps)
}

fn dye_world(with_colours: bool) -> HashMap<Did, Vec<u8>> {
    let mut m = HashMap::new();
    // The invoked scriptlet: edits of 2003 and 2013, both coloured from the dye value
    // parameter, the first also taking its mesh type from the passed-down parameter.
    m.insert(
        Did(DYE_INNER),
        scriptlet(
            &[
                dye_edit(
                    1,
                    2003,
                    &[
                        (PID_EDIT_COLOR, PID_DYE_VALUE),
                        (PID_EDIT_MESH, PID_PARAM_MESH),
                    ],
                ),
                dye_edit(2, 2013, &[(PID_EDIT_COLOR, PID_DYE_VALUE)]),
                // An edit fed by no pair keeps its stored colour.
                dye_edit(3, 2000, &[]),
            ],
            &[],
        ),
    );
    m.insert(
        Did(DYE_OUTER),
        scriptlet(&[dye_chain(100, with_colours)], &[]),
    );
    m.insert(
        Did(CLASS),
        table(
            2,
            &[],
            &[(
                STARTUP,
                vec![scriptlet_pairs(
                    1,
                    DYE_OUTER,
                    pset(&[(PID_PARAM_MESH, B::new().u32(7))]),
                    &[],
                    &[],
                )],
            )],
        ),
    );
    m
}

/// The active EditMaterial candidates: (parameter, mesh type, colour).
fn dye_edits(seed: Option<u64>, with_colours: bool) -> Vec<(u32, u32, [u8; 4])> {
    use tessera_formats::script_table::resolve::LookEffect;
    let m = dye_world(with_colours);
    let mut r = ClipResolver::new(&dye_schema, dye_vocab(), fetch(&m)).with_random_seed(seed);
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let res = r.resolve_with_state(&e, PropertyState::new()).unwrap();
    assert!(res.issues.is_empty(), "{:?}", res.issues);
    res.look
        .iter()
        .filter(|l| l.active())
        .map(|l| match &l.effect {
            LookEffect::EditMaterial(e) => (e.material_property, e.mesh_type, e.color_value),
            other => panic!("{other:?}"),
        })
        .collect()
}

/// `appearance.md` §7.4 (OBS10, A58): the Scriptlet node's driven pair fills its dye
/// parameter with the colour the Random chose, and every edit fed by that parameter takes
/// it, whichever material parameter it edits; the placeholder is never left on them.
#[test]
fn random_dye_chain_replaces_the_placeholder() {
    let mut seen = std::collections::HashSet::new();
    for seed in 0..32u64 {
        let edits = dye_edits(Some(seed), true);
        assert_eq!(edits.len(), 3, "{edits:?}");
        let (p0, mesh0, c0) = edits[0];
        let (p1, mesh1, c1) = edits[1];
        assert_eq!((p0, p1), (2003, 2013), "the parameter is each node's own");
        assert_eq!(mesh0, 7, "the outer parameter is passed down");
        assert_eq!(mesh1, 2, "no pair: the stored mesh type");
        assert!(DYE_COLOURS.contains(&c0), "seed {seed}: {c0:?}");
        assert_eq!(c0, c1, "one chosen colour for both edits");
        assert_eq!(
            edits[2],
            (2000, 2, PLACEHOLDER),
            "an unfed edit keeps its value"
        );
        seen.insert(c0);
    }
    // Only the picked SetProperty sets the value: the seeds reach several colours.
    assert!(seen.len() >= 3, "{seen:?}");
}

/// Without a SetProperty of the chain the parameter keeps its stored white
/// (`appearance.md` §7.4).
#[test]
fn a_dye_chain_without_a_colour_sets_white() {
    let edits = dye_edits(Some(1), false);
    assert_eq!(edits[0].2, [0xFF; 4]);
    assert_eq!(edits[1].2, [0xFF; 4]);
}

fn label_node(id: u32, name: &str, children: &[Vec<u8>]) -> Vec<u8> {
    node(
        25,
        B::new().pascal(name),
        base(id),
        &[port(6, Some(B::new().u32(1)), children)],
    )
}

fn goto_node(id: u32, name: &str) -> Vec<u8> {
    node(61, B::new().pascal(name), base(id), &[])
}

/// `script-runtime.md` §15: a Goto starts the Label of its name in its own definition,
/// wherever that Label stands (here under a case that never runs); a Goto whose Label is
/// already running through it starts nothing more; an unknown name starts nothing.
#[test]
fn goto_starts_its_label_once() {
    use tessera_formats::script_table::resolve::LookEffect;
    let hidden = switch(
        3,
        PID_NEVER,
        &[switch_case(
            &[(PID_NEVER, 99)],
            &[label_node(
                4,
                "chain",
                &[goto_node(5, "chain"), dye_edit(6, 2013, &[])],
            )],
        )],
    );
    let mut m = HashMap::new();
    m.insert(
        Did(CLASS),
        table(
            2,
            &[],
            &[(
                STARTUP,
                vec![goto_node(1, "chain"), goto_node(2, "missing"), hidden],
            )],
        ),
    );
    let mut r = ClipResolver::new(&dye_schema, dye_vocab(), fetch(&m)).with_random_seed(Some(3));
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let res = r.resolve_with_state(&e, PropertyState::new()).unwrap();
    let edits: Vec<u32> = res
        .look
        .iter()
        .map(|l| match &l.effect {
            LookEffect::EditMaterial(e) => e.material_property,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(edits, [2013]);
    assert_eq!(res.look[0].node_id, 6);
}

// --- Parameters reach fields only through pairs (`appearance.md` §7.4.1) ---

const PAIRS_INNER: u32 = 0x0C00_0901;
const PAIRS_DEEP: u32 = 0x0C00_0902;
const PAIRS_DEEP2: u32 = 0x0C00_0903;
const PAIRS_CHOSEN: u32 = 0x0C00_0904;
/// A float scriptlet parameter of another name than the field it feeds.
const PID_P_SPEED: u32 = 0x1000_0190;
/// `ScriptletScript_File`.
const PID_SCRIPTLET_FILE: u32 = 0x1000_0191;
/// An entity property holding a scriptlet DID.
const PID_FILE_SOURCE: u32 = 0x1000_0192;
/// A property no Scriptlet node has a field or parameter for.
const PID_UNHELD: u32 = 0x1000_0193;

fn pairs_schema(pid: u32) -> Option<PropertyType> {
    match pid {
        PID_P_SPEED | PID_UNHELD => Some(PropertyType::Float),
        PID_SCRIPTLET_FILE | PID_FILE_SOURCE => Some(PropertyType::DidRef),
        PID_FX_SCALE => Some(PropertyType::Vector3),
        _ => schema(pid),
    }
}

fn pairs_vocab() -> ScriptVocabulary {
    use tessera_formats::script_table::resolve::{MeshFxFieldPids, ScriptletFieldPids};
    ScriptVocabulary {
        impulse: ImpulseFieldPids {
            air_time: Some(PID_AIR_TIME),
            ..ImpulseFieldPids::default()
        },
        mesh_fx: MeshFxFieldPids {
            scale: Some(PID_FX_SCALE),
            ..MeshFxFieldPids::default()
        },
        scriptlet: ScriptletFieldPids {
            file: Some(PID_SCRIPTLET_FILE),
            ..ScriptletFieldPids::default()
        },
        ..vocab()
    }
}

fn anim_pairs(
    id: u32,
    clip: u32,
    layer: u32,
    driven: &[(u32, u32)],
    from_params: &[(u32, u32)],
) -> Vec<u8> {
    node(
        36,
        anim_data(clip, layer, 9.0),
        base_pairs(id, driven, from_params),
        &[],
    )
}

fn pairs_world() -> HashMap<Did, Vec<u8>> {
    let disp = |y: f32| B::new().f32(0.0).f32(y).f32(0.0);
    let mut m = HashMap::new();
    let impulse = node(
        9,
        B::new()
            .u8(1)
            .u32(2)
            .u8(0)
            .f32(0.0)
            .f32(0.0)
            .f32(9.0)
            .f32(0.2)
            .f32(15.0),
        base_pairs(3, &[], &[(PID_AIR_TIME, PID_AIR_TIME)]),
        &[],
    );
    let fx = node(
        41,
        mesh_fx_data(0, 0, [0.0; 3], 1.0, false),
        base_pairs(4, &[], &[(PID_FX_SCALE, PID_FX_SCALE)]),
        &[],
    );
    m.insert(
        Did(PAIRS_INNER),
        scriptlet(
            &[
                // Paired: displacement from the parameter; speed driven (2.0), then from a
                // parameter of another name (3.0), the later write winning.
                anim_pairs(
                    1,
                    CLIP_WALK,
                    20,
                    &[(PID_ANIM_SPEED, PID_SPEEDMULT)],
                    &[
                        (PID_ANIM_DISP, PID_ANIM_DISP),
                        (PID_ANIM_SPEED, PID_P_SPEED),
                    ],
                ),
                // Unpaired: keeps its stored displacement although the invocation holds one.
                anim_pairs(2, CLIP_RUN, 21, &[], &[]),
                impulse,
                fx,
                // An invocation that holds no displacement: the inner pair finds nothing,
                // whatever this record's invocation holds.
                scriptlet_pairs(5, PAIRS_DEEP, pset(&[]), &[], &[]),
                // An invocation that passes the displacement down with a pair of its own; its
                // driven pair names a PID its set does not hold, so it writes nothing.
                scriptlet_pairs(
                    6,
                    PAIRS_DEEP2,
                    pset(&[(PID_ANIM_DISP, disp(1.5))]),
                    &[(PID_P_SPEED, PID_SPEEDMULT), (PID_UNHELD, PID_SPEEDMULT)],
                    &[(PID_ANIM_DISP, PID_ANIM_DISP)],
                ),
            ],
            &[],
        ),
    );
    m.insert(
        Did(PAIRS_DEEP),
        scriptlet(
            &[anim_pairs(
                1,
                CLIP_IDLE,
                22,
                &[],
                &[(PID_ANIM_DISP, PID_ANIM_DISP)],
            )],
            &[],
        ),
    );
    m.insert(
        Did(PAIRS_DEEP2),
        scriptlet(
            &[anim_pairs(
                1,
                CLIP_EMOTE,
                23,
                &[],
                &[
                    (PID_ANIM_DISP, PID_ANIM_DISP),
                    (PID_ANIM_SPEED, PID_P_SPEED),
                ],
            )],
            &[],
        ),
    );
    m.insert(
        Did(PAIRS_CHOSEN),
        scriptlet(&[anim_pairs(1, CLIP_BACK, 24, &[], &[])], &[]),
    );
    m.insert(
        Did(CLASS),
        table(
            2,
            &[],
            &[(
                STARTUP,
                vec![
                    scriptlet_pairs(
                        1,
                        PAIRS_INNER,
                        pset(&[
                            (PID_ANIM_DISP, disp(6.5)),
                            (PID_P_SPEED, B::new().f32(3.0)),
                            (PID_AIR_TIME, B::new().f32(0.4)),
                            (PID_FX_SCALE, B::new().f32(2.0).f32(2.0).f32(2.0)),
                        ]),
                        &[],
                        &[],
                    ),
                    // A placeholder whose driven `ScriptletScript_File` pair chooses the
                    // record: its set does not hold that PID, so the pair goes to the field.
                    scriptlet_pairs(
                        2,
                        0,
                        pset(&[]),
                        &[(PID_SCRIPTLET_FILE, PID_FILE_SOURCE)],
                        &[],
                    ),
                ],
            )],
        ),
    );
    m
}

/// `appearance.md` §7.4.1 rules 1–4: driven pairs, then scriptlet-driven pairs, for
/// animation, impulse and MeshFX nodes alike; a scriptlet-driven pair reads only the
/// invoking Scriptlet node's own set; a Scriptlet node's pair writes a parameter only when
/// its set holds the PID, else its own field; nothing reaches a field without a pair.
#[test]
fn parameters_reach_fields_only_through_pairs() {
    let m = pairs_world();
    let mut r = ClipResolver::new(&pairs_schema, pairs_vocab(), fetch(&m));
    let e = EntityScripts {
        class_table: Some(Did(CLASS)),
        ..EntityScripts::default()
    };
    let mut state = PropertyState::new();
    state.set(PID_SPEEDMULT, PropertyValue::Float(2.0));
    state.set(PID_FILE_SOURCE, PropertyValue::DidRef(Did(PAIRS_CHOSEN)));
    let res = r.resolve_with_state(&e, state).unwrap();
    assert!(res.issues.is_empty(), "{:?}", res.issues);
    let clip = |layer: u32| {
        let c = res
            .primary(layer)
            .unwrap_or_else(|| panic!("layer {layer}"));
        (c.clip.0, c.displacement[1], c.speed)
    };
    assert_eq!(
        clip(20),
        (CLIP_WALK, 6.5, 3.0),
        "paired; the later pair wins"
    );
    assert_eq!(clip(21), (CLIP_RUN, 9.0, 1.0), "no pair: stored values");
    assert_eq!(
        clip(22),
        (CLIP_IDLE, 9.0, 1.0),
        "the outer invocation is not read"
    );
    assert_eq!(
        clip(23),
        (CLIP_EMOTE, 6.5, 1.0),
        "passed down by a pair; a pair of an unheld PID writes no parameter"
    );
    assert_eq!(clip(24).0, CLIP_BACK, "the file pair chose the record");
    assert_eq!(res.impulses.len(), 1);
    assert_eq!(res.impulses[0].node.air_time, 0.4);
    assert_eq!(res.mesh_fx.len(), 1);
    assert_eq!(res.mesh_fx[0].node.scale, [2.0; 3]);
    assert!(res.mesh_fx[0].driven.scale);
    // Without the driven source the placeholder invokes nothing, and the paired speed
    // still takes the parameter.
    let res = r.resolve_with_state(&e, PropertyState::new()).unwrap();
    assert!(res.primary(24).is_none());
    assert_eq!(res.primary(20).unwrap().speed, 3.0);
}
