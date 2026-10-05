// SPDX-License-Identifier: GPL-3.0-or-later
//! Skinned meshes: the per-vertex bone streams ([`MeshSkin`]), their
//! validation and packing, and per-frame skinning palettes ([`SkinPalette`]).
//!
//! Design (Tessera's own, `docs/specs/render.md` §2.4):
//!
//! - A skinned mesh is an ordinary [`crate::MeshData`] plus a [`MeshSkin`]
//!   side stream with exactly four influences per vertex (bone index and
//!   weight). Bone indices address the palette of the draw directly
//!   (`skeleton-anim.md` §4.1, §4.3). [`MeshSkin::palette_size`] is an
//!   upper bound checked at upload: every bone index with a non-zero weight
//!   must be below it. A draw needs a palette of at least
//!   [`SkinInfo::required_palette_len`] matrices (largest used index + 1).
//! - Weights must be finite, non-negative and sum to 1 within
//!   [`WEIGHT_SUM_TOLERANCE`]. They are renormalised and quantised to
//!   16-bit fixed point whose four values sum to exactly 65535, so the
//!   GPU-side weights sum to 1. [`MeshSkin::drawn_weights`] returns the
//!   weights the GPU uses, for CPU code that must match the drawn result.
//! - Palettes are uploaded per frame ([`crate::Frame::skin_palette`]) into
//!   one shared buffer and addressed by a dynamic offset, so a skinned draw
//!   creates no buffers or bind groups.

use bytemuck::{Pod, Zeroable};
use glam::Mat4;

use crate::error::{RenderError, Result};

/// Largest palette (bones per skinned draw). Covers every skeleton of the
/// data (`skeleton-anim.md` §4.3: at most 248 bones, largest vertex bone
/// index 247) and every `u8` bone index.
pub const MAX_PALETTE_BONES: usize = 256;

/// How far the four weights of a vertex may sum away from 1 (absolute).
/// The data sums to 1 within 2 × 10⁻⁴ (`mesh.md` §5.7); larger deviations
/// indicate a broken decoder and are rejected.
pub const WEIGHT_SUM_TOLERANCE: f32 = 1.0e-3;

/// Bytes of one palette entry on the GPU: the top three rows of the
/// skinning matrix (an affine 3 × 4 matrix, row-major).
pub(crate) const PALETTE_ENTRY_BYTES: u64 = 48;

/// Bytes the shader's palette binding covers (`MAX_PALETTE_BONES` entries).
/// Below the 16 KiB uniform binding size every wgpu adapter offers.
pub(crate) const PALETTE_BINDING_BYTES: u64 = MAX_PALETTE_BONES as u64 * PALETTE_ENTRY_BYTES;

/// Per-vertex bone influences of a skinned mesh, handed to
/// [`crate::MeshData::prepare_skinned`] or
/// [`crate::Renderer::upload_skinned_mesh`] next to the mesh streams.
///
/// Unused influence slots carry weight 0 (their bone index is ignored).
/// Sources with more than four influences keep the four largest weights,
/// renormalised (`mesh.md` §17).
///
/// [`Default`] gives empty streams and `palette_size` =
/// [`MAX_PALETTE_BONES`] (the widest valid bound), so struct-update syntax
/// (`MeshSkin { joints, weights, ..Default::default() }`) builds a skin
/// that validates.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshSkin {
    /// Four bone indices per vertex (one entry per mesh position).
    pub joints: Vec<[u8; 4]>,
    /// Four weights per vertex, summing to 1 (one entry per mesh position).
    pub weights: Vec<[f32; 4]>,
    /// Upper bound on the bone indices, checked at upload
    /// (`1..=MAX_PALETTE_BONES`): every bone index with a non-zero weight
    /// must be smaller. Typically the bone count of the skeleton the mesh
    /// is drawn with. Draws do **not** compare palettes against it: a
    /// palette of at least [`SkinInfo::required_palette_len`] matrices
    /// (largest used bone index + 1) is accepted, so the renderer cannot
    /// tell a palette of the wrong skeleton from the right one; callers
    /// that want that check compare [`SkinInfo::palette_size`] themselves.
    pub palette_size: u16,
}

