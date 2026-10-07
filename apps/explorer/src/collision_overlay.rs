// SPDX-License-Identifier: GPL-3.0-or-later
//! Collision overlays of the 3D view (M4.5), built on the workers from
//! `tessera-physics` geometry:
//!
//! | Selected record | Overlays | Spec |
//! |---|---|---|
//! | 0x66 collision mesh | its triangles ([`ShapeTemplate::from_collision_mesh`]) | `collision.md` §4 |
//! | 0x04 setup, 0x1F template, 0x47 entity, objects | the setup's 0x66 mesh, boxes, spheres and capsules ([`ShapeTemplate::from_setup`]) | §4.3, §8 |
//! | 0x01 cell geometry | physics mesh, portals, wall polygons, hull vertices | §5.3–§5.6 |
//! | landblock records | terrain and ceiling triangles, statics and generated objects that block a walker, bodies a walker passes through (ethereal to Player), cell meshes of the collision set, portals, the water plane and liquid boxes | §5.8, §6.1–§6.4, §9.2–§9.4, §12.1, `water.md` §6–§7 |
//!
//! Triangles become flat-shaded meshes prepared here (drawn translucent with their edges
//! by the render thread); volumes, portals and boxes become line lists. Every overlay is
//! bounded ([`MAX_OVERLAY_TRIANGLES`], [`MAX_OVERLAY_LINES`]); what is left out is counted
//! in the overlay's caption.

use std::sync::Arc;

use tessera_formats::CellGeometry;
use tessera_physics::geometry::{
    BodyPurpose, BodySource, Capsule, CollisionSet, QueryFilter, ShapeTemplate, Sphere,
};
use tessera_render::{Indices, MeshConfig, MeshData, PreparedMesh};
use tessera_world::{CellPortals, LandblockBundle};

/// Most triangles one overlay prepares.
pub const MAX_OVERLAY_TRIANGLES: usize = 600_000;
/// Most lines one overlay keeps.
pub const MAX_OVERLAY_LINES: usize = 32_768;
/// Segments of a sphere or capsule ring.
const RING_SEGMENTS: usize = 24;

/// What an overlay shows; decides its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlayKind {
    /// A collision mesh (0x66), the setup's or the record itself.
    Mesh,
    /// Boxes, spheres and capsules of a setup.
    Volumes,
    /// A cell's physics mesh.
    CellPhysics,
    /// A cell's portals.
    Portals,
    /// A cell's wall polygons.
    Walls,
    /// A cell's hull vertices.
    Hull,
    /// Terrain collision triangles.
    Terrain,
    /// Ceiling collision triangles.
    Ceiling,
    /// Static entities that block a walker (`collision.md` §9.3).
    Statics,
    /// Generated objects that block a walker (§9.4).
    Generated,
    /// Movement bodies a walker passes through: their effective `Physics_EtherealToType`
    /// contains Player (§9.2); other movers may still collide with them.
    PassThrough,
    /// Detail geometry (picking and camera only).
    Detail,
    /// The landblock's water plane.
    Water,
    /// Liquid boxes.
    Liquids,
}

