// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests of set building from decoded records (`collision.md` §5.8, §9.1–§9.4,
//! §12.1): the object resolver's success path (0x47 chain with extra sets → 0x1F → 0x04 →
//! 0x66), statics, generated objects and cells. Every record is Tessera's own encoding of
//! the layouts in `docs/specs/`; no game data is involved.

use std::collections::HashMap;
use std::sync::Arc;

use tessera_formats::collision::{CellGeometry, CellPlane, CollisionMesh, PlaneListEnd, TreeBlock};
use tessera_formats::landblock::{
    Cell, GridSurface, HeightGrid, LandblockInfo, Placement, StaticEntity, TSizeList, VertexFlags,
};
use tessera_formats::reader::{TSIZE_BUCKET_COUNTS, write_vle};
use tessera_formats::setup::{
    CollisionBox, ENTITY_FLAGS_REQUIRED, EntityHeader, Frame, Quaternion, SETUP_SECTION_DID_LIST,
    SetupSection, SetupSectionEntry,
};
use tessera_formats::{
    Did, EntityDescriptor, Position, PropertySet, PropertyType, Setup, TSize, VisualTemplate,
};
use tessera_physics::PhysicsError;
use tessera_physics::geometry::{
    BodySource, CollisionFlags, CollisionLimits, CollisionSetBuilder, ETHEREAL_PLAYER,
    EntityLookups, FetchedRecord, GeneratedObject, GridMesh, MAX_CELL_HULL, MAX_SET_CELL_HULL,
    MAX_TEMPLATE_TRIANGLES, ObjectCollision, ObjectCollisionCache, PhysicsPids, QueryFilter,
    SetupCollision, StaticOptions, Transform, content_layer_list, content_layers_pass,
    resolve_object_collision, static_blocks_walker, static_has_walker_collision,
};

// ------------------------------------------------------------------ synthetic encoding

const NO_PHYSICS: u32 = 0x1000_0001;
const ETHEREAL_TO: u32 = 0x1000_0002;
const UNCONTROLLED: u32 = 0x1000_0003;
const SURFACE: u32 = 0x1000_0004;
const LEDGE: u32 = 0x1000_0005;
const SCALE: u32 = 0x1000_0006;
const LAYERS: u32 = 0x1000_0007;
const LAYER: u32 = 0x1000_0008;
const CREATION: u32 = 0x1000_0009;

#[derive(Debug, Clone)]
enum V {
    Bool(bool),
    Bits(u32),
    Did(u32),
    Vec3([f32; 3]),
    Layers(Vec<u32>),
}

fn schema() -> HashMap<u32, PropertyType> {
    HashMap::from([
        (NO_PHYSICS, PropertyType::Bool),
        (ETHEREAL_TO, PropertyType::Bitfield32),
        (UNCONTROLLED, PropertyType::Bool),
        (SURFACE, PropertyType::DidRef),
        (LEDGE, PropertyType::Bool),
        (SCALE, PropertyType::Vector3),
        (LAYERS, PropertyType::Array),
        (LAYER, PropertyType::Enum),
        (CREATION, PropertyType::Enum),
    ])
}

fn pids() -> PhysicsPids {
    PhysicsPids {
        no_physics: Some(NO_PHYSICS),
        adjustable_scale: Some(SCALE),
        ethereal_to_type: Some(ETHEREAL_TO),
        always_uncontrolled_surface: Some(UNCONTROLLED),
        surface_properties_to_apply: Some(SURFACE),
        can_ledge_climb: Some(LEDGE),
        content_layers: Some(LAYERS),
        creation_type: Some(CREATION),
        ..PhysicsPids::default()
    }
}

