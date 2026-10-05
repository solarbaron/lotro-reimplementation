// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic meshes generated from a small description, for tests, fuzz seeds and
//! renderer golden scenes. Nothing here comes from game data.
//!
//! A generated mesh satisfies every invariant of `mesh.md` §14: bit-exact AABBs, unit
//! normals, weights summing to 1 with zero slots on bone 0, tight non-degenerate index
//! buffers, each submesh drawn exactly once (when the levels say so), and in-range,
//! ascending collision data.

use tessera_formats::Did;

use crate::bounds::Aabb;
use crate::error::{MeshError, Result};
use crate::format::VertexFormat;
use crate::record::{
    BuildByteRule, CollisionCounts, CollisionEdge, CollisionMesh, CollisionTree, CollisionTriangle,
    CollisionVertex, Draw, IndexBuffer, Level, LevelKind, MAX_SUBMESH_VERTICES, Mesh,
    PRIMITIVE_TRIANGLE_LIST, Skin, Submesh, TreeCode, VertexStreams,
};

/// One submesh to generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubmeshSpec {
    /// Vertex format word (must be supported, §4.2).
    pub format: u32,
    /// Vertex count, 3 … [`MAX_SUBMESH_VERTICES`].
    pub vertices: u32,
}

/// The collision tree block of a generated level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeSpec {
    /// `has_tree` = 0.
    None,
    /// `has_tree` = 1, `has_code` = 0.
    NoCode,
    /// `has_tree` = 1 with `len` code bytes.
    Code {
        /// Code length.
        len: u32,
    },
}

/// One level to generate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelSpec {
    /// Level kind.
    pub kind: LevelKind,
    /// Submesh drawn by each draw entry, in order (vertex block = index buffer = surface).
    pub draws: Vec<u32>,
    /// Whether the level carries a collision mesh (built over its first draw).
    pub collision: bool,
    /// The tree block.
    pub tree: TreeSpec,
}

impl LevelSpec {
    /// A detail level drawing `draws`, with collision mesh and tree code, as observed.
    #[must_use]
    pub fn detail(draws: Vec<u32>) -> Self {
        Self {
            kind: LevelKind::Detail,
            draws,
            collision: true,
            tree: TreeSpec::Code { len: 9 },
        }
    }

    /// A proxy level drawing `draws`, without collision data, as observed.
    #[must_use]
    pub fn proxy(draws: Vec<u32>) -> Self {
        Self {
            kind: LevelKind::Proxy,
            draws,
            collision: false,
            tree: TreeSpec::None,
        }
    }
}

/// A whole mesh to generate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshSpec {
    /// The DID (prefix 0x06 for a valid mesh).
    pub did: u32,
    /// The `flags` word.
    pub flags: u32,
    /// Submeshes; submesh *i* uses surface `0x31000000 + did serial + i`.
    pub submeshes: Vec<SubmeshSpec>,
    /// Levels.
    pub levels: Vec<LevelSpec>,
    /// Whether tree code blocks end with a `build_byte` (stamp ≥ 38, §10.4).
    pub build_byte: bool,
}

impl MeshSpec {
    /// One submesh drawn by one detail level.
    #[must_use]
    pub fn single(format: u32, vertices: u32) -> Self {
        Self {
            did: 0x0600_0001,
            flags: 1,
            submeshes: vec![SubmeshSpec { format, vertices }],
            levels: vec![LevelSpec::detail(vec![0])],
            build_byte: true,
        }
    }

    /// The stamp that matches [`MeshSpec::build_byte`] (39 or 7).
    #[must_use]
    pub fn stamp(&self) -> u16 {
        if self.build_byte { 39 } else { 7 }
    }

