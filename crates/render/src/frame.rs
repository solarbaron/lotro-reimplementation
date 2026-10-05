// SPDX-License-Identifier: GPL-3.0-or-later
//! Frame graph-lite: a frame is an ordered list of named passes, each with
//! its own coordinate space, clear behaviour and draw lists. Nothing touches
//! the GPU until [`crate::Renderer::end_frame`] records and submits them.

use glam::{Mat4, Vec3};

use crate::error::Result;
use crate::material::{Lighting, MeshDebug, MeshDraw};
use crate::pipelines::LineVertex;
use crate::skin::{PaletteData, SkinPalette};
use crate::texture::TextureId;

/// Linear RGBA colour.
pub type Rgba = [f32; 4];

/// Coordinate space of a pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PassSpace {
    /// 3D world pass with a depth buffer. `view_proj` maps world → clip
    /// with **reversed depth** (near plane → `z = 1`, far plane → `z = 0`),
    /// as [`crate::Camera::view_proj`] produces; the depth buffer is cleared
    /// to [`crate::DEPTH_CLEAR`] and tested with [`crate::DEPTH_COMPARE`].
    ///
    /// All world passes of a frame share one depth buffer. The first world
    /// pass starts from a cleared depth buffer; later ones keep testing
    /// against the depth written before them unless they call
    /// [`Pass::clear`] (colour + depth) or [`Pass::clear_depth`] (depth
    /// only, e.g. for a view-model layer or a sky dome drawn under a
    /// different camera).
    World {
        /// Combined projection * view matrix.
        view_proj: Mat4,
    },
    /// Screen-space overlay without depth. Units are pixels, origin at the
    /// top-left corner, +Y down.
    Screen,
}

/// A textured, tinted quad. The local quad spans `-0.5..0.5` in X and Y
/// (Z = 0); `transform` places it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    /// Local → pass-space transform.
    pub transform: Mat4,
    /// Multiplied with the sampled texel.
    pub color: Rgba,
    /// `None` = plain colour (a 1×1 white texture is bound).
    pub texture: Option<TextureId>,
    /// `[u_offset, v_offset, u_scale, v_scale]`; `v = 0` is the top edge.
    pub uv_rect: [f32; 4],
}

impl Quad {
    /// White, untextured quad with the given transform.
    #[must_use]
    pub fn new(transform: Mat4) -> Self {
        Self {
            transform,
            color: [1.0; 4],
            texture: None,
            uv_rect: [0.0, 0.0, 1.0, 1.0],
        }
    }

    /// Axis-aligned rectangle in screen pixels (for [`PassSpace::Screen`]);
    /// `(x, y)` is the top-left corner.
    #[must_use]
    pub fn screen_rect(x: f32, y: f32, w: f32, h: f32) -> Self {
        // Negative Y scale keeps texture "up" pointing to the top of the screen.
        Self::new(
            Mat4::from_translation(Vec3::new(x + w * 0.5, y + h * 0.5, 0.0))
                * Mat4::from_scale(Vec3::new(w, -h, 1.0)),
        )
    }

    /// Set the tint colour.
    #[must_use]
    pub fn with_color(mut self, color: Rgba) -> Self {
        self.color = color;
        self
    }

    /// Set the texture.
    #[must_use]
    pub fn with_texture(mut self, texture: TextureId) -> Self {
        self.texture = Some(texture);
        self
    }

    /// Set the UV sub-rectangle.
    #[must_use]
    pub fn with_uv(mut self, offset: [f32; 2], scale: [f32; 2]) -> Self {
        self.uv_rect = [offset[0], offset[1], scale[0], scale[1]];
        self
    }
}

/// One pass of a frame.
#[derive(Debug, Clone)]
pub struct Pass {
    pub(crate) label: String,
    pub(crate) space: PassSpace,
    pub(crate) clear_color: Option<wgpu::Color>,
    pub(crate) clear_depth: bool,
    pub(crate) quads: Vec<Quad>,
    pub(crate) lines: Vec<LineVertex>,
    pub(crate) meshes: Vec<MeshDraw>,
    pub(crate) lighting: Lighting,
    pub(crate) mesh_debug: MeshDebug,
    pub(crate) eye: Option<Vec3>,
}

impl Pass {
    pub(crate) fn new(label: String, space: PassSpace) -> Self {
        Self {
            label,
            space,
            clear_color: None,
            clear_depth: false,
            quads: Vec::new(),
            lines: Vec::new(),
            meshes: Vec::new(),
            lighting: Lighting::default(),
            mesh_debug: MeshDebug::default(),
            eye: None,
        }
    }

