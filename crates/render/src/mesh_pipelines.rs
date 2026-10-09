// SPDX-License-Identifier: GPL-3.0-or-later
//! Mesh pipelines: layouts, per-draw uniforms and the lazily built variants
//! (surface alpha mode × cull mode × target format × static or skinned,
//! plus the overlays).
//!
//! Skinned variants read a second vertex stream (bone indices and weights,
//! [`crate::skin`]) and the draw's palette, a dynamic-offset uniform range of
//! the per-frame palette buffer bound next to the draw uniform (group 1,
//! binding 1). Static variants share the layout and ignore the palette.

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4};

use crate::material::{AlphaMode, DyeMask, Material, MeshShading};
use crate::mesh::GpuVertex;
use crate::pipelines::{DEPTH_FORMAT, SharedLayouts};
use crate::skin::{GpuSkin, PALETTE_BINDING_BYTES};

const MESH_WGSL: &str = include_str!("shaders/mesh.wgsl");

/// Feature bits (must match `F_*` in `mesh.wgsl`).
pub(crate) mod feature {
    pub const NORMAL_MAP: u32 = 1;
    pub const DYE: u32 = 2;
    pub const SPECULAR: u32 = 4;
    pub const SPECULAR_TEX: u32 = 8;
    pub const UNLIT: u32 = 16;
    pub const TWO_SIDED: u32 = 32;
    pub const MIRRORED: u32 = 64;
}

/// Per-draw uniform block (must match `Draw` in `mesh.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub(crate) struct DrawUniform {
    pub model: [[f32; 4]; 4],
    pub normal: [[f32; 4]; 3],
    pub base_color: [f32; 4],
    pub dye: [f32; 4],
    pub specular: [f32; 4],
    pub params: [f32; 4],
    /// Wireframe colour.
    pub overlay_color: [f32; 4],
    /// Vertex-normal line colour ([`crate::NormalLines::color`]).
    pub normal_color: [f32; 4],
    pub flags: [u32; 4],
    /// Lit pipeline only: rgb = glow colour, w = specular power.
    pub lit: [f32; 4],
    /// Lit pipeline only: packed light indices (x, y), light count, lit flags.
    pub lit_lights: [u32; 4],
    /// Lit pipeline only: texture matrices 0 and 1 ([`crate::UvTransform::pack`]).
    pub uv: [[f32; 4]; 3],
}

/// Default alpha-test threshold for a non-finite cutoff.
const DEFAULT_CUTOFF: f32 = 0.5;
/// Fraction of the eye distance overlay lines are moved towards the camera.
const OVERLAY_PULL: f32 = 1.0 / 512.0;

/// Shading mode index (must match `MODE_*` in `mesh.wgsl`).
fn mode_index(shading: MeshShading) -> u32 {
    match shading {
        MeshShading::Lit | MeshShading::Hidden => 0,
        MeshShading::BaseColor => 1,
        MeshShading::Normals => 2,
        MeshShading::UvChecker => 3,
    }
}

fn finite_or(v: f32, fallback: f32) -> f32 {
    if v.is_finite() { v } else { fallback }
}

/// Inverse transpose of the upper 3×3, falling back to the 3×3 itself when
/// the transform is singular or non-finite.
fn normal_matrix(model: Mat4) -> Mat3 {
    let m = Mat3::from_mat4(model);
    let det = m.determinant();
    if det.is_finite() && det.abs() > 1e-20 {
        let n = m.inverse().transpose();
        if n.is_finite() {
            return n;
        }
    }
    if m.is_finite() { m } else { Mat3::IDENTITY }
}

/// Whether `model` mirrors (negative determinant): winding flips.
pub(crate) fn is_mirrored(model: Mat4) -> bool {
    Mat3::from_mat4(model).determinant() < 0.0
}

/// Inputs to [`DrawUniform::new`] beyond the material.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DrawContext {
    pub shading: MeshShading,
    /// The mesh has tangents and the normal texture resolved.
    pub normal_map: bool,
    /// The dye mask texture resolved (for `DyeMask::Texture`).
    pub dye_mask: bool,
    /// The specular texture resolved.
    pub specular_tex: bool,
    /// Wireframe colour.
    pub overlay_color: [f32; 4],
    /// Vertex-normal line colour.
    pub normal_color: [f32; 4],
    pub normal_length: f32,
}