fn u32le(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// An embedded property set (`property-system.md` §5.1): TSize, then (pid, pid, value) in
/// bucket order.
fn set_bytes(entries: &[(u32, V)]) -> Vec<u8> {
    let (hint, buckets) = TSIZE_BUCKET_COUNTS
        .iter()
        .enumerate()
        .find(|(_, b)| (**b as usize) * 2 >= entries.len())
        .map(|(h, b)| (u8::try_from(h).expect("hint"), *b))
        .expect("bucket");
    let mut out = vec![hint];
    write_vle(&mut out, u32::try_from(entries.len()).expect("count"));
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|(pid, _)| pid % buckets);
    for (pid, v) in sorted {
        u32le(&mut out, pid);
        u32le(&mut out, pid);
        match v {
            V::Bool(b) => out.push(u8::from(b)),
            V::Bits(k) | V::Did(k) => u32le(&mut out, k),
            V::Vec3(c) => c
                .iter()
                .for_each(|x| out.extend_from_slice(&x.to_le_bytes())),
            V::Layers(l) => {
                u32le(&mut out, u32::try_from(l.len()).expect("len"));
                for k in l {
                    u32le(&mut out, LAYER);
                    u32le(&mut out, k);
                }
            }
        }
    }
    out
}

fn set(entries: &[(u32, V)]) -> PropertySet {
    PropertySet::parse(&set_bytes(entries), &schema()).expect("property set")
}

fn entity(did: u32, template: u32, entries: &[(u32, V)], extra: &[u32]) -> Vec<u8> {
    let bytes = set_bytes(entries);
    EntityDescriptor {
        header: EntityHeader {
            flags: ENTITY_FLAGS_REQUIRED,
            did: Did(did),
            kind: 7,
            template: Did(template),
            block: None,
        },
        properties: set(entries),
        extra_sets: extra.iter().map(|&d| Did(d)).collect(),
    }
    .to_bytes_with(&bytes)
    .expect("entity")
}

fn property_set_record(did: u32, entries: &[(u32, V)]) -> Vec<u8> {
    let mut out = did.to_le_bytes().to_vec();
    out.extend_from_slice(&set_bytes(entries));
    out
}

fn visual_template(did: u32, setup: u32) -> Vec<u8> {
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
    .to_bytes()
}

fn unit_box() -> CollisionBox {
    CollisionBox {
        min: [-1.0, -1.0, 0.0],
        max: [1.0, 1.0, 2.0],
        frame: Frame::IDENTITY,
    }
}

fn setup(did: u32, mesh: Option<u32>, boxes: usize) -> Vec<u8> {
    Setup {
        did: Did(did),
        collision_mesh: Did(mesh.unwrap_or(0)),
        boxes: vec![unit_box(); boxes],
        ..Setup::default()
    }
    .to_bytes()
    .expect("setup")
}

fn ground_triangle() -> CollisionMesh {
    CollisionMesh {
        vertices: vec![[-4.0, -4.0, 0.0], [4.0, -4.0, 0.0], [4.0, 4.0, 0.0]],
        triangles: vec![[0, 1, 2]],
        tree: TreeBlock {
            origin: [-4.05, -4.05, -0.05],
            #[allow(clippy::cast_possible_truncation)]
            scale: (254.0 * 65_536.0 / 8.1) as f32,
            code: vec![0x30],
            build_byte: None,
        },
        ..CollisionMesh::default()
    }
}

/// An in-memory archive.
#[derive(Default)]
struct Records(HashMap<u32, Vec<u8>>);

impl Records {
    fn put(&mut self, did: u32, bytes: Vec<u8>) -> &mut Self {
        self.0.insert(did, bytes);
        self
    }

    fn fetch(&self, did: Did) -> Result<Option<FetchedRecord>, String> {
        Ok(self.0.get(&did.0).map(|b| FetchedRecord {
            bytes: b.clone(),
            stamp: None,
        }))
    }

    fn resolve(&self, object: u32) -> ObjectCollision {
        resolve_object_collision(Did(object), |d| self.fetch(d), &schema(), &pids())
    }
}

const A: u32 = 0x4700_0001;
const B: u32 = 0x4700_0002;
const T: u32 = 0x1F00_0001;
const S: u32 = 0x0400_0001;
const M: u32 = 0x6600_0001;
const S1: u32 = 0x7800_0001;
const S2: u32 = 0x7800_0002;

