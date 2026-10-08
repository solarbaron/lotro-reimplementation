// SPDX-License-Identifier: GPL-3.0-or-later
//! Cell geometry, prefix 0x01 (`collision.md` §5): the optional physics mesh, the cell
//! volume, portals, wall polygons, hull vertices, properties and the embedded render mesh.
//!
//! The embedded render mesh (§5.7) is a mesh record body without its DID, decoded by the
//! mesh crate; this decoder keeps its bytes ([`CellGeometry::render_mesh`]) and can rebuild
//! a complete mesh record for it ([`CellGeometry::render_mesh_record`]). Deciding which cell
//! contains a point (§5.4, §5.8) is left to the physics layer.

use crate::did::{Did, DidKind};
use crate::error::{FormatError, Result};
use crate::landblock::{LayoutRule, read_embedded_set};
use crate::property::PropertySchema;
use crate::property_set::PropertySet;
use crate::reader::Reader;
use crate::record::read_prologue;

use super::mesh::{CollisionMesh, Follower};

/// Type prefix of cell-geometry records ([`DidKind::CollisionGeometry`], DB type CELLMESH;
/// `records.md` §2.1).
pub const CELL_GEOMETRY_PREFIX: u8 = 0x01;

/// Tag of every plane entry but the last: "BP--" read as big-endian characters
/// (`collision.md` §5.4).
pub const PLANE_TAG_NEXT: u32 = 0x4250_2D2D;

/// One of the two tags of the last plane entry: "BPFL".
pub const PLANE_TAG_BPFL: u32 = 0x4250_464C;

/// The other tag of the last plane entry: "LEAF".
pub const PLANE_TAG_LEAF: u32 = 0x4C45_4146;

/// Stored size of a plane entry: tag, normal, distance.
pub const PLANE_ENTRY_LEN: usize = 20;

/// Stored `mesh_count` of every record (part 9).
pub const CELL_MESH_COUNT: u32 = 1;

/// Stored `mesh_flag` of every record (part 10).
pub const CELL_MESH_FLAG: u8 = 0;

/// Stored size of a polygon corner (3 × f32).
const CORNER_LEN: usize = 12;

/// Whether `tag` is one of the three plane tags of §5.4.
#[must_use]
pub fn is_plane_tag(tag: u32) -> bool {
    matches!(tag, PLANE_TAG_NEXT | PLANE_TAG_BPFL | PLANE_TAG_LEAF)
}

/// How a cell's plane list ends (`collision.md` §5.4; the difference is OPEN, readers treat
/// both alike).
///
/// §5.4 ends the list at the first entry whose tag is not [`PLANE_TAG_NEXT`], whatever that
/// tag is, and that entry is still a plane; the observed data uses only "BPFL" and "LEAF"
/// (CO2), so any other tag is kept as [`PlaneListEnd::Other`] rather than refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaneListEnd {
    /// 0x4250464C, "BPFL".
    Bpfl,
    /// 0x4C454146, "LEAF".
    Leaf,
    /// Any other tag (never observed), as stored.
    Other(u32),
}

impl PlaneListEnd {
    /// The end of a plane list whose last entry carries `tag`; `None` for
    /// [`PLANE_TAG_NEXT`], which does not end the list.
    #[must_use]
    pub fn from_tag(tag: u32) -> Option<Self> {
        match tag {
            PLANE_TAG_NEXT => None,
            PLANE_TAG_BPFL => Some(Self::Bpfl),
            PLANE_TAG_LEAF => Some(Self::Leaf),
            other => Some(Self::Other(other)),
        }
    }

    /// The stored tag.
    #[must_use]
    pub fn tag(self) -> u32 {
        match self {
            Self::Bpfl => PLANE_TAG_BPFL,
            Self::Leaf => PLANE_TAG_LEAF,
            Self::Other(tag) => tag,
        }
    }
}

/// One plane of the cell volume (§5.4). The volume is the convex region where
/// *n* · *p* + *d* ≥ 0 for every plane.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CellPlane {
    /// Unit normal *n*, pointing into the cell.
    pub normal: [f32; 3],
    /// Distance *d*.
    pub distance: f32,
}

impl CellPlane {
    /// *n* · *p* + *d*: positive inside, negative outside.
    #[must_use]
    pub fn signed_distance(&self, p: [f32; 3]) -> f32 {
        self.normal[0] * p[0] + self.normal[1] * p[1] + self.normal[2] * p[2] + self.distance
    }
}

