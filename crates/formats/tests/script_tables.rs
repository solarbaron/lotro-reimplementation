// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests for script tables (0x07), scriptlets (0x0C) and clip selection
//! (`docs/specs/script-table.md`). Every buffer is built here; values are invented.

mod common;

use std::collections::HashMap;
use std::convert::Infallible;

use common::B;
use tessera_formats::script_table::resolve::{
    AnimationFieldPids, ClipResolver, EntityScripts, MotionInputs, MotionState, MovementStyle,
    PropertyState, ResolveIssue, ResolveLimits, ScriptVocabulary,
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
    // The run binding overrides the displacement through the Scriptlet node's parameters.
    let run_clip = scriptlet(&[anim(1, CLIP_RUN, 20, 9.0, speed)], &[CLIP_RUN]);
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

fn shape_schema(pid: u32) -> Option<PropertyType> {
    match pid {
        PID_MANUAL_TIME | PID_HEIGHT | PID_BONE_SCALE | PID_DEFORM_TIME => {
            Some(PropertyType::Float)
        }
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

/// `script-table.md` §6.7 worked example: clip time = fraction × 2.0 s.
#[test]
fn held_fraction_example_6_7() {
    use tessera_formats::script_table::resolve::held_fraction;
    let rows: [(f32, f32, f32, f32); 5] = [
        (0.25, 0.5, 0.5, 0.5),
        (0.95, 1.9, 1.9, 1.9),
        (1.0, 0.0, 2.0, 0.0),
        (1.3, 0.6, 2.0, 0.6),
        (-0.2, 1.6, 0.0, 0.0),
    ];
    for (v, looping, once, twice) in rows {
        let t = |lf: bool, count: i32| held_fraction(v, lf, count) * 2.0;
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
    assert_eq!(held_fraction(f32::NAN, true, 1), 0.0);
    assert_eq!(held_fraction(f32::INFINITY, false, 1), 0.0);
    assert!(held_fraction(-1e-9, true, 1) < 1.0);
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