impl Default for MeshSkin {
    fn default() -> Self {
        Self {
            joints: Vec::new(),
            weights: Vec::new(),
            palette_size: MAX_PALETTE_BONES as u16,
        }
    }
}

/// Summary of a validated [`MeshSkin`] (part of [`crate::MeshInfo`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkinInfo {
    /// [`MeshSkin::palette_size`] as validated.
    pub palette_size: u16,
    /// Largest bone index with a non-zero weight. A palette of at least
    /// `max_joint + 1` entries is needed to draw the mesh skinned.
    pub max_joint: u8,
    /// Largest number of non-zero weights on one vertex (1..=4).
    pub max_influences: u8,
}

impl SkinInfo {
    /// Fewest palette entries a skinned draw of the mesh needs.
    #[must_use]
    pub fn required_palette_len(&self) -> usize {
        usize::from(self.max_joint) + 1
    }
}

/// GPU skin vertex (second vertex stream; must match `SkinIn` in
/// `mesh.wgsl`). 12 bytes: `uint8x4` bone indices, `unorm16x4` weights.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct GpuSkin {
    pub joints: [u8; 4],
    pub weights: [u16; 4],
}

fn invalid(msg: String) -> RenderError {
    RenderError::InvalidSkin(msg)
}

/// Quantise four valid weights (finite, ≥ 0, positive sum) to u16 values
/// summing to exactly 65535: each weight is renormalised by the sum, scaled,
/// floored, and the remaining units go to the largest fractional parts
/// (ties to the lower slot). Deterministic.
fn quantise(w: [f32; 4], sum: f64) -> [u16; 4] {
    const FULL: u32 = u16::MAX as u32;
    let scaled = w.map(|x| f64::from(x) / sum * f64::from(FULL));
    let mut q = scaled.map(|s| s.floor().clamp(0.0, f64::from(FULL)) as u32);
    let total: u32 = q.iter().sum();
    let mut left = FULL.saturating_sub(total);
    // Order slots by fractional part, largest first (stable).
    let mut order = [0usize, 1, 2, 3];
    order.sort_by(|&a, &b| {
        let fa = scaled[a] - scaled[a].floor();
        let fb = scaled[b] - scaled[b].floor();
        fb.total_cmp(&fa)
    });
    // At most 4 units are missing (each floor loses < 1), but loop
    // defensively without assuming it.
    let mut i = 0;
    while left > 0 && i < 16 {
        let slot = order[i % 4];
        if w[slot] > 0.0 {
            q[slot] += 1;
            left -= 1;
        }
        i += 1;
    }
    q.map(|v| v.min(FULL) as u16)
}

impl MeshSkin {
    /// Validate the streams for a mesh of `vertex_count` vertices and
    /// return their summary.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidSkin`] when a stream length differs from
    /// `vertex_count`, `palette_size` is 0 or above [`MAX_PALETTE_BONES`], a
    /// weight is negative or not finite, the weights of a vertex do not sum
    /// to 1 within [`WEIGHT_SUM_TOLERANCE`], or a bone index with a non-zero
    /// weight is not below `palette_size`. The message names the vertex.
    pub fn validate(&self, vertex_count: usize) -> Result<SkinInfo> {
        self.pack(vertex_count, false).map(|(info, _)| info)
    }

