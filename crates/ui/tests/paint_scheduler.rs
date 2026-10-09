// SPDX-License-Identifier: GPL-3.0-or-later
//! Paint jobs over frames, uploads, back textures, generations and first paints (decision
//! 0015 Decision 1.5, 1.8, U3–U5; `render.md` §6.12). A model of the GPU applies every
//! emitted operation, so the tests check what the screen would show. Synthetic data only.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tessera_ui::paint::{
    BandBudget, DisplayList, FillRect, GpuOp, GroupId, Layer, PaintLimits, PaintOp, Rect,
    Scheduler, Slot, Snapshot, SnapshotSource, SurfaceId, TexKey, Unlimited,
};

fn ms(t: u64) -> Duration {
    Duration::from_millis(t)
}

/// Display lists by surface.
#[derive(Default)]
struct Source(HashMap<SurfaceId, Snapshot>);

impl Source {
    fn set(&mut self, id: u32, size: [u32; 2], ops: Vec<PaintOp>) {
        let mut l = DisplayList::new();
        for o in ops {
            l.push(o).unwrap();
        }
        self.0.insert(
            SurfaceId(id),
            Snapshot {
                list: Arc::new(l),
                size,
            },
        );
    }
}

impl SnapshotSource for Source {
    fn snapshot(&mut self, surface: SurfaceId) -> Option<Snapshot> {
        self.0.get(&surface).cloned()
    }
}

fn fill(rect: Rect, colour: u32) -> PaintOp {
    PaintOp::Fill(FillRect {
        rect,
        clip: rect,
        colour,
    })
}

/// The GPU side: textures by key, each a box and its pixels.
#[derive(Default, Debug)]
struct Gpu {
    tex: HashMap<TexKey, (Rect, Vec<u32>)>,
}

impl Gpu {
    fn apply(&mut self, ops: &[GpuOp]) {
        for op in ops {
            match op {
                GpuOp::Create { tex, tile_box } => {
                    assert!(!self.tex.contains_key(tex), "created twice: {tex:?}");
                    self.tex
                        .insert(*tex, (*tile_box, vec![0; tile_box.area() as usize]));
                }
                GpuOp::Copy { from, to } => {
                    let (fb, fp) = self.tex.get(from).expect("copy source").clone();
                    let (tb, tp) = self.tex.get_mut(to).expect("copy target");
                    let o = fb.intersect(tb);
                    for y in o.y0..o.y1 {
                        for x in o.x0..o.x1 {
                            tp[idx(tb, x, y)] = fp[idx(&fb, x, y)];
                        }
                    }
                }
                GpuOp::Upload { tex, rect, bytes } => {
                    let (b, p) = self.tex.get_mut(tex).expect("upload target");
                    assert!(b.contains_rect(rect), "upload outside the box");
                    assert_eq!(bytes.len() as u64, rect.area() * 4);
                    let (px, _) = bytes.as_chunks::<4>();
                    let mut it = px.iter();
                    for y in rect.y0..rect.y1 {
                        for x in rect.x0..rect.x1 {
                            p[idx(b, x, y)] = u32::from_le_bytes(*it.next().unwrap());
                        }
                    }
                }
                GpuOp::Clear { tex, rect } => {
                    let (b, p) = self.tex.get_mut(tex).expect("clear target");
                    assert!(b.contains_rect(rect));
                    for y in rect.y0..rect.y1 {
                        for x in rect.x0..rect.x1 {
                            p[idx(b, x, y)] = 0;
                        }
                    }
                }
                GpuOp::Promote { surface, layer } => {
                    let back = self
                        .tex
                        .remove(&TexKey {
                            surface: *surface,
                            layer: *layer,
                            slot: Slot::Back,
                        })
                        .expect("promoted back exists");
                    self.tex.insert(
                        TexKey {
                            surface: *surface,
                            layer: *layer,
                            slot: Slot::Front,
                        },
                        back,
                    );
                }
                GpuOp::Destroy { tex } => {
                    assert!(self.tex.remove(tex).is_some(), "destroyed twice: {tex:?}");
                }
            }
        }
    }

    fn front(&self, s: u32, layer: Layer, x: i32, y: i32) -> u32 {
        let k = TexKey {
            surface: SurfaceId(s),
            layer,
            slot: Slot::Front,
        };
        match self.tex.get(&k) {
            Some((b, p)) if b.contains(x, y) => p[idx(b, x, y)],
            _ => 0,
        }
    }

    fn front_snapshot(&self, s: u32, size: [u32; 2]) -> Vec<u32> {
        let mut v = Vec::new();
        for y in 0..size[1] as i32 {
            for x in 0..size[0] as i32 {
                v.push(self.front(s, Layer::Below, x, y));
            }
        }
        v
    }
}

fn idx(b: &Rect, x: i32, y: i32) -> usize {
    ((y - b.y0) as usize) * b.width() as usize + (x - b.x0) as usize
}

fn uploads_to(ops: &[GpuOp], s: u32, slot: Slot) -> usize {
    ops.iter()
        .filter(|o| {
            matches!(o, GpuOp::Upload { tex, .. } if tex.surface == SurfaceId(s) && tex.slot == slot)
        })
        .count()
}

