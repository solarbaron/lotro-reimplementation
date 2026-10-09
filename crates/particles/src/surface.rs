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
//! vertex or material alpha, and three records test nothing. 0x2B000084 reads the
//! frame-buffer image (heat distortion); Tessera draws it as that record's variant without
//! a frame-buffer input, which leaves the target unchanged, that is not at all
//! ([`SurfaceProgram::distortion`]).
//!
//! [`SurfaceProgram::of`] gives the row of a known record; [`SurfaceProgram::fallback`]
//! is the engineering choice for any other shader record (none in the data; recorded as
//! an implementer OPEN in `particle-runtime.md` §9.1.1). [`ParticleSurface::of`] combines
//! the row with a resolved surface material: the material colour 2000 (*M* before the
//! draw's tint, `material.md` §8.3) and the second layer's texture.
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
use tessera_material::ResolvedMaterial;
use tessera_material::records::BlendFactor;
use tessera_material::vocab::ids;

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
            0x2B00_071B | 0x2B00_072D => Self {
                layers: Some(LayerCombine::Product),
                ..a
            },
            0x2B00_07AD => Self {
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

    /// The program assumed for a shader record the table does not list (Tessera choice,
    /// `particle-runtime.md` §9.1.1 implementer OPEN): the common row of its blend class,
    /// one layer, no doubling, fogged when it is not additive.
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

/// A particle surface ready for drawing (`particle-runtime.md` §9.1, §9.1.1).
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
}

impl ParticleSurface {
    /// The surface of a resolved material. Non-finite material colour components are 1.
    #[must_use]
    pub fn of(m: &ResolvedMaterial) -> Self {
        let program = SurfaceProgram::of_material(m);
        let slot = |id: u32| m.slot(id).and_then(|s| s.texture);
        Self {
            program,
            material_colour: m.tint().map(|c| if c.is_finite() { c } else { 1.0 }),
            texture: slot(ids::DIFFUSE),
            second_layer: program.layers.and_then(|_| slot(ids::SECOND_LAYER)),
        }
    }

    /// Whether Tessera draws this surface (not the frame-buffer distortion program).
    #[must_use]
    pub fn is_drawn(&self) -> bool {
        !self.program.distortion
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                Some(LayerCombine::Product),
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

    #[test]
    fn fallback_follows_the_blend_class() {
        let a = SurfaceProgram::fallback(SurfaceBlend::Alpha);
        assert!(a.fogged && a.texture_alpha && a.vertex_colour && a.tests_texture);
        assert_eq!(a.shader, None);
        let b = SurfaceProgram::fallback(SurfaceBlend::Additive);
        assert!(!b.fogged && !b.doubled && b.layers.is_none());
    }
}
