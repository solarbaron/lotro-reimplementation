// SPDX-License-Identifier: GPL-3.0-or-later
//! Decal previews of the 3D view (`decals.md` §9.3, Tessera Decision).
//!
//! For a landblock-info record (0x8RR2XXYY) the mesh view offers one choice per Decal SED
//! after the landblock itself. A decal choice shows:
//!
//! - the box of §4.2 as a wireframe and the local axes X′ (red), Y′ (green, the projection
//!   direction) and Z′ (blue) from the SED's position, as overlays;
//! - a preview mesh built by §4.3–§4.5 (`tessera_world::decals::build_mesh`) from a
//!   synthetic receiver: a flat square facing −Y′ through the box centre (normal −Y′),
//!   larger than the box, drawn with the decal's surface record (`DecalVolume_Material`,
//!   §6.1) and the vertex colour of §6.2 for the light *L* = (1, 1, 1, 1) and opacity 1.
//!
//! The scene is centred on the box centre (the landblock position is in the notes). The
//! parameters come from the SED's per-instance property set and its marker object's chain
//! (§3.2); the default material from the `UNIQUEDB` map. Nothing is hard-coded.

use std::sync::Arc;

use tessera_formats::landblock::{InstanceId, StaticEntityType};
use tessera_formats::{Did, EntityDescriptor, LandblockInfo, PropertySet};
use tessera_render::{Indices, MeshConfig, MeshData};
use tessera_world::decals::shading::decal_colour;
use tessera_world::decals::{
    DEFAULT_MATERIAL_UNIQUEDB_KEY, DecalBox, DecalParams, DecalPids, ReceiverKind,
    ReceiverTriangle, ReceiverVertex, build_mesh, marker_chain,
};

use crate::collision_overlay::{CollisionOverlay, OverlayKind};
use crate::data::DataContext;
use crate::error::ExplorerError;
use crate::geometry::srgb_unit_to_linear;
use crate::scene::{Collector, IDENTITY, LevelSummary, ScenePart, SceneSubmesh};

/// Most decals a landblock offers as choices (the choice index is one byte, and choice 0
/// is the landblock itself).
pub const MAX_DECAL_CHOICES: usize = 254;
/// Most Decal SEDs read from one landblock-info record.
pub const MAX_DECALS: usize = 8_192;

/// One Decal SED of a landblock with its effective parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct DecalSource {
    /// The SED's instance ID.
    pub iid: Option<InstanceId>,
    /// The marker object (0x47).
    pub object: Option<Did>,
    /// The effective parameters (§3.2).
    pub params: DecalParams,
    /// The projection volume (§4.2), when the SED has a frame.
    pub volume: Option<DecalBox>,
}

impl DecalSource {
    /// The caption of the mesh view's choice list.
    #[must_use]
    pub fn caption(&self, index: usize) -> String {
        let iid = self
            .iid
            .map_or_else(|| "no IID".to_owned(), |i| format!("{:#018x}", i.0));
        format!("decal {} · {iid} · {}", index + 1, self.params.material)
    }
}

/// The Decal SEDs of landblock-info record `did`, in record order (at most
/// [`MAX_DECALS`]), with their parameters. The mesh view offers the first
/// [`MAX_DECAL_CHOICES`] as choices.
///
/// # Errors
///
/// When the record cannot be read or decoded.
pub fn landblock_decals(ctx: &DataContext, did: u32) -> Result<Vec<DecalSource>, ExplorerError> {
    let raw = crate::decoders::read_raw(ctx, did, None)?;
    let registry = ctx.game.registry()?;
    let dict = registry.dictionary();
    let info = LandblockInfo::parse_expecting(&raw.bytes, Did(did), Some(raw.entry.stamp), dict)?;
    let pids = DecalPids::resolve(dict);
    let default_material = ctx
        .game
        .did_map_by_label(tessera_formats::gameplay::common::UNIQUEDB_LABEL)
        .ok()
        .flatten()
        .and_then(|m| m.get(DEFAULT_MATERIAL_UNIQUEDB_KEY).map(|e| e.value))
        .unwrap_or(Did(0));
    let weenies: std::collections::HashMap<InstanceId, &PropertySet> = info
        .weenies
        .iter()
        .map(|w| (w.iid, &w.properties))
        .collect();
    let mut out = Vec::new();
    for e in info
        .entities
        .iter()
        .filter(|e| e.kind() == Some(StaticEntityType::Decal))
        .take(MAX_DECALS)
    {
        let chain = e
            .object
            .map(|o| {
                marker_chain(o, |d| {
                    let bytes = ctx.game.read(d).ok().flatten()?;
                    let ed = EntityDescriptor::parse(&bytes, dict).ok()?;
                    Some((ed.properties, Some(ed.header.template)))
                })
            })
            .unwrap_or_default();
        let refs: Vec<&PropertySet> = chain.iter().collect();
        let instance = e
            .iid
            .and_then(|i| weenies.get(&i).copied())
            .or(e.properties.as_ref());
        let params = DecalParams::collect(&pids, default_material, &refs, instance);
        let volume = e
            .placement
            .map(|pl| DecalBox::new(pl.position, pl.rotation, params.ortho_scale));
        out.push(DecalSource {
            iid: e.iid,
            object: e.object,
            params,
            volume,
        });
    }
    Ok(out)
}

