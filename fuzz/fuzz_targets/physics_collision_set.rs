// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-physics` geometry (`docs/specs/collision.md` §5.4, §5.8.1, §6, §8, §9, §12.1):
//! collision sets built from hostile geometry, then queried. Grid heights and vertex flags,
//! triangle lists with out-of-range indices, boxes, spheres and capsules (NaN, infinite,
//! inverted, negative radii), transforms (zero rotations, zero or huge scales), cell volumes
//! and hulls, a 0x66 record decoded from the input, and the query capsules, sweeps, rays,
//! contacts, radius and plane queries and cell lookups all come from the input. With header
//! bit 0x80 the set is also built the way `tessera-world` builds it: a landblock-info value
//! (static entities with fuzzed kinds, placements, flags, scales and content layers; cells
//! with fuzzed geometry) and an in-memory archive of 0x47 / 0x1F / 0x04 / 0x66 / 0x78
//! records, each either synthetic or raw input bytes, go through `add_statics`,
//! `add_generated`, `add_cells` and `resolve_object_collision`. Nothing may panic; every
//! reported hit must be finite with a fraction in [0, 1]; and in moderate coordinates a sweep
//! must never end deeper in a body than it started (more than half a skin of f32 rounding
//! inside, for a body it did not overlap at the start).
//!
//! Input layout: a 4-byte header (counts and switches), then the bytes every value is read
//! from (cycled when short).

#![no_main]

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use libfuzzer_sys::fuzz_target;
use tessera_formats::collision::{CellGeometry, CellPlane, CollisionMesh, PlaneListEnd, TreeBlock};
use tessera_formats::landblock::{
    Cell, GRID_SAMPLES, GridSurface, HeightGrid, LandblockInfo, LayoutRule, Placement,
    StaticEntity, TSizeList, VertexFlags,
};
use tessera_formats::reader::{TSIZE_BUCKET_COUNTS, write_vle};
use tessera_formats::setup::{
    CollisionBox, CollisionCapsule, CollisionSphere, ENTITY_FLAGS_REQUIRED, EntityHeader, Frame,
    Quaternion, SETUP_SECTION_DID_LIST, SetupSection, SetupSectionEntry,
};
use tessera_formats::{
    Did, EntityDescriptor, Position, PropertySet, PropertyType, Setup, TSize, VisualTemplate,
};
use tessera_physics::geometry::{
    BodyId, BodyPurpose, BodySource, Capsule, CellPlacement, CellVolume, CollisionFlags,
    CollisionLimits, CollisionQuery, CollisionSet, CollisionSetBuilder, CollisionView,
    FetchedRecord, GeneratedObject, GridMesh, ObjectCollisionCache, PhysicsPids, PlacedCell,
    QueryFilter, SKIN, ShapeTemplate, StaticOptions, Transform, resolve_object_collision,
};

/// Reads little-endian values from the input, cycling, zero when it is empty.
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
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.byte(), self.byte()])
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.byte(), self.byte(), self.byte(), self.byte()])
    }
    /// Mostly moderate values, sometimes raw floats (NaN, inf, huge, subnormal).
    fn f(&mut self) -> f32 {
        let tag = self.byte();
        if tag & 0x7 == 0 {
            f32::from_bits(self.u32())
        } else {
            f32::from(self.u16()) / 65_535.0 * 200.0 - 20.0
        }
    }
    fn v3(&mut self) -> [f32; 3] {
        [self.f(), self.f(), self.f()]
    }
    fn small(&mut self) -> f32 {
        f32::from(self.byte()) / 64.0
    }
    fn quat(&mut self) -> Quaternion {
        match self.byte() % 4 {
            0 => Quaternion::IDENTITY,
            1 => Quaternion::from_yaw(self.small()),
            2 => Quaternion {
                w: 0.0,
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            _ => Quaternion {
                w: self.f(),
                x: self.f(),
                y: self.f(),
                z: self.f(),
            },
        }
    }
}