/// A → B → T → S (mesh M and one box); A names extra sets S1 and S2.
fn chain() -> Records {
    let mut r = Records::default();
    r.put(
        A,
        entity(
            A,
            B,
            &[(NO_PHYSICS, V::Bool(false)), (LEDGE, V::Bool(true))],
            &[S1, S2],
        ),
    )
    .put(
        B,
        entity(
            B,
            T,
            &[
                (NO_PHYSICS, V::Bool(true)),
                (ETHEREAL_TO, V::Bits(1 << 29)),
                (SURFACE, V::Did(0x7800_0CCC)),
                // SharedStaticImmobile: the client creates statics of this chain (§9.3).
                (CREATION, V::Bits(3)),
            ],
            &[],
        ),
    )
    .put(
        S1,
        property_set_record(
            S1,
            &[
                (SURFACE, V::Did(0x7800_0AAA)),
                (LEDGE, V::Bool(false)),
                (UNCONTROLLED, V::Bool(true)),
            ],
        ),
    )
    .put(
        S2,
        property_set_record(S2, &[(SURFACE, V::Did(0x7800_0BBB))]),
    )
    .put(T, visual_template(T, S))
    .put(S, setup(S, Some(M), 1))
    .put(M, ground_triangle().to_bytes());
    r
}

// ------------------------------------------------------------------ the resolver

/// Finding: the resolver ignored the 0x78 extra sets (`scenery.md` §9.2 rule 1).
#[test]
fn the_resolver_layers_extra_sets_and_the_chain() {
    let r = chain().resolve(A);
    assert_eq!(r.error, None);
    assert_eq!(r.chain, vec![Did(A), Did(B), Did(T), Did(S)]);
    assert_eq!(r.setup, Some(Did(S)));
    assert_eq!(r.collision, SetupCollision::Mesh);
    let f = r.flags;
    // The object's own level wins over its parent ...
    assert_eq!(f.no_physics, Some(false));
    // ... its later extra set over the earlier one ...
    assert_eq!(f.surface_properties, Some(Did(0x7800_0BBB)));
    // ... its embedded set over its extra sets ...
    assert_eq!(f.can_ledge_climb, Some(true));
    // ... an extra set's value fills what the embedded set lacks ...
    assert_eq!(f.always_uncontrolled_surface, Some(true));
    // ... and the parent fills the rest.
    assert_eq!(f.ethereal_to, Some(ETHEREAL_PLAYER));
    let t = r.template.as_ref().expect("template");
    assert_eq!(
        (t.triangles.len(), t.mesh_triangles, t.boxes.len()),
        (13, 1, 1)
    );
    assert!(!r.incomplete);
    // The parent alone.
    let b = chain().resolve(B);
    assert_eq!(b.flags.no_physics, Some(true));
    assert_eq!(b.flags.surface_properties, Some(Did(0x7800_0CCC)));
}

/// Finding: errors were overwritten and an object without its mesh was not reported.
#[test]
fn the_resolver_keeps_the_first_error_and_flags_incomplete_shapes() {
    let mut r = chain();
    r.0.remove(&S1);
    r.0.remove(&M);
    let o = r.resolve(A);
    // The missing extra set came first; the missing mesh is also noted (logged).
    assert!(matches!(
        &o.error,
        Some(PhysicsError::Record { did, what: "extra property set", .. }) if *did == S1
    ));
    assert_eq!(o.flags.surface_properties, Some(Did(0x7800_0BBB)));
    assert_eq!(o.flags.always_uncontrolled_surface, None);
    assert!(o.incomplete, "only the box collides");
    let t = o.template.as_ref().expect("volumes");
    assert_eq!((t.triangles.len(), t.boxes.len()), (12, 1));
    // Mesh missing and no volume: no template, not "incomplete".
    let mut r = chain();
    r.0.remove(&M);
    r.put(S, setup(S, Some(M), 0));
    let o = r.resolve(A);
    assert!(o.template.is_none() && !o.incomplete);
    assert!(matches!(
        &o.error,
        Some(PhysicsError::Record { did, what: "collision mesh", .. }) if *did == M
    ));
    // A broken property set of one level keeps the other levels.
    let mut r = chain();
    r.put(A, vec![0xFF; 9]);
    let o = r.resolve(A);
    assert!(o.error.is_some());
}

// ------------------------------------------------------------------ statics and generated

fn flat_terrain() -> Arc<GridMesh> {
    Arc::new(GridMesh::from_grid(
        GridSurface::Terrain,
        &HeightGrid::flat(0),
        &VertexFlags::filled(0),
    ))
}

