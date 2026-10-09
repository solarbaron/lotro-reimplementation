// SPDX-License-Identifier: GPL-3.0-or-later
//! The walker's walk mode on collision (`apps/client/src/walker/movement`) on synthetic
//! worlds built by our own code: landblocks of grid terrain with boxes, steps, a steep
//! ramp and water. Scripted walks of every kind the runner checks (`m4-5-visual.md` §2.2),
//! the fixed 1/60 s accumulator against the frame rate, waiting for missing collision sets,
//! the solid boundary of absent landblocks, and the soak's generator, planner, script and
//! fall-through check (§2.5).

use std::sync::Arc;

use tessera_formats::landblock::{HeightGrid, height_from_raw};
use tessera_formats::setup::Quaternion;
use tessera_formats::setup::{CollisionBox, Frame};
use tessera_formats::{LandblockData, LandblockId};
use tessera_physics::controller::WaterQuery as _;
use tessera_physics::geometry::{
    BodyPurpose, BodySource, CellPlacement, CellVolume, CollisionFlags, CollisionLimits,
    CollisionSet, CollisionSetBuilder, CollisionView, GridMesh, LiquidBox, LiquidFlags, PlacedCell,
    ShapeTemplate, Transform, WaterPlane,
};
use tessera_world::{LandblockKey, WorldPos};

use tessera_client::walk_path::{Expect, parse_walk_file};
use tessera_client::walker::movement::run::{
    LOG_HEADER, StaticWorld, compass_degrees, evaluate, run_walk, terrain_allowance, write_log,
    yaw_of,
};
use tessera_client::walker::movement::setup::MoverSetup;
use tessera_client::walker::movement::soak::{
    SOAK_SEED, SegmentKind, SoakPlanner, SoakScript, SplitMix64, StartCheck, find_start,
    run_soak_walk,
};
use tessera_client::walker::movement::{
    BlockReadiness, CellMaskedWater, CollisionWorld, ContactKind, FrameSteps, MAX_STEPS_PER_FRAME,
    MoveInput, MovementState, PhysicsWalker, STEP_SECONDS,
};

use tessera_client::wait::{LoadWait, ProgressWait};

/// The wait for missing sets: a static world loads nothing, so a set that is missing stays
/// missing and the wait gives up after the stall limit.
const SYNTH_WAIT: LoadWait = LoadWait::Progress(ProgressWait::new(1_000, 10_000));

const REGION: u8 = 1;
const BX: u8 = 100;
const BY: u8 = 100;

fn key(dx: i32, dy: i32) -> LandblockKey {
    LandblockKey::new(REGION, BX, BY)
        .unwrap()
        .offset(dx, dy)
        .unwrap()
}

/// Raw height sample for `z` metres (`landblock.md` §3.4).
fn raw(z: f32) -> u16 {
    (z * 65_535.0 / 2_000.0).round() as u16
}

/// A landblock set with terrain heights `h(x, y)` (metres, region-frame x and y, so
/// neighbouring landblocks meet without seams) and extra shapes.
fn block(
    k: LandblockKey,
    h: impl Fn(f64, f64) -> f32,
    extra: impl FnOnce(&mut CollisionSetBuilder),
) -> Arc<CollisionSet> {
    let o = k.origin();
    let grid = HeightGrid::from_fn(|i, j| raw(h(o[0] + 5.0 * i as f64, o[1] + 5.0 * j as f64)));
    let data = LandblockData::from_heights(k.data_did(), grid);
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.set_landblock(LandblockId::from_did(k.data_did()).unwrap());
    b.add_grid(Arc::new(GridMesh::terrain(&data))).unwrap();
    extra(&mut b);
    Arc::new(b.build())
}

fn solid_box(
    b: &mut CollisionSetBuilder,
    tag: u32,
    min: [f32; 3],
    max: [f32; 3],
    source: Option<BodySource>,
) {
    let mut t = ShapeTemplate::default();
    t.push_box(&CollisionBox {
        min,
        max,
        frame: Frame::IDENTITY,
    })
    .unwrap();
    b.add_shape(
        source.unwrap_or(BodySource::Other { tag }),
        CollisionFlags::default(),
        BodyPurpose::Movement,
        &t,
    )
    .unwrap();
}

/// Flat ground at `z0` on the 3 × 3 landblocks around the centre one, `extra` on the centre.
fn flat_world(z0: f32, extra: impl FnOnce(&mut CollisionSetBuilder)) -> StaticWorld {
    let mut w = StaticWorld::default();
    let mut extra = Some(extra);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let k = key(dx, dy);
            let set = if (dx, dy) == (0, 0) {
                let f = extra.take().unwrap();
                block(k, |_, _| z0, f)
            } else {
                block(k, |_, _| z0, |_| {})
            };
            w.sets.insert(k, set);
        }
    }
    w
}

/// The flat height the raw rounding gives for `z`.
fn ground(z: f32) -> f64 {
    f64::from(height_from_raw(raw(z)))
}

fn walk_text(
    expect: &str,
    inputs: &str,
    start: [f64; 3],
    heading: f64,
    mode: &str,
    duration: f64,
) -> String {
    format!(
        "id = T1\ncategory = wall\nregion = {REGION}\nlandblock = 0x{BX:02X}{BY:02X}\ncell = 0\n\
         start = {},{},{}\nloc = r1 synthetic\nheading = {heading}\nmode = {mode}\n\
         inputs = {inputs}\nduration = {duration}\nexpect = {expect}\n",
        start[0], start[1], start[2]
    )
}

fn setup() -> MoverSetup {
    MoverSetup::builtin()
}

