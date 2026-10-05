// SPDX-License-Identifier: GPL-3.0-or-later
//! Invariant checks on a decoded mesh (`mesh.md` §14).
//!
//! The decoder rejects only what stops it from reading the record or would make the
//! result unsafe to draw (truncation, unsupported formats, out-of-range indices, bad
//! counts). Everything else the spec states as an invariant is checked here, after
//! decoding, and reported as categorised [`Issue`]s. A viewer may log issues and still
//! draw (§14); strict decoding ([`crate::DecodeOptions::strict`]) turns the first
//! structural one into an error.

use std::fmt;

use crate::record::{
    CollisionEdge, CollisionTree, LevelKind, MESH_PREFIX, Mesh, SURFACE_PREFIX, Submesh,
};

/// Tolerance on the length of a normal (§5.2 observes ±0.001).
pub const NORMAL_TOLERANCE: f64 = 1e-3;

/// Tolerance on the sum of a vertex's weights (§5.7 observes ±2 × 10⁻⁴; the check is
/// looser so that summation order cannot matter).
pub const WEIGHT_SUM_TOLERANCE: f64 = 1e-3;

/// Most issues kept in one report; further ones are only counted.
pub const MAX_ISSUES: usize = 1024;

/// Whether an issue means the decoder and the data disagree, or only that the data is
/// unusual (`mesh.md` §14, last paragraph).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IssueClass {
    /// MI1, MI2, MI8, MI9, MI11, MI12-type disagreement.
    Structural,
    /// MI4, MI5, MI6, MI7, MI10 and non-finite values: a viewer may still draw.
    Quality,
}

/// A vertex attribute, for [`IssueKind::NonFinite`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Attribute {
    /// Position.
    Position,
    /// Normal.
    Normal,
    /// Scalar.
    Scalar,
    /// A UV set.
    Uv,
    /// Tangent.
    Tangent,
    /// Binormal.
    Binormal,
    /// Bone weight.
    Weight,
    /// The stored AABB.
    Aabb,
    /// Collision tree origin or scale.
    TreeQuantisation,
}

/// What an [`Issue`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum IssueKind {
    /// The record DID is not a mesh DID (prefix 0x06).
    NotMeshDid,
    /// A surface entry is not a surface-record DID (prefix 0x31, §12).
    NotSurfaceDid,
    /// A submesh has no vertices (never observed).
    ZeroVertices,
    /// A stream does not hold one entry per vertex (see [`crate::Submesh::check_streams`]).
    /// Never true of a decoded mesh; reported for hand-built or edited ones, which
    /// [`Mesh::level_geometry`] refuses.
    StreamLength,
    /// NaN or infinity in an attribute (count of values).
    NonFinite(Attribute),
    /// MI4: the stored AABB is not the min/max of the positions.
    AabbMismatch,
    /// MI5: a normal is not unit length (count of vertices).
    NormalNotUnit,
    /// MI5: a vertex's weights do not sum to 1.
    WeightSum,
    /// MI5 / §5.7: a negative weight.
    NegativeWeight,
    /// MI5: the first weight of a vertex is not positive.
    FirstWeightNotPositive,
    /// MI5: a zero weight with a non-zero bone index.
    UnusedSlotNonZeroIndex,
    /// MI6: influence-list length ≠ number of non-zero weights.
    InfluenceListLength,
    /// §6.2: the influence list has the right length but differs from the list the vertex
    /// data implies ([`crate::Submesh::computed_influence_list`]; never observed).
    InfluenceListContent,
    /// MI7: the smallest index of a non-empty buffer is not 0.
    IndexMinNotZero,
    /// MI7: the largest index is not `vertex_count − 1`.
    IndexMaxNotLast,
    /// MI7: vertices no triangle uses (count).
    UnusedVertex,
    /// MI7: triangles that repeat a vertex (count).
    DegenerateTriangle,
    /// MI8: a draw entry whose vertex block, index buffer and surface differ.
    DrawIndicesDiffer,
    /// MI8: a submesh no level draws.
    SubmeshNotDrawn,
    /// MI8: a submesh drawn more than once over all levels.
    SubmeshDrawnTwice,
    /// MI9: level kind other than 1 or 2.
    UnknownLevelKind,
    /// MI9: a detail level after a proxy level.
    ProxyBeforeDetail,
    /// MI9: a detail level without a collision mesh.
    DetailWithoutCollision,
    /// MI9: a proxy level with a collision mesh.
    ProxyWithCollision,
    /// MI10: a collision vertex refers to a missing draw or vertex (count).
    CollisionVertexOutOfRange,
    /// MI10: collision vertices not strictly ascending by (draw, vertex).
    CollisionVerticesNotAscending,
    /// MI10: a triangle or edge index out of range (count).
    CollisionIndexOutOfRange,
    /// MI10: an edge with two faces where fA ≥ fB (count). Not checked on levels with
    /// [`IssueKind::CollisionIndexOverflow`].
    CollisionEdgeFaceOrder,
    /// A collision mesh with more than 65,536 vertices, triangles or edges: its u16
    /// indices cannot address every element (`mesh.md` §9.1, OPEN). Reported once per
    /// level.
    CollisionIndexOverflow,
    /// §10.1: a `build_byte` other than 1.
    BuildByteValue,
}

