// SPDX-License-Identifier: GPL-3.0-or-later
//! `census <dir|archive>`: decode every record of the selected DID prefixes and count the
//! results, for the decode criteria of the roadmap (M4.5 exit criterion 1).
//!
//! Every selected record is decoded to its last byte with its directory stamp and, where
//! needed, the property dictionary. A cell-geometry record (0x01) counts as decoded only
//! when its embedded render mesh (part 11, `collision.md` §5.1 and §5.7) decodes as well,
//! with the record's stamp. Mesh records (0x06) are decoded only when `--prefix 06` asks
//! for them (the mesh archives are large).
//!
//! The quantised collision trees of every host in the selection are walked structurally
//! (`collision.md` §3.6, CO4): collision meshes (0x66), cell physics meshes and the levels
//! of the embedded render meshes (0x01), mesh levels (0x06) and landblock terrain and
//! ceiling trees (0x80 kind 0). A walk must finish without an unknown opcode or
//! out-of-range jump, and every key must be valid for its host (§3.5): below the triangle
//! count for 0x66, physics meshes and landblocks; a (draw position, triangle) pair of the
//! level for mesh levels, where a level whose keys are valid only with one single-triangle
//! draw left out of the numbering is counted as renumbered (§3.5's exception), not as a
//! failure. The command reads only; it reports counts, never record content.

use std::collections::BTreeMap;
use std::io::Write;
use std::process::ExitCode;

use anyhow::{Result, bail};
use serde_json::{Value, json};
use tessera_dat::{Archive, Entry};
use tessera_formats::collision::{CollisionMesh, TreeCode, TreeWalk};
use tessera_formats::landblock::{FailureCategory, GRID_TRIANGLE_COUNT, LandblockPids, LayoutRule};
use tessera_formats::{AnyRecord, Did, DidKind, LandblockKind};
use tessera_mesh::{CollisionTree, DecodeOptions, Level, Mesh, MeshError};

use crate::Out;
use crate::input::{Ctx, Input};
use crate::util::{Align, Table, did_hex, par_map};
use crate::world::{self, Decoded, Schema};

/// The default selection: the record kinds of M4.5 exit criterion 1 (`ROADMAP.md`), the
/// new world records and the M2 material family (0x2B, 0x30, 0x31) that must still decode.
pub const WORLD_PREFIXES: [u8; 17] = [
    0x01, 0x02, 0x08, 0x0F, 0x11, 0x12, 0x15, 0x16, 0x17, 0x19, 0x1A, 0x2B, 0x30, 0x31, 0x32, 0x35,
    0x66,
];

/// Records per parallel work item.
const CHUNK: usize = 512;

/// A mesh-level key is draw position · 2²⁰ + triangle index (`collision.md` §3.5).
const SUBPART_SHIFT: u32 = 20;

/// What a mesh decode keeps for the census: the tree code, not the collision meshes.
const MESH_OPTIONS: DecodeOptions = DecodeOptions {
    keep_collision: false,
    keep_tree_code: true,
    strict: false,
};

/// Options of `census`.
#[derive(Debug)]
pub struct Opts {
    /// `<dir|archive>`.
    pub target: String,
    /// DID prefixes to decode; empty = [`WORLD_PREFIXES`].
    pub prefixes: Vec<u8>,
    /// Failures listed (0 = none).
    pub max_failures: usize,
}

/// One tree host: where the tree lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Host {
    CollisionMesh,
    CellPhysicsMesh,
    CellRenderMesh,
    MeshLevel,
    Terrain,
    Ceiling,
}

