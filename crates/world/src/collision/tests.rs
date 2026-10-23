// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests of collision residency on synthetic records.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tessera_assets::{AssetError, Priority};
use tessera_formats::collision::{CellGeometry, CellPlane, CellPortal, PlaneListEnd};
use tessera_formats::landblock::{Cell, CellNeighbour, LandblockInfo, TSizeList};
use tessera_formats::{Did, LandblockData, Position, PropertySet, TSize};
use tessera_physics::geometry::{
    BodySource, CollisionLimits, GeneratedObject, PhysicsPids, StaticOptions, Transform,
};

use super::*;
use crate::source::{MemoryWorldSource, StampedRecord};

pub(crate) fn key() -> LandblockKey {
    LandblockKey::new(1, 10, 20).expect("key")
}

/// A 8 × 8 × 3 room around its origin with one portal on its +X face.
pub(crate) fn room(did: u32) -> CellGeometry {
    let plane = |n: [f32; 3], d: f32| CellPlane {
        normal: n,
        distance: d,
    };
    CellGeometry {
        did: Did(did),
        physics_mesh: None,
        planes: vec![
            plane([1.0, 0.0, 0.0], 4.0),
            plane([-1.0, 0.0, 0.0], 4.0),
            plane([0.0, 1.0, 0.0], 4.0),
            plane([0.0, -1.0, 0.0], 4.0),
            plane([0.0, 0.0, 1.0], 0.0),
            plane([0.0, 0.0, -1.0], 3.0),
        ],
        plane_list_end: PlaneListEnd::Bpfl,
        portals: vec![CellPortal {
            lead: 1,
            corners: vec![
                [4.0, -1.0, 0.0],
                [4.0, 1.0, 0.0],
                [4.0, 1.0, 2.0],
                [4.0, -1.0, 2.0],
            ],
        }],
        walls: Vec::new(),
        hull: (0..8)
            .map(|k| {
                [
                    if k & 1 == 0 { -4.0 } else { 4.0 },
                    if k & 2 == 0 { -4.0 } else { 4.0 },
                    if k & 4 == 0 { 0.0 } else { 3.0 },
                ]
            })
            .collect(),
        properties: PropertySet::default(),
        render_mesh: Vec::new(),
        layout_rule: tessera_formats::landblock::LayoutRule::default(),
    }
}

pub(crate) fn cell(
    index: u16,
    geometry: u32,
    at: Option<[f32; 3]>,
    neighbour: Option<u16>,
) -> Cell {
    let position = at.map_or_else(
        || Position::new(0).with_region(1),
        |p| Position::new(0).with_region(1).with_offset(p),
    );
    Cell {
        index,
        position,
        geometry: Did(geometry),
        neighbours: neighbour
            .map(|n| CellNeighbour {
                ordinal: 0,
                cell: n,
                back_reference: 0,
                reserved: [0; 3],
                flag: 1,
            })
            .into_iter()
            .collect(),
        ..Cell::default()
    }
}

pub(crate) fn info(cells: Vec<Cell>) -> LandblockInfo {
    LandblockInfo {
        did: key().info_did(),
        flags: u32::from(!cells.is_empty()),
        size_hint: 0,
        cross_links: None,
        header_indices: None,
        cells: (!cells.is_empty()).then(|| TSizeList {
            size: TSize {
                hint: 0,
                count: u32::try_from(cells.len()).expect("count"),
            },
            items: cells,
        }),
        reserved_a: 0,
        entities: Vec::new(),
        links: Vec::new(),
        properties: PropertySet::default(),
        reserved_b: 0,
        weenies: TSizeList::default(),
        trailer: None,
        layout_rule: tessera_formats::landblock::LayoutRule::default(),
    }
}

pub(crate) fn system() -> CollisionSystem {
    CollisionSystem::new(
        PhysicsPids::default(),
        CollisionLimits::default(),
        StaticOptions::default(),
    )
}

/// Builds `info`'s collision with `sys` over an empty source.
fn build_cells(sys: &CollisionSystem, info: &LandblockInfo) -> BlockCollision {
    let src = MemoryWorldSource::new();
    let schema: HashMap<u32, tessera_formats::PropertyType> = HashMap::new();
    let data = LandblockData::from_heights(
        key().data_did(),
        tessera_formats::landblock::HeightGrid::flat(0),
    );
    sys.build(&CollisionInput {
        key: key(),
        data: &data,
        terrain: None,
        info: Some(info),
        generated: &[],
        water: None,
        source: &src,
        schema: &schema,
    })
}

