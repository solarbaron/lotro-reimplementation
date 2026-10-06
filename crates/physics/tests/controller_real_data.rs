// SPDX-License-Identifier: GPL-3.0-or-later
//! The character controller against a user's own install (read-only):
//!
//! - the player movement record (`collision.md` §10.3) is found by the presence of
//!   `PlayerPhysicsControl_BasePlayerParameters` among the 0x79 sets, read through
//!   `tessera-props`, and composes the worked example §14.6 and the swimming example of
//!   §11.1; the default float height is 1.35;
//! - the Generic controller set of §10.2 is read through the `PHYSICSCONTROLLER` DID map;
//! - seeded random walks on full collision sets of sampled landblocks (terrain, ceiling,
//!   statics, cells) never go non-finite, never end a step deeper than 0.01 m in geometry,
//!   never fall through the terrain of a landblock without ceiling (feet more than 0.05 m
//!   below it, outside cells), and repeat bit for bit;
//! - wet landblocks (`water.md` §6.2) carry their water plane and the three liquid bands of
//!   the landblock volume entity (§7.1; the bands of 0x47000754 are used for every water
//!   type, a test simplification). Walks spawn above the water, so swimmers come from falls;
//!   a swimmer more than 0.05 m below the float line (surface − 1.35, `water.md` §7.4
//!   OPEN) never sinks further, none is ever 0.5 m below it, and the water states 1–3
//!   occur;
//! - every fourth walk spawns inside a cell of the landblock when it has one (such walks
//!   may be under the terrain, so the fall-through check skips them), and the cell lookup
//!   of §12.3 step 8 then reports cells.
//!
//! Skipped (reported as passing) unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-physics --test controller_real_data -- --nocapture`. A
//! debug build walks a small sample unless `TESSERA_FULL=1`. Only counts, DIDs and
//! parameter values named by the spec are printed.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use tessera_dat::{ArchiveKind, ArchiveSet};
use tessera_formats::collision::{CELL_GEOMETRY_PREFIX, CellGeometry};
use tessera_formats::water::{WaterTable, decode_water_table};
use tessera_formats::{
    BlockMap, Did, LandblockData, LandblockInfo, PropertyDictionary, Record, RegionDescriptor,
};
use tessera_physics::controller::{
    Controller, ControllerConfig, ControllerKind, LayerConditions, MoveInput, MovementPids,
    MovementState, MoverState, PlayerMovementRecord, StepEnv, StepOutcome, controller_set,
};
use tessera_physics::geometry::{
    CollisionLimits, CollisionSet, CollisionSetBuilder, FetchedRecord, GridMesh, LiquidBox,
    LiquidFlags, ObjectCollisionCache, PhysicsPids, QueryFilter, StaticOptions, Transform,
    WaterPlane, resolve_object_collision,
};
use tessera_props::GameData;

fn game_data() -> Option<&'static GameData> {
    static DATA: OnceLock<Option<GameData>> = OnceLock::new();
    DATA.get_or_init(|| {
        let dir = match std::env::var_os("TESSERA_GAME_DIR") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => {
                eprintln!("TESSERA_GAME_DIR not set; skipping");
                return None;
            }
        };
        Some(GameData::open(&dir).expect("open install"))
    })
    .as_ref()
}