    /// Generates the mesh.
    ///
    /// # Errors
    ///
    /// [`MeshError::UnsupportedVertexFormat`] for a refused format word,
    /// [`MeshError::LimitExceeded`] for fewer than 3 or more than
    /// [`MAX_SUBMESH_VERTICES`] vertices, [`MeshError::DrawOutOfRange`] for a draw naming
    /// a missing submesh, [`MeshError::Empty`] for no levels or a level without draws
    /// (§8.1 requires at least one of each; the decoder rejects both).
    pub fn build(&self) -> Result<Mesh> {
        if self.levels.is_empty() {
            return Err(MeshError::Empty {
                what: "level list".into(),
            });
        }
        if let Some(li) = self.levels.iter().position(|l| l.draws.is_empty()) {
            return Err(MeshError::Empty {
                what: format!("draw list of level {li}"),
            });
        }
        let mut submeshes = Vec::with_capacity(self.submeshes.len());
        let mut index_buffers = Vec::with_capacity(self.submeshes.len());
        for (i, s) in self.submeshes.iter().enumerate() {
            let format =
                VertexFormat::new(s.format).map_err(|e| MeshError::UnsupportedVertexFormat {
                    submesh: i as u32,
                    vfmt: s.format,
                    reason: e.reason,
                })?;
            if !(3..=MAX_SUBMESH_VERTICES).contains(&s.vertices) {
                return Err(MeshError::LimitExceeded {
                    what: "synthetic vertices",
                    value: u64::from(s.vertices),
                    limit: u64::from(MAX_SUBMESH_VERTICES),
                });
            }
            let (sm, ib) = submesh(format, s.vertices, i as u32);
            submeshes.push(sm);
            index_buffers.push(ib);
        }
        let n = submeshes.len() as u32;
        let mut levels = Vec::with_capacity(self.levels.len());
        for (li, l) in self.levels.iter().enumerate() {
            let mut draws = Vec::with_capacity(l.draws.len());
            for (di, &s) in l.draws.iter().enumerate() {
                if s >= n {
                    return Err(MeshError::DrawOutOfRange {
                        level: li as u32,
                        draw: di as u32,
                        field: "vertex_block",
                        value: s,
                        count: n,
                    });
                }
                draws.push(Draw {
                    primitive: PRIMITIVE_TRIANGLE_LIST,
                    vertex_block: s,
                    index_buffer: s,
                    surface: s,
                });
            }
            let collision = (l.collision)
                .then(|| {
                    draws
                        .first()
                        .map(|d| collision(&index_buffers[d.vertex_block as usize]))
                })
                .map(Option::unwrap_or_default);
            let tree = match l.tree {
                TreeSpec::None => None,
                TreeSpec::NoCode => Some(CollisionTree::NoCode),
                TreeSpec::Code { len } => Some(CollisionTree::Code(TreeCode {
                    origin: [-0.05, -0.05, -0.05],
                    scale: 1.0e6,
                    code_len: len,
                    code: Some((0..len).map(|i| (0x28 + i) as u8).collect()),
                    build_byte: self.build_byte.then_some(1),
                })),
            };
            levels.push(Level {
                kind: l.kind,
                draws,
                collision_counts: collision.as_ref().map(|c: &CollisionMesh| CollisionCounts {
                    vertices: c.vertices.len() as u32,
                    triangles: c.triangles.len() as u32,
                    edges: c.edges.len() as u32,
                }),
                collision,
                tree,
            });
        }
        let serial = self.did & 0x00FF_FFFF;
        let mut mesh = Mesh {
            did: Did(self.did),
            flags: self.flags,
            surfaces: (0..n)
                .map(|i| Did(0x3100_0000 + ((serial + i) & 0x00FF_FFFF)))
                .collect(),
            submeshes,
            index_buffers,
            levels,
            bounds: None,
            build_byte_rule: BuildByteRule::Stamp(self.stamp()),
        };
        mesh.bounds = mesh.level_bounds(0);
        Ok(mesh)
    }
}