/// Checks that the front textures show exactly the CPU canvases.
fn assert_front_is_canvas(gpu: &Gpu, sched: &Scheduler<&'static str>, s: u32) {
    let set = sched.canvases(SurfaceId(s)).expect("canvases");
    for (layer, canvas) in set.layers() {
        for y in 0..canvas.height() as i32 {
            for x in 0..canvas.width() as i32 {
                assert_eq!(
                    gpu.front(s, layer, x, y),
                    canvas.pixel(x, y),
                    "surface {s} {layer:?} at ({x}, {y})"
                );
            }
        }
    }
}

fn limits() -> PaintLimits {
    PaintLimits::default()
}

#[test]
fn first_paint_of_a_new_group_swaps_with_its_geometry() {
    let mut src = Source::default();
    src.set(
        1,
        [300, 40],
        vec![fill(Rect::new(0, 0, 300, 40), 0xFF11_2233)],
    );
    let mut s = Scheduler::new(limits());
    let mut gpu = Gpu::default();
    s.create_surface(SurfaceId(1), GroupId(1), Duration::ZERO)
        .unwrap();
    s.layout_change(GroupId(1), "g1", &[SurfaceId(1)]);
    assert!(s.shown(GroupId(1)).is_none());
    let out = s.frame(Duration::ZERO, &mut src, &mut Unlimited);
    gpu.apply(&out.ops);
    assert_eq!(out.swapped, vec![GroupId(1)]);
    assert_eq!(s.shown(GroupId(1)), Some(&"g1"));
    assert!(s.is_drawn(SurfaceId(1)));
    assert!(!s.is_busy(SurfaceId(1)));
    assert_front_is_canvas(&gpu, &s, 1);
    // The texture covers the stored part (two tiles across, cut at the canvas edge).
    assert_eq!(
        s.front_box(SurfaceId(1), Layer::Below),
        Some(Rect::new(0, 0, 300, 40))
    );
}

#[test]
fn first_paint_waits_for_images_at_most_100_ms() {
    let mut src = Source::default();
    src.set(1, [16, 16], vec![fill(Rect::of_size(16, 16), 0xFF00_00FF)]);
    let mut s = Scheduler::<&str>::new(limits());
    s.create_surface(SurfaceId(1), GroupId(1), Duration::ZERO)
        .unwrap();
    s.set_images_pending(SurfaceId(1), true);
    for t in [0u64, 16, 33, 50, 66, 83, 99] {
        let out = s.frame(ms(t), &mut src, &mut Unlimited);
        assert!(out.first_drawn.is_empty(), "withheld at {t} ms");
        assert!(!s.is_drawn(SurfaceId(1)));
    }
    let out = s.frame(ms(100), &mut src, &mut Unlimited);
    assert_eq!(out.first_drawn, vec![SurfaceId(1)]);

    // With the images in, it is drawn as soon as the job is complete and uploaded.
    let mut s = Scheduler::<&str>::new(limits());
    s.create_surface(SurfaceId(1), GroupId(1), Duration::ZERO)
        .unwrap();
    s.set_images_pending(SurfaceId(1), true);
    s.frame(Duration::ZERO, &mut src, &mut Unlimited);
    s.set_images_pending(SurfaceId(1), false);
    let out = s.frame(ms(16), &mut src, &mut Unlimited);
    assert_eq!(out.first_drawn, vec![SurfaceId(1)]);
}

#[test]
fn slow_first_paint_appears_in_bands_after_100_ms() {
    let mut src = Source::default();
    src.set(
        1,
        [64, 128],
        vec![fill(Rect::of_size(64, 128), 0xFF80_8080)],
    );
    let mut s = Scheduler::<&str>::new(limits());
    let mut gpu = Gpu::default();
    s.create_surface(SurfaceId(1), GroupId(1), Duration::ZERO)
        .unwrap();
    let mut t = 0;
    // One band (16 rows) per frame: 8 frames for 128 rows.
    loop {
        let out = s.frame(ms(t), &mut src, &mut BandBudget(0));
        gpu.apply(&out.ops);
        if s.is_drawn(SurfaceId(1)) {
            break;
        }
        t += 40;
        assert!(t <= 120, "drawn by the 100 ms bound");
    }
    t += 40;
    assert!(
        s.is_busy(SurfaceId(1)),
        "released before its job is complete"
    );
    // The released bands are in the texture; the rest is transparent black.
    assert_eq!(gpu.front(1, Layer::Below, 0, 0), 0xFF80_8080);
    assert_eq!(gpu.front(1, Layer::Below, 0, 127), 0);
    while s.is_busy(SurfaceId(1)) {
        let out = s.frame(ms(t), &mut src, &mut BandBudget(0));
        assert_eq!(
            uploads_to(&out.ops, 1, Slot::Back),
            0,
            "bands go to the front"
        );
        gpu.apply(&out.ops);
        t += 16;
    }
    assert_front_is_canvas(&gpu, &s, 1);
}

/// Shows surface 1 (64 × 64, grey) and runs until it is idle.
fn shown_surface(lim: PaintLimits) -> (Scheduler<&'static str>, Source, Gpu, u64) {
    let mut src = Source::default();
    src.set(1, [64, 64], vec![fill(Rect::of_size(64, 64), 0xFF80_8080)]);
    let mut s = Scheduler::new(lim);
    let mut gpu = Gpu::default();
    s.create_surface(SurfaceId(1), GroupId(1), Duration::ZERO)
        .unwrap();
    s.layout_change(GroupId(1), "g1", &[SurfaceId(1)]);
    let mut t = 0;
    while !s.is_drawn(SurfaceId(1)) || s.is_busy(SurfaceId(1)) {
        let out = s.frame(ms(t), &mut src, &mut Unlimited);
        gpu.apply(&out.ops);
        t += 16;
        assert!(t < 1000);
    }
    (s, src, gpu, t)
}

