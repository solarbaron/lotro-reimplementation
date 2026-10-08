// SPDX-License-Identifier: GPL-3.0-or-later
//! Texture-coordinate transforms (`material.md` §7.3, §8.4; `lighting.md` §4.1, §5.2.1).
//!
//! A shader record binds float parameters to the seven values of a **texture matrix**
//! with binding semantics 38–44; the binding's `slot` is the matrix index (§7.3, Tessera
//! Decision: semantics 33–37 of older bindings are ignored). [`TextureMatrixLayout`] is
//! that binding table for one shader record. Evaluated over an effective parameter set it
//! gives one [`TextureTransform`] per matrix, which composes to the affine
//! [`TextureMatrix`] the programs apply to (u, v):
//!
//! 1. early translation: u₁ = u + *e*ᵤ, v₁ = v + *e*ᵥ;
//! 2. scale: u₂ = *s*ᵤ · u₁, v₂ = *s*ᵥ · v₁;
//! 3. counter-clockwise rotation by θ = rotation · π / 180;
//! 4. translation: u′ = u₃ + *t*ᵤ, v′ = v₃ + *t*ᵥ.
//!
//! A matrix value that no binding sets keeps the client's reset value, 0 (§8.4), so an
//! unbound scale collapses the coordinates; a matrix index with no binding at all is not
//! listed, and the programs then use (u, v) unchanged (identity, `lighting.md` §5.2.1).
//!
//! ```
//! use tessera_material::transform::{TextureMatrix, TextureTransform};
//!
//! // Rotate a texture by 90° about its centre (early −0.5, translation +0.5, §8.4).
//! let t = TextureTransform {
//!     early: [-0.5, -0.5],
//!     scale: [1.0, 1.0],
//!     rotation_degrees: 90.0,
//!     translate: [0.5, 0.5],
//! };
//! let (u, v) = t.matrix().apply(1.0, 0.5);
//! assert!((u - 0.5).abs() < 1e-6 && (v - 1.0).abs() < 1e-6);
//! assert_eq!(TextureMatrix::IDENTITY.apply(0.25, 0.75), (0.25, 0.75));
//! ```

use std::collections::{BTreeMap, HashMap};
use std::mem::size_of;

use crate::records::{CurveStates, ShaderRecord};
use crate::resolve::ParameterSet;

/// Highest texture-matrix index + 1 that a binding may name. The `slot` of non-texture
/// bindings is 0–7 in observed data (§7.3) and the programs use matrices 0–3
/// (`lighting.md` §4.2 E14); bindings naming a higher index are ignored and counted.
pub const MAX_TEXTURE_MATRICES: u32 = 8;

/// The seven values of a texture matrix that bindings set (§7.3 semantics 38–44).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TransformValue {
    /// Semantic 38: translation U (UTranslate…).
    TranslateU,
    /// Semantic 39: translation V (VTranslate…).
    TranslateV,
    /// Semantic 40: rotation in degrees, counter-clockwise (UVRotate…).
    Rotate,
    /// Semantic 41: scale U (UScale…).
    ScaleU,
    /// Semantic 42: scale V (VScale…).
    ScaleV,
    /// Semantic 43: early translation U (UTranslateEarly…).
    EarlyU,
    /// Semantic 44: early translation V (VTranslateEarly…).
    EarlyV,
}

impl TransformValue {
    /// All seven, in semantic order 38–44.
    pub const ALL: [Self; 7] = [
        Self::TranslateU,
        Self::TranslateV,
        Self::Rotate,
        Self::ScaleU,
        Self::ScaleV,
        Self::EarlyU,
        Self::EarlyV,
    ];

    /// The value a binding `semantic` writes (§7.3): 38–44. The older semantics 33–37
    /// give `None` (Tessera Decision: ignored), as does every other semantic.
    #[must_use]
    pub fn from_semantic(semantic: u16) -> Option<Self> {
        let i = usize::from(semantic.checked_sub(38)?);
        Self::ALL.get(i).copied()
    }

    /// The binding semantic of this value (38–44).
    #[must_use]
    pub fn semantic(self) -> u16 {
        38 + self as u16
    }

    /// Position in [`TransformValue::ALL`] and in [`MatrixBinding::params`].
    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }
}

