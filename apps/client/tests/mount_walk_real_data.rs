// SPDX-License-Identifier: GPL-3.0-or-later
//! P5 exit criterion 2 on the user's install (`walker.md` §16.11; `mounts.md` §14): a
//! playable body mounts one normal mount and one war-steed, rides a headless script through
//! every gait on flat synthetic ground, and the log rows show the clips the resolver gives
//! and speeds that follow the install's parameters; then a census of every mount effect
//! for one body of each size (drawable or categorised, no crash).
//!
//! Reads only `TESSERA_GAME_DIR` (read-only) and skips itself when it is unset. No DID is
//! written here: every expectation is derived from the install at run time. Heavy: run
//! with `--release`.

use std::sync::Arc;

use tessera_client::avatar::parse_avatar;
use tessera_client::walker::characters::catalog::CharacterCatalog;
use tessera_client::walker::mount::eval::{ClipKey, MountVocab, Stage};
use tessera_client::walker::mount::profile::{
    MountData, MountKind, MountProfile, resolve_mount, rider_body,
};
use tessera_client::walker::mount::ride::{MountMover, MountedJump};
use tessera_client::walker::mount::session::plain_request;
use tessera_client::walker::mount::spec::{MountSource, SteedTree};
use tessera_client::walker::mount::walk::{RideRow, Segment, ride_segments};
use tessera_client::walker::mount::{MountEvaluator, census::census};
use tessera_client::walker::movement::run::StaticWorld;
use tessera_client::walker::movement::setup::MoverSetup;
use tessera_client::walker::movement::{MovementState, PhysicsWalker};
use tessera_formats::LandblockData;
use tessera_formats::landblock::HeightGrid;
use tessera_formats::{Did, LandblockId};
use tessera_mesh::assembly::AssemblyOptions;
use tessera_net::Body;
use tessera_physics::controller::LayerConditions;
use tessera_physics::controller::warsteed::WarSteedParams;
use tessera_physics::geometry::{CollisionLimits, CollisionSetBuilder, GridMesh};
use tessera_world::LandblockKey;

const REGION: u8 = 1;

fn game_dir() -> Option<std::path::PathBuf> {
    let d = std::env::var_os("TESSERA_GAME_DIR")?;
    Some(std::path::PathBuf::from(d))
}

/// Flat ground at 10 m on the 5 × 5 landblocks around (100, 100).
fn flat_world() -> (StaticWorld, [f64; 3]) {
    let z0 = 10.0_f32;
    let raw = (z0 * 65_535.0 / 2_000.0).round() as u16;
    let mut w = StaticWorld::default();
    let centre = LandblockKey::new(REGION, 100, 100).unwrap();
    for dy in -2..=2 {
        for dx in -2..=2 {
            let k = centre.offset(dx, dy).unwrap();
            let grid = HeightGrid::from_fn(|_, _| raw);
            let data = LandblockData::from_heights(k.data_did(), grid);
            let mut b = CollisionSetBuilder::new(CollisionLimits::default());
            b.set_landblock(LandblockId::from_did(k.data_did()).unwrap());
            b.add_grid(Arc::new(GridMesh::terrain(&data))).unwrap();
            w.sets.insert(k, Arc::new(b.build()));
        }
    }
    let o = centre.origin();
    let z = f64::from(raw) * 2_000.0 / 65_535.0;
    (w, [o[0] + 96.0, o[1] + 96.0, z + 0.01])
}

struct Install {
    dir: std::path::PathBuf,
    catalog: CharacterCatalog,
    data: MountData,
}

fn open() -> Option<Install> {
    let dir = game_dir()?;
    let set = Arc::new(
        tessera_dat::ArchiveSet::open(&dir, tessera_dat::SetOptions::default()).expect("archives"),
    );
    let catalog = CharacterCatalog::build(set);
    let data = MountData::open(&dir).expect("mount data");
    Some(Install { dir, catalog, data })
}

