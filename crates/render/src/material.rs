// SPDX-License-Identifier: GPL-3.0-or-later
//! Mesh materials, lighting and debug views — the inputs of a mesh draw.
//!
//! A [`Material`] is plain data (it holds texture handles, not GPU
//! objects), so a caller can change a dye colour or alpha threshold from one
//! frame to the next for free. The renderer picks one of its own WGSL
//! pipelines from the material's [`AlphaMode`] and `two_sided` flag; the
//! remaining features (textures, dye, normal map, specular) are uniform
//! switches in the same shader.
//!
//! Material **semantics** for game data (which texture slot is which, the
//! dye blend rule, alpha thresholds) belong to the material spec; this module
//! only defines what the renderer can draw. See `docs/specs/render.md` §2.3.

use glam::{Mat4, Vec3};

use crate::arena::MeshId;
use crate::frame::Rgba;
use crate::texture::TextureId;

/// How a material's alpha is used.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AlphaMode {
    /// Alpha is ignored; depth is written.
    #[default]
    Opaque,
    /// Alpha test: fragments with alpha below `cutoff` are discarded, the
    /// rest are drawn opaque with depth writes.
    Mask {
        /// Threshold in `0..=1` (non-finite values act as 0.5).
        cutoff: f32,
    },
    /// Alpha blending (`src * a + dst * (1 - a)`), no depth writes. Blended
    /// draws of a pass are drawn after everything else, sorted back to
    /// front by the distance of their submesh's bounding-sphere centre.
    Blend,
}

/// Where a [`Dye`]'s mask comes from.
///
/// Mask convention (`docs/specs/render.md` §2.3): single-channel masks are
/// read from the **red** channel. A mask decoded from an alpha-only source
/// (e.g. an A8 surface, which `tessera-formats` expands to
/// `(255, 255, 255, a)`) must be swizzled so the value is in red (or in all
/// four channels) before upload; uploaded as decoded it would dye the
/// whole surface. A mask that *is* a base texture's alpha uses
/// [`DyeMask::BaseAlpha`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DyeMask {
    /// The red channel of a separate mask texture (sampled with the mesh
    /// UVs; upload it with [`crate::ColorSpace::Linear`]).
    Texture(TextureId),
    /// The base texture's alpha channel. That alpha then only masks the dye;
    /// coverage alpha comes from the base colour factor and vertex colour.
    BaseAlpha,
}

/// Dye: `colour = base * mix(1, dye_color, mask)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dye {
    /// Linear RGB dye colour.
    pub color: [f32; 3],
    /// Mask source (1 = fully dyed).
    pub mask: DyeMask,
}

/// Blinn-Phong specular highlight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Specular {
    /// Linear RGB highlight colour (scaled by the light colour).
    pub color: [f32; 3],
    /// Blinn-Phong exponent (clamped to `1..=1024`).
    pub shininess: f32,
    /// Optional mask: its RGB multiplies `color` (upload linear). A
    /// single-channel mask must carry its value in R, G and B; an
    /// alpha-only source (A8 decodes to `(255, 255, 255, a)`) has to be
    /// replicated into RGB first, or the mask is fully on everywhere.
    pub texture: Option<TextureId>,
}

/// Surface description of a mesh draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    /// Linear RGBA factor multiplied with the base texture and vertex colour.
    pub base_color: Rgba,
    /// Base colour texture (`None`: white).
    pub base_texture: Option<TextureId>,
    /// Alpha handling.
    pub alpha: AlphaMode,
    /// Draw back faces too (their normals are flipped for lighting).
    pub two_sided: bool,
    /// Tangent-space normal map (RGB = XYZ * 0.5 + 0.5, upload linear).
    /// Only used when the mesh has tangents. Red points along the tangent
    /// (increasing `u`); green along increasing `v`, i.e. **down** the
    /// image since `v = 0` is the top row (the "Y-down" convention).
    pub normal_texture: Option<TextureId>,
    /// The normal map's green channel points **up** the image (along
    /// decreasing `v`, the "Y-up" convention) instead.
    pub normal_green_up: bool,
    /// Dye tint.
    pub dye: Option<Dye>,
    /// Specular highlight.
    pub specular: Option<Specular>,
    /// Skip lighting: output the base colour as is.
    pub unlit: bool,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            base_color: [1.0; 4],
            base_texture: None,
            alpha: AlphaMode::Opaque,
            two_sided: false,
            normal_texture: None,
            normal_green_up: false,
            dye: None,
            specular: None,
            unlit: false,
        }
    }
}

