// SPDX-License-Identifier: GPL-3.0-or-later
//! Realising placed decals (`decals.md` §2.3, §4.3, §7, §9.1): the Decal SEDs of a
//! landblock that the content-layer test lets through, their parameters and projection
//! volumes, and the receiving triangles a build collects from the terrain grid and from
//! placed meshes.
//!
//! | Item | Purpose | Spec |
//! |---|---|---|
//! | [`decal_seds`], [`DecalSed`] | The Decal SEDs of a bundle that the content-layer test passes, with their own property sets | §2.3, ED §9.3, §4.5.1 |
//! | [`PlacedDecal::realise`] | A decal object: parameters by §3.2, the box of §4.2, the LOD distance of §7 step 1 | §2.3 steps 1–3, §3.2, §4.2, §7 |
//! | [`terrain_receivers`], [`TerrainGrid`] | The full-resolution terrain triangles around the box with the vertex normals of `landblock.md` §3.14 | §4.3 step 2 |
//! | [`mesh_receivers`] | The triangles of a placed mesh that meet the box's search box | §4.3 step 1 |
//! | [`default_material`] | The `UNIQUEDB` default decal material (key 0x29) | §3.1 |
//!
//! Everything works on decoded records; nothing indexes by a data-derived value and every
//! output is bounded ([`MAX_RECEIVER_TRIANGLES`]).

use std::collections::HashMap;

use tessera_formats::landblock::sector_normals::{NeighbourWindow, NormalSurface, rule_normal};
use tessera_formats::landblock::{GRID_SPACING, InstanceId, QUAD_SIDE, StaticEntityType};
use tessera_formats::setup::Quaternion;
use tessera_formats::{Did, LandblockData, PropertySet};
use tessera_physics::geometry::content_layers_pass;

use super::builder::{DecalBox, ReceiverKind, ReceiverTriangle, ReceiverVertex};
use super::shading::lod_distance;
use super::{DecalParams, DecalPids, INITIAL_LOD_DISTANCE};
use crate::bundle::{LandblockBundle, PlacedEntity};
use crate::draw_distance::LodClasses;

/// Most receiving triangles one build collects (terrain and meshes together). A 20 m box
/// meets at most a few hundred terrain triangles; the bound is for hostile meshes. Beyond
/// it further triangles are left out (logged by the caller through
/// [`ReceiverBudget::overflowed`]).
pub const MAX_RECEIVER_TRIANGLES: usize = 1 << 18;

/// Size of a landblock in units (32 quads of 5 units; `crate::coords::LANDBLOCK_SIZE`).
#[allow(clippy::cast_precision_loss)]
pub const LANDBLOCK_UNITS: f32 = GRID_SPACING * QUAD_SIDE as f32;

/// A Decal SED of a landblock (§2.3): the placed entity and its own property set.
#[derive(Debug, Clone, Copy)]
pub struct DecalSed<'a> {
    /// The placed entity (kind Decal; frame, interior flag, marker object).
    pub entity: &'a PlacedEntity,
    /// The SED's own property set (§3.2 step 2), when the bundle kept the info record.
    pub own: Option<&'a PropertySet>,
}

/// The Decal SEDs of `bundle` in stored order whose content-layer test passes for the
/// shared world with the active layers `active` (`entity-descriptor.md` §4.5.1, §9.3:
/// tested at every pass of the landblock; the interior-viewer rule of §4.8 is the
/// caller's).
#[must_use]
pub fn decal_seds<'a>(bundle: &'a LandblockBundle, active: &[u32]) -> Vec<DecalSed<'a>> {
    let info = bundle.info.as_ref();
    bundle
        .entities
        .iter()
        .filter(|e| e.kind == Some(StaticEntityType::Decal))
        .filter(|e| content_layers_pass(e.content_layers.as_deref(), active, 0))
        .map(|e| {
            let own = info
                .and_then(|i| {
                    usize::try_from(e.index)
                        .ok()
                        .and_then(|k| i.entities.get(k))
                })
                .filter(|s| s.kind() == Some(StaticEntityType::Decal))
                .and_then(|s| s.properties.as_ref());
            DecalSed { entity: e, own }
        })
        .collect()
}