impl OverlayKind {
    /// Display name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Mesh => "collision mesh",
            Self::Volumes => "collision volumes",
            Self::CellPhysics => "cell physics mesh",
            Self::Portals => "portals",
            Self::Walls => "wall polygons",
            Self::Hull => "hull vertices",
            Self::Terrain => "terrain collision",
            Self::Ceiling => "ceiling collision",
            Self::Statics => "statics",
            Self::Generated => "generated objects",
            Self::PassThrough => "ethereal to Player (a walker passes through)",
            Self::Detail => "detail (non-blocking)",
            Self::Water => "water plane",
            Self::Liquids => "liquid boxes",
        }
    }

    /// Linear RGBA colour (alpha is the fill opacity).
    #[must_use]
    pub fn color(self) -> [f32; 4] {
        match self {
            Self::Mesh => [1.0, 0.25, 0.2, 0.3],
            Self::Volumes => [1.0, 0.6, 0.1, 0.3],
            Self::CellPhysics => [0.2, 0.9, 0.9, 0.25],
            Self::Portals => [1.0, 1.0, 0.2, 1.0],
            Self::Walls => [0.6, 0.6, 1.0, 1.0],
            Self::Hull => [1.0, 0.4, 1.0, 1.0],
            Self::Terrain => [0.2, 0.9, 0.3, 0.15],
            Self::Ceiling => [0.7, 0.45, 0.25, 0.25],
            Self::Statics => [1.0, 0.25, 0.2, 0.3],
            Self::Generated => [1.0, 0.65, 0.1, 0.3],
            Self::PassThrough => [0.8, 0.5, 1.0, 0.2],
            Self::Detail => [0.6, 0.6, 0.6, 0.15],
            Self::Water => [0.15, 0.4, 1.0, 0.35],
            Self::Liquids => [0.3, 0.6, 1.0, 1.0],
        }
    }

    /// Whether the overlay is shown before the user changes anything.
    #[must_use]
    pub fn shown_by_default(self) -> bool {
        !matches!(
            self,
            Self::Detail | Self::Hull | Self::Walls | Self::PassThrough
        )
    }

    /// Whether the 3D view frames the scene on this overlay. Liquid boxes (the volume
    /// entity's deep box reaches far below the water, `water.md` §7.1) and ceiling
    /// triangles (which sit at height 0 over zero samples, `collision.md` §6.4) are left
    /// out, so a landblock is framed on its terrain and objects.
    #[must_use]
    pub fn frames(self) -> bool {
        !matches!(self, Self::Liquids | Self::Ceiling)
    }
}

/// One overlay of a scene.
#[derive(Debug, Clone)]
pub struct CollisionOverlay {
    /// What it shows.
    pub kind: OverlayKind,
    /// Caption for the overlay list.
    pub caption: String,
    /// Flat-shaded triangles, prepared for upload.
    pub prepared: Option<Arc<PreparedMesh>>,
    /// Line segments (scene frame).
    pub lines: Vec<([f32; 3], [f32; 3])>,
    /// Triangles drawn.
    pub triangles: usize,
    /// Bounds (min, max).
    pub aabb: Option<([f32; 3], [f32; 3])>,
}

impl CollisionOverlay {
    /// Approximate heap bytes.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        self.prepared.as_ref().map_or(0, |m| m.gpu_bytes() as usize)
            + self.lines.len() * 24
            + self.caption.len()
            + 96
    }
}

fn finite(p: [f32; 3]) -> bool {
    p.iter().all(|v| v.is_finite())
}

fn grow(aabb: &mut Option<([f32; 3], [f32; 3])>, p: [f32; 3]) {
    if !finite(p) {
        return;
    }
    let b = aabb.get_or_insert((p, p));
    for (i, v) in p.into_iter().enumerate() {
        b.0[i] = b.0[i].min(v);
        b.1[i] = b.1[i].max(v);
    }
}

/// An overlay of triangles (flat normals, two-sided when drawn). Triangles with a
/// non-finite corner are skipped; beyond [`MAX_OVERLAY_TRIANGLES`] the rest are counted.
#[must_use]
pub fn triangle_overlay(
    kind: OverlayKind,
    caption: impl Into<String>,
    tris: impl IntoIterator<Item = [[f32; 3]; 3]>,
) -> CollisionOverlay {
    let mut caption = caption.into();
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut aabb = None;
    let mut left_out = 0usize;
    for t in tris {
        if !t.iter().all(|p| finite(*p)) {
            continue;
        }
        if positions.len() / 3 >= MAX_OVERLAY_TRIANGLES {
            left_out += 1;
            continue;
        }
        let e1 = [t[1][0] - t[0][0], t[1][1] - t[0][1], t[1][2] - t[0][2]];
        let e2 = [t[2][0] - t[0][0], t[2][1] - t[0][1], t[2][2] - t[0][2]];
        let n = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let n = if len > 1e-12 && len.is_finite() {
            [n[0] / len, n[1] / len, n[2] / len]
        } else {
            [0.0, 0.0, 1.0]
        };
        for p in t {
            positions.push(p);
            normals.push(n);
            grow(&mut aabb, p);
        }
    }
    let triangles = positions.len() / 3;
    if left_out > 0 {
        caption.push_str(&format!(" ({left_out} triangles left out)"));
    }
    let mut prepared = None;
    if triangles > 0 {
        let count = u32::try_from(positions.len()).unwrap_or(u32::MAX);
        let data = MeshData {
            label: Some(caption.clone()),
            positions,
            normals: Some(normals),
            tangents: None,
            uvs: None,
            colors: None,
            indices: Indices::U32((0..count).collect()),
            submeshes: Vec::new(),
        };
        match data.prepare(&MeshConfig::default()) {
            Ok(p) => prepared = Some(Arc::new(p)),
            // The renderer refused the streams: the caption says so.
            Err(e) => caption.push_str(&format!(" (refused: {e})")),
        }
    }
    CollisionOverlay {
        kind,
        caption,
        prepared,
        lines: Vec::new(),
        triangles,
        aabb,
    }
}