/// Whether `semantic` is one of the older transform semantics 33–37, which Tessera
/// ignores (§7.3 Decision).
#[must_use]
pub fn is_older_transform_semantic(semantic: u16) -> bool {
    (33..=37).contains(&semantic)
}

/// The values of one texture matrix, before composition (§8.4). Each is the effective
/// float curve of the bound parameter, evaluated at the clock of §5.3.1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextureTransform {
    /// Early translation (U, V), added before scaling.
    pub early: [f32; 2],
    /// Scale (U, V).
    pub scale: [f32; 2],
    /// Rotation in degrees, counter-clockwise in the (u, v) plane.
    pub rotation_degrees: f32,
    /// Translation (U, V), added last.
    pub translate: [f32; 2],
}

impl TextureTransform {
    /// The client's reset value of every matrix value: 0 (§8.4). It collapses every
    /// coordinate to the translation.
    pub const RESET: Self = Self {
        early: [0.0; 2],
        scale: [0.0; 2],
        rotation_degrees: 0.0,
        translate: [0.0; 2],
    };

    /// The transform that leaves coordinates unchanged (scale 1, everything else 0).
    pub const IDENTITY: Self = Self {
        early: [0.0; 2],
        scale: [1.0; 2],
        rotation_degrees: 0.0,
        translate: [0.0; 2],
    };

    /// Sets one of the seven values.
    pub fn set(&mut self, value: TransformValue, v: f32) {
        match value {
            TransformValue::TranslateU => self.translate[0] = v,
            TransformValue::TranslateV => self.translate[1] = v,
            TransformValue::Rotate => self.rotation_degrees = v,
            TransformValue::ScaleU => self.scale[0] = v,
            TransformValue::ScaleV => self.scale[1] = v,
            TransformValue::EarlyU => self.early[0] = v,
            TransformValue::EarlyV => self.early[1] = v,
        }
    }

    /// One of the seven values.
    #[must_use]
    pub fn get(&self, value: TransformValue) -> f32 {
        match value {
            TransformValue::TranslateU => self.translate[0],
            TransformValue::TranslateV => self.translate[1],
            TransformValue::Rotate => self.rotation_degrees,
            TransformValue::ScaleU => self.scale[0],
            TransformValue::ScaleV => self.scale[1],
            TransformValue::EarlyU => self.early[0],
            TransformValue::EarlyV => self.early[1],
        }
    }

    /// The composed affine map (§8.4): early translation, scale, counter-clockwise
    /// rotation, translation. Computed in `f64`; every element of the result is finite
    /// (with hostile values only: a non-finite rotation counts as 0°, and a non-finite
    /// element becomes 0 or the largest finite `f32` of its sign, as for
    /// [`crate::records::FloatCurve::evaluate_finite`]).
    #[must_use]
    pub fn matrix(&self) -> TextureMatrix {
        let [eu, ev] = self.early.map(f64::from);
        let [su, sv] = self.scale.map(f64::from);
        let [tu, tv] = self.translate.map(f64::from);
        let theta = f64::from(self.rotation_degrees).to_radians();
        let (sin, cos) = if theta.is_finite() {
            theta.sin_cos()
        } else {
            (0.0, 1.0)
        };
        // u′ = cos θ · sᵤ · (u + eᵤ) − sin θ · sᵥ · (v + eᵥ) + tᵤ
        // v′ = sin θ · sᵤ · (u + eᵤ) + cos θ · sᵥ · (v + eᵥ) + tᵥ
        let m00 = cos * su;
        let m01 = -sin * sv;
        let m10 = sin * su;
        let m11 = cos * sv;
        let m03 = m00 * eu + m01 * ev + tu;
        let m13 = m10 * eu + m11 * ev + tv;
        let f = |x: f64| crate::records::curve_finite(x as f32);
        TextureMatrix {
            rows: [[f(m00), f(m01), f(m03)], [f(m10), f(m11), f(m13)]],
        }
    }
}

impl Default for TextureTransform {
    fn default() -> Self {
        Self::RESET
    }
}

/// An affine 2-D texture-coordinate map: u′ = m00·u + m01·v + m03 and
/// v′ = m10·u + m11·v + m13 (`lighting.md` §5.2.1). `rows[0]` is (m00, m01, m03) and
/// `rows[1]` is (m10, m11, m13).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextureMatrix {
    /// The two rows: linear part, then the translation term.
    pub rows: [[f32; 3]; 2],
}