#[test]
fn content_job_uploads_straight_into_the_front() {
    let (mut s, mut src, mut gpu, t) = shown_surface(limits());
    src.set(
        1,
        [64, 64],
        vec![
            fill(Rect::of_size(64, 64), 0xFF80_8080),
            fill(Rect::new(4, 4, 8, 8), 0xFFFF_0000),
        ],
    );
    s.add_damage(SurfaceId(1), Rect::new(4, 4, 8, 8));
    let out = s.frame(ms(t), &mut src, &mut Unlimited);
    gpu.apply(&out.ops);
    assert_eq!(out.stats.upload_bytes, 4 * 4 * 4);
    assert!(
        out.ops
            .iter()
            .all(|o| !matches!(o, GpuOp::Create { .. } | GpuOp::Promote { .. }))
    );
    assert_eq!(gpu.front(1, Layer::Below, 5, 5), 0xFFFF_0000);
    assert_front_is_canvas(&gpu, &s, 1);
    // A setter that changes nothing adds no damage: no job, no upload.
    let out = s.frame(ms(t + 16), &mut src, &mut Unlimited);
    assert!(out.ops.is_empty() && out.stats.jobs_started == 0);
}

#[test]
fn erased_tiles_are_cleared_not_uploaded() {
    let (mut s, mut src, mut gpu, t) = shown_surface(limits());
    // A canvas wider than one tile: two tiles, then the right one emptied.
    src.set(1, [300, 8], vec![fill(Rect::of_size(300, 8), 0xFF01_0203)]);
    s.damage_all(SurfaceId(1));
    let out = s.frame(ms(t), &mut src, &mut Unlimited);
    gpu.apply(&out.ops);
    s.frame(ms(t + 16), &mut src, &mut Unlimited);
    src.set(1, [300, 8], vec![fill(Rect::of_size(256, 8), 0xFF01_0203)]);
    s.add_damage(SurfaceId(1), Rect::new(256, 0, 300, 8));
    let out = s.frame(ms(t + 32), &mut src, &mut Unlimited);
    gpu.apply(&out.ops);
    assert_eq!(out.stats.upload_bytes, 0);
    assert!(out.ops.iter().any(|o| matches!(o, GpuOp::Clear { .. })));
    assert_front_is_canvas(&gpu, &s, 1);
}

#[test]
fn content_over_the_cap_goes_through_a_back_texture() {
    let lim = PaintLimits {
        upload_cap_bytes: 64 * 4 * 8, // 8 rows of 64 pixels per frame
        ..limits()
    };
    let (mut s, mut src, mut gpu, mut t) = shown_surface(lim);
    let old = gpu.front_snapshot(1, [64, 64]);
    src.set(1, [64, 64], vec![fill(Rect::of_size(64, 64), 0xFF00_FF00)]);
    s.damage_all(SurfaceId(1));
    let mut frames = 0;
    loop {
        // New damage keeps arriving; it waits for the next job.
        s.add_damage(SurfaceId(1), Rect::new(0, 0, 2, 2));
        let out = s.frame(ms(t), &mut src, &mut Unlimited);
        assert_eq!(
            uploads_to(&out.ops, 1, Slot::Front),
            0,
            "never into the shown texture"
        );
        assert!(out.stats.upload_bytes <= 64 * 4 * 8);
        gpu.apply(&out.ops);
        t += 16;
        frames += 1;
        let promoted = out.ops.iter().any(|o| matches!(o, GpuOp::Promote { .. }));
        if promoted {
            break;
        }
        assert_eq!(
            gpu.front_snapshot(1, [64, 64]),
            old,
            "old picture until the swap"
        );
        assert_eq!(out.stats.jobs_started, u32::from(frames == 1));
    }
    assert_eq!(frames, 8, "64 rows at 8 rows per frame");
    assert_front_is_canvas(&gpu, &s, 1);
    assert_eq!(gpu.front(1, Layer::Below, 10, 10), 0xFF00_FF00);
}

