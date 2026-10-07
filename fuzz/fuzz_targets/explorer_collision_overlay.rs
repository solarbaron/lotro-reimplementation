// SPDX-License-Identifier: GPL-3.0-or-later
//! `tessera-explorer`'s collision overlays (`apps/explorer/src/collision_overlay.rs`,
//! included by path so this crate does not pull in the GUI toolkit): the 3D view's
//! overlays of 0x66 collision meshes, setup volumes and cell geometry
//! (`docs/specs/collision.md` §4, §5.3–§5.6, §8), and the triangle and line overlays they
//! are made of.
//!
//! Input layout: byte 0 selects the mode, byte 1 the directory stamp (0xFF = none).
//!
//! - mode 0: the rest is a 0x66 record; an accepted one becomes a shape template and its
//!   overlays;
//! - mode 1: a cell DID followed by the rest is a 0x01 record (decoded without a schema);
//!   an accepted one becomes its overlays;
//! - mode 2: the rest is read as f32s for a hand-built hostile shape template (triangles,
//!   a `mesh_triangles` count that may exceed them, spheres and capsules with non-finite
//!   or huge values) and for direct triangle and line overlays;
//! - mode 3: a collision set built from the input (bodies of every source, movement or
//!   detail, some ethereal to Player, triangles from the rest as f32s) and the portals of
//!   hand-built cells (`CellPortals`, §5.8: up to 15 cells of up to 4 portals of up to 6
//!   corners, the corners from the same f32s, so NaN, infinite and huge; for byte 0 ≥ 0xC0
//!   64 cells of 32 portals of 24 corners, past the line bound) become the landblock
//!   overlays (`collision_set_overlays`, §9.2–§9.4).
//!
//! Nothing may panic. Every overlay stays within its triangle and line bounds, keeps only
//! finite lines and a finite, ordered bounding box, and has prepared streams only when it
//! has triangles. In mode 3 each group's caption counts its bodies (at least one) and its
//! triangles: the drawn ones plus those it says were left out are at most that count, and
//! all groups together count no more triangles than the set's bodies hold; the portal
//! overlay draws at most one line per corner of the portals with two or more corners and
//! names the number of cells and portals. (Groups past `MAX_OVERLAY_TRIANGLES`, the only
//! way to leave triangles out, need sets far larger than a fuzzing run builds; the
//! explorer's unit test `oversized_groups_count_what_is_left_out` covers them.)

#![no_main]

#[allow(dead_code)]
#[path = "../../apps/explorer/src/collision_overlay.rs"]
mod collision_overlay;

use collision_overlay::{
    CollisionOverlay, MAX_OVERLAY_LINES, MAX_OVERLAY_TRIANGLES, OverlayKind, cell_overlays,
    collision_set_overlays, line_overlay, template_overlays, triangle_overlay,
};
use libfuzzer_sys::fuzz_target;
use tessera_formats::NoSchema;
use tessera_formats::collision::{CellGeometry, CollisionMesh};
use tessera_physics::geometry::{
    BodyPurpose, BodySource, Capsule, CollisionFlags, CollisionLimits, CollisionSetBuilder,
    ETHEREAL_CAMERA, ETHEREAL_PLAYER, ShapeTemplate, Sphere,
};
use tessera_world::{CellPortals, Portal};

fn check(o: &CollisionOverlay) {
    assert!(o.triangles <= MAX_OVERLAY_TRIANGLES);
    assert!(o.lines.len() <= MAX_OVERLAY_LINES);
    assert!(o.prepared.is_none() || o.triangles > 0);
    for (a, b) in &o.lines {
        assert!(a.iter().chain(b).all(|v| v.is_finite()));
    }
    if let Some((lo, hi)) = o.aabb {
        for k in 0..3 {
            assert!(lo[k].is_finite() && hi[k].is_finite() && lo[k] <= hi[k]);
        }
    }
    let _ = o.kind.color();
    let _ = o.kind.name();
    let _ = o.kind.frames();
    let _ = o.kind.shown_by_default();
    let _ = o.heap_bytes();
}

fn floats(data: &[u8]) -> Vec<f32> {
    data.as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .take(4096)
        .collect()
}