#[test]
fn portals_are_placed_in_the_landblock_frame() {
    let sys = system();
    sys.seed_cell_geometry(room(0x0100_0001));
    let info = info(vec![
        cell(0x100, 0x0100_0001, Some([50.0, 60.0, 10.0]), Some(0x101)),
        cell(0x101, 0x0100_0001, Some([58.0, 60.0, 10.0]), Some(0x100)),
        // No offset: at the landblock origin (`landblock.md` §4.4, L42).
        cell(0x102, 0x0100_0001, None, Some(0x100)),
        // A geometry that is missing: kept with links, no corners.
        cell(0x103, 0x0100_0009, Some([0.0; 3]), Some(0x100)),
    ]);
    let built = build_cells(&sys, &info);
    assert!(built.complete);
    assert_eq!(built.portals_left_out, 0);
    let cells = &built.cells;
    assert_eq!(cells.len(), 4);
    let c = &cells[0];
    assert_eq!((c.index, c.geometry_available), (0x100, true));
    assert_eq!(c.portals.len(), 1);
    let p = &c.portals[0];
    assert_eq!((p.ordinal, p.neighbour, p.flag), (0, 0x101, 1));
    assert_eq!(
        p.corners,
        vec![
            [54.0, 59.0, 10.0],
            [54.0, 61.0, 10.0],
            [54.0, 61.0, 12.0],
            [54.0, 59.0, 12.0]
        ]
    );
    let origin = cells[2].placement.expect("placed at the origin");
    assert_eq!(origin.position, [0.0; 3]);
    assert_eq!(cells[2].portals[0].corners.len(), 4);
    assert!(!cells[3].geometry_available);
    assert_eq!(cells[3].portals[0].neighbour, 0x100);
    assert!(cells[3].portals[0].corners.is_empty());
    // The bounds of the three placed rooms, in the landblock frame.
    let b = built.cell_bounds.expect("placed cells");
    assert_eq!((b.min, b.max), ([-4.0, -4.0, 0.0], [62.0, 64.0, 13.0]));
    assert_eq!(built.cell_at([1.0, 1.0, 1.0]).map(|c| c.index), Some(0x102));
    assert_eq!(
        built.cell_at([50.0, 60.0, 11.0]).map(|c| c.index),
        Some(0x100)
    );
}

/// A room like [`room`] with `portals` portals of `corners` corners each.
fn room_with_portals(did: u32, portals: usize, corners: usize) -> CellGeometry {
    CellGeometry {
        portals: (0..portals)
            .map(|_| CellPortal {
                lead: 1,
                corners: vec![[4.0, 0.0, 1.0]; corners],
            })
            .collect(),
        ..room(did)
    }
}

/// A cell with `n` neighbour entries (all naming cell 0).
fn cell_with_neighbours(index: u16, geometry: u32, at: [f32; 3], n: usize) -> Cell {
    Cell {
        neighbours: (0..n)
            .map(|_| CellNeighbour {
                ordinal: 0,
                cell: 0,
                back_reference: 0,
                reserved: [0; 3],
                flag: 1,
            })
            .collect(),
        ..cell(index, geometry, Some(at), None)
    }
}