fn body_of(inst: &Install, avatar: &str) -> (Body, tessera_client::avatar::AvatarSpec) {
    let spec = parse_avatar(avatar).expect("avatar");
    let body = inst.catalog.appearance(&spec).expect("body").body;
    (body, spec)
}

struct Ride {
    profile: Arc<MountProfile>,
    rows: Vec<RideRow>,
    setup: MoverSetup,
    jump: MountedJump,
}

/// Mounts `source` on `avatar` (instant) and plays `segments`.
fn ride(inst: &Install, avatar: &str, source: MountSource, segments: &[Segment]) -> Ride {
    let (body, spec) = body_of(inst, avatar);
    let Body::Character { species, sex } = body else {
        panic!("{avatar} is not a playable body");
    };
    let rb = rider_body(inst.data.game_data(), species, sex).expect("race entry");
    let profile = Arc::new(resolve_mount(&inst.data, &rb, source).expect("mount resolves"));
    let mut ev = MountEvaluator::new(
        &inst.catalog,
        AssemblyOptions::default(),
        &plain_request(body),
        Arc::clone(&profile),
    )
    .expect("mount evaluates");
    let setup = MoverSetup::load(&inst.catalog, &spec);
    let (world, feet) = flat_world();
    let mut walker = PhysicsWalker::new(&setup, REGION, feet, 0.0);
    walker.check_placement();
    let jump = ev.setup().jump;
    walker.set_mount(Some(
        MountMover::new(Arc::clone(&profile), ev.setup().timeline, false, None).with_jump(jump),
    ));
    let mut clips = |k: &ClipKey| Some(ev.clips(k));
    let mut segs = vec![Segment::idle(0.5)];
    segs.extend_from_slice(segments);
    let rows = ride_segments(&mut walker, &world, &segs, &mut clips);
    assert!(!rows.is_empty(), "{avatar}: no step ran");
    let _ = &inst.dir;
    Ride {
        profile,
        rows,
        setup,
        jump,
    }
}

fn steady(rows: &[RideRow], segment: usize) -> &RideRow {
    rows.iter()
        .rev()
        .find(|r| r.segment == segment)
        .expect("segment rows")
}