impl Host {
    fn name(self) -> &'static str {
        match self {
            Self::CollisionMesh => "0x66 collision meshes",
            Self::CellPhysicsMesh => "0x01 cell physics meshes",
            Self::CellRenderMesh => "0x01 embedded mesh levels",
            Self::MeshLevel => "0x06 mesh levels",
            Self::Terrain => "0x80 terrain trees",
            Self::Ceiling => "0x80 ceiling trees",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct TreeRow {
    walked: u64,
    failed: u64,
    keys_out_of_range: u64,
    /// Mesh levels whose keys skip one single-triangle draw (`collision.md` §3.5).
    renumbered: u64,
    /// Mesh-level tree blocks with `has_code` = 0 (nothing to walk).
    without_code: u64,
}

/// How the keys of a walked tree relate to its host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyCheck {
    /// Every key is valid.
    Valid,
    /// Valid with §3.5's renumbering (mesh levels only).
    Renumbered,
    /// This key is not valid for the host.
    OutOfRange(u32),
}

/// Counts for one record kind.
#[derive(Debug, Clone, Default)]
struct KindRow {
    records: u64,
    decoded: u64,
    failed: u64,
    bytes: u64,
    items: u64,
    fallback: u64,
    unobserved: u64,
    categories: BTreeMap<&'static str, u64>,
}

#[derive(Debug, Clone)]
struct Failure {
    did: u32,
    archive: String,
    error: String,
    category: &'static str,
}

/// Row key: the DID prefix and the record kind (the landblock family has one row per K).
type KindKey = (u8, DidKind);

/// Aggregated results (per work item, then merged in input order).
#[derive(Debug, Default)]
struct Tally {
    kinds: BTreeMap<KindKey, KindRow>,
    trees: BTreeMap<Host, TreeRow>,
    tree_failures: Vec<Failure>,
    failures: Vec<Failure>,
    failures_total: u64,
}

impl Tally {
    fn merge(&mut self, o: Self, max: usize) {
        for (k, r) in o.kinds {
            let e = self.kinds.entry(k).or_default();
            e.records += r.records;
            e.decoded += r.decoded;
            e.failed += r.failed;
            e.bytes += r.bytes;
            e.items += r.items;
            e.fallback += r.fallback;
            e.unobserved += r.unobserved;
            for (c, n) in r.categories {
                *e.categories.entry(c).or_default() += n;
            }
        }
        for (h, r) in o.trees {
            let e = self.trees.entry(h).or_default();
            e.walked += r.walked;
            e.failed += r.failed;
            e.keys_out_of_range += r.keys_out_of_range;
            e.renumbered += r.renumbered;
            e.without_code += r.without_code;
        }
        self.failures_total += o.failures_total;
        let room = max.saturating_sub(self.failures.len());
        self.failures.extend(o.failures.into_iter().take(room));
        let room = max.saturating_sub(self.tree_failures.len());
        self.tree_failures
            .extend(o.tree_failures.into_iter().take(room));
    }

    fn tree_problems(&self) -> u64 {
        self.trees
            .values()
            .map(|t| t.failed + t.keys_out_of_range)
            .sum()
    }
}

/// What the items column counts for a kind.
fn item_label(kind: DidKind) -> &'static str {
    match kind {
        DidKind::TerrainTypeTable
        | DidKind::TerrainSceneTable
        | DidKind::SceneChoiceTable
        | DidKind::BlockMapProperties
        | DidKind::Landblock(LandblockKind::BlockMap) => "entries",
        DidKind::WaterTable | DidKind::FogZoneTable => "listed entries",
        DidKind::Scene => "children",
        DidKind::SkyDescription => "day groups",
        DidKind::DayDescription => "keyframes",
        DidKind::GameTime => "months",
        DidKind::LodClassTable => "classes",
        DidKind::CollisionMesh => "triangles",
        DidKind::CollisionGeometry => "portals",
        DidKind::Mesh => "levels",
        DidKind::RegionDescriptor => "with all 8 slots",
        DidKind::Landblock(LandblockKind::Heightmap) => "with ceiling",
        DidKind::Landblock(LandblockKind::Info) => "cells",
        DidKind::Landblock(LandblockKind::Properties) => "light maps",
        _ => "",
    }
}

/// The KIND column: [`DidKind::name`], with the K nibble for unnamed landblock kinds.
fn kind_label(kind: DidKind) -> String {
    match kind {
        DidKind::Landblock(LandblockKind::Other(k)) if kind.name() == "landblock (other)" => {
            format!("landblock kind {k}")
        }
        k => k.name().to_owned(),
    }
}

/// Whether `census` decodes `did`: what `show` decodes, plus mesh records (0x06).
fn decodable(did: u32) -> bool {
    world::is_decodable(did) || Did(did).kind() == DidKind::Mesh
}