    /// Debug label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Coordinate space.
    #[must_use]
    pub fn space(&self) -> PassSpace {
        self.space
    }

    /// Clear the colour target before drawing (linear RGBA). On a world
    /// pass this also clears the shared depth buffer, since depth written
    /// for the overwritten colours is meaningless afterwards.
    pub fn clear(&mut self, color: Rgba) -> &mut Self {
        self.clear_color = Some(wgpu::Color {
            r: f64::from(color[0]),
            g: f64::from(color[1]),
            b: f64::from(color[2]),
            a: f64::from(color[3]),
        });
        self.clear_depth = true;
        self
    }

    /// Clear only the shared depth buffer before drawing, keeping the colour
    /// produced by earlier passes. No effect on screen passes (they have no
    /// depth).
    pub fn clear_depth(&mut self) -> &mut Self {
        self.clear_depth = true;
        self
    }

    /// Queue a quad. Quads are drawn in submission order.
    pub fn quad(&mut self, quad: Quad) -> &mut Self {
        self.quads.push(quad);
        self
    }

    /// Queue a mesh draw (world passes only; screen passes ignore meshes).
    ///
    /// Within a world pass, opaque meshes are drawn first, then alpha-tested
    /// ones, then quads, lines and debug overlays, then alpha-blended meshes
    /// sorted back to front (`docs/specs/render.md` §2.3, "Draw order").
    ///
    /// Opaque and alpha-tested draws are each grouped by pipeline (skinned,
    /// face culling), material and atlas page to save state changes, so they
    /// are **not** drawn in submission order: only draws with equal keys keep
    /// it. Depth testing passes equal depth, so of two coplanar surfaces the
    /// one drawn later wins; a coplanar decal therefore wins reliably only
    /// when it shares the pipeline, material and page of the surface beneath
    /// it and is submitted after it.
    pub fn mesh(&mut self, draw: MeshDraw) -> &mut Self {
        self.meshes.push(draw);
        self
    }

    /// Mesh draws queued so far.
    #[must_use]
    pub fn meshes(&self) -> &[MeshDraw] {
        &self.meshes
    }

    /// Set the directional + ambient light used by this pass's meshes.
    pub fn lighting(&mut self, lighting: Lighting) -> &mut Self {
        self.lighting = lighting;
        self
    }

    /// Set the debug view of this pass's meshes.
    pub fn mesh_debug(&mut self, debug: MeshDebug) -> &mut Self {
        self.mesh_debug = debug;
        self
    }

    /// Set the camera position used for specular highlights. Passes made
    /// with [`Frame::world_pass`] set it from the camera; for a raw
    /// [`PassSpace::World`] pass without it, the centre of the near plane
    /// (derived from `view_proj`) is used.
    pub fn eye(&mut self, eye: Vec3) -> &mut Self {
        self.eye = Some(eye);
        self
    }

    /// Queue one line segment.
    pub fn line(&mut self, a: Vec3, b: Vec3, color: Rgba) -> &mut Self {
        self.lines.push(LineVertex {
            position: a.to_array(),
            color,
        });
        self.lines.push(LineVertex {
            position: b.to_array(),
            color,
        });
        self
    }

    /// Queue an XYZ axis gizmo (X red, Y green, Z blue).
    pub fn axes(&mut self, origin: Vec3, length: f32) -> &mut Self {
        self.line(origin, origin + Vec3::X * length, [1.0, 0.0, 0.0, 1.0])
            .line(origin, origin + Vec3::Y * length, [0.0, 1.0, 0.0, 1.0])
            .line(origin, origin + Vec3::Z * length, [0.0, 0.0, 1.0, 1.0])
    }

    /// Queue a square grid of `2 * half_cells` cells per side on the plane
    /// spanned by `u` and `v`, centred on `center`.
    pub fn grid(
        &mut self,
        center: Vec3,
        u: Vec3,
        v: Vec3,
        cell: f32,
        half_cells: u32,
        color: Rgba,
    ) -> &mut Self {
        let half_cells = half_cells.min(1024);
        let n = half_cells as f32;
        for i in 0..=(2 * half_cells) {
            let t = (i as f32 - n) * cell;
            let ext = n * cell;
            self.line(center + u * t - v * ext, center + u * t + v * ext, color);
            self.line(center + v * t - u * ext, center + v * t + u * ext, color);
        }
        self
    }