#[test]
fn generation_swaps_once_with_its_withheld_nested_first_paint() {
    // Root 1 (128 × 64) gets a nested entry 2 shown inside it: the root repaints its split
    // (a generation member), the child needs a first paint whose images are pending.
    let (mut s, mut src, mut gpu, mut t) = shown_surface(limits());
    let old_root = gpu.front_snapshot(1, [64, 64]);
    src.set(
        1,
        [64, 64],
        vec![
            fill(Rect::of_size(64, 64), 0xFF80_8080),
            PaintOp::Nested {
                entry: 2,
                coverage: Rect::new(8, 8, 24, 24),
            },
            fill(Rect::new(0, 0, 64, 16), 0xFFAA_AAAA),
        ],
    );
    src.set(2, [16, 16], vec![fill(Rect::of_size(16, 16), 0xFF00_0080)]);
    // The coverage region and the new top band.
    s.add_damage(SurfaceId(1), Rect::new(8, 8, 24, 24));
    s.add_damage(SurfaceId(1), Rect::new(0, 0, 64, 16));
    s.create_surface(SurfaceId(2), GroupId(1), ms(t)).unwrap();
    s.set_images_pending(SurfaceId(2), true);
    let shown_at = t;
    s.layout_change(GroupId(1), "g2", &[SurfaceId(1), SurfaceId(2)]);
    let mut swapped_at = None;
    while swapped_at.is_none() {
        let out = s.frame(ms(t), &mut src, &mut Unlimited);
        if out.swapped.contains(&GroupId(1)) {
            swapped_at = Some(t);
            // The root's new canvas above is a new texture, so the root used back
            // textures; they are promoted in the swap frame.
            assert!(out.ops.iter().any(|o| matches!(
                o,
                GpuOp::Promote {
                    surface: SurfaceId(1),
                    layer: Layer::Above(2)
                }
            )));
            gpu.apply(&out.ops);
            break;
        }
        assert_eq!(
            uploads_to(&out.ops, 1, Slot::Front),
            0,
            "front untouched before the swap"
        );
        gpu.apply(&out.ops);
        assert_eq!(gpu.front_snapshot(1, [64, 64]), old_root);
        assert_eq!(s.shown(GroupId(1)), Some(&"g1"));
        assert!(!s.is_drawn(SurfaceId(2)));
        t += 16;
        assert!(t - shown_at <= 112);
    }
    // The child's 100 ms ran out (its images never arrived), then both appeared at once.
    let at = swapped_at.unwrap();
    assert!(at - shown_at >= 100 && at - shown_at < 117);
    assert_eq!(s.shown(GroupId(1)), Some(&"g2"));
    assert!(s.is_drawn(SurfaceId(2)));
    assert_front_is_canvas(&gpu, &s, 1);
    // The parent content drawn after the entry inside its region is in the canvas above.
    assert_eq!(gpu.front(1, Layer::Above(2), 10, 10), 0xFFAA_AAAA);
    assert_eq!(gpu.front(1, Layer::Below, 10, 10), 0xFF80_8080);
    assert_eq!(gpu.front(1, Layer::Below, 30, 10), 0xFFAA_AAAA);
}

#[test]
fn resize_generation_uses_back_textures_and_keeps_the_old_picture() {
    let (mut s, mut src, mut gpu, mut t) = shown_surface(limits());
    let old = gpu.front_snapshot(1, [64, 64]);
    src.set(1, [96, 80], vec![fill(Rect::of_size(96, 80), 0xFF12_3456)]);
    s.layout_change(GroupId(1), "big", &[SurfaceId(1)]);
    let mut frames = 0;
    loop {
        let out = s.frame(ms(t), &mut src, &mut BandBudget(0));
        assert_eq!(uploads_to(&out.ops, 1, Slot::Front), 0);
        gpu.apply(&out.ops);
        t += 16;
        frames += 1;
        if out.swapped.contains(&GroupId(1)) {
            assert!(out.ops.iter().any(|o| matches!(o, GpuOp::Promote { .. })));
            break;
        }
        assert_eq!(gpu.front_snapshot(1, [64, 64]), old);
        assert_eq!(s.shown(GroupId(1)), Some(&"g1"));
        assert!(frames < 20);
    }
    assert_eq!(frames, 5, "80 rows in 16-row bands, one band per frame");
    assert_eq!(s.shown(GroupId(1)), Some(&"big"));
    assert_front_is_canvas(&gpu, &s, 1);
}

#[test]
fn a_layout_change_during_a_generation_waits_for_the_next() {
    let (mut s, mut src, mut gpu, mut t) = shown_surface(limits());
    src.set(1, [64, 128], vec![fill(Rect::of_size(64, 128), 1)]);
    s.layout_change(GroupId(1), "g2", &[SurfaceId(1)]);
    let out = s.frame(ms(t), &mut src, &mut BandBudget(0));
    gpu.apply(&out.ops);
    t += 16;
    assert!(s.generation_running(GroupId(1)));
    src.set(1, [64, 32], vec![fill(Rect::of_size(64, 32), 2)]);
    s.layout_change(GroupId(1), "g3", &[SurfaceId(1)]);
    let mut seen = Vec::new();
    for _ in 0..40 {
        let out = s.frame(ms(t), &mut src, &mut BandBudget(0));
        gpu.apply(&out.ops);
        t += 16;
        if !out.swapped.is_empty() {
            seen.push(*s.shown(GroupId(1)).unwrap());
        }
    }
    assert_eq!(seen, vec!["g2", "g3"]);
    assert_front_is_canvas(&gpu, &s, 1);
}