fn full() -> bool {
    std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

fn exhaustive() -> bool {
    !cfg!(debug_assertions) || full()
}

fn dictionary(data: &GameData) -> &PropertyDictionary {
    data.registry().expect("registry").dictionary()
}

/// DIDs with `prefix` in archives of `kind`, ascending.
fn dids_of(set: &ArchiveSet, prefix: u8, kind: impl Fn(&ArchiveKind) -> bool) -> Vec<u32> {
    let lo = u32::from(prefix) << 24;
    let mut out = Vec::new();
    for m in set.members() {
        if !kind(&m.id.kind) {
            continue;
        }
        for e in m.archive.entries_range(lo..=lo | 0x00FF_FFFF) {
            out.push(e.expect("directory walk").did);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn record() -> Option<&'static (Did, PlayerMovementRecord)> {
    static RECORD: OnceLock<Option<(Did, PlayerMovementRecord)>> = OnceLock::new();
    RECORD
        .get_or_init(|| {
            let data = game_data()?;
            let candidates = dids_of(data.source(), 0x79, |k| *k == ArchiveKind::Gamelogic);
            eprintln!("{} property sets with prefix 0x79", candidates.len());
            let started = Instant::now();
            let found = PlayerMovementRecord::locate(data, candidates.into_iter().map(Did))
                .expect("locate");
            eprintln!("located in {:.1} s", started.elapsed().as_secs_f64());
            found
        })
        .as_ref()
}

fn pids() -> Option<MovementPids> {
    Some(MovementPids::from_registry(game_data()?.registry().ok()?))
}

#[test]
fn player_movement_record() {
    let Some((did, r)) = record() else {
        if game_data().is_some() {
            panic!("no 0x79 set holds PlayerPhysicsControl_BasePlayerParameters");
        }
        return;
    };
    let pids = pids().expect("pids");
    eprintln!("player movement record: {:#010x}", did.0);
    assert!(pids.fields.iter().all(Option::is_some), "§10.1 names");
    assert!(pids.move_to_float_height.is_some());
    let present: Vec<&str> = tessera_physics::controller::LayerKind::ALL
        .iter()
        .filter(|k| r.layer(**k).is_some())
        .map(|k| k.name())
        .collect();
    eprintln!("layers: {present:?}");
    assert_eq!(present.len(), 14, "every §10.3 layer");
    assert_eq!(r.default_float_height, Some(1.35));
    // BasePlayer (§10.3).
    let base = r
        .compose(
            &LayerConditions::default(),
            MovementState::EarthControlledSurface,
            &pids,
        )
        .movement;
    assert_eq!(
        (
            base.forward_speed,
            base.backward_speed,
            base.strafe_left_speed,
            base.strafe_right_speed
        ),
        (6.5, 3.25, 5.0, 5.0)
    );
    assert_eq!(base.control_acceleration, [65.0, 65.0, 0.0]);
    assert_eq!(base.velocity_damper, [0.2, 0.2, 1.0]);
    assert_eq!(
        (
            base.ramp_time,
            base.gravity,
            base.terminal_velocity,
            base.jump_acceleration,
            base.step_up_height
        ),
        (0.13, 0.0, 50.0, 40.0, 0.5)
    );
    assert_eq!(base.float_height, 1.35);
    // §14.6.
    let l = r.overlay(&LayerConditions::default(), MovementState::EarthNoSurface);
    assert_eq!(
        (
            l.forward_speed,
            l.backward_speed,
            l.strafe_left_speed,
            l.control_acceleration,
            l.velocity_damper
        ),
        (
            Some(5.0),
            Some(1.0),
            Some(5.0),
            Some([10.0, 10.0, 0.0]),
            Some([1.0; 3])
        )
    );
    assert_eq!(
        (
            l.ramp_time,
            l.gravity,
            l.terminal_velocity,
            l.jump_acceleration,
            l.step_up_height
        ),
        (Some(0.01), Some(15.0), Some(50.0), Some(0.0), Some(0.5))
    );
    // §11.1 example.
    let s = r
        .compose(
            &LayerConditions::default(),
            MovementState::WaterNoSurface,
            &pids,
        )
        .movement;
    assert_eq!(
        (s.forward_speed, s.backward_speed, s.strafe_left_speed),
        (3.2, 2.0, 2.0)
    );
    assert_eq!(s.control_acceleration, [65.0, 65.0, 0.0]);
    assert_eq!(s.velocity_damper, [0.4; 3]);
    assert_eq!(
        (
            s.ramp_time,
            s.gravity,
            s.terminal_velocity,
            s.jump_acceleration
        ),
        (0.4, -20.0, 50.0, 0.0)
    );
    // MoveTo: the float height is written into the MoveTo set too (§11.2).
    let c = r.compose(
        &LayerConditions {
            avatar_float_height: Some(0.92),
            ..LayerConditions::default()
        },
        MovementState::EarthControlledSurface,
        &pids,
    );
    assert_eq!(
        c.move_to
            .get(pids.move_to_float_height.unwrap())
            .and_then(tessera_props::Value::as_f32),
        Some(0.92)
    );
    eprintln!("BasePlayer MoveTo fields: {}", c.move_to.fields.len());
}

#[test]
fn generic_controller_set() {
    let Some(data) = game_data() else { return };
    let l = controller_set(data, ControllerKind::Generic)
        .expect("read")
        .expect("Generic set (§10.2)");
    assert_eq!(
        (
            l.forward_speed,
            l.backward_speed,
            l.strafe_left_speed,
            l.strafe_right_speed
        ),
        (Some(4.5), Some(2.5), Some(4.5), Some(4.5))
    );
    assert_eq!(l.control_acceleration, Some([35.0, 25.0, 0.0]));
    assert_eq!(l.velocity_damper, Some([0.4; 3]));
    assert_eq!(
        (
            l.ramp_time,
            l.gravity,
            l.terminal_velocity,
            l.jump_acceleration,
            l.step_up_height,
            l.float_height
        ),
        (
            Some(0.3),
            Some(40.0),
            Some(60.0),
            Some(230.0),
            Some(0.4),
            Some(0.0)
        )
    );
    assert_eq!(
        (l.force_strafe, l.allow_contact_loss),
        (Some(false), Some(false))
    );
    let mounted = controller_set(data, ControllerKind::Mounted)
        .expect("read")
        .expect("Mounted set");
    assert_eq!(mounted, l, "0x780010B1 holds the Generic values (§10.2)");
}

// ---------------------------------------------------------------------------------------
// Random walks.

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) as u32
    }
    fn unit(&mut self) -> f32 {
        self.next() as f32 / u32::MAX as f32
    }
}