/// A strip of triangles over `v` vertices on a wavy grid; every attribute the format
/// asks for is filled with simple, valid values that vary per vertex.
fn submesh(format: VertexFormat, v: u32, seed: u32) -> (Submesh, IndexBuffer) {
    let l = format.layout();
    let n = v as usize;
    let k = usize::from(l.influences);
    let positions: Vec<[f32; 3]> = (0..v)
        .map(|i| {
            let x = (i % 16) as f32 * 0.25 + seed as f32;
            let y = (i / 16) as f32 * 0.25;
            let z = ((i * 7 + seed) % 5) as f32 * 0.125;
            [x, y, z]
        })
        .collect();
    let unit = |i: u32| {
        let a = (i % 13) as f32 * 0.4;
        let (s, c) = a.sin_cos();
        let v = [c * 0.6, s * 0.6, 0.8f32];
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        v.map(|x| x / len)
    };
    let streams = VertexStreams {
        normals: l.normal.map(|_| (0..v).map(unit).collect()),
        scalars: l
            .scalar
            .map(|_| (0..v).map(|i| 4.0 + (i % 20) as f32).collect()),
        colors: l.color.map(|_| {
            (0..v)
                .map(|i| [i as u8, (i * 3) as u8, (i * 7) as u8, 255])
                .collect()
        }),
        uv_sets: (0..l.uv_sets)
            .map(|s| {
                (0..v)
                    .map(|i| [(i % 16) as f32 / 15.0 + f32::from(s), (i / 16) as f32 * 0.5])
                    .collect()
            })
            .collect(),
        tangents: l.tangent.map(|_| vec![[1.0, 0.0, 0.0]; n]),
        binormals: l.binormal.map(|_| {
            (0..v)
                .map(|i| {
                    if i % 3 == 0 {
                        [0.0, -1.0, 0.0]
                    } else {
                        [0.0, 1.0, 0.0]
                    }
                })
                .collect()
        }),
        skin: (k > 0).then(|| {
            let mut joints = Vec::with_capacity(n * k);
            let mut weights = Vec::with_capacity(n * k);
            for i in 0..v {
                // Vertex i uses 1 + (i mod K) influences; the rest are zero slots on bone 0.
                let used = 1 + (i as usize % k);
                let rest = 0.25 / used as f32;
                let first = 1.0 - rest * (used - 1) as f32;
                for slot in 0..k {
                    if slot < used {
                        joints.push(((i as usize + slot * 5) % 60) as u8 + u8::from(slot > 0));
                        weights.push(if slot == 0 { first } else { rest });
                    } else {
                        joints.push(0);
                        weights.push(0.0);
                    }
                }
            }
            Skin {
                influences: l.influences,
                joints,
                weights,
            }
        }),
        positions,
    };
    // The influence list the vertex data implies (`mesh.md` §6.2).
    let influence_list = streams
        .skin
        .as_ref()
        .map(Skin::influence_list)
        .unwrap_or_default();
    let stored_aabb = Aabb::from_points(&streams.positions).unwrap_or(Aabb {
        min: [0.0; 3],
        max: [0.0; 3],
    });
    let indices: Vec<u16> = (0..v.saturating_sub(2))
        .flat_map(|i| {
            if i % 2 == 0 {
                [i, i + 1, i + 2]
            } else {
                [i + 1, i, i + 2]
            }
        })
        .map(|x| x as u16)
        .collect();
    (
        Submesh {
            format,
            vertex_count: v,
            streams,
            stored_aabb,
            influence_list,
        },
        IndexBuffer { indices },
    )
}

/// A collision mesh over the first triangle of the level's first draw.
fn collision(ib: &IndexBuffer) -> CollisionMesh {
    let Some(t) = ib.triangles().next() else {
        return CollisionMesh::default();
    };
    let mut verts: Vec<u16> = t.to_vec();
    verts.sort_unstable();
    let pos = |x: u16| verts.iter().position(|&y| y == x).unwrap_or(0) as u16;
    let tri = [pos(t[0]), pos(t[1]), pos(t[2])];
    CollisionMesh {
        vertices: verts
            .iter()
            .map(|&vertex| CollisionVertex { draw: 0, vertex })
            .collect(),
        triangles: vec![CollisionTriangle {
            vertices: tri,
            edges: [0, 1, 2],
        }],
        edges: (0..3)
            .map(|k| CollisionEdge {
                vertices: [tri[k], tri[(k + 1) % 3]],
                faces: [0, CollisionEdge::NO_FACE],
            })
            .collect(),
    }
}
