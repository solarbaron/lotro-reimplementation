// SPDX-License-Identifier: GPL-3.0-or-later
//! Collision records (`docs/specs/collision.md` §3–§5) against a user's own install
//! (read-only), with the record-level invariants of §13:
//!
//! - **CO1** every 0x66 record decodes to its last byte with its directory stamp (`has_code`
//!   1, indices below the vertex count, `build_byte` iff stamp ≥ 38, value 1); the
//!   stamp-less fallback decides every `build_byte` the same way;
//! - **CO2** every 0x01 record decodes to its last byte (`mesh_count` 1, `mesh_flag` 0
//!   enforced by the decoder; last plane tag "BPFL" or "LEAF", portal lead byte 1 and four
//!   corners, unit plane normals within 10⁻³ and hull vertices within 5.2 mm of the volume
//!   (§5.4) checked here). Decoding the embedded render mesh is the mesh crate's to check
//!   (it depends on this crate, not the reverse);
//! - **CO3** every landblock-info cell has as many neighbours as its geometry has portals,
//!   and every neighbour link is reciprocal;
//! - **CO4** the tree code of every 0x66 record, every cell physics mesh, every terrain
//!   tree and every ceiling tree walks with no unknown opcode and no out-of-range jump, and
//!   its keys are valid for the host;
//! - **CO5** origin and scale follow §3.2 for 0x66 records and cell physics meshes;
//! - **CO6** the point query at every triangle's centroid and corners reports the
//!   triangle's key (0x66 and cell physics meshes: all; terrain: a 1-in-16 sample of the
//!   trees unless `TESSERA_FULL=1`);
//! - **CO7** every terrain code reports exactly the keys 0–2,047.
//!
//! Every code is also queried over the whole integer range (which takes every branch, so
//! it must report exactly the structural walk's keys) to check that the most expensive
//! query of the real data stays far below [`MAX_QUERY_STEPS`] and the largest walk below
//! [`MAX_WALK_STATES`]. The 0x66 records and cell physics meshes also get box queries
//! (§3.7): the mesh's bounds report every key of the walk, and every triangle's box
//! reports the triangle.
//!
//! Skipped unless `TESSERA_GAME_DIR` points at an install. Run with
//! `cargo test --release -p tessera-formats --test collision_real_data -- --nocapture`.
//! A debug build visits a 1-in-23 sample of the records unless `TESSERA_FULL=1`. Only
//! counts, DIDs and error kinds are printed; no game data leaves the install.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use tessera_dat::{ArchiveKind, ArchiveSet, Entry, SetOptions};
use tessera_formats::collision::{
    CELL_GEOMETRY_PREFIX, COLLISION_MESH_PREFIX, CellGeometry, CollisionMesh, MAX_QUERY_STEPS,
    MAX_WALK_STATES, PlaneListEnd, TreeCode, TreeWalk, quantisation_for_bounds,
};
use tessera_formats::landblock::{GRID_TRIANGLE_COUNT, GridSurface, LayoutRule};
use tessera_formats::{Did, LandblockData, LandblockInfo, PropertyDictionary, Record};

const PROPERTY_DICTIONARY: u32 = 0x3400_0000;

/// §5.4: every hull vertex satisfies all planes within 5.2 mm.
const HULL_TOLERANCE: f32 = 0.0052;

/// Queries `code` over the whole integer range, which takes every branch: it must report
/// exactly the structural walk's keys. Returns the instructions executed.
fn full_range_steps(what: &str, code: &TreeCode<'_>, walk: &TreeWalk) -> usize {
    let mut keys = BTreeSet::new();
    let steps = code
        .query_quantised([i64::MIN; 3], [i64::MAX; 3], |k| {
            keys.insert(k);
        })
        .unwrap_or_else(|e| panic!("{what}: full-range query: {e}"));
    assert!(
        keys.iter().eq(walk.keys.iter()),
        "{what}: full-range query keys differ from the walk's"
    );
    steps
}

