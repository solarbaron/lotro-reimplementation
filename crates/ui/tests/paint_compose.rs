// SPDX-License-Identifier: GPL-3.0-or-later
//! The composition list (decision 0015 Decision 1.6, 1.7; `render.md` §6.5–§6.8, §6.14):
//! entry order with canvases above, transform chains, the clip chain including mode-0 child
//! windows (M5-1), opacity rules, caps, and the coverage regions of rotated entries (M5-2,
//! P1 check 4). Synthetic trees only.

use std::collections::{HashMap, HashSet};

use tessera_ui::paint::{
    Addressing, CompNode, ComposeConfig, CompositionList, EntryTexture, FootprintNode, Layer,
    NodeContent, OpacityRule, ParentCanvas, Rect, SurfaceId, TextureLookup, build,
    coverage_change_damage, coverage_region,
};

/// Every layer has a texture covering the canvas at offset `(0, 0)` unless listed.
#[derive(Default)]
struct Lookup {
    boxes: HashMap<(SurfaceId, Layer), Rect>,
    hidden: HashSet<SurfaceId>,
    all_above: bool,
}

impl TextureLookup for Lookup {
    fn front_box(&self, s: SurfaceId, layer: Layer) -> Option<Rect> {
        if let Some(b) = self.boxes.get(&(s, layer)) {
            return Some(*b);
        }
        match layer {
            Layer::Below => Some(Rect::new(0, 0, 1, 1)),
            Layer::Above(_) if self.all_above => Some(Rect::new(0, 0, 1, 1)),
            Layer::Above(_) => None,
        }
    }
    fn is_drawn(&self, s: SurfaceId) -> bool {
        !self.hidden.contains(&s)
    }
}

fn node(id: u32, parent: Option<usize>, b: Rect) -> CompNode {
    CompNode::new(SurfaceId(id), parent, b)
}

fn window(id: u32, parent: Option<usize>, b: Rect) -> CompNode {
    CompNode {
        window: true,
        ..node(id, parent, b)
    }
}

fn textures(l: &CompositionList) -> Vec<(u32, Layer)> {
    l.entries
        .iter()
        .map(|e| match e.texture {
            EntryTexture::Surface { surface, layer } => (surface.0, layer),
            EntryTexture::External(t) => (t as u32 + 1000, Layer::Below),
        })
        .collect()
}

fn clips(l: &CompositionList, i: usize) -> Vec<([i32; 4], u32)> {
    l.clips_of(&l.entries[i])
        .iter()
        .map(|c| (c.rect, c.space))
        .collect()
}

#[test]
fn order_with_canvases_above() {
    // R (root) holds N1 (which holds M) and N2.
    let nodes = [
        window(1, None, Rect::from_xywh(0, 0, 100, 100)),
        node(2, Some(0), Rect::from_xywh(10, 10, 20, 20)),
        node(3, Some(1), Rect::from_xywh(12, 12, 5, 5)),
        node(4, Some(0), Rect::from_xywh(50, 50, 20, 20)),
    ];
    let lookup = Lookup {
        all_above: true,
        ..Lookup::default()
    };
    let l = build(&nodes, &lookup, &ComposeConfig::default()).unwrap();
    assert_eq!(
        textures(&l),
        vec![
            (1, Layer::Below),
            (2, Layer::Below),
            (3, Layer::Below),
            (2, Layer::Above(3)),
            (1, Layer::Above(2)),
            (4, Layer::Below),
            (1, Layer::Above(4)),
        ]
    );
    // A canvas above has exactly its surface's placement.
    let (r, a) = (&l.entries[0], &l.entries[4]);
    assert_eq!((r.dest, r.canvas, r.opacity), (a.dest, a.canvas, a.opacity));
    assert!(l.clips_of(r).is_empty(), "a root entry has no clip");
}