/// A portal: an opening in the cell volume's boundary that leads to a neighbouring cell
/// (§5.5, §5.8). Portal *k* is landblock-info neighbour entry *k*.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CellPortal {
    /// The leading byte; 1 in every observed portal (meaning OPEN, §5.5).
    pub lead: u8,
    /// Corners in the cell frame; 4 in every observed portal (winding OPEN).
    pub corners: Vec<[f32; 3]>,
}

/// A closed piece of the cell volume's boundary (§5.5): 3–8 corners in the observed data.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WallPolygon {
    /// Corners in the cell frame (winding OPEN).
    pub corners: Vec<[f32; 3]>,
}

/// A decoded cell-geometry record 0x01 (`collision.md` §5.1), consumed to the last byte.
/// All coordinates are in the cell frame (§5.8).
#[derive(Debug, Clone, PartialEq)]
pub struct CellGeometry {
    /// Own DID.
    pub did: Did,
    /// The physics mesh (§5.3): the cell's solid collision body. Absent in 47 records.
    pub physics_mesh: Option<CollisionMesh>,
    /// The cell volume's planes in stored order (§5.4); at least one. Readers must not
    /// assume a floor plane at any position.
    pub planes: Vec<CellPlane>,
    /// The tag of the last plane entry.
    pub plane_list_end: PlaneListEnd,
    /// Portals (§5.5), index for index the landblock-info cell's neighbour entries.
    pub portals: Vec<CellPortal>,
    /// Closed wall pieces of the volume's faces (§5.5).
    pub walls: Vec<WallPolygon>,
    /// Hull vertices (§5.6): corners of the volume and of the pieces its faces are split
    /// into. The client's cell phantom is their convex hull (§5.8.1).
    pub hull: Vec<[f32; 3]>,
    /// The embedded property set (part 8, §5.7).
    pub properties: PropertySet,
    /// The embedded render mesh (part 11, §5.7): a mesh record from its `flags` onward,
    /// without the DID (`mesh.md` §13), to the end of the record.
    pub render_mesh: Vec<u8>,
    /// How the physics mesh tree's `build_byte` was decided (`Stamp` when there is no
    /// physics mesh and a stamp was given).
    pub layout_rule: LayoutRule,
}

impl CellGeometry {
    /// Human-readable record type name, used in errors.
    pub const NAME: &'static str = "cell geometry";

    /// Decodes a whole 0x01 payload (`collision.md` §5.1).
    ///
    /// `stamp` is the record's directory stamp: the physics mesh tree's `build_byte` is
    /// present iff it is ≥ 38 (§3.1); the embedded render mesh uses the same stamp when it
    /// is decoded. Without a stamp, or with an unobserved one (34–37), a `build_byte` is
    /// taken unless the four bytes after the code are a plane tag (a Tessera fallback).
    /// `schema` decodes the embedded property set (`property-system.md` §5).
    ///
    /// # Errors
    ///
    /// [`FormatError::WrongRecordType`] for a DID without prefix 0x01; everything
    /// [`CollisionMesh::parse`] reports for the physics mesh; [`FormatError::InvalidBool`]
    /// for `has_physics_mesh` > 1; [`FormatError::Unsupported`] for a `mesh_count` other
    /// than 1 or a `mesh_flag` other than 0 (never observed; the layout would differ); [`FormatError::InPropertySet`] wrapping a
    /// property-set failure; [`FormatError::Truncated`] when the record ends before the
    /// render mesh's `flags`.
    pub fn parse<S: PropertySchema + ?Sized>(
        data: &[u8],
        stamp: Option<u16>,
        schema: &S,
    ) -> Result<Self> {
        let mut r = Reader::new(data);
        let rec = Self::read(&mut r, stamp, schema)?;
        r.expect_end(Self::NAME)?;
        r.log_unknown_hints(Self::NAME);
        Ok(rec)
    }

    /// [`CellGeometry::parse`], also checking that the record carries `expected`.
    ///
    /// # Errors
    ///
    /// [`FormatError::DidMismatch`], or anything [`CellGeometry::parse`] returns.
    pub fn parse_expecting<S: PropertySchema + ?Sized>(
        data: &[u8],
        expected: impl Into<Did>,
        stamp: Option<u16>,
        schema: &S,
    ) -> Result<Self> {
        let expected = expected.into();
        let rec = Self::parse(data, stamp, schema)?;
        if rec.did != expected {
            return Err(FormatError::DidMismatch {
                expected: expected.0,
                found: rec.did.0,
            });
        }
        Ok(rec)
    }