/// `walker.md` §16.11 checks 1 and 2: `auto` rides a normal mount of the body's group.
/// `auto` skips the directory group's war-steed skills and picks the first normal mount
/// (`walker.md` §16.2, §16.12 resolution), so Human M on `auto` is a horse, not a war-steed.
fn normal_mount(inst: &Install, avatar: &str) {
    let r = ride(
        inst,
        avatar,
        MountSource::Auto,
        &[
            Segment::moving(4.0, 1.0, false),
            Segment::idle(1.0),
            Segment::moving(2.0, -1.0, false),
            Segment::moving(2.0, 1.0, true),
        ],
    );
    let p = &r.profile;
    assert!(
        matches!(p.kind, MountKind::Normal),
        "{avatar}: auto is a normal mount ({:?})",
        p.kind
    );
    for row in &r.rows {
        assert_eq!(
            row.mount.state, 3,
            "{avatar}: mounted throughout at {}",
            row.t
        );
        assert_eq!(row.mount.effect, p.effect.0);
    }
    // §14.2, §14.5 item 4: the idle shown is the mounted idle layer's.
    let vocab = MountVocab::from_catalog(&inst.catalog);
    assert_eq!(
        Some(steady(&r.rows, 0).mount.rider_layer),
        vocab.layers.mounted_idle,
        "{avatar}: mounted idle layer"
    );
    // `mounts.md` §14.6 "Jumping while mounted": a normal mount takes the other branch,
    // whose node the install gives where its gates hold (checked in check 6 below).
    assert!(!r.jump.war_steed_branch, "{avatar}: {:?}", p.mount_type);
    // Layers: Mounted while running, MountedWalking while walking.
    let run = steady(&r.rows, 1);
    let walk = steady(&r.rows, 4);
    assert_eq!(run.mount.move_layer, "Mounted", "{avatar}");
    assert_eq!(walk.mount.move_layer, "MountedWalking", "{avatar}");
    // Steady speed: the Mounted layer's forward speed × the speed factor (§5.1 Decision).
    let conds = LayerConditions {
        mounted: true,
        ..r.setup.conditions
    };
    let layer = r
        .setup
        .record
        .compose_movement(&conds, MovementState::EarthControlledSurface);
    // Tessera's step multiplies the horizontal velocity by the layer's damper to the power
    // of the step length (`collision.md` §12.3 step 3), so the steady speed is the target
    // times that factor (`walker.md` §16.12 OPEN on check 1).
    let damper = layer.velocity_damper[0].clamp(0.0, 1.0).powf(1.0 / 60.0);
    let expected = layer.forward_speed * p.speed_factor * damper;
    eprintln!(
        "{avatar}: {} speed factor {:.3}, mounted forward {:.3} → expected {:.3}, steady {:.3}, ground {:.3}; damper {:?}",
        p.effect,
        p.speed_factor,
        layer.forward_speed,
        expected,
        run.mount.control_speed,
        run.mount.ground_speed,
        layer.velocity_damper
    );
    assert!(
        (run.mount.control_speed - expected).abs() <= 0.05,
        "{avatar}: steady {} vs {expected}",
        run.mount.control_speed
    );
    // Clips: every state shows a rider and a mount clip; idle, run, back and walk differ.
    let idle = steady(&r.rows, 2);
    let back = steady(&r.rows, 3);
    for (name, row) in [("run", run), ("idle", idle), ("back", back), ("walk", walk)] {
        eprintln!(
            "{avatar} {name}: rider {:#x} (layer {}, rate {:.2}), mount {:#x} (layer {}, rate {:.2})",
            row.mount.rider_clip,
            row.mount.rider_layer,
            row.mount.rider_rate,
            row.mount.mount_clip,
            row.mount.mount_layer,
            row.mount.mount_rate
        );
        assert_ne!(row.mount.rider_clip, 0, "{avatar} {name}: rider clip");
        assert_ne!(row.mount.mount_clip, 0, "{avatar} {name}: mount clip");
    }
    assert_ne!(idle.mount.rider_clip, run.mount.rider_clip, "{avatar}");
    assert_ne!(run.mount.rider_clip, back.mount.rider_clip, "{avatar}");
    assert_ne!(idle.mount.mount_clip, run.mount.mount_clip, "{avatar}");
    assert_ne!(walk.mount.mount_clip, run.mount.mount_clip, "{avatar}");
    // Normal mounts do not read the gait (§14.4): it stays unset (logged 2).
    assert!(r.rows.iter().all(|row| row.mount.gait == 2));
}

#[test]
fn normal_mounts_ride_with_the_install_speed_and_clips() {
    let Some(inst) = open() else {
        eprintln!("TESSERA_GAME_DIR not set; skipped");
        return;
    };
    normal_mount(&inst, "human,m");
    normal_mount(&inst, "hobbit,m");
}