fn open_set() -> Option<&'static ArchiveSet> {
    static SET: OnceLock<Option<ArchiveSet>> = OnceLock::new();
    SET.get_or_init(|| {
        let dir = match std::env::var_os("TESSERA_GAME_DIR") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => {
                eprintln!("TESSERA_GAME_DIR not set; skipping");
                return None;
            }
        };
        Some(ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set"))
    })
    .as_ref()
}

fn dictionary(set: &ArchiveSet) -> &'static PropertyDictionary {
    static DICT: OnceLock<PropertyDictionary> = OnceLock::new();
    DICT.get_or_init(|| {
        let payload = set.read(PROPERTY_DICTIONARY).expect("read dictionary");
        PropertyDictionary::parse_expecting(&payload, PROPERTY_DICTIONARY).expect("dictionary")
    })
}

/// The real data must stay far below the absolute caps (here: 4× margin at least).
fn assert_caps(what: &str, steps: usize, states: usize) {
    assert!(
        steps <= MAX_QUERY_STEPS / 4,
        "{what}: full-range query of {steps} instructions near MAX_QUERY_STEPS"
    );
    assert!(
        states <= MAX_WALK_STATES / 4,
        "{what}: walk of {states} states near MAX_WALK_STATES"
    );
}

fn full() -> bool {
    std::env::var_os("TESSERA_FULL").is_some_and(|v| v == "1")
}

fn exhaustive() -> bool {
    !cfg!(debug_assertions) || full()
}

fn sample<T: Clone>(v: &[T], every: usize) -> Vec<T> {
    v.iter()
        .enumerate()
        .filter(|(i, _)| i.is_multiple_of(every))
        .map(|(_, e)| e.clone())
        .collect()
}

/// Runs `f` over `items` on every core; results come back in no fixed order.
fn par_each<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out = Mutex::new(Vec::with_capacity(items.len()));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let mut local = Vec::new();
                loop {
                    let i = next.fetch_add(16, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    for item in &items[i..(i + 16).min(items.len())] {
                        local.push(f(item));
                    }
                }
                out.lock().unwrap().extend(local);
            });
        }
    });
    out.into_inner().unwrap()
}

/// (member index, entry) of every record with type prefix `prefix` in `client_general`
/// (first archive wins).
fn general_entries(set: &ArchiveSet, prefix: u8) -> Vec<(usize, Entry)> {
    let lo = u32::from(prefix) << 24;
    let mut out = BTreeMap::new();
    for (i, m) in set.members().iter().enumerate() {
        if m.id.kind != ArchiveKind::General {
            continue;
        }
        for e in m.archive.entries_range(lo..=lo | 0x00FF_FFFF) {
            let e = e.expect("directory walk");
            out.entry(e.did).or_insert((i, e));
        }
    }
    out.into_values().collect()
}

/// (member index, entry) of every landblock record of kind `k` in the cell archives.
fn landblock_entries(set: &ArchiveSet, k: u32) -> Vec<(usize, Entry)> {
    let mut out = Vec::new();
    for (i, m) in set.members().iter().enumerate() {
        if !matches!(m.id.kind, ArchiveKind::Cell(_)) {
            continue;
        }
        let lo = 0x8000_0000 | (k << 20);
        for e in m.archive.entries_range(lo..=lo | 0x000F_FFFF) {
            out.push((i, e.expect("directory walk")));
        }
    }
    out
}

fn read(set: &ArchiveSet, member: usize, e: &Entry) -> Vec<u8> {
    set.member(member)
        .archive
        .read_entry(e)
        .unwrap_or_else(|err| panic!("{:#010x}: read: {err}", e.did))
}

fn rule_name(r: LayoutRule) -> &'static str {
    match r {
        LayoutRule::Stamp => "stamp",
        LayoutRule::Fallback => "fallback",
        LayoutRule::UnobservedStamp => "unobserved stamp",
    }
}