    /// Validate and convert to the GPU layout (`with_data`: also build the
    /// packed stream).
    pub(crate) fn pack(
        &self,
        vertex_count: usize,
        with_data: bool,
    ) -> Result<(SkinInfo, Vec<GpuSkin>)> {
        if self.joints.len() != vertex_count || self.weights.len() != vertex_count {
            return Err(invalid(format!(
                "skin streams have {} joint and {} weight entries, the mesh has {vertex_count} \
                 vertices",
                self.joints.len(),
                self.weights.len()
            )));
        }
        let palette = usize::from(self.palette_size);
        if palette == 0 || palette > MAX_PALETTE_BONES {
            return Err(invalid(format!(
                "palette size {palette} is outside 1..={MAX_PALETTE_BONES}"
            )));
        }
        let mut out = Vec::new();
        if with_data {
            out.reserve_exact(vertex_count);
        }
        let mut max_joint = 0u8;
        let mut max_influences = 0u8;
        for (v, (j, w)) in self.joints.iter().zip(&self.weights).enumerate() {
            let mut sum = 0.0f64;
            let mut used = 0u8;
            for k in 0..4 {
                let x = w[k];
                if !x.is_finite() || x < 0.0 {
                    return Err(invalid(format!(
                        "vertex {v}: weight {k} is {x} (must be finite and >= 0)"
                    )));
                }
                if x > 0.0 {
                    if usize::from(j[k]) >= palette {
                        return Err(invalid(format!(
                            "vertex {v}: bone index {} (weight {x}) is not below the palette \
                             size {palette}",
                            j[k]
                        )));
                    }
                    max_joint = max_joint.max(j[k]);
                    used += 1;
                }
                sum += f64::from(x);
            }
            // Finite: every weight was checked above.
            if (sum - 1.0).abs() > f64::from(WEIGHT_SUM_TOLERANCE) {
                return Err(invalid(format!(
                    "vertex {v}: weights {w:?} sum to {sum} (must be 1 ± {WEIGHT_SUM_TOLERANCE})"
                )));
            }
            max_influences = max_influences.max(used);
            if with_data {
                let weights = quantise(*w, sum);
                // A slot without weight reads bone 0 (always in the palette).
                let joints = [0, 1, 2, 3].map(|k| if weights[k] > 0 { j[k] } else { 0 });
                out.push(GpuSkin { joints, weights });
            }
        }
        Ok((
            SkinInfo {
                palette_size: self.palette_size,
                max_joint,
                max_influences,
            },
            out,
        ))
    }

    /// The weights the GPU draws with, per vertex: the stored weights
    /// renormalised by their sum and quantised to 16 bits exactly as they
    /// are packed (`docs/specs/render.md` §2.4), each sum being 1 up to
    /// `f32` rounding. CPU code that must agree with the drawn positions
    /// (picking, CPU bounds, comparisons against a CPU skinner) blends with
    /// these instead of [`Self::weights`]; the two differ by up to
    /// |1 − Σw| plus the 16-bit quantisation step.
    ///
    /// # Errors
    ///
    /// As [`Self::validate`].
    pub fn drawn_weights(&self, vertex_count: usize) -> Result<Vec<[f32; 4]>> {
        let (_, packed) = self.pack(vertex_count, true)?;
        Ok(packed
            .iter()
            .map(|g| g.weights.map(|w| f32::from(w) / f32::from(u16::MAX)))
            .collect())
    }

    /// A skin binding every vertex rigidly to `bone` (weight 1), with a
    /// palette of `palette_size` entries.
    #[must_use]
    pub fn rigid(vertex_count: usize, bone: u8, palette_size: u16) -> Self {
        Self {
            joints: vec![[bone, 0, 0, 0]; vertex_count],
            weights: vec![[1.0, 0.0, 0.0, 0.0]; vertex_count],
            palette_size,
        }
    }
}

/// A skinning palette uploaded into one frame with
/// [`crate::Frame::skin_palette`]; attach it to a draw with
/// [`crate::MeshDraw::with_skin`].
///
/// Only valid in the frame that issued it: a draw in another frame, or in
/// a frame of another renderer, that references it falls back to the bind
/// pose (with a warning).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkinPalette {
    /// Id of the renderer whose frame issued the palette.
    pub(crate) owner: u64,
    /// Serial of the issuing frame (counted per renderer).
    pub(crate) frame: u64,
    /// Byte offset in the frame's palette buffer (aligned for a dynamic
    /// uniform offset).
    pub(crate) offset: u32,
    pub(crate) len: u16,
}

impl SkinPalette {
    /// Number of matrices in the palette.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    /// Whether the palette holds no matrices (never true for a palette
    /// returned by [`crate::Frame::skin_palette`]).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Per-frame palette data: rows of four floats, each palette starting at
/// an aligned offset.
#[derive(Debug, Clone, Default)]
pub(crate) struct PaletteData {
    /// Three rows per palette entry.
    pub rows: Vec<[f32; 4]>,
    /// Alignment of each palette's start, in rows.
    pub align_rows: usize,
    /// Largest number of rows the frame may hold.
    pub max_rows: usize,
    /// Palettes pushed.
    pub count: u32,
    /// First row of the last palette pushed (0 when none): the GPU buffer
    /// must hold a full binding from here.
    pub last_start: usize,
}

const IDENTITY_ROWS: [[f32; 4]; 3] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];

