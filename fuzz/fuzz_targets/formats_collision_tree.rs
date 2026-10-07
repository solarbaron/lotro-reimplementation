// SPDX-License-Identifier: GPL-3.0-or-later
//! The quantised tree reader (`docs/specs/collision.md` §3) on arbitrary code. The first
//! 8 bytes choose a query box (two corners of three bytes each, read as byte values 0–255
//! of the integer coordinates), a scale (2^0–2^15, times 256 when bit 4 is set) and the
//! origin (0 or −1 per axis); the rest is the code.
//!
//! Nothing may panic, and walks stay within their budgets. When the walks succeed: every key
//! a point query reports is one the structural walk found; a box query around a point
//! reports at least the point's keys; and every offset decodes without panicking.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::collision::{Instruction, TreeCode};

fuzz_target!(|input: &[u8]| {
    let Some((head, code)) = input.split_first_chunk::<8>() else {
        return;
    };
    let [ax, ay, az, bx, by, bz, exp, mode] = *head;
    let scale = f32::from(1u16 << (exp % 16)) * if exp & 0x10 != 0 { 256.0 } else { 1.0 };
    let origin = if mode & 1 != 0 { [-1.0; 3] } else { [0.0; 3] };
    let t = TreeCode::new(origin, scale, code);
    let unit = 65_536.0 / scale;
    let to_point = |x: u8, y: u8, z: u8| [x, y, z].map(|v| f32::from(v) * unit);
    let p = to_point(ax, ay, az);
    let q = to_point(bx, by, bz);
    let (lo, hi) = (
        [p[0].min(q[0]), p[1].min(q[1]), p[2].min(q[2])],
        [p[0].max(q[0]), p[1].max(q[1]), p[2].max(q[2])],
    );

    for pc in 0..code.len().min(256) {
        if let Ok((ins, len)) = Instruction::decode(code, pc) {
            assert!(len >= 1 && pc + len <= code.len());
            let _ = ins.ends_branch();
        }
    }

    let walk = t.walk();
    let point = t.point_keys(p);
    let boxed = t.box_keys(lo, hi);
    if let (Ok(w), Ok(keys)) = (&walk, &point) {
        assert!(keys.iter().all(|k| w.contains(*k)));
        assert!(w.keys.windows(2).all(|k| k[0] < k[1]));
        assert!(w.counts.total() <= code.len());
    }
    if let (Ok(keys), Ok(b)) = (&point, &boxed) {
        assert!(keys.iter().all(|k| b.binary_search(k).is_ok()));
    }
    if let Ok(b) = &boxed
        && let Ok(w) = &walk
    {
        assert!(b.iter().all(|k| w.contains(*k)));
    }
    // A point query on the quantised point matches the float query.
    if let (Some(qp), Ok(keys)) = (t.quantise(p), &point) {
        let mut raw = Vec::new();
        if let Ok(steps) = t.query_quantised(qp, qp, |k| raw.push(k)) {
            assert!(steps <= t.step_budget() && raw.len() <= steps);
            raw.sort_unstable();
            raw.dedup();
            assert_eq!(&raw, keys);
        }
    }
    // Non-finite points and boxes report nothing, without walking.
    assert_eq!(t.point_keys([f32::NAN, 0.0, 0.0]), Ok(Vec::new()));
    assert_eq!(
        t.point_keys([f32::INFINITY, f32::NEG_INFINITY, 1e30]),
        Ok(Vec::new())
    );
    assert_eq!(
        t.box_keys([f32::NEG_INFINITY; 3], [f32::INFINITY; 3]),
        Ok(Vec::new())
    );
    assert!(t.step_budget() <= tessera_formats::collision::MAX_QUERY_STEPS);
    assert!(t.walk_state_budget() <= tessera_formats::collision::MAX_WALK_STATES);
});