#[test]
fn generation_holds_its_rectangles_until_the_swap() {
    let (mut s, mut src, mut gpu, mut t) = shown_surface(limits());
    let old = gpu.front_snapshot(1, [64, 64]);
    // A same-size layout change whose repaint takes four frames (one band each).
    src.set(1, [64, 64], vec![fill(Rect::of_size(64, 64), 0xFF44_4444)]);
    s.damage_all(SurfaceId(1));
    s.layout_change(GroupId(1), "g2", &[SurfaceId(1)]);
    let mut frames = 0;
    loop {
        let out = s.frame(ms(t), &mut src, &mut BandBudget(0));
        assert!(
            !out.ops.iter().any(|o| matches!(o, GpuOp::Create { .. })),
            "no back texture"
        );
        t += 16;
        frames += 1;
        if !out.swapped.is_empty() {
            assert_eq!(
                uploads_to(&out.ops, 1, Slot::Front),
                1,
                "the held canvas, one tile, at the swap"
            );
            gpu.apply(&out.ops);
            break;
        }
        assert!(out.ops.is_empty(), "held in the CPU canvas");
        gpu.apply(&out.ops);
        assert_eq!(gpu.front_snapshot(1, [64, 64]), old);
    }
    assert_eq!(frames, 4);
    assert_front_is_canvas(&gpu, &s, 1);
}

#[test]
fn held_rectangles_over_the_cap_migrate_to_back_textures() {
    let lim = PaintLimits {
        upload_cap_bytes: 64 * 4 * 16,
        ..limits()
    };
    let (mut s, mut src, mut gpu, mut t) = shown_surface(lim);
    src.set(1, [64, 64], vec![fill(Rect::of_size(64, 64), 0xFF33_3333)]);
    s.damage_all(SurfaceId(1));
    s.layout_change(GroupId(1), "g2", &[SurfaceId(1)]);
    let mut frames = 0;
    loop {
        let out = s.frame(ms(t), &mut src, &mut Unlimited);
        assert_eq!(uploads_to(&out.ops, 1, Slot::Front), 0);
        gpu.apply(&out.ops);
        t += 16;
        frames += 1;
        if !out.swapped.is_empty() {
            break;
        }
        assert!(frames < 20);
    }
    assert!(frames >= 4, "64 rows at 16 rows per frame");
    assert_front_is_canvas(&gpu, &s, 1);
}

#[test]
fn back_texture_over_the_memory_budget_waits() {
    let lim = PaintLimits {
        texture_bytes: 2 * 64 * 64 * 4, // two 64 × 64 textures
        upload_cap_bytes: 2048,
        ..limits()
    };
    let (mut s, mut src, mut gpu, mut t) = shown_surface(lim);
    // Another surface holds the rest of the texture budget.
    src.set(5, [64, 64], vec![fill(Rect::of_size(64, 64), 0xFF00_0005)]);
    s.create_surface(SurfaceId(5), GroupId(5), ms(t)).unwrap();
    while !s.is_drawn(SurfaceId(5)) || s.is_busy(SurfaceId(5)) {
        let out = s.frame(ms(t), &mut src, &mut Unlimited);
        gpu.apply(&out.ops);
        t += 16;
        assert!(t < 2000);
    }
    src.set(1, [64, 64], vec![fill(Rect::of_size(64, 64), 0xFF00_0001)]);
    s.damage_all(SurfaceId(1));
    let before = gpu.front_snapshot(1, [64, 64]);
    for _ in 0..5 {
        let out = s.frame(ms(t), &mut src, &mut Unlimited);
        gpu.apply(&out.ops);
        t += 16;
    }
    assert!(s.counters().back_texture_waits >= 1);
    assert_eq!(s.counters().memory, 0, "a wait, not a refusal");
    assert_eq!(
        gpu.front_snapshot(1, [64, 64]),
        before,
        "the shown picture stays"
    );
    assert!(s.is_busy(SurfaceId(1)));
    // Memory is freed: the back texture is made and swapped in.
    s.release_surface(SurfaceId(5));
    for _ in 0..40 {
        let out = s.frame(ms(t), &mut src, &mut Unlimited);
        gpu.apply(&out.ops);
        t += 16;
        if !s.is_busy(SurfaceId(1)) {
            break;
        }
    }
    assert!(!s.is_busy(SurfaceId(1)));
    assert_eq!(gpu.front(1, Layer::Below, 3, 3), 0xFF00_0001);
    assert_front_is_canvas(&gpu, &s, 1);
}

#[test]
fn back_texture_that_can_never_fit_does_not_stall_its_generation() {
    // Finding: with a texture budget below one back texture of the surface, the job waited
    // in AwaitingBack forever and the group's generation never swapped.
    let lim = PaintLimits {
        texture_bytes: 64 * 64 * 4, // the front only: front + back can never fit
        ..limits()
    };
    let (mut s, mut src, mut gpu, mut t) = shown_surface(lim);
    // A resize generation needs back textures.
    src.set(1, [64, 80], vec![fill(Rect::of_size(64, 80), 0xFF00_0002)]);
    s.layout_change(GroupId(1), "g2", &[SurfaceId(1)]);
    let mut swapped = false;
    for _ in 0..20 {
        let out = s.frame(ms(t), &mut src, &mut Unlimited);
        gpu.apply(&out.ops);
        t += 16;
        swapped |= out.swapped.contains(&GroupId(1));
        if swapped {
            break;
        }
    }
    assert!(swapped, "the generation swaps");
    assert_eq!(s.shown(GroupId(1)), Some(&"g2"));
    assert!(!s.generation_running(GroupId(1)));
    assert!(!s.is_busy(SurfaceId(1)));
    assert!(s.counters().memory >= 1, "counted as a memory refusal");
    // The surface draws nothing rather than a stale picture, and nothing leaks.
    assert!(s.front_layers(SurfaceId(1)).is_empty());
    assert!(gpu.tex.is_empty());
    assert_eq!(s.texture_bytes(), 0);
    // A later layout change of the group still swaps.
    src.set(1, [32, 32], vec![fill(Rect::of_size(32, 32), 0xFF00_0003)]);
    s.layout_change(GroupId(1), "g3", &[SurfaceId(1)]);
    let mut swapped = false;
    for _ in 0..20 {
        let out = s.frame(ms(t), &mut src, &mut Unlimited);
        gpu.apply(&out.ops);
        t += 16;
        swapped |= out.swapped.contains(&GroupId(1));
    }
    assert!(swapped);
    assert_eq!(s.shown(GroupId(1)), Some(&"g3"));
    assert_front_is_canvas(&gpu, &s, 1);
}