/// A realised decal object (§2.3): its parameters, frame and projection volume.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedDecal {
    /// The SED's instance ID (the per-manager table key, §2.3 step 5).
    pub iid: Option<InstanceId>,
    /// The SED's interior flag (cell 1, §2.3 step 2; ED §4.8).
    pub interior: bool,
    /// The effective parameters (§3).
    pub params: DecalParams,
    /// The projection volume (§4.2) in the landblock frame.
    pub volume: DecalBox,
    /// The LOD distance *L* (§7 step 1).
    pub lod_distance: f32,
}

impl PlacedDecal {
    /// The decal object of a SED at `position` with rotation `rotation` (landblock frame):
    /// the initial values (§3.1, `default_material`), then the marker chain **root parent
    /// first** (`chain_root_first`), then `own` (§3.2); the box of §4.2 (scale not used,
    /// §2.3); *L* from `lod_classes` (§7 step 1: the class's first value, else 30 m).
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn realise(
        pids: &DecalPids,
        default_material: Did,
        chain_root_first: &[&PropertySet],
        own: Option<&PropertySet>,
        position: [f32; 3],
        rotation: Quaternion,
        iid: Option<InstanceId>,
        interior: bool,
        lod_classes: Option<&LodClasses>,
    ) -> Self {
        let params = DecalParams::collect(pids, default_material, chain_root_first, own);
        let volume = DecalBox::new(position, rotation, params.ortho_scale);
        Self {
            iid,
            interior,
            params,
            volume,
            lod_distance: decal_lod_distance(lod_classes, params.lod_class),
        }
    }
}

/// The LOD distance of `class` (§7 step 1): the first value of the class's entry in the
/// LOD-class table, else [`INITIAL_LOD_DISTANCE`].
#[must_use]
pub fn decal_lod_distance(lod_classes: Option<&LodClasses>, class: u32) -> f32 {
    let d0 = lod_classes
        .and_then(|c| c.table().get(class))
        .map(|e| e.values[0]);
    lod_distance(d0, INITIAL_LOD_DISTANCE)
}

/// Counts receiving triangles against a limit ([`MAX_RECEIVER_TRIANGLES`] by default).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReceiverBudget {
    used: usize,
    limit: usize,
    overflowed: bool,
}

impl Default for ReceiverBudget {
    fn default() -> Self {
        Self::with_limit(MAX_RECEIVER_TRIANGLES)
    }
}

impl ReceiverBudget {
    /// A budget of `limit` triangles, at most [`MAX_RECEIVER_TRIANGLES`] (a caller's
    /// shared budget for several decals hands each the rest it has left).
    #[must_use]
    pub fn with_limit(limit: usize) -> Self {
        Self {
            used: 0,
            limit: limit.min(MAX_RECEIVER_TRIANGLES),
            overflowed: false,
        }
    }

    /// Triangles taken.
    #[must_use]
    pub fn used(&self) -> usize {
        self.used
    }

    /// Takes one slot; `false` (and the budget marked) when none is left.
    pub fn take(&mut self) -> bool {
        if self.used >= self.limit {
            self.overflowed = true;
            return false;
        }
        self.used += 1;
        true
    }

    /// Whether a triangle was left out.
    #[must_use]
    pub fn overflowed(&self) -> bool {
        self.overflowed
    }
}

/// The terrain records around a decal's landblock.
pub trait TerrainGrid {
    /// The landblock-data record at offset (`dx`, `dy`) from the decal's landblock
    /// (−2 ≤ `dx`, `dy` ≤ 2), or `None` when it is not loaded (§4.3 step 2: a landblock
    /// whose terrain is not loaded contributes nothing).
    fn data(&self, dx: i32, dy: i32) -> Option<&LandblockData>;
}