fn layout(row: &mut KindRow, r: LayoutRule) {
    match r {
        LayoutRule::Stamp => {}
        LayoutRule::Fallback => row.fallback += 1,
        LayoutRule::UnobservedStamp => row.unobserved += 1,
    }
}

/// Keys below `limit` (triangle-indexed hosts).
fn below(w: &TreeWalk, limit: u64) -> KeyCheck {
    match w.keys.last() {
        Some(&k) if u64::from(k) >= limit => KeyCheck::OutOfRange(k),
        _ => KeyCheck::Valid,
    }
}

/// Keys of a mesh level (`collision.md` §3.5): draw position · 2²⁰ + triangle index in
/// that draw's index buffer. If the plain numbering fails, the level is accepted as
/// renumbered when leaving out one draw that has exactly one triangle and is not the last
/// (the observed form of the exception) makes every key valid.
fn level_keys(mesh: &Mesh, level: &Level, keys: &[u32]) -> KeyCheck {
    let triangles: Vec<usize> = level
        .draws
        .iter()
        .map(|d| {
            mesh.index_buffers
                .get(d.index_buffer as usize)
                .map_or(0, tessera_mesh::IndexBuffer::triangle_count)
        })
        .collect();
    // Keys are sorted, so the subparts come in ascending order: keep the largest triangle
    // index of each (and its key, for the report).
    let mut groups: Vec<(usize, u32, u32)> = Vec::new();
    for &k in keys {
        let s = (k >> SUBPART_SHIFT) as usize;
        let t = k & ((1 << SUBPART_SHIFT) - 1);
        match groups.last_mut() {
            Some(g) if g.0 == s => {
                if t > g.1 {
                    (g.1, g.2) = (t, k);
                }
            }
            _ => groups.push((s, t, k)),
        }
    }
    let first_bad = |skip: Option<usize>| {
        groups.iter().find_map(|&(s, t, k)| {
            let draw = match skip {
                Some(d) if s >= d => s + 1,
                _ => s,
            };
            let ok = triangles.get(draw).is_some_and(|&n| (t as usize) < n);
            (!ok).then_some(k)
        })
    };
    let Some(bad) = first_bad(None) else {
        return KeyCheck::Valid;
    };
    let last = triangles.len().saturating_sub(1);
    if (0..last).any(|d| triangles[d] == 1 && first_bad(Some(d)).is_none()) {
        KeyCheck::Renumbered
    } else {
        KeyCheck::OutOfRange(bad)
    }
}

/// Walks one tree, checks its keys with `check` and counts the result.
#[allow(clippy::too_many_arguments)]
fn walk(
    tally: &mut Tally,
    host: Host,
    code: TreeCode<'_>,
    check: impl FnOnce(&TreeWalk) -> KeyCheck,
    what: &str,
    did: u32,
    archive: &str,
    max: usize,
) {
    let row = tally.trees.entry(host).or_default();
    let problem = match code.walk() {
        Ok(w) => {
            row.walked += 1;
            match check(&w) {
                KeyCheck::Valid => None,
                KeyCheck::Renumbered => {
                    row.renumbered += 1;
                    None
                }
                KeyCheck::OutOfRange(k) => {
                    row.keys_out_of_range += 1;
                    Some(format!(
                        "{}{what}: key {k} not valid for its host",
                        host.name()
                    ))
                }
            }
        }
        Err(e) => {
            row.failed += 1;
            Some(format!("{}{what}: {e}", host.name()))
        }
    };
    if let Some(error) = problem
        && tally.tree_failures.len() < max
    {
        tally.tree_failures.push(Failure {
            did,
            archive: archive.to_owned(),
            error,
            category: "TreeWalk",
        });
    }
}

fn mesh_trees(tally: &mut Tally, host: Host, m: &CollisionMesh, did: u32, a: &str, max: usize) {
    let limit = u64::try_from(m.triangles.len()).unwrap_or(u64::MAX);
    walk(
        tally,
        host,
        m.tree_code(),
        |w| below(w, limit),
        "",
        did,
        a,
        max,
    );
}