/// What one triangle mesh with a tree (0x66 or a cell physics mesh) showed.
#[derive(Default)]
struct MeshCheck {
    triangles: usize,
    /// Triangles the code does not report (§3.6 slivers).
    uncovered: usize,
    /// CO5 failures.
    fit_failures: usize,
    /// CO6 point queries and misses.
    queries: usize,
    misses: usize,
    /// Keys reported by two leaves (never observed).
    shared_keys: usize,
    build_byte_not_one: bool,
    /// Box queries (one over the mesh bounds, one per covered triangle) and the keys they
    /// should have reported but did not.
    box_queries: usize,
    box_misses: usize,
    /// Instructions of the full-range query.
    full_range_steps: usize,
    walk: Option<TreeWalk>,
}

/// CO4–CO6 on one mesh. Panics (with the DID) on a walk failure or a key out of range.
fn check_mesh(did: u32, m: &CollisionMesh) -> MeshCheck {
    let mut out = MeshCheck {
        triangles: m.triangles.len(),
        build_byte_not_one: m.tree.build_byte.is_some_and(|b| b != 1),
        ..MeshCheck::default()
    };
    let code = m.tree_code();
    let walk = code
        .walk()
        .unwrap_or_else(|e| panic!("{did:#010x}: CO4 tree walk: {e}"));
    assert!(
        walk.keys_below(m.triangles.len() as u64),
        "{did:#010x}: CO4 key {:?} >= {} triangles",
        walk.keys.last(),
        m.triangles.len()
    );
    out.uncovered = m.triangles.len() - walk.keys.len();
    out.shared_keys = walk.keys_from_several_leaves;
    out.full_range_steps = full_range_steps(&format!("{did:#010x}"), &code, &walk);

    // CO5 over the vertices the triangles use.
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for t in &m.triangles {
        for &i in t {
            let v = m.vertices[usize::from(i)];
            for a in 0..3 {
                lo[a] = lo[a].min(v[a]);
                hi[a] = hi[a].max(v[a]);
            }
        }
    }
    if !m.triangles.is_empty() {
        let (origin, scale) = quantisation_for_bounds(lo, hi);
        let origin_ok = (0..3).all(|a| (f64::from(m.tree.origin[a]) - origin[a]).abs() <= 1e-4);
        let scale_ok = ((f64::from(m.tree.scale) - scale) / scale).abs() <= 1e-5;
        if !(origin_ok && scale_ok) {
            out.fit_failures += 1;
        }
        // §3.7 box query over the mesh's bounds: every key of the walk.
        let keys = code
            .box_keys(lo, hi)
            .unwrap_or_else(|e| panic!("{did:#010x}: box query: {e}"));
        out.box_queries += 1;
        out.box_misses += walk
            .keys
            .iter()
            .filter(|k| keys.binary_search(k).is_err())
            .count();
    }

    // CO6.
    for &key in &walk.keys {
        let p = m.triangle_positions(key as usize).expect("key below count");
        let c = [0, 1, 2]
            .map(|a| ((f64::from(p[0][a]) + f64::from(p[1][a]) + f64::from(p[2][a])) / 3.0) as f32);
        for q in [c, p[0], p[1], p[2]] {
            out.queries += 1;
            let keys = code
                .point_keys(q)
                .unwrap_or_else(|e| panic!("{did:#010x}: point query: {e}"));
            if keys.binary_search(&key).is_err() {
                out.misses += 1;
            }
        }
        // The triangle's own box reports it.
        let lo = [0, 1, 2].map(|a| p[0][a].min(p[1][a]).min(p[2][a]));
        let hi = [0, 1, 2].map(|a| p[0][a].max(p[1][a]).max(p[2][a]));
        out.box_queries += 1;
        let keys = code
            .box_keys(lo, hi)
            .unwrap_or_else(|e| panic!("{did:#010x}: box query: {e}"));
        if keys.binary_search(&key).is_err() {
            out.box_misses += 1;
        }
    }
    out.walk = Some(walk);
    out
}

#[derive(Default)]
struct Totals {
    records: usize,
    triangles: usize,
    fully_covered: usize,
    uncovered: usize,
    fit_failures: usize,
    queries: usize,
    misses: usize,
    shared_keys: usize,
    build_byte_not_one: usize,
    box_queries: usize,
    box_misses: usize,
    max_steps: usize,
    max_states: usize,
    opcodes: [usize; 11],
}