fn moderate(v: [f32; 3]) -> bool {
    v.iter().all(|c| c.is_finite() && c.abs() < 1000.0)
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }
    let (head, rest) = data.split_at(4);
    let mut b = Bytes { data: rest, pos: 0 };
    let limits = CollisionLimits {
        max_triangles: 8192,
        max_rounds: 64,
        max_bodies: 32,
        max_cells: 16,
    };
    let mut builder = CollisionSetBuilder::new(limits);

    // Grid meshes.
    let mut raw = [0u16; GRID_SAMPLES];
    let mut flags = VertexFlags::filled(0);
    if head[0] & 1 != 0 {
        for h in &mut raw {
            *h = b.u16();
        }
        for f in flags.raw_mut().iter_mut() {
            *f = b.byte();
        }
    }
    let heights = HeightGrid::from_raw(raw);
    let terrain = Arc::new(GridMesh::from_grid(GridSurface::Terrain, &heights, &flags));
    // The grids are large next to everything else; add them to the set only on request so
    // most executions stay fast.
    if head[0] & 0x1C == 0x1C {
        let _ = builder.add_grid(Arc::clone(&terrain));
    }
    if head[0] & 0x62 == 0x62 {
        let _ = builder.add_grid(Arc::new(GridMesh::from_grid(
            GridSurface::Ceiling,
            &heights,
            &flags,
        )));
    }

    // Objects: triangle lists, setups with volumes, a decoded 0x66 record.
    for k in 0..usize::from(head[1] % 6) {
        let mut setup = Setup::default();
        for _ in 0..b.byte() % 3 {
            setup.boxes.push(CollisionBox {
                min: b.v3(),
                max: b.v3(),
                frame: Frame {
                    position: b.v3(),
                    scale: if b.byte() & 1 == 0 { [1.0; 3] } else { b.v3() },
                    rotation: b.quat(),
                },
            });
        }
        for _ in 0..b.byte() % 3 {
            setup.spheres.push(CollisionSphere {
                centre: b.v3(),
                radius: b.f(),
            });
        }
        for _ in 0..b.byte() % 3 {
            setup.capsules.push(CollisionCapsule {
                base: b.v3(),
                length: b.f(),
                radius: b.f(),
            });
        }
        let mesh = if b.byte().is_multiple_of(3) {
            let n = usize::from(b.byte() % 64);
            let start = b.pos % rest.len().max(1);
            let end = (start + n * 8).min(rest.len());
            CollisionMesh::parse(&rest[start..end], None).ok()
        } else {
            let positions: Vec<[f32; 3]> = (0..b.byte() % 16).map(|_| b.v3()).collect();
            let tris: Vec<[u16; 3]> = (0..b.byte() % 24)
                .map(|_| {
                    [
                        u16::from(b.byte() % 20),
                        u16::from(b.byte() % 20),
                        u16::from(b.byte() % 20),
                    ]
                })
                .collect();
            let t = ShapeTemplate::from_triangle_list(&positions, &tris).ok();
            if let Some(t) = t {
                let _ = builder.add_shape(
                    BodySource::Other { tag: k as u32 },
                    CollisionFlags::default(),
                    if b.byte() & 1 == 0 {
                        BodyPurpose::Movement
                    } else {
                        BodyPurpose::Detail
                    },
                    &t,
                );
            }
            None
        };
        let Ok(template) = ShapeTemplate::from_setup(&setup, mesh.as_ref()) else {
            continue;
        };
        let _ = template.bounds();
        let transform = Transform {
            position: b.v3(),
            rotation: b.quat(),
            scale: match b.byte() % 3 {
                0 => [1.0; 3],
                1 => [b.small(); 3],
                _ => b.v3(),
            },
        };
        let flags = CollisionFlags {
            ethereal_to: (b.byte() & 1 == 0).then(|| u64::from(b.u32())),
            ..CollisionFlags::default()
        };
        let _ = builder.add_object(
            BodySource::Generated {
                index: k as u32,
                object: Did(0x4700_0000 | k as u32),
            },
            flags,
            BodyPurpose::Movement,
            &template,
            &transform,
        );
    }

    // Cells.
    for k in 0..u16::from(head[2] % 5) {
        let planes: Vec<([f32; 3], f32)> = (0..b.byte() % 10).map(|_| (b.v3(), b.f())).collect();
        let hull: Vec<[f32; 3]> = (0..b.byte() % 10).map(|_| b.v3()).collect();
        let (Ok(volume), Ok(placement)) = (
            CellVolume::new(planes),
            CellPlacement::new(b.v3(), b.quat()),
        ) else {
            continue;
        };
        if let Ok(cell) = PlacedCell::with_volume(
            k,
            Did(0x0100_0000 | u32::from(k)),
            Arc::new(volume),
            &hull,
            placement,
        ) {
            let _ = builder.add_cell(cell, None);
        }
    }
    if head[0] & 0x80 != 0 {
        build_from_records(&mut builder, &mut b, rest);
    }
    let _ = builder.report();
    let set = builder.build();
    static OTHER: OnceLock<CollisionSet> = OnceLock::new();
    let other = OTHER.get_or_init(|| {
        // A small neighbour (one box) keeps the view cheap to query.
        let mut t = ShapeTemplate::default();
        let _ = t.push_box(&CollisionBox {
            min: [0.0; 3],
            max: [10.0, 10.0, 10.0],
            frame: Frame::IDENTITY,
        });
        let mut b = CollisionSetBuilder::new(CollisionLimits::default());
        let _ = b.add_shape(
            BodySource::Other { tag: 0 },
            CollisionFlags::default(),
            BodyPurpose::Movement,
            &t,
        );
        b.build()
    });
    let mut view = CollisionView::new();
    view.push(&set, [0.0; 3]);
    view.push(other, [160.0, 0.0, 0.0]);

    // Queries.
    let filters = [
        QueryFilter::player(),
        QueryFilter::camera(),
        QueryFilter::all(),
    ];
    for q in 0..usize::from(head[3] % 12) {
        let capsule = Capsule {
            a: b.v3(),
            b: if b.byte() & 1 == 0 { b.v3() } else { [0.0; 3] },
            radius: if b.byte() & 3 == 0 { b.f() } else { b.small() },
        };
        let delta = b.v3();
        let filter = &filters[q % 3];
        for hit in [
            set.sweep_capsule(&capsule, delta, filter),
            view.sweep_capsule(&capsule, delta, filter),
            set.raycast(capsule.a, delta, filter),
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                (0.0..=1.0).contains(&hit.fraction),
                "fraction {}",
                hit.fraction
            );
            assert!(hit.gap.is_finite() || !moderate(capsule.a));
            if moderate(capsule.a) && moderate(capsule.b) && moderate(delta) {
                assert!(hit.normal.iter().all(|c| c.is_finite()));
            }
        }
        // Non-penetration in moderate coordinates: no body ends deeper than it started
        // (bodies not overlapped at the start: not inside at all), up to half a skin of f32
        // rounding of the moved capsule.
        if capsule.is_valid()
            && moderate(capsule.a)
            && moderate(capsule.b)
            && moderate(delta)
            && capsule.radius < 50.0
        {
            let mut start = Vec::new();
            set.contacts(&capsule, 0.0, filter, &mut start);
            let mut deepest: HashMap<BodyId, f32> = HashMap::new();
            for c in &start {
                let d = deepest.entry(c.body).or_insert(0.0);
                *d = d.min(c.separation);
            }
            let fraction = set
                .sweep_capsule(&capsule, delta, filter)
                .map_or(1.0, |h| h.fraction);
            let moved = capsule.translated(delta.map(|d| d * fraction));
            let mut end = Vec::new();
            set.contacts(&moved, 0.0, filter, &mut end);
            for c in &end {
                let allowed = deepest.get(&c.body).copied().unwrap_or(0.0) - 0.5 * SKIN;
                assert!(
                    c.separation >= allowed,
                    "ended {} inside body {:?} (allowed {allowed})",
                    c.separation,
                    c.body
                );
            }
        }
        let mut contacts = Vec::new();
        view.contacts(&capsule, b.small(), filter, &mut contacts);
        assert!(contacts.len() <= 8192 + 64 + 4096);
        let p = b.v3();
        let _ = set.cell_at(p);
        let _ = view.cell_at(p);
        let (x, y, r) = (b.f(), b.f(), b.f());
        let keys = GridMesh::client_keys_near(x, y, r);
        assert!(keys.len() <= 2048 && keys.iter().all(|&k| k < 2048));
        assert!(terrain.keys_in_disc(x, y, r).len() <= 2048);
        let plane = terrain.client_plane_at(x, y, &flags);
        assert!(plane.normal.iter().all(|c| c.is_finite()));
        let _ = terrain.key_under(x, y, &flags);
    }
});