fn sed(kind: u32, object: u32, position: Option<[f32; 3]>, props: &[(u32, V)]) -> StaticEntity {
    StaticEntity {
        flags: 0,
        did: None,
        entity_type: Some(kind),
        iid: None,
        object: Some(Did(object)),
        placement: position.map(|p| Placement::new(p, Quaternion::IDENTITY).expect("placement")),
        vector: None,
        properties: Some(set(props)),
        word: None,
        name: None,
        dids: None,
    }
}

fn info(entities: Vec<StaticEntity>, cells: Vec<Cell>) -> LandblockInfo {
    LandblockInfo {
        did: Did(0x8021_0A14),
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
        entities,
        links: Vec::new(),
        properties: PropertySet::default(),
        reserved_b: 0,
        weenies: TSizeList::default(),
        trailer: None,
        layout_rule: tessera_formats::landblock::LayoutRule::default(),
    }
}

/// Finding: `add_statics`, `add_generated` and `add_cells` ran only in real-data tests.
#[test]
fn statics_follow_the_walker_rule() {
    let records = chain();
    let cache = ObjectCollisionCache::default();
    let object = |o: Did| cache.get_or_resolve(o, |o| records.resolve(o.0));
    let ghost = [(ETHEREAL_TO, V::Bits(1 << 29))];
    let entities = vec![
        // 0: blocks.
        sed(7, A, Some([10.0, 10.0, 0.0]), &[(SCALE, V::Vec3([2.0; 3]))]),
        // 1: its own NoPhysics wins over the chain.
        sed(
            7,
            A,
            Some([20.0, 10.0, 0.0]),
            &[(NO_PHYSICS, V::Bool(true))],
        ),
        // 2: ethereal to Player: a body the player passes through.
        sed(7, A, Some([30.0, 10.0, 0.0]), &ghost),
        // 3: a content layer that is not active.
        sed(
            7,
            A,
            Some([40.0, 10.0, 0.0]),
            &[(LAYERS, V::Layers(vec![0x1000_008D]))],
        ),
        // 4: layer 1 in the shared world.
        sed(
            7,
            A,
            Some([50.0, 10.0, 0.0]),
            &[(LAYERS, V::Layers(vec![1]))],
        ),
        // 5: no position.
        sed(7, A, None, &[]),
        // 6: not a Physics SED.
        sed(1, A, Some([60.0, 10.0, 0.0]), &[]),
        // 7: an object that does not resolve.
        sed(7, 0x4700_0999, Some([70.0, 10.0, 0.0]), &[]),
    ];
    let lb = info(entities, Vec::new());
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.add_grid(flat_terrain()).expect("terrain");
    b.add_statics(&lb, &pids(), &StaticOptions::default(), object);
    let r = *b.report();
    assert_eq!(r.statics_seen, 7);
    assert_eq!(r.statics_blocking, 3);
    // SED 3 (inactive layer) and SED 7 (its object does not resolve, so no creation type
    // is found: 0, refused by §9.3 item 3).
    assert_eq!(r.statics_filtered, 2);
    assert_eq!(r.unresolved, 1);
    let set = b.build();
    let bodies: HashMap<u32, _> = set
        .bodies()
        .iter()
        .filter_map(|body| match body.source {
            BodySource::Static { index, .. } => Some((index, body)),
            _ => None,
        })
        .collect();
    let mut keys: Vec<u32> = bodies.keys().copied().collect();
    keys.sort_unstable();
    assert_eq!(keys, vec![0, 2, 4]);
    // Scaled by its own Physics_AdjustableScale: the box is 4 m tall.
    let first = bodies[&0];
    let tri = (first.first_triangle..first.first_triangle + first.triangle_count)
        .filter_map(|k| set.triangle(k as usize))
        .flat_map(|(v, _, _)| v)
        .map(|v| v[2])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!((tri - 4.0).abs() < 1e-5, "top {tri}");
    // The rule of §9.3 agrees with "a body the player filter accepts", SED by SED.
    let player = QueryFilter::player();
    for (n, e) in lb.entities.iter().enumerate() {
        let n = u32::try_from(n).expect("index");
        let resolved = records.resolve(e.object.expect("object").0);
        let own = e
            .properties
            .as_ref()
            .map(|s| CollisionFlags::from_set(s, &pids()))
            .unwrap_or_default();
        let own_lookups = e
            .properties
            .as_ref()
            .map(|s| EntityLookups::from_set(s, &pids()))
            .unwrap_or_default();
        let lookups = own_lookups.or(resolved.lookups.clone());
        let layers = content_layers_pass(lookups.layer_list(), &[1], 0);
        // The SED's own list agrees with `content_layer_list` where it has one.
        if let Some(own) = e
            .properties
            .as_ref()
            .and_then(|s| content_layer_list(s, &pids()))
        {
            assert_eq!(lookups.layer_list(), Some(own.as_slice()));
        }
        let physics = e.entity_type == Some(7) && e.placement.is_some();
        let rule = static_blocks_walker(
            physics,
            layers,
            &CollisionFlags::of_static(own, resolved.flags),
            resolved.collision,
        );
        let blocks = bodies.get(&n).is_some_and(|b| player.accepts(b));
        assert_eq!(rule, blocks, "SED {n}");
    }
    // The ghost blocks the camera but not the player.
    let ray = |f: &QueryFilter| set.raycast([25.0, 10.0, 1.0], [10.0, 0.0, 0.0], f);
    assert!(ray(&player).is_none());
    assert!(ray(&QueryFilter::camera()).is_some());
}