    fn read<S: PropertySchema + ?Sized>(
        r: &mut Reader<'_>,
        stamp: Option<u16>,
        schema: &S,
    ) -> Result<Self> {
        let did = read_prologue(r, Self::NAME, |k| k == DidKind::CollisionGeometry)?;
        let (physics_mesh, layout_rule) = if r.bool8()? {
            let m = CollisionMesh::read(r, stamp, Follower::PlaneList)?;
            let rule = m.layout_rule;
            (Some(m), rule)
        } else {
            let rule = if stamp.is_some() {
                LayoutRule::Stamp
            } else {
                LayoutRule::Fallback
            };
            (None, rule)
        };

        // Part 4: planes until a tag other than "BP--"; that entry is the last plane, whatever
        // its tag (§5.4).
        let max_planes = u32::try_from(r.remaining() / PLANE_ENTRY_LEN).unwrap_or(u32::MAX);
        let mut planes = r.alloc_vec(1, PLANE_ENTRY_LEN, usize::MAX)?;
        let plane_list_end = loop {
            let at = r.position();
            let tag = r.u32()?;
            let plane = CellPlane {
                normal: [r.f32()?, r.f32()?, r.f32()?],
                distance: r.f32()?,
            };
            r.push(&mut planes, plane, max_planes)?;
            match PlaneListEnd::from_tag(tag) {
                None => {}
                Some(end @ PlaneListEnd::Other(_)) => {
                    tracing::debug!(
                        offset = at,
                        tag = format_args!("{tag:#010x}"),
                        "cell geometry: unobserved last-plane tag, read as the last plane"
                    );
                    break end;
                }
                Some(end) => break end,
            }
        };

        // Part 5: portals.
        let count = r.u32()?;
        let mut portals = r.alloc_vec(count, 5, usize::MAX)?;
        for _ in 0..count {
            let lead = r.u8()?;
            let corners = read_corners(r)?;
            r.push(&mut portals, CellPortal { lead, corners }, count)?;
        }

        // Part 6: wall polygons.
        let count = r.u32()?;
        let mut walls = r.alloc_vec(count, 4, usize::MAX)?;
        for _ in 0..count {
            let corners = read_corners(r)?;
            r.push(&mut walls, WallPolygon { corners }, count)?;
        }

        // Part 7: hull vertices.
        let count = r.vle()?;
        let mut hull = r.alloc_vec(count, CORNER_LEN, usize::MAX)?;
        for _ in 0..count {
            let v = [r.f32()?, r.f32()?, r.f32()?];
            r.push(&mut hull, v, count)?;
        }

        // Part 8: properties.
        let properties = read_embedded_set(r, schema)?;

        // Parts 9 and 10.
        let at = r.position();
        let mesh_count = r.u32()?;
        if mesh_count != CELL_MESH_COUNT {
            return Err(FormatError::Unsupported {
                what: "cell mesh_count",
                offset: at,
                value: mesh_count,
            });
        }
        let at = r.position();
        let flag = r.u8()?;
        if flag != CELL_MESH_FLAG {
            return Err(FormatError::Unsupported {
                what: "cell mesh_flag",
                offset: at,
                value: u32::from(flag),
            });
        }

        // Part 11: the render mesh body, from `flags` to the end.
        r.peek(4)?;
        let body = r.bytes(r.remaining())?;
        r.charge(body.len())?;
        let mut render_mesh = Vec::new();
        render_mesh
            .try_reserve_exact(body.len())
            .map_err(|_| FormatError::MemoryLimit {
                offset: r.position(),
                limit: r.budget(),
            })?;
        render_mesh.extend_from_slice(body);

        Ok(Self {
            did,
            physics_mesh,
            planes,
            plane_list_end,
            portals,
            walls,
            hull,
            properties,
            render_mesh,
            layout_rule,
        })
    }

    /// The embedded render mesh as a complete mesh record (`mesh.md` §13): the cell's DID
    /// followed by [`CellGeometry::render_mesh`]. Decode it with the mesh crate, passing
    /// this record's directory stamp (§5.7: the stamp decides the `build_byte`s).
    #[must_use]
    pub fn render_mesh_record(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + self.render_mesh.len());
        out.extend_from_slice(&self.did.0.to_le_bytes());
        out.extend_from_slice(&self.render_mesh);
        out
    }
}