// ------------------------------------------------------------------ building from records

const NO_PHYSICS: u32 = 0x1000_0001;
const ETHEREAL_TO: u32 = 0x1000_0002;
const SURFACE: u32 = 0x1000_0004;
const SCALE: u32 = 0x1000_0006;
const LAYERS: u32 = 0x1000_0007;
const LAYER: u32 = 0x1000_0008;
const CREATION: u32 = 0x1000_0009;

fn schema() -> &'static HashMap<u32, PropertyType> {
    static S: OnceLock<HashMap<u32, PropertyType>> = OnceLock::new();
    S.get_or_init(|| {
        HashMap::from([
            (NO_PHYSICS, PropertyType::Bool),
            (ETHEREAL_TO, PropertyType::Bitfield32),
            (SURFACE, PropertyType::DidRef),
            (SCALE, PropertyType::Vector3),
            (LAYERS, PropertyType::Array),
            (LAYER, PropertyType::Enum),
            (CREATION, PropertyType::Enum),
        ])
    })
}

fn pids() -> PhysicsPids {
    PhysicsPids {
        no_physics: Some(NO_PHYSICS),
        adjustable_scale: Some(SCALE),
        ethereal_to_type: Some(ETHEREAL_TO),
        surface_properties_to_apply: Some(SURFACE),
        content_layers: Some(LAYERS),
        creation_type: Some(CREATION),
        ..PhysicsPids::default()
    }
}