impl TextureMatrix {
    /// The identity map: what a permutation without texture transforms uses.
    pub const IDENTITY: Self = Self {
        rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
    };

    /// Applies the map to (u, v).
    #[must_use]
    pub fn apply(&self, u: f32, v: f32) -> (f32, f32) {
        let [a, b] = self.rows;
        (a[0] * u + a[1] * v + a[2], b[0] * u + b[1] * v + b[2])
    }

    /// `self` followed by `next` (the client appends a transform to whatever the matrix
    /// holds, §8.4): the result maps (u, v) to `next.apply(self.apply(u, v))`.
    ///
    /// Computed in `f64`; like [`TextureTransform::matrix`], every element of the result
    /// is finite (a NaN element becomes 0 and an infinite one the largest finite `f32` of
    /// its sign), so composing two finite matrices whose product overflows still gives a
    /// finite map.
    #[must_use]
    pub fn then(&self, next: &Self) -> Self {
        let [a, b] = self.rows.map(|r| r.map(f64::from));
        let [c, d] = next.rows.map(|r| r.map(f64::from));
        let f = |x: f64| crate::records::curve_finite(x as f32);
        Self {
            rows: [
                [
                    f(c[0] * a[0] + c[1] * b[0]),
                    f(c[0] * a[1] + c[1] * b[1]),
                    f(c[0] * a[2] + c[1] * b[2] + c[2]),
                ],
                [
                    f(d[0] * a[0] + d[1] * b[0]),
                    f(d[0] * a[1] + d[1] * b[1]),
                    f(d[0] * a[2] + d[1] * b[2] + d[2]),
                ],
            ],
        }
    }

    /// The 4 × 4 form the programs read (row-major; `lighting.md` §5.2.1): the first two
    /// rows hold the map with its translation in the fourth column, the other two rows
    /// are those of the identity.
    #[must_use]
    pub fn to_mat4_rows(&self) -> [[f32; 4]; 4] {
        let [a, b] = self.rows;
        [
            [a[0], a[1], 0.0, a[2]],
            [b[0], b[1], 0.0, b[2]],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }
}

impl Default for TextureMatrix {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// The parameters bound to the seven values of one texture matrix (§7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MatrixBinding {
    /// The texture-matrix index (the binding `slot`), below [`MAX_TEXTURE_MATRICES`].
    pub matrix: u32,
    /// The parameter id bound to each value, indexed by [`TransformValue::index`]; `None`
    /// keeps the reset value 0.
    pub params: [Option<u32>; 7],
}

impl MatrixBinding {
    /// The parameter bound to `value`.
    #[must_use]
    pub fn param(&self, value: TransformValue) -> Option<u32> {
        self.params[value.index()]
    }

    /// Whether all seven values are bound (true of every matrix in observed data, §7.3).
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.params.iter().all(Option::is_some)
    }
}

/// One evaluated texture matrix of a surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvaluatedMatrix {
    /// The texture-matrix index.
    pub matrix: u32,
    /// The seven values.
    pub transform: TextureTransform,
    /// Their composition.
    pub map: TextureMatrix,
}

/// The texture-matrix bindings of one shader record (§7.3 semantics 38–44).
///
/// The matrix index of a binding (its `slot`) belongs to one permutation (§7.3), so the
/// layout keeps one table per permutation ([`TextureMatrixLayout::for_permutation`],
/// [`TextureMatrixLayout::evaluate_for`]). [`TextureMatrixLayout::matrices`] is a single
/// record-wide table merged over all permutations, for a caller that does not track
/// permutations (the colour passes in observed data; see
/// [`TextureMatrixLayout::from_record`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextureMatrixLayout {
    /// The record-wide table: bound matrices by ascending index, each value bound to the
    /// parameter that the most bindings name (see [`TextureMatrixLayout::from_record`]).
    pub matrices: Vec<MatrixBinding>,
    /// Bindings with the older semantics 33–37, ignored (§7.3 Decision).
    pub older_bindings: usize,
    /// Bindings naming a matrix index at or above [`MAX_TEXTURE_MATRICES`], ignored.
    pub out_of_range: usize,
    /// (matrix, value) pairs of the record-wide table that several parameters are bound
    /// to (in different permutations); see [`TextureMatrixLayout::from_record`] for the
    /// one kept.
    pub conflicts: usize,
    /// (permutation, matrix, value) triples that several parameters are bound to within
    /// one permutation (never observed); the one declared first is kept.
    pub permutation_conflicts: usize,
    /// The distinct per-permutation tables, each by ascending matrix index.
    tables: Vec<Vec<MatrixBinding>>,
    /// (permutation index, index into `tables`) for every permutation that binds a
    /// texture matrix, by ascending permutation index.
    permutations: Vec<(u32, usize)>,
}

