// SPDX-License-Identifier: GPL-3.0-or-later
//! Script-table (0x07) and scriptlet (0x0C) decoding (`docs/specs/script-table.md` §2–§5,
//! §12) on arbitrary bytes, with a total synthetic schema (every PID typed from its value,
//! so PVs and property sets inside nodes reach every value decoder) and with no schema.
//!
//! Nothing may panic and nesting stays within the limits of §4.5. An accepted record must
//! agree with its own census: node counts from walking equal the census, every port of an
//! animation node with an F32 payload is an event time, the typed Light and MeshFX nodes
//! sit on nodes of their kinds, and invariant checks run.
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_script_table -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::script_table::{MAX_NODE_DEPTH, Node, NodeCensus, NodeData, NodeKind};
use tessera_formats::{
    AnyRecord, NoSchema, PropertyType, ScriptTable, Scriptlet, decode_script_table,
    decode_scriptlet,
};

/// Every PID has a type: codes 0 and 23 map to `Unknown`, the rest to types 1-22.
fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

fn check_roots(roots: &[Node], census: &NodeCensus) {
    let mut nodes = 0u64;
    let mut deepest = 0usize;
    for r in roots {
        r.walk(&mut |n, depth| {
            nodes += 1;
            deepest = deepest.max(depth + 1);
            let _ = n.kind.name();
            let _ = n.event_times().count();
            if let Some(a) = n.animation() {
                let _ = a.displacement_length();
            }
            // The typed Light and MeshFX nodes (§5.2, §5.3) keep what was stored.
            match &n.data {
                NodeData::Light(l) => {
                    assert!(l.diffuse_intensity.values.len() <= 10);
                    assert_eq!(n.kind, NodeKind::LIGHT);
                }
                NodeData::MeshFx(m) => {
                    assert_eq!(n.kind, NodeKind::MESH_FX);
                    let _ = m.appearances.len();
                }
                _ => {}
            }
        });
    }
    assert_eq!(nodes, census.nodes);
    assert_eq!(deepest, census.max_depth);
    assert!(deepest <= MAX_NODE_DEPTH);
    let by_kind: u64 = census.by_kind.iter().map(|(_, n)| n).sum();
    assert_eq!(by_kind, census.nodes);
}

fn check_table(t: &ScriptTable) {
    let c = t.census();
    let roots: Vec<Node> = t.scripts.iter().flat_map(|s| s.roots.clone()).collect();
    check_roots(&roots, &c);
    let _ = t.first_unusual_field();
    for s in &t.scripts {
        assert!(t.script(s.key).is_some());
    }
}

fn check_scriptlet(s: &Scriptlet) {
    check_roots(&s.roots, &s.census());
    let _ = s.first_unusual_field();
}

fuzz_target!(|data: &[u8]| {
    if let Ok(t) = decode_script_table(data, &total) {
        check_table(&t);
    }
    if let Ok(t) = decode_script_table(data, &NoSchema) {
        check_table(&t);
    }
    if let Ok((t, left)) = ScriptTable::parse_prefix(data, &total) {
        assert!(left <= data.len());
        check_table(&t);
    }
    if let Ok(s) = decode_scriptlet(data, &total) {
        check_scriptlet(&s);
    }
    if let Ok((s, left)) = Scriptlet::parse_prefix(data, &NoSchema) {
        assert!(left <= data.len());
        check_scriptlet(&s);
    }
    if let Ok(Some(r)) = AnyRecord::decode_with_schema(0x0C00_0001u32, data, &total) {
        assert_eq!(r.did().0, 0x0C00_0001);
    }
});