/// An embedded property set with up to six fuzzed entries (`property-system.md` §5.1).
fn set_bytes(b: &mut Bytes) -> Vec<u8> {
    let mut entries: Vec<(u32, Vec<u8>)> = Vec::new();
    let mask = b.byte();
    if mask & 1 != 0 {
        entries.push((NO_PHYSICS, vec![b.byte() & 1]));
    }
    if mask & 2 != 0 {
        entries.push((ETHEREAL_TO, b.u32().to_le_bytes().to_vec()));
    }
    if mask & 4 != 0 {
        entries.push((
            SURFACE,
            (0x7800_0000 | u32::from(b.byte() % 4))
                .to_le_bytes()
                .to_vec(),
        ));
    }
    if mask & 8 != 0 {
        let mut v = Vec::new();
        for c in b.v3() {
            v.extend_from_slice(&c.to_le_bytes());
        }
        entries.push((SCALE, v));
    }
    if mask & 16 != 0 {
        let n = b.byte() % 3;
        let mut v = u32::from(n).to_le_bytes().to_vec();
        for _ in 0..n {
            v.extend_from_slice(&LAYER.to_le_bytes());
            let layer = match b.byte() % 3 {
                0 => 1,
                1 => 2,
                _ => b.u32(),
            };
            v.extend_from_slice(&layer.to_le_bytes());
        }
        entries.push((LAYERS, v));
    }
    if mask & 32 != 0 {
        // Entity_CreationType: mostly 7 and 8 (the §9.3 drop), sometimes anything.
        let key = match b.byte() % 4 {
            0 => 7,
            1 => 8,
            2 => 3,
            _ => b.u32(),
        };
        entries.push((CREATION, key.to_le_bytes().to_vec()));
    }
    let (hint, buckets) = TSIZE_BUCKET_COUNTS
        .iter()
        .enumerate()
        .find(|(_, k)| (**k as usize) * 2 >= entries.len())
        .map_or((0, 1), |(h, k)| (h as u8, *k));
    entries.sort_by_key(|(pid, _)| pid % buckets);
    let mut out = vec![hint];
    write_vle(&mut out, entries.len() as u32);
    for (pid, v) in entries {
        out.extend_from_slice(&pid.to_le_bytes());
        out.extend_from_slice(&pid.to_le_bytes());
        out.extend_from_slice(&v);
    }
    out
}

/// A record: synthetic (`make`), raw input bytes, or absent.
fn record(
    b: &mut Bytes,
    rest: &[u8],
    make: impl FnOnce(&mut Bytes) -> Option<Vec<u8>>,
) -> Option<Vec<u8>> {
    match b.byte() % 8 {
        0 => None,
        1 => {
            let start = b.pos % rest.len().max(1);
            let n = usize::from(b.byte());
            Some(rest[start..(start + n).min(rest.len())].to_vec())
        }
        _ => make(b),
    }
}

