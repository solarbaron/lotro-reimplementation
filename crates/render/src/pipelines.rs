// SPDX-License-Identifier: GPL-3.0-or-later
//! Built-in pipelines: instanced textured quads and debug lines.

use bytemuck::{Pod, Zeroable};

/// Depth buffer format used by world passes.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

const QUAD_WGSL: &str = include_str!("shaders/quad.wgsl");
const LINE_WGSL: &str = include_str!("shaders/line.wgsl");

/// Per-pass uniform block (must match `Globals` in the shaders).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct Globals {
    pub view_proj: [[f32; 4]; 4],
}

/// Per-instance quad data (must match `InstanceIn` in `quad.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct QuadInstance {
    pub model: [[f32; 4]; 4],
    pub color: [f32; 4],
    pub uv_rect: [f32; 4],
}

/// One debug-line vertex (must match `VsIn` in `line.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct LineVertex {
    /// Position in the pass's space.
    pub position: [f32; 3],
    /// Linear RGBA colour.
    pub color: [f32; 4],
}

const QUAD_ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
    0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4,
    4 => Float32x4, 5 => Float32x4
];
const LINE_ATTRS: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];

/// Bind group layouts and shader modules shared by every pipeline variant.
#[derive(Debug)]
pub(crate) struct SharedLayouts {
    pub globals: wgpu::BindGroupLayout,
    pub texture: wgpu::BindGroupLayout,
    quad_layout: wgpu::PipelineLayout,
    line_layout: wgpu::PipelineLayout,
    quad_shader: wgpu::ShaderModule,
    line_shader: wgpu::ShaderModule,
}

impl SharedLayouts {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let globals = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tessera-globals-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(size_of::<Globals>() as u64),
                },
                count: None,
            }],
        });
        let texture = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tessera-texture-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let quad_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tessera-quad-layout"),
            bind_group_layouts: &[Some(&globals), Some(&texture)],
            immediate_size: 0,
        });
        let line_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tessera-line-layout"),
            bind_group_layouts: &[Some(&globals)],
            immediate_size: 0,
        });
        let quad_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tessera-quad.wgsl"),
            source: wgpu::ShaderSource::Wgsl(QUAD_WGSL.into()),
        });
        let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tessera-line.wgsl"),
            source: wgpu::ShaderSource::Wgsl(LINE_WGSL.into()),
        });
        Self {
            globals,
            texture,
            quad_layout,
            line_layout,
            quad_shader,
            line_shader,
        }
    }
}

/// Pipelines for one (colour format, depth on/off) combination.
#[derive(Debug)]
pub(crate) struct PipelineSet {
    pub quad: wgpu::RenderPipeline,
    pub line: wgpu::RenderPipeline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Key {
    format: wgpu::TextureFormat,
    depth: bool,
}

/// Lazily built pipeline variants.
#[derive(Debug, Default)]
pub(crate) struct PipelineCache {
    sets: Vec<(Key, PipelineSet)>,
}

impl PipelineCache {
    pub(crate) fn get(
        &mut self,
        device: &wgpu::Device,
        shared: &SharedLayouts,
        format: wgpu::TextureFormat,
        depth: bool,
    ) -> &PipelineSet {
        let key = Key { format, depth };
        let idx = match self.sets.iter().position(|(k, _)| *k == key) {
            Some(i) => i,
            None => {
                tracing::debug!(?format, depth, "building pipeline set");
                self.sets.push((key, build_set(device, shared, key)));
                self.sets.len() - 1
            }
        };
        &self.sets[idx].1
    }

    pub(crate) fn lookup(&self, format: wgpu::TextureFormat, depth: bool) -> Option<&PipelineSet> {
        let key = Key { format, depth };
        self.sets.iter().find(|(k, _)| *k == key).map(|(_, s)| s)
    }
}

fn depth_state(depth: bool) -> Option<wgpu::DepthStencilState> {
    depth.then(|| wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(true),
        depth_compare: Some(wgpu::CompareFunction::LessEqual),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    })
}

fn build_set(device: &wgpu::Device, shared: &SharedLayouts, key: Key) -> PipelineSet {
    let target = [Some(wgpu::ColorTargetState {
        format: key.format,
        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
        write_mask: wgpu::ColorWrites::ALL,
    })];

    let quad = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("tessera-quad"),
        layout: Some(&shared.quad_layout),
        vertex: wgpu::VertexState {
            module: &shared.quad_shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<QuadInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &QUAD_ATTRS,
            })],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: depth_state(key.depth),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shared.quad_shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &target,
        }),
        multiview_mask: None,
        cache: None,
    });

    let line = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("tessera-line"),
        layout: Some(&shared.line_layout),
        vertex: wgpu::VertexState {
            module: &shared.line_shader,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<LineVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &LINE_ATTRS,
            })],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: depth_state(key.depth),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shared.line_shader,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &target,
        }),
        multiview_mask: None,
        cache: None,
    });

    PipelineSet { quad, line }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shaders_parse_and_validate() {
        for (name, src) in [("quad", QUAD_WGSL), ("line", LINE_WGSL)] {
            let module = wgpu::naga::front::wgsl::parse_str(src)
                .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(src)));
            let mut v = wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::empty(),
            );
            if let Err(e) = v.validate(&module) {
                panic!("{name}: {}", e.emit_to_string(src));
            }
        }
    }

    #[test]
    fn instance_layouts_match_shader_expectations() {
        assert_eq!(size_of::<QuadInstance>(), 96);
        assert_eq!(size_of::<LineVertex>(), 28);
        assert_eq!(size_of::<Globals>(), 64);
    }
}
