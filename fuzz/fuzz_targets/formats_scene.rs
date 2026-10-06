// SPDX-License-Identifier: GPL-3.0-or-later
//! Scene records (prefix 0x02, `docs/specs/scenery.md` §5) on arbitrary bytes. The first
//! byte selects whether the rest is the payload as is, or follows a valid prologue
//! (reserved word, own DID, `flag_a` and `has_frill_layer` from bit 1) so that short inputs
//! reach the placement nodes and their children.
//!
//! Nothing may panic, and node nesting is capped (`scenery.md` §13.1). An accepted scene
//! re-encodes canonically (a fixpoint, never longer than the input), its traversals agree
//! with each other, and the scene-reference checker terminates on the graph the scene forms
//! with itself (every reference resolving to the scene).
//!
//! Run with a memory cap so that allocation blow-ups count as failures, e.g.
//! `cargo fuzz run formats_scene -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_formats::scenery::{
    MAX_CHILD_DEPTH, MAX_PLACEMENT_NODE_DEPTH, Scene, SceneChild, SceneReferenceChecker, SceneSite,
    decode_scene,
};
use tessera_formats::{Did, Record};

fn payload(mode: u8, rest: &[u8]) -> Vec<u8> {
    if mode.is_multiple_of(2) {
        return rest.to_vec();
    }
    let mut out = Vec::with_capacity(10 + rest.len());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0x0200_0001u32.to_le_bytes());
    out.push(1);
    out.push((mode >> 1) & 1);
    out.extend_from_slice(rest);
    out
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let data = &payload(*mode, rest)[..];
    let Ok(s) = decode_scene(data) else {
        return;
    };
    let bytes = s.to_bytes();
    assert!(bytes.len() <= data.len());
    let again = Scene::parse(&bytes).expect("re-encoded scene decodes");
    // Node floats are finite (z_range may be infinite, never NaN), so values compare.
    assert_eq!(again, s);
    assert_eq!(again.to_bytes(), bytes);

    let mut objects = 0usize;
    let mut refs = 0usize;
    let mut nodes = 0usize;
    for c in s.descendants() {
        // Nodes sit at their nesting level (at most the cap); objects and references one
        // below their container. The frill layer root is level 1, so its children are 2.
        let min = if c.site == SceneSite::FrillLayer {
            2
        } else {
            1
        };
        let max = match c.child {
            SceneChild::Node(_) => MAX_PLACEMENT_NODE_DEPTH,
            _ => MAX_CHILD_DEPTH,
        };
        assert!(c.depth >= min && c.depth <= max, "depth {}", c.depth);
        match c.child {
            SceneChild::Object(_) => objects += 1,
            SceneChild::Scene(_) => refs += 1,
            SceneChild::Node(n) => {
                nodes += 1;
                assert!(n.frequency.is_finite() && !n.z_range.min.is_nan());
                let _ = n.height_mode_kind();
                let _ = n.below_ground();
            }
        }
    }
    assert_eq!(s.objects().count(), objects);
    assert_eq!(s.scene_references().count(), refs);
    assert_eq!(
        s.nodes().count(),
        nodes + usize::from(s.frill_layer.is_some())
    );
    assert!(
        s.nodes()
            .all(|(_, d, _)| (1..=MAX_PLACEMENT_NODE_DEPTH).contains(&d))
    );
    let _ = s.placement_group();
    let _ = s.is_empty();

    // Every reference resolves to this scene: any reference is then a cycle.
    let mut checker = SceneReferenceChecker::new();
    let h = checker.height(s.did, |_: Did| Some(&s));
    assert_eq!(h.is_ok(), refs == 0);
});
