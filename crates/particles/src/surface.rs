// SPDX-License-Identifier: GPL-3.0-or-later
//! What the programs of a particle surface compute (`particle-runtime.md` §9.1.1).
//!
//! The 19 shader records that particle surfaces use are all unlit and draw in one pass.
//! Their programs differ in four ways the draw has to follow:
//!
//! - **output alpha**: texture opacity × vertex alpha × material alpha in most records; the
//!   texture's alpha left out (0x2B0007F2, 0x2B000726); no vertex colour (0x2B000755);
//! - **doubling**: 2 × *T* × *V* × *M* in four records;
//! - **fog**: the alpha-blended records are fogged, the additive ones are not;
//! - **layers**: two-layer records multiply both layers' alphas, and multiply or add their
//!   colours.
//!
//! The alpha test reads the texture's opacity alone (two layers: the product), never the
//! vertex or material alpha, and three records test nothing. Two-layer records read the
//! first layer at texture matrix 0 and the second at texture matrix 1 (`material.md` §8.4,
//! RT27); one-layer records apply no matrix.
//!
//! Variants (§9.1.2, Tessera rule 11): Tessera draws the permutations that exclude
//! feature bit 1, as the client does with `Render.VideoPostEffects` off: no soft-particle
//! factor, and 0x2B000084 (heat distortion) is drawn as its variant without the
//! frame-buffer input, which leaves the target unchanged, that is not at all
//! ([`SurfaceProgram::distortion`]).
//!
//! Opacity (§9.1.3): particle draws take their alpha test and the source alpha of their
//! blend from these programs, not from the mesh opacity-source rule. The test is on when
//! the material's effective 4004 is on and the program tests the texture, whatever the
//! shader's slot roles say ([`ParticleSurface::alpha_test`]); its reference is the
//! effective 1043, evaluated at the draw's clock when the curve is not constant
//! ([`ParticleSurface::at`]).
//!
//! Curve states (§9.1.1 answer of 2026-10-10, rule 12): the caller keeps one
//! [`CurveStates`] per surface record DID, shared by every 0x39 record and instance that
//! draws that surface, for as long as the surface stays loaded (a preview: for its run).
//! Mode 10 keeps its start time and last value there; mode 3 keeps no state: at every
//! evaluation it takes its *r* as one real-range draw over [−1, 1] of the shared particle
//! stream ([`Ran2`], rules 1, 9 and 12; RT33).
//!
//! [`SurfaceProgram::of`] gives the row of a known record; [`SurfaceProgram::fallback`]
//! is rule 10 for any other shader record (none in the data): the common row of its blend
//! class. [`ParticleSurface::of`] combines the row with a resolved surface material: the
//! material colour 2000 (*M* before the draw's tint, `material.md` §8.3), the second
//! layer's texture, the alpha test and the texture matrices.
//!
//! ```
//! use tessera_particles::surface::{LayerCombine, SurfaceBlend, SurfaceProgram};
//!
//! let p = SurfaceProgram::of(0x2B00_07EB).unwrap();
//! assert!(p.doubled && p.fogged && p.texture_alpha);
//! assert_eq!(p.blend, SurfaceBlend::Alpha);
//! let add = SurfaceProgram::of(0x2B00_07F2).unwrap();
//! assert!(!add.texture_alpha && !add.tests_texture && !add.fogged);
//! assert_eq!(SurfaceProgram::of(0x2B00_0717).unwrap().layers, Some(LayerCombine::Sum));
//! ```

use tessera_formats::Did;
use tessera_material::records::BlendFactor;
use tessera_material::resolve::FALLBACK_ALPHA_THRESHOLD;
use tessera_material::vocab::ids;
use tessera_material::{CurveStates, ResolvedMaterial, TextureMatrix, TextureMatrixLayout};

use crate::rng::Ran2;

/// Effective 4004: the alpha-test switch (`material.md` §11.2).
const ALPHA_TEST: u32 = ids::ALPHA_TEST;
/// Effective 1043: the alpha-test reference (`material.md` §11.2).
const ALPHA_THRESHOLD: u32 = ids::ALPHA_THRESHOLD;
/// The random curve mode of `material.md` §5.3.1.
const RANDOM_MODE: u32 = 3;

/// The blend class of a row of §9.1.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceBlend {
    /// SRCALPHA / INVSRCALPHA.
    Alpha,
    /// SRCALPHA / ONE.
    Additive,
}

/// How a two-layer record combines its layers' colours (the alphas always multiply).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerCombine {
    /// *T*.rgb = layer 1 × layer 2.
    Product,
    /// *T*.rgb = layer 1 + layer 2.
    Sum,
}