#[test]
fn blocked_by_a_wall_without_penetration() {
    let z = ground(100.0);
    #[allow(clippy::cast_possible_truncation)]
    let zf = z as f32;
    let mut world = flat_world(100.0, |b| {
        solid_box(b, 1, [60.0, 90.0, zf - 1.0], [100.0, 91.0, zf + 5.0], None);
    });
    // Feet stop one radius (0.35) and two skins before the wall at y = 90.
    let p = [80.0, 90.0 - 0.35, z];
    let text = walk_text(
        &format!("blocked P={},{},{} d=0.10", p[0], p[1], p[2]),
        "0:fwd",
        [80.0, 84.0, z],
        0.0,
        "run",
        3.0,
    );
    let walk = parse_walk_file(&text).unwrap();
    let out = run_walk(&walk, &mut world, &setup(), SYNTH_WAIT).unwrap();
    assert!(out.verdict.passed, "{:?}", out.verdict);
    assert!(out.verdict.max_penetration <= 0.01);
    let end = out.rows.last().unwrap();
    assert!(end.local[1] < 90.0 - 0.34, "{:?}", end.local);
    assert!(
        end.record
            .contacts
            .iter()
            .any(|c| c.kind == ContactKind::Other)
    );
    // 0.5 s settle + 3 s at 60 steps per second.
    assert_eq!(out.rows.len(), 30 + 180);
    let mut csv = Vec::new();
    write_log(&mut csv, &out).unwrap();
    let csv = String::from_utf8(csv).unwrap();
    assert_eq!(csv.lines().count(), out.rows.len());
    assert_eq!(
        csv.lines().next().unwrap().split(',').count(),
        LOG_HEADER.split(',').count()
    );
    // Asking for a stop point too far on fails.
    let mut far = walk.clone();
    far.expect = Expect::Blocked {
        p: [80.0, 95.0, z],
        d: 0.1,
    };
    assert!(!evaluate(&far, &out.rows, key(0, 0), setup().shape.radius()).passed);
}

#[test]
fn reach_up_a_walkable_ramp_and_release_at_the_waypoint() {
    let z = ground(100.0);
    #[allow(clippy::cast_possible_truncation)]
    let zf = z as f32;
    // A 20° ramp rising north from y = 80 to y = 90, then a landing.
    let rise = 10.0 * 20f32.to_radians().tan();
    let mut world = flat_world(100.0, |b| {
        let t = ShapeTemplate::from_triangle_list(
            &[
                [70.0, 80.0, zf],
                [90.0, 80.0, zf],
                [90.0, 90.0, zf + rise],
                [70.0, 90.0, zf + rise],
                [70.0, 100.0, zf + rise],
                [90.0, 100.0, zf + rise],
            ],
            &[[0, 1, 2], [0, 2, 3], [3, 2, 5], [3, 5, 4]],
        )
        .unwrap();
        b.add_shape(
            BodySource::Other { tag: 4 },
            CollisionFlags::default(),
            BodyPurpose::Movement,
            &t,
        )
        .unwrap();
    });
    let e = [80.0, 94.0, z + f64::from(rise)];
    let text = walk_text(
        &format!("reach E={},{},{} d=0.15 risers=0", e[0], e[1], e[2]),
        &format!("wp=80,85;{},{}", e[0], e[1]),
        [80.0, 76.0, z],
        0.0,
        "walk",
        // Walk mode asks for 2.5 m/s (`collision.md` §11.8); uphill the controller keeps
        // less of it.
        16.0,
    );
    let walk = parse_walk_file(&text).unwrap();
    let out = run_walk(&walk, &mut world, &setup(), SYNTH_WAIT).unwrap();
    assert!(out.verdict.passed, "{:?}", out.verdict);
    // Walk mode selects the Walking layer from the first step with input on (the style
    // keeps its value while no move key is held, §11.8 item 6).
    assert!(
        out.rows
            .iter()
            .filter(|r| r.t >= 0.0)
            .all(|r| r.record.walking)
    );
    // Forward is released near the last waypoint.
    assert!(
        out.rows
            .iter()
            .any(|r| r.t >= 0.0 && r.record.input.forward == 0.0)
    );
}

#[test]
#[ignore = "tessera-physics: after the second step-up the capsule rests on the top edge of the upper box and every later step is refused (the edge contact is classified by the box's vertical face, an order-dependent tie; reported to the physics owner)"]
fn reach_climbs_box_steps() {
    let z = ground(100.0);
    #[allow(clippy::cast_possible_truncation)]
    let zf = z as f32;
    // Two steps of 0.25 m and 0.15 m across the path north of y = 80 (solid boxes, each
    // running to the end of the flight).
    let mut world = flat_world(100.0, |b| {
        solid_box(b, 1, [70.0, 80.0, zf - 2.0], [90.0, 95.0, zf + 0.25], None);
        solid_box(b, 2, [70.0, 82.0, zf - 2.0], [90.0, 95.0, zf + 0.40], None);
    });
    let e = [80.0, 86.0, z + 0.40];
    let text = walk_text(
        &format!("reach E={},{},{} d=0.15 risers=0.25/0.15", e[0], e[1], e[2]),
        &format!("wp={},{}", e[0], e[1]),
        [80.0, 76.0, z],
        0.0,
        "walk",
        6.0,
    );
    let walk = parse_walk_file(&text).unwrap();
    let out = run_walk(&walk, &mut world, &setup(), SYNTH_WAIT).unwrap();
    assert!(out.verdict.passed, "{:?}", out.verdict);
    assert!(out.rows.iter().any(|r| r.record.report.stepped_up));
    assert!(
        out.rows
            .iter()
            .filter(|r| r.t >= 0.0)
            .all(|r| r.record.walking)
    );
}

#[test]
fn slides_on_a_steep_slope() {
    let z = ground(100.0);
    // A 60° ramp rising north from y = 85.
    let mut world = flat_world(100.0, |b| {
        #[allow(clippy::cast_possible_truncation)]
        let zf = z as f32;
        let rise = 10.0 * 60f32.to_radians().tan();
        let t = ShapeTemplate::from_triangle_list(
            &[
                [70.0, 85.0, zf],
                [90.0, 85.0, zf],
                [90.0, 95.0, zf + rise],
                [70.0, 95.0, zf + rise],
            ],
            &[[0, 1, 2], [0, 2, 3]],
        )
        .unwrap();
        b.add_shape(
            BodySource::Other { tag: 9 },
            CollisionFlags::default(),
            BodyPurpose::Movement,
            &t,
        )
        .unwrap();
    });
    // Standing on the 60° ramp 3 m up its foot, pushing uphill: it slides down (state 6) and
    // never climbs. (From flat ground the controller meets such a ramp as a wall.)
    let plane = z + 3.0 * 60f64.to_radians().tan();
    let feet = plane + 0.35;
    let text = walk_text(
        &format!("noclimb B=80,88,{plane} zmax={} rmax=0.6", feet + 0.6),
        "0:fwd",
        [80.0, 88.0, feet],
        0.0,
        "run",
        4.0,
    );
    let walk = parse_walk_file(&text).unwrap();
    let out = run_walk(&walk, &mut world, &setup(), SYNTH_WAIT).unwrap();
    assert!(out.verdict.passed, "{:?}", out.verdict);
    let end = out.rows.last().unwrap();
    assert!(end.local[1] < 85.5, "slid to the foot: {:?}", end.local);
}