/// `mounts.md` §14.6 ("the sliders follow the input as on foot"), `script-table.md` §8.4
/// slider rule (T30): a normal mount's published `Slider_Propel` eases toward the input at
/// the active layer's ramp time — the install's Mounted layer, 0.25 s — instead of jumping
/// to 1 in one step, so the mounted jump gate of `walker.md` §16.6 (`Slider_Propel` > 0.5)
/// reads the smoothed value and the first forward steps do not yet clear it.
#[test]
fn a_normal_mount_eases_its_sliders_at_the_install_ramp() {
    let Some(inst) = open() else {
        eprintln!("TESSERA_GAME_DIR not set; skipped");
        return;
    };
    // Settle at rest (prepended idle seeds the slider to 0), then hold forward for 1 s.
    let r = ride(
        &inst,
        "human,m",
        MountSource::Auto,
        &[Segment::moving(1.0, 1.0, false)],
    );
    assert_eq!(r.profile.kind, MountKind::Normal);
    // The ramp time of the install's Mounted layer (0.25 s here).
    let conds = LayerConditions {
        mounted: true,
        ..r.setup.conditions
    };
    let layer = r
        .setup
        .record
        .compose_movement(&conds, MovementState::EarthControlledSurface);
    let t = layer.ramp_time;
    let fwd: Vec<&RideRow> = r.rows.iter().filter(|row| row.segment == 1).collect();
    let first = fwd.first().expect("forward rows");
    eprintln!(
        "mounted ramp {t:.3} s; Slider_Propel after one forward step {:.3}",
        first.sliders[0]
    );
    assert!(
        first.sliders[0] > 0.0 && first.sliders[0] < 0.5,
        "eased, not jumped: {}",
        first.sliders[0]
    );
    // The gate's propel threshold (0.5) is not met until the ramp has run for a while.
    assert!(
        first.sliders[0] <= 0.5,
        "a jump pressed at the first forward step is turned away: {}",
        first.sliders[0]
    );
    // Rises only (while forward is held) and reaches full propel at about the ramp time.
    for w in fwd.windows(2) {
        assert!(
            w[1].sliders[0] >= w[0].sliders[0] - 1e-6,
            "Slider_Propel only rises while forward is held: {} then {}",
            w[0].sliders[0],
            w[1].sliders[0]
        );
    }
    let hit = fwd
        .iter()
        .position(|row| row.sliders[0] >= 1.0 - 1e-3)
        .expect("reaches full propel");
    let expected = (f64::from(t) * 60.0).round() as i64;
    eprintln!(
        "full Slider_Propel after {} steps (expected about {expected})",
        hit + 1
    );
    assert!(
        (hit as i64 + 1 - expected).abs() <= 2,
        "eased over the ramp: {} steps vs about {expected}",
        hit + 1
    );
}