fn p3(f: &[f32], i: usize) -> [f32; 3] {
    let g = |k: usize| f.get(i + k).copied().unwrap_or(0.0);
    [g(0), g(1), g(2)]
}

/// Hand-built cells with portals whose corners come from `f` (cyclically).
fn cells(mode: u8, f: &[f32]) -> Vec<CellPortals> {
    let stress = mode >= 0xC0;
    let (cells, portals, corners) = if stress {
        (64, 32, 24)
    } else {
        (usize::from(mode >> 2) % 16, 4, 6)
    };
    let mut at = 0usize;
    let mut next = |n: usize| -> Vec<[f32; 3]> {
        (0..n)
            .map(|_| {
                let g = |k: usize| {
                    if f.is_empty() {
                        0.0
                    } else {
                        f[(at + k) % f.len()]
                    }
                };
                let p = [g(0), g(1), g(2)];
                at = at.wrapping_add(3);
                p
            })
            .collect()
    };
    (0..cells)
        .map(|c| {
            // Varying counts per cell and portal unless stressed.
            let n = if stress { portals } else { c % (portals + 1) };
            CellPortals {
                index: u16::try_from(c).unwrap_or(0),
                geometry: tessera_formats::Did(0x0100_0001),
                placement: None,
                geometry_available: c % 3 != 0,
                portals: (0..n)
                    .map(|k| Portal {
                        ordinal: u16::try_from(k).unwrap_or(0),
                        neighbour: u16::try_from(c + 1).unwrap_or(0),
                        back_portal: 0,
                        flag: 0,
                        corners: next(if stress {
                            corners
                        } else {
                            (c + k) % (corners + 1)
                        }),
                    })
                    .collect(),
            }
        })
        .collect()
}

/// The numbers in `caption`, in order, without a trailing "(refused: …)" note (the
/// renderer's message may hold numbers of its own).
fn numbers(caption: &str) -> Vec<usize> {
    let caption = caption.split(" (refused").next().unwrap_or(caption);
    caption
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|w| w.parse().ok())
        .collect()
}