impl DrawUniform {
    pub(crate) fn new(model: Mat4, m: &Material, ctx: &DrawContext) -> Self {
        let n = normal_matrix(model);
        let mut flags = 0u32;
        if ctx.normal_map {
            flags |= feature::NORMAL_MAP;
        }
        let mut dye = [1.0; 4];
        let mut dye_source = 0u32;
        if let Some(d) = m.dye {
            match d.mask {
                DyeMask::Texture(_) if ctx.dye_mask => dye_source = 1,
                DyeMask::Texture(_) => {}
                DyeMask::BaseAlpha => dye_source = 2,
            }
            if dye_source != 0 {
                flags |= feature::DYE;
                dye = [
                    finite_or(d.color[0], 1.0),
                    finite_or(d.color[1], 1.0),
                    finite_or(d.color[2], 1.0),
                    1.0,
                ];
            }
        }
        let mut specular = [0.0, 0.0, 0.0, 1.0];
        if let Some(s) = m.specular {
            flags |= feature::SPECULAR;
            if ctx.specular_tex {
                flags |= feature::SPECULAR_TEX;
            }
            specular = [
                finite_or(s.color[0], 0.0),
                finite_or(s.color[1], 0.0),
                finite_or(s.color[2], 0.0),
                finite_or(s.shininess, 32.0).clamp(1.0, 1024.0),
            ];
        }
        if m.unlit {
            flags |= feature::UNLIT;
        }
        if m.two_sided {
            flags |= feature::TWO_SIDED;
        }
        if is_mirrored(model) {
            flags |= feature::MIRRORED;
        }
        let cutoff = match m.alpha {
            AlphaMode::Mask { cutoff } | AlphaMode::TestBlend { cutoff } => {
                finite_or(cutoff, DEFAULT_CUTOFF)
            }
            _ => 0.0,
        };
        Self {
            model: model.to_cols_array_2d(),
            normal: [
                n.x_axis.extend(0.0).to_array(),
                n.y_axis.extend(0.0).to_array(),
                n.z_axis.extend(0.0).to_array(),
            ],
            base_color: m.base_color.map(|c| finite_or(c, 1.0)),
            dye,
            specular,
            params: [
                cutoff,
                if m.normal_green_up { -1.0 } else { 1.0 },
                finite_or(ctx.normal_length, 0.0),
                OVERLAY_PULL,
            ],
            overlay_color: ctx.overlay_color.map(|c| finite_or(c, 1.0)),
            normal_color: ctx.normal_color.map(|c| finite_or(c, 1.0)),
            flags: [flags, mode_index(ctx.shading), dye_source, 0],
            lit: [0.0; 4],
            lit_lights: [0; 4],
            uv: crate::material::UvTransform::pack([crate::material::UvTransform::IDENTITY; 2]),
        }
    }
}

/// What a pipeline draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Kind {
    Opaque,
    Mask,
    Blend,
    Wire,
    NormalLines,
}

/// Face culling of a surface pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Cull {
    None,
    Back,
    Front,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MeshKey {
    pub format: wgpu::TextureFormat,
    pub kind: Kind,
    pub cull: Cull,
    /// Skin the vertices with the draw's palette.
    pub skinned: bool,
}

/// Layouts and shader shared by every mesh pipeline.
#[derive(Debug)]
pub(crate) struct MeshLayouts {
    pub draw: wgpu::BindGroupLayout,
    pub material: wgpu::BindGroupLayout,
    surface_layout: wgpu::PipelineLayout,
    overlay_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
}

/// The M2 pipelines read the first five attributes of `GpuVertex`; they derive the
/// bitangent and skip the stored binormal at offset 40 (render.md §2.1).
const VERTEX_ATTRS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    0 => Float32x3, 1 => Snorm16x4, 2 => Snorm16x4, 3 => Float32x2, 4 => Unorm8x4
];
const SKIN_ATTRS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
    5 => Uint8x4, 6 => Unorm16x4
];