#[test]
fn clip_chain_mode_0_child_window_m5_1() {
    // render.md §6.8 example: window P has a surface; child window C is in mode 0 (no
    // surface, drawn into P's canvas); control K in stretch mode inside C extends past C.
    let p = Rect::from_xywh(0, 0, 200, 200);
    let c = Rect::from_xywh(20, 20, 50, 50);
    let k = Rect::from_xywh(60, 60, 40, 40);
    let mut nodes = vec![
        window(1, None, p),
        CompNode {
            content: NodeContent::None,
            ..window(2, Some(0), c)
        },
        node(3, Some(1), k),
    ];
    let lookup = Lookup::default();
    let l = build(&nodes, &lookup, &ComposeConfig::default()).unwrap();
    assert_eq!(textures(&l), vec![(1, Layer::Below), (3, Layer::Below)]);
    // C's box and P's box share k = 0: merged into C ∩ P, so K is cut at C's edge.
    assert_eq!(clips(&l, 1), vec![([20, 20, 70, 70], 0)]);

    // With P rotated by z = 30, K's list is [P's rotation] and the item keeps k = 0.
    nodes[0].rotation = [0.0, 0.0, 30.0];
    let l = build(&nodes, &lookup, &ComposeConfig::default()).unwrap();
    let rots = l.rotations_of(&l.entries[1]);
    assert_eq!(rots.len(), 1);
    assert_eq!(rots[0].centre, [100.0, 100.0]);
    assert_eq!(clips(&l, 1), vec![([20, 20, 70, 70], 0)]);
}

#[test]
fn clip_chain_child_window_with_surface_and_rotation() {
    // Child window C with its own surface extends past P; control K inside C.
    let p = Rect::from_xywh(0, 0, 100, 100);
    let c = Rect::from_xywh(50, 50, 100, 100);
    let k = Rect::from_xywh(120, 60, 50, 20);
    let mut nodes = vec![
        window(1, None, p),
        window(2, Some(0), c),
        node(3, Some(1), k),
    ];
    let lookup = Lookup::default();
    let l = build(&nodes, &lookup, &ComposeConfig::default()).unwrap();
    // C is cut at P's edge; K by C and P, which share one space.
    assert_eq!(clips(&l, 1), vec![([0, 0, 100, 100], 0)]);
    assert_eq!(clips(&l, 2), vec![([50, 50, 100, 100], 0)]);
    // C rotated by z = 30: K turns with C; C's box is tested in C's rotated space (k = 0,
    // before C's rotation), P's after it (k = 1).
    nodes[1].rotation = [0.0, 0.0, 30.0];
    let l = build(&nodes, &lookup, &ComposeConfig::default()).unwrap();
    assert_eq!(l.rotations_of(&l.entries[2]).len(), 1);
    assert_eq!(
        clips(&l, 2),
        vec![([50, 50, 150, 150], 0), ([0, 0, 100, 100], 1)]
    );
    // C's own entry: its rotation is its own; P's box is tested after it (screen space).
    assert_eq!(clips(&l, 1), vec![([0, 0, 100, 100], 1)]);
}

#[test]
fn surfaces_of_other_elements_do_not_clip() {
    // K inside stretch-mode control S inside window P: only P clips K.
    let nodes = [
        window(1, None, Rect::from_xywh(0, 0, 100, 100)),
        node(2, Some(0), Rect::from_xywh(10, 10, 20, 20)),
        node(3, Some(1), Rect::from_xywh(25, 25, 40, 40)),
    ];
    let l = build(&nodes, &Lookup::default(), &ComposeConfig::default()).unwrap();
    assert_eq!(clips(&l, 2), vec![([0, 0, 100, 100], 0)]);
}