impl Totals {
    fn add(&mut self, c: &MeshCheck) {
        self.records += 1;
        self.triangles += c.triangles;
        self.fully_covered += usize::from(c.uncovered == 0);
        self.uncovered += c.uncovered;
        self.fit_failures += c.fit_failures;
        self.queries += c.queries;
        self.misses += c.misses;
        self.shared_keys += c.shared_keys;
        self.build_byte_not_one += usize::from(c.build_byte_not_one);
        self.box_queries += c.box_queries;
        self.box_misses += c.box_misses;
        self.max_steps = self.max_steps.max(c.full_range_steps);
        if let Some(w) = &c.walk {
            self.max_states = self.max_states.max(w.states);
            let k = w.counts;
            for (slot, v) in self.opcodes.iter_mut().zip([
                k.end,
                k.rescale,
                k.jump,
                k.base,
                k.axis_split,
                k.diagonal_split,
                k.threshold_split,
                k.wide_split,
                k.filter,
                k.filter24,
                k.leaf,
            ]) {
                *slot += v;
            }
        }
    }

    fn report(&self, what: &str) {
        eprintln!(
            "{what}: {} trees, {} triangles; {} report every triangle, {} triangles uncovered; \
             CO5 failures {}; CO6 {} queries, {} misses; keys from several leaves {}; \
             build bytes != 1: {}; box queries {}, misses {}; largest full-range query {} \
             instructions, largest walk {} states",
            self.records,
            self.triangles,
            self.fully_covered,
            self.uncovered,
            self.fit_failures,
            self.queries,
            self.misses,
            self.shared_keys,
            self.build_byte_not_one,
            self.box_queries,
            self.box_misses,
            self.max_steps,
            self.max_states
        );
        eprintln!(
            "{what}: instructions end {} rescale {} jump {} base {} axis {} diagonal {} \
             threshold {} wide {} filter {} filter24 {} leaf {}",
            self.opcodes[0],
            self.opcodes[1],
            self.opcodes[2],
            self.opcodes[3],
            self.opcodes[4],
            self.opcodes[5],
            self.opcodes[6],
            self.opcodes[7],
            self.opcodes[8],
            self.opcodes[9],
            self.opcodes[10]
        );
    }

    fn assert_invariants(&self, what: &str) {
        assert_eq!(self.fit_failures, 0, "{what}: CO5");
        assert_eq!(self.misses, 0, "{what}: CO6");
        assert_eq!(self.build_byte_not_one, 0, "{what}: build_byte value");
        assert_eq!(self.box_misses, 0, "{what}: box queries");
        assert_caps(what, self.max_steps, self.max_states);
    }
}

#[test]
fn co1_co4_co6_collision_meshes() {
    let Some(set) = open_set() else { return };
    let mut entries = general_entries(set, COLLISION_MESH_PREFIX);
    assert!(!entries.is_empty(), "no 0x66 records");
    let total = entries.len();
    if !exhaustive() {
        entries = sample(&entries, 23);
    }
    let results = par_each(&entries, |(member, e)| {
        let data = read(set, *member, e);
        let m = CollisionMesh::parse(&data, Some(e.stamp))
            .unwrap_or_else(|err| panic!("{:#010x} (stamp {}): CO1: {err}", e.did, e.stamp));
        assert_eq!(
            m.tree.build_byte.is_some(),
            e.stamp >= 38,
            "{:#010x}: CO1 build_byte vs stamp {}",
            e.did,
            e.stamp
        );
        let fallback = CollisionMesh::parse(&data, None)
            .unwrap_or_else(|err| panic!("{:#010x}: stamp-less fallback: {err}", e.did));
        assert_eq!(
            fallback.tree.build_byte, m.tree.build_byte,
            "{:#010x}",
            e.did
        );
        // The encoder is canonical and the stored records use shortest VLEs.
        assert!(
            m.to_bytes() == data,
            "{:#010x}: CO1 re-encoded bytes differ",
            e.did
        );
        (e.stamp, m.layout_rule, check_mesh(e.did, &m))
    });
    let mut totals = Totals::default();
    let mut stamps: BTreeMap<u16, usize> = BTreeMap::new();
    let mut rules: BTreeMap<&str, usize> = BTreeMap::new();
    for (stamp, rule, c) in &results {
        totals.add(c);
        *stamps.entry(*stamp).or_default() += 1;
        *rules.entry(rule_name(*rule)).or_default() += 1;
    }
    eprintln!(
        "0x66: {} of {} records decoded exactly (of {total} in the archive); stamps {stamps:?}; \
         rules {rules:?}",
        results.len(),
        entries.len()
    );
    totals.report("0x66");
    assert_eq!(results.len(), entries.len());
    totals.assert_invariants("0x66");
}