#[test]
fn first_paints_take_the_slice_before_content_jobs() {
    let (mut s, mut src, mut gpu, t) = shown_surface(limits());
    // A big content job on surface 1 starts first...
    src.set(1, [64, 64], vec![fill(Rect::of_size(64, 64), 7)]);
    s.damage_all(SurfaceId(1));
    // ...and a new root in another group is shown in the same frame.
    src.set(9, [16, 32], vec![fill(Rect::of_size(16, 32), 9)]);
    s.create_surface(SurfaceId(9), GroupId(9), ms(t)).unwrap();
    let out = s.frame(ms(t), &mut src, &mut BandBudget(0));
    gpu.apply(&out.ops);
    assert_eq!(out.stats.bands, 1);
    assert_eq!(s.canvases(SurfaceId(9)).unwrap().below().pixel(0, 0), 9);
    assert_eq!(
        s.canvases(SurfaceId(1)).unwrap().below().pixel(0, 0),
        0xFF80_8080
    );
}

#[test]
fn release_destroys_textures() {
    let (mut s, mut src, mut gpu, t) = shown_surface(limits());
    s.release_surface(SurfaceId(1));
    let out = s.frame(ms(t), &mut src, &mut Unlimited);
    gpu.apply(&out.ops);
    assert!(gpu.tex.is_empty());
    assert_eq!(s.texture_bytes(), 0);
    assert!(
        s.create_surface(SurfaceId(1), GroupId(1), Duration::ZERO)
            .is_ok()
    );
    assert!(
        s.create_surface(SurfaceId(1), GroupId(1), Duration::ZERO)
            .is_err()
    );
}

/// Runs frames until a frame starts no job, emits nothing, and leaves no surface busy and
/// no generation running (at most `n` frames).
fn run_idle(
    s: &mut Scheduler<&'static str>,
    src: &mut Source,
    gpu: &mut Gpu,
    t: &mut u64,
    ids: &[u32],
    n: usize,
) {
    for _ in 0..n {
        let out = s.frame(ms(*t), src, &mut Unlimited);
        gpu.apply(&out.ops);
        *t += 16;
        let busy = ids.iter().any(|i| s.is_busy(SurfaceId(*i)));
        let quiet = out.ops.is_empty() && out.stats.jobs_started == 0;
        if quiet && !busy && !s.generation_running(GroupId(1)) {
            return;
        }
    }
    panic!("not idle after {n} frames");
}

#[test]
fn content_changed_while_a_generation_waits_for_a_content_job_is_painted() {
    // Finding: the generation member's snapshot was taken when the generation started,
    // but its damage when its job started frames later, so damage of newer content was
    // painted from the old snapshot and dropped.
    const A: u32 = 0xFFAA_0000;
    const B: u32 = 0xFF00_BB00;
    let (mut s, mut src, mut gpu, mut t) = shown_surface(limits());
    src.set(1, [64, 64], vec![fill(Rect::of_size(64, 64), A)]);
    s.damage_all(SurfaceId(1));
    // A content job over four frames (one band each).
    let out = s.frame(ms(t), &mut src, &mut BandBudget(1));
    gpu.apply(&out.ops);
    t += 16;
    assert!(s.is_busy(SurfaceId(1)));
    // A layout change while the content job runs: the generation starts with colour A.
    s.add_damage(SurfaceId(1), Rect::new(0, 0, 4, 4));
    s.layout_change(GroupId(1), "g2", &[SurfaceId(1)]);
    let out = s.frame(ms(t), &mut src, &mut BandBudget(1));
    gpu.apply(&out.ops);
    t += 16;
    assert!(s.generation_running(GroupId(1)));
    // The content changes to B before the member's generation job has started.
    src.set(1, [64, 64], vec![fill(Rect::of_size(64, 64), B)]);
    s.damage_all(SurfaceId(1));
    run_idle(&mut s, &mut src, &mut gpu, &mut t, &[1], 100);
    assert_eq!(s.shown(GroupId(1)), Some(&"g2"));
    assert_eq!(s.canvases(SurfaceId(1)).unwrap().below().pixel(10, 60), B);
    assert_eq!(
        gpu.front(1, Layer::Below, 10, 60),
        B,
        "B reaches the screen"
    );
    assert_front_is_canvas(&gpu, &s, 1);
}

