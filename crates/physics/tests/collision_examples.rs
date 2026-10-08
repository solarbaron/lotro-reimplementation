// SPDX-License-Identifier: GPL-3.0-or-later
//! The synthetic worked examples of `docs/specs/collision.md` §14.3, §14.4, §14.7 and
//! §14.8, reproduced exactly, plus the terrain-plane examples of `scenery.md` §14 that use
//! the same mesh (§6.6 and `scenery.md` §7.4 step 8).

use tessera_formats::collision::TreeCode;
use tessera_formats::landblock::{GridSurface, HeightGrid, VertexFlags};
use tessera_formats::setup::Quaternion;
use tessera_physics::geometry::{CellPlacement, CellSet, CellVolume, GridMesh, PlacedCell};

/// §14.3: code `17 90 8C 01 31 30`, a split on (x + y) / 2 with hi 144, lo 140, right
/// child at +1; points on whole byte values (scale 65,536, origin 0).
#[test]
fn s14_3_diagonal_split() {
    let code = [0x17, 0x90, 0x8C, 0x01, 0x31, 0x30];
    let tree = TreeCode::new([0.0; 3], 65_536.0, &code);
    let keys = |p: [f32; 3]| {
        let mut k = tree.point_keys(p).expect("walk");
        k.sort_unstable();
        k
    };
    assert_eq!(
        keys([100.0, 150.0, 0.0]),
        vec![1],
        "(100, 150): 125 → left only"
    );
    assert_eq!(
        keys([150.0, 150.0, 0.0]),
        vec![0],
        "(150, 150): 150 → right only"
    );
    assert_eq!(
        keys([140.0, 144.0, 0.0]),
        vec![0, 1],
        "(140, 144): 142 → both"
    );
}

fn flags_with(i: usize, j: usize, value: u8) -> VertexFlags {
    let mut f = VertexFlags::filled(0);
    f.raw_mut()[33 * i + j] = value;
    f
}

/// Grid sample numbers (33 · j + i) of the corners (i, j).
fn verts(c: [(u16, u16); 3]) -> [u16; 3] {
    c.map(|(i, j)| 33 * j + i)
}

/// §14.4: quad i = 3, j = 7, base = 454, both diagonals.
#[test]
fn s14_4_terrain_keys() {
    let heights = HeightGrid::flat(0);
    let set = GridMesh::from_grid(GridSurface::Terrain, &heights, &flags_with(3, 7, 0x80));
    assert_eq!(
        set.triangle_vertices(454),
        Some(verts([(3, 7), (4, 8), (3, 8)]))
    );
    assert_eq!(
        set.triangle_vertices(455),
        Some(verts([(3, 7), (4, 7), (4, 8)]))
    );
    let clear = GridMesh::from_grid(GridSurface::Terrain, &heights, &flags_with(3, 7, 0));
    assert_eq!(
        clear.triangle_vertices(454),
        Some(verts([(3, 7), (4, 7), (3, 8)]))
    );
    assert_eq!(
        clear.triangle_vertices(455),
        Some(verts([(4, 7), (4, 8), (3, 8)]))
    );
    // Positions follow (5i, 5j, height).
    assert_eq!(
        set.triangle(454),
        Some([[15.0, 35.0, 0.0], [20.0, 40.0, 0.0], [15.0, 40.0, 0.0]])
    );
    // The ceiling uses bit 0, not bit 7, and reverses the corners (§6.4).
    let ceiling = GridMesh::from_grid(GridSurface::Ceiling, &heights, &flags_with(3, 7, 0x01));
    assert_eq!(
        ceiling.triangle_vertices(454),
        Some(verts([(3, 8), (4, 8), (3, 7)]))
    );
    assert_eq!(
        ceiling.triangle_vertices(455),
        Some(verts([(4, 8), (4, 7), (3, 7)]))
    );
    let ceiling_bit7 = GridMesh::from_grid(GridSurface::Ceiling, &heights, &flags_with(3, 7, 0x80));
    assert_eq!(
        ceiling_bit7.triangle_vertices(454),
        Some(verts([(3, 8), (4, 7), (3, 7)]))
    );
    // Terrain faces up, ceiling down.
    assert!(set.plane(454).expect("plane").normal[2] > 0.99);
    assert!(ceiling.plane(454).expect("plane").normal[2] < -0.99);
}