/// `collision.md` §9.3 item 3 (C29, CO14): the effective `Entity_CreationType` is the
/// SED's own value when its set has one, else the value found along its 0x47 chain; statics
/// of creation type 0, 7 or 8 get no body, and a SED with no value anywhere counts as 0.
/// Generated objects pass no creation-type filter (§9.4).
#[test]
fn statics_the_client_does_not_create_get_no_body() {
    const C: u32 = 0x4700_0003;
    const D: u32 = 0x4700_0004;
    const E: u32 = 0x4700_0005;
    const F: u32 = 0x4700_0006;
    let mut records = chain();
    records
        .put(C, entity(C, T, &[(CREATION, V::Bits(8))], &[]))
        .put(D, entity(D, C, &[], &[]))
        .put(E, entity(E, T, &[(CREATION, V::Bits(3))], &[]))
        // No Entity_CreationType anywhere on F's chain.
        .put(F, entity(F, T, &[], &[]));
    let cache = ObjectCollisionCache::default();
    let object = |o: Did| cache.get_or_resolve(o, |o| records.resolve(o.0));
    let at = |x: f32| Some([x, 10.0, 0.0]);
    let entities = vec![
        // 0: creation type 3 on the object: blocks.
        sed(7, E, at(10.0), &[]),
        // 1, 2: the SED's own 7 / 8 wins over the object's 3: refused.
        sed(7, E, at(20.0), &[(CREATION, V::Bits(7))]),
        sed(7, E, at(30.0), &[(CREATION, V::Bits(8))]),
        // 3: 8 on the object; 4: 8 on the object's parent: refused.
        sed(7, C, at(40.0), &[]),
        sed(7, D, at(50.0), &[]),
        // 5: the SED's own 3 wins over the object's 8: blocks.
        sed(7, C, at(60.0), &[(CREATION, V::Bits(3))]),
        // 6: the SED's own 0 (Invalid) over the object's 3: refused.
        sed(7, E, at(70.0), &[(CREATION, V::Bits(0))]),
        // 7: no value on the SED or its chain: 0, refused.
        sed(7, F, at(80.0), &[]),
        // 8: the SED's own 9 on that chain: blocks.
        sed(7, F, at(90.0), &[(CREATION, V::Bits(9))]),
    ];
    let lb = info(entities, Vec::new());
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.add_statics(&lb, &pids(), &StaticOptions::default(), object);
    let r = *b.report();
    assert_eq!(
        (r.statics_seen, r.statics_blocking, r.statics_filtered),
        (9, 3, 6)
    );
    let set = b.build();
    let bodies: HashMap<u32, _> = set
        .bodies()
        .iter()
        .filter_map(|body| match body.source {
            BodySource::Static { index, .. } => Some((index, body)),
            _ => None,
        })
        .collect();
    let mut indices: Vec<u32> = bodies.keys().copied().collect();
    indices.sort_unstable();
    assert_eq!(indices, vec![0, 5, 8]);
    // The walker rule says the same, SED by SED.
    let player = QueryFilter::player();
    for (n, e) in lb.entities.iter().enumerate() {
        let resolved = records.resolve(e.object.expect("object").0);
        let own = e
            .properties
            .as_ref()
            .map(|s| CollisionFlags::from_set(s, &pids()))
            .unwrap_or_default();
        let flags = CollisionFlags::of_static(own, resolved.flags);
        let rule = static_blocks_walker(true, true, &flags, resolved.collision);
        let index = u32::try_from(n).expect("index");
        let blocks = bodies.get(&index).is_some_and(|b| player.accepts(b));
        assert_eq!(rule, blocks, "SED {n}");
        assert_eq!(rule, [0, 5, 8].contains(&n), "SED {n}");
        assert_eq!(
            flags.refuses_static_creation(),
            !rule,
            "SED {n}: only the creation type refuses it"
        );
        // The data census of §9.3 counts without the filter.
        assert!(static_has_walker_collision(&flags, resolved.collision));
    }
    // A generated object of creation type 8, or of none, still gets a body.
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    let placed = |object: u32, x: f32| GeneratedObject {
        object: Did(object),
        transform: Transform::new([x, 50.0, 0.0], Quaternion::IDENTITY, 1.0),
    };
    let out = b.add_generated(&[placed(C, 10.0), placed(F, 20.0)], |o: Did| {
        Arc::new(records.resolve(o.0))
    });
    assert!(out.iter().all(Option::is_some));
    assert_eq!(b.report().generated_blocking, 2);
}