/// The synthetic receiver of §9.3: a square through the box centre facing −Y′ (normal
/// −Y′), twice the terrain half-width of the box on each side, as two triangles wound so
/// that their geometric normal is −Y′.
#[must_use]
pub fn preview_receiver(b: &DecalBox) -> Vec<ReceiverTriangle> {
    let [x, y, z] = b.axes;
    let r = b.terrain_half_width() * 2.0;
    let n = [-y[0], -y[1], -y[2]];
    let at = |sx: f32, sz: f32| ReceiverVertex {
        position: [
            b.center[0] + x[0] * sx + z[0] * sz,
            b.center[1] + x[1] * sx + z[1] * sz,
            b.center[2] + x[2] * sx + z[2] * sz,
        ],
        normal: n,
    };
    let (a, bb, c, d) = (at(-r, -r), at(r, -r), at(r, r), at(-r, r));
    vec![
        ReceiverTriangle {
            kind: ReceiverKind::Object,
            vertices: [a, bb, c],
        },
        ReceiverTriangle {
            kind: ReceiverKind::Object,
            vertices: [a, c, d],
        },
    ]
}

/// The vertex colour of the preview (§6.2) for *L* = (1, 1, 1, 1) and opacity 1, as
/// linear RGBA for the renderer (the packed bytes are display values).
#[must_use]
pub fn preview_colour(params: &DecalParams) -> [f32; 4] {
    let [r, g, b, a] = decal_colour(params.colour, [1.0; 4], 1.0, params.lighting_blend);
    [
        srgb_unit_to_linear(f32::from(r) / 255.0),
        srgb_unit_to_linear(f32::from(g) / 255.0),
        srgb_unit_to_linear(f32::from(b) / 255.0),
        f32::from(a) / 255.0,
    ]
}