#[test]
fn a_surface_that_becomes_empty_loses_its_texture() {
    // Finding: a `None` snapshot only dropped the damage; the old texture stayed composed.
    let (mut s, mut src, mut gpu, mut t) = shown_surface(limits());
    assert!(s.front_box(SurfaceId(1), Layer::Below).is_some());
    src.0.remove(&SurfaceId(1));
    s.damage_all(SurfaceId(1));
    run_idle(&mut s, &mut src, &mut gpu, &mut t, &[1], 10);
    assert_eq!(s.front_box(SurfaceId(1), Layer::Below), None);
    assert!(s.front_layers(SurfaceId(1)).is_empty());
    assert!(gpu.tex.is_empty());
    assert_eq!(s.texture_bytes(), 0);
    assert_eq!(s.canvas_bytes(), 0);
    // Content again: painted and shown.
    src.set(1, [32, 32], vec![fill(Rect::of_size(32, 32), 0xFF00_00EE)]);
    s.damage_all(SurfaceId(1));
    run_idle(&mut s, &mut src, &mut gpu, &mut t, &[1], 10);
    assert_eq!(gpu.front(1, Layer::Below, 5, 5), 0xFF00_00EE);
    assert_front_is_canvas(&gpu, &s, 1);

    // As a generation member: the texture goes at the swap, not before.
    src.0.remove(&SurfaceId(1));
    s.damage_all(SurfaceId(1));
    s.layout_change(GroupId(1), "g-empty", &[SurfaceId(1)]);
    let out = s.frame(ms(t), &mut src, &mut Unlimited);
    gpu.apply(&out.ops);
    assert_eq!(out.swapped, vec![GroupId(1)]);
    assert!(s.front_layers(SurfaceId(1)).is_empty());
    assert!(gpu.tex.is_empty());
}

#[test]
fn a_surface_empty_at_its_first_paint_paints_later_content() {
    let mut src = Source::default();
    let mut s = Scheduler::new(limits());
    let mut gpu = Gpu::default();
    let mut t = 0;
    s.create_surface(SurfaceId(1), GroupId(1), Duration::ZERO)
        .unwrap();
    run_idle(&mut s, &mut src, &mut gpu, &mut t, &[1], 10);
    assert!(s.is_drawn(SurfaceId(1)));
    assert!(s.front_layers(SurfaceId(1)).is_empty());
    src.set(1, [16, 16], vec![fill(Rect::of_size(16, 16), 0xFF00_0077)]);
    s.damage_all(SurfaceId(1));
    run_idle(&mut s, &mut src, &mut gpu, &mut t, &[1], 10);
    assert_eq!(gpu.front(1, Layer::Below, 1, 1), 0xFF00_0077);
}

#[test]
fn an_invalid_snapshot_size_does_not_stall_the_generation() {
    // Finding: a canvas that could not be made left a generation member Idle, so the
    // group's generation never swapped.
    let (mut s, mut src, mut gpu, mut t) = shown_surface(limits());
    src.set(1, [20_000, 16], vec![fill(Rect::of_size(64, 16), 1)]);
    s.damage_all(SurfaceId(1));
    s.layout_change(GroupId(1), "g2", &[SurfaceId(1)]);
    run_idle(&mut s, &mut src, &mut gpu, &mut t, &[1], 20);
    assert_eq!(s.shown(GroupId(1)), Some(&"g2"));
    assert!(s.counters().invalid_snapshots >= 1);
    assert!(s.front_layers(SurfaceId(1)).is_empty(), "painted as empty");
    // A zero size is refused the same way, also for a first paint.
    src.set(2, [0, 16], Vec::new());
    s.create_surface(SurfaceId(2), GroupId(1), ms(t)).unwrap();
    s.layout_change(GroupId(1), "g3", &[SurfaceId(2)]);
    run_idle(&mut s, &mut src, &mut gpu, &mut t, &[1, 2], 20);
    assert_eq!(s.shown(GroupId(1)), Some(&"g3"));
    assert!(s.is_drawn(SurfaceId(2)));
    // Valid content afterwards is painted.
    src.set(1, [16, 16], vec![fill(Rect::of_size(16, 16), 0xFF00_0099)]);
    s.damage_all(SurfaceId(1));
    run_idle(&mut s, &mut src, &mut gpu, &mut t, &[1, 2], 20);
    assert_eq!(gpu.front(1, Layer::Below, 0, 0), 0xFF00_0099);
}

#[test]
fn sparse_first_paint_texture_is_shrunk_once() {
    // Finding: a first paint's texture covered the rest of the canvas from its first band
    // with content and was never shrunk (render.md §6.12 "Shrink once").
    let mut src = Source::default();
    src.set(
        1,
        [3840, 2160],
        vec![fill(Rect::new(0, 0, 10, 10), 0xFF12_3456)],
    );
    let mut s = Scheduler::<&str>::new(limits());
    let mut gpu = Gpu::default();
    s.create_surface(SurfaceId(1), GroupId(1), Duration::ZERO)
        .unwrap();
    let out = s.frame(Duration::ZERO, &mut src, &mut Unlimited);
    gpu.apply(&out.ops);
    let creates = out
        .ops
        .iter()
        .filter(|o| matches!(o, GpuOp::Create { .. }))
        .count();
    assert_eq!(creates, 2, "created once, plus one shrink");
    let alloc = s.canvases(SurfaceId(1)).unwrap().below().allocated_box();
    assert_eq!(alloc, Rect::new(0, 0, 256, 256));
    assert_eq!(s.front_box(SurfaceId(1), Layer::Below), Some(alloc));
    assert_eq!(s.texture_bytes(), 256 * 256 * 4);
    assert_eq!(gpu.tex.len(), 1);
    assert_eq!(gpu.front(1, Layer::Below, 5, 5), 0xFF12_3456);
    assert_eq!(gpu.front(1, Layer::Below, 100, 100), 0);
    assert!(s.is_drawn(SurfaceId(1)));
}