/// One row of the table of `particle-runtime.md` §9.1.1: what a surface program computes.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceProgram {
    /// The shader record of the row, `None` for [`SurfaceProgram::fallback`].
    pub shader: Option<u32>,
    /// The blend class.
    pub blend: SurfaceBlend,
    /// The alpha test, when the material enables it, reads the texture's opacity; `false`:
    /// the program discards nothing.
    pub tests_texture: bool,
    /// The texture's alpha is a factor of the output alpha.
    pub texture_alpha: bool,
    /// The vertex colour (the particle colour) is a factor of colour and alpha.
    pub vertex_colour: bool,
    /// *M* (material colour 2000 × the draw's tint) is a factor of colour and alpha.
    pub material_colour: bool,
    /// The colour is doubled.
    pub doubled: bool,
    /// The colour is fogged (`lighting.md` §10, after the doubling).
    pub fogged: bool,
    /// Two colour layers and how their colours combine.
    pub layers: Option<LayerCombine>,
    /// The program reads the frame-buffer image (0x2B000084); Tessera does not draw it.
    pub distortion: bool,
}

impl SurfaceProgram {
    const ALPHA: Self = Self {
        shader: None,
        blend: SurfaceBlend::Alpha,
        tests_texture: true,
        texture_alpha: true,
        vertex_colour: true,
        material_colour: true,
        doubled: false,
        fogged: true,
        layers: None,
        distortion: false,
    };

    const ADDITIVE: Self = Self {
        blend: SurfaceBlend::Additive,
        fogged: false,
        ..Self::ALPHA
    };

    /// The row of `shader` in §9.1.1, `None` for a record the table does not list.
    #[must_use]
    pub fn of(shader: u32) -> Option<Self> {
        let a = Self::ALPHA;
        let add = Self::ADDITIVE;
        let row = match shader {
            0x2B00_070F | 0x2B00_0779 | 0x2B00_07F1 => a,
            0x2B00_07EB | 0x2B00_0760 => Self { doubled: true, ..a },
            0x2B00_071B => Self {
                layers: Some(LayerCombine::Product),
                ..a
            },
            0x2B00_07AD | 0x2B00_072D => Self {
                layers: Some(LayerCombine::Sum),
                ..a
            },
            0x2B00_0703 | 0x2B00_077A => Self {
                doubled: true,
                layers: Some(LayerCombine::Product),
                ..a
            },
            0x2B00_0755 => Self {
                vertex_colour: false,
                ..a
            },
            0x2B00_07CB | 0x2B00_0701 => add,
            0x2B00_0742 | 0x2B00_0776 => Self {
                layers: Some(LayerCombine::Product),
                ..add
            },
            0x2B00_0717 => Self {
                layers: Some(LayerCombine::Sum),
                ..add
            },
            0x2B00_07F2 | 0x2B00_0726 => Self {
                tests_texture: false,
                texture_alpha: false,
                ..add
            },
            0x2B00_0084 => Self {
                tests_texture: false,
                material_colour: false,
                distortion: true,
                ..a
            },
            _ => return None,
        };
        Some(Self {
            shader: Some(shader),
            ..row
        })
    }

    /// The program of a shader record the table does not list (`particle-runtime.md`
    /// §12 rule 10): the common row of its blend class, one layer at the vertex UV, no
    /// doubling, fogged unless the destination factor is ONE.
    #[must_use]
    pub fn fallback(blend: SurfaceBlend) -> Self {
        match blend {
            SurfaceBlend::Alpha => Self::ALPHA,
            SurfaceBlend::Additive => Self::ADDITIVE,
        }
    }

    /// The program of a resolved surface material: its shader record's row, else the
    /// fallback for its blend (additive when the destination factor is ONE).
    #[must_use]
    pub fn of_material(m: &ResolvedMaterial) -> Self {
        Self::of(m.shader.did.0).unwrap_or_else(|| {
            Self::fallback(if m.features.blend.dst == BlendFactor::One {
                SurfaceBlend::Additive
            } else {
                SurfaceBlend::Alpha
            })
        })
    }
}

/// A particle surface ready for drawing (`particle-runtime.md` §9.1–§9.1.3), with its
/// clock-dependent values at time 0 ([`ParticleSurface::at`] gives them at another time).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleSurface {
    /// What the programs compute.
    pub program: SurfaceProgram,
    /// Material colour 2000 (white when not declared); the draw multiplies it by its tint
    /// (1, 1, 1, instance opacity) to form *M* (`material.md` §8.3).
    pub material_colour: [f32; 4],
    /// The colour texture (slot 3000).
    pub texture: Option<Did>,
    /// The second layer's texture (slot 3001), for two-layer programs only.
    pub second_layer: Option<Did>,
    /// The record's alpha-test reference (effective 1043; the shader's render-state
    /// reference, then 0.5, when 1043 is missing or not finite) when the test is on: the
    /// effective 4004 is on and the program tests the texture (§9.1.3: the mesh
    /// opacity-source rule does not apply). `None`: no test. The renderer applies the
    /// per-draw steps of `material.md` §11.2 to it (§9.1.1 rule 1).
    pub alpha_test: Option<f32>,
    /// Texture matrices 0 and 1 (`material.md` §8.4): the first layer's and the second
    /// layer's in two-layer programs (RT27); the identity for one-layer programs, which
    /// apply none.
    pub matrices: [TextureMatrix; 2],
    /// Whether the 1043 curve of a tested surface is not constant: evaluate it per draw.
    pub reference_animated: bool,
    /// Whether a curve of texture matrix 0 or 1 of a two-layer program is not constant:
    /// evaluate the matrices per draw.
    pub matrices_animated: bool,
}