#[test]
fn climbs_triangle_risers() {
    let z = ground(100.0);
    #[allow(clippy::cast_possible_truncation)]
    let zf = z as f32;
    let mut world = flat_world(100.0, |b| {
        let (x0, x1, z1) = (70.0, 90.0, zf + 0.25);
        let t = ShapeTemplate::from_triangle_list(
            &[
                [x0, 80.0, zf],
                [x1, 80.0, zf],
                [x1, 80.0, z1],
                [x0, 80.0, z1],
                [x0, 95.0, z1],
                [x1, 95.0, z1],
            ],
            &[[0, 1, 2], [0, 2, 3], [3, 2, 5], [3, 5, 4]],
        )
        .unwrap();
        b.add_shape(
            BodySource::Other { tag: 3 },
            CollisionFlags::default(),
            BodyPurpose::Movement,
            &t,
        )
        .unwrap();
    });
    let e = [80.0, 86.0, f64::from(zf + 0.25)];
    let text = walk_text(
        &format!("reach E={},{},{} d=0.15 risers=0.25", e[0], e[1], e[2]),
        &format!("wp={},{}", e[0], e[1]),
        [80.0, 76.0, z],
        0.0,
        "walk",
        6.0,
    );
    let walk = parse_walk_file(&text).unwrap();
    let out = run_walk(&walk, &mut world, &setup(), SYNTH_WAIT).unwrap();
    assert!(out.verdict.passed, "{:?}", out.verdict);
}

#[test]
fn wades_then_swims_at_the_float_line() {
    // Ground sloping down north, 1 m in 8 (100 m at the centre landblock's south edge);
    // water at 95 m.
    let h = |_: f64, y: f64| (100.0 - (y - 16_000.0) / 8.0) as f32;
    let level = 95.0;
    let mut world = StaticWorld::default();
    for dy in -1..=1 {
        for dx in -1..=1 {
            let k = key(dx, dy);
            let set = block(k, h, |b| {
                b.set_water_plane(Some(WaterPlane::new(level, 1).unwrap()));
                let liquid = LiquidFlags {
                    liquid: true,
                    deep: true,
                    ..LiquidFlags::default()
                };
                b.add_liquid_box(
                    LiquidBox::new(
                        [0.0, 0.0, 0.0],
                        [160.0, 160.0, level],
                        &Transform::IDENTITY,
                        liquid,
                    )
                    .unwrap(),
                )
                .unwrap();
            });
            world.sets.insert(k, set);
        }
    }
    // Terrain height under y: 100 − y / 8.
    let y_at = |z: f64| (100.0 - z) * 8.0;
    let fh = f64::from(setup().record.float_height(&setup().conditions));
    let entry = y_at(f64::from(level));
    let swim = y_at(f64::from(level) - fh);
    let start_y = 20.0;
    let start_z = f64::from(
        world
            .terrain_height(&WorldPos::new(
                REGION,
                16_000.0 + 80.0,
                16_000.0 + start_y,
                0.0,
            ))
            .unwrap(),
    );
    let text = walk_text(
        &format!("wade_swim in=80,{entry} swim=80,{swim} h={level} fh={fh} d=0.5"),
        "wp=80,150",
        [80.0, start_y, start_z],
        0.0,
        "run",
        25.0,
    );
    let walk = parse_walk_file(&text).unwrap();
    let out = run_walk(&walk, &mut world, &setup(), SYNTH_WAIT).unwrap();
    assert!(out.verdict.passed, "{:?}", out.verdict);
    assert!(
        out.rows
            .iter()
            .any(|r| r.record.state.element == MovementState::WaterControlledSurface)
    );
}

#[test]
fn fixed_step_is_independent_of_the_frame_rate() {
    let world = flat_world(100.0, |_| {});
    let start = [16_080.0, 16_080.0, ground(100.0) + 0.01];
    let input = MoveInput {
        forward: 1.0,
        ..MoveInput::default()
    };
    let mut ends = Vec::new();
    for fps in [30.0, 60.0, 120.0, 144.0] {
        let mut w = PhysicsWalker::new(&setup(), REGION, start, 0.3);
        let frames = (3.0f64 * fps).round() as usize;
        let mut steps = 0;
        for _ in 0..frames {
            steps += w.frame(1.0 / fps, &input, 0.0, false, &world).steps;
        }
        // Exactly 180 steps of 1/60 s for 3 s of frames (rounding may leave one behind).
        assert!((179..=180).contains(&steps), "{fps}: {steps}");
        ends.push((steps, w.state().position));
    }
    // Same number of steps, same input: the same position bit for bit.
    for (s, p) in &ends {
        for (s2, p2) in &ends {
            if s == s2 {
                assert_eq!(p, p2);
            }
        }
    }
    // A stall drops time beyond the cap of 5 steps.
    let mut w = PhysicsWalker::new(&setup(), REGION, start, 0.3);
    assert_eq!(
        w.frame(1.0, &input, 0.0, false, &world).steps,
        MAX_STEPS_PER_FRAME
    );
    assert!(
        w.frame(0.0, &input, 0.0, false, &world).steps <= 1,
        "at most one step of the stalled second is kept (not 55)"
    );
}

/// A world whose centre landblock is still loading.
struct Loading(StaticWorld);

impl CollisionWorld for Loading {
    fn readiness(&self, key: LandblockKey) -> BlockReadiness {
        if key == self::key(0, 0) {
            BlockReadiness::Pending
        } else {
            self.0.readiness(key)
        }
    }
    fn view(&self, centre: &WorldPos) -> tessera_physics::geometry::CollisionView<'_> {
        self.0.view(centre)
    }
    fn terrain_height(&self, pos: &WorldPos) -> Option<f32> {
        self.0.terrain_height(pos)
    }
}