#[test]
fn shrink_after_a_released_banded_first_paint_keeps_the_uploaded_bands() {
    let mut src = Source::default();
    src.set(
        1,
        [64, 600],
        vec![fill(Rect::new(0, 0, 64, 20), 0xFF00_4400)],
    );
    let mut s = Scheduler::<&str>::new(limits());
    let mut gpu = Gpu::default();
    s.create_surface(SurfaceId(1), GroupId(1), Duration::ZERO)
        .unwrap();
    let mut t = 0;
    while s.is_busy(SurfaceId(1)) || !s.is_drawn(SurfaceId(1)) {
        let out = s.frame(ms(t), &mut src, &mut BandBudget(4));
        gpu.apply(&out.ops);
        t += 40;
        assert!(t < 2000);
    }
    assert_eq!(
        s.front_box(SurfaceId(1), Layer::Below),
        Some(Rect::new(0, 0, 64, 256))
    );
    assert_front_is_canvas(&gpu, &s, 1);
}

#[test]
fn generation_uploads_take_the_cap_before_content_jobs() {
    // Finding: content jobs of other groups used the frame's cap before the held
    // rectangles of a generation were checked, so the generation migrated to back
    // textures instead of swapping in the frame.
    let lim = PaintLimits {
        upload_cap_bytes: 64 * 64 * 4,
        ..limits()
    };
    let (mut s, mut src, mut gpu, mut t) = shown_surface(lim);
    src.set(2, [64, 64], vec![fill(Rect::of_size(64, 64), 0xFF00_0002)]);
    s.create_surface(SurfaceId(2), GroupId(2), ms(t)).unwrap();
    s.layout_change(GroupId(2), "h1", &[SurfaceId(2)]);
    run_idle(&mut s, &mut src, &mut gpu, &mut t, &[1, 2], 20);
    // Group 1: a same-size generation with full damage (holds 16 KiB). Group 2: a full
    // content job of 16 KiB, in the same frame.
    src.set(1, [64, 64], vec![fill(Rect::of_size(64, 64), 0xFF00_1111)]);
    s.damage_all(SurfaceId(1));
    s.layout_change(GroupId(1), "g2", &[SurfaceId(1)]);
    src.set(2, [64, 64], vec![fill(Rect::of_size(64, 64), 0xFF00_2222)]);
    s.damage_all(SurfaceId(2));
    let out = s.frame(ms(t), &mut src, &mut Unlimited);
    gpu.apply(&out.ops);
    assert_eq!(
        out.swapped,
        vec![GroupId(1)],
        "the generation swaps in frame 1"
    );
    assert_eq!(gpu.front(1, Layer::Below, 1, 1), 0xFF00_1111);
    assert!(out.stats.upload_bytes <= 64 * 64 * 4);
    t += 16;
    run_idle(&mut s, &mut src, &mut gpu, &mut t, &[1, 2], 20);
    assert_eq!(gpu.front(2, Layer::Below, 1, 1), 0xFF00_2222);
}

#[test]
fn canvas_memory_budget_holds_while_painting() {
    // Finding: canvases above were allocated without bound while painting.
    let lim = PaintLimits {
        canvas_bytes: 4 * 256 * 256 * 4, // four full tiles
        ..limits()
    };
    let mut ops = Vec::new();
    for e in 0..40 {
        ops.push(PaintOp::Nested {
            entry: e,
            coverage: Rect::of_size(256, 256),
        });
        ops.push(fill(Rect::of_size(256, 256), 0xFF00_0000 | e));
    }
    let mut src = Source::default();
    src.set(1, [256, 256], ops);
    let mut s = Scheduler::<&str>::new(lim);
    let mut gpu = Gpu::default();
    s.create_surface(SurfaceId(1), GroupId(1), Duration::ZERO)
        .unwrap();
    let mut t = 0;
    run_idle(&mut s, &mut src, &mut gpu, &mut t, &[1], 40);
    assert!(s.canvas_bytes() <= lim.canvas_bytes, "{}", s.canvas_bytes());
    assert_eq!(s.counters().memory, 1, "counted once per job");
    assert!(s.texture_bytes() <= lim.texture_bytes);
}

#[test]
fn refused_first_paint_texture_is_counted_once() {
    let lim = PaintLimits {
        texture_bytes: 1024 * 1024,
        ..limits()
    };
    let mut src = Source::default();
    src.set(
        1,
        [1024, 1024],
        vec![fill(Rect::of_size(1024, 1024), 0xFFFF_FFFF)],
    );
    let mut s = Scheduler::<&str>::new(lim);
    s.create_surface(SurfaceId(1), GroupId(1), Duration::ZERO)
        .unwrap();
    s.frame(Duration::ZERO, &mut src, &mut Unlimited);
    assert_eq!(
        s.counters().memory,
        1,
        "one refused texture, not one per band"
    );
    assert!(s.front_layers(SurfaceId(1)).is_empty());
}