#[test]
fn hidden_tile_depth_and_entry_caps() {
    let b = Rect::from_xywh(0, 0, 64, 64);
    // Hidden ancestor: no entry for the subtree and no canvas above for it.
    let nodes = [
        window(1, None, b),
        CompNode {
            shown: false,
            content: NodeContent::None,
            ..node(2, Some(0), b)
        },
        node(3, Some(1), b),
    ];
    let lookup = Lookup {
        all_above: true,
        ..Lookup::default()
    };
    let l = build(&nodes, &lookup, &ComposeConfig::default()).unwrap();
    assert_eq!(textures(&l), vec![(1, Layer::Below)]);

    // Below a Tile surface: no entry, counted.
    let nodes = [
        CompNode {
            addressing: Addressing::Tile {
                node_scale: [1.0, 1.0],
            },
            ..window(1, None, b)
        },
        node(2, Some(0), b),
        CompNode {
            content: NodeContent::External(7),
            ..node(3, Some(0), b)
        },
    ];
    let l = build(&nodes, &lookup, &ComposeConfig::default()).unwrap();
    assert_eq!(textures(&l), vec![(1, Layer::Below)]);
    assert_eq!(l.counters.tile_parent, 2);

    // Ten nested windows: entries past the depth cap of 8 are dropped and counted, and no
    // clip chain is longer than 8.
    let mut nodes = vec![window(0, None, b)];
    for i in 1..=10u32 {
        let mut n = window(i, Some(i as usize - 1), b);
        n.rotation = [0.0, 0.0, 10.0 * i as f32];
        nodes.push(n);
    }
    let l = build(&nodes, &Lookup::default(), &ComposeConfig::default()).unwrap();
    assert_eq!(l.entries.len(), 9);
    assert_eq!(l.counters.depth, 2);
    assert!(l.entries.iter().all(|e| l.clips_of(e).len() <= 8));
    assert!(l.entries.iter().all(|e| l.rotations_of(e).len() <= 9));

    // Entry cap.
    let mut nodes = vec![window(0, None, b)];
    for i in 1..100u32 {
        nodes.push(node(i, Some(0), b));
    }
    let config = ComposeConfig {
        max_entries: 10,
        ..ComposeConfig::default()
    };
    let l = build(&nodes, &Lookup::default(), &config).unwrap();
    assert_eq!(l.entries.len(), 10);
    assert_eq!(l.counters.entries, 90);
}

#[test]
fn opacity_rules_and_stand_in() {
    let b = Rect::from_xywh(0, 0, 64, 64);
    let nodes = [
        CompNode {
            opacity: 0.5,
            ..window(1, None, b)
        },
        CompNode {
            opacity: 0.5,
            ..node(2, Some(0), b)
        },
    ];
    let own = build(&nodes, &Lookup::default(), &ComposeConfig::default()).unwrap();
    assert_eq!(own.entries[1].opacity, 0.5, "own only");
    let both = build(
        &nodes,
        &Lookup::default(),
        &ComposeConfig {
            opacity_rule: OpacityRule::OwnTimesAncestors,
            ..ComposeConfig::default()
        },
    )
    .unwrap();
    assert_eq!(both.entries[1].opacity, 0.25);
    let stand_in = [CompNode {
        opacity: 0.2,
        stand_in: true,
        ..window(1, None, b)
    }];
    let l = build(&stand_in, &Lookup::default(), &ComposeConfig::default()).unwrap();
    assert_eq!(l.entries[0].opacity, 1.0);
    let nan = [CompNode {
        opacity: f32::NAN,
        rotation: [f32::NAN, f32::INFINITY, 0.0],
        ..window(1, None, b)
    }];
    let l = build(&nan, &Lookup::default(), &ComposeConfig::default()).unwrap();
    assert_eq!(l.entries[0].opacity, 0.0);
    assert!(
        l.rotations_of(&l.entries[0]).is_empty(),
        "non-finite angles count as 0"
    );
}