#[test]
fn waits_for_missing_sets_and_keeps_the_jump() {
    let inner = flat_world(100.0, |_| {});
    let start = [16_080.0, 16_080.0, ground(100.0) + 0.01];
    let mut w = PhysicsWalker::new(&setup(), REGION, start, 0.0);
    // Settle first.
    for _ in 0..30 {
        w.step(&MoveInput::default(), false, &inner);
    }
    let before = *w.state();
    let loading = Loading(inner.clone());
    let jump = MoveInput {
        jump: true,
        ..MoveInput::default()
    };
    let f = w.frame(0.1, &jump, 0.0, false, &loading);
    assert_eq!(
        f,
        FrameSteps {
            steps: 0,
            waiting: true
        }
    );
    assert_eq!(w.state(), &before, "nothing changes while waiting");
    assert_eq!(w.pending(&loading), vec![key(0, 0)]);
    // Once installed, the latched jump is taken by the first step.
    let f = w.frame(1.0 / 60.0, &MoveInput::default(), 0.0, false, &inner);
    assert_eq!(f.steps, 1);
    assert!(w.last().unwrap().report.jumped);
}

#[test]
fn absent_landblocks_are_a_solid_boundary() {
    // Only the centre landblock exists; walk east into the absent neighbour.
    let mut world = StaticWorld::default();
    world
        .sets
        .insert(key(0, 0), block(key(0, 0), |_, _| 100.0, |_| {}));
    let start = [16_000.0 + 155.0, 16_080.0, ground(100.0) + 0.01];
    let mut w = PhysicsWalker::new(&setup(), REGION, start, 0.0);
    let east = MoveInput {
        forward: 1.0,
        heading: Some(0.0),
        ..MoveInput::default()
    };
    let mut held = 0;
    for _ in 0..240 {
        if w.step(&east, false, &world).is_some_and(|r| r.at_boundary) {
            held += 1;
        }
    }
    let x = w.state().position[0];
    assert!(held > 0);
    assert!(x <= 16_160.0 - 0.35 + 1e-6, "{x}");
    assert!(x > 16_158.0, "{x}");
}

#[test]
fn splitmix64_reference() {
    // The generator's published reference output for seed 0.
    let mut r = SplitMix64::new(0);
    assert_eq!(r.next_u64(), 0xE220_A839_7B1D_CDAF);
    assert_eq!(r.next_u64(), 0x6E78_9E6A_A1B9_65F4);
    let mut s = SplitMix64::new(SOAK_SEED);
    for _ in 0..1000 {
        let u = s.uniform();
        assert!((0.0..1.0).contains(&u));
    }
    assert_eq!(SOAK_SEED.to_be_bytes(), *b"M45-SOAK");
}

#[test]
fn soak_plan_and_script() {
    let lists = vec![
        (1u8, Arc::new(vec![key(0, 0)])),
        (2u8, Arc::new(vec![key(0, 0), key(1, 0)])),
    ];
    let mut p = SoakPlanner::new(SOAK_SEED, lists.clone());
    let mut checks = 0;
    let a = p
        .plan(|_, _| {
            checks += 1;
            if checks < 3 {
                StartCheck::Overlap
            } else {
                StartCheck::Feet(10.0)
            }
        })
        .unwrap();
    assert_eq!(a.start_draws, 3);
    assert_eq!(a.start_z, Some(10.0));
    assert!(a.segments.iter().map(|s| s.seconds).sum::<f64>() >= 60.0);
    assert!(
        a.segments
            .iter()
            .all(|s| (2.0..6.0).contains(&s.seconds) && (-90.0..90.0).contains(&s.turn))
    );
    // The same seed and the same start answers give the same plans.
    let mut q = SoakPlanner::new(SOAK_SEED, lists);
    let mut checks = 0;
    let b = q
        .plan(|_, _| {
            checks += 1;
            if checks < 3 {
                StartCheck::Overlap
            } else {
                StartCheck::Feet(10.0)
            }
        })
        .unwrap();
    assert_eq!(a, b);
    assert_eq!(p.peek_block(), q.peek_block());
    let c = p.plan(|_, _| StartCheck::Feet(1.0)).unwrap();
    assert_eq!(c.index, 1);
    // The script: segment kinds become inputs, jumps only at a segment start.
    let mut s = SoakScript::new(&a);
    let mut t = 0.0;
    let mut jumps = 0;
    while t < 60.0 {
        let c = s.command(t);
        jumps += usize::from(c.input.jump);
        assert!(c.input.heading.is_some_and(f32::is_finite));
        t += f64::from(STEP_SECONDS);
    }
    // One press at the start of every run-and-jump segment that starts within the walk.
    let mut begin = 0.0;
    let mut jump_segments = 0;
    for seg in &a.segments {
        if seg.kind == SegmentKind::RunJump && begin < 60.0 {
            jump_segments += 1;
        }
        begin += seg.seconds;
    }
    assert_eq!(jumps, jump_segments);
    assert_eq!(SegmentKind::from_draw(0.49), SegmentKind::Run);
    assert_eq!(SegmentKind::from_draw(0.96), SegmentKind::Stand);
}

#[test]
fn soak_walks_on_hills_never_fall_through() {
    // Rolling hills on 3 × 3 landblocks, with boxes to bump into.
    let h = |x: f64, y: f64| {
        let (x, y) = ((x - 16_000.0) as f32, (y - 16_000.0) as f32);
        120.0 + 6.0 * (x / 23.0).sin() * (y / 31.0).cos() + 0.02 * x
    };
    let mut world = StaticWorld::default();
    for dy in -1..=1 {
        for dx in -1..=1 {
            let k = key(dx, dy);
            world.sets.insert(
                k,
                block(k, h, |b| {
                    solid_box(
                        b,
                        1,
                        [40.0, 40.0, 100.0],
                        [44.0, 80.0, 140.0],
                        Some(BodySource::Static {
                            index: 0,
                            object: tessera_formats::Did(0x4700_0001),
                        }),
                    );
                }),
            );
        }
    }
    let lists = vec![(REGION, Arc::new(vec![key(0, 0)]))];
    let mut planner = SoakPlanner::new(SOAK_SEED, lists);
    let s = setup();
    for _ in 0..6 {
        let walk = planner
            .plan(|k, xy| find_start(&world, &s.shape, k, xy))
            .unwrap();
        assert!(walk.start_z.is_some());
        let out = run_soak_walk(&walk, &mut world, &s, SYNTH_WAIT);
        assert!(out.failures.is_empty(), "{:?}", out.failures);
        assert!(out.skipped.is_none(), "{:?}", out.skipped);
        assert_eq!(out.steps, 3_600);
    }
    // A start inside the box overlaps; one on open ground does not.
    assert_eq!(
        find_start(&world, &s.shape, key(0, 0), [42.0, 60.0]),
        StartCheck::Overlap
    );
    assert!(matches!(
        find_start(&world, &s.shape, key(0, 0), [100.0, 100.0]),
        StartCheck::Feet(_)
    ));
}