/// Every 0x01 record decoded with its stamp and the dictionary, by DID.
fn cell_geometries(set: &'static ArchiveSet) -> &'static BTreeMap<u32, (u16, CellGeometry)> {
    static CELLS: OnceLock<BTreeMap<u32, (u16, CellGeometry)>> = OnceLock::new();
    CELLS.get_or_init(|| {
        let dict = dictionary(set);
        let entries = general_entries(set, CELL_GEOMETRY_PREFIX);
        par_each(&entries, |(member, e)| {
            let data = read(set, *member, e);
            let c = CellGeometry::parse_expecting(&data, e.did, Some(e.stamp), dict)
                .unwrap_or_else(|err| panic!("{:#010x} (stamp {}): CO2: {err}", e.did, e.stamp));
            (e.did, (e.stamp, c))
        })
        .into_iter()
        .collect()
    })
}

#[test]
fn co2_co4_co6_cell_geometry() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let all = cell_geometries(set);
    assert!(!all.is_empty(), "no 0x01 records");
    let mut items: Vec<(&u32, &(u16, CellGeometry))> = all.iter().collect();
    if !exhaustive() {
        items = sample(&items, 23);
    }
    let results = par_each(&items, |&(&did, (stamp, c))| {
        // CO2 beyond what the decoder enforces.
        assert!(
            matches!(c.plane_list_end, PlaneListEnd::Bpfl | PlaneListEnd::Leaf),
            "{did:#010x}: CO2 last plane tag {:#010x}",
            c.plane_list_end.tag()
        );
        for (k, p) in c.portals.iter().enumerate() {
            assert_eq!(p.lead, 1, "{did:#010x}: CO2 portal {k} lead byte");
            assert_eq!(p.corners.len(), 4, "{did:#010x}: CO2 portal {k} corners");
        }
        for (k, p) in c.planes.iter().enumerate() {
            let n = p.normal;
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!(
                (len - 1.0).abs() <= 1e-3,
                "{did:#010x}: CO2 plane {k} normal length {len}"
            );
        }
        // The physics mesh's build byte follows the stamp; the fallback agrees.
        if let Some(m) = &c.physics_mesh {
            assert_eq!(
                m.tree.build_byte.is_some(),
                *stamp >= 38,
                "{did:#010x}: build_byte"
            );
        }
        let data = read_cell(set, did);
        let fallback = CellGeometry::parse(&data, None, dict)
            .unwrap_or_else(|err| panic!("{did:#010x}: stamp-less fallback: {err}"));
        assert_eq!(
            fallback.physics_mesh.as_ref().map(|m| m.tree.build_byte),
            c.physics_mesh.as_ref().map(|m| m.tree.build_byte),
            "{did:#010x}: fallback"
        );
        // Hull vertices against the volume (§5.4: within 5.2 mm).
        let mut worst = 0.0f32;
        for v in &c.hull {
            for p in &c.planes {
                worst = worst.max(-p.signed_distance(*v));
            }
        }
        let check = c.physics_mesh.as_ref().map(|m| check_mesh(did, m));
        (
            c.planes.len(),
            c.portals.len(),
            c.walls.len(),
            c.hull.len(),
            c.render_mesh.len(),
            worst,
            check,
        )
    });
    let mut totals = Totals::default();
    let (mut planes, mut portals, mut walls, mut hull, mut mesh_bytes) = (0, 0, 0, 0, 0);
    let mut worst = 0.0f32;
    let mut without_mesh = 0usize;
    for (p, o, w, h, s, d, check) in &results {
        planes += p;
        portals += o;
        walls += w;
        hull += h;
        mesh_bytes += s;
        worst = worst.max(*d);
        match check {
            Some(c) => totals.add(c),
            None => without_mesh += 1,
        }
    }
    eprintln!(
        "0x01: {} records decoded exactly, {without_mesh} without a physics mesh; planes \
         {planes}, portals {portals}, walls {walls}, hull vertices {hull}, embedded render mesh \
         bytes {mesh_bytes}; worst hull vertex outside a plane by {worst:.4}",
        results.len()
    );
    totals.report("0x01 physics");
    assert!(
        worst <= HULL_TOLERANCE,
        "CO2: hull vertex {worst} outside the volume (§5.4: within 5.2 mm)"
    );
    assert_eq!(
        totals.uncovered, 0,
        "0x01 physics meshes: §3.6 every triangle reported"
    );
    totals.assert_invariants("0x01 physics");
}

