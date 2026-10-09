// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui::paint` composition-list builder and coverage regions (decision 0015
//! Decision 1.6, 1.7, §7; `render.md` §6.5–§6.8): arbitrary trees of nested surfaces,
//! windows with and without surfaces, external content, Tile parents, hidden ancestors,
//! rotations (NaN and infinite included), opacities, scales and crops, under small caps.
//! Nothing may panic; the entry cap holds; no clip chain is longer than 8 and no transform
//! chain longer than 9; no entry is emitted for a surface under a hidden ancestor, below a
//! Tile surface or past the depth cap; every opacity and threshold is in 0–1; every range
//! points into the list. Coverage regions lie inside the stored part of the parent canvas.
//!
//! Input layout: a config byte, then the bytes every field reads its value from (cycled
//! when short).

#![no_main]

use libfuzzer_sys::fuzz_target;
use tessera_ui::paint::{
    Addressing, CompNode, ComposeConfig, EntryTexture, FootprintNode, Layer, NodeContent,
    OpacityRule, ParentCanvas, Rect, SurfaceId, TextureLookup, build, coverage_region,
};

struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bytes<'_> {
    fn byte(&mut self) -> u8 {
        if self.data.is_empty() {
            return 0;
        }
        let b = self.data[self.pos % self.data.len()];
        self.pos += 1;
        b
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }
    fn coord(&mut self) -> i32 {
        match self.byte() % 6 {
            0 => self.u32() as i32,
            _ => i32::from(self.byte()) * 4 - 200,
        }
    }
    fn rect(&mut self) -> Rect {
        let x = self.coord();
        let y = self.coord();
        Rect::from_xywh(x, y, u32::from(self.byte()) * 3, u32::from(self.byte()) * 3)
    }
    fn f32(&mut self) -> f32 {
        match self.byte() % 5 {
            0 => f32::from_bits(self.u32()),
            1 => 0.0,
            _ => f32::from(self.byte() as i8),
        }
    }
}

struct Lookup(u8);

impl TextureLookup for Lookup {
    fn front_box(&self, s: SurfaceId, layer: Layer) -> Option<Rect> {
        let k = (s.0 as u8) ^ self.0 ^ matches!(layer, Layer::Below) as u8;
        (!k.is_multiple_of(3)).then(|| Rect::new(0, 0, 256, 256).translate(i32::from(k), 0))
    }
    fn is_drawn(&self, s: SurfaceId) -> bool {
        (s.0 as u8) ^ self.0 != 0xFF
    }
}