/// `collision.md` §9.3 (the paragraph after item 3): the content-layer list and the scale
/// of a static are looked up like its creation type, the SED's own set first, then its
/// 0x47 chain.
#[test]
fn statics_take_layers_and_scale_from_the_chain_when_the_sed_has_none() {
    const G: u32 = 0x4700_0007;
    let mut records = chain();
    records.put(
        G,
        entity(
            G,
            T,
            &[
                (CREATION, V::Bits(3)),
                (LAYERS, V::Layers(vec![0x1000_008D])),
                (SCALE, V::Vec3([2.0; 3])),
            ],
            &[],
        ),
    );
    let cache = ObjectCollisionCache::default();
    let object = |o: Did| cache.get_or_resolve(o, |o| records.resolve(o.0));
    assert_eq!(
        records.resolve(G).lookups.layer_list(),
        Some(&[0x1000_008D][..])
    );
    let at = |x: f32| Some([x, 10.0, 0.0]);
    let entities = vec![
        // 0: the object's inactive layer: filtered.
        sed(7, G, at(10.0), &[]),
        // 1: its own shared-world list wins; the object's scale (2) applies.
        sed(7, G, at(20.0), &[(LAYERS, V::Layers(vec![1]))]),
        // 2: its own list and its own scale (0.5).
        sed(
            7,
            G,
            at(30.0),
            &[(LAYERS, V::Layers(vec![1])), (SCALE, V::Vec3([0.5; 3]))],
        ),
    ];
    let lb = info(entities, Vec::new());
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.add_statics(&lb, &pids(), &StaticOptions::default(), object);
    let r = *b.report();
    assert_eq!((r.statics_blocking, r.statics_filtered), (2, 1));
    let set = b.build();
    let mut tops: Vec<(u32, f32)> = set
        .bodies()
        .iter()
        .filter_map(|body| match body.source {
            BodySource::Static { index, .. } => {
                let top = (body.first_triangle..body.first_triangle + body.triangle_count)
                    .filter_map(|k| set.triangle(k as usize))
                    .flat_map(|(v, _, _)| v)
                    .map(|v| v[2])
                    .fold(f32::NEG_INFINITY, f32::max);
                Some((index, top))
            }
            _ => None,
        })
        .collect();
    tops.sort_by_key(|t| t.0);
    assert_eq!(tops.len(), 2);
    assert_eq!(tops[0].0, 1);
    assert!((tops[0].1 - 4.0).abs() < 1e-5, "object scale: {tops:?}");
    assert_eq!(tops[1].0, 2);
    assert!((tops[1].1 - 1.0).abs() < 1e-5, "own scale: {tops:?}");
    // In an instanced copy under the object's layer, SED 0 passes.
    let options = StaticOptions {
        active_content_layers: vec![0x1000_008D],
        instance: 3,
    };
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.add_statics(&lb, &pids(), &options, |o: Did| {
        cache.get_or_resolve(o, |o| records.resolve(o.0))
    });
    // SEDs 1 and 2 name only layer 1, which fails in an instanced copy under another layer.
    assert_eq!(
        (b.report().statics_blocking, b.report().statics_filtered),
        (1, 2)
    );
}