impl Material {
    /// Opaque material with a constant colour.
    #[must_use]
    pub fn color(base_color: Rgba) -> Self {
        Self {
            base_color,
            ..Self::default()
        }
    }

    /// Opaque material with a base texture.
    #[must_use]
    pub fn textured(texture: TextureId) -> Self {
        Self {
            base_texture: Some(texture),
            ..Self::default()
        }
    }
}

/// Directional light plus ambient term of a world pass.
///
/// The defaults are Tessera's own neutral studio lighting, not values from
/// any other client.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lighting {
    /// Direction **towards** the light (normalised by the renderer; a zero
    /// or non-finite vector falls back to the default).
    pub direction: Vec3,
    /// Linear RGB light colour (intensity included).
    pub color: [f32; 3],
    /// Linear RGB ambient light.
    pub ambient: [f32; 3],
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            // Up-ish on both the Y-up and Z-up conventions, from the front-right.
            direction: Vec3::new(0.45, 0.6, 0.66),
            color: [0.85, 0.85, 0.82],
            ambient: [0.22, 0.23, 0.26],
        }
    }
}

impl Lighting {
    /// Normalised direction towards the light.
    #[must_use]
    pub fn direction_normalized(&self) -> Vec3 {
        let d = self.direction;
        let m = d.abs().max_element();
        if d.is_finite() && m.is_normal() {
            (d / m).try_normalize()
        } else {
            None
        }
        .unwrap_or_else(|| Self::default().direction.normalize())
    }
}

/// How mesh surfaces are coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MeshShading {
    /// The material, lit.
    #[default]
    Lit,
    /// The material's base colour (texture × factor × vertex colour × dye),
    /// unlit.
    BaseColor,
    /// World-space normal (after normal mapping) as `n * 0.5 + 0.5`.
    Normals,
    /// A procedural checkerboard (8 × 8 cells per UV unit, tinted by the
    /// UV value), lightly lit, to show UV layout and stretching.
    UvChecker,
    /// Surfaces are not drawn (for wireframe-only views).
    Hidden,
}

/// Vertex-normal line overlay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalLines {
    /// Line length in world units.
    pub length: f32,
    /// Linear RGBA colour.
    pub color: Rgba,
}

/// Debug view of the meshes in a world pass.
///
/// Debug shadings other than [`MeshShading::Lit`] draw blended materials as
/// opaque; alpha-tested materials keep their cut-outs.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MeshDebug {
    /// Surface colouring.
    pub shading: MeshShading,
    /// Draw triangle edges in this colour (needs
    /// [`crate::MeshConfig::wireframe_edges`]).
    pub wireframe: Option<Rgba>,
    /// Draw a line along each vertex normal. Lines cover every vertex of
    /// the mesh, also for draws restricted to one submesh.
    pub normals: Option<NormalLines>,
}

/// One mesh draw in a world pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshDraw {
    /// The mesh.
    pub mesh: MeshId,
    /// Which submesh to draw; `None` draws every submesh with this material.
    pub submesh: Option<u32>,
    /// Local → world transform.
    pub transform: Mat4,
    /// Surface.
    pub material: Material,
}

impl MeshDraw {
    /// Draw all submeshes of `mesh` at `transform` with `material`.
    #[must_use]
    pub fn new(mesh: MeshId, transform: Mat4, material: Material) -> Self {
        Self {
            mesh,
            submesh: None,
            transform,
            material,
        }
    }

    /// Restrict the draw to one submesh.
    #[must_use]
    pub fn with_submesh(mut self, submesh: u32) -> Self {
        self.submesh = Some(submesh);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_direction_is_sanitised() {
        let d = Lighting {
            direction: Vec3::new(f32::NAN, 0.0, 0.0),
            ..Lighting::default()
        };
        assert!((d.direction_normalized().length() - 1.0).abs() < 1e-6);
        let z = Lighting {
            direction: Vec3::ZERO,
            ..Lighting::default()
        };
        assert!(z.direction_normalized().is_finite());
        let big = Lighting {
            direction: Vec3::splat(f32::MAX),
            ..Lighting::default()
        };
        assert!((big.direction_normalized() - Vec3::splat(1.0 / 3f32.sqrt())).length() < 1e-6);
    }
}