#[test]
fn portals_only_for_accepted_cells_and_within_the_set_budgets() {
    // Corners only for cells the set accepted: against an empty set none gets any.
    let geometries: HashMap<Did, Arc<CellGeometry>> = [room(0x0100_0001)]
        .into_iter()
        .map(|g| (g.did, Arc::new(g)))
        .collect();
    let two = info(vec![
        cell(0x100, 0x0100_0001, Some([50.0, 60.0, 10.0]), Some(0x101)),
        cell(0x101, 0x0100_0001, Some([58.0, 60.0, 10.0]), Some(0x100)),
    ]);
    let empty =
        tessera_physics::geometry::CollisionSetBuilder::new(CollisionLimits::default()).build();
    let (cells, left_out) = cell_portals(&two, &empty, 16, |d| geometries.get(&d).cloned());
    assert_eq!((cells.len(), left_out), (2, 0));
    assert!(cells.iter().all(|c| c.portals[0].corners.is_empty()));
    // The set's cell limit bounds the cells kept; the links of the rest are counted.
    let (cells, left_out) = cell_portals(&two, &empty, 1, |d| geometries.get(&d).cloned());
    assert_eq!((cells.len(), left_out), (1, 1));

    // The per-set corner budget: 1,024 portals of 64 corners per cell, so the fifth cell's
    // portals keep their links without polygons.
    let sys = system();
    sys.seed_cell_geometry(room_with_portals(0x0100_0002, 1_024, 64));
    let cells: Vec<Cell> = (0..5u16)
        .map(|i| cell_with_neighbours(i, 0x0100_0002, [10.0 * f32::from(i), 0.0, 0.0], 1_024))
        .collect();
    let built = build_cells(&sys, &info(cells));
    let corners: usize = built
        .cells
        .iter()
        .flat_map(|c| &c.portals)
        .map(|p| p.corners.len())
        .sum();
    assert_eq!(corners, MAX_SET_PORTAL_CORNERS);
    assert_eq!(built.portals_left_out, 1_024);
    assert!(built.cells[4].portals.iter().all(|p| p.corners.is_empty()));
    assert_eq!(built.cells[4].portals.len(), 1_024, "links are kept");

    // The per-set link budget: 65 cells of 1,024 neighbours (without geometry).
    let many: Vec<Cell> = (0..65u16)
        .map(|i| cell_with_neighbours(i, 0x0100_0009, [0.0; 3], 1_024))
        .collect();
    let built = build_cells(&system(), &info(many));
    let links: usize = built.cells.iter().map(|c| c.portals.len()).sum();
    assert_eq!(links, MAX_SET_PORTALS);
    assert_eq!(built.portals_left_out, 1_024);
}

/// A source whose reads of DIDs with prefix `prefix` fail with a full queue (not the
/// data's fault) while `flaky` is set.
pub(crate) struct FlakySource {
    pub inner: MemoryWorldSource,
    pub prefix: u8,
    pub flaky: AtomicBool,
}

impl WorldSource for FlakySource {
    fn read_stamped(&self, did: Did) -> Result<Option<StampedRecord>, AssetError> {
        if did.prefix() == self.prefix && self.flaky.load(Ordering::SeqCst) {
            return Err(AssetError::QueueFull { capacity: 0 });
        }
        self.inner.read_stamped(did)
    }
}

#[test]
fn transient_object_reads_are_not_cached() {
    let src = FlakySource {
        inner: crate::testkit::SyntheticWorld::rectangle(1, 0..=0, 0..=0).build(),
        prefix: 0x04,
        flaky: AtomicBool::new(true),
    };
    let schema = crate::testkit::schema();
    let sys = system();
    let obj = Did(crate::testkit::obj::TREE);
    let a = sys.object_collision(&src, &schema, obj);
    assert!(a.error.is_some() && a.setup.is_none(), "{a:?}");
    assert!(
        sys.object_cache().is_empty(),
        "a transient failure is not cached"
    );
    src.flaky.store(false, Ordering::SeqCst);
    let b = sys.object_collision(&src, &schema, obj);
    assert!(!Arc::ptr_eq(&a, &b));
    assert_eq!(b.setup, Some(Did(crate::testkit::obj::TREE_SETUP)));
    assert_eq!(sys.object_cache().len(), 1);
    let c = sys.object_collision(&src, &schema, obj);
    assert!(Arc::ptr_eq(&b, &c), "a readable result is cached");
    sys.object_cache().clear();
    assert!(sys.object_cache().is_empty() && sys.object_cache().bytes() == 0);
}