impl MeshLayouts {
    pub(crate) fn new(device: &wgpu::Device, shared: &SharedLayouts) -> Self {
        let draw = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tessera-mesh-draw-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(size_of::<DrawUniform>() as u64),
                    },
                    count: None,
                },
                // Skinning palette: MAX_PALETTE_BONES affine 3 × 4 matrices.
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(PALETTE_BINDING_BYTES),
                    },
                    count: None,
                },
            ],
        });
        let mut entries = Vec::with_capacity(8);
        for binding in 0..4 {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
        }
        for binding in 4..8 {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });
        }
        let material = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tessera-mesh-material-bgl"),
            entries: &entries,
        });
        let surface_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tessera-mesh-layout"),
            bind_group_layouts: &[Some(&shared.globals), Some(&draw), Some(&material)],
            immediate_size: 0,
        });
        let overlay_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tessera-mesh-overlay-layout"),
            bind_group_layouts: &[Some(&shared.globals), Some(&draw)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tessera-mesh.wgsl"),
            source: wgpu::ShaderSource::Wgsl(MESH_WGSL.into()),
        });
        Self {
            draw,
            material,
            surface_layout,
            overlay_layout,
            shader,
        }
    }

    fn build(&self, device: &wgpu::Device, key: MeshKey) -> wgpu::RenderPipeline {
        let (vs, fs, topology, step) = match key.kind {
            Kind::Opaque | Kind::Mask | Kind::Blend if key.skinned => (
                "vs_skinned",
                match key.kind {
                    Kind::Opaque => "fs_opaque",
                    Kind::Mask => "fs_mask",
                    _ => "fs_blend",
                },
                wgpu::PrimitiveTopology::TriangleList,
                wgpu::VertexStepMode::Vertex,
            ),
            Kind::Wire if key.skinned => (
                "vs_wire_skinned",
                "fs_overlay",
                wgpu::PrimitiveTopology::LineList,
                wgpu::VertexStepMode::Vertex,
            ),
            Kind::NormalLines if key.skinned => (
                "vs_normals_skinned",
                "fs_normal_lines",
                wgpu::PrimitiveTopology::LineList,
                wgpu::VertexStepMode::Instance,
            ),
            Kind::Opaque => (
                "vs_main",
                "fs_opaque",
                wgpu::PrimitiveTopology::TriangleList,
                wgpu::VertexStepMode::Vertex,
            ),
            Kind::Mask => (
                "vs_main",
                "fs_mask",
                wgpu::PrimitiveTopology::TriangleList,
                wgpu::VertexStepMode::Vertex,
            ),
            Kind::Blend => (
                "vs_main",
                "fs_blend",
                wgpu::PrimitiveTopology::TriangleList,
                wgpu::VertexStepMode::Vertex,
            ),
            Kind::Wire => (
                "vs_wire",
                "fs_overlay",
                wgpu::PrimitiveTopology::LineList,
                wgpu::VertexStepMode::Vertex,
            ),
            Kind::NormalLines => (
                "vs_normals",
                "fs_normal_lines",
                wgpu::PrimitiveTopology::LineList,
                wgpu::VertexStepMode::Instance,
            ),
        };
        let blend = match key.kind {
            Kind::Opaque | Kind::Mask => None,
            Kind::Blend | Kind::Wire | Kind::NormalLines => Some(wgpu::BlendState::ALPHA_BLENDING),
        };
        let layout = match key.kind {
            Kind::Wire | Kind::NormalLines => &self.overlay_layout,
            _ => &self.surface_layout,
        };
        let cull_mode = match key.cull {
            Cull::None => None,
            Cull::Back => Some(wgpu::Face::Back),
            Cull::Front => Some(wgpu::Face::Front),
        };
        let label = format!(
            "tessera-mesh-{:?}-{:?}{}",
            key.kind,
            key.cull,
            if key.skinned { "-skinned" } else { "" }
        );
        let vertex_buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<GpuVertex>() as u64,
                step_mode: step,
                attributes: &VERTEX_ATTRS,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<GpuSkin>() as u64,
                step_mode: step,
                attributes: &SKIN_ATTRS,
            }),
        ];
        let buffers = if key.skinned {
            &vertex_buffers[..]
        } else {
            &vertex_buffers[..1]
        };
        let targets = [Some(wgpu::ColorTargetState {
            format: key.format,
            blend,
            write_mask: wgpu::ColorWrites::ALL,
        })];
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&label),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some(vs),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers,
            },
            primitive: wgpu::PrimitiveState {
                topology,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(key.kind != Kind::Blend),
                depth_compare: Some(crate::pipelines::DEPTH_COMPARE),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some(fs),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &targets,
            }),
            multiview_mask: None,
            cache: None,
        })
    }
}