fuzz_target!(|data: &[u8]| {
    let mut b = Bytes { data, pos: 0 };
    let cfg = b.byte();
    let config = ComposeConfig {
        max_entries: usize::from(b.byte() % 64) + 1,
        max_depth: u32::from(b.byte() % 10),
        opacity_rule: if cfg & 1 == 0 {
            OpacityRule::Own
        } else {
            OpacityRule::OwnTimesAncestors
        },
        ..ComposeConfig::default()
    };
    let n = usize::from(b.byte() % 48);
    let mut nodes: Vec<CompNode> = Vec::with_capacity(n);
    // Mostly valid pre-order trees: the parent is on the open path (an ancestor of the
    // previous node), sometimes anything.
    let mut path: Vec<usize> = Vec::new();
    for j in 0..n {
        let parent = match b.byte() % 8 {
            0 => None,
            1 => Some(usize::from(b.byte())),
            _ => {
                let keep = usize::from(b.byte()) % (path.len() + 1);
                path.truncate(keep);
                path.last().copied()
            }
        };
        if parent.is_none() {
            path.clear();
        }
        let content = match b.byte() % 6 {
            0 => NodeContent::None,
            1 => NodeContent::External(u64::from(b.byte())),
            _ => NodeContent::Canvas,
        };
        let displayed = b.rect();
        nodes.push(CompNode {
            id: SurfaceId(j as u32),
            parent,
            window: b.byte().is_multiple_of(3),
            content,
            shown: !b.byte().is_multiple_of(7),
            displayed,
            canvas: [u32::from(b.byte()) + 1, u32::from(b.byte()) + 1],
            canvas_origin: [b.coord(), b.coord()],
            addressing: if b.byte().is_multiple_of(9) {
                Addressing::Tile {
                    node_scale: [b.f32(), b.f32()],
                }
            } else {
                Addressing::Fit
            },
            opacity: b.f32(),
            stand_in: b.byte().is_multiple_of(11),
            rotation: [b.f32(), b.f32(), b.f32()],
            alpha_threshold: b.f32(),
        });
        path.push(j);
    }
    let lookup = Lookup(cfg);
    let Ok(list) = build(&nodes, &lookup, &config) else {
        return;
    };
    assert!(list.entries.len() <= config.max_entries);
    // Which nodes may have entries: displayed, not below a Tile surface, within the depth.
    let mut ok = vec![false; nodes.len()];
    let mut displayed = vec![false; nodes.len()];
    let mut depth = vec![0u32; nodes.len()];
    let mut tile = vec![false; nodes.len()];
    for (j, nd) in nodes.iter().enumerate() {
        let p = nd.parent;
        displayed[j] = nd.shown && p.is_none_or(|p| displayed[p]);
        if let Some(p) = p {
            let ps = nodes[p].content == NodeContent::Canvas;
            depth[j] = depth[p] + u32::from(ps);
            tile[j] = tile[p] || (ps && matches!(nodes[p].addressing, Addressing::Tile { .. }));
        }
        ok[j] = displayed[j] && !tile[j] && depth[j] <= config.max_depth.min(8);
    }
    for e in &list.entries {
        assert!(list.clips_of(e).len() <= 8, "clip chain over 8");
        assert!(list.rotations_of(e).len() <= 9, "transform chain over 9");
        assert!(e.clips.end <= list.clips.len() && e.rotations.end <= list.rotations.len());
        assert!((0.0..=1.0).contains(&e.opacity));
        assert!((0.0..=1.0).contains(&e.alpha_threshold));
        if let EntryTexture::Surface {
            surface,
            layer: Layer::Below,
        } = e.texture
        {
            assert!(
                ok[surface.0 as usize],
                "entry for a surface that must have none"
            );
        }
        if let EntryTexture::Surface {
            layer: Layer::Above(n),
            ..
        } = e.texture
        {
            assert!(
                ok[n as usize],
                "canvas above for an entry that is not displayed"
            );
        }
    }

    // Coverage regions of an arbitrary small footprint tree.
    let leaf = |b: &mut Bytes<'_>| FootprintNode {
        displayed: b.rect(),
        own_rotation: b.byte().is_multiple_of(2),
        window: b.byte().is_multiple_of(2),
        nested: Vec::new(),
    };
    let mut fp = leaf(&mut b);
    for _ in 0..usize::from(b.byte() % 4) {
        let mut c = leaf(&mut b);
        c.nested.push(leaf(&mut b));
        fp.nested.push(c);
    }
    let canvas = [u32::from(b.byte()) * 64 + 1, u32::from(b.byte()) * 64 + 1];
    // The stored part: the whole canvas, or a crop of it (§7).
    let stored = if b.byte().is_multiple_of(2) {
        canvas
    } else {
        [
            canvas[0].min(u32::from(b.byte()) * 16 + 1),
            canvas[1].min(u32::from(b.byte()) * 16 + 1),
        ]
    };
    let parent = ParentCanvas {
        displayed: b.rect(),
        canvas,
        canvas_origin: [b.coord(), b.coord()],
        stored,
    };
    if let Some(r) = coverage_region(&fp, &parent) {
        assert!(Rect::of_size(parent.stored[0], parent.stored[1]).contains_rect(&r));
        assert!(!r.is_empty());
    }
});