/// Walks the tree of every level of a render mesh; returns the number of coded levels.
fn level_trees(tally: &mut Tally, host: Host, mesh: &Mesh, did: u32, a: &str, max: usize) -> u64 {
    let mut coded = 0;
    for (n, level) in mesh.levels.iter().enumerate() {
        let code = match &level.tree {
            None => continue,
            Some(CollisionTree::NoCode) => {
                tally.trees.entry(host).or_default().without_code += 1;
                continue;
            }
            Some(CollisionTree::Code(c)) => c,
        };
        coded += 1;
        // `MESH_OPTIONS` keeps the code, so it is always present.
        let bytes = code.code.as_deref().unwrap_or_default();
        walk(
            tally,
            host,
            TreeCode::new(code.origin, code.scale, bytes),
            |w| level_keys(mesh, level, &w.keys),
            &format!(" (level {n})"),
            did,
            a,
            max,
        );
    }
    coded
}

/// Failure category of a render mesh that does not decode.
fn mesh_category(e: &MeshError) -> &'static str {
    use tessera_mesh::FailureCategory as C;
    match e.category() {
        C::Truncated => "RenderMesh/Truncated",
        C::UnsupportedVertexFormat => "RenderMesh/UnsupportedVertexFormat",
        C::IndexOutOfRange => "RenderMesh/IndexOutOfRange",
        C::Structure => "RenderMesh/Structure",
        C::TrailingBytes => "RenderMesh/TrailingBytes",
        C::LimitExceeded => "RenderMesh/LimitExceeded",
        _ => "RenderMesh/Other",
    }
}

/// A decoded record with what `census` decoded beyond [`world::decode`]: the embedded
/// render mesh of a 0x01 record.
struct Checked<'a> {
    rec: Option<Decoded<'a>>,
    mesh: Option<Mesh>,
}

/// Decodes one record for the census. `Ok(None)`: no decoder.
fn decode<'a>(
    did: u32,
    payload: &'a [u8],
    stamp: u16,
    schema: &Schema,
) -> Result<Option<Checked<'a>>, (String, &'static str)> {
    if Did(did).kind() == DidKind::Mesh {
        return Mesh::decode_expecting(payload, Did(did), Some(stamp), &MESH_OPTIONS)
            .map(|m| {
                Some(Checked {
                    rec: None,
                    mesh: Some(m),
                })
            })
            .map_err(|e| (e.to_string(), mesh_category(&e)));
    }
    let rec = match world::decode(did, payload, Some(stamp), schema.get()) {
        Ok(Some(r)) => r,
        Ok(None) => return Ok(None),
        Err(e) => return Err((e.to_string(), FailureCategory::of(&e).name())),
    };
    let mut mesh = None;
    if let Decoded::Any(a) = &rec
        && let AnyRecord::CellGeometry(c) = a.as_ref()
    {
        // Part 11 is a mesh record without its DID, decoded with the record's stamp (§5.7).
        let m = Mesh::decode(&c.render_mesh_record(), Some(stamp), &MESH_OPTIONS)
            .map_err(|e| (format!("embedded render mesh: {e}"), mesh_category(&e)))?;
        mesh = Some(m);
    }
    Ok(Some(Checked {
        rec: Some(rec),
        mesh,
    }))
}