#[test]
fn texture_offset_is_tile_box_plus_canvas_origin() {
    let mut lookup = Lookup::default();
    lookup
        .boxes
        .insert((SurfaceId(1), Layer::Below), Rect::new(256, 512, 512, 768));
    let nodes = [CompNode {
        canvas_origin: [1000, 0],
        canvas: [3840, 2000],
        ..window(1, None, Rect::from_xywh(-1000, 0, 20_000, 2000))
    }];
    let l = build(&nodes, &lookup, &ComposeConfig::default()).unwrap();
    assert_eq!(l.entries[0].tex_offset, [1256, 512]);
    assert_eq!(l.entries[0].canvas, [3840, 2000]);
    // A surface not drawn yet (first paint withheld) has no entry.
    lookup.hidden.insert(SurfaceId(1));
    let l = build(&nodes, &lookup, &ComposeConfig::default()).unwrap();
    assert!(l.entries.is_empty());
}

#[test]
fn malformed_trees_are_refused() {
    let b = Rect::from_xywh(0, 0, 8, 8);
    let lookup = Lookup::default();
    let c = ComposeConfig::default();
    assert!(build(&[node(1, Some(0), b)], &lookup, &c).is_err());
    // Subtree of 0 not contiguous: 2's parent is 0, but 1 (a root) closed it.
    let nodes = [node(1, None, b), node(2, None, b), node(3, Some(0), b)];
    assert!(build(&nodes, &lookup, &c).is_err());
}

#[test]
fn external_content_is_premultiplied() {
    let b = Rect::from_xywh(0, 0, 64, 64);
    let nodes = [
        window(1, None, b),
        CompNode {
            content: NodeContent::External(42),
            ..node(2, Some(0), Rect::from_xywh(4, 4, 32, 32))
        },
    ];
    let l = build(&nodes, &Lookup::default(), &ComposeConfig::default()).unwrap();
    assert_eq!(l.entries[1].texture, EntryTexture::External(42));
    assert_eq!(
        l.entries[1].source,
        tessera_ui::paint::Source::Premultiplied
    );
}

/// Rotates `p` about `c` by `deg` (any direction: the square must hold every angle).
fn rot(p: [f64; 2], c: [f64; 2], deg: f64) -> [f64; 2] {
    let (s, co) = deg.to_radians().sin_cos();
    let (x, y) = (p[0] - c[0], p[1] - c[1]);
    [c[0] + x * co - y * s, c[1] + x * s + y * co]
}

#[test]
fn marker_turning_is_no_damage_p1_check_4() {
    let parent = ParentCanvas {
        displayed: Rect::from_xywh(0, 0, 224, 280),
        canvas: [224, 280],
        canvas_origin: [0, 0],
        stored: [224, 280],
    };
    let mut marker = FootprintNode {
        displayed: Rect::from_xywh(98, 126, 28, 28),
        ..FootprintNode::default()
    };
    let unrotated = coverage_region(&marker, &parent);
    assert_eq!(unrotated, Some(Rect::from_xywh(98, 126, 28, 28)));
    // The first rotation the surface holds is one coverage change...
    marker.own_rotation = true;
    let square = coverage_region(&marker, &parent);
    assert_eq!(coverage_change_damage(unrotated, square).len(), 2);
    let sq = square.unwrap();
    assert_eq!((sq.width(), sq.height()), (40, 40));
    // ...and turning through 360° in 1° steps changes nothing after it.
    let c = [112.0, 140.0];
    for deg in 0..360 {
        let now = coverage_region(&marker, &parent);
        assert!(coverage_change_damage(square, now).is_empty());
        for corner in [[98.0, 126.0], [126.0, 126.0], [98.0, 154.0], [126.0, 154.0]] {
            let p = rot(corner, c, f64::from(deg));
            assert!(f64::from(sq.x0) <= p[0] && p[0] <= f64::from(sq.x1));
            assert!(f64::from(sq.y0) <= p[1] && p[1] <= f64::from(sq.y1));
        }
    }
}