    /// Queue an axis-aligned box outline.
    pub fn aabb(&mut self, min: Vec3, max: Vec3, color: Rgba) -> &mut Self {
        let c = |x: bool, y: bool, z: bool| {
            Vec3::new(
                if x { max.x } else { min.x },
                if y { max.y } else { min.y },
                if z { max.z } else { min.z },
            )
        };
        for (a, b) in [
            ((0, 0, 0), (1, 0, 0)),
            ((0, 1, 0), (1, 1, 0)),
            ((0, 0, 1), (1, 0, 1)),
            ((0, 1, 1), (1, 1, 1)),
            ((0, 0, 0), (0, 1, 0)),
            ((1, 0, 0), (1, 1, 0)),
            ((0, 0, 1), (0, 1, 1)),
            ((1, 0, 1), (1, 1, 1)),
            ((0, 0, 0), (0, 0, 1)),
            ((1, 0, 0), (1, 0, 1)),
            ((0, 1, 0), (0, 1, 1)),
            ((1, 1, 0), (1, 1, 1)),
        ] {
            self.line(
                c(a.0 == 1, a.1 == 1, a.2 == 1),
                c(b.0 == 1, b.1 == 1, b.2 == 1),
                color,
            );
        }
        self
    }
}

pub(crate) enum FrameTarget {
    /// Acquired surface texture plus the renderer's liveness token.
    Surface(wgpu::SurfaceTexture, std::sync::Arc<()>),
    Offscreen,
}

impl std::fmt::Debug for FrameTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Surface(..) => f.write_str("Surface"),
            Self::Offscreen => f.write_str("Offscreen"),
        }
    }
}

/// A frame being built. Obtain from [`crate::Renderer::begin_frame`] or
/// [`crate::Renderer::begin_offscreen_frame`]; hand back to
/// [`crate::Renderer::end_frame`].
#[derive(Debug)]
pub struct Frame {
    /// Id of the renderer that created the frame.
    pub(crate) owner: u64,
    pub(crate) target: FrameTarget,
    pub(crate) view: wgpu::TextureView,
    pub(crate) format: wgpu::TextureFormat,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) passes: Vec<Pass>,
    /// Serial of this frame within its renderer (stamped, together with
    /// `owner`, into its palettes; the pair is process-unique).
    pub(crate) serial: u64,
    /// Skinning palettes of this frame, uploaded by `end_frame`.
    pub(crate) palettes: PaletteData,
}

impl Frame {
    /// Upload a skinning palette for this frame: `matrices[b]` is the
    /// skinning matrix of bone `b` (pose transform times inverse bind
    /// transform, `skeleton-anim.md` §4.4), mapping bind-pose model space to
    /// posed model space. Only the affine part (top three rows) is used;
    /// a non-finite matrix is replaced by the identity.
    ///
    /// Attach the returned handle to any number of draws of this frame with
    /// [`MeshDraw::with_skin`] (all parts of one character share one
    /// palette). All palettes of a frame go to the GPU in one copy; no
    /// buffer or bind group is created per palette or per draw.
    ///
    /// # Errors
    ///
    /// [`crate::RenderError::InvalidSkin`] for an empty palette or one with
    /// more than [`crate::MAX_PALETTE_BONES`] matrices;
    /// [`crate::RenderError::LimitExceeded`] when the frame's palettes would
    /// exceed the renderer's per-frame palette budget (64 MiB, or less on
    /// devices with smaller buffers).
    pub fn skin_palette(&mut self, matrices: &[Mat4]) -> Result<SkinPalette> {
        let offset = self
            .palettes
            .push(matrices.len(), |i| PaletteData::mat4_rows(&matrices[i]))?;
        Ok(self.palette_handle(offset, matrices.len()))
    }

    /// [`Self::skin_palette`] with each matrix given as its top three rows
    /// (`[r0c0, r0c1, r0c2, r0c3, r1c0, …, r2c3]`, the affine 3 × 4 part,
    /// row-major).
    ///
    /// # Errors
    ///
    /// As [`Self::skin_palette`].
    pub fn skin_palette_rows(&mut self, rows: &[[f32; 12]]) -> Result<SkinPalette> {
        let offset = self.palettes.push(rows.len(), |i| {
            let r = &rows[i];
            [
                [r[0], r[1], r[2], r[3]],
                [r[4], r[5], r[6], r[7]],
                [r[8], r[9], r[10], r[11]],
            ]
        })?;
        Ok(self.palette_handle(offset, rows.len()))
    }