fn read_cell(set: &ArchiveSet, did: u32) -> Vec<u8> {
    set.read(did)
        .unwrap_or_else(|err| panic!("{did:#010x}: read: {err}"))
}

#[test]
fn co3_cells_link_portals() {
    let Some(set) = open_set() else { return };
    let dict = dictionary(set);
    let geometries = cell_geometries(set);
    let portal_count: HashMap<u32, usize> = geometries
        .iter()
        .map(|(&did, (_, c))| (did, c.portals.len()))
        .collect();
    let mut entries = landblock_entries(set, 2);
    if !exhaustive() {
        entries = sample(&entries, 23);
    }
    let results = par_each(&entries, |(member, e)| {
        let data = read(set, *member, e);
        let info = LandblockInfo::parse_expecting(&data, e.did, Some(e.stamp), dict)
            .unwrap_or_else(|err| panic!("{:#010x}: landblock info: {err}", e.did));
        let Some(cells) = &info.cells else {
            return (0usize, 0usize);
        };
        let by_index: HashMap<u16, &tessera_formats::landblock::Cell> =
            cells.items.iter().map(|c| (c.index, c)).collect();
        let mut links = 0;
        for c in &cells.items {
            let portals = portal_count.get(&c.geometry.0).unwrap_or_else(|| {
                panic!(
                    "{:#010x}: cell {} geometry {} missing",
                    e.did, c.index, c.geometry
                )
            });
            assert_eq!(
                c.neighbours.len(),
                *portals,
                "{:#010x}: CO3 cell {} neighbours vs portals of {}",
                e.did,
                c.index,
                c.geometry
            );
            for (k, n) in c.neighbours.iter().enumerate() {
                links += 1;
                let other = by_index.get(&n.cell).unwrap_or_else(|| {
                    panic!(
                        "{:#010x}: cell {} neighbour {} missing",
                        e.did, c.index, n.cell
                    )
                });
                let back = other
                    .neighbours
                    .get(usize::from(n.back_reference))
                    .unwrap_or_else(|| {
                        panic!(
                            "{:#010x}: cell {} back-reference out of range",
                            e.did, c.index
                        )
                    });
                assert_eq!(
                    (back.cell, usize::from(back.back_reference)),
                    (c.index, k),
                    "{:#010x}: CO3 cell {} neighbour {k} not reciprocal",
                    e.did,
                    c.index
                );
            }
        }
        (cells.items.len(), links)
    });
    let cells: usize = results.iter().map(|r| r.0).sum();
    let links: usize = results.iter().map(|r| r.1).sum();
    eprintln!(
        "CO3: {} landblock-info records, {cells} cells, {links} neighbour entries, all reciprocal",
        results.len()
    );
}