/// The overlays of a collision set and cells, checked against their sources.
fn check_set(overlays: &[CollisionOverlay], body_triangles: usize, cells: &[CellPortals]) {
    let mut counted = 0usize;
    for o in overlays {
        check(o);
        let n = numbers(&o.caption);
        match o.kind {
            OverlayKind::Portals => {
                // "portals of C cells: P" and maybe "(L lines left out)".
                let portals: usize = cells.iter().map(|c| c.portals.len()).sum();
                let (c, p, left) = match n[..] {
                    [c, p] => (c, p, 0),
                    [c, p, l] => (c, p, l),
                    _ => panic!("caption {:?}", o.caption),
                };
                assert_eq!((c, p), (cells.len(), portals), "{}", o.caption);
                let segments: usize = cells
                    .iter()
                    .flat_map(|c| &c.portals)
                    .map(|p| {
                        if p.corners.len() >= 2 {
                            p.corners.len()
                        } else {
                            0
                        }
                    })
                    .sum();
                assert!(o.lines.len().saturating_add(left) <= segments);
                assert!(left == 0 || o.lines.len() == MAX_OVERLAY_LINES);
            }
            OverlayKind::Volumes => {}
            _ => {
                // "<name>: B bodies, T triangles" and maybe "(L triangles left out)".
                let (bodies, total, left) = match n[..] {
                    [b, t] => (b, t, 0),
                    [b, t, l] => (b, t, l),
                    _ => panic!("caption {:?}", o.caption),
                };
                assert!(bodies >= 1, "{}", o.caption);
                assert!(o.triangles.saturating_add(left) <= total, "{}", o.caption);
                counted = counted.saturating_add(total);
            }
        }
    }
    assert!(counted <= body_triangles, "{counted} > {body_triangles}");
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let Some((&stamp, rest)) = rest.split_first() else {
        return;
    };
    let stamp = (stamp != 0xFF).then_some(u16::from(stamp));
    match mode % 4 {
        0 => {
            if let Ok(m) = CollisionMesh::parse(rest, stamp)
                && let Ok(t) = ShapeTemplate::from_collision_mesh(&m)
            {
                for o in template_overlays(&t, "fuzz") {
                    check(&o);
                }
            }
        }
        1 => {
            let mut rec = 0x0100_0123u32.to_le_bytes().to_vec();
            rec.extend_from_slice(rest);
            if let Ok(c) = CellGeometry::parse(&rec, stamp, &NoSchema) {
                for o in cell_overlays(&c) {
                    check(&o);
                }
            }
        }
        3 => {
            let f = floats(rest);
            let tris: Vec<[[f32; 3]; 3]> = (0..f.len() / 9)
                .map(|k| [p3(&f, 9 * k), p3(&f, 9 * k + 3), p3(&f, 9 * k + 6)])
                .collect();
            let mut b = CollisionSetBuilder::new(CollisionLimits {
                max_triangles: 1 << 13,
                ..CollisionLimits::default()
            });
            // Repeat the triangles (up to the set's bound) so that bodies get large.
            let repeat = usize::from(stamp.unwrap_or(0)) * 16;
            let count = (tris.len() * (1 + repeat)).min(1 << 10);
            let t = ShapeTemplate {
                mesh_triangles: count,
                triangles: tris.iter().copied().cycle().take(count).collect(),
                ..ShapeTemplate::default()
            };
            for (k, &sel) in rest.iter().take(32).enumerate() {
                let index = u32::try_from(k).unwrap_or(0);
                let object = tessera_formats::Did(0x4700_0001);
                let source = match sel % 6 {
                    0 => BodySource::Static { index, object },
                    1 => BodySource::Generated { index, object },
                    2 => BodySource::Cell {
                        cell: u16::try_from(k).unwrap_or(0),
                        geometry: tessera_formats::Did(0x0100_0001),
                    },
                    3 => BodySource::Other { tag: index },
                    4 => BodySource::Static {
                        index: u32::MAX,
                        object,
                    },
                    _ => BodySource::Generated {
                        index: u32::MAX - index,
                        object,
                    },
                };
                let flags = CollisionFlags {
                    ethereal_to: match (sel >> 3) % 4 {
                        0 => None,
                        1 => Some(ETHEREAL_PLAYER),
                        2 => Some(ETHEREAL_CAMERA),
                        _ => Some(u64::MAX),
                    },
                    ..CollisionFlags::default()
                };
                let purpose = if sel & 0x80 != 0 {
                    BodyPurpose::Detail
                } else {
                    BodyPurpose::Movement
                };
                let _ = b.add_shape(source, flags, purpose, &t);
            }
            let set = b.build();
            let body_triangles = set
                .bodies()
                .iter()
                .map(|b| b.triangle_count as usize)
                .fold(0usize, usize::saturating_add);
            let cells = cells(mode, &f);
            check_set(
                &collision_set_overlays(&set, &cells),
                body_triangles,
                &cells,
            );
        }
        _ => {
            let f = floats(rest);
            let n = f.len() / 9;
            let tris: Vec<[[f32; 3]; 3]> = (0..n)
                .map(|k| [p3(&f, 9 * k), p3(&f, 9 * k + 3), p3(&f, 9 * k + 6)])
                .collect();
            let mut t = ShapeTemplate {
                mesh_triangles: usize::from(mode) * 7,
                triangles: tris.clone(),
                ..ShapeTemplate::default()
            };
            for k in 0..(f.len() / 4).min(64) {
                let c = p3(&f, 4 * k);
                let r = f.get(4 * k + 3).copied().unwrap_or(1.0);
                if k % 2 == 0 {
                    t.spheres.push(Sphere {
                        centre: c,
                        radius: r,
                    });
                } else {
                    t.capsules.push(Capsule {
                        a: c,
                        b: p3(&f, 4 * k + 1),
                        radius: r,
                    });
                }
            }
            for o in template_overlays(&t, "fuzz") {
                check(&o);
            }
            check(&triangle_overlay(OverlayKind::Statics, "t", tris));
            let lines = (0..f.len() / 6).map(|k| (p3(&f, 6 * k), p3(&f, 6 * k + 3)));
            check(&line_overlay(
                OverlayKind::Portals,
                "l",
                lines.cycle().take(usize::from(mode) * 300),
            ));
        }
    }
});