#[test]
fn a_war_steed_shifts_through_every_gait() {
    let Some(inst) = open() else {
        eprintln!("TESSERA_GAME_DIR not set; skipped");
        return;
    };
    let r = ride(
        &inst,
        "human,m",
        MountSource::Steed(SteedTree::Medium),
        &[
            Segment::moving(4.0, 1.0, false),
            Segment::idle(1.0),
            Segment::moving(3.0, -1.0, false),
        ],
    );
    let p = &r.profile;
    assert_eq!(p.kind, MountKind::WarSteed);
    // §14.2, §14.5 item 4: a war-steed rider's idle is on `mounted_warsteed_idle`.
    let vocab = MountVocab::from_catalog(&inst.catalog);
    assert_eq!(
        Some(steady(&r.rows, 0).mount.rider_layer),
        vocab.layers.mounted_warsteed_idle,
        "war-steed idle layer"
    );
    let ws = p.war_steed.expect("war-steed values");
    // The forward speed F of the block (§5.3) for the Warsteed layer.
    let conds = LayerConditions {
        mounted: true,
        ramp_up_controller: true,
        ..r.setup.conditions
    };
    let layered = r
        .setup
        .record
        .compose_movement(&conds, MovementState::EarthControlledSurface);
    let f = WarSteedParams::build(&layered, &ws.actor).forward;
    let g_rate = WarSteedParams::build(&layered, &ws.actor).gear_change_rate;
    eprintln!(
        "steed {}: F {f:.3}, G {g_rate}, world limit {}, speeds {:?}",
        p.effect, ws.world_limit, p.gaits.speed
    );
    for row in &r.rows {
        assert_eq!(row.mount.state, 3);
        assert_eq!(row.mount.move_layer, "Warsteed", "at {}", row.t);
    }
    // Forward held: gaits 4, 5, 6, 7 in that order.
    let forward: Vec<&RideRow> = r.rows.iter().filter(|r| r.segment == 1).collect();
    let mut seen: Vec<u32> = Vec::new();
    for row in &forward {
        if seen.last() != Some(&row.mount.gait) {
            seen.push(row.mount.gait);
        }
        // Never above the limit of §5.4 step 3 (the selected gear's).
        let limit = p.gaits.speed_multiplier(row.mount.locked_gear) * f;
        assert!(
            row.mount.control_speed <= limit.min(ws.world_limit) + 1e-3,
            "{} above {limit} at {}",
            row.mount.control_speed,
            row.t
        );
    }
    eprintln!("gaits while forward: {seen:?}");
    assert_eq!(seen, vec![4, 5, 6, 7], "gaits while forward is held");
    // Gears rise one step per 1/G s at most.
    let mut last_change: Option<(f64, u32)> = None;
    for row in &forward {
        let g = row.mount.locked_gear;
        if last_change.is_none_or(|(_, lg)| lg != g) {
            if let Some((t0, lg)) = last_change
                && g > lg
            {
                assert!(
                    row.t - t0 >= 1.0 / f64::from(g_rate) - 1e-3,
                    "gear {lg} → {g} after {} s",
                    row.t - t0
                );
            }
            last_change = Some((row.t, g));
        }
    }
    // Steady gallop at the gallop limit.
    let top = steady(&r.rows, 1);
    let gallop = (p.gaits.speed_multiplier(7) * f).min(ws.world_limit);
    assert!(
        (top.mount.control_speed - gallop).abs() < 0.01,
        "{} vs {gallop}",
        top.mount.control_speed
    );
    // Released: the speed holds.
    let released: Vec<&RideRow> = r.rows.iter().filter(|r| r.segment == 2).collect();
    for row in &released {
        assert!(
            (row.mount.control_speed - top.mount.control_speed).abs() <= 0.01,
            "held speed {} at {}",
            row.mount.control_speed,
            row.t
        );
        assert_eq!(row.mount.gait, 7);
    }
    // Backward: the gaits fall step by step through Neutral (2) to Backwards (1)
    // (`walker.md` §16.11 check 3, §16.12 resolution: the step whose forward velocity crosses
    // zero reads Neutral, so the gait falls 4 → 2 → 1 rather than 4 → 1).
    let mut back_seen: Vec<u32> = Vec::new();
    for row in r.rows.iter().filter(|r| r.segment == 3) {
        if back_seen.last() != Some(&row.mount.gait) {
            back_seen.push(row.mount.gait);
        }
    }
    eprintln!("gaits while backward: {back_seen:?}");
    assert_eq!(back_seen.first(), Some(&7), "{back_seen:?}");
    assert_eq!(back_seen.last(), Some(&1), "{back_seen:?}");
    assert!(
        back_seen.contains(&2),
        "Neutral at the zero crossing: {back_seen:?}"
    );
    assert!(
        back_seen.windows(2).all(|w| w[1] < w[0]),
        "gaits only fall: {back_seen:?}"
    );
    // Clips follow the gait: one rider and one mount forward clip per gait 5, 6, 7.
    let clip_of = |g: u32| {
        forward
            .iter()
            .rev()
            .find(|r| r.mount.gait == g)
            .map(|r| (r.mount.rider_clip, r.mount.mount_clip))
            .unwrap()
    };
    let per: Vec<(u32, u32)> = [4, 5, 6, 7].into_iter().map(clip_of).collect();
    eprintln!("clips per gait 4–7: {per:x?}");
    for (i, a) in per.iter().enumerate().skip(1) {
        for b in per.iter().skip(i + 1) {
            assert_ne!(a.0, b.0, "rider clips of gaits 5–7 differ");
            assert_ne!(a.1, b.1, "mount clips of gaits 5–7 differ");
        }
    }
    for row in &r.rows {
        assert_ne!(row.mount.rider_clip, 0);
        assert_ne!(row.mount.mount_clip, 0);
    }
    // The rider's evaluated state carries the rider fields and the gait (§14.2).
    let key = r.rows.last().and_then(|row| row.key).unwrap();
    assert_eq!(key.stage, Stage::Riding);
}