/// Item count, layout rule and tree walks of one decoded record.
#[allow(clippy::too_many_arguments)]
fn account(
    tally: &mut Tally,
    key: &KindKey,
    checked: &Checked<'_>,
    pids: Option<&LandblockPids>,
    did: u32,
    archive: &str,
    max: usize,
) {
    let grid = GRID_TRIANGLE_COUNT as u64;
    let mut items = 0u64;
    let mut rule = None;
    match &checked.rec {
        None => {
            if let Some(m) = &checked.mesh {
                items = m.levels.len() as u64;
                level_trees(tally, Host::MeshLevel, m, did, archive, max);
            }
        }
        Some(Decoded::Any(a)) => match a.as_ref() {
            AnyRecord::TerrainTypeTable(t) => items = t.len() as u64,
            AnyRecord::TerrainSceneTable(t) => items = t.entries().len() as u64,
            AnyRecord::SceneChoiceTable(t) => items = t.entries().len() as u64,
            AnyRecord::Scene(t) => items = t.descendants().count() as u64,
            AnyRecord::WaterTable(t) => items = t.entries().len() as u64,
            AnyRecord::FogZoneTable(t) => items = t.entries().len() as u64,
            AnyRecord::SkyDescription(t) => items = t.groups.len() as u64,
            AnyRecord::DayDescription(t) => items = t.keyframes.len() as u64,
            AnyRecord::GameTime(t) => items = t.months.len() as u64,
            AnyRecord::LodClassTable(t) => items = t.len() as u64,
            AnyRecord::CollisionMesh { mesh, .. } => {
                items = mesh.triangles.len() as u64;
                rule = Some(mesh.layout_rule);
                mesh_trees(tally, Host::CollisionMesh, mesh, did, archive, max);
            }
            AnyRecord::CellGeometry(c) => {
                items = c.portals.len() as u64;
                rule = Some(c.layout_rule);
                if let Some(m) = &c.physics_mesh {
                    mesh_trees(tally, Host::CellPhysicsMesh, m, did, archive, max);
                }
                if let Some(m) = &checked.mesh {
                    level_trees(tally, Host::CellRenderMesh, m, did, archive, max);
                }
            }
            _ => {}
        },
        Some(Decoded::Region(r)) => {
            items = u64::from(r.tables().iter().all(|(_, d)| d.is_some()));
        }
        Some(Decoded::BlockMapDescriptor(d)) => items = d.entries.len() as u64,
        Some(Decoded::BlockMap(b)) => items = b.entries.len() as u64,
        Some(Decoded::LandblockData(d)) => {
            items = u64::from(d.ceiling.is_some());
            rule = Some(d.layout_rule);
            let check = |w: &TreeWalk| below(w, grid);
            if let Some(t) = &d.terrain_tree {
                let code = TreeCode::from(t);
                walk(tally, Host::Terrain, code, check, "", did, archive, max);
            }
            if let Some(t) = d.ceiling.as_ref().and_then(|c| c.tree.as_ref()) {
                let code = TreeCode::from(t);
                walk(tally, Host::Ceiling, code, check, "", did, archive, max);
            }
        }
        Some(Decoded::LandblockInfo(i)) => {
            items = i.cells.as_ref().map_or(0, |c| c.items.len() as u64);
            rule = Some(i.layout_rule);
        }
        Some(Decoded::LandblockProperties(p)) => {
            if let Some(pids) = pids {
                let lm = p.light_maps(pids);
                items = u64::from(lm.terrain.is_some()) + u64::from(lm.ceiling.is_some());
            }
        }
        Some(Decoded::LandblockGroup(_) | Decoded::Material { .. }) => {}
    }
    let row = tally.kinds.entry(*key).or_default();
    row.items += items;
    if let Some(r) = rule {
        layout(row, r);
    }
}