fn window<G: TerrainGrid + ?Sized>(grid: &G, dx: i32, dy: i32) -> NeighbourWindow<'_> {
    let mut w = NeighbourWindow::default();
    for (row, oy) in w.0.iter_mut().zip(-1..=1) {
        for (cell, ox) in row.iter_mut().zip(-1..=1) {
            *cell = grid.data(dx + ox, dy + oy);
        }
    }
    w
}

/// Quad indices 0 ..= 31 whose span [5 i, 5 i + 5] meets [`lo`, `hi`].
fn quad_range(lo: f32, hi: f32) -> Option<(usize, usize)> {
    if !(lo.is_finite() && hi.is_finite()) || hi < 0.0 || lo > LANDBLOCK_UNITS {
        return None;
    }
    let first = (lo / GRID_SPACING).floor().max(0.0);
    let last = (hi / GRID_SPACING).floor().min((QUAD_SIDE - 1) as f32);
    if first > last {
        return None;
    }
    // Both in 0 ..= 31 after the clamps.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Some((first as usize, last as usize))
}

/// The terrain triangles of §4.3 step 2 for the box `b` (landblock frame of the decal's
/// landblock): every full-resolution quad of the landblocks at offsets −1 ..= 1 that meets
/// the horizontal square of half-width |*e*| around *c*, split by its stored diagonal
/// (`landblock.md` §3.6; counter-clockwise from above), with the vertex normals of
/// `landblock.md` §3.14 (terrain, read through the neighbouring records; a sample without
/// a contributing triangle takes +Z). Positions are in the decal's landblock frame.
/// Appended to `out` within `budget`; an outdoor test and `StickToLandscape` are the
/// caller's.
pub fn terrain_receivers<G: TerrainGrid + ?Sized>(
    b: &DecalBox,
    grid: &G,
    budget: &mut ReceiverBudget,
    out: &mut Vec<ReceiverTriangle>,
) {
    let hw = b.terrain_half_width();
    let c = b.center;
    if !(hw.is_finite() && c.iter().all(|v| v.is_finite())) {
        return;
    }
    for dy in -1..=1i32 {
        for dx in -1..=1i32 {
            let Some(data) = grid.data(dx, dy) else {
                continue;
            };
            #[allow(clippy::cast_precision_loss)]
            let (ox, oy) = (dx as f32 * LANDBLOCK_UNITS, dy as f32 * LANDBLOCK_UNITS);
            let Some((i0, i1)) = quad_range(c[0] - hw - ox, c[0] + hw - ox) else {
                continue;
            };
            let Some((j0, j1)) = quad_range(c[1] - hw - oy, c[1] + hw - oy) else {
                continue;
            };
            let nb = window(grid, dx, dy);
            let mut normals: HashMap<(usize, usize), [f32; 3]> = HashMap::new();
            let mut vertex = |i: usize, j: usize| -> Option<ReceiverVertex> {
                let h = data.heights.height(i, j)?;
                let n = *normals.entry((i, j)).or_insert_with(|| {
                    #[allow(clippy::cast_possible_truncation)]
                    rule_normal(&nb, NormalSurface::Terrain, i, j)
                        .map(|n| n.map(|c| c as f32))
                        .filter(|n| n.iter().all(|c| c.is_finite()))
                        .unwrap_or([0.0, 0.0, 1.0])
                });
                #[allow(clippy::cast_precision_loss)]
                let p = [
                    ox + GRID_SPACING * i as f32,
                    oy + GRID_SPACING * j as f32,
                    h,
                ];
                Some(ReceiverVertex {
                    position: p,
                    normal: n,
                })
            };
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let (Some(sw), Some(se), Some(nw), Some(ne)) = (
                        vertex(i, j),
                        vertex(i + 1, j),
                        vertex(i, j + 1),
                        vertex(i + 1, j + 1),
                    ) else {
                        continue;
                    };
                    let tris = if data.vertex_flags.split_sw_ne(i, j) {
                        [[sw, se, ne], [sw, ne, nw]]
                    } else {
                        [[sw, se, nw], [se, ne, nw]]
                    };
                    for vertices in tris {
                        if !budget.take() {
                            return;
                        }
                        out.push(ReceiverTriangle {
                            kind: ReceiverKind::Terrain,
                            vertices,
                        });
                    }
                }
            }
        }
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The triangles of one placed mesh for §4.3 step 1: `triangles` index `positions` and
/// `normals` (object space); `place` moves a position into the decal's frame (the
/// object's scale, rotation and position) and `turn` a normal (its rotation). Triangles
/// whose box misses the box's search box (§4.3 step 1, a superset is harmless: §4.5 clips)
/// and triangles with an index out of range are left out; a vertex without a normal takes
/// the triangle's geometric normal. Appended to `out` within `budget`.
#[allow(clippy::too_many_arguments)]
pub fn mesh_receivers(
    b: &DecalBox,
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    triangles: impl IntoIterator<Item = [u32; 3]>,
    place: impl Fn([f32; 3]) -> [f32; 3],
    turn: impl Fn([f32; 3]) -> [f32; 3],
    budget: &mut ReceiverBudget,
    out: &mut Vec<ReceiverTriangle>,
) {
    let (lo, hi) = b.query_aabb();
    for t in triangles {
        let mut p = [[0.0f32; 3]; 3];
        let mut ok = true;
        for (dst, &k) in p.iter_mut().zip(&t) {
            match usize::try_from(k).ok().and_then(|k| positions.get(k)) {
                Some(v) => *dst = place(*v),
                None => ok = false,
            }
        }
        if !ok || !p.iter().flatten().all(|v| v.is_finite()) {
            continue;
        }
        let misses = (0..3).any(|a| {
            let tl = p[0][a].min(p[1][a]).min(p[2][a]);
            let th = p[0][a].max(p[1][a]).max(p[2][a]);
            th < lo[a] || tl > hi[a]
        });
        if misses {
            continue;
        }
        let e1 = sub(p[1], p[0]);
        let e2 = sub(p[2], p[0]);
        let face = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        let fl = (face[0] * face[0] + face[1] * face[1] + face[2] * face[2]).sqrt();
        let face = if fl > 1e-12 && fl.is_finite() {
            face.map(|c| c / fl)
        } else {
            [0.0; 3]
        };
        let mut vertices = [ReceiverVertex {
            position: [0.0; 3],
            normal: face,
        }; 3];
        for ((v, pos), &k) in vertices.iter_mut().zip(p).zip(&t) {
            v.position = pos;
            if let Some(n) = usize::try_from(k).ok().and_then(|k| normals.get(k)) {
                v.normal = turn(*n);
            }
        }
        if !budget.take() {
            return;
        }
        out.push(ReceiverTriangle {
            kind: ReceiverKind::Object,
            vertices,
        });
    }
}