/// Per (matrix, value index, parameter id): how many bindings name it and the stored
/// position of the first one.
type CandidateCounts = BTreeMap<(u32, usize, u32), (usize, usize)>;

/// The parameter kept for one (matrix, value) of the record-wide table.
struct Winner {
    id: u32,
    count: usize,
    first: usize,
    /// How many distinct parameters are bound to that (matrix, value).
    candidates: usize,
}

impl TextureMatrixLayout {
    /// Collects the transform bindings of `record`'s declarations.
    ///
    /// Per permutation, each (matrix, value) takes the parameter of the first binding in
    /// stored order that names it (a second one is counted in
    /// [`TextureMatrixLayout::permutation_conflicts`]).
    ///
    /// The record-wide table [`TextureMatrixLayout::matrices`] merges all permutations.
    /// In a few records a glow-pass permutation binds the second set's parameters to its
    /// matrix 0 while the colour passes bind the first set there (four records on the
    /// reference install, `material.md` §18). When several parameters are bound to the
    /// same (matrix, value), the one bound by the most bindings (permutations) is kept,
    /// ties to the one met first in stored order, and the pair is counted in
    /// [`TextureMatrixLayout::conflicts`].
    ///
    /// Runs in O(*B* log *B*) for *B* bindings.
    #[must_use]
    pub fn from_record(record: &ShaderRecord) -> Self {
        let mut out = Self::default();
        let mut counts: CandidateCounts = BTreeMap::new();
        let mut per_permutation: BTreeMap<u32, BTreeMap<u32, [Option<u32>; 7]>> = BTreeMap::new();
        let mut position = 0usize;
        for entry in &record.params {
            for b in &entry.bindings {
                if is_older_transform_semantic(b.semantic) {
                    out.older_bindings += 1;
                    continue;
                }
                let Some(value) = TransformValue::from_semantic(b.semantic) else {
                    continue;
                };
                if b.slot >= MAX_TEXTURE_MATRICES {
                    out.out_of_range += 1;
                    continue;
                }
                let c = counts
                    .entry((b.slot, value.index(), entry.id))
                    .or_insert((0, position));
                c.0 += 1;
                position += 1;

                let params = per_permutation
                    .entry(b.permutation)
                    .or_default()
                    .entry(b.slot)
                    .or_insert([None; 7]);
                match params[value.index()] {
                    None => params[value.index()] = Some(entry.id),
                    Some(id) if id != entry.id => out.permutation_conflicts += 1,
                    Some(_) => {}
                }
            }
        }

        // The record-wide table: per (matrix, value), the highest count, ties to the
        // earliest first binding.
        let mut best: BTreeMap<(u32, usize), Winner> = BTreeMap::new();
        for (&(matrix, value, id), &(count, first)) in &counts {
            let w = best.entry((matrix, value)).or_insert(Winner {
                id,
                count,
                first,
                candidates: 0,
            });
            w.candidates += 1;
            if count > w.count || (count == w.count && first < w.first) {
                (w.id, w.count, w.first) = (id, count, first);
            }
        }
        let mut by_index: BTreeMap<u32, [Option<u32>; 7]> = BTreeMap::new();
        for ((matrix, value), w) in best {
            if w.candidates > 1 {
                out.conflicts += 1;
            }
            if let Some(slot) = by_index.entry(matrix).or_insert([None; 7]).get_mut(value) {
                *slot = Some(w.id);
            }
        }
        out.matrices = by_index
            .into_iter()
            .map(|(matrix, params)| MatrixBinding { matrix, params })
            .collect();

        // Per-permutation tables, deduplicated (most permutations of a record share one).
        let mut seen: HashMap<Vec<MatrixBinding>, usize> = HashMap::new();
        for (permutation, matrices) in per_permutation {
            let table: Vec<MatrixBinding> = matrices
                .into_iter()
                .map(|(matrix, params)| MatrixBinding { matrix, params })
                .collect();
            let index = match seen.get(&table) {
                Some(&i) => i,
                None => {
                    let i = out.tables.len();
                    out.tables.push(table.clone());
                    seen.insert(table, i);
                    i
                }
            };
            out.permutations.push((permutation, index));
        }
        out
    }