#[test]
fn census_of_every_mount_effect_for_a_tall_and_a_short_body() {
    let Some(inst) = open() else {
        eprintln!("TESSERA_GAME_DIR not set; skipped");
        return;
    };
    let effects: Vec<Did> = inst
        .data
        .store()
        .mount_types()
        .expect("mount effects")
        .keys()
        .map(|d| Did(*d))
        .collect();
    assert!(!effects.is_empty());
    for avatar in ["human,m", "hobbit,m"] {
        let (body, _) = body_of(&inst, avatar);
        let report = census(
            &inst.data,
            &inst.catalog,
            AssemblyOptions::default(),
            &plain_request(body),
            &effects,
        )
        .expect("census");
        eprintln!("{avatar}: {report}");
        for (d, why) in report.refusals.iter().take(20) {
            eprintln!("  refused {d}: {why}");
        }
        assert_eq!(report.effects, effects.len());
        assert!(report.drawable > 0);
        // Every refusal is categorised, and none of a category §14.11 draws.
        assert!(
            report
                .categories
                .iter()
                .all(|(c, n)| c.is_some() || n.refused == 0),
            "unrecognised mount effects"
        );
        for (c, n) in &report.categories {
            if let Some(c) = c
                && c.is_drawable()
            {
                assert_eq!(n.refused, 0, "{avatar}: {c:?} refused");
                assert!(n.drawable > 0 || n.refused == 0);
            }
        }
    }
}

/// `walker.md` §16.11 check 4: with summoning, states 1 → 2 → 3 at the induction's
/// duration and the start script's length (±1 step); forward input during summoning
/// returns to on foot.
#[test]
fn summoning_and_mounting_follow_the_install_timeline() {
    let Some(inst) = open() else {
        eprintln!("TESSERA_GAME_DIR not set; skipped");
        return;
    };
    let (body, spec) = body_of(&inst, "human,m");
    let Body::Character { species, sex } = body else {
        panic!("playable");
    };
    let rb = rider_body(inst.data.game_data(), species, sex).expect("race entry");
    let profile = Arc::new(resolve_mount(&inst.data, &rb, MountSource::Auto).expect("resolves"));
    let induction = profile.induction.expect("the skill's induction");
    let mut ev = MountEvaluator::new(
        &inst.catalog,
        AssemblyOptions::default(),
        &plain_request(body),
        Arc::clone(&profile),
    )
    .expect("evaluates");
    let timeline = ev.setup().timeline;
    eprintln!(
        "timeline {timeline:?} (induction script {:?}), notes {:?}",
        induction.script,
        ev.setup().notes
    );
    assert_eq!(timeline.summon, Some(induction.duration));
    assert!(timeline.mounting > 0.0, "the start script runs for a while");
    assert!(timeline.fade_in > 0.0 && timeline.fade_out > 0.0);
    let setup = MoverSetup::load(&inst.catalog, &spec);
    let (world, feet) = flat_world();
    let run = |segments: &[Segment], ev: &mut MountEvaluator<'_>| {
        let mut walker = PhysicsWalker::new(&setup, REGION, feet, 0.0);
        walker.check_placement();
        let mut settle = vec![Segment::idle(0.5)];
        let mut clips = |k: &ClipKey| Some(ev.clips(k));
        let _ = ride_segments(&mut walker, &world, &settle, &mut clips);
        walker.set_mount(Some(MountMover::new(
            Arc::clone(&profile),
            timeline,
            true,
            None,
        )));
        settle.clear();
        ride_segments(&mut walker, &world, segments, &mut clips)
    };
    let rows = run(
        &[Segment::idle(timeline.mounting + induction.duration + 1.0)],
        &mut ev,
    );
    let mut changes: Vec<(f64, u8)> = Vec::new();
    let mut last = 1;
    for r in &rows {
        if r.mount.state != last {
            changes.push((r.t, r.mount.state));
            last = r.mount.state;
        }
    }
    eprintln!("state changes {changes:?}");
    // ±1 frame (§16.11 check 4); the script lengths count whole steps.
    let step = 1.0 / 60.0 + 1e-4;
    assert!(
        (timeline.mounting * 60.0 - (timeline.mounting * 60.0).round()).abs() < 1e-3,
        "whole steps: {}",
        timeline.mounting
    );
    assert_eq!(changes.len(), 2, "{changes:?}");
    assert_eq!(changes[0].1, 2);
    assert!((changes[0].0 - f64::from(induction.duration)).abs() <= step);
    assert_eq!(changes[1].1, 3);
    assert!((changes[1].0 - f64::from(induction.duration + timeline.mounting)).abs() <= step);
    // The summoning rider plays the induction's clip, the mounting rider the start
    // script's clip, then the mounted idle.
    let clip_in = |s: u8| {
        rows.iter()
            .find(|r| r.mount.state == s)
            .map(|r| r.mount.rider_clip)
    };
    eprintln!(
        "rider clips: summoning {:x?}, mounting {:x?}, mounted {:x?}",
        clip_in(1),
        clip_in(2),
        clip_in(3)
    );
    assert!(clip_in(3).is_some_and(|c| c != 0));
    // Forward during summoning: back on foot.
    let rows = run(
        &[Segment::idle(0.5), Segment::moving(0.5, 1.0, false)],
        &mut ev,
    );
    assert!(rows.iter().any(|r| r.mount.state == 1));
    assert_eq!(rows.last().map(|r| r.mount.state), Some(0));
}