#[test]
fn yaw_of_compass_headings() {
    use std::f32::consts::{FRAC_PI_2, PI};
    assert!((yaw_of(0.0) - FRAC_PI_2).abs() < 1e-6, "north is +Y");
    assert!(yaw_of(90.0).abs() < 1e-6, "east is +X");
    assert!((yaw_of(180.0) + FRAC_PI_2).abs() < 1e-6);
    assert!((yaw_of(270.0).abs() - PI).abs() < 1e-6);
    assert_eq!(yaw_of(f64::NAN), 0.0);
}

/// An axis-aligned room: cell `index` whose volume spans `half` either side of `centre`
/// (landblock frame, the floor's middle) and `height` up, with a floor slab 0.5 m thick
/// under it spanning x from −half to `floor_east` (cell frame) when `floor_east` is given.
fn add_room(
    b: &mut CollisionSetBuilder,
    index: u16,
    centre: [f32; 3],
    half: [f32; 2],
    height: f32,
    floor_east: Option<f32>,
) {
    let planes = [
        ([1.0, 0.0, 0.0], half[0]),
        ([-1.0, 0.0, 0.0], half[0]),
        ([0.0, 1.0, 0.0], half[1]),
        ([0.0, -1.0, 0.0], half[1]),
        ([0.0, 0.0, 1.0], 0.0),
        ([0.0, 0.0, -1.0], height),
    ];
    let volume = Arc::new(CellVolume::new(planes).unwrap());
    let mut hull = Vec::new();
    for x in [-half[0], half[0]] {
        for y in [-half[1], half[1]] {
            for z in [0.0, height] {
                hull.push([x, y, z]);
            }
        }
    }
    let placement = CellPlacement::new(centre, Quaternion::IDENTITY).unwrap();
    let cell = PlacedCell::with_volume(
        index,
        tessera_formats::Did(0x0D00_0000 | u32::from(index)),
        volume,
        &hull,
        placement,
    )
    .unwrap();
    let floor = floor_east.map(|east| {
        let mut t = ShapeTemplate::default();
        t.push_box(&CollisionBox {
            min: [-half[0], -half[1], -0.5],
            max: [east, half[1], 0.0],
            frame: Frame::IDENTITY,
        })
        .unwrap();
        t
    });
    b.add_cell(cell, floor.as_ref()).unwrap();
}

/// A set of `owner` holding only `extra` (no terrain).
fn bare_set(
    owner: LandblockKey,
    extra: impl FnOnce(&mut CollisionSetBuilder),
) -> Arc<CollisionSet> {
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.set_landblock(LandblockId::from_did(owner.data_did()).unwrap());
    extra(&mut b);
    Arc::new(b.build())
}

/// Two rooms side by side on the centre landblock, 0.5 m above flat ground at 100 m: cell
/// 1 west of x = 80, cell 2 east of it, one floor slab under both (owned by cell 1), and
/// portals between them.
fn two_rooms() -> (StaticWorld, f64) {
    let z0 = ground(100.0) + 0.5;
    #[allow(clippy::cast_possible_truncation)]
    let zf = z0 as f32;
    let mut world = flat_world(100.0, |b| {
        add_room(b, 1, [75.0, 80.0, zf], [5.0, 5.0], 5.0, Some(15.0));
        add_room(b, 2, [85.0, 80.0, zf], [5.0, 5.0], 5.0, None);
    });
    world.portals.insert((key(0, 0), 1), vec![2]);
    world.portals.insert((key(0, 0), 2), vec![1]);
    (world, z0)
}

#[test]
fn cells_walk_through_a_doorway() {
    let (mut world, z0) = two_rooms();
    for row_cell in [1, 0] {
        let text = walk_text(
            &format!("cells 1>2 E=85,80,{z0} d=0.15"),
            "wp=85,80",
            [75.0, 80.0, z0],
            90.0,
            "walk",
            // 10 m at the walk-mode 2.5 m/s (`collision.md` §11.8).
            6.0,
        )
        .replace("cell = 0", &format!("cell = {row_cell}"));
        let walk = parse_walk_file(&text).unwrap();
        assert_eq!(walk.cell, row_cell);
        let out = run_walk(&walk, &mut world, &setup(), SYNTH_WAIT).unwrap();
        assert!(out.verdict.passed, "{row_cell}: {:?}", out.verdict);
        // The start cell is the one after the settle.
        let settled = out.rows.iter().rfind(|r| r.t < 0.0).unwrap();
        assert_eq!(settled.record.cell, Some((key(0, 0), 1)));
        assert_eq!(out.rows.last().unwrap().record.cell, Some((key(0, 0), 2)));
        // A row whose cell differs from the one after the settle is noted.
        let noted = out
            .verdict
            .notes
            .iter()
            .any(|n| n.contains("the row's cell 0"));
        assert_eq!(noted, row_cell == 0, "{:?}", out.verdict.notes);
    }
    // Expecting another start cell fails.
    let text = walk_text(
        &format!("cells 2>2 E=85,80,{z0} d=0.15"),
        "wp=85,80",
        [75.0, 80.0, z0],
        90.0,
        "walk",
        4.0,
    );
    let walk = parse_walk_file(&text).unwrap();
    let out = run_walk(&walk, &mut world, &setup(), SYNTH_WAIT).unwrap();
    assert!(!out.verdict.passed);
    assert!(
        out.verdict
            .failures
            .iter()
            .any(|f| f.contains("start cell 1")),
        "{:?}",
        out.verdict
    );
}