#[test]
fn generated_objects_get_bodies_without_a_layer_filter() {
    let records = chain();
    let object = |o: Did| Arc::new(records.resolve(o.0));
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    let placed = |object: u32, x: f32| GeneratedObject {
        object: Did(object),
        transform: Transform::new([x, 50.0, 0.0], Quaternion::IDENTITY, 1.0),
    };
    let out = b.add_generated(
        &[placed(A, 10.0), placed(B, 20.0), placed(0x4700_0999, 30.0)],
        object,
    );
    assert!(out[0].is_some());
    assert!(out[1].is_none(), "B alone has NoPhysics");
    assert!(out[2].is_none());
    let r = *b.report();
    assert_eq!(
        (r.generated_seen, r.generated_blocking, r.unresolved),
        (3, 1, 1)
    );
    // Incomplete shapes are counted.
    let mut broken = chain();
    broken.0.remove(&M);
    let object = |o: Did| Arc::new(broken.resolve(o.0));
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    let out = b.add_generated(&[placed(A, 10.0)], object);
    assert!(out[0].is_some());
    assert_eq!(b.report().incomplete, 1);
}

// ------------------------------------------------------------------ cells

fn room_geometry(did: u32, surface: Option<u32>, mesh: bool) -> CellGeometry {
    let plane = |n: [f32; 3], d: f32| CellPlane {
        normal: n,
        distance: d,
    };
    CellGeometry {
        did: Did(did),
        physics_mesh: mesh.then(ground_triangle),
        planes: vec![
            plane([1.0, 0.0, 0.0], 4.0),
            plane([-1.0, 0.0, 0.0], 4.0),
            plane([0.0, 1.0, 0.0], 4.0),
            plane([0.0, -1.0, 0.0], 4.0),
            plane([0.0, 0.0, 1.0], 0.0),
            plane([0.0, 0.0, -1.0], 3.0),
        ],
        plane_list_end: PlaneListEnd::Bpfl,
        portals: Vec::new(),
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
        properties: surface.map_or_else(PropertySet::default, |d| {
            set(&[(SURFACE, V::Did(d)), (LEDGE, V::Bool(false))])
        }),
        render_mesh: Vec::new(),
        layout_rule: tessera_formats::landblock::LayoutRule::default(),
    }
}

fn cell(index: u16, geometry: u32, at: Option<[f32; 3]>) -> Cell {
    let position = at.map_or_else(
        || Position::new(0).with_region(1),
        |p| Position::new(0).with_region(1).with_offset(p),
    );
    Cell {
        index,
        position,
        geometry: Did(geometry),
        ..Cell::default()
    }
}