/// The rows of a ride's take-offs.
fn take_offs(rows: &[RideRow]) -> Vec<&RideRow> {
    rows.iter().filter(|r| r.take_off.is_some()).collect()
}

/// How far the feet rose above `ground` after the first take-off.
fn rise(rows: &[RideRow], ground: f64) -> f64 {
    rows.iter().map(|r| r.feet[2] - ground).fold(0.0, f64::max)
}

/// `walker.md` §16.11 check 6 (normal mount): Human M on `auto` presses jump once while
/// standing (no take-off, no `Jump`) and again after 2 s of forward input: exactly one
/// take-off, whose vertical impulse is (`AirTime` + 0.2) × `ExpectedGravity` of the
/// mounted-branch node the install gives for the state (`mounts.md` §14.6); mounted
/// throughout.
#[test]
fn check_6_a_normal_mount_jumps_only_moving_forward() {
    let Some(inst) = open() else {
        eprintln!("TESSERA_GAME_DIR not set; skipped");
        return;
    };
    let r = ride(
        &inst,
        "human,m",
        MountSource::Auto,
        &[
            Segment {
                seconds: 1.0,
                jump: true,
                ..Segment::default()
            },
            Segment::moving(2.0, 1.0, false),
            Segment {
                seconds: 2.0,
                forward: 1.0,
                jump: true,
                ..Segment::default()
            },
        ],
    );
    assert_eq!(r.profile.kind, MountKind::Normal);
    assert!(!r.jump.war_steed_branch);
    let node = r
        .jump
        .forward
        .get(MovementState::EarthControlledSurface)
        .expect("the install's mounted-branch node on land");
    eprintln!(
        "auto {}: mounted jump node {node:?}, {:.3} m/s",
        r.profile.effect,
        node.vertical_speed()
    );
    for row in &r.rows {
        assert_eq!(row.mount.state, 3, "mounted throughout at {}", row.t);
    }
    // The standing press: no take-off and no `Jump`.
    let standing: Vec<&RideRow> = r.rows.iter().filter(|row| row.segment == 1).collect();
    assert!(standing.iter().all(|row| row.take_off.is_none()));
    assert!(standing.iter().all(|row| !row.key.is_some_and(|k| k.jump)));
    let ground = standing.last().expect("standing rows").feet[2];
    assert!(
        standing
            .iter()
            .all(|row| (row.feet[2] - ground).abs() < 0.01),
        "the standing press does not lift"
    );
    // The moving press: exactly one take-off, at the first step of segment 3.
    let offs = take_offs(&r.rows);
    assert_eq!(offs.len(), 1, "one take-off");
    let off = offs[0];
    assert_eq!(off.segment, 3);
    let v = off.take_off.unwrap().vertical_speed();
    eprintln!("take-off {v:.3} m/s at {:.3} s", off.t);
    assert!((v - node.vertical_speed()).abs() < 1e-5, "{v}");
    // The impulse lifted the rider: under the Falling layer's gravity (equal to the node's
    // `ExpectedGravity`, `collision.md` §11.10) the apex is about v² / (2 g).
    let apex = rise(&r.rows, ground);
    let expected = f64::from(v * v / (2.0 * node.expected_gravity));
    eprintln!("rose {apex:.3} m (about {expected:.3} m expected)");
    assert!(apex > 0.8 * expected, "rose {apex}");
}