#[test]
fn rotated_non_window_parent_covers_its_turning_child_m5_2() {
    // A stretch-mode child M extends past a rotated stretch-mode parent N (not a window),
    // which is a nested entry of window P. M turns with N about N's centre and is not
    // clipped by N; N's coverage region in P's canvas must hold M at every angle (option
    // (a)), so parent content drawn after N always lands in N's canvas above.
    let n_box = Rect::from_xywh(100, 100, 40, 20);
    let m_box = Rect::from_xywh(130, 90, 60, 15);
    let n = FootprintNode {
        displayed: n_box,
        own_rotation: true,
        window: false,
        nested: vec![FootprintNode {
            displayed: m_box,
            ..FootprintNode::default()
        }],
    };
    // P scaled by 1.5 with a cropped canvas origin, to exercise step 3.
    let parent = ParentCanvas {
        displayed: Rect::from_xywh(0, 0, 450, 450),
        canvas: [300, 300],
        canvas_origin: [0, 0],
        stored: [300, 300],
    };
    let region = coverage_region(&n, &parent).unwrap();
    let c = [120.0, 110.0];
    for deg in 0..360 {
        for b in [n_box, m_box] {
            for corner in [[b.x0, b.y0], [b.x1, b.y0], [b.x0, b.y1], [b.x1, b.y1]] {
                let p = rot(
                    [f64::from(corner[0]), f64::from(corner[1])],
                    c,
                    f64::from(deg),
                );
                // Into P's canvas pixels.
                let q = [p[0] / 1.5, p[1] / 1.5];
                assert!(
                    f64::from(region.x0) <= q[0] && q[0] <= f64::from(region.x1),
                    "x at {deg}°"
                );
                assert!(
                    f64::from(region.y0) <= q[1] && q[1] <= f64::from(region.y1),
                    "y at {deg}°"
                );
            }
        }
    }
    // A rotated window's own box is enough (its nested entries are clipped by it).
    let w = FootprintNode {
        window: true,
        ..n.clone()
    };
    assert!(coverage_region(&w, &parent).unwrap().area() < region.area());
}

#[test]
fn deep_chain_of_rotating_surfaces_is_linear() {
    // Finding: every node kept a copy of all its rotating ancestors, so a linear chain of
    // MAX_NODES rotating surfaces cost O(n²) memory (about 16 GiB) and time.
    let b = Rect::from_xywh(0, 0, 64, 64);
    let mut nodes = vec![CompNode {
        rotation: [0.0, 0.0, 45.0],
        ..window(0, None, b)
    }];
    for i in 1..tessera_ui::paint::MAX_NODES {
        nodes.push(CompNode {
            rotation: [0.0, 0.0, 45.0],
            ..node(i as u32, Some(i - 1), b)
        });
    }
    let start = std::time::Instant::now();
    let l = build(&nodes, &Lookup::default(), &ComposeConfig::default()).unwrap();
    // The root and 8 nested levels are drawn; everything deeper is skipped and counted.
    assert_eq!(l.entries.len(), 9);
    assert_eq!(l.counters.depth as usize, tessera_ui::paint::MAX_NODES - 9);
    for (k, e) in l.entries.iter().enumerate() {
        assert_eq!(l.rotations_of(e).len(), k + 1, "entry {k}");
    }
    // Generous: linear work finishes in well under a second even unoptimised.
    assert!(start.elapsed() < std::time::Duration::from_secs(20));
}

#[test]
fn compose_caps_come_from_the_paint_limits() {
    let lim = tessera_ui::paint::PaintLimits {
        max_entries: 3,
        max_depth: 2,
        ..tessera_ui::paint::PaintLimits::default()
    };
    let c = ComposeConfig::from_limits(&lim);
    assert_eq!((c.max_entries, c.max_depth), (3, 2));
    let d = ComposeConfig::default();
    let pd = tessera_ui::paint::PaintLimits::default();
    assert_eq!((d.max_entries, d.max_depth), (pd.max_entries, pd.max_depth));
}
