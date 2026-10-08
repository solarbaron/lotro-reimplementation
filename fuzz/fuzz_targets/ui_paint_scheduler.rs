// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-ui::paint::Scheduler` (decision 0015 Decision 1.5, 1.8, U3–U5; `render.md`
//! §6.12): arbitrary sequences of surface creation and release, damage, snapshot changes
//! (sizes across the tile edge, fills and nested-entry markers), layout changes, pending
//! images and frames with small upload caps, paint slices and texture budgets. A model GPU
//! applies every operation. Nothing may panic; every operation names a texture that exists
//! (none created twice); uploads lie inside their texture with the right byte count; the
//! upload cap holds; and once everything has settled, every drawn surface's front textures
//! show exactly its CPU canvases.
//!
//! Input layout: a limits byte, then one command per byte group (cycled when short, at most
//! 96 commands).

#![no_main]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use libfuzzer_sys::fuzz_target;
use tessera_ui::paint::{
    BandBudget, DisplayList, FillRect, GpuOp, GroupId, PaintLimits, PaintOp, Rect, Scheduler, Slot,
    Snapshot, SnapshotSource, SurfaceId, TexKey,
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
}

#[derive(Default)]
struct Source(HashMap<SurfaceId, Snapshot>);

impl SnapshotSource for Source {
    fn snapshot(&mut self, s: SurfaceId) -> Option<Snapshot> {
        self.0.get(&s).cloned()
    }
}

#[derive(Default)]
struct Gpu(HashMap<TexKey, (Rect, Vec<u32>)>);

fn idx(b: &Rect, x: i32, y: i32) -> usize {
    ((y - b.y0) as usize) * b.width() as usize + (x - b.x0) as usize
}