/// Finding: cell physics meshes got default flags although the 0x01 record carries
/// `Physics_SurfacePropertiesToApply` and `Physics_CanLedgeClimb`.
#[test]
fn cells_carry_their_geometry_flags() {
    let geometries: HashMap<Did, Arc<CellGeometry>> = [
        room_geometry(0x0100_0001, Some(0x7800_0D0D), true),
        room_geometry(0x0100_0002, None, false),
    ]
    .into_iter()
    .map(|g| (g.did, Arc::new(g)))
    .collect();
    let lb = info(
        Vec::new(),
        vec![
            cell(2, 0x0100_0001, Some([100.0, 100.0, 10.0])),
            cell(1, 0x0100_0002, Some([20.0, 20.0, 10.0])),
            // No offset (two such cells in the data): refused.
            cell(3, 0x0100_0001, None),
            // Missing geometry: refused.
            cell(4, 0x0100_0009, Some([50.0, 50.0, 0.0])),
            // Shares the first geometry.
            cell(5, 0x0100_0001, Some([120.0, 100.0, 10.0])),
        ],
    );
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.add_cells(&lb, &pids(), |d| geometries.get(&d).cloned());
    let r = *b.report();
    assert_eq!((r.cells, r.cells_refused), (3, 2));
    let set = b.build();
    let mesh_bodies: Vec<_> = set
        .bodies()
        .iter()
        .filter(|b| matches!(b.source, BodySource::Cell { .. }))
        .collect();
    assert_eq!(mesh_bodies.len(), 2);
    for body in mesh_bodies {
        assert_eq!(body.flags.surface_properties, Some(Did(0x7800_0D0D)));
        assert_eq!(body.flags.can_ledge_climb, Some(false));
    }
    assert_eq!(set.cell_at([100.0, 100.0, 11.0]).map(|h| h.cell), Some(2));
    assert_eq!(set.cell_at([20.0, 20.0, 11.0]).map(|h| h.cell), Some(1));
    assert_eq!(set.cell_at([120.0, 100.0, 11.0]).map(|h| h.cell), Some(5));
    assert!(set.cell_at([60.0, 60.0, 11.0]).is_none());
    let numbers: Vec<u16> = set.cells().cells().iter().map(|c| c.index).collect();
    assert_eq!(numbers, vec![1, 2, 5]);
}

/// Finding: a refused cell (no offset, too many hull vertices, limits) still used up the
/// set's hull budget, so later valid cells were refused and `cell_at` reported outdoors
/// inside them. Only placed cells count now.
#[test]
fn refused_cells_do_not_use_up_the_hull_budget() {
    let mut big = room_geometry(0x0100_0003, None, false);
    big.hull = vec![[0.0, 0.0, 1.0]; MAX_CELL_HULL + 1];
    let geometries: HashMap<Did, Arc<CellGeometry>> = [big, room_geometry(0x0100_0001, None, true)]
        .into_iter()
        .map(|g| (g.did, Arc::new(g)))
        .collect();
    // Enough refused hull vertices to exceed the per-set budget several times over.
    let refused = MAX_SET_CELL_HULL / MAX_CELL_HULL + 10;
    let mut cells: Vec<Cell> = (0..refused)
        .map(|k| {
            let k = u16::try_from(k).expect("index");
            cell(k + 10, 0x0100_0003, Some([80.0, 80.0, 10.0]))
        })
        .collect();
    cells.push(cell(2, 0x0100_0001, Some([100.0, 100.0, 10.0])));
    let lb = info(Vec::new(), cells);
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.add_cells(&lb, &pids(), |d| geometries.get(&d).cloned());
    let r = *b.report();
    assert_eq!(
        (r.cells, r.cells_refused),
        (1, u32::try_from(refused).expect("count"))
    );
    let set = b.build();
    assert_eq!(set.cell_at([100.0, 100.0, 11.0]).map(|h| h.cell), Some(2));
}

/// Finding: a cell physics mesh beyond the template limits was dropped silently. It is now
/// refused and counted as a body over a limit (§12.1); the cell itself is placed.
#[test]
fn a_cell_mesh_beyond_the_limits_is_refused_and_counted() {
    let mut huge = room_geometry(0x0100_0004, None, true);
    if let Some(m) = huge.physics_mesh.as_mut() {
        m.triangles = vec![[0, 1, 2]; MAX_TEMPLATE_TRIANGLES + 1];
    }
    let geometries: HashMap<Did, Arc<CellGeometry>> =
        [huge].into_iter().map(|g| (g.did, Arc::new(g))).collect();
    let lb = info(
        Vec::new(),
        vec![
            cell(1, 0x0100_0004, Some([20.0, 20.0, 10.0])),
            cell(2, 0x0100_0004, Some([60.0, 20.0, 10.0])),
        ],
    );
    let mut b = CollisionSetBuilder::new(CollisionLimits::default());
    b.add_cells(&lb, &pids(), |d| geometries.get(&d).cloned());
    let r = *b.report();
    assert_eq!((r.cells, r.cells_refused, r.over_limit), (2, 0, 2));
    let set = b.build();
    assert!(
        !set.bodies()
            .iter()
            .any(|b| matches!(b.source, BodySource::Cell { .. }))
    );
    assert_eq!(set.cell_at([20.0, 20.0, 11.0]).map(|h| h.cell), Some(1));
}