/// `census <dir|archive>`.
pub fn run(out: &mut Out, ctx: &Ctx, o: &Opts) -> Result<ExitCode> {
    let prefixes: Vec<u8> = if o.prefixes.is_empty() {
        WORLD_PREFIXES.to_vec()
    } else {
        o.prefixes.clone()
    };
    for &p in &prefixes {
        if p != 0x80 && !decodable(u32::from(p) << 24) {
            bail!(
                "no decoder for prefix 0x{p:02X} ({}); `census` decodes {}",
                Did(u32::from(p) << 24).kind(),
                "the prefixes `show` accepts and 06 (meshes); 80 selects the landblock family"
            );
        }
    }
    let mut wanted = [false; 256];
    for &p in &prefixes {
        wanted[usize::from(p)] = true;
    }

    let input = Input::open(&o.target, ctx)?;
    let unopened = input.unopened_recognised();
    for u in &unopened {
        tracing::warn!(path = %u.path.display(), "not opened: {}", u.error);
    }
    let archives = input.archives();

    // Collect the selected entries of every archive.
    let mut work: Vec<(usize, Entry)> = Vec::new();
    let mut walk_errors: Vec<(String, String)> = Vec::new();
    let mut skipped = 0u64;
    for (i, (label, a)) in archives.iter().enumerate() {
        for e in a.entries() {
            let e = match e {
                Ok(e) => e,
                Err(err) => {
                    walk_errors.push((label.clone(), err.to_string()));
                    break;
                }
            };
            if !wanted[usize::from(Did(e.did).prefix())] {
                continue;
            }
            if decodable(e.did) {
                work.push((i, e));
            } else {
                skipped += 1;
            }
        }
    }

    let schema = if work.iter().any(|(_, e)| world::needs_schema(e.did)) {
        Schema::load(&input, ctx)?
    } else {
        Schema::none("not needed")
    };
    let pids = schema.dict.as_ref().map(LandblockPids::from_dictionary);
    let max = o.max_failures;

    let chunks: Vec<&[(usize, Entry)]> = work.chunks(CHUNK).collect();
    let partial = par_map(&chunks, |chunk| {
        census_chunk(chunk, &archives, &schema, pids.as_ref(), max)
    });
    let mut tally = Tally::default();
    for p in partial {
        tally.merge(p, max);
    }

    let complete = walk_errors.is_empty() && unopened.is_empty();
    let ok = complete && tally.failures_total == 0 && tally.tree_problems() == 0;
    if out.json {
        write_json(
            out,
            &o.target,
            &tally,
            &schema,
            skipped,
            &walk_errors,
            &unopened,
            ok,
        )?;
    } else {
        write_text(
            out,
            &o.target,
            &tally,
            &schema,
            skipped,
            &walk_errors,
            &unopened,
            ok,
        )?;
    }
    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

fn census_chunk(
    chunk: &[(usize, Entry)],
    archives: &[(String, &Archive)],
    schema: &Schema,
    pids: Option<&LandblockPids>,
    max: usize,
) -> Tally {
    let mut tally = Tally::default();
    for (i, e) in chunk {
        let Some((label, a)) = archives.get(*i) else {
            continue;
        };
        let key: KindKey = (Did(e.did).prefix(), Did(e.did).kind());
        let row = tally.kinds.entry(key).or_default();
        row.records += 1;
        let fail = |tally: &mut Tally, error: String, category: &'static str| {
            let row = tally.kinds.entry(key).or_default();
            row.failed += 1;
            *row.categories.entry(category).or_default() += 1;
            tally.failures_total += 1;
            if tally.failures.len() < max {
                tally.failures.push(Failure {
                    did: e.did,
                    archive: label.clone(),
                    error,
                    category,
                });
            }
        };
        let payload = match a.read_entry(e) {
            Ok(p) => p,
            Err(err) => {
                fail(&mut tally, err.to_string(), "Read");
                continue;
            }
        };
        row.bytes += payload.len() as u64;
        match decode(e.did, &payload, e.stamp, schema) {
            Ok(Some(checked)) => {
                if let Some(r) = tally.kinds.get_mut(&key) {
                    r.decoded += 1;
                }
                account(&mut tally, &key, &checked, pids, e.did, label, max);
            }
            Ok(None) => fail(&mut tally, "no decoder".into(), "NoDecoder"),
            Err((error, category)) => fail(&mut tally, error, category),
        }
    }
    tally
}

#[allow(clippy::too_many_arguments)]
fn write_json(
    out: &mut Out,
    target: &str,
    t: &Tally,
    schema: &Schema,
    skipped: u64,
    walk_errors: &[(String, String)],
    unopened: &[crate::input::Unopened],
    ok: bool,
) -> Result<()> {
    let kinds: Vec<Value> = t
        .kinds
        .iter()
        .map(|((p, kind), r)| {
            json!({
                "prefix": format!("0x{p:02X}"),
                "kind": kind_label(*kind),
                "records": r.records,
                "decoded": r.decoded,
                "failed": r.failed,
                "payload_bytes": r.bytes,
                "items": r.items,
                "item_label": item_label(*kind),
                "layout_fallback": r.fallback,
                "layout_unobserved_stamp": r.unobserved,
                "failure_categories": r.categories,
            })
        })
        .collect();
    let trees: Vec<Value> = t
        .trees
        .iter()
        .map(|(h, r)| {
            json!({
                "host": h.name(),
                "walked": r.walked,
                "failed": r.failed,
                "keys_out_of_range": r.keys_out_of_range,
                "renumbered": r.renumbered,
                "without_code": r.without_code,
            })
        })
        .collect();
    let fail_json = |f: &Failure| {
        json!({
            "did": did_hex(f.did),
            "archive": f.archive,
            "category": f.category,
            "error": f.error,
        })
    };
    out.json(&json!({
        "target": target,
        "dictionary": schema.source,
        "kinds": kinds,
        "trees": trees,
        "skipped_without_decoder": skipped,
        "failures_total": t.failures_total,
        "failures": t.failures.iter().map(fail_json).collect::<Vec<_>>(),
        "tree_failures": t.tree_failures.iter().map(fail_json).collect::<Vec<_>>(),
        "walk_errors": walk_errors.iter().map(|(a, e)| json!({"archive": a, "error": e})).collect::<Vec<_>>(),
        "unopened": unopened.iter().map(|u| json!({"archive": u.label, "error": u.error})).collect::<Vec<_>>(),
        "complete": walk_errors.is_empty() && unopened.is_empty(),
        "ok": ok,
    }))
}