impl IssueKind {
    /// Structural or quality.
    #[must_use]
    pub fn class(self) -> IssueClass {
        match self {
            Self::NotMeshDid
            | Self::DrawIndicesDiffer
            | Self::SubmeshNotDrawn
            | Self::SubmeshDrawnTwice
            | Self::UnknownLevelKind
            | Self::ProxyBeforeDetail
            | Self::DetailWithoutCollision
            | Self::ProxyWithCollision
            | Self::BuildByteValue
            | Self::StreamLength => IssueClass::Structural,
            _ => IssueClass::Quality,
        }
    }

    /// The spec invariant this checks, for reports.
    #[must_use]
    pub fn invariant(self) -> &'static str {
        match self {
            Self::NotMeshDid => "MI1",
            Self::NotSurfaceDid => "§12",
            Self::ZeroVertices => "§3.2",
            Self::StreamLength => "§5",
            Self::NonFinite(_) => "§5",
            Self::AabbMismatch => "MI4",
            Self::NormalNotUnit
            | Self::WeightSum
            | Self::NegativeWeight
            | Self::FirstWeightNotPositive
            | Self::UnusedSlotNonZeroIndex => "MI5",
            Self::InfluenceListLength => "MI6",
            Self::InfluenceListContent => "§6.2",
            Self::IndexMinNotZero
            | Self::IndexMaxNotLast
            | Self::UnusedVertex
            | Self::DegenerateTriangle => "MI7",
            Self::DrawIndicesDiffer | Self::SubmeshNotDrawn | Self::SubmeshDrawnTwice => "MI8",
            Self::UnknownLevelKind
            | Self::ProxyBeforeDetail
            | Self::DetailWithoutCollision
            | Self::ProxyWithCollision => "MI9",
            Self::CollisionVertexOutOfRange
            | Self::CollisionVerticesNotAscending
            | Self::CollisionIndexOutOfRange
            | Self::CollisionEdgeFaceOrder
            | Self::CollisionIndexOverflow => "MI10",
            Self::BuildByteValue => "MI11",
        }
    }
}

/// One failed invariant, aggregated per kind and location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Issue {
    /// What failed.
    pub kind: IssueKind,
    /// The submesh (or index buffer) concerned, if any.
    pub submesh: Option<u32>,
    /// The level concerned, if any.
    pub level: Option<u32>,
    /// How many values, vertices, triangles or entries fail (≥ 1).
    pub count: u64,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {:?}", self.kind.invariant(), self.kind)?;
        if let Some(s) = self.submesh {
            write!(f, " submesh {s}")?;
        }
        if let Some(l) = self.level {
            write!(f, " level {l}")?;
        }
        write!(f, " ×{}", self.count)
    }
}