/// Reads a u32 corner count and that many corners.
fn read_corners(r: &mut Reader<'_>) -> Result<Vec<[f32; 3]>> {
    let count = r.u32()?;
    let mut corners = r.alloc_vec(count, CORNER_LEN, usize::MAX)?;
    for _ in 0..count {
        let v = [r.f32()?, r.f32()?, r.f32()?];
        r.push(&mut corners, v, count)?;
    }
    Ok(corners)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::property::NoSchema;

    /// DID, no physics mesh, one "BPFL" plane, no portals or walls, one hull vertex, an
    /// empty property set, mesh count and flag, and a 4-byte mesh body.
    fn minimal(tag: u32) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&0x0100_0001u32.to_le_bytes());
        b.push(0);
        b.extend_from_slice(&tag.to_le_bytes());
        for v in [0.0f32, 0.0, 1.0, 0.0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.push(1);
        b.extend_from_slice(&[0; 12]);
        b.extend_from_slice(&[0, 0]);
        b.extend_from_slice(&1u32.to_le_bytes());
        b.push(0);
        b.extend_from_slice(&1u32.to_le_bytes());
        b
    }

    #[test]
    fn minimal_record() {
        let c = CellGeometry::parse(&minimal(PLANE_TAG_BPFL), Some(7), &NoSchema).unwrap();
        assert_eq!(c.did, Did(0x0100_0001));
        assert!(c.physics_mesh.is_none());
        assert_eq!(c.planes.len(), 1);
        assert_eq!(c.plane_list_end, PlaneListEnd::Bpfl);
        assert_eq!(c.hull.len(), 1);
        assert_eq!(c.render_mesh, 1u32.to_le_bytes());
        assert_eq!(c.layout_rule, LayoutRule::Stamp);
        let rec = c.render_mesh_record();
        assert_eq!(&rec[..4], &0x0100_0001u32.to_le_bytes());
        let c = CellGeometry::parse(&minimal(PLANE_TAG_LEAF), None, &NoSchema).unwrap();
        assert_eq!(c.plane_list_end, PlaneListEnd::Leaf);
        assert_eq!(c.layout_rule, LayoutRule::Fallback);
        assert!(matches!(
            CellGeometry::parse_expecting(
                &minimal(PLANE_TAG_LEAF),
                0x0100_0002u32,
                None,
                &NoSchema
            ),
            Err(FormatError::DidMismatch { .. })
        ));
    }

    #[test]
    fn any_other_tag_ends_the_plane_list() {
        // §5.4: the first tag other than "BP--" ends the list, and that entry is a plane.
        let c = CellGeometry::parse(&minimal(0x1234_5678), None, &NoSchema).unwrap();
        assert_eq!(c.plane_list_end, PlaneListEnd::Other(0x1234_5678));
        assert_eq!(c.plane_list_end.tag(), 0x1234_5678);
        assert_eq!(c.planes.len(), 1);
        assert_eq!(c.planes[0].normal, [0.0, 0.0, 1.0]);
        assert_eq!(c.hull.len(), 1);
        for end in [
            PlaneListEnd::Bpfl,
            PlaneListEnd::Leaf,
            PlaneListEnd::Other(0),
        ] {
            assert_eq!(PlaneListEnd::from_tag(end.tag()), Some(end));
        }
        assert_eq!(PlaneListEnd::from_tag(PLANE_TAG_NEXT), None);
    }

    #[test]
    fn rejects_unknown_shapes() {
        let mut b = minimal(PLANE_TAG_BPFL);
        b[0..4].copy_from_slice(&0x0400_0001u32.to_le_bytes());
        assert!(matches!(
            CellGeometry::parse(&b, None, &NoSchema),
            Err(FormatError::WrongRecordType { .. })
        ));
        let mut b = minimal(PLANE_TAG_BPFL);
        let n = b.len();
        b[n - 5] = 1; // mesh_flag
        assert!(matches!(
            CellGeometry::parse(&b, None, &NoSchema),
            Err(FormatError::Unsupported {
                what: "cell mesh_flag",
                ..
            })
        ));
        let mut b = minimal(PLANE_TAG_BPFL);
        b[n - 9] = 2; // mesh_count
        assert!(matches!(
            CellGeometry::parse(&b, None, &NoSchema),
            Err(FormatError::Unsupported {
                what: "cell mesh_count",
                ..
            })
        ));
        // No render mesh body at all.
        let b = &minimal(PLANE_TAG_BPFL)[..n - 4];
        assert!(matches!(
            CellGeometry::parse(b, None, &NoSchema),
            Err(FormatError::Truncated { .. })
        ));
        // has_physics_mesh = 2.
        let mut b = minimal(PLANE_TAG_BPFL);
        b[4] = 2;
        assert!(matches!(
            CellGeometry::parse(&b, None, &NoSchema),
            Err(FormatError::InvalidBool { value: 2, .. })
        ));
    }

    #[test]
    fn plane_distance() {
        let p = CellPlane {
            normal: [-1.0, 0.0, 0.0],
            distance: 4.0,
        };
        assert_eq!(p.signed_distance([5.0, 0.0, 2.0]), -1.0);
        assert!(is_plane_tag(PLANE_TAG_NEXT) && !is_plane_tag(0));
    }
}