    /// The binding of matrix `index` in the record-wide table, if any value of it is
    /// bound.
    #[must_use]
    pub fn matrix(&self, index: u32) -> Option<&MatrixBinding> {
        self.matrices.iter().find(|m| m.matrix == index)
    }

    /// Whether the record binds no texture matrix.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.matrices.is_empty()
    }

    /// The matrices that permutation `permutation` binds, by ascending index (§7.3: the
    /// binding `slot` is that permutation's matrix index). Empty when the permutation
    /// binds none: its programs then use (u, v) unchanged (`lighting.md` §5.2.1).
    #[must_use]
    pub fn for_permutation(&self, permutation: u32) -> &[MatrixBinding] {
        self.permutations
            .binary_search_by_key(&permutation, |&(p, _)| p)
            .ok()
            .and_then(|i| self.permutations.get(i))
            .and_then(|&(_, t)| self.tables.get(t))
            .map_or(&[], Vec::as_slice)
    }

    /// Every permutation that binds a texture matrix, by ascending index, with its table.
    pub fn permutations(&self) -> impl Iterator<Item = (u32, &[MatrixBinding])> + '_ {
        self.permutations.iter().map(|&(p, t)| {
            (
                p,
                self.tables
                    .get(t)
                    .map_or(&[] as &[MatrixBinding], Vec::as_slice),
            )
        })
    }

    /// The distinct per-permutation tables (most records have one).
    #[must_use]
    pub fn permutation_tables(&self) -> &[Vec<MatrixBinding>] {
        &self.tables
    }

    /// Whether permutation `permutation` binds some matrix value to another parameter
    /// than the record-wide table does (or binds a matrix value that table leaves
    /// unbound). Matrices the permutation does not bind are not compared.
    #[must_use]
    pub fn differs_from_merged(&self, permutation: u32) -> bool {
        self.for_permutation(permutation).iter().any(|m| {
            let merged = self.matrix(m.matrix);
            m.params
                .iter()
                .enumerate()
                .any(|(i, p)| p.is_some() && merged.and_then(|x| x.params[i]) != *p)
        })
    }

    /// Every bound matrix of the record-wide table evaluated over `params` at time `t`
    /// seconds, with the curve states of the drawn instance (§8.4, §5.3.1). A bound
    /// parameter that is not a declared float (never observed) keeps the reset value 0,
    /// like an unbound value.
    pub fn evaluate(
        &self,
        params: &ParameterSet,
        t: f64,
        states: &mut CurveStates,
    ) -> Vec<EvaluatedMatrix> {
        evaluate_bindings(&self.matrices, params, t, states)
    }

    /// The matrices of permutation `permutation` ([`TextureMatrixLayout::for_permutation`])
    /// evaluated as in [`TextureMatrixLayout::evaluate`]. Empty when the permutation binds
    /// none (identity).
    pub fn evaluate_for(
        &self,
        permutation: u32,
        params: &ParameterSet,
        t: f64,
        states: &mut CurveStates,
    ) -> Vec<EvaluatedMatrix> {
        evaluate_bindings(self.for_permutation(permutation), params, t, states)
    }

    /// Approximate heap bytes retained, for cache budgets.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        let tables: usize = self
            .tables
            .iter()
            .map(|t| size_of::<Vec<MatrixBinding>>() + t.capacity() * size_of::<MatrixBinding>())
            .sum();
        self.matrices.capacity() * size_of::<MatrixBinding>()
            + tables
            + self.permutations.capacity() * size_of::<(u32, usize)>()
    }
}