#[test]
fn transient_reads_make_the_set_incomplete() {
    let schema: HashMap<u32, tessera_formats::PropertyType> = HashMap::new();
    let data = LandblockData::from_heights(
        key().data_did(),
        tessera_formats::landblock::HeightGrid::flat(0),
    );
    let info = info(vec![cell(
        0x100,
        0x0100_0001,
        Some([50.0, 60.0, 10.0]),
        None,
    )]);
    let src = FlakySource {
        inner: MemoryWorldSource::new(),
        prefix: 0x01,
        flaky: AtomicBool::new(true),
    };
    let sys = system();
    let build = |sys: &CollisionSystem| {
        sys.build(&CollisionInput {
            key: key(),
            data: &data,
            terrain: None,
            info: Some(&info),
            generated: &[],
            water: None,
            source: &src,
            schema: &schema,
        })
    };
    let built = build(&sys);
    assert!(!built.complete);
    assert_eq!(sys.cell_cache_len(), 0, "not cached");
    // Readable again (the record is missing: the data's answer, cached).
    src.flaky.store(false, Ordering::SeqCst);
    let built = build(&sys);
    assert!(built.complete);
    assert_eq!(sys.cell_cache_len(), 1);
    // A generated object whose chain cannot be read now also makes it incomplete.
    let objects = FlakySource {
        inner: crate::testkit::SyntheticWorld::rectangle(1, 0..=0, 0..=0).build(),
        prefix: 0x47,
        flaky: AtomicBool::new(true),
    };
    let generated = [GeneratedObject {
        object: Did(crate::testkit::obj::TREE),
        transform: Transform {
            position: [10.0, 10.0, 0.0],
            rotation: tessera_formats::setup::Quaternion::IDENTITY,
            scale: [1.0; 3],
        },
    }];
    let built = sys.build(&CollisionInput {
        key: key(),
        data: &data,
        terrain: None,
        info: None,
        generated: &generated,
        water: None,
        source: &objects,
        schema: &schema,
    });
    assert!(!built.complete);
}

#[test]
fn cell_cache_rejects_other_prefixes_and_caches_missing_records() {
    let sys = system();
    let src = MemoryWorldSource::new();
    let schema: HashMap<u32, tessera_formats::PropertyType> = HashMap::new();
    assert!(sys.cell_geometry(&src, &schema, Did(0x0200_0001)).is_none());
    assert_eq!(sys.cell_cache_len(), 0);
    assert!(sys.cell_geometry(&src, &schema, Did(0x0100_0001)).is_none());
    assert_eq!(sys.cell_cache_len(), 1);
    // Garbage bytes do not decode: cached as unusable, no panic.
    let mut src = MemoryWorldSource::new();
    src.insert(Did(0x0100_0002), vec![1u8, 2, 3], None);
    assert!(sys.cell_geometry(&src, &schema, Did(0x0100_0002)).is_none());
    assert_eq!(sys.cell_cache_len(), 2);
}

#[test]
fn build_has_terrain_ceiling_and_generated_report() {
    let sys = system();
    let src = MemoryWorldSource::new();
    let schema: HashMap<u32, tessera_formats::PropertyType> = HashMap::new();
    let data = LandblockData::from_heights(
        key().data_did(),
        tessera_formats::landblock::HeightGrid::flat(1_000),
    );
    // A generated object whose chain does not resolve gets no body, and is counted.
    let generated = [GeneratedObject {
        object: Did(0x4700_0001),
        transform: Transform {
            position: [10.0, 10.0, 0.0],
            rotation: tessera_formats::setup::Quaternion::IDENTITY,
            scale: [1.0; 3],
        },
    }];
    let built = sys.build(&CollisionInput {
        key: key(),
        data: &data,
        terrain: None,
        info: None,
        generated: &generated,
        water: None,
        source: &src,
        schema: &schema,
    });
    assert_eq!(built.key, key());
    assert!(built.set.terrain().is_some());
    assert_eq!(
        built.set.bodies().first().map(|b| b.source),
        Some(BodySource::Terrain)
    );
    assert_eq!(built.generated_bodies, vec![None]);
    assert_eq!(built.report().generated_seen, 1);
    assert_eq!(built.report().generated_blocking, 0);
    let id = built.set.landblock().expect("keyed");
    assert_eq!((id.region, id.x, id.y), (1, 10, 20));
    assert!(built.heap_bytes >= built.set.heap_size());
    assert!(built.cell_at([80.0, 80.0, 5.0]).is_none());
    // Tiny limits refuse the terrain without panicking.
    let tiny = CollisionSystem::new(
        PhysicsPids::default(),
        CollisionLimits {
            max_triangles: 1,
            max_rounds: 0,
            max_bodies: 0,
            max_cells: 0,
        },
        StaticOptions::default(),
    );
    let built = tiny.build(&CollisionInput {
        key: key(),
        data: &data,
        terrain: None,
        info: None,
        generated: &generated,
        water: None,
        source: &src,
        schema: &schema,
    });
    assert_eq!(built.set.triangle_count(), 0);
}