/// An overlay of line segments; non-finite segments are skipped, at most
/// [`MAX_OVERLAY_LINES`] kept.
#[must_use]
pub fn line_overlay(
    kind: OverlayKind,
    caption: impl Into<String>,
    lines: impl IntoIterator<Item = ([f32; 3], [f32; 3])>,
) -> CollisionOverlay {
    let mut caption = caption.into();
    let mut kept = Vec::new();
    let mut aabb = None;
    let mut left_out = 0usize;
    for (a, b) in lines {
        if !finite(a) || !finite(b) {
            continue;
        }
        if kept.len() >= MAX_OVERLAY_LINES {
            left_out += 1;
            continue;
        }
        grow(&mut aabb, a);
        grow(&mut aabb, b);
        kept.push((a, b));
    }
    if left_out > 0 {
        caption.push_str(&format!(" ({left_out} lines left out)"));
    }
    CollisionOverlay {
        kind,
        caption,
        prepared: None,
        lines: kept,
        triangles: 0,
        aabb,
    }
}

/// The closed outline of a polygon as segments.
fn polygon(corners: &[[f32; 3]]) -> impl Iterator<Item = ([f32; 3], [f32; 3])> + '_ {
    let n = corners.len();
    (0..if n >= 2 { n } else { 0 }).map(move |i| (corners[i], corners[(i + 1) % n]))
}

/// A ring of radius `r` around `c` in the plane spanned by axes `u` and `v`.
fn ring(c: [f32; 3], r: f32, u: usize, v: usize) -> Vec<([f32; 3], [f32; 3])> {
    let at = |k: usize| {
        let a = k as f32 / RING_SEGMENTS as f32 * std::f32::consts::TAU;
        let mut p = c;
        p[u] += r * a.cos();
        p[v] += r * a.sin();
        p
    };
    (0..RING_SEGMENTS).map(|k| (at(k), at(k + 1))).collect()
}

fn sphere_lines(s: &Sphere) -> Vec<([f32; 3], [f32; 3])> {
    let mut out = ring(s.centre, s.radius, 0, 1);
    out.extend(ring(s.centre, s.radius, 0, 2));
    out.extend(ring(s.centre, s.radius, 1, 2));
    out
}

fn capsule_lines(c: &Capsule) -> Vec<([f32; 3], [f32; 3])> {
    let mut out = ring(c.a, c.radius, 0, 1);
    out.extend(ring(c.b, c.radius, 0, 1));
    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        let off = |p: [f32; 3]| [p[0] + dx * c.radius, p[1] + dy * c.radius, p[2]];
        out.push((off(c.a), off(c.b)));
    }
    let mut bottom = c.a;
    bottom[2] -= c.radius;
    let mut top = c.b;
    top[2] += c.radius;
    out.push((bottom, c.a));
    out.push((c.b, top));
    out
}

/// The overlays of a setup's (or a 0x66 record's) collision shape: the mesh triangles,
/// then the boxes, spheres and capsules.
#[must_use]
pub fn template_overlays(t: &ShapeTemplate, caption: &str) -> Vec<CollisionOverlay> {
    let mut out = Vec::new();
    let mesh = t.mesh_triangles.min(t.triangles.len());
    if mesh > 0 {
        out.push(triangle_overlay(
            OverlayKind::Mesh,
            format!("{caption}: {mesh} triangles"),
            t.triangles[..mesh].iter().copied(),
        ));
    }
    let boxes = &t.triangles[mesh..];
    if !boxes.is_empty() || !t.spheres.is_empty() || !t.capsules.is_empty() {
        let mut o = triangle_overlay(
            OverlayKind::Volumes,
            format!(
                "volumes: {} boxes, {} spheres, {} capsules{}",
                t.boxes.len(),
                t.spheres.len(),
                t.capsules.len(),
                if t.dropped > 0 {
                    format!(", {} dropped", t.dropped)
                } else {
                    String::new()
                }
            ),
            boxes.iter().copied(),
        );
        let rounds = line_overlay(
            OverlayKind::Volumes,
            "",
            t.spheres
                .iter()
                .flat_map(sphere_lines)
                .chain(t.capsules.iter().flat_map(capsule_lines)),
        );
        o.lines = rounds.lines;
        // The rings' caption is only their " (N lines left out)" suffix, if any.
        o.caption.push_str(&rounds.caption);
        if let Some((lo, hi)) = rounds.aabb {
            grow(&mut o.aabb, lo);
            grow(&mut o.aabb, hi);
        }
        out.push(o);
    }
    out
}