impl ParticleSurface {
    /// The surface of a resolved material whose shader record binds the texture matrices
    /// of `layout` (`MaterialLibrary::texture_matrices`; an empty layout binds none).
    /// Non-finite material colour components are 1.
    ///
    /// Its values are a **snapshot at clock 0, not a draw**: where a value follows the
    /// clock ([`ParticleSurface::is_animated`]), every draw evaluates it again with
    /// [`ParticleSurface::at`], so the snapshot of such a value is never drawn. In the
    /// snapshot a mode-3 curve reads its centre value *B* (*r* = 0): building it takes
    /// nothing from the shared stream, which mode 3 draws from only at draws
    /// (`particle-runtime.md` §12 rule 12, RT33), and uses no other random source.
    #[must_use]
    pub fn of(m: &ResolvedMaterial, layout: &TextureMatrixLayout) -> Self {
        let program = SurfaceProgram::of_material(m);
        let slot = |id: u32| m.slot(id).and_then(|s| s.texture);
        let tested = program.tests_texture && m.params.flag(ALPHA_TEST) == Some(true);
        let two_layers = program.layers.is_some();
        let base = Self {
            program,
            material_colour: material_colour(m.tint()),
            texture: slot(ids::DIFFUSE),
            second_layer: program.layers.and_then(|_| slot(ids::SECOND_LAYER)),
            alpha_test: tested.then_some(FALLBACK_ALPHA_THRESHOLD),
            matrices: [TextureMatrix::IDENTITY; 2],
            reference_animated: tested
                && m.params
                    .curve(ALPHA_THRESHOLD)
                    .is_some_and(tessera_material::FloatCurve::is_animated),
            matrices_animated: two_layers
                && layout
                    .matrices
                    .iter()
                    .filter(|b| b.matrix <= 1)
                    .flat_map(|b| b.params.iter().flatten())
                    .any(|&id| {
                        m.params
                            .curve(id)
                            .is_some_and(tessera_material::FloatCurve::is_animated)
                    }),
        };
        let mut states = CurveStates::new();
        let values = layout
            .matrices
            .iter()
            .flat_map(|b| b.params.iter().flatten().copied());
        for id in std::iter::once(ALPHA_THRESHOLD).chain(values) {
            if m.params.curve(id).is_some_and(|c| c.mode == RANDOM_MODE) {
                states.state(id).supply_random(0.0);
            }
        }
        base.evaluated(m, layout, 0.0, &mut states, true)
    }

    /// Whether a value depends on the clock ([`ParticleSurface::at`] then changes it).
    #[must_use]
    pub fn is_animated(&self) -> bool {
        self.reference_animated || self.matrices_animated
    }

    /// This surface at the draw's clock `t` (`material.md` §5.3.1): the 1043 reference and
    /// the texture matrices re-evaluated where they are not constant
    /// (`particle-runtime.md` §9.1.1 answers (d) and (e)), everything else as it is. `m`
    /// and `layout` must be the ones given to [`ParticleSurface::of`].
    ///
    /// `states` are the curve states of the surface record (rule 12: one set per surface
    /// DID, not per 0x39 record or instance), where mode-10 curves keep their start time
    /// and last value. Mode-3 curves keep no state: at each evaluation every one of them
    /// takes its *r* as one real-range draw over [−1, 1] of `rng`, the shared particle
    /// stream (rules 1, 9 and 12; RT33), and nothing of it stays in `states`.
    #[must_use]
    pub fn at(
        &self,
        m: &ResolvedMaterial,
        layout: &TextureMatrixLayout,
        t: f64,
        states: &mut CurveStates,
        rng: &mut Ran2,
    ) -> Self {
        if !self.is_animated() {
            return *self;
        }
        let t = if t.is_finite() { t } else { 0.0 };
        let random = self.random_curves(m, layout);
        for &id in &random {
            // One real-range draw over [−1, 1] of the shared stream per mode-3 curve and
            // evaluation (§3.4; rule 9; rule 12).
            let r = f64::from(rng.range(-1.0, 1.0));
            states.state(id).supply_random(r);
        }
        let out = self.evaluated(m, layout, t, states, false);
        // Mode 3 keeps nothing between evaluations (rule 12).
        for id in random {
            states.remove(id);
        }
        out
    }

    /// The parameter ids of the mode-3 curves that [`ParticleSurface::at`] evaluates, each
    /// once, in evaluation order: the 1043 reference, then the values of texture matrices 0
    /// and 1 (`particle-runtime.md` §9.1.1; the order in which the client's curves take
    /// their numbers is not known and does not matter, rule 1).
    fn random_curves(&self, m: &ResolvedMaterial, layout: &TextureMatrixLayout) -> Vec<u32> {
        let reference =
            (self.alpha_test.is_some() && self.reference_animated).then_some(ALPHA_THRESHOLD);
        let matrices = layout
            .matrices
            .iter()
            .filter(|b| self.program.layers.is_some() && self.matrices_animated && b.matrix <= 1)
            .flat_map(|b| b.params.iter().flatten().copied());
        let mut ids: Vec<u32> = Vec::new();
        for id in reference.into_iter().chain(matrices) {
            if !ids.contains(&id) && m.params.curve(id).is_some_and(|c| c.mode == RANDOM_MODE) {
                ids.push(id);
            }
        }
        ids
    }