#[test]
fn the_tracker_keeps_the_current_cell_in_a_doorway() {
    let (world, z0) = two_rooms();
    // In the doorway (x = 80) both rooms contain the feet: the walker keeps the cell it
    // came from, whichever it is.
    for (from, x) in [(1u16, 78.0), (2u16, 82.0)] {
        let mut w = PhysicsWalker::new(&setup(), REGION, [16_000.0 + x, 16_080.0, z0], 0.0);
        w.set_cell(Some((key(0, 0), from)));
        let heading = if from == 1 { 0.0 } else { std::f32::consts::PI };
        let input = MoveInput {
            forward: 1.0,
            heading: Some(heading),
            ..MoveInput::default()
        };
        let mut seen = Vec::new();
        for _ in 0..240 {
            let r = w.step(&input, true, &world).unwrap();
            let x = r.state.position[0] - 16_000.0;
            if (79.99..=80.01).contains(&x) {
                seen.push(r.cell);
            }
            // Through the doorway and 2 m into the other room.
            if (from == 1 && x > 82.0) || (from == 2 && x < 78.0) {
                break;
            }
        }
        assert!(
            seen.iter().all(|c| *c == Some((key(0, 0), from))),
            "{seen:?}"
        );
        let to = 3 - from;
        assert_eq!(w.cell(), Some((key(0, 0), to)), "walked through into {to}");
        // Standing in the doorway itself, the cell is kept.
        let mut d = PhysicsWalker::new(&setup(), REGION, [16_080.0, 16_080.0, z0], 0.0);
        d.set_cell(Some((key(0, 0), from)));
        for _ in 0..30 {
            let r = d.step(&MoveInput::default(), true, &world).unwrap();
            assert_eq!(r.cell, Some((key(0, 0), from)));
        }
    }
    // Without a current cell, the lookup gives the lowest number (§5.8.1).
    let mut d = PhysicsWalker::new(&setup(), REGION, [16_080.0, 16_080.0, z0], 0.0);
    let r = d.step(&MoveInput::default(), true, &world).unwrap();
    assert_eq!(r.cell, Some((key(0, 0), 1)));
}

#[test]
fn no_landscape_water_inside_a_cell() {
    let z0 = ground(100.0) + 0.5;
    #[allow(clippy::cast_possible_truncation)]
    let zf = z0 as f32;
    let mut world = StaticWorld::default();
    let level = zf + 10.0;
    world.sets.insert(
        key(0, 0),
        block(
            key(0, 0),
            |_, _| 100.0,
            |b| {
                b.set_water_plane(Some(WaterPlane::new(level, 1).unwrap()));
                b.add_liquid_box(
                    LiquidBox::new(
                        [0.0, 0.0, 0.0],
                        [160.0, 160.0, level],
                        &Transform::IDENTITY,
                        LiquidFlags {
                            liquid: true,
                            deep: true,
                            ..LiquidFlags::default()
                        },
                    )
                    .unwrap(),
                )
                .unwrap();
                add_room(b, 1, [75.0, 80.0, zf], [5.0, 5.0], 5.0, Some(5.0));
            },
        ),
    );
    let centre = WorldPos::new(REGION, 16_080.0, 16_080.0, z0);
    let view: CollisionView<'_> = world.view(&centre);
    let masked = CellMaskedWater(&view);
    let inside = view.to_view([16_075.0, 16_080.0, z0]);
    let outside = view.to_view([16_020.0, 16_080.0, z0]);
    assert!(view.water_at(inside).is_some(), "the lake reaches the room");
    assert!(masked.water_at(inside).is_none(), "but not inside the cell");
    assert!(masked.water_at(outside).is_some());
}

/// The centre landblock and its neighbours installed; `owner` (four landblocks east) holds a
/// room above the centre landblock and is installed only while `loaded`.
struct FarOwner {
    near: StaticWorld,
    owner: LandblockKey,
    set: Arc<CollisionSet>,
    loaded: std::cell::Cell<bool>,
}

impl CollisionWorld for FarOwner {
    fn readiness(&self, k: LandblockKey) -> BlockReadiness {
        if k == self.owner {
            if self.loaded.get() {
                BlockReadiness::Ready
            } else {
                BlockReadiness::Pending
            }
        } else {
            self.near.readiness(k)
        }
    }
    fn view(&self, centre: &WorldPos) -> CollisionView<'_> {
        let mut v = self.near.view(centre);
        if self.loaded.get() {
            v.push_landblock(&self.set).unwrap();
        }
        v
    }
    fn terrain_height(&self, pos: &WorldPos) -> Option<f32> {
        self.near.terrain_height(pos)
    }
}

fn far_owner() -> FarOwner {
    let owner = key(4, 0);
    // The room's floor at 300 m over the centre landblock's (80, 80): x = 80 − 640 in the
    // owner's frame.
    let set = bare_set(owner, |b| {
        add_room(b, 7, [-560.0, 80.0, 300.0], [10.0, 10.0], 5.0, Some(10.0));
    });
    FarOwner {
        near: flat_world(100.0, |_| {}),
        owner,
        set,
        loaded: std::cell::Cell::new(false),
    }
}

#[test]
fn steps_wait_for_the_owner_of_a_far_cell() {
    let world = far_owner();
    let start = [16_080.0, 16_080.0, 300.01];
    // A scripted walk requires its landblock (the owner of its cells): no step runs, and
    // the mover does not fall, until the owner is installed.
    let mut w = PhysicsWalker::new(&setup(), REGION, start, 0.0);
    w.set_required(&[world.owner]);
    assert_eq!(w.pending(&world), vec![world.owner]);
    assert!(w.step(&MoveInput::default(), false, &world).is_none());
    assert_eq!(w.state().position, start);
    world.loaded.set(true);
    for _ in 0..60 {
        w.step(&MoveInput::default(), false, &world).unwrap();
    }
    let z = w.state().position[2];
    assert!((z - 300.0).abs() < 0.02, "on the room's floor: {z}");
    assert_eq!(w.cell(), Some((world.owner, 7)));

    // A walker in a cell needs the cell's owner: when it leaves residency, steps wait.
    let mut v = PhysicsWalker::new(&setup(), REGION, start, 0.0);
    v.step(&MoveInput::default(), false, &world).unwrap();
    assert_eq!(v.cell(), Some((world.owner, 7)));
    assert!(v.needed().contains(&Some(world.owner)));
    assert_eq!(v.pins(), vec![world.owner]);
    world.loaded.set(false);
    let before = *v.state();
    for _ in 0..60 {
        assert!(v.step(&MoveInput::default(), false, &world).is_none());
    }
    assert_eq!(v.state(), &before, "waits instead of falling through");
}