/// The overlays of a cell geometry record in its cell frame (`collision.md` §5.3–§5.6).
#[must_use]
pub fn cell_overlays(c: &CellGeometry) -> Vec<CollisionOverlay> {
    let mut out = Vec::new();
    if let Some(m) = &c.physics_mesh {
        match ShapeTemplate::from_collision_mesh(m) {
            Ok(t) => out.push(triangle_overlay(
                OverlayKind::CellPhysics,
                format!("physics mesh: {} triangles", t.triangles.len()),
                t.triangles,
            )),
            Err(e) => out.push(line_overlay(
                OverlayKind::CellPhysics,
                format!("physics mesh refused: {e}"),
                std::iter::empty(),
            )),
        }
    }
    out.push(line_overlay(
        OverlayKind::Portals,
        format!("{} portals", c.portals.len()),
        c.portals.iter().flat_map(|p| polygon(&p.corners)),
    ));
    out.push(line_overlay(
        OverlayKind::Walls,
        format!("{} wall polygons", c.walls.len()),
        c.walls.iter().flat_map(|w| polygon(&w.corners)),
    ));
    // Hull vertices as small crosses.
    let cross = |p: [f32; 3]| {
        let d = 0.15;
        [0, 1, 2].map(|a| {
            let (mut lo, mut hi) = (p, p);
            lo[a] -= d;
            hi[a] += d;
            (lo, hi)
        })
    };
    out.push(line_overlay(
        OverlayKind::Hull,
        format!("{} hull vertices", c.hull.len()),
        c.hull.iter().flat_map(|p| cross(*p)),
    ));
    out.retain(|o| o.triangles > 0 || !o.lines.is_empty() || o.caption.contains("refused"));
    out
}