/// Lazily built mesh pipeline variants.
#[derive(Debug, Default)]
pub(crate) struct MeshPipelineCache {
    pipelines: Vec<(MeshKey, wgpu::RenderPipeline)>,
}

impl MeshPipelineCache {
    pub(crate) fn ensure(&mut self, device: &wgpu::Device, layouts: &MeshLayouts, key: MeshKey) {
        if self.lookup(key).is_none() {
            tracing::debug!(?key, "building mesh pipeline");
            self.pipelines.push((key, layouts.build(device, key)));
        }
    }

    pub(crate) fn lookup(&self, key: MeshKey) -> Option<&wgpu::RenderPipeline> {
        self.pipelines
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, p)| p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::{Dye, Specular};
    use glam::Vec3;

    #[test]
    fn mesh_shader_validates() {
        let module = wgpu::naga::front::wgsl::parse_str(MESH_WGSL)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(MESH_WGSL)));
        let mut v = wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        );
        if let Err(e) = v.validate(&module) {
            panic!("{}", e.emit_to_string(MESH_WGSL));
        }
    }

    #[test]
    fn uniform_layout() {
        // 224 bytes of M2 fields, the two lit vectors and the three vectors of the texture
        // matrices (material.md §8.4).
        assert_eq!(size_of::<DrawUniform>(), 304);
    }

    fn ctx() -> DrawContext {
        DrawContext {
            shading: MeshShading::Lit,
            normal_map: false,
            dye_mask: false,
            specular_tex: false,
            overlay_color: [1.0; 4],
            normal_color: [1.0; 4],
            normal_length: 0.1,
        }
    }

    #[test]
    fn flags_follow_material_and_resolution() {
        let m = Material {
            dye: Some(Dye {
                color: [1.0, 0.0, 0.0],
                mask: DyeMask::BaseAlpha,
            }),
            specular: Some(Specular {
                color: [1.0; 3],
                shininess: f32::NAN,
                texture: None,
            }),
            two_sided: true,
            alpha: AlphaMode::Mask { cutoff: f32::NAN },
            ..Material::default()
        };
        let u = DrawUniform::new(Mat4::IDENTITY, &m, &ctx());
        assert_eq!(
            u.flags[0],
            feature::DYE | feature::SPECULAR | feature::TWO_SIDED
        );
        assert_eq!(u.flags[2], 2);
        assert_eq!(u.params[0], DEFAULT_CUTOFF);
        assert_eq!(u.specular[3], 32.0);

        // A texture dye mask that did not resolve disables dyeing.
        let tex_dye = Material {
            dye: Some(Dye {
                color: [1.0, 0.0, 0.0],
                mask: DyeMask::Texture(crate::TextureId::dangling()),
            }),
            ..Material::default()
        };
        let u = DrawUniform::new(Mat4::IDENTITY, &tex_dye, &ctx());
        assert_eq!(u.flags[0] & feature::DYE, 0);

        let mirrored = DrawUniform::new(
            Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0)),
            &Material::default(),
            &ctx(),
        );
        assert_eq!(mirrored.flags[0], feature::MIRRORED);
    }

    #[test]
    fn normal_matrix_handles_scale_and_singular() {
        let n = normal_matrix(Mat4::from_scale(Vec3::new(2.0, 1.0, 1.0)));
        assert!((n.x_axis.x - 0.5).abs() < 1e-6);
        let s = normal_matrix(Mat4::from_scale(Vec3::new(0.0, 1.0, 1.0)));
        assert!(s.is_finite());
        let nan = normal_matrix(Mat4::from_cols_array(&[f32::NAN; 16]));
        assert_eq!(nan, Mat3::IDENTITY);
    }
}