/// Evaluates `matrices` over `params` at `t` (§8.4): unbound values keep the reset value.
fn evaluate_bindings(
    matrices: &[MatrixBinding],
    params: &ParameterSet,
    t: f64,
    states: &mut CurveStates,
) -> Vec<EvaluatedMatrix> {
    matrices
        .iter()
        .map(|m| {
            let mut transform = TextureTransform::RESET;
            for value in TransformValue::ALL {
                if let Some(v) = m.param(value).and_then(|id| params.float_at(id, t, states)) {
                    transform.set(value, v);
                }
            }
            EvaluatedMatrix {
                matrix: m.matrix,
                transform,
                map: transform.matrix(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-5 && (a.1 - b.1).abs() < 1e-5
    }

    #[test]
    fn semantics() {
        assert_eq!(
            TransformValue::from_semantic(38),
            Some(TransformValue::TranslateU)
        );
        assert_eq!(
            TransformValue::from_semantic(44),
            Some(TransformValue::EarlyV)
        );
        assert_eq!(TransformValue::from_semantic(45), None);
        assert_eq!(TransformValue::from_semantic(37), None);
        assert_eq!(TransformValue::from_semantic(0), None);
        for v in TransformValue::ALL {
            assert_eq!(TransformValue::from_semantic(v.semantic()), Some(v));
        }
        assert!(is_older_transform_semantic(33) && is_older_transform_semantic(37));
        assert!(!is_older_transform_semantic(38));
    }

    #[test]
    fn composition_order() {
        // §8.4: early translation, scale, rotation, translation.
        let t = TextureTransform {
            early: [1.0, 2.0],
            scale: [2.0, 3.0],
            rotation_degrees: 90.0,
            translate: [10.0, 20.0],
        };
        // (0, 0) → (1, 2) → (2, 6) → rotated CCW 90°: (−6, 2) → (4, 22).
        assert!(close(t.matrix().apply(0.0, 0.0), (4.0, 22.0)));
        // The single affine form of §8.4.
        let m = t.matrix();
        let (s, c) = 90f32.to_radians().sin_cos();
        assert!((m.rows[0][0] - c * 2.0).abs() < 1e-6);
        assert!((m.rows[0][1] + s * 3.0).abs() < 1e-6);
        assert!((m.rows[1][0] - s * 2.0).abs() < 1e-6);
        assert!((m.rows[1][1] - c * 3.0).abs() < 1e-6);
    }

    #[test]
    fn reset_collapses_and_identity_keeps() {
        assert_eq!(TextureTransform::RESET.matrix().apply(0.3, 0.7), (0.0, 0.0));
        assert_eq!(TextureTransform::IDENTITY.matrix(), TextureMatrix::IDENTITY);
        assert_eq!(TextureTransform::default(), TextureTransform::RESET);
        assert_eq!(TextureMatrix::default(), TextureMatrix::IDENTITY);
    }

    #[test]
    fn rotation_about_the_centre() {
        // §8.4: early −0.5 with translation +0.5 rotates about (0.5, 0.5).
        let t = TextureTransform {
            early: [-0.5, -0.5],
            scale: [1.0, 1.0],
            rotation_degrees: 180.0,
            translate: [0.5, 0.5],
        };
        assert!(close(t.matrix().apply(0.5, 0.5), (0.5, 0.5)));
        assert!(close(t.matrix().apply(0.0, 0.0), (1.0, 1.0)));
    }

    #[test]
    fn then_and_mat4() {
        let a = TextureTransform {
            scale: [2.0, 2.0],
            ..TextureTransform::IDENTITY
        }
        .matrix();
        let b = TextureTransform {
            translate: [1.0, -1.0],
            ..TextureTransform::IDENTITY
        }
        .matrix();
        let ab = a.then(&b);
        assert!(close(ab.apply(1.0, 1.0), (3.0, 1.0)));
        let m4 = ab.to_mat4_rows();
        assert_eq!(m4[0], [2.0, 0.0, 0.0, 1.0]);
        assert_eq!(m4[1], [0.0, 2.0, 0.0, -1.0]);
        assert_eq!(TextureMatrix::IDENTITY.then(&ab), ab);
    }

    fn decl(id: u32, bindings: &[(u16, u32, u32)]) -> crate::records::ParamEntry {
        crate::records::ParamEntry {
            id,
            group: 1000,
            value: crate::records::ParamValue::Float(crate::records::FloatCurve::constant(1.0)),
            flag: 0,
            bindings: bindings
                .iter()
                .map(|&(semantic, permutation, slot)| crate::records::Binding {
                    semantic,
                    group: 1000,
                    permutation,
                    slot,
                })
                .collect(),
        }
    }

    #[test]
    fn majority_of_bindings_decides_a_conflict() {
        // A glow-like permutation (2) binds the second set to matrix 0; the two colour
        // permutations bind the first set there. Declared second-set first.
        let record = ShaderRecord {
            did: tessera_formats::Did(0x2B00_0001),
            reserved: 0,
            params: vec![
                decl(1003, &[(41, 2, 0), (41, 0, 1), (41, 1, 1)]),
                decl(1001, &[(41, 0, 0), (41, 1, 0)]),
                // A tie: one binding each, the one met first is kept.
                decl(1035, &[(40, 0, 3)]),
                decl(1034, &[(40, 1, 3)]),
                // Older semantic, and an index out of range.
                decl(1017, &[(33, 0, 0), (38, 0, MAX_TEXTURE_MATRICES)]),
            ],
            permutations: Vec::new(),
        };
        let layout = TextureMatrixLayout::from_record(&record);
        assert_eq!(
            layout.matrix(0).unwrap().param(TransformValue::ScaleU),
            Some(1001)
        );
        assert_eq!(
            layout.matrix(1).unwrap().param(TransformValue::ScaleU),
            Some(1003)
        );
        assert_eq!(
            layout.matrix(3).unwrap().param(TransformValue::Rotate),
            Some(1035)
        );
        assert_eq!(layout.conflicts, 2);
        assert_eq!(layout.older_bindings, 1);
        assert_eq!(layout.out_of_range, 1);
        assert_eq!(layout.matrices.len(), 3);
        assert!(!layout.is_empty());
        assert!(layout.heap_bytes() >= 3 * size_of::<MatrixBinding>());
    }

    #[test]
    fn then_stays_finite_when_the_product_overflows() {
        // Review regression: two finite matrices whose product overflows f32.
        let m = TextureTransform {
            scale: [f32::MAX; 2],
            ..TextureTransform::RESET
        }
        .matrix();
        let mm = m.then(&m);
        assert!(mm.rows.iter().flatten().all(|x| x.is_finite()), "{mm:?}");
        assert_eq!(mm.rows[0][0], f32::MAX);
        let nan = TextureMatrix {
            rows: [
                [f32::NAN, f32::INFINITY, 0.0],
                [0.0, f32::NEG_INFINITY, 1.0],
            ],
        };
        assert!(
            nan.then(&nan).rows.iter().flatten().all(|x| x.is_finite()),
            "{:?}",
            nan.then(&nan)
        );
    }

    #[test]
    fn many_candidates_on_one_value_build_in_n_log_n() {
        // Review regression: a hostile record binding 100,000 distinct parameters to one
        // (matrix, value) used to take quadratic time (minutes; now milliseconds).
        const N: u32 = 100_000;
        let mut params: Vec<_> = (0..N).map(|i| decl(10_000 + i, &[(38, 0, 0)])).collect();
        // The last one is bound twice (two permutations): it wins the merged table.
        params.push(decl(5, &[(38, 1, 0), (38, 2, 0)]));
        let record = ShaderRecord {
            did: tessera_formats::Did(0x2B00_0001),
            reserved: 0,
            params,
            permutations: Vec::new(),
        };
        let start = std::time::Instant::now();
        let layout = TextureMatrixLayout::from_record(&record);
        let elapsed = start.elapsed();
        assert!(elapsed.as_secs() < 10, "{elapsed:?}");
        assert_eq!(layout.conflicts, 1);
        assert_eq!(
            layout.matrix(0).unwrap().param(TransformValue::TranslateU),
            Some(5)
        );
        // Permutation 0 keeps the first declared; the others conflict within it.
        assert_eq!(layout.permutation_conflicts, N as usize - 1);
        assert_eq!(
            layout.for_permutation(0)[0].param(TransformValue::TranslateU),
            Some(10_000)
        );
        assert_eq!(layout.permutation_tables().len(), 2);
        assert_eq!(layout.for_permutation(1), layout.for_permutation(2));
    }

    #[test]
    fn hostile_values_stay_finite() {
        for v in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::MAX,
            -f32::MAX,
        ] {
            let t = TextureTransform {
                early: [v, v],
                scale: [v, f32::MAX],
                rotation_degrees: v,
                translate: [v, 1.0],
            };
            let m = t.matrix();
            assert!(m.rows.iter().flatten().all(|x| x.is_finite()), "{v} {m:?}");
        }
    }
}
