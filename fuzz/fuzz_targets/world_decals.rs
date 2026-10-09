// SPDX-License-Identifier: GPL-3.0-or-later
//! Placed decal volumes (`tessera_world::decals`, `docs/specs/decals.md`) on arbitrary
//! bytes.
//!
//! Byte 0 is a mode byte. The next bytes are read as the decal's frame (position, rotation,
//! `OrthoScale`), the switches of the back-face test and the stick-to-water rule, and then
//! up to 64 receiving triangles (position and normal per vertex, a kind bit); what follows
//! is an embedded property set (total synthetic schema, type = PID mod 24) read as the
//! SED's own set on top of the initial parameters, and as a depth-bias definition set.
//!
//! Nothing may panic. With a finite frame, every output vertex before the lift lies in the
//! box (within a tolerance relative to the box size) with texture coordinates in [0, 1], and
//! one source triangle yields at most seven output triangles (`decals.md` §10 invariants 6
//! and 7); colours, lights and opacities are finite.
//!
//! Run with a memory cap, e.g.
//! `cargo fuzz run world_decals -- -malloc_limit_mb=512 -rss_limit_mb=1024`.

#![no_main]

use std::collections::HashMap;

use libfuzzer_sys::fuzz_target;
use tessera_formats::setup::Quaternion;
use tessera_formats::{Did, PropertySet, PropertyType};
use tessera_world::decals::shading::{
    DecalLight, DecalLightKind, DepthBiasPids, DepthBiasTable, decal_colour, decal_light,
    depth_opacity, draw_range, height_alpha,
};
use tessera_world::decals::{
    DecalBox, DecalParams, DecalPids, LIFT, ReceiverKind, ReceiverTriangle, ReceiverVertex,
    build_mesh,
};

fn total(pid: u32) -> Option<PropertyType> {
    Some(PropertyType::from_code(pid % 24))
}

struct Floats<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Floats<'_> {
    fn next(&mut self) -> f32 {
        let mut b = [0u8; 4];
        for x in &mut b {
            *x = self.bytes.get(self.at).copied().unwrap_or(0);
            self.at += 1;
        }
        f32::from_le_bytes(b)
    }
    fn small(&mut self) -> f32 {
        let v = self.next();
        if v.is_finite() {
            v.clamp(-1.0e4, 1.0e4)
        } else {
            0.0
        }
    }
    fn v3(&mut self) -> [f32; 3] {
        [self.small(), self.small(), self.small()]
    }
}

fn type_of(label: &str) -> u32 {
    match label {
        "DecalVolume_Material" => 20,
        "DecalVolume_ColorAndOpacity" => 16,
        "DecalVolume_OrthoScale" => 6,
        l if l.contains("Use") || l.contains("Stick") || l.starts_with("Render_") => 21,
        l if l.contains("Angle") || l.contains("Range") || l.contains("Min") => 9,
        _ => 8,
    }
}

fuzz_target!(|input: &[u8]| {
    let [mode, rest @ ..] = input else {
        return;
    };
    let mut f = Floats { bytes: rest, at: 0 };
    let position = f.v3();
    let rotation = Quaternion {
        w: f.small(),
        x: f.small(),
        y: f.small(),
        z: f.small(),
    };
    let scale = f.v3();
    let water = f.small();
    let map: HashMap<String, u32> = DecalPids::LABELS
        .iter()
        .enumerate()
        .map(|(i, l)| {
            (
                (*l).to_owned(),
                (u32::try_from(i).unwrap_or(0) + 1) * 24 + type_of(l),
            )
        })
        .collect();
    let pids = DecalPids::resolve(&map);
    let mut params = DecalParams::initial(Did(0x3100_0008));
    params.ortho_scale = scale;
    params.use_backface_culling = mode & 1 != 0;
    params.use_precise_backface_culling = mode & 2 != 0;
    params.use_landscape_backface_culling = mode & 4 != 0;
    params.stick_to_water = mode & 8 != 0;
    params.lighting_blend = u32::from(mode >> 4);
    let count = usize::from(rest.get(f.at).copied().unwrap_or(0) % 65);
    f.at += 1;
    let triangles: Vec<ReceiverTriangle> = (0..count)
        .map(|k| {
            let mut v = || ReceiverVertex {
                position: [
                    position[0] + f.small() / 100.0,
                    position[1] + f.small() / 100.0,
                    position[2] + f.small() / 100.0,
                ],
                normal: f.v3(),
            };
            ReceiverTriangle {
                kind: if k % 2 == 0 {
                    ReceiverKind::Object
                } else {
                    ReceiverKind::Terrain
                },
                vertices: [v(), v(), v()],
            }
        })
        .collect();
    let tail = rest.get(f.at..).unwrap_or(&[]);
    if let Ok(set) = PropertySet::parse(tail, &total) {
        let mut p = params;
        let _ = p.assign(&pids, &set);
        let chain: [&PropertySet; 1] = [&set];
        let _ = DecalParams::collect(&pids, Did(1), &chain, Some(&set));
        let bias_map: HashMap<String, u32> = DepthBiasPids::LABELS
            .iter()
            .enumerate()
            .map(|(i, l)| {
                (
                    (*l).to_owned(),
                    (u32::try_from(i).unwrap_or(0) + 40) * 24 + [12, 11, 8, 9, 9, 9][i % 6],
                )
            })
            .collect();
        let (t, _) = DepthBiasTable::build(
            &DepthBiasPids::resolve(&bias_map),
            &[&set, &set],
            mode & 1 != 0,
        );
        let _ = t.get(u32::from(*mode));
    }
    let b = DecalBox::new(position, rotation, params.ortho_scale);
    let mesh = build_mesh(&params, &b, triangles.iter().copied(), Some(water));
    assert!(mesh.triangle_count() <= 7 * count);
    let size = b.extents.iter().fold(1.0f32, |a, e| a.max(*e))
        + position.iter().fold(0.0f32, |a, p| a.max(p.abs()));
    for v in &mesh.vertices {
        if !v.normal.iter().all(|c| c.is_finite() && c.abs() < 1.0e3) {
            continue;
        }
        let before = [
            v.position[0] - v.normal[0] * LIFT,
            v.position[1] - v.normal[1] * LIFT,
            v.position[2] - v.normal[2] * LIFT,
        ];
        let tol = 1.0e-3 * size;
        assert!(b.contains(before, tol), "{before:?} outside {b:?}");
        let ut = tol / b.extents[0].min(b.extents[2]);
        assert!(v.uv.iter().all(|c| *c >= -ut && *c <= 1.0 + ut));
    }
    let light = decal_light(
        [f.small(), f.small(), f.small()],
        &[DecalLight {
            kind: DecalLightKind::Point {
                position: f.v3(),
                range: f.small(),
                falloff: f.small(),
            },
            colour: f.v3(),
            intensity: f.small(),
        }],
        mesh.mean_normal.unwrap_or([0.0, 0.0, 1.0]),
        mesh.center.unwrap_or(position),
        [f.small(), f.small(), f.small(), f.small()],
    );
    assert!(light.iter().all(|c| (0.0..=1.0).contains(c)));
    let _ = decal_colour([*mode, 1, 2, 3], light, f.small(), u32::from(*mode));
    assert!(depth_opacity(f.small(), f.small(), f.small()).is_finite());
    assert!(height_alpha(f.small(), f.small(), f.small(), f.small()).is_finite());
    let _ = draw_range(f.small(), f.small(), f.small());
});