    fn evaluated(
        mut self,
        m: &ResolvedMaterial,
        layout: &TextureMatrixLayout,
        t: f64,
        states: &mut CurveStates,
        all: bool,
    ) -> Self {
        if self.alpha_test.is_some() && (all || self.reference_animated) {
            let fallback = m
                .shader
                .default_alpha_ref
                .filter(|r| r.is_finite())
                .unwrap_or(FALLBACK_ALPHA_THRESHOLD);
            // The raw value, not the finite-made one (which turns NaN into 0): a non-finite
            // 1043, constant or animated, falls back to the render-state reference, then 0.5
            // (`material.md` §18).
            self.alpha_test = Some(
                m.params
                    .curve(ALPHA_THRESHOLD)
                    .map(|c| c.evaluate_at(t, states.state(ALPHA_THRESHOLD)))
                    .filter(|r| r.is_finite())
                    .unwrap_or(fallback),
            );
        }
        if self.program.layers.is_some() && (all || self.matrices_animated) {
            let mut out = [TextureMatrix::IDENTITY; 2];
            for e in layout.evaluate(&m.params, t, states) {
                if let Some(slot) = usize::try_from(e.matrix).ok().and_then(|i| out.get_mut(i)) {
                    *slot = e.map;
                }
            }
            self.matrices = out;
        }
        self
    }

    /// Whether Tessera draws this surface: not the heat-distortion program, whose variant
    /// without feature bit 1 leaves the target unchanged (§9.1.2, rule 11).
    #[must_use]
    pub fn is_drawn(&self) -> bool {
        !self.program.distortion
    }
}