impl PaletteData {
    pub(crate) fn new(align_bytes: u64, max_bytes: u64) -> Self {
        let align_rows = usize::try_from(align_bytes.div_ceil(16).max(1)).unwrap_or(1);
        Self {
            rows: Vec::new(),
            align_rows,
            max_rows: usize::try_from(max_bytes / 16).unwrap_or(usize::MAX),
            count: 0,
            last_start: 0,
        }
    }

    /// Append one palette given as rows (three per entry) and return its
    /// byte offset. `entries` is the entry count (`1..=MAX_PALETTE_BONES`).
    pub(crate) fn push(
        &mut self,
        entries: usize,
        mut row: impl FnMut(usize) -> [[f32; 4]; 3],
    ) -> Result<u32> {
        if entries == 0 || entries > MAX_PALETTE_BONES {
            return Err(invalid(format!(
                "palette of {entries} matrices (must be 1..={MAX_PALETTE_BONES})"
            )));
        }
        let start = self.rows.len().div_ceil(self.align_rows) * self.align_rows;
        let end = start + entries * 3;
        // The binding always covers MAX_PALETTE_BONES entries from `start`.
        let needed = start + MAX_PALETTE_BONES * 3;
        if needed > self.max_rows {
            return Err(RenderError::LimitExceeded {
                what: "skin palette bytes per frame",
                requested: (needed as u64).saturating_mul(16),
                max: (self.max_rows as u64).saturating_mul(16),
            });
        }
        let offset = u32::try_from(start * 16).map_err(|_| RenderError::LimitExceeded {
            what: "skin palette bytes per frame",
            requested: (start as u64).saturating_mul(16),
            max: u64::from(u32::MAX),
        })?;
        self.rows.resize(start, [0.0; 4]);
        self.rows.reserve(end - start);
        for e in 0..entries {
            let mut r = row(e);
            if !r.iter().flatten().all(|v| v.is_finite()) {
                r = IDENTITY_ROWS;
            }
            self.rows.extend_from_slice(&r);
        }
        self.count = self.count.saturating_add(1);
        self.last_start = start;
        Ok(offset)
    }

    /// Rows of a column-major matrix's top three rows.
    pub(crate) fn mat4_rows(m: &Mat4) -> [[f32; 4]; 3] {
        [
            m.row(0).to_array(),
            m.row(1).to_array(),
            m.row(2).to_array(),
        ]
    }