    fn palette_handle(&self, offset: u32, len: usize) -> SkinPalette {
        SkinPalette {
            owner: self.owner,
            frame: self.serial,
            offset,
            // `push` accepted it, so len <= MAX_PALETTE_BONES.
            len: u16::try_from(len).unwrap_or(u16::MAX),
        }
    }

    /// Bytes of skinning-palette data queued in this frame so far.
    #[must_use]
    pub fn palette_bytes(&self) -> u64 {
        self.palettes.rows.len() as u64 * 16
    }

    /// Target size in pixels.
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Width / height (1.0 for a degenerate target).
    #[must_use]
    pub fn aspect(&self) -> f32 {
        if self.height == 0 {
            1.0
        } else {
            self.width as f32 / self.height as f32
        }
    }

    /// Colour format of the target.
    #[must_use]
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Append a pass; passes execute in the order they were added.
    pub fn pass(&mut self, label: impl Into<String>, space: PassSpace) -> &mut Pass {
        self.passes.push(Pass::new(label.into(), space));
        let last = self.passes.len() - 1;
        &mut self.passes[last]
    }

    /// Convenience: append a world pass using a camera.
    pub fn world_pass(&mut self, label: impl Into<String>, camera: &crate::Camera) -> &mut Pass {
        let aspect = self.aspect();
        self.pass(
            label,
            PassSpace::World {
                view_proj: camera.view_proj(aspect),
            },
        )
        .eye(camera.eye)
    }

    /// Passes recorded so far.
    #[must_use]
    pub fn passes(&self) -> &[Pass] {
        &self.passes
    }
}

/// Pixel-space orthographic projection (origin top-left, +Y down).
#[must_use]
pub fn screen_projection(width: u32, height: u32) -> Mat4 {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    glam::camera::rh::proj::directx::orthographic(0.0, w, h, 0.0, -1.0, 1.0)
}

/// Split quads into runs sharing a texture, preserving order.
pub(crate) fn texture_runs(quads: &[Quad]) -> Vec<(Option<TextureId>, std::ops::Range<u32>)> {
    let mut runs: Vec<(Option<TextureId>, std::ops::Range<u32>)> = Vec::new();
    for (i, q) in quads.iter().enumerate() {
        let i = u32::try_from(i).unwrap_or(u32::MAX);
        match runs.last_mut() {
            Some((tex, range)) if *tex == q.texture && range.end == i => range.end = i + 1,
            _ => runs.push((q.texture, i..i + 1)),
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_rect_maps_to_ndc_corners() {
        let proj = screen_projection(200, 100);
        let q = Quad::screen_rect(0.0, 0.0, 200.0, 100.0);
        // local top-left corner (-0.5, +0.5) → screen (0,0) → NDC (-1, +1)
        let p = proj * q.transform * glam::Vec4::new(-0.5, 0.5, 0.0, 1.0);
        assert!(
            (p.x + 1.0).abs() < 1e-6 && (p.y - 1.0).abs() < 1e-6,
            "{p:?}"
        );
        let p = proj * q.transform * glam::Vec4::new(0.5, -0.5, 0.0, 1.0);
        assert!(
            (p.x - 1.0).abs() < 1e-6 && (p.y + 1.0).abs() < 1e-6,
            "{p:?}"
        );
        assert!((0.0..=1.0).contains(&p.z));
    }

    #[test]
    fn runs_group_consecutive_textures() {
        let a = None;
        let mk = |t| Quad {
            texture: t,
            ..Quad::new(Mat4::IDENTITY)
        };
        let runs = texture_runs(&[mk(a), mk(a), mk(a)]);
        assert_eq!(runs, vec![(None, 0..3)]);
        assert!(texture_runs(&[]).is_empty());
    }

    #[test]
    fn grid_and_aabb_line_counts() {
        let mut p = Pass::new("t".into(), PassSpace::Screen);
        p.aabb(Vec3::ZERO, Vec3::ONE, [1.0; 4]);
        assert_eq!(p.lines.len(), 24);
        let mut p = Pass::new("t".into(), PassSpace::Screen);
        p.grid(Vec3::ZERO, Vec3::X, Vec3::Z, 1.0, 2, [1.0; 4]);
        assert_eq!(p.lines.len(), 5 * 2 * 2);
    }
}