#[test]
fn build_places_cells_from_the_cache() {
    // Cells whose geometry cannot be read are refused, their links kept.
    let sys = system();
    let src = MemoryWorldSource::new();
    let schema: HashMap<u32, tessera_formats::PropertyType> = HashMap::new();
    let data = LandblockData::from_heights(
        key().data_did(),
        tessera_formats::landblock::HeightGrid::flat(0),
    );
    let info = info(vec![cell(
        0x100,
        0x0100_0001,
        Some([50.0, 60.0, 10.0]),
        None,
    )]);
    let built = sys.build(&CollisionInput {
        key: key(),
        data: &data,
        terrain: None,
        info: Some(&info),
        generated: &[],
        water: None,
        source: &src,
        schema: &schema,
    });
    assert_eq!(built.report().cells_refused, 1);
    assert_eq!(built.cells.len(), 1);
    assert!(!built.cells[0].geometry_available);
    assert_eq!(built.cell(0x100).map(|c| c.index), Some(0x100));
}

#[test]
fn hierarchy_rule_follows_the_priority() {
    // `collision.md` §12.1: Prefetch builds stay on their worker, Visible and higher draw
    // on the shared budget.
    assert_eq!(
        HierarchyThreads::for_priority(Priority::Prefetch),
        HierarchyThreads::Worker
    );
    assert_eq!(
        HierarchyThreads::for_priority(Priority::Visible),
        HierarchyThreads::Shared
    );
    assert_eq!(
        HierarchyThreads::for_priority(Priority::Immediate),
        HierarchyThreads::Shared
    );
    assert_eq!(HierarchyThreads::Worker.max_threads(), 1);
    assert_eq!(HierarchyThreads::Shared.max_threads(), MAX_BUILD_THREADS);
}

#[test]
fn prefetch_builds_stay_on_the_worker_and_give_the_same_set() {
    let sys = system();
    sys.seed_cell_geometry(room(0x0100_0001));
    let info = info(vec![cell(
        0x100,
        0x0100_0001,
        Some([50.0, 60.0, 10.0]),
        None,
    )]);
    let src = MemoryWorldSource::new();
    let schema: HashMap<u32, tessera_formats::PropertyType> = HashMap::new();
    let data = LandblockData::from_heights(
        key().data_did(),
        tessera_formats::landblock::HeightGrid::flat(0),
    );
    let input = CollisionInput {
        key: key(),
        data: &data,
        terrain: None,
        info: Some(&info),
        generated: &[],
        water: None,
        source: &src,
        schema: &schema,
    };
    assert_eq!(sys.hierarchy_builds(), HierarchyBuildCounts::default());

    let prefetch = sys.build_at(&input, Priority::Prefetch);
    assert_eq!(
        sys.hierarchy_builds(),
        HierarchyBuildCounts {
            worker: 1,
            shared: 0,
            last_max_threads: Some(1),
        }
    );
    let visible = sys.build_at(&input, Priority::Visible);
    let immediate = sys.build_at(&input, Priority::Immediate);
    let plain = sys.build(&input);
    assert_eq!(
        sys.hierarchy_builds(),
        HierarchyBuildCounts {
            worker: 1,
            shared: 3,
            last_max_threads: Some(MAX_BUILD_THREADS),
        }
    );
    // A Prefetch build after the shared ones is again given one thread.
    let again = sys.build_at(&input, Priority::Prefetch);
    assert_eq!(
        sys.hierarchy_builds(),
        HierarchyBuildCounts {
            worker: 2,
            shared: 3,
            last_max_threads: Some(1),
        }
    );
    assert_eq!(again.set.triangle_count(), prefetch.set.triangle_count());
    // The set does not depend on the rule.
    for other in [&visible, &immediate, &plain] {
        assert_eq!(prefetch.set.triangle_count(), other.set.triangle_count());
        assert_eq!(prefetch.cells, other.cells);
        assert_eq!(prefetch.heap_bytes, other.heap_bytes);
        assert_eq!(prefetch.cell_bounds, other.cell_bounds);
    }
}

/// A source that raises `raised` on every read: the cells of the set read their geometry
/// through it before the hierarchy build starts.
struct RaisingSource {
    inner: MemoryWorldSource,
    raised: Arc<AtomicBool>,
}