#[test]
fn a_teleport_below_the_terrain_lands_on_it() {
    let world = flat_world(100.0, |_| {});
    let g = ground(100.0);
    for below in [1.0, 1.65, 3.0, 20.0] {
        let mut w = PhysicsWalker::new(&setup(), REGION, [16_080.0, 16_080.0, g], 0.0);
        assert!(w.teleport(REGION, [16_080.0, 16_080.0, g - below], 0.3));
        assert!(w.placement_pending());
        for _ in 0..300 {
            w.step(&MoveInput::default(), false, &world).unwrap();
        }
        let z = w.state().position[2];
        assert!(
            (z - g).abs() < 0.02,
            "{below} m below: ends at {z} (ground {g})"
        );
        assert!(!w.placement_pending());
    }
    // Above the terrain nothing is changed: the walker falls onto it.
    let mut w = PhysicsWalker::new(&setup(), REGION, [16_080.0, 16_080.0, g], 0.0);
    w.teleport(REGION, [16_080.0, 16_080.0, g + 2.0], 0.0);
    w.step(&MoveInput::default(), false, &world).unwrap();
    assert!(w.state().position[2] > g + 1.9);
    // Onto an object standing on the terrain: its top.
    let zf = g as f32;
    let world = flat_world(100.0, |b| {
        solid_box(b, 1, [70.0, 70.0, zf - 1.0], [90.0, 90.0, zf + 1.0], None);
    });
    let mut w = PhysicsWalker::new(&setup(), REGION, [16_080.0, 16_080.0, g], 0.0);
    w.teleport(REGION, [16_080.0, 16_080.0, g - 5.0], 0.0);
    for _ in 0..60 {
        w.step(&MoveInput::default(), false, &world).unwrap();
    }
    let z = w.state().position[2];
    assert!((z - (g + 1.0)).abs() < 0.02, "on the box: {z}");
    // While the sets are missing the check waits with the step.
    let loading = Loading(flat_world(100.0, |_| {}));
    let mut w = PhysicsWalker::new(&setup(), REGION, [16_080.0, 16_080.0, g], 0.0);
    w.teleport(REGION, [16_080.0, 16_080.0, g - 2.0], 0.0);
    assert!(w.step(&MoveInput::default(), false, &loading).is_none());
    assert!(w.placement_pending());
}

#[test]
fn a_teleport_into_a_cell_is_kept() {
    let world = far_owner();
    world.loaded.set(true);
    let mut w = PhysicsWalker::new(&setup(), REGION, [16_080.0, 16_080.0, 100.0], 0.0);
    // The room is above the terrain here, so this checks only that a cell position is not
    // moved; feet below the terrain inside a cell are not lifted either (same rule).
    w.teleport(REGION, [16_080.0, 16_080.0, 300.01], 0.0);
    for _ in 0..30 {
        w.step(&MoveInput::default(), false, &world).unwrap();
    }
    assert!((w.state().position[2] - 300.0).abs() < 0.02);
    assert_eq!(w.cell(), Some((world.owner, 7)));
}

#[test]
fn mouse_look_shows_at_once() {
    let world = flat_world(100.0, |_| {});
    let mut w = PhysicsWalker::new(&setup(), REGION, [16_080.0, 16_080.0, ground(100.0)], 0.0);
    let f = w.frame(1.5 / 60.0, &MoveInput::default(), 0.0, false, &world);
    assert_eq!(f.steps, 1);
    w.turn(1.0);
    let (_, h) = w.interpolated();
    assert!(
        (h - w.state().heading).abs() < 1e-6,
        "{h} vs {}",
        w.state().heading
    );
    assert!((h - 1.0).abs() < 1e-6);
}

#[test]
fn log_headings_stay_below_360() {
    assert_eq!(compass_degrees(yaw_of(0.0)), 0.0);
    assert_eq!(compass_degrees(yaw_of(359.999)), 0.0);
    assert!((compass_degrees(yaw_of(90.0)) - 90.0).abs() < 1e-9);
    assert!((compass_degrees(yaw_of(359.9)) - 359.9).abs() < 0.006);
    let world = flat_world(100.0, |_| {});
    let text = walk_text(
        "pass E=80,80,100 d=10",
        "",
        [80.0, 80.0, ground(100.0)],
        0.0,
        "run",
        0.1,
    );
    let mut world = world;
    let out = run_walk(
        &parse_walk_file(&text).unwrap(),
        &mut world,
        &setup(),
        SYNTH_WAIT,
    )
    .unwrap();
    let mut csv = Vec::new();
    write_log(&mut csv, &out).unwrap();
    let csv = String::from_utf8(csv).unwrap();
    let heading = csv.lines().next().unwrap().split(',').nth(9).unwrap();
    assert_eq!(heading, "0.00");
}