/// The material colour 2000 made finite: a non-finite colour channel is 1 (white), while a
/// NaN alpha counts as 0 and an infinite one is clamped to [0, 1], so *A* = *M*.alpha of the
/// per-draw reference follows `material.md` §18 (answer to the implementer note of commit
/// b949a54, item 2 (b)).
fn material_colour(c: [f32; 4]) -> [f32; 4] {
    let rgb = |v: f32| if v.is_finite() { v } else { 1.0 };
    let a = c[3];
    let alpha = if a.is_nan() {
        0.0
    } else if a.is_finite() {
        a
    } else {
        a.clamp(0.0, 1.0)
    };
    [rgb(c[0]), rgb(c[1]), rgb(c[2]), alpha]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_colour_alpha_rule() {
        assert_eq!(
            material_colour([0.5, 0.25, 1.0, 0.75]),
            [0.5, 0.25, 1.0, 0.75]
        );
        assert_eq!(
            material_colour([f32::NAN, 0.5, f32::INFINITY, f32::NAN]),
            [1.0, 0.5, 1.0, 0.0]
        );
        assert_eq!(material_colour([1.0; 4].map(|_| f32::INFINITY))[3], 1.0);
        assert_eq!(material_colour([0.0, 0.0, 0.0, f32::NEG_INFINITY])[3], 0.0);
    }

    /// shader, blend, tests, texture alpha, vertex colour, doubled, fogged, layers.
    type Row = (
        u32,
        SurfaceBlend,
        bool,
        bool,
        bool,
        bool,
        bool,
        Option<LayerCombine>,
    );

    /// Every row of the table of §9.1.1.
    #[test]
    fn the_nineteen_records() {
        let rows: [Row; 19] = [
            // shader, blend, tests, texture alpha, vertex, doubled, fogged, layers
            (
                0x2B00_070F,
                SurfaceBlend::Alpha,
                true,
                true,
                true,
                false,
                true,
                None,
            ),
            (
                0x2B00_0779,
                SurfaceBlend::Alpha,
                true,
                true,
                true,
                false,
                true,
                None,
            ),
            (
                0x2B00_07F1,
                SurfaceBlend::Alpha,
                true,
                true,
                true,
                false,
                true,
                None,
            ),
            (
                0x2B00_07EB,
                SurfaceBlend::Alpha,
                true,
                true,
                true,
                true,
                true,
                None,
            ),
            (
                0x2B00_0760,
                SurfaceBlend::Alpha,
                true,
                true,
                true,
                true,
                true,
                None,
            ),
            (
                0x2B00_071B,
                SurfaceBlend::Alpha,
                true,
                true,
                true,
                false,
                true,
                Some(LayerCombine::Product),
            ),
            (
                0x2B00_07AD,
                SurfaceBlend::Alpha,
                true,
                true,
                true,
                false,
                true,
                Some(LayerCombine::Sum),
            ),
            (
                0x2B00_072D,
                SurfaceBlend::Alpha,
                true,
                true,
                true,
                false,
                true,
                Some(LayerCombine::Sum),
            ),
            (
                0x2B00_0703,
                SurfaceBlend::Alpha,
                true,
                true,
                true,
                true,
                true,
                Some(LayerCombine::Product),
            ),
            (
                0x2B00_077A,
                SurfaceBlend::Alpha,
                true,
                true,
                true,
                true,
                true,
                Some(LayerCombine::Product),
            ),
            (
                0x2B00_0755,
                SurfaceBlend::Alpha,
                true,
                true,
                false,
                false,
                true,
                None,
            ),
            (
                0x2B00_07CB,
                SurfaceBlend::Additive,
                true,
                true,
                true,
                false,
                false,
                None,
            ),
            (
                0x2B00_0701,
                SurfaceBlend::Additive,
                true,
                true,
                true,
                false,
                false,
                None,
            ),
            (
                0x2B00_0742,
                SurfaceBlend::Additive,
                true,
                true,
                true,
                false,
                false,
                Some(LayerCombine::Product),
            ),
            (
                0x2B00_0717,
                SurfaceBlend::Additive,
                true,
                true,
                true,
                false,
                false,
                Some(LayerCombine::Sum),
            ),
            (
                0x2B00_0776,
                SurfaceBlend::Additive,
                true,
                true,
                true,
                false,
                false,
                Some(LayerCombine::Product),
            ),
            (
                0x2B00_07F2,
                SurfaceBlend::Additive,
                false,
                false,
                true,
                false,
                false,
                None,
            ),
            (
                0x2B00_0726,
                SurfaceBlend::Additive,
                false,
                false,
                true,
                false,
                false,
                None,
            ),
            (
                0x2B00_0084,
                SurfaceBlend::Alpha,
                false,
                true,
                true,
                false,
                true,
                None,
            ),
        ];
        for (did, blend, tests, ta, vc, doubled, fogged, layers) in rows {
            let p = SurfaceProgram::of(did).unwrap_or_else(|| panic!("{did:#x} listed"));
            assert_eq!(p.shader, Some(did));
            assert_eq!(p.blend, blend, "{did:#x}");
            assert_eq!(p.tests_texture, tests, "{did:#x}");
            assert_eq!(p.texture_alpha, ta, "{did:#x}");
            assert_eq!(p.vertex_colour, vc, "{did:#x}");
            assert_eq!(p.doubled, doubled, "{did:#x}");
            assert_eq!(p.fogged, fogged, "{did:#x}");
            assert_eq!(p.layers, layers, "{did:#x}");
            assert_eq!(p.distortion, did == 0x2B00_0084, "{did:#x}");
            assert_eq!(p.material_colour, did != 0x2B00_0084, "{did:#x}");
        }
        assert_eq!(SurfaceProgram::of(0x2B00_0700), None);
    }

    /// `particle-runtime.md` §9.1.1: the two-layer records whose colours multiply and the
    /// ones whose colours add, exactly as the notation paragraph lists them.
    #[test]
    fn two_layer_records_combine_as_listed() {
        for did in [
            0x2B00_0703,
            0x2B00_071B,
            0x2B00_0742,
            0x2B00_0776,
            0x2B00_077A,
        ] {
            let p = SurfaceProgram::of(did).unwrap_or_else(|| panic!("{did:#x} listed"));
            assert_eq!(p.layers, Some(LayerCombine::Product), "{did:#x}");
        }
        for did in [0x2B00_07AD, 0x2B00_0717, 0x2B00_072D] {
            let p = SurfaceProgram::of(did).unwrap_or_else(|| panic!("{did:#x} listed"));
            assert_eq!(p.layers, Some(LayerCombine::Sum), "{did:#x}");
        }
    }

    #[test]
    fn fallback_follows_the_blend_class() {
        let a = SurfaceProgram::fallback(SurfaceBlend::Alpha);
        assert!(a.fogged && a.texture_alpha && a.vertex_colour && a.tests_texture);
        assert_eq!(a.shader, None);
        let b = SurfaceProgram::fallback(SurfaceBlend::Additive);
        assert!(!b.fogged && !b.doubled && b.layers.is_none());
    }

    mod synthetic {
        use std::sync::Arc;

        use tessera_formats::Did;
        use tessera_material::records::{
            Binding, PERMUTATION_KEY_WORDS, ParamEntry, ParamValue, Permutation, RENDER_STATE_SIZE,
            RenderState,
        };
        use tessera_material::{
            CurveStates, FloatCurve, ResolvedMaterial, ShaderInterface, ShaderRecord,
            TextureMatrixLayout,
        };

        use crate::rng::Ran2;
        use crate::surface::ParticleSurface;

        fn float(id: u32, value: FloatCurve, binding: Option<(u16, u32)>) -> ParamEntry {
            ParamEntry {
                id,
                group: 1000,
                value: ParamValue::Float(value),
                flag: 0,
                bindings: binding
                    .map(|(semantic, slot)| Binding {
                        semantic,
                        group: 1000,
                        permutation: 0,
                        slot,
                    })
                    .into_iter()
                    .collect(),
            }
        }

        /// Shader record `did` with the full texture-matrix sets 0 and 1 (semantics
        /// 38–44, all identity values except 1019 = `translate_u1`), alpha test 4004 =
        /// `test` and reference 1043 = `reference`. No sampler roles: the mesh
        /// opacity-source rule finds no source.
        fn surface(
            did: u32,
            translate_u1: f32,
            test: bool,
            reference: FloatCurve,
        ) -> (Arc<ResolvedMaterial>, TextureMatrixLayout) {
            surface_with(did, FloatCurve::constant(translate_u1), test, reference)
        }

        /// [`surface`] with any curve for 1019.
        fn surface_with(
            did: u32,
            translate_u1: FloatCurve,
            test: bool,
            reference: FloatCurve,
        ) -> (Arc<ResolvedMaterial>, TextureMatrixLayout) {
            surface_values(did, &[(1019, translate_u1)], test, reference)
        }

        /// [`surface`] with any curves for the matrix values in `values`.
        fn surface_values(
            did: u32,
            values: &[(u32, FloatCurve)],
            test: bool,
            reference: FloatCurve,
        ) -> (Arc<ResolvedMaterial>, TextureMatrixLayout) {
            let mut params = Vec::new();
            for (slot, set) in [
                (0u32, [1017, 1018, 1033, 1001, 1002, 1070, 1071]),
                (1, [1019, 1020, 1034, 1003, 1004, 1072, 1073]),
            ] {
                for (k, id) in set.into_iter().enumerate() {
                    let v = match (values.iter().find(|(v, _)| *v == id), k) {
                        (Some((_, c)), _) => c.clone(),
                        (None, 3 | 4) => FloatCurve::constant(1.0),
                        (None, _) => FloatCurve::constant(0.0),
                    };
                    let semantic = 38 + u16::try_from(k).unwrap_or(0);
                    params.push(float(id, v, Some((semantic, slot))));
                }
            }
            params.push(float(1043, reference, None));
            params.push(ParamEntry {
                id: 4004,
                group: 4000,
                value: ParamValue::Flag(u32::from(test)),
                flag: 0,
                bindings: Vec::new(),
            });
            let mut raw = [0u8; RENDER_STATE_SIZE];
            // SRCALPHA / INVSRCALPHA, no depth write; block reference 0.9.
            raw[..7].copy_from_slice(&[5, 6, 1, 7, 0, 2, 0]);
            raw[11..15].copy_from_slice(&0.9f32.to_le_bytes());
            let record = ShaderRecord {
                did: Did(did),
                reserved: 0,
                params,
                permutations: vec![Permutation {
                    key: [0; PERMUTATION_KEY_WORDS],
                    programs: None,
                    samplers: Vec::new(),
                    extra_items: Vec::new(),
                    render_state: RenderState { raw },
                }],
            };
            let layout = TextureMatrixLayout::from_record(&record);
            let shader = Arc::new(ShaderInterface::from_record(&record));
            (Arc::new(ResolvedMaterial::resolve(shader, &[])), layout)
        }

        fn close(a: (f32, f32), b: (f32, f32)) -> bool {
            (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
        }

        /// `particle-runtime.md` RT-I12: a two-layer surface reads the second layer
        /// through texture matrix 1 and the first through matrix 0; a one-layer surface
        /// with the same values applies none.
        #[test]
        fn two_layer_records_apply_matrices_0_and_1() {
            let (m, layout) = surface(0x2B00_0703, 0.5, false, FloatCurve::constant(0.5));
            let s = ParticleSurface::of(&m, &layout);
            assert!(s.program.layers.is_some());
            assert!(close(s.matrices[0].apply(0.25, 0.75), (0.25, 0.75)));
            assert!(close(s.matrices[1].apply(0.25, 0.75), (0.75, 0.75)));
            assert!(!s.is_animated());

            let (m, layout) = surface(0x2B00_070F, 0.5, false, FloatCurve::constant(0.5));
            let s = ParticleSurface::of(&m, &layout);
            assert!(s.program.layers.is_none());
            for k in 0..2 {
                assert!(close(s.matrices[k].apply(0.25, 0.75), (0.25, 0.75)));
            }
        }

        /// `particle-runtime.md` §9.1.3: the test is on whenever 4004 is on and the
        /// program tests the texture, although the mesh rule finds no opacity source here;
        /// programs that test nothing and 4004 off give no test.
        #[test]
        fn alpha_test_follows_4004_not_the_mesh_rule() {
            let (m, layout) = surface(0x2B00_070F, 0.0, true, FloatCurve::constant(0.65));
            assert!(m.features.alpha_test.is_none(), "the mesh rule drops it");
            let s = ParticleSurface::of(&m, &layout);
            assert_eq!(s.alpha_test, Some(0.65));
            let (m, layout) = surface(0x2B00_07F2, 0.0, true, FloatCurve::constant(0.65));
            assert_eq!(ParticleSurface::of(&m, &layout).alpha_test, None);
            let (m, layout) = surface(0x2B00_070F, 0.0, false, FloatCurve::constant(0.65));
            assert_eq!(ParticleSurface::of(&m, &layout).alpha_test, None);
        }

        /// `material.md` §18: a non-finite 1043, constant or animated, falls back to the
        /// render-state reference (0.9 here), not to 0.
        #[test]
        fn non_finite_reference_falls_back_to_the_block_value() {
            let (m, layout) = surface(0x2B00_070F, 0.0, true, FloatCurve::constant(f32::NAN));
            let s = ParticleSurface::of(&m, &layout);
            assert_eq!(s.alpha_test, Some(0.9));
            // An animated curve whose value is not finite (mode 2 with an infinite slope).
            let mut ramp = FloatCurve::constant(0.5);
            ramp.mode = 2;
            ramp.params[1] = f32::INFINITY;
            ramp.param_count = 10;
            let (m, layout) = surface(0x2B00_070F, 0.0, true, ramp);
            let s = ParticleSurface::of(&m, &layout);
            assert!(s.reference_animated);
            let mut rng = Ran2::new(1);
            let at = s.at(&m, &layout, 2.0, &mut CurveStates::new(), &mut rng);
            assert_eq!(at.alpha_test, Some(0.9));
        }

        /// §9.1.1 answer (e): a reference curve that is not constant is evaluated at the
        /// draw's clock; a constant one is the same at every time.
        #[test]
        fn animated_reference_is_evaluated_per_draw() {
            let mut ramp = FloatCurve::constant(0.5);
            ramp.mode = 2;
            ramp.params[1] = 0.1;
            ramp.param_count = 10;
            let (m, layout) = surface(0x2B00_070F, 0.0, true, ramp);
            let s = ParticleSurface::of(&m, &layout);
            assert!(s.reference_animated && s.is_animated());
            let mut states = CurveStates::new();
            let mut rng = Ran2::new(1);
            let mut r = |t: f64, states: &mut CurveStates| {
                s.at(&m, &layout, t, states, &mut rng)
                    .alpha_test
                    .unwrap_or(f32::NAN)
            };
            assert!((r(0.0, &mut states) - 0.5).abs() < 1e-6);
            assert!((r(2.0, &mut states) - 0.7).abs() < 1e-5);
            assert!(r(f64::NAN, &mut states).is_finite());

            let (m, layout) = surface(0x2B00_070F, 0.0, true, FloatCurve::constant(0.8));
            let c = ParticleSurface::of(&m, &layout);
            assert_eq!(c.at(&m, &layout, 5.0, &mut states, &mut rng), c);
        }

        /// `particle-runtime.md` RT-I13 (rule 12): two 0x39 records that name one surface
        /// read one state set, kept per surface record, started by the first draw at clock
        /// 10: at 10.5 both read 0.5, from 11 on the held last value. A state per 0x39
        /// record would give the second system its start value 0.001 at 10.5.
        #[test]
        fn rt_i13_one_curve_state_per_surface_record() {
            let mut keyed = FloatCurve::constant(0.0);
            keyed.mode = 10;
            keyed.params[10] = 1.0;
            keyed.param_count = 11;
            keyed.loop_flag = 0;
            keyed.keys = vec![(0.0, 0.0), (1.0, 100.0)];
            let (m, layout) = surface_with(0x2B00_0703, keyed, false, FloatCurve::constant(0.5));
            let s = ParticleSurface::of(&m, &layout);
            assert!(s.matrices_animated);
            let u = |look: ParticleSurface| look.matrices[1].apply(0.0, 0.0).0;
            let mut rng = Ran2::new(3);
            // The caller's map from surface DID to states (both records name this surface).
            let mut per_surface = CurveStates::new();
            let first = u(s.at(&m, &layout, 10.0, &mut per_surface, &mut rng));
            // The start value: position 0.1 % of the period (`material.md` §5.3.1).
            assert!((first - 0.001).abs() < 1e-5, "{first}");
            let a = u(s.at(&m, &layout, 10.5, &mut per_surface, &mut rng));
            let b = u(s.at(&m, &layout, 10.5, &mut per_surface, &mut rng));
            assert!((a - 0.5).abs() < 1e-5 && (b - 0.5).abs() < 1e-5, "{a} {b}");
            let held = u(s.at(&m, &layout, 11.2, &mut per_surface, &mut rng));
            assert!((held - u(s.at(&m, &layout, 12.0, &mut per_surface, &mut rng))).abs() < 1e-6);
            // A fresh state at 10.5 (a state per record) starts at the first key instead.
            let fresh = u(s.at(&m, &layout, 10.5, &mut CurveStates::new(), &mut rng));
            assert!((fresh - 0.001).abs() < 1e-5, "{fresh}");
        }

        /// A mode-3 curve `B + A · r` with constant `b` and `a`.
        fn flicker(b: f32, a: f32) -> FloatCurve {
            let mut c = FloatCurve::constant(b);
            c.mode = 3;
            c.params[2] = a;
            c.param_count = 10;
            c
        }

        /// Rule 12: a mode-3 curve keeps no state; every evaluation takes one real-range
        /// draw over [−1, 1] of the shared stream, so the same stream gives the same values
        /// whatever the state set holds, and the set keeps nothing for it.
        #[test]
        fn random_curves_draw_from_the_shared_stream() {
            let (m, layout) = surface(0x2B00_070F, 0.0, true, flicker(0.5, 0.25));
            let s = ParticleSurface::of(&m, &layout);
            assert!(s.reference_animated);
            let run = |states: &mut CurveStates| {
                let mut rng = Ran2::new(7);
                (0..8)
                    .map(|k| {
                        s.at(&m, &layout, f64::from(k), states, &mut rng)
                            .alpha_test
                            .unwrap_or(f32::NAN)
                    })
                    .collect::<Vec<_>>()
            };
            // The snapshot at load is not a draw: it reads the centre value B and takes
            // nothing from any random source.
            assert_eq!(s.alpha_test, Some(0.5));
            assert_eq!(ParticleSurface::of(&m, &layout), s);
            let mut used = CurveStates::new();
            let first = run(&mut used);
            assert!(used.is_empty(), "mode 3 keeps no state");
            assert_eq!(run(&mut used), first, "no state carries over");
            assert_eq!(run(&mut CurveStates::new()), first);
            // Each value is B + A · r with r the stream's real-range draw over [−1, 1].
            let mut stream = Ran2::new(7);
            for v in &first {
                let r = f64::from(stream.range(-1.0, 1.0));
                assert!((f64::from(*v) - (0.5 + 0.25 * r)).abs() < 1e-6, "{v} {r}");
            }
            assert!(first.iter().all(|v| (0.25..=0.75).contains(v)), "{first:?}");
            assert!(first.windows(2).any(|w| w[0] != w[1]), "{first:?}");
        }

        /// `particle-runtime.md` RT-I16 (rule 12): a surface with two mode-3 texture-matrix
        /// values, drawn three times at one clock, advances the shared stream by exactly six
        /// draws; each value is B + A · r with r = −1 + 2u for the stream's next draw u, so
        /// the draws at one clock get their own values. Without mode-3 curves the stream
        /// does not advance.
        #[test]
        fn rt_i16_mode_3_draws_per_evaluation() {
            // 1019 and 1020: translation u and v of texture matrix 1.
            let (b_u, a_u, b_v, a_v) = (0.25f64, 0.125f64, -0.5f64, 0.375f64);
            let (m, layout) = surface_values(
                0x2B00_0703,
                &[
                    (1019, flicker(b_u as f32, a_u as f32)),
                    (1020, flicker(b_v as f32, a_v as f32)),
                ],
                false,
                FloatCurve::constant(0.5),
            );
            let s = ParticleSurface::of(&m, &layout);
            assert!(s.matrices_animated);
            // The snapshot at load reads the centre values B (r = 0).
            let (u0, v0) = s.matrices[1].apply(0.0, 0.0);
            assert!((f64::from(u0) - b_u).abs() < 1e-6 && (f64::from(v0) - b_v).abs() < 1e-6);
            let mut rng = Ran2::new(11);
            let mut reference = Ran2::new(11);
            let mut states = CurveStates::new();
            let mut seen = Vec::new();
            for _ in 0..3 {
                let (u, v) =
                    s.at(&m, &layout, 4.0, &mut states, &mut rng).matrices[1].apply(0.0, 0.0);
                let r1 = -1.0 + 2.0 * reference.next_f64();
                let r2 = -1.0 + 2.0 * reference.next_f64();
                // The order in which the two curves take their numbers does not matter.
                let fits = |ru: f64, rv: f64| {
                    (f64::from(u) - (b_u + a_u * ru)).abs() < 1e-5
                        && (f64::from(v) - (b_v + a_v * rv)).abs() < 1e-5
                };
                assert!(fits(r1, r2) || fits(r2, r1), "{u} {v} {r1} {r2}");
                seen.push((u, v));
            }
            assert!(states.is_empty(), "mode 3 keeps no state");
            assert_eq!(
                rng.next_f64().to_bits(),
                reference.next_f64().to_bits(),
                "exactly six draws"
            );
            assert!(seen.windows(2).any(|w| w[0] != w[1]), "{seen:?}");

            // No mode-3 curve: no draw.
            let (m, layout) = surface(0x2B00_0703, 0.5, false, FloatCurve::constant(0.5));
            let s = ParticleSurface::of(&m, &layout);
            let mut rng = Ran2::new(11);
            let _ = s.at(&m, &layout, 4.0, &mut states, &mut rng);
            assert_eq!(rng.next_f64().to_bits(), Ran2::new(11).next_f64().to_bits());
        }

        /// Rule 10: a shader record outside the table draws as the common row of its blend
        /// class with one layer and no matrices.
        #[test]
        fn unknown_records_use_the_common_row() {
            let (m, layout) = surface(0x2B00_0700, 0.5, true, FloatCurve::constant(0.5));
            let s = ParticleSurface::of(&m, &layout);
            assert_eq!(s.program.shader, None);
            assert!(s.program.layers.is_none() && s.program.fogged && !s.program.doubled);
            assert!(close(s.matrices[1].apply(0.0, 0.0), (0.0, 0.0)));
            assert_eq!(s.alpha_test, Some(0.5));
        }
    }
}