/// §14.7: the client's radius query.
#[test]
fn s14_7_terrain_radius_query() {
    assert_eq!(GridMesh::client_keys_near(12.0, 13.0, 0.5), vec![133]);
    assert_eq!(GridMesh::client_keys_near(12.0, 12.2, 0.5), vec![132, 133]);
    assert_eq!(GridMesh::client_keys_near(13.0, 12.0, 0.5), vec![132]);
    // With planes: flat terrain, n = (0, 0, 1).
    let mesh = GridMesh::from_grid(
        GridSurface::Terrain,
        &HeightGrid::flat(0),
        &VertexFlags::filled(0),
    );
    let near = mesh.client_triangles_near(12.0, 13.0, 0.5);
    assert_eq!(near.len(), 1);
    assert_eq!(near[0].0, 133);
    assert_eq!(near[0].1.normal, [0.0, 0.0, 1.0]);
    // Step 1: nothing outside the block.
    assert!(GridMesh::client_keys_near(-1.0, 50.0, 0.5).is_empty());
    assert!(GridMesh::client_keys_near(160.5, 50.0, 0.5).is_empty());
}

/// `scenery.md` §14 "Terrain plane band" (§7.4 step 8), the client's point rule on the
/// same mesh.
#[test]
fn scenery_plane_band_and_tie() {
    let h = HeightGrid::flat(0);
    let clear = flags_with(3, 7, 0);
    let mesh = GridMesh::from_grid(GridSurface::Terrain, &h, &clear);
    assert_eq!(mesh.client_key_at(17.5, 37.0, &clear), Some(455));
    assert_eq!(mesh.key_under(17.5, 37.0, &clear), Some(454));
    assert_eq!(mesh.client_key_at(16.0, 36.0, &clear), Some(454));
    assert_eq!(mesh.client_key_at(19.0, 38.5, &clear), Some(455));
    let set = flags_with(3, 7, 0x80);
    let mesh = GridMesh::from_grid(GridSurface::Terrain, &h, &set);
    assert_eq!(mesh.client_key_at(17.5, 37.5, &set), Some(454));
    assert_eq!(mesh.key_under(17.5, 37.5, &set), Some(455));
    // Outside 0 … 32 quads: no plane.
    assert!(mesh.client_plane_at(-0.5, 10.0, &set).is_none());
    assert!(mesh.client_plane_at(170.0, 10.0, &set).is_none());
    // 160 belongs to quad 31.
    assert!(mesh.client_key_at(160.0, 160.0, &set).is_some());
}

/// The §14.5 box room: six planes.
fn box_room() -> CellVolume {
    CellVolume::new([
        ([0.0, 1.0, 0.0], 4.0),
        ([-1.0, 0.0, 0.0], 4.0),
        ([0.0, -1.0, 0.0], 4.0),
        ([1.0, 0.0, 0.0], 4.0),
        ([0.0, 0.0, -1.0], 4.0),
        ([0.0, 0.0, 1.0], 0.0),
    ])
    .expect("volume")
}

fn box_hull() -> Vec<[f32; 3]> {
    let mut hull = Vec::new();
    for x in [-4.0, 4.0] {
        for y in [-4.0, 4.0] {
            for z in [0.0, 4.0] {
                hull.push([x, y, z]);
            }
        }
    }
    hull
}

/// §14.5 (the volume part) and §14.8: point in cell with tolerance.
#[test]
fn s14_8_point_in_cell_with_tolerance() {
    let volume = std::sync::Arc::new(box_room());
    assert!(volume.contains([0.0, 0.0, 2.0]), "§14.5: (0, 0, 2) inside");
    assert!(
        !volume.contains([5.0, 0.0, 2.0]),
        "§14.5: (5, 0, 2) outside"
    );
    assert!((volume.min_signed_distance([5.0, 0.0, 2.0]) + 1.0).abs() < 1e-12);

    let placement =
        CellPlacement::new([100.0, 50.0, 20.0], Quaternion::IDENTITY).expect("placement");
    let cell = PlacedCell::with_volume(
        7,
        tessera_formats::Did(0x0100_0001),
        volume,
        &box_hull(),
        placement,
    )
    .expect("cell");
    let mut cells = CellSet::new();
    cells.push(cell);
    let inside = cells.cell_at([104.0001, 50.0, 22.0]);
    assert_eq!(
        inside.map(|c| c.index),
        Some(7),
        "−0.0001 ≥ −0.0002: inside"
    );
    assert!(
        cells.cell_at([104.001, 50.0, 22.0]).is_none(),
        "−0.001: outside, so outdoors"
    );
}