/// The result of [`Mesh::validate`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationReport {
    /// Issues, at most [`MAX_ISSUES`], in check order.
    pub issues: Vec<Issue>,
    /// Issues found beyond [`MAX_ISSUES`].
    pub dropped: u64,
}

impl ValidationReport {
    /// Whether no invariant failed.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty() && self.dropped == 0
    }

    /// Whether a structural invariant failed.
    #[must_use]
    pub fn has_structural(&self) -> bool {
        self.issues
            .iter()
            .any(|i| i.kind.class() == IssueClass::Structural)
    }

    fn add(&mut self, kind: IssueKind, submesh: Option<u32>, level: Option<u32>, count: u64) {
        if count == 0 {
            return;
        }
        if self.issues.len() >= MAX_ISSUES {
            self.dropped += 1;
        } else {
            self.issues.push(Issue {
                kind,
                submesh,
                level,
                count,
            });
        }
    }
}

/// Whether `x` is within `tol` of 1 (false for NaN).
fn within(x: f64, tol: f64) -> bool {
    (x - 1.0).abs() <= tol
}

fn non_finite3(v: &[[f32; 3]]) -> u64 {
    v.iter().flatten().filter(|x| !x.is_finite()).count() as u64
}

impl Mesh {
    /// Checks every invariant of `mesh.md` §14 that decoding does not already enforce.
    ///
    /// Cost: one pass over every vertex, index and collision element.
    #[must_use]
    pub fn validate(&self) -> ValidationReport {
        let mut r = ValidationReport::default();
        if self.did.prefix() != MESH_PREFIX {
            r.add(IssueKind::NotMeshDid, None, None, 1);
        }
        let bad_surfaces = self
            .surfaces
            .iter()
            .filter(|d| d.prefix() != SURFACE_PREFIX)
            .count();
        r.add(IssueKind::NotSurfaceDid, None, None, bad_surfaces as u64);
        for (i, sm) in self.submeshes.iter().enumerate() {
            check_submesh(&mut r, i as u32, sm);
        }
        for (i, (buf, sm)) in self.index_buffers.iter().zip(&self.submeshes).enumerate() {
            let s = Some(i as u32);
            // u16 indices address at most 65,536 vertices; any beyond are unused anyway.
            let v = sm.vertex_count as usize;
            let addressable = v.min(1 << 16);
            let mut used = vec![false; addressable];
            let mut degenerate = 0u64;
            for t in buf.triangles() {
                if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
                    degenerate += 1;
                }
                for x in t {
                    if let Some(u) = used.get_mut(usize::from(x)) {
                        *u = true;
                    }
                }
            }
            if let (Some(&min), Some(&max)) = (buf.indices.iter().min(), buf.indices.iter().max()) {
                r.add(IssueKind::IndexMinNotZero, s, None, u64::from(min != 0));
                r.add(
                    IssueKind::IndexMaxNotLast,
                    s,
                    None,
                    u64::from(usize::from(max) + 1 != v),
                );
            }
            r.add(
                IssueKind::UnusedVertex,
                s,
                None,
                (used.iter().filter(|u| !**u).count() + (v - addressable)) as u64,
            );
            r.add(IssueKind::DegenerateTriangle, s, None, degenerate);
        }
        self.check_levels(&mut r);
        r
    }

    fn check_levels(&self, r: &mut ValidationReport) {
        let n = self.submeshes.len();
        let mut drawn = vec![0u32; n];
        let mut seen_proxy = false;
        for (li, level) in self.levels.iter().enumerate() {
            let l = Some(li as u32);
            let differ = level
                .draws
                .iter()
                .filter(|d| d.vertex_block != d.index_buffer || d.vertex_block != d.surface)
                .count();
            r.add(IssueKind::DrawIndicesDiffer, None, l, differ as u64);
            for d in &level.draws {
                if let Some(c) = drawn.get_mut(d.vertex_block as usize) {
                    *c = c.saturating_add(1);
                }
            }
            match level.kind {
                LevelKind::Detail => {
                    r.add(IssueKind::ProxyBeforeDetail, None, l, u64::from(seen_proxy));
                    r.add(
                        IssueKind::DetailWithoutCollision,
                        None,
                        l,
                        u64::from(level.collision_counts.is_none()),
                    );
                }
                LevelKind::Proxy => {
                    seen_proxy = true;
                    r.add(
                        IssueKind::ProxyWithCollision,
                        None,
                        l,
                        u64::from(level.collision_counts.is_some()),
                    );
                }
                LevelKind::Other(_) => r.add(IssueKind::UnknownLevelKind, None, l, 1),
            }
            if let Some(CollisionTree::Code(t)) = &level.tree {
                r.add(
                    IssueKind::BuildByteValue,
                    None,
                    l,
                    u64::from(t.build_byte.is_some_and(|b| b != 1)),
                );
                let bad = t
                    .origin
                    .iter()
                    .chain(std::iter::once(&t.scale))
                    .filter(|x| !x.is_finite())
                    .count();
                r.add(
                    IssueKind::NonFinite(Attribute::TreeQuantisation),
                    None,
                    l,
                    bad as u64,
                );
            }
            if let Some(cm) = &level.collision {
                let mut out_of_range = 0u64;
                for cv in &cm.vertices {
                    let ok = level
                        .draws
                        .get(usize::from(cv.draw))
                        .and_then(|d| self.submeshes.get(d.vertex_block as usize))
                        .is_some_and(|s| u32::from(cv.vertex) < s.vertex_count);
                    out_of_range += u64::from(!ok);
                }
                r.add(IssueKind::CollisionVertexOutOfRange, None, l, out_of_range);
                let unordered = cm.vertices.windows(2).filter(|w| w[0] >= w[1]).count();
                r.add(
                    IssueKind::CollisionVerticesNotAscending,
                    None,
                    l,
                    unordered as u64,
                );
                let (cv, ct, ce) = (cm.vertices.len(), cm.triangles.len(), cm.edges.len());
                let mut bad = 0u64;
                for t in &cm.triangles {
                    bad += t.vertices.iter().filter(|&&v| usize::from(v) >= cv).count() as u64;
                    bad += t.edges.iter().filter(|&&e| usize::from(e) >= ce).count() as u64;
                }
                let mut order = 0u64;
                for e in &cm.edges {
                    bad += e.vertices.iter().filter(|&&v| usize::from(v) >= cv).count() as u64;
                    let [fa, fb] = e.faces;
                    bad += u64::from(usize::from(fa) >= ct);
                    if fb != CollisionEdge::NO_FACE {
                        bad += u64::from(usize::from(fb) >= ct);
                        order += u64::from(fa >= fb);
                    }
                }
                r.add(IssueKind::CollisionIndexOutOfRange, None, l, bad);
                let overflow = [cv, ct, ce].iter().any(|&x| x > 65_536);
                r.add(
                    IssueKind::CollisionIndexOverflow,
                    None,
                    l,
                    u64::from(overflow),
                );
                if !overflow {
                    r.add(IssueKind::CollisionEdgeFaceOrder, None, l, order);
                }
            }
        }
        for (i, &c) in drawn.iter().enumerate() {
            let s = Some(i as u32);
            r.add(IssueKind::SubmeshNotDrawn, s, None, u64::from(c == 0));
            r.add(IssueKind::SubmeshDrawnTwice, s, None, u64::from(c > 1));
        }
    }
}