/// The twelve edges of the box, relative to `origin`.
#[must_use]
pub fn box_edges(b: &DecalBox, origin: [f32; 3]) -> Vec<([f32; 3], [f32; 3])> {
    let c = b.corners().map(|p| sub(p, origin));
    let mut out = Vec::with_capacity(12);
    for i in 0..8usize {
        for bit in [1usize, 2, 4] {
            if i & bit == 0 {
                out.push((c[i], c[i | bit]));
            }
        }
    }
    out
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn overlay(
    kind: OverlayKind,
    caption: String,
    lines: Vec<([f32; 3], [f32; 3])>,
) -> CollisionOverlay {
    let mut aabb: Option<([f32; 3], [f32; 3])> = None;
    for (a, b) in &lines {
        for p in [a, b] {
            let e = aabb.get_or_insert((*p, *p));
            for (i, v) in p.iter().enumerate() {
                e.0[i] = e.0[i].min(*v);
                e.1[i] = e.1[i].max(*v);
            }
        }
    }
    CollisionOverlay {
        kind,
        caption,
        prepared: None,
        lines,
        triangles: 0,
        aabb,
    }
}

/// The preview of `d`: the mesh part (drawn with the decal's surface record through
/// `mats`), the box and axis overlays, and notes. `None` part when the SED has no frame.
pub(crate) fn preview(
    d: &DecalSource,
    mats: &mut Collector<'_>,
) -> (Option<ScenePart>, Vec<CollisionOverlay>, Vec<String>) {
    let mut notes = Vec::new();
    let Some(b) = d.volume else {
        notes.push("this Decal SED has no frame: nothing to preview".into());
        return (None, Vec::new(), notes);
    };
    let origin = b.center;
    notes.push(format!(
        "decal preview (decals.md §9.3): the scene is centred on the box centre {:.3?} \
         (landblock frame); position {:.3?}, extents {:.3?}, material {}, lighting blend {}, \
         colour (B, G, R, A) {:?}",
        b.center,
        b.position,
        b.extents,
        d.params.material,
        d.params.lighting_blend,
        d.params.colour
    ));
    // The overlays: the box, then the three local axes from the SED's position.
    let mut overlays = vec![overlay(
        OverlayKind::DecalBox,
        "decal box (§4.2)".into(),
        box_edges(&b, origin),
    )];
    let p = sub(b.position, origin);
    for (k, (kind, name)) in [
        (OverlayKind::DecalAxisX, "X′"),
        (OverlayKind::DecalAxisY, "Y′ (projection)"),
        (OverlayKind::DecalAxisZ, "Z′"),
    ]
    .into_iter()
    .enumerate()
    {
        let len = b.extents[k].max(0.1);
        let a = b.axes[k];
        let end = [p[0] + a[0] * len, p[1] + a[1] * len, p[2] + a[2] * len];
        overlays.push(overlay(kind, format!("local axis {name}"), vec![(p, end)]));
    }
    // The preview mesh.
    let mesh = build_mesh(&d.params, &b, preview_receiver(&b), None);
    if mesh.vertices.is_empty() {
        notes.push(format!(
            "the preview receiver left no triangles ({:?})",
            mesh.report
        ));
        return (None, overlays, notes);
    }
    let colour = preview_colour(&d.params);
    let n = mesh.vertices.len();
    let mut data = MeshData {
        label: Some("decal preview".into()),
        positions: mesh
            .vertices
            .iter()
            .map(|v| sub(v.position, origin))
            .collect(),
        normals: Some(mesh.vertices.iter().map(|v| v.normal).collect()),
        tangents: None,
        binormals: None,
        uvs: Some(mesh.vertices.iter().map(|v| v.uv).collect()),
        colors: Some(vec![colour; n]),
        indices: Indices::U32((0..u32::try_from(n).unwrap_or(0)).collect()),
        submeshes: Vec::new(),
    };
    data.compute_tangents();
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for p in &data.positions {
        for (i, v) in p.iter().enumerate() {
            lo[i] = lo[i].min(*v);
            hi[i] = hi[i].max(*v);
        }
    }
    let center = [
        (lo[0] + hi[0]) / 2.0,
        (lo[1] + hi[1]) / 2.0,
        (lo[2] + hi[2]) / 2.0,
    ];
    let radius = (0..3)
        .map(|i| (hi[i] - lo[i]) / 2.0)
        .fold(0.0f32, |a, v| a + v * v)
        .sqrt();
    let triangles = n / 3;
    let material = mats.surface_material(d.params.material, None);
    let mut part = ScenePart {
        mesh: d.params.material.0,
        caption: format!("decal preview · {}", d.params.material),
        level: 0,
        levels: vec![LevelSummary {
            kind: "decal preview".into(),
            draws: 1,
            triangles,
        }],
        flags: 0,
        transform: IDENTITY,
        prepared: None,
        error: None,
        submeshes: Vec::new(),
        issues: Vec::new(),
        vertices: n,
        triangles,
        sphere: Some((center, radius)),
        skinned: false,
        attach: None,
        figure: 0,
    };
    match data.prepare(&MeshConfig::default()) {
        Ok(p) => part.prepared = Some(Arc::new(p)),
        Err(e) => part.error = Some(e.to_string()),
    }
    part.submeshes.push(SceneSubmesh {
        draw: 0,
        submesh: 0,
        surface: Some(d.params.material.0),
        format: 0,
        vertices: u32::try_from(n).unwrap_or(u32::MAX),
        triangles: u32::try_from(triangles).unwrap_or(u32::MAX),
        skinned: false,
        aabb: Some((lo, hi)),
        material: Some(material),
    });
    (Some(part), overlays, notes)
}

#[cfg(test)]
mod tests {
    use tessera_formats::setup::Quaternion;

    use super::*;

    #[test]
    fn receiver_faces_minus_y_and_survives_the_builder() {
        let params = DecalParams::initial(Did(0x3100_0008));
        let b = DecalBox::new([10.0, 20.0, 5.0], Quaternion::IDENTITY, [2.0, 1.0, 3.0]);
        let tris = preview_receiver(&b);
        assert_eq!(tris.len(), 2);
        for t in &tris {
            let [a, bb, c] = t.vertices.map(|v| v.position);
            let e1 = sub(bb, a);
            let e2 = sub(c, a);
            let n = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            assert!(n[1] < 0.0, "geometric normal −Y′: {n:?}");
        }
        let mesh = build_mesh(&params, &b, tris, None);
        assert!(mesh.triangle_count() > 0);
        // Clipped to the box: every vertex within the extents around the centre.
        for v in &mesh.vertices {
            let l = b.local(v.position);
            assert!(
                l[0].abs() <= 1.0 + 1e-3 && l[2].abs() <= 1.5 + 1e-3,
                "{l:?}"
            );
            assert!(v.uv.iter().all(|c| (-1e-4..=1.0 + 1e-4).contains(c)));
        }
        assert_eq!(box_edges(&b, b.center).len(), 12);
        // White, opaque, unlit: white.
        assert_eq!(preview_colour(&params), [1.0; 4]);
    }
}