/// Walks one terrain or ceiling tree; returns (walk, CO6 queries, misses, full-range
/// query instructions).
fn check_grid_tree(
    did: Did,
    d: &LandblockData,
    surface: GridSurface,
    points: bool,
) -> Option<(TreeWalk, usize, usize, usize)> {
    let tree = d.surface_tree(surface)?;
    let code = TreeCode::from(tree);
    let walk = code
        .walk()
        .unwrap_or_else(|e| panic!("{did}: CO4 {surface:?} tree walk: {e}"));
    assert!(
        walk.keys_below(GRID_TRIANGLE_COUNT as u64),
        "{did}: CO4 {surface:?} key {:?}",
        walk.keys.last()
    );
    let steps = full_range_steps(&format!("{did} {surface:?}"), &code, &walk);
    let (mut queries, mut misses) = (0, 0);
    if points {
        for &key in &walk.keys {
            let p = d
                .grid_triangle_positions(surface, key as u16)
                .expect("grid triangle");
            let c = [0, 1, 2].map(|a| {
                ((f64::from(p[0][a]) + f64::from(p[1][a]) + f64::from(p[2][a])) / 3.0) as f32
            });
            for q in [c, p[0], p[1], p[2]] {
                queries += 1;
                let keys = code
                    .point_keys(q)
                    .unwrap_or_else(|e| panic!("{did}: point query: {e}"));
                if keys.binary_search(&key).is_err() {
                    misses += 1;
                }
            }
        }
    }
    Some((walk, queries, misses, steps))
}

#[test]
fn co4_co6_co7_terrain_and_ceiling_trees() {
    let Some(set) = open_set() else { return };
    let mut entries = landblock_entries(set, 0);
    assert!(!entries.is_empty(), "no landblock data");
    if !exhaustive() {
        entries = sample(&entries, 23);
    }
    let point_every = if full() { 1 } else { 16 };
    let terrain_trees = AtomicUsize::new(0);
    let results = par_each(&entries, |(member, e)| {
        let data = read(set, *member, e);
        let d = LandblockData::parse_expecting(&data, e.did, Some(e.stamp))
            .unwrap_or_else(|err| panic!("{:#010x}: landblock data: {err}", e.did));
        let did = Did(e.did);
        let mut out = [0usize; 10];
        // Sample the terrain point queries by tree, in no fixed but even order.
        let points = d.terrain_tree.is_some()
            && terrain_trees
                .fetch_add(1, Ordering::Relaxed)
                .is_multiple_of(point_every);
        if let Some((walk, q, m, steps)) = check_grid_tree(did, &d, GridSurface::Terrain, points) {
            out[0] = 1;
            // CO7: exactly the keys 0–2,047.
            assert!(
                walk.keys.len() == GRID_TRIANGLE_COUNT
                    && walk.keys.iter().enumerate().all(|(i, &k)| k as usize == i),
                "{did}: CO7 terrain keys ({} distinct)",
                walk.keys.len()
            );
            out[1] = usize::from(points);
            out[2] = q;
            out[3] = m;
            out[4] = walk.counts.total();
            out[8] = steps;
            out[9] = walk.states;
        }
        if let Some((walk, _, _, steps)) = check_grid_tree(did, &d, GridSurface::Ceiling, false) {
            out[5] = 1;
            out[6] = usize::from(walk.keys.len() == GRID_TRIANGLE_COUNT);
            out[7] = walk.counts.total();
            out[8] = out[8].max(steps);
            out[9] = out[9].max(walk.states);
        }
        out
    });
    let sum = |k: usize| results.iter().map(|r| r[k]).sum::<usize>();
    let max = |k: usize| results.iter().map(|r| r[k]).max().unwrap_or(0);
    eprintln!(
        "landblock data: {} records; terrain trees {} (instructions {}), CO6 on {} of them: {} \
         queries, {} misses; ceiling trees {} ({} with all 2,048 keys; instructions {}); largest full-range query {} \
         instructions, largest walk {} states",
        results.len(),
        sum(0),
        sum(4),
        sum(1),
        sum(2),
        sum(3),
        sum(5),
        sum(6),
        sum(7),
        max(8),
        max(9)
    );
    assert_eq!(sum(3), 0, "CO6 terrain");
    assert_caps("landblock trees", max(8), max(9));
}