    /// Bytes the GPU buffer needs: one full binding from the last palette
    /// start (which covers that palette's own data, at most a binding
    /// long), and never less than the data. `push` keeps this within the
    /// frame budget, so a palette it accepted never fails the upload.
    pub(crate) fn buffer_bytes(&self) -> u64 {
        let data = (self.rows.len() as u64).saturating_mul(16);
        let binding = (self.last_start as u64)
            .saturating_mul(16)
            .saturating_add(PALETTE_BINDING_BYTES);
        data.max(binding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skin(joints: Vec<[u8; 4]>, weights: Vec<[f32; 4]>, palette_size: u16) -> MeshSkin {
        MeshSkin {
            joints,
            weights,
            palette_size,
        }
    }

    #[test]
    fn gpu_skin_is_12_bytes() {
        assert_eq!(size_of::<GpuSkin>(), 12);
        assert_eq!(PALETTE_BINDING_BYTES, 12_288);
    }

    #[test]
    fn quantised_weights_sum_exactly() {
        let cases = [
            [1.0, 0.0, 0.0, 0.0],
            [0.75, 0.25, 0.0, 0.0],
            [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0, 0.0],
            [0.25, 0.25, 0.25, 0.25],
            [0.1, 0.2, 0.3, 0.4001],
            [0.5, 0.0, 0.0, 0.4999],
            [1e-9, 0.0, 0.0, 1.0],
        ];
        for w in cases {
            let sum: f64 = w.iter().map(|&x| f64::from(x)).sum();
            let q = quantise(w, sum);
            assert_eq!(q.iter().map(|&x| u32::from(x)).sum::<u32>(), 65535, "{w:?}");
            for k in 0..4 {
                if w[k] == 0.0 {
                    assert_eq!(q[k], 0, "zero weight stays zero: {w:?}");
                }
                let exact = f64::from(w[k]) / sum;
                assert!((f64::from(q[k]) / 65535.0 - exact).abs() <= 2.0 / 65535.0);
            }
        }
    }

    #[test]
    fn validation_reports_each_problem() {
        let ok = skin(vec![[3, 1, 0, 0]], vec![[0.75, 0.25, 0.0, 0.0]], 4);
        let info = ok.validate(1).expect("valid");
        assert_eq!(
            info,
            SkinInfo {
                palette_size: 4,
                max_joint: 3,
                max_influences: 2
            }
        );
        assert_eq!(info.required_palette_len(), 4);

        let err = |s: &MeshSkin, n: usize| match s.validate(n) {
            Err(RenderError::InvalidSkin(m)) => m,
            other => panic!("expected InvalidSkin, got {other:?}"),
        };
        assert!(err(&ok, 2).contains("2 vertices"));
        assert!(err(&skin(vec![[0; 4]], vec![[1.0, 0.0, 0.0, 0.0]], 0), 1).contains("palette"));
        assert!(err(&skin(vec![[0; 4]], vec![[1.0, 0.0, 0.0, 0.0]], 257), 1).contains("palette"));
        assert!(
            err(&skin(vec![[4, 0, 0, 0]], vec![[1.0, 0.0, 0.0, 0.0]], 4), 1)
                .contains("bone index 4")
        );
        assert!(err(&skin(vec![[0; 4]], vec![[f32::NAN, 1.0, 0.0, 0.0]], 4), 1).contains("finite"));
        assert!(err(&skin(vec![[0; 4]], vec![[1.5, -0.5, 0.0, 0.0]], 4), 1).contains(">= 0"));
        assert!(err(&skin(vec![[0; 4]], vec![[0.5, 0.0, 0.0, 0.0]], 4), 1).contains("sum"));
        assert!(err(&skin(vec![[0; 4]], vec![[0.0; 4]], 4), 1).contains("sum"));
        assert!(
            err(
                &skin(vec![[0; 4]], vec![[f32::INFINITY, 0.0, 0.0, 0.0]], 4),
                1
            )
            .contains("finite")
        );
    }

    #[test]
    fn zero_weight_slots_may_hold_any_index() {
        // Index 200 with weight 0 is ignored and packed as bone 0.
        let s = skin(vec![[1, 200, 0, 0]], vec![[1.0, 0.0, 0.0, 0.0]], 2);
        let (info, data) = s.pack(1, true).expect("valid");
        assert_eq!(info.max_joint, 1);
        assert_eq!(
            data,
            vec![GpuSkin {
                joints: [1, 0, 0, 0],
                weights: [65535, 0, 0, 0]
            }]
        );
    }

    #[test]
    fn weights_within_tolerance_are_renormalised() {
        let s = skin(vec![[0, 1, 0, 0]], vec![[0.5004, 0.5004, 0.0, 0.0]], 2);
        let (_, data) = s.pack(1, true).expect("within tolerance");
        assert_eq!(data[0].weights, [32768, 32767, 0, 0]);
        let s = skin(vec![[0, 1, 0, 0]], vec![[0.501, 0.501, 0.0, 0.0]], 2);
        assert!(s.validate(1).is_err(), "sum 1.002 is outside the tolerance");
    }

    #[test]
    fn default_skin_validates() {
        // Regression: the derived Default had palette_size 0, which never
        // validates.
        let d = MeshSkin::default();
        assert_eq!(usize::from(d.palette_size), MAX_PALETTE_BONES);
        assert!(d.validate(0).is_ok());
        let s = MeshSkin {
            joints: vec![[255, 0, 0, 0]],
            weights: vec![[1.0, 0.0, 0.0, 0.0]],
            ..MeshSkin::default()
        };
        assert_eq!(s.validate(1).map(|i| i.max_joint).ok(), Some(255));
    }

    #[test]
    fn drawn_weights_are_the_packed_weights() {
        // Stored weights off 1 by 4e-4: the drawn weights are renormalised.
        let s = skin(
            vec![[0, 1, 2, 0], [3, 0, 0, 0]],
            vec![[0.5002, 0.3, 0.2002, 0.0], [1.0, 0.0, 0.0, 0.0]],
            4,
        );
        let drawn = s.drawn_weights(2).expect("valid");
        let (_, packed) = s.pack(2, true).expect("valid");
        for (v, (d, w)) in drawn.iter().zip(&s.weights).enumerate() {
            let sum: f32 = d.iter().sum();
            assert!((sum - 1.0).abs() < 1e-6, "vertex {v}: {sum}");
            let wsum: f32 = w.iter().sum();
            for k in 0..4 {
                assert_eq!(d[k], f32::from(packed[v].weights[k]) / 65535.0);
                assert!((d[k] - w[k] / wsum).abs() <= 1.0 / 65535.0);
            }
        }
        assert_eq!(drawn[1], [1.0, 0.0, 0.0, 0.0]);
        assert!(s.drawn_weights(3).is_err());
    }

    #[test]
    fn rigid_skin_validates() {
        let s = MeshSkin::rigid(3, 5, 6);
        assert_eq!(s.validate(3).map(|i| i.max_joint).ok(), Some(5));
        assert!(MeshSkin::rigid(3, 6, 6).validate(3).is_err());
    }

    #[test]
    fn palette_data_aligns_and_sanitises() {
        let mut p = PaletteData::new(256, 1 << 20);
        assert_eq!(p.align_rows, 16);
        let a = p.push(2, |_| IDENTITY_ROWS).expect("a");
        let b = p
            .push(1, |_| [[f32::NAN, 0.0, 0.0, 0.0], [0.0; 4], [0.0; 4]])
            .expect("b");
        assert_eq!((a, b), (0, 256));
        assert_eq!(p.rows.len(), 16 + 3);
        assert_eq!(
            &p.rows[16..19],
            &IDENTITY_ROWS,
            "non-finite matrix → identity"
        );
        assert!(p.push(0, |_| IDENTITY_ROWS).is_err());
        assert!(p.push(257, |_| IDENTITY_ROWS).is_err());
        assert_eq!(p.buffer_bytes(), 16 * 16 + PALETTE_BINDING_BYTES);
        // A frame cap that leaves no room for a full binding.
        let mut small = PaletteData::new(256, PALETTE_BINDING_BYTES);
        assert!(small.push(1, |_| IDENTITY_ROWS).is_ok());
        assert!(matches!(
            small.push(1, |_| IDENTITY_ROWS),
            Err(RenderError::LimitExceeded { .. })
        ));
    }

    #[test]
    fn buffer_bytes_stay_within_the_budget() {
        // Regression: `push` checked `start + binding` against the budget
        // while `buffer_bytes` added a binding past the end of the data, so
        // a frame of full palettes could ask for up to 12 KiB more than the
        // budget (and the device's buffer limit) allowed.
        let budget = 1u64 << 20;
        let mut p = PaletteData::new(256, budget);
        assert_eq!(p.buffer_bytes(), PALETTE_BINDING_BYTES, "empty frame");
        let mut accepted = 0;
        while p.push(MAX_PALETTE_BONES, |_| IDENTITY_ROWS).is_ok() {
            accepted += 1;
            assert!(
                p.buffer_bytes() <= budget,
                "{} > {budget}",
                p.buffer_bytes()
            );
            assert!(p.buffer_bytes() >= p.rows.len() as u64 * 16);
        }
        assert_eq!(accepted, 85);
        assert_eq!(
            p.buffer_bytes(),
            p.last_start as u64 * 16 + PALETTE_BINDING_BYTES
        );
        // Short palettes: the last binding reaches past the data.
        let mut q = PaletteData::new(256, budget);
        q.push(1, |_| IDENTITY_ROWS).expect("one");
        q.push(1, |_| IDENTITY_ROWS).expect("two");
        assert_eq!(q.buffer_bytes(), 256 + PALETTE_BINDING_BYTES);
    }

    #[test]
    fn mat4_rows_are_the_affine_rows() {
        let m = Mat4::from_translation(glam::Vec3::new(1.0, 2.0, 3.0));
        let r = PaletteData::mat4_rows(&m);
        assert_eq!(r[0], [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(r[1], [0.0, 1.0, 0.0, 2.0]);
        assert_eq!(r[2], [0.0, 0.0, 1.0, 3.0]);
    }
}