fn par_each<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out = Mutex::new(Vec::with_capacity(items.len()));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(i) else { break };
                    let r = f(item);
                    out.lock().unwrap().push(r);
                }
            });
        }
    });
    out.into_inner().unwrap()
}

fn fetch(set: &ArchiveSet, did: Did) -> Result<Option<FetchedRecord>, String> {
    match set.read_with_entry(did.0) {
        Ok((l, bytes)) => Ok(Some(FetchedRecord {
            bytes,
            stamp: Some(l.entry.stamp),
        })),
        Err(tessera_dat::Error::NotFound { .. }) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Region number → its water table (`water.md` §4), from the region descriptors.
fn water_tables(data: &GameData) -> &'static HashMap<u32, WaterTable> {
    static TABLES: OnceLock<HashMap<u32, WaterTable>> = OnceLock::new();
    TABLES.get_or_init(|| {
        let set = data.source();
        let mut out = HashMap::new();
        for did in dids_of(set, 0x0F, |_| true) {
            let Ok((_, bytes)) = set.read_with_entry(did) else {
                continue;
            };
            let Ok(d) = RegionDescriptor::parse_expecting(&bytes, did) else {
                continue;
            };
            let Some(table) = d.water_table() else {
                continue;
            };
            if let Ok((_, bytes)) = set.read_with_entry(table.0)
                && let Ok(t) = decode_water_table(&bytes)
            {
                out.insert(d.region, t);
            }
        }
        out
    })
}

/// The landblock's water height *h* by `water.md` §6.2, or `None` when it is dry.
fn landblock_water(data: &GameData, info_did: u32, land: &LandblockData) -> Option<f32> {
    let region = (info_did >> 16) & 0xF;
    let table = water_tables(data).get(&region)?;
    let (_, bytes) = data.source().read_with_entry(info_did - 0x0010_0000).ok()?;
    let map = BlockMap::parse(&bytes).ok()?;
    let t = map.get(table.type_key)?;
    let entry = table.select(u32::from(t))?;
    if entry.is_dry() {
        return None;
    }
    let level = map.get(table.level_key)?;
    // §6.2 step 6: f32(L) × 1000.0, then × the f32 nearest 1/255.
    let h = (f32::from(level) * 1000.0) * (1.0f32 / 255.0);
    let lowest = tessera_formats::landblock::height_from_raw(land.heights.min_max().0);
    (h > lowest).then_some(h)
}

/// Adds the water plane at `h` and the three bands of the landblock volume entity
/// (`water.md` §7.1, entity 0x47000754) placed at (0, 0, h).
fn add_water(b: &mut CollisionSetBuilder, h: f32) {
    let flags = |surface, wading, deep| LiquidFlags {
        liquid: true,
        surface,
        wading,
        deep,
        ..LiquidFlags::default()
    };
    let t = Transform::new(
        [0.0, 0.0, h],
        tessera_formats::setup::Quaternion::IDENTITY,
        1.0,
    );
    for (min_z, max_z, f) in [
        (-0.5, 0.0, flags(true, false, false)),
        (-1.0, -0.5, flags(false, true, false)),
        (-1600.0, -1.0, flags(false, false, true)),
    ] {
        b.add_liquid_box(
            LiquidBox::new([0.0, 0.0, min_z], [160.0, 160.0, max_z], &t, f).expect("band"),
        )
        .expect("liquid box");
    }
    b.set_water_plane(Some(WaterPlane::new(h, 1).expect("plane")));
}

/// The landblock's collision set (terrain, ceiling, statics, cells, water), its terrain
/// data and its water height.
fn build(
    data: &GameData,
    info_did: u32,
    cache: &ObjectCollisionCache,
    cells: &Mutex<HashMap<Did, Option<Arc<CellGeometry>>>>,
) -> Option<(CollisionSet, LandblockData, Option<f32>)> {
    let set = data.source();
    let dict = dictionary(data);
    let pids = PhysicsPids::from_dictionary(dict);
    let (l, bytes) = set.read_with_entry(info_did).ok()?;
    let info = LandblockInfo::parse_expecting(&bytes, info_did, Some(l.entry.stamp), dict)
        .unwrap_or_else(|e| panic!("{info_did:#010x}: {e}"));
    let (l, bytes) = set.read_with_entry(info_did - 0x0020_0000).ok()?;
    let land = LandblockData::parse(&bytes, Some(l.entry.stamp)).expect("landblock data");
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.add_grid(Arc::new(GridMesh::terrain(&land)))
        .expect("terrain");
    if let Some(c) = GridMesh::ceiling(&land) {
        b.add_grid(Arc::new(c)).expect("ceiling");
    }
    let resolve = |o: Did| {
        cache.get_or_resolve(o, |o| {
            resolve_object_collision(o, |d| fetch(set, d), dict, &pids)
        })
    };
    b.add_statics(&info, &pids, &StaticOptions::default(), resolve);
    b.add_cells(&info, &pids, |did| {
        if did.prefix() != CELL_GEOMETRY_PREFIX {
            return None;
        }
        let mut cache = cells.lock().unwrap();
        cache
            .entry(did)
            .or_insert_with(|| {
                let (l, bytes) = set.read_with_entry(did.0).ok()?;
                CellGeometry::parse_expecting(&bytes, did.0, Some(l.entry.stamp), dict)
                    .ok()
                    .map(Arc::new)
            })
            .clone()
    });
    let water = landblock_water(data, info_did, &land);
    if let Some(h) = water {
        add_water(&mut b, h);
    }
    Some((b.build(), land, water))
}

#[derive(Debug, Default, Clone)]
struct Tally {
    landblocks: usize,
    walks: usize,
    steps: usize,
    reverted: usize,
    non_finite: usize,
    deep_penetrations: usize,
    deepest: f32,
    fall_throughs: usize,
    pushed_steps: usize,
    step_ups: usize,
    states: [usize; 7],
    in_cell_steps: usize,
    left_block: usize,
    no_spawn: usize,
    wet_landblocks: usize,
    swim_steps: usize,
    below_float_line: usize,
    lowest_swim: f64,
    cell_spawns: usize,
}

impl Tally {
    fn add(&mut self, o: &Self) {
        self.landblocks += o.landblocks;
        self.walks += o.walks;
        self.steps += o.steps;
        self.reverted += o.reverted;
        self.non_finite += o.non_finite;
        self.deep_penetrations += o.deep_penetrations;
        self.deepest = self.deepest.min(o.deepest);
        self.fall_throughs += o.fall_throughs;
        self.pushed_steps += o.pushed_steps;
        self.step_ups += o.step_ups;
        for i in 0..7 {
            self.states[i] += o.states[i];
        }
        self.in_cell_steps += o.in_cell_steps;
        self.left_block += o.left_block;
        self.no_spawn += o.no_spawn;
        self.wet_landblocks += o.wet_landblocks;
        self.swim_steps += o.swim_steps;
        self.below_float_line += o.below_float_line;
        self.lowest_swim = self.lowest_swim.min(o.lowest_swim);
        self.cell_spawns += o.cell_spawns;
    }
}

/// One seeded walk; returns the trajectory's bits and adds to `t`.
#[allow(clippy::too_many_arguments)]
fn walk(
    set: &CollisionSet,
    land: &LandblockData,
    water: Option<f32>,
    in_cell: bool,
    record: &Arc<PlayerMovementRecord>,
    pids: &Arc<MovementPids>,
    seed: u64,
    seconds: usize,
    t: &mut Tally,
) -> Vec<[u64; 3]> {
    let mut g = Lcg(seed);
    let terrain = set.terrain().expect("terrain").clone();
    let ground = |x: f32, y: f32| {
        terrain
            .key_under(x, y, &land.vertex_flags)
            .and_then(|k| terrain.plane(k))
            .and_then(|p| p.height_at(x, y))
    };
    // A spawn 30 m above the terrain (and above the water) whose capsule touches nothing
    // (not inside a hill mesh or a building wall), or 0.5 m above a cell's origin.
    let shape = ControllerConfig::default().shape;
    let free = |p: [f32; 3]| {
        let mut near = Vec::new();
        set.contacts(
            &shape.capsule_at(p),
            0.05,
            &QueryFilter::player(),
            &mut near,
        );
        near.is_empty()
    };
    let mut spawn = None;
    let mut spawned_in_cell = false;
    let cells = set.cells().cells();
    if in_cell && !cells.is_empty() {
        for _ in 0..8 {
            let c = &cells[g.next() as usize % cells.len()];
            let p = c.placement.position;
            let p = [p[0], p[1], p[2] + 0.5];
            if (20.0..=140.0).contains(&p[0]) && (20.0..=140.0).contains(&p[1]) && free(p) {
                spawn = Some((p[0], p[1], p[2]));
                spawned_in_cell = true;
                t.cell_spawns += 1;
                break;
            }
        }
    }
    for _ in 0..32 {
        if spawn.is_some() {
            break;
        }
        let (x, y) = (20.0 + 120.0 * g.unit(), 20.0 + 120.0 * g.unit());
        let z = ground(x, y).expect("ground").max(water.unwrap_or(f32::MIN)) + 30.0;
        if free([x, y, z]) {
            spawn = Some((x, y, z));
        }
    }
    let Some((x, y, start_z)) = spawn else {
        t.no_spawn += 1;
        return Vec::new();
    };
    let mut c = Controller::new(
        ControllerConfig::default(),
        Arc::clone(record),
        Arc::clone(pids),
        LayerConditions::default(),
        MoverState::at(
            [f64::from(x), f64::from(y), f64::from(start_z)],
            g.unit() * std::f32::consts::TAU,
        ),
    );
    let env = StepEnv::new(set, [0.0; 3]).with_water(set);
    let has_ceiling = set.ceiling().is_some();
    let mut input = MoveInput::default();
    let mut out = Vec::new();
    t.walks += 1;
    for k in 0..seconds * 60 {
        if k % 60 == 0 {
            input = MoveInput {
                forward: [1.0, 1.0, 0.5, -1.0, 0.0][(g.next() % 5) as usize],
                strafe: [0.0, 0.0, 1.0, -1.0][(g.next() % 4) as usize],
                heading: Some(g.unit() * std::f32::consts::TAU - std::f32::consts::PI),
                jump: false,
            };
        }
        let step_input = MoveInput {
            jump: g.next().is_multiple_of(120),
            ..input
        };
        let swimming_before = c.state().element == MovementState::WaterNoSurface;
        let z_before = c.state().position[2];
        let r = c.step(&step_input, &env);
        t.steps += 1;
        let s = *c.state();
        if r.outcome == StepOutcome::Reverted {
            t.reverted += 1;
        }
        if !s.is_finite() {
            t.non_finite += 1;
            break;
        }
        if r.pushed_out > 0.0 {
            t.pushed_steps += 1;
        }
        if r.stepped_up {
            t.step_ups += 1;
        }
        t.states[s.element.key() as usize] += 1;
        if s.element == MovementState::WaterNoSurface
            && let Some(w) = s.water
        {
            t.swim_steps += 1;
            let below = s.position[2] - (w.surface - 1.35);
            t.lowest_swim = t.lowest_swim.min(below);
            // A swimmer may end up below the line by one step's descent (a wader snapped
            // down a slope into deeper water), but never sinks further while below it.
            if below < -0.05 && swimming_before && s.position[2] < z_before - 1e-4 {
                t.below_float_line += 1;
            }
        }
        let feet = s.position.map(|v| v as f32);
        let cap = c.config().shape.capsule_at(feet);
        let mut contacts = Vec::new();
        set.contacts(&cap, 0.0, &QueryFilter::player(), &mut contacts);
        let deepest = contacts.iter().map(|c| c.separation).fold(0.0, f32::min);
        t.deepest = t.deepest.min(deepest);
        if deepest < -0.01 {
            t.deep_penetrations += 1;
        }
        if s.cell.is_some() {
            t.in_cell_steps += 1;
        } else if !has_ceiling
            && !spawned_in_cell
            && let Some(h) = ground(feet[0], feet[1])
            && feet[2] < h - 0.05
        {
            t.fall_throughs += 1;
        }
        out.push(s.position.map(f64::to_bits));
        if !(2.0..=158.0).contains(&feet[0]) || !(2.0..=158.0).contains(&feet[1]) {
            // Neighbouring landblocks are not loaded here.
            t.left_block += 1;
            break;
        }
    }
    out
}

#[test]
fn random_walks_on_real_landblocks() {
    let Some(data) = game_data() else { return };
    let Some((_, rec)) = record() else {
        panic!("no player movement record");
    };
    let record = Arc::new(rec.clone());
    let pids = Arc::new(pids().expect("pids"));
    let set = data.source();
    let infos: Vec<u32> = dids_of(set, 0x80, |k| matches!(k, ArchiveKind::Cell(_)))
        .into_iter()
        .filter(|d| (d >> 20) & 0xF == 2)
        .collect();
    let (every, walks, seconds) = if exhaustive() {
        (211, 8, 30)
    } else {
        (39_989, 2, 4)
    };
    let picked: Vec<u32> = infos.iter().step_by(every).copied().collect();
    let cache = ObjectCollisionCache::default();
    let cells = Mutex::new(HashMap::new());
    let started = Instant::now();
    let results = par_each(&picked, |did| {
        let mut t = Tally::default();
        let Some((s, land, water)) = build(data, *did, &cache, &cells) else {
            return t;
        };
        t.landblocks = 1;
        t.wet_landblocks = usize::from(water.is_some());
        for w in 0..walks {
            let seed = u64::from(*did) * 31 + w as u64;
            let in_cell = w % 4 == 3;
            let a = walk(
                &s, &land, water, in_cell, &record, &pids, seed, seconds, &mut t,
            );
            if w == 0 {
                // Bit-for-bit repeat.
                let mut again = Tally::default();
                let b = walk(
                    &s, &land, water, in_cell, &record, &pids, seed, seconds, &mut again,
                );
                assert_eq!(a, b, "{did:#010x}: walk {w} repeats");
            }
        }
        t
    });
    let mut total = Tally::default();
    for r in &results {
        total.add(r);
    }
    eprintln!(
        "{} infos, {} sampled; {total:?}; {:.1} s",
        infos.len(),
        picked.len(),
        started.elapsed().as_secs_f64()
    );
    assert!(total.landblocks > 0 && total.steps > 0);
    assert_eq!(total.non_finite, 0, "non-finite states");
    assert_eq!(total.reverted, 0, "reverted steps");
    assert_eq!(total.deep_penetrations, 0, "penetration deeper than 0.01 m");
    assert_eq!(total.fall_throughs, 0, "fell through the terrain");
    assert_eq!(
        total.below_float_line, 0,
        "swimmers sinking below the float line (lowest {})",
        total.lowest_swim
    );
    assert!(
        total.lowest_swim > -0.5,
        "lowest swimmer {}",
        total.lowest_swim
    );
    if total.wet_landblocks > 0 && exhaustive() {
        assert!(
            total.states[1] + total.states[2] + total.states[3] > 0,
            "water states 1–3 occur on {} wet landblocks",
            total.wet_landblocks
        );
    }
    if total.cell_spawns > 0 {
        assert!(total.in_cell_steps > 0, "cell lookup reports cells");
    }
}