/// The default decal material (§3.1): the DID the `UNIQUEDB` map holds under key 0x29
/// ([`super::DEFAULT_MATERIAL_UNIQUEDB_KEY`]) in the install behind `ctx`; `None` when the
/// maps or the key are missing.
#[must_use]
pub fn default_material(ctx: &crate::context::WorldContext) -> Option<Did> {
    use crate::context::ROOT_DID_MAP;
    use tessera_formats::{DataIdMap, Record};
    let source = ctx.source();
    let root = source.read(Did(ROOT_DID_MAP)).ok().flatten()?;
    let root = DataIdMap::parse_expecting(&root, ROOT_DID_MAP).ok()?;
    let unique = root.by_label(crate::draw_distance::UNIQUEDB_LABEL)?;
    let map = source.read(unique).ok().flatten()?;
    let map = DataIdMap::parse_expecting(&map, unique.0).ok()?;
    map.get(super::DEFAULT_MATERIAL_UNIQUEDB_KEY)
        .map(|e| e.value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decals::{DecalParams, build_mesh};
    use tessera_formats::landblock::HeightGrid;

    struct Grid(HashMap<(i32, i32), LandblockData>);

    impl TerrainGrid for Grid {
        fn data(&self, dx: i32, dy: i32) -> Option<&LandblockData> {
            self.0.get(&(dx, dy))
        }
    }

    fn flat(raw: u16) -> LandblockData {
        LandblockData::from_heights(Did(0x8001_0000), HeightGrid::from_fn(|_, _| raw))
    }

    /// The down-projecting rotation of worked example 1 (−90° about X).
    fn down() -> Quaternion {
        Quaternion {
            w: std::f32::consts::FRAC_1_SQRT_2,
            x: -std::f32::consts::FRAC_1_SQRT_2,
            y: 0.0,
            z: 0.0,
        }
    }

    /// `decals.md` §11 example 1 on terrain: flat terrain at the height of a stored
    /// sample, a 4 × 6 m ground decal, every output vertex in the box, lifted 0.008 m and
    /// with the corner coordinates of the example.
    #[test]
    fn a_ground_decal_on_flat_terrain() {
        let raw = 3_276u16;
        let z = tessera_formats::landblock::height_from_raw(raw);
        let mut g = Grid(HashMap::new());
        for dy in -1..=1 {
            for dx in -1..=1 {
                g.0.insert((dx, dy), flat(raw));
            }
        }
        let params = DecalParams {
            ortho_scale: [4.0, 2.0, 6.0],
            ..DecalParams::initial(Did(0x3100_0008))
        };
        let b = DecalBox::new([80.0, 80.0, z + 1.0], down(), params.ortho_scale);
        let mut budget = ReceiverBudget::default();
        let mut tris = Vec::new();
        terrain_receivers(&b, &g, &mut budget, &mut tris);
        assert!(!tris.is_empty() && !budget.overflowed());
        // Flat terrain: §3.14 normals are +Z.
        for t in &tris {
            for v in &t.vertices {
                assert!((v.normal[2] - 1.0).abs() < 1e-5, "{:?}", v.normal);
            }
        }
        let mesh = build_mesh(&params, &b, tris, None);
        assert!(mesh.triangle_count() >= 2);
        let mut area = 0.0f32;
        for t in mesh.vertices.as_chunks::<3>().0 {
            for v in t {
                assert!((v.position[2] - (z + crate::decals::LIFT)).abs() < 1e-3);
                assert!(v.position[0] >= 78.0 - 1e-3 && v.position[0] <= 82.0 + 1e-3);
                assert!(v.position[1] >= 77.0 - 1e-3 && v.position[1] <= 83.0 + 1e-3);
                assert!((-1e-5..=1.0 + 1e-5).contains(&v.uv[0]));
                assert!((-1e-5..=1.0 + 1e-5).contains(&v.uv[1]));
            }
            let e1 = sub(t[1].position, t[0].position);
            let e2 = sub(t[2].position, t[0].position);
            area += 0.5 * (e1[0] * e2[1] - e1[1] * e2[0]);
        }
        assert!(
            (area - 24.0).abs() < 1e-2,
            "the 4 m × 6 m rectangle: {area}"
        );
        // Corner (82, 83): u = 1, v = 0.
        let corner = mesh
            .vertices
            .iter()
            .find(|v| (v.position[0] - 82.0).abs() < 1e-3 && (v.position[1] - 83.0).abs() < 1e-3)
            .expect("corner");
        assert!((corner.uv[0] - 1.0).abs() < 1e-4 && corner.uv[1].abs() < 1e-4);
    }

    /// The square around the box reaches into the neighbouring landblock; a landblock
    /// whose terrain is not loaded contributes nothing.
    #[test]
    fn neighbours_contribute_only_when_loaded() {
        let mut g = Grid(HashMap::new());
        g.0.insert((0, 0), flat(1000));
        let b = DecalBox::new([158.0, 80.0, 40.0], down(), [4.0, 2.0, 4.0]);
        let mut budget = ReceiverBudget::default();
        let mut only_own = Vec::new();
        terrain_receivers(&b, &g, &mut budget, &mut only_own);
        assert!(
            only_own
                .iter()
                .flat_map(|t| t.vertices)
                .all(|v| v.position[0] <= 160.0)
        );
        g.0.insert((1, 0), flat(1000));
        let mut both = Vec::new();
        terrain_receivers(&b, &g, &mut budget, &mut both);
        assert!(both.len() > only_own.len());
        assert!(
            both.iter()
                .flat_map(|t| t.vertices)
                .any(|v| v.position[0] > 160.0)
        );
        // Every triangle is counter-clockwise from above.
        for t in &both {
            let [a, b, c] = t.vertices.map(|v| v.position);
            let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
            assert!(cross > 0.0);
        }
        // A box far outside the landblocks finds nothing; hostile values neither.
        let mut none = Vec::new();
        terrain_receivers(
            &DecalBox::new([900.0, 80.0, 0.0], down(), [1.0; 3]),
            &g,
            &mut budget,
            &mut none,
        );
        terrain_receivers(
            &DecalBox::new([f32::NAN, 80.0, 0.0], down(), [1.0; 3]),
            &g,
            &mut budget,
            &mut none,
        );
        assert!(none.is_empty());
    }

    /// Mesh triangles: placed by the object's transform, filtered by the search box,
    /// out-of-range indices left out, a missing normal replaced by the face normal; the
    /// budget bounds the output.
    #[test]
    fn mesh_triangles_are_placed_and_filtered() {
        let b = DecalBox::new([0.0, 0.0, 1.0], down(), [2.0, 2.0, 2.0]);
        let positions = [
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [50.0, 50.0, 0.0],
            [51.0, 50.0, 0.0],
            [50.0, 51.0, 0.0],
        ];
        let normals = [[0.0, 0.0, 1.0]; 2];
        let mut budget = ReceiverBudget::default();
        let mut out = Vec::new();
        mesh_receivers(
            &b,
            &positions,
            &normals,
            [[0, 1, 2], [3, 4, 5], [0, 1, 99]],
            |p| [p[0], p[1], p[2] + 0.5],
            |n| n,
            &mut budget,
            &mut out,
        );
        assert_eq!(out.len(), 1);
        let t = out[0];
        assert_eq!(t.kind, ReceiverKind::Object);
        assert_eq!(t.vertices[0].position, [-1.0, -1.0, 0.5]);
        assert_eq!(t.vertices[1].normal, [0.0, 0.0, 1.0]);
        // Vertex 2 has no normal: the face normal (+Z for this winding).
        assert!((t.vertices[2].normal[2] - 1.0).abs() < 1e-6);
        let mesh = build_mesh(&DecalParams::initial(Did(1)), &b, out, None);
        assert_eq!(mesh.triangle_count(), 1, "inside the box: kept whole");
        let mut full = ReceiverBudget::with_limit(0);
        let mut none = Vec::new();
        mesh_receivers(
            &b,
            &positions,
            &normals,
            [[0, 1, 2]],
            |p| p,
            |n| n,
            &mut full,
            &mut none,
        );
        assert!(none.is_empty() && full.overflowed());
    }

    /// A shared budget hands out at most its limit, never more than the per-decal bound;
    /// the landblock size agrees with the world's coordinates.
    #[test]
    fn budgets_and_landblock_size() {
        let mut b = ReceiverBudget::with_limit(2);
        assert!(b.take() && b.take() && !b.take());
        assert_eq!(b.used(), 2);
        assert!(b.overflowed());
        let b = ReceiverBudget::with_limit(usize::MAX);
        assert_eq!(b, ReceiverBudget::default());
        assert!((f64::from(LANDBLOCK_UNITS) - crate::coords::LANDBLOCK_SIZE).abs() < 1e-9);
    }

    #[test]
    fn lod_distance_falls_back_to_the_initial_value() {
        assert_eq!(decal_lod_distance(None, 1), INITIAL_LOD_DISTANCE);
    }
}