/// The 12 edges of the box with corners `c` (bit 0 x, bit 1 y, bit 2 z).
fn box_edges(c: [[f32; 3]; 8]) -> [([f32; 3], [f32; 3]); 12] {
    let pairs = [
        (0, 1),
        (2, 3),
        (4, 5),
        (6, 7),
        (0, 2),
        (1, 3),
        (4, 6),
        (5, 7),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    pairs.map(|(a, b)| (c[a], c[b]))
}

/// The overlays of a collision set (landblock frame): one triangle overlay per group of
/// bodies (terrain, ceiling, statics and generated objects that block a walker, bodies a
/// walker passes through, cell meshes, detail), a note on its spheres and capsules, and
/// the portals of `cells`. Each group keeps at most [`MAX_OVERLAY_TRIANGLES`] triangles;
/// its caption counts every triangle of the group and how many were left out.
#[must_use]
pub fn collision_set_overlays(set: &CollisionSet, cells: &[CellPortals]) -> Vec<CollisionOverlay> {
    let mut out = Vec::new();
    let walker = QueryFilter::player();
    // (kind, triangles kept, bodies, triangles of the group)
    type Group = (OverlayKind, Vec<[[f32; 3]; 3]>, usize, usize);
    let mut groups: Vec<Group> = [
        OverlayKind::Terrain,
        OverlayKind::Ceiling,
        OverlayKind::Statics,
        OverlayKind::Generated,
        OverlayKind::PassThrough,
        OverlayKind::CellPhysics,
        OverlayKind::Detail,
    ]
    .into_iter()
    .map(|k| (k, Vec::new(), 0, 0))
    .collect();
    for body in set.bodies() {
        let kind = if body.purpose == BodyPurpose::Detail {
            OverlayKind::Detail
        } else if !walker.accepts(body) {
            OverlayKind::PassThrough
        } else {
            match body.source {
                BodySource::Terrain => OverlayKind::Terrain,
                BodySource::Ceiling => OverlayKind::Ceiling,
                BodySource::Static { .. } => OverlayKind::Statics,
                BodySource::Generated { .. } => OverlayKind::Generated,
                BodySource::Cell { .. } => OverlayKind::CellPhysics,
                BodySource::Other { .. } => OverlayKind::Detail,
            }
        };
        let Some(g) = groups.iter_mut().find(|g| g.0 == kind) else {
            continue;
        };
        g.2 += 1;
        let first = body.first_triangle as usize;
        let count = body.triangle_count as usize;
        g.3 = g.3.saturating_add(count);
        let room = MAX_OVERLAY_TRIANGLES.saturating_sub(g.1.len());
        for k in first..first.saturating_add(count.min(room)) {
            if let Some((t, _, _)) = set.triangle(k) {
                g.1.push(t);
            }
        }
    }
    for (kind, tris, bodies, total) in groups {
        if tris.is_empty() {
            continue;
        }
        let left_out = total.saturating_sub(tris.len());
        let caption = if left_out > 0 {
            format!(
                "{}: {bodies} bodies, {total} triangles ({left_out} triangles left out)",
                kind.name()
            )
        } else {
            format!("{}: {bodies} bodies, {total} triangles", kind.name())
        };
        out.push(triangle_overlay(kind, caption, tris));
    }
    if set.round_count() > 0 {
        out.push(line_overlay(
            OverlayKind::Volumes,
            format!(
                "{} spheres and capsules (not drawn; see the objects' 3D views)",
                set.round_count()
            ),
            std::iter::empty(),
        ));
    }
    let portals: usize = cells.iter().map(|cell| cell.portals.len()).sum();
    out.push(line_overlay(
        OverlayKind::Portals,
        format!("portals of {} cells: {portals}", cells.len()),
        cells
            .iter()
            .flat_map(|cell| cell.portals.iter().flat_map(|p| polygon(&p.corners))),
    ));
    out.retain(|o| o.triangles > 0 || !o.lines.is_empty() || o.kind == OverlayKind::Volumes);
    out
}

/// The overlays of a landblock's collision set ([`collision_set_overlays`]) and water, in
/// the landblock frame.
#[must_use]
pub fn landblock_overlays(b: &LandblockBundle) -> Vec<CollisionOverlay> {
    let mut out = Vec::new();
    if let Some(c) = &b.collision {
        out.extend(collision_set_overlays(&c.set, &c.cells));
    }
    if let Some(w) = &b.water {
        if let Some(lw) = w.water() {
            let h = lw.height;
            let q = |x: f32, y: f32| [x, y, h];
            out.push(triangle_overlay(
                OverlayKind::Water,
                format!("water plane at {h:.2} m (type {})", lw.water_type),
                [
                    [q(0.0, 0.0), q(160.0, 0.0), q(160.0, 160.0)],
                    [q(0.0, 0.0), q(160.0, 160.0), q(0.0, 160.0)],
                ],
            ));
        }
        if !w.boxes.is_empty() {
            out.push(line_overlay(
                OverlayKind::Liquids,
                format!("{} liquid boxes", w.boxes.len()),
                w.boxes.iter().flat_map(|lb| {
                    let corners: [[f32; 3]; 8] = std::array::from_fn(|k| {
                        let local = [
                            if k & 1 == 0 { lb.min[0] } else { lb.max[0] },
                            if k & 2 == 0 { lb.min[1] } else { lb.max[1] },
                            if k & 4 == 0 { lb.min[2] } else { lb.max[2] },
                        ];
                        lb.transform.apply(local)
                    });
                    box_edges(corners)
                }),
            ));
        }
    }
    out.retain(|o| o.triangles > 0 || !o.lines.is_empty() || o.kind == OverlayKind::Volumes);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangles_are_prepared_and_bounded() {
        let t = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 2.0]];
        let bad = [[f32::NAN, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let o = triangle_overlay(OverlayKind::Mesh, "m", [t, bad, t]);
        assert_eq!(o.triangles, 2);
        assert!(o.prepared.is_some());
        assert_eq!(o.aabb, Some(([0.0; 3], [1.0, 1.0, 2.0])));
        let empty = triangle_overlay(OverlayKind::Mesh, "e", []);
        assert!(empty.prepared.is_none());
        assert!(empty.aabb.is_none());
    }

    #[test]
    fn lines_skip_non_finite_and_are_capped() {
        let ok = ([0.0; 3], [1.0; 3]);
        let bad = ([f32::INFINITY, 0.0, 0.0], [1.0; 3]);
        let o = line_overlay(
            OverlayKind::Portals,
            "p",
            std::iter::repeat_n(ok, MAX_OVERLAY_LINES + 5).chain([bad]),
        );
        assert_eq!(o.lines.len(), MAX_OVERLAY_LINES);
        assert!(o.caption.contains("5 lines left out"));
    }

    #[test]
    fn templates_split_mesh_and_volumes() {
        let mut t = ShapeTemplate::from_triangle_list(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[[0, 1, 2]],
        )
        .unwrap();
        t.spheres.push(Sphere {
            centre: [0.0, 0.0, 1.0],
            radius: 0.5,
        });
        t.capsules.push(Capsule::vertical([0.0; 3], 2.0, 0.3));
        let o = template_overlays(&t, "mesh");
        assert_eq!(o.len(), 2);
        assert_eq!(o[0].kind, OverlayKind::Mesh);
        assert_eq!(o[0].triangles, 1);
        assert_eq!(o[1].kind, OverlayKind::Volumes);
        assert_eq!(o[1].lines.len(), 3 * RING_SEGMENTS + 2 * RING_SEGMENTS + 6);
        assert!(template_overlays(&ShapeTemplate::default(), "x").is_empty());
    }

    #[test]
    fn volume_captions_count_the_rings_left_out() {
        // Regression: the sphere and capsule rings were capped but the count of lines left
        // out was dropped with the rings' own caption.
        let spheres = 1_400;
        let t = ShapeTemplate {
            spheres: vec![
                Sphere {
                    centre: [0.0; 3],
                    radius: 1.0,
                };
                spheres
            ],
            ..ShapeTemplate::default()
        };
        let o = template_overlays(&t, "setup");
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].lines.len(), MAX_OVERLAY_LINES);
        assert_eq!(
            o[0].caption,
            format!(
                "volumes: 0 boxes, {spheres} spheres, 0 capsules ({} lines left out)",
                spheres * 3 * RING_SEGMENTS - MAX_OVERLAY_LINES
            )
        );
    }

    #[test]
    fn polygons_close() {
        let c = [[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]];
        let segs: Vec<_> = polygon(&c).collect();
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[2], (c[2], c[0]));
        assert_eq!(polygon(&c[..1]).count(), 0);
    }

    #[test]
    fn oversized_groups_count_what_is_left_out() {
        // Regression: a group stopped at MAX_OVERLAY_TRIANGLES + 1 triangles and its caption
        // said "(1 triangles left out)" however many were dropped.
        use tessera_formats::Did;
        use tessera_physics::geometry::{CollisionFlags, CollisionLimits, CollisionSetBuilder};
        let per_body = 350_000;
        let t = ShapeTemplate {
            mesh_triangles: per_body,
            triangles: vec![[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]; per_body],
            ..ShapeTemplate::default()
        };
        let mut b = CollisionSetBuilder::new(CollisionLimits::default());
        for index in 0..2 {
            b.add_shape(
                BodySource::Static {
                    index,
                    object: Did(0x4700_0001),
                },
                CollisionFlags::default(),
                BodyPurpose::Movement,
                &t,
            )
            .expect("body");
        }
        let o = collision_set_overlays(&b.build(), &[]);
        let statics = o
            .iter()
            .find(|o| o.kind == OverlayKind::Statics)
            .expect("statics");
        assert_eq!(statics.triangles, MAX_OVERLAY_TRIANGLES);
        assert_eq!(
            statics.caption,
            format!(
                "statics: 2 bodies, {} triangles ({} triangles left out)",
                2 * per_body,
                2 * per_body - MAX_OVERLAY_TRIANGLES
            )
        );
    }

    #[test]
    fn liquids_and_ceilings_do_not_frame() {
        assert!(!OverlayKind::Liquids.frames());
        assert!(!OverlayKind::Ceiling.frames());
        assert!(OverlayKind::Terrain.frames() && OverlayKind::Mesh.frames());
    }
}