/// `walker.md` §16.11 check 6 (war-steed): Human M on `steed:medium` presses jump at rest
/// (gait 2: no take-off) and after 1 s of forward input (gait 4 or above: one take-off at
/// the node the install gives for that gait).
#[test]
fn check_6_a_war_steed_jumps_only_at_gait_four_or_above() {
    let Some(inst) = open() else {
        eprintln!("TESSERA_GAME_DIR not set; skipped");
        return;
    };
    let r = ride(
        &inst,
        "human,m",
        MountSource::Steed(SteedTree::Medium),
        &[
            Segment {
                seconds: 1.0,
                jump: true,
                ..Segment::default()
            },
            Segment::moving(1.0, 1.0, false),
            Segment {
                seconds: 2.0,
                forward: 1.0,
                jump: true,
                ..Segment::default()
            },
        ],
    );
    assert_eq!(r.profile.kind, MountKind::WarSteed);
    assert!(r.jump.war_steed_branch, "{:#x}", r.profile.mount_type);
    for row in &r.rows {
        assert_eq!(row.mount.state, 3, "mounted throughout at {}", row.t);
    }
    let rest: Vec<&RideRow> = r.rows.iter().filter(|row| row.segment == 1).collect();
    assert_eq!(rest.first().map(|row| row.mount.gait), Some(2), "at rest");
    assert!(rest.iter().all(|row| row.take_off.is_none()));
    assert!(rest.iter().all(|row| !row.key.is_some_and(|k| k.jump)));
    let offs = take_offs(&r.rows);
    assert_eq!(offs.len(), 1, "one take-off");
    let off = offs[0];
    assert_eq!(off.segment, 3);
    // The gait at the press: that of the step before it.
    let i = r
        .rows
        .iter()
        .position(|row| std::ptr::eq(row, off))
        .unwrap();
    let gait = r.rows[i - 1].mount.gait;
    assert!(gait >= 4, "gait {gait} at the press");
    let node = r.jump.by_gait[usize::try_from(gait - 4).unwrap()]
        .get(MovementState::EarthControlledSurface)
        .expect("the install's node for the gait");
    let v = off.take_off.unwrap().vertical_speed();
    eprintln!(
        "steed {}: take-off at gait {gait}, {v:.3} m/s (node {node:?})",
        r.profile.effect
    );
    assert!((v - node.vertical_speed()).abs() < 1e-5);
    let ground = rest.last().unwrap().feet[2];
    assert!(rise(&r.rows, ground) > 0.5);
}
