// SPDX-License-Identifier: GPL-3.0-or-later
//! Cell geometry, prefix 0x01 (`docs/specs/collision.md` §5), on arbitrary bytes. The first
//! byte selects the directory stamp (0xFF = none, so the stamp-less `build_byte` fallback
//! runs). The second selects how much valid frame comes before the input: a cell DID only,
//! or the DID, `has_physics_mesh` = 0 and a plane list of 1–8 planes, so that short inputs
//! reach the portals, walls, hull, property set, mesh count and flag and the render mesh.
//! Property sets are decoded with a small schema of float, vector and struct PIDs.
//!
//! Nothing may panic. An accepted record has at least one plane, its render mesh body is at
//! least 4 bytes, the rebuilt mesh record starts with the DID, and the physics mesh tree
//! walks within its budget.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::PropertyType;
use tessera_formats::collision::{CellGeometry, PLANE_TAG_BPFL, PLANE_TAG_LEAF, PLANE_TAG_NEXT};

const DID: u32 = 0x0100_0123;

fn schema(pid: u32) -> Option<PropertyType> {
    // Float, Vector3 and Struct values; other PIDs are unknown.
    match pid % 4 {
        0 => Some(PropertyType::from_code(9)),
        1 => Some(PropertyType::from_code(6)),
        2 => Some(PropertyType::from_code(11)),
        _ => None,
    }
}

fn payload(mode: u8, rest: &[u8]) -> Vec<u8> {
    let mut out = DID.to_le_bytes().to_vec();
    if mode.is_multiple_of(2) {
        out.extend_from_slice(rest);
        return out;
    }
    out.push(0);
    let planes = usize::from(mode / 2 % 8) + 1;
    for k in 0..planes {
        let tag = match (k + 1 == planes, mode & 0x10 != 0) {
            (false, _) => PLANE_TAG_NEXT,
            (true, false) => PLANE_TAG_BPFL,
            (true, true) => PLANE_TAG_LEAF,
        };
        out.extend_from_slice(&tag.to_le_bytes());
        for v in [0.0f32, 0.0, 1.0, k as f32] {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out.extend_from_slice(rest);
    out
}

fuzz_target!(|input: &[u8]| {
    let [s, mode, rest @ ..] = input else {
        return;
    };
    let stamp = (*s != 0xFF).then_some(u16::from(*s));
    let data = payload(*mode, rest);
    let Ok(c) = CellGeometry::parse(&data, stamp, &schema) else {
        return;
    };
    assert!(!c.planes.is_empty());
    assert_ne!(c.plane_list_end.tag(), PLANE_TAG_NEXT);
    assert!(c.render_mesh.len() >= 4);
    let rec = c.render_mesh_record();
    assert_eq!(&rec[..4], &c.did.0.to_le_bytes());
    assert_eq!(rec.len(), c.render_mesh.len() + 4);
    for p in &c.planes {
        let _ = p.signed_distance([1.0, 2.0, 3.0]);
    }
    if let Some(m) = &c.physics_mesh {
        if let Ok(w) = m.tree_code().walk() {
            let _ = w.keys_below(m.triangles.len() as u64);
        }
        for t in 0..m.triangles.len() {
            assert!(m.triangle_positions(t).is_some());
        }
    }
    let _ = CellGeometry::parse(&data, None, &schema);
});