impl WorldSource for RaisingSource {
    fn read_stamped(&self, did: Did) -> Result<Option<StampedRecord>, AssetError> {
        self.raised.store(true, Ordering::SeqCst);
        self.inner.read_stamped(did)
    }
}

/// Builds one cell whose geometry is read through a [`RaisingSource`], with the priority
/// `before` until the read and `after` from it on. Returns the set and, for each time the
/// priority was asked, whether the read had happened by then.
fn build_with_raise(
    sys: &CollisionSystem,
    before: Priority,
    after: Priority,
) -> (BlockCollision, Vec<bool>) {
    let raised = Arc::new(AtomicBool::new(false));
    let src = RaisingSource {
        inner: MemoryWorldSource::new(),
        raised: Arc::clone(&raised),
    };
    let info = info(vec![cell(
        0x100,
        0x0100_0002,
        Some([50.0, 60.0, 10.0]),
        None,
    )]);
    let schema: HashMap<u32, tessera_formats::PropertyType> = HashMap::new();
    let data = LandblockData::from_heights(
        key().data_did(),
        tessera_formats::landblock::HeightGrid::flat(0),
    );
    let input = CollisionInput {
        key: key(),
        data: &data,
        terrain: None,
        info: Some(&info),
        generated: &[],
        water: None,
        source: &src,
        schema: &schema,
    };
    let asks = std::sync::Mutex::new(Vec::new());
    let priority = || {
        let r = raised.load(Ordering::SeqCst);
        asks.lock().expect("asks").push(r);
        if r { after } else { before }
    };
    let built = sys.build_at(&input, &priority as &dyn Fn() -> Priority);
    (built, asks.into_inner().expect("asks"))
}

#[test]
fn priority_is_read_when_the_hierarchy_build_starts() {
    // `collision.md` §12.1: a job raised from Prefetch to Visible after `build_at` was
    // entered but before its hierarchy build starts builds with the shared budget.
    let sys = system();
    let (raised, asks) = build_with_raise(&sys, Priority::Prefetch, Priority::Visible);
    assert_eq!(asks, vec![true], "asked once, after the cell reads");
    assert_eq!(
        sys.hierarchy_builds(),
        HierarchyBuildCounts {
            worker: 0,
            shared: 1,
            last_max_threads: Some(MAX_BUILD_THREADS),
        }
    );
    // Lowered to Prefetch in the same window: the build stays on the worker.
    let sys = system();
    let (lowered, asks) = build_with_raise(&sys, Priority::Visible, Priority::Prefetch);
    assert_eq!(asks, vec![true]);
    assert_eq!(
        sys.hierarchy_builds(),
        HierarchyBuildCounts {
            worker: 1,
            shared: 0,
            last_max_threads: Some(1),
        }
    );
    // The set does not depend on the rule.
    assert_eq!(raised.set.triangle_count(), lowered.set.triangle_count());
    assert_eq!(raised.cells, lowered.cells);
    assert_eq!(raised.heap_bytes, lowered.heap_bytes);
}

#[test]
fn priority_sources_of_every_kind_choose_the_rule() {
    let sys = system();
    let info = info(Vec::new());
    let src = MemoryWorldSource::new();
    let schema: HashMap<u32, tessera_formats::PropertyType> = HashMap::new();
    let data = LandblockData::from_heights(
        key().data_did(),
        tessera_formats::landblock::HeightGrid::flat(0),
    );
    let input = CollisionInput {
        key: key(),
        data: &data,
        terrain: None,
        info: Some(&info),
        generated: &[],
        water: None,
        source: &src,
        schema: &schema,
    };
    let prefetch = tessera_assets::JobContext::detached_at(Priority::Prefetch);
    let _ = sys.build_at(&input, &prefetch);
    let _ = sys.build_at(
        &input,
        tessera_assets::JobContext::detached_at(Priority::Visible),
    );
    let _ = sys.build_at(&input, || Priority::Prefetch);
    let _ = sys.build_at(&input, Priority::Immediate);
    // No job priority counts as Visible.
    let _ = sys.build(&input);
    assert_eq!(
        sys.hierarchy_builds(),
        HierarchyBuildCounts {
            worker: 2,
            shared: 3,
            last_max_threads: Some(MAX_BUILD_THREADS),
        }
    );
}