#[test]
fn pass_and_follow_conditions() {
    let g = ground(100.0);
    // `pass` over open ground: passes.
    let text = walk_text(
        &format!("pass E=80,95,{g} d=0.15"),
        "wp=80,95",
        [80.0, 70.0, g],
        0.0,
        "run",
        8.0,
    );
    let walk = parse_walk_file(&text).unwrap();
    let mut open = flat_world(100.0, |_| {});
    let out = run_walk(&walk, &mut open, &setup(), SYNTH_WAIT).unwrap();
    assert!(out.verdict.passed, "{:?}", out.verdict);
    // With a placed object on the path: the contact fails it.
    let zf = g as f32;
    let mut blocked = flat_world(100.0, |b| {
        solid_box(
            b,
            1,
            [79.0, 82.0, zf - 1.0],
            [81.0, 83.0, zf + 3.0],
            Some(BodySource::Static {
                index: 0,
                object: tessera_formats::Did(0x0100_0001),
            }),
        );
    });
    let out = run_walk(&walk, &mut blocked, &setup(), SYNTH_WAIT).unwrap();
    assert!(!out.verdict.passed);
    assert!(
        out.verdict
            .failures
            .iter()
            .any(|f| f.contains("touched static#0")),
        "{:?}",
        out.verdict
    );

    // `follow` over the ground: the feet follow the terrain. The end condition is
    // horizontal only (§2.2): an end point given 1 m too high still passes. (Flat ground:
    // on a slope the controller stops at triangle edges, the fault reported for FR3.)
    let mut ground_world = flat_world(100.0, |_| {});
    for lift in [0.0, 1.0] {
        let text = walk_text(
            &format!("follow E=80,110,{} d=0.15 dz=0.05 headroom=10", g + lift),
            "wp=80,110",
            [80.0, 70.0, g],
            0.0,
            "run",
            10.0,
        );
        let out = run_walk(
            &parse_walk_file(&text).unwrap(),
            &mut ground_world,
            &setup(),
            SYNTH_WAIT,
        )
        .unwrap();
        assert!(out.verdict.passed, "{lift}: {:?}", out.verdict);
    }
    // Blocked by a wall across the path: fails as blocked.
    let mut walled = flat_world(100.0, |b| {
        solid_box(b, 2, [60.0, 90.0, zf - 2.0], [100.0, 91.0, zf + 5.0], None);
    });
    let text = walk_text(
        &format!("follow E=80,110,{g} d=0.15 dz=0.05 headroom=10"),
        "wp=80,110",
        [80.0, 70.0, g],
        0.0,
        "run",
        6.0,
    );
    let out = run_walk(
        &parse_walk_file(&text).unwrap(),
        &mut walled,
        &setup(),
        SYNTH_WAIT,
    )
    .unwrap();
    assert!(!out.verdict.passed);
    assert!(
        out.verdict
            .failures
            .iter()
            .any(|f| f.starts_with("blocked at")),
        "{:?}",
        out.verdict
    );
}

#[test]
fn blocked_measures_the_first_touch_and_allows_the_slide_after_it() {
    // checklist §2.2 `blocked` as revised from `collision.md` §11.7: a walker held forward
    // against a wall 20° off its heading first stops at the wall, then slides along it.
    let z = ground(100.0);
    #[allow(clippy::cast_possible_truncation)]
    let zf = z as f32;
    let mut world = flat_world(100.0, |b| {
        solid_box(b, 1, [40.0, 90.0, zf - 1.0], [140.0, 91.0, zf + 5.0], None);
    });
    let heading = 20f64;
    let gap = 90.0 - 0.35 - 84.0;
    let p = [80.0 + gap * heading.to_radians().tan(), 90.0 - 0.35, z];
    let text = walk_text(
        &format!("blocked P={},{},{} d=0.10", p[0], p[1], p[2]),
        "0:fwd",
        [80.0, 84.0, z],
        heading,
        "run",
        3.0,
    );
    let walk = parse_walk_file(&text).unwrap();
    let out = run_walk(&walk, &mut world, &setup(), SYNTH_WAIT).unwrap();
    assert!(out.verdict.passed, "{:?}", out.verdict);
    let end = out.rows.last().unwrap();
    // It slid on along the wall, farther than d from the stop point.
    assert!(
        end.local[0] > p[0] + 0.1,
        "{:?} {:?}",
        end.local,
        out.verdict
    );
    assert!(end.local[1] < 90.0 - 0.34, "{:?}", end.local);
    // A walker that never touches a blocker fails.
    let mut open = flat_world(100.0, |_| {});
    let out = run_walk(&walk, &mut open, &setup(), SYNTH_WAIT).unwrap();
    assert!(!out.verdict.passed);
    assert!(
        out.verdict
            .failures
            .iter()
            .any(|f| f == "never touched a blocker"),
        "{:?}",
        out.verdict
    );
}

#[test]
fn terrain_allowance_is_the_capsule_rest_offset_on_a_slope() {
    // collision.md §11.7 table, Tessera column (c = 0), r = 0.35.
    let at = |deg: f32| terrain_allowance(0.35, Some(deg.to_radians().cos()));
    assert!(at(0.0).abs() < 1e-9);
    assert!((at(29.0) - 0.050).abs() < 0.001, "{}", at(29.0));
    assert!((at(43.4) - 0.132).abs() < 0.001, "{}", at(43.4));
    assert!((at(45.0) - 0.145).abs() < 0.001, "{}", at(45.0));
    assert_eq!(terrain_allowance(0.35, None), 0.0);
    assert_eq!(terrain_allowance(0.35, Some(f32::NAN)), 0.0);
    assert_eq!(terrain_allowance(0.35, Some(0.0)), 0.0);
    assert_eq!(terrain_allowance(0.35, Some(-0.5)), 0.0);
    assert_eq!(terrain_allowance(f32::NAN, Some(0.5)), 0.0);
}

#[test]
fn follow_across_a_walkable_slope_allows_the_rest_offset() {
    // Terrain rising north at 40° (walkable, below 45°); the walker walks east along the
    // contour. Its capsule rests 0.35 · (sec 40° − 1) ≈ 0.107 m above the terrain under its
    // axis, more than dz = 0.05, which the slope allowance of §2.2 covers.
    let slope = 40f64.to_radians().tan();
    let o = key(0, 0).origin();
    let h = move |_x: f64, y: f64| (300.0 + (y - o[1] - 80.0) * slope) as f32;
    let mut world = StaticWorld::default();
    for dy in -1..=1 {
        for dx in -1..=1 {
            let k = key(dx, dy);
            world.sets.insert(k, block(k, h, |_| {}));
        }
    }
    let g = f64::from(h(0.0, o[1] + 80.0));
    let text = walk_text(
        &format!("follow E=90,80,{g} d=0.15 dz=0.05 headroom=10"),
        "wp=90,80",
        [70.0, 80.0, g + 0.11],
        90.0,
        "walk",
        // 20 m at the walk-mode 2.5 m/s (`collision.md` §11.8).
        12.0,
    );
    let out = run_walk(
        &parse_walk_file(&text).unwrap(),
        &mut world,
        &setup(),
        SYNTH_WAIT,
    )
    .unwrap();
    assert!(out.verdict.passed, "{:?}", out.verdict);
    let highest = out
        .rows
        .iter()
        .filter_map(|r| r.terrain.map(|t| r.local[2] - f64::from(t)))
        .fold(f64::MIN, f64::max);
    assert!(highest > 0.05, "{highest}");
    assert!(highest < 0.05 + 0.108, "{highest}");
}