#[allow(clippy::too_many_arguments)]
fn write_text(
    out: &mut Out,
    target: &str,
    t: &Tally,
    schema: &Schema,
    skipped: u64,
    walk_errors: &[(String, String)],
    unopened: &[crate::input::Unopened],
    ok: bool,
) -> Result<()> {
    let records: u64 = t.kinds.values().map(|r| r.records).sum();
    writeln!(
        out.w,
        "census of {target}: {records} records of {} kinds (property dictionary: {})",
        t.kinds.len(),
        schema.source
    )?;
    let mut tab = Table::new(&[
        ("PREFIX", Align::Left),
        ("KIND", Align::Left),
        ("RECORDS", Align::Right),
        ("DECODED", Align::Right),
        ("FAILED", Align::Right),
        ("PAYLOAD BYTES", Align::Right),
        ("ITEMS", Align::Right),
        ("", Align::Left),
    ]);
    for ((p, kind), r) in &t.kinds {
        tab.row(vec![
            format!("0x{p:02X}"),
            kind_label(*kind),
            r.records.to_string(),
            r.decoded.to_string(),
            r.failed.to_string(),
            r.bytes.to_string(),
            r.items.to_string(),
            item_label(*kind).to_owned(),
        ]);
    }
    tab.write(&mut out.w)?;
    if !t.trees.is_empty() {
        writeln!(out.w)?;
        let mut tab = Table::new(&[
            ("TREES", Align::Left),
            ("WALKED", Align::Right),
            ("FAILED", Align::Right),
            ("KEYS OUT OF RANGE", Align::Right),
            ("RENUMBERED", Align::Right),
            ("WITHOUT CODE", Align::Right),
        ]);
        for (h, r) in &t.trees {
            tab.row(vec![
                h.name().to_owned(),
                r.walked.to_string(),
                r.failed.to_string(),
                r.keys_out_of_range.to_string(),
                r.renumbered.to_string(),
                r.without_code.to_string(),
            ]);
        }
        tab.write(&mut out.w)?;
    }
    let fallback: u64 = t.kinds.values().map(|r| r.fallback).sum();
    let unobserved: u64 = t.kinds.values().map(|r| r.unobserved).sum();
    if fallback + unobserved > 0 {
        writeln!(
            out.w,
            "layout: {fallback} records decided by the fallback without a stamp, {unobserved} \
             with an unobserved stamp"
        )?;
    }
    if skipped > 0 {
        writeln!(
            out.w,
            "{skipped} records of the selected prefixes have no decoder (not counted)"
        )?;
    }
    for (a, e) in walk_errors {
        writeln!(out.w, "directory walk of {a} stopped early: {e}")?;
    }
    for u in unopened {
        writeln!(
            out.w,
            "archive {} could not be opened: {}",
            u.label, u.error
        )?;
    }
    if t.failures_total > 0 {
        writeln!(
            out.w,
            "\nfailures ({} listed of {}):",
            t.failures.len(),
            t.failures_total
        )?;
        for f in &t.failures {
            writeln!(
                out.w,
                "  {} in {} [{}]: {}",
                did_hex(f.did),
                f.archive,
                f.category,
                f.error
            )?;
        }
    }
    if !t.tree_failures.is_empty() {
        writeln!(out.w, "\ntree problems (first {}):", t.tree_failures.len())?;
        for f in &t.tree_failures {
            writeln!(out.w, "  {} in {}: {}", did_hex(f.did), f.archive, f.error)?;
        }
    }
    writeln!(
        out.w,
        "\nresult: {}",
        if ok {
            "every selected record decoded, every tree in them walked with valid keys"
        } else {
            "FAILED"
        }
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_follow_kinds() {
        let k = |did: u32| Did(did).kind();
        assert_eq!(item_label(k(0x1900_0001)), "entries");
        assert_eq!(item_label(k(0x3200_0000)), "classes");
        assert_eq!(item_label(k(0x8001_0203)), "with ceiling");
        assert_eq!(item_label(k(0x8051_0203)), "");
        assert_eq!(item_label(k(0x0600_0001)), "levels");
        // Both labels come from the kind, so `show` and `census` name K = 5 alike.
        assert_eq!(kind_label(k(0x8051_0203)), "distant-landscape sector");
        assert_eq!(kind_label(k(0x8061_0203)), "landblock kind 6");
        assert_eq!(kind_label(k(0x8001_0203)), "landblock heightmap");
        for p in WORLD_PREFIXES {
            assert!(world::is_decodable(u32::from(p) << 24 | 1), "{p:#x}");
        }
        assert!(decodable(0x0600_0001));
        assert!(!world::is_decodable(0x0600_0001));
        assert!(!decodable(0x1400_0001));
    }

    /// A mesh whose one level draws index buffers with these triangle counts.
    fn level_mesh(triangles: &[usize]) -> Mesh {
        let mut m = tessera_mesh::synth::MeshSpec::single(0x0000_0112, 3)
            .build()
            .expect("synthetic mesh");
        m.index_buffers = triangles
            .iter()
            .map(|&n| tessera_mesh::IndexBuffer {
                indices: vec![0; n * 3],
            })
            .collect();
        let draw = m.levels[0].draws[0];
        m.levels[0].draws = (0..triangles.len() as u32)
            .map(|i| tessera_mesh::Draw {
                index_buffer: i,
                ..draw
            })
            .collect();
        m
    }

    #[test]
    fn mesh_level_keys_follow_the_draw_numbering() {
        let key = |s: u32, t: u32| s << SUBPART_SHIFT | t;
        let m = level_mesh(&[4, 1, 3]);
        let l = &m.levels[0];
        assert_eq!(level_keys(&m, l, &[]), KeyCheck::Valid);
        let plain = [key(0, 0), key(0, 3), key(1, 0), key(2, 2)];
        assert_eq!(level_keys(&m, l, &plain), KeyCheck::Valid);
        // Draw 1 (one triangle) left out: the third draw is numbered 1 (§3.5).
        let renumbered = [key(0, 3), key(1, 2)];
        assert_eq!(level_keys(&m, l, &renumbered), KeyCheck::Renumbered);
        // A triangle past its draw, or a draw past the level, is out of range.
        assert_eq!(
            level_keys(&m, l, &[key(0, 4)]),
            KeyCheck::OutOfRange(key(0, 4))
        );
        assert_eq!(
            level_keys(&m, l, &[key(0, 0), key(3, 0)]),
            KeyCheck::OutOfRange(key(3, 0))
        );
        // Only a single-triangle draw that is not the last may be left out.
        let m = level_mesh(&[4, 2, 3]);
        assert_eq!(
            level_keys(&m, &m.levels[0], &renumbered),
            KeyCheck::OutOfRange(key(1, 2))
        );
        let m = level_mesh(&[4, 3, 1]);
        assert_eq!(level_keys(&m, &m.levels[0], &[key(2, 0)]), KeyCheck::Valid);
        assert_eq!(
            level_keys(&m, &m.levels[0], &[key(3, 0)]),
            KeyCheck::OutOfRange(key(3, 0))
        );
    }

    #[test]
    fn merge_caps_failures() {
        let f = |did| Failure {
            did,
            archive: "a".into(),
            error: "e".into(),
            category: "Truncated",
        };
        let mut a = Tally {
            failures: vec![f(1)],
            failures_total: 1,
            ..Tally::default()
        };
        let b = Tally {
            failures: vec![f(2), f(3)],
            failures_total: 2,
            ..Tally::default()
        };
        a.merge(b, 2);
        assert_eq!(a.failures_total, 3);
        assert_eq!(
            a.failures.iter().map(|f| f.did).collect::<Vec<_>>(),
            vec![1, 2]
        );
    }
}