fn build_from_records(builder: &mut CollisionSetBuilder, b: &mut Bytes, rest: &[u8]) {
    // The archive: four objects 0x47 (some with a 0x47 parent and extra sets), one template,
    // setups and meshes.
    let mut archive: HashMap<u32, Vec<u8>> = HashMap::new();
    for k in 0..4u32 {
        let did = 0x4700_0000 | k;
        let parent = if b.byte() & 1 == 0 {
            0x1F00_0000 | (k % 2)
        } else {
            0x4700_0000 | ((k + 1) % 4)
        };
        let extra: Vec<Did> = (0..b.byte() % 3)
            .map(|_| Did(0x7800_0000 | u32::from(b.byte() % 4)))
            .collect();
        if let Some(r) = record(b, rest, |b| {
            let bytes = set_bytes(b);
            let properties = PropertySet::parse(&bytes, schema()).ok()?;
            EntityDescriptor {
                header: EntityHeader {
                    flags: ENTITY_FLAGS_REQUIRED,
                    did: Did(did),
                    kind: 7,
                    template: Did(parent),
                    block: None,
                },
                properties,
                extra_sets: extra,
            }
            .to_bytes_with(&bytes)
            .ok()
        }) {
            archive.insert(did, r);
        }
    }
    for k in 0..4u32 {
        let did = 0x7800_0000 | k;
        if let Some(r) = record(b, rest, |b| {
            let mut out = did.to_le_bytes().to_vec();
            out.extend_from_slice(&set_bytes(b));
            Some(out)
        }) {
            archive.insert(did, r);
        }
    }
    for k in 0..2u32 {
        let did = 0x1F00_0000 | k;
        let setup = 0x0400_0000 | k;
        if let Some(r) = record(b, rest, |_| {
            Some(
                VisualTemplate {
                    flags: 0x201,
                    did: Did(did),
                    parent: None,
                    vec3: None,
                    value_a: None,
                    value_b: None,
                    appearance: None,
                    setup_section: Some(SetupSection {
                        size: TSize { hint: 0, count: 1 },
                        entries: vec![SetupSectionEntry {
                            key: 1,
                            kind: SETUP_SECTION_DID_LIST,
                            dids: vec![Did(setup)],
                        }],
                    }),
                }
                .to_bytes(),
            )
        }) {
            archive.insert(did, r);
        }
        let mesh = 0x6600_0000 | k;
        if let Some(r) = record(b, rest, |b| {
            let mut setup_rec = Setup {
                did: Did(setup),
                collision_mesh: if b.byte() & 1 == 0 { Did(mesh) } else { Did(0) },
                ..Setup::default()
            };
            for _ in 0..b.byte() % 3 {
                setup_rec.boxes.push(CollisionBox {
                    min: b.v3(),
                    max: b.v3(),
                    frame: Frame::IDENTITY,
                });
            }
            for _ in 0..b.byte() % 2 {
                setup_rec.spheres.push(CollisionSphere {
                    centre: b.v3(),
                    radius: b.small(),
                });
            }
            setup_rec.to_bytes().ok()
        }) {
            archive.insert(setup, r);
        }
        if let Some(r) = record(b, rest, |b| {
            let vertices: Vec<[f32; 3]> = (0..3 + b.byte() % 6).map(|_| b.v3()).collect();
            let n = vertices.len() as u16;
            let triangles: Vec<[u16; 3]> = (0..1 + b.byte() % 8)
                .map(|_| {
                    [
                        u16::from(b.byte()) % n,
                        u16::from(b.byte()) % n,
                        u16::from(b.byte()) % n,
                    ]
                })
                .collect();
            Some(
                CollisionMesh {
                    vertices,
                    triangles,
                    tree: TreeBlock {
                        origin: [-4.05, -4.05, -0.05],
                        scale: 2_055_293.5,
                        code: vec![0x30],
                        build_byte: None,
                    },
                    ..CollisionMesh::default()
                }
                .to_bytes(),
            )
        }) {
            archive.insert(mesh, r);
        }
    }
    let fetch = |d: Did| -> Result<Option<FetchedRecord>, String> {
        if d.0 & 0xFF == 0xFF {
            return Err("injected".to_owned());
        }
        Ok(archive.get(&d.0).map(|bytes| FetchedRecord {
            bytes: bytes.clone(),
            stamp: None,
        }))
    };
    let pids = pids();
    // Resolve an arbitrary DID once directly (hostile chains end in errors, never panics).
    let any = resolve_object_collision(Did(b.u32()), fetch, schema(), &pids);
    let _ = any.heap_size();
    let cache = ObjectCollisionCache::with_limits(8, 1 << 20);
    let object =
        |o: Did| cache.get_or_resolve(o, |o| resolve_object_collision(o, fetch, schema(), &pids));

    // Static entities and generated objects.
    let mut entities = Vec::new();
    for _ in 0..b.byte() % 8 {
        let bytes = set_bytes(b);
        entities.push(StaticEntity {
            flags: 0,
            did: None,
            entity_type: Some(if b.byte().is_multiple_of(4) { 1 } else { 7 }),
            iid: None,
            object: (!b.byte().is_multiple_of(8))
                .then(|| Did(0x4700_0000 | u32::from(b.byte() % 5))),
            placement: Placement::new(b.v3(), b.quat()).ok(),
            vector: None,
            properties: PropertySet::parse(&bytes, schema()).ok(),
            word: None,
            name: None,
            dids: None,
        });
    }
    // Cells over a few fuzzed geometries.
    let mut geometries: HashMap<Did, Arc<CellGeometry>> = HashMap::new();
    for k in 0..b.byte() % 3 {
        let did = Did(0x0100_0000 | u32::from(k));
        let planes = (0..1 + b.byte() % 8)
            .map(|_| CellPlane {
                normal: b.v3(),
                distance: b.f(),
            })
            .collect();
        let hull = (0..b.byte() % 10).map(|_| b.v3()).collect();
        let props = set_bytes(b);
        let physics_mesh = (b.byte() & 1 == 0).then(|| CollisionMesh {
            vertices: vec![b.v3(), b.v3(), b.v3()],
            triangles: vec![[0, 1, 2]],
            ..CollisionMesh::default()
        });
        geometries.insert(
            did,
            Arc::new(CellGeometry {
                did,
                physics_mesh,
                planes,
                plane_list_end: PlaneListEnd::Bpfl,
                portals: Vec::new(),
                walls: Vec::new(),
                hull,
                properties: PropertySet::parse(&props, schema()).unwrap_or_default(),
                render_mesh: Vec::new(),
                layout_rule: LayoutRule::default(),
            }),
        );
    }
    let cells: Vec<Cell> = (0..b.byte() % 6)
        .map(|_| {
            let mut position = Position::new(0).with_region(1);
            if !b.byte().is_multiple_of(5) {
                position = position.with_offset(b.v3());
            }
            if b.byte() & 1 == 0 {
                let q = b.quat();
                position = position.with_orientation([q.w, q.x, q.y, q.z]);
            }
            Cell {
                index: b.u16() % 64,
                position,
                geometry: Did(0x0100_0000 | u32::from(b.byte() % 4)),
                ..Cell::default()
            }
        })
        .collect();
    let info = LandblockInfo {
        did: Did(0x8021_0101),
        flags: u32::from(!cells.is_empty()),
        size_hint: 0,
        cross_links: None,
        header_indices: None,
        cells: (!cells.is_empty()).then_some(TSizeList {
            size: TSize {
                hint: 0,
                count: cells.len() as u32,
            },
            items: cells,
        }),
        reserved_a: 0,
        entities,
        links: Vec::new(),
        properties: PropertySet::default(),
        reserved_b: 0,
        weenies: TSizeList::default(),
        trailer: None,
        layout_rule: LayoutRule::default(),
    };
    let options = StaticOptions {
        active_content_layers: (0..b.byte() % 3).map(|_| u32::from(b.byte() % 4)).collect(),
        instance: u32::from(b.byte() % 2),
    };
    builder.add_statics(&info, &pids, &options, object);
    let generated: Vec<GeneratedObject> = (0..b.byte() % 6)
        .map(|_| GeneratedObject {
            object: Did(0x4700_0000 | u32::from(b.byte() % 5)),
            transform: Transform {
                position: b.v3(),
                rotation: b.quat(),
                scale: [b.small(); 3],
            },
        })
        .collect();
    let out = builder.add_generated(&generated, object);
    assert_eq!(out.len(), generated.len());
    builder.add_cells(&info, &pids, |d| geometries.get(&d).cloned());
}