fn check_submesh(r: &mut ValidationReport, index: u32, sm: &Submesh) {
    let s = Some(index);
    let st = &sm.streams;
    r.add(
        IssueKind::StreamLength,
        s,
        None,
        u64::from(sm.check_streams(index).is_err()),
    );
    r.add(
        IssueKind::ZeroVertices,
        s,
        None,
        u64::from(sm.vertex_count == 0),
    );
    r.add(
        IssueKind::NonFinite(Attribute::Position),
        s,
        None,
        non_finite3(&st.positions),
    );
    let aabb_bad = sm
        .stored_aabb
        .min
        .iter()
        .chain(&sm.stored_aabb.max)
        .filter(|x| !x.is_finite())
        .count();
    r.add(
        IssueKind::NonFinite(Attribute::Aabb),
        s,
        None,
        aabb_bad as u64,
    );
    // MI4. Compared by value, so that −0 and +0 count as equal whatever order a min/max
    // implementation picks between them; observed data matches bit for bit anyway.
    let computed = crate::bounds::Aabb::from_points(&st.positions);
    let matches = match computed {
        Some(b) => {
            (0..3).all(|i| b.min[i] == sm.stored_aabb.min[i] && b.max[i] == sm.stored_aabb.max[i])
        }
        None => st.positions.is_empty(),
    };
    r.add(IssueKind::AabbMismatch, s, None, u64::from(!matches));
    if let Some(n) = &st.normals {
        r.add(
            IssueKind::NonFinite(Attribute::Normal),
            s,
            None,
            non_finite3(n),
        );
        let off = n
            .iter()
            .filter(|v| {
                let len = v.iter().map(|&c| f64::from(c).powi(2)).sum::<f64>().sqrt();
                !within(len, NORMAL_TOLERANCE)
            })
            .count();
        r.add(IssueKind::NormalNotUnit, s, None, off as u64);
    }
    if let Some(x) = &st.scalars {
        let bad = x.iter().filter(|v| !v.is_finite()).count();
        r.add(IssueKind::NonFinite(Attribute::Scalar), s, None, bad as u64);
    }
    let uv_bad: usize = st
        .uv_sets
        .iter()
        .flatten()
        .flatten()
        .filter(|v| !v.is_finite())
        .count();
    r.add(IssueKind::NonFinite(Attribute::Uv), s, None, uv_bad as u64);
    if let Some(t) = &st.tangents {
        r.add(
            IssueKind::NonFinite(Attribute::Tangent),
            s,
            None,
            non_finite3(t),
        );
    }
    if let Some(b) = &st.binormals {
        r.add(
            IssueKind::NonFinite(Attribute::Binormal),
            s,
            None,
            non_finite3(b),
        );
    }
    let mut nonzero = 0u64;
    if let Some(skin) = &st.skin {
        let (mut nf, mut sum_bad, mut neg, mut first, mut slot) = (0u64, 0u64, 0u64, 0u64, 0u64);
        // Bounded by the stored positions, not the (possibly hand-edited) vertex count.
        let vertices = (sm.vertex_count as usize).min(st.positions.len());
        for v in 0..vertices {
            let w = skin.weights_of(v);
            let j = skin.joints_of(v);
            nf += w.iter().filter(|x| !x.is_finite()).count() as u64;
            let sum: f64 = w.iter().map(|&x| f64::from(x)).sum();
            sum_bad += u64::from(!within(sum, WEIGHT_SUM_TOLERANCE));
            neg += w.iter().filter(|&&x| x < 0.0).count() as u64;
            first += u64::from(!w.first().is_some_and(|&x| x > 0.0));
            for (wi, ji) in w.iter().zip(j) {
                if *wi == 0.0 {
                    slot += u64::from(*ji != 0);
                } else {
                    nonzero += 1;
                }
            }
        }
        r.add(IssueKind::NonFinite(Attribute::Weight), s, None, nf);
        r.add(IssueKind::WeightSum, s, None, sum_bad);
        r.add(IssueKind::NegativeWeight, s, None, neg);
        r.add(IssueKind::FirstWeightNotPositive, s, None, first);
        r.add(IssueKind::UnusedSlotNonZeroIndex, s, None, slot);
    }
    let length_ok = sm.influence_list.len() as u64 == nonzero;
    r.add(
        IssueKind::InfluenceListLength,
        s,
        None,
        u64::from(!length_ok),
    );
    if length_ok && st.skin.is_some() {
        r.add(
            IssueKind::InfluenceListContent,
            s,
            None,
            u64::from(!sm.influence_list_matches()),
        );
    }
}