impl Gpu {
    fn apply(&mut self, ops: &[GpuOp]) {
        for op in ops {
            match op {
                GpuOp::Create { tex, tile_box } => {
                    assert!(!tile_box.is_empty());
                    let old = self
                        .0
                        .insert(*tex, (*tile_box, vec![0; tile_box.area() as usize]));
                    assert!(old.is_none(), "texture created twice");
                }
                GpuOp::Copy { from, to } => {
                    let (fb, fp) = self.0.get(from).expect("copy source").clone();
                    let (tb, tp) = self.0.get_mut(to).expect("copy target");
                    let o = fb.intersect(tb);
                    for y in o.y0..o.y1 {
                        for x in o.x0..o.x1 {
                            tp[idx(tb, x, y)] = fp[idx(&fb, x, y)];
                        }
                    }
                }
                GpuOp::Upload { tex, rect, bytes } => {
                    let (b, p) = self.0.get_mut(tex).expect("upload target");
                    assert!(b.contains_rect(rect) && !rect.is_empty());
                    assert_eq!(bytes.len() as u64, rect.area() * 4);
                    let (px, _) = bytes.as_chunks::<4>();
                    let mut it = px.iter();
                    for y in rect.y0..rect.y1 {
                        for x in rect.x0..rect.x1 {
                            p[idx(b, x, y)] = u32::from_le_bytes(*it.next().expect("bytes"));
                        }
                    }
                }
                GpuOp::Clear { tex, rect } => {
                    let (b, p) = self.0.get_mut(tex).expect("clear target");
                    assert!(b.contains_rect(rect));
                    for y in rect.y0..rect.y1 {
                        for x in rect.x0..rect.x1 {
                            p[idx(b, x, y)] = 0;
                        }
                    }
                }
                GpuOp::Promote { surface, layer } => {
                    let back = self
                        .0
                        .remove(&TexKey {
                            surface: *surface,
                            layer: *layer,
                            slot: Slot::Back,
                        })
                        .expect("promoted back");
                    self.0.insert(
                        TexKey {
                            surface: *surface,
                            layer: *layer,
                            slot: Slot::Front,
                        },
                        back,
                    );
                }
                GpuOp::Destroy { tex } => {
                    assert!(self.0.remove(tex).is_some(), "destroyed a missing texture");
                }
            }
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let mut b = Bytes { data, pos: 0 };
    let l = b.byte();
    let limits = PaintLimits {
        upload_cap_bytes: [64, 4096, 65_536, 8 << 20][usize::from(l % 4)],
        texture_bytes: [1 << 14, 1 << 20, 256 << 20, 256 << 20][usize::from((l >> 2) % 4)],
        ..PaintLimits::default()
    };
    let mut s: Scheduler<u8> = Scheduler::new(limits);
    let mut src = Source::default();
    let mut gpu = Gpu::default();
    let mut now = Duration::ZERO;
    let mut live: Vec<SurfaceId> = Vec::new();
    let cmds = usize::from(b.byte() % 96);
    for _ in 0..cmds {
        let id = SurfaceId(u32::from(b.byte() % 6));
        let group = GroupId(id.0 % 3);
        match b.byte() % 8 {
            0 => {
                if s.create_surface(id, group, now).is_ok() {
                    live.push(id);
                }
            }
            1 => {
                s.release_surface(id);
                live.retain(|x| *x != id);
            }
            2 => {
                let r = Rect::from_xywh(
                    i32::from(b.byte()) - 20,
                    i32::from(b.byte() % 40) - 5,
                    u32::from(b.byte()),
                    u32::from(b.byte() % 40),
                );
                s.add_damage(id, r);
            }
            3 => {
                // A new snapshot: fills and markers on a canvas up to 300 × 40.
                let w = u32::from(b.byte()) + u32::from(b.byte() % 2) * 60 + 1;
                let h = u32::from(b.byte() % 40) + 1;
                let mut list = DisplayList::new();
                for _ in 0..usize::from(b.byte() % 5) {
                    let r = Rect::from_xywh(
                        i32::from(b.byte()) - 10,
                        i32::from(b.byte() % 40),
                        u32::from(b.byte()),
                        u32::from(b.byte() % 40),
                    );
                    let op = if b.byte().is_multiple_of(3) {
                        PaintOp::Nested {
                            entry: u32::from(b.byte() % 3),
                            coverage: r,
                        }
                    } else {
                        let c = u32::from_le_bytes([b.byte(), b.byte(), b.byte(), b.byte()]);
                        PaintOp::Fill(FillRect {
                            rect: r,
                            clip: r,
                            colour: c,
                        })
                    };
                    let _ = list.push(op);
                }
                src.0.insert(
                    id,
                    Snapshot {
                        list: Arc::new(list),
                        size: [w, h],
                    },
                );
                s.damage_all(id);
            }
            4 => {
                let members: Vec<SurfaceId> = live
                    .iter()
                    .copied()
                    .filter(|m| GroupId(m.0 % 3) == group && b.byte().is_multiple_of(2))
                    .collect();
                s.layout_change(group, b.byte(), &members);
            }
            5 => s.set_images_pending(id, b.byte().is_multiple_of(2)),
            _ => {
                now += Duration::from_millis(u64::from(b.byte() % 40));
                let out = s.frame(now, &mut src, &mut BandBudget(u32::from(b.byte() % 4)));
                assert!(out.stats.upload_bytes <= limits.upload_cap_bytes);
                gpu.apply(&out.ops);
            }
        }
    }
    // Settle: no new work, enough frames for every job, upload and generation to end.
    for id in &live {
        s.set_images_pending(*id, false);
    }
    for _ in 0..4000 {
        now += Duration::from_millis(16);
        let out = s.frame(now, &mut src, &mut BandBudget(64));
        gpu.apply(&out.ops);
        if out.ops.is_empty() && live.iter().all(|i| !s.is_busy(*i)) {
            break;
        }
    }
    for id in &live {
        if s.is_busy(*id) || !s.is_drawn(*id) {
            // Waiting for texture memory is allowed to last.
            continue;
        }
        let Some(set) = s.canvases(*id) else {
            continue;
        };
        for (layer, canvas) in set.layers() {
            let front = gpu.0.get(&TexKey {
                surface: *id,
                layer,
                slot: Slot::Front,
            });
            let Some((bx, px)) = front else {
                if limits.texture_bytes >= 256 << 20 {
                    assert_eq!(canvas.allocated_tiles(), 0, "content without a texture");
                }
                continue;
            };
            for y in 0..canvas.height() as i32 {
                for x in 0..canvas.width() as i32 {
                    let shown = if bx.contains(x, y) {
                        px[idx(bx, x, y)]
                    } else {
                        0
                    };
                    assert_eq!(shown, canvas.pixel(x, y), "front differs from the canvas");
                }
            }
        }
    }
});
