// SPDX-License-Identifier: GPL-3.0-or-later
//! The sky (M4.5, `docs/specs/environment.md` §11.4, §12; `docs/specs/render.md` §5.4).
//!
//! [`Pass::sky`] draws the visible sky objects of a frame around the camera: each a
//! [`WorldModel`] (the geometry of the object's 0x47 record, uploaded by the caller),
//! turned by its two angles, tinted by its keyframe tint (alpha = opacity), unlit and
//! without fog, behind everything else and without writing depth (`environment.md` §12,
//! Tessera Decisions where the original is OPEN). The angles, tints and visibility come
//! from the caller's environment blending (`environment.md` §11.3–§11.4).
//!
//! Placement (Tessera Decision of `environment.md` §12: *B* is the azimuth of the object's
//! arc and *A* the rotation along it): a model vertex *v* is drawn at
//! eye + R_z(−*B*) · R_x(*A*) · (scale · *v*), so with *A* = *B* = 0 the model keeps its
//! own axes (x east, y north, z up); *A* turns the model about the east axis (+y toward
//! +z) and *B* then turns the result clockwise seen from above (north toward east), like
//! the heading of `environment.md` §11.3 step 6. The sky is drawn at the far plane, so it
//! covers only pixels nothing else has drawn, whether it is queued before or after the
//! world in the same pass or in an earlier pass.

use std::fmt;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use glam::Mat3;
use wgpu::util::DeviceExt;

use crate::error::{RenderError, Result};
use crate::frame::{Pass, Rgba};
use crate::instanced::{
    ADDITIVE, InstancedState, SurfaceKind, WorldModel, common_source, lit_source,
    mesh_vertex_layout, uniform_entry, uniform_stride,
};
use crate::pass_hook::{HookRecord, HookStage, PassHook};
use crate::pipelines::DEPTH_FORMAT;
use crate::renderer::{FrameStats, WorldFrame};

const SKY_WGSL: &str = include_str!("shaders/sky.wgsl");

/// Largest number of sky objects per pass and frame (the data has at most 136 per day
/// description, `environment.md` §5.4).
pub const MAX_SKY_OBJECTS: usize = 4096;

/// One visible sky object ([`SkyDraw`]).
#[derive(Debug, Clone)]
pub struct SkyObject {
    /// The object's geometry and surfaces.
    pub model: WorldModel,
    /// Angle *A* in degrees: the rotation along the arc (`environment.md` §11.4 step 3,
    /// plus the keyframe offsets of step 4).
    pub angle_a: f32,
    /// Angle *B* in degrees: the azimuth of the arc.
    pub angle_b: f32,
    /// Keyframe tint; alpha is the object's opacity (`environment.md` §11.4 step 4).
    pub tint: Rgba,
    /// Uniform scale of the model (1: as stored).
    pub scale: f32,
}

impl SkyObject {
    /// An untinted object at angles (*A*, *B*) in degrees.
    #[must_use]
    pub fn new(model: WorldModel, angle_a: f32, angle_b: f32) -> Self {
        Self {
            model,
            angle_a,
            angle_b,
            tint: [1.0; 4],
            scale: 1.0,
        }
    }

    /// The placement rotation R_z(−*B*) · R_x(*A*) (non-finite angles count as 0).
    #[must_use]
    pub fn rotation(&self) -> Mat3 {
        sky_rotation(self.angle_a, self.angle_b)
    }
}

/// R_z(−*B*) · R_x(*A*) for angles in degrees; non-finite angles count as 0.
fn sky_rotation(angle_a: f32, angle_b: f32) -> Mat3 {
    let f = |d: f32| if d.is_finite() { d.to_radians() } else { 0.0 };
    Mat3::from_rotation_z(-f(angle_b)) * Mat3::from_rotation_x(f(angle_a))
}

/// The sky objects of one frame, drawn in order (later objects over earlier ones).
#[derive(Debug, Clone, Default)]
pub struct SkyDraw {
    /// Visible objects, back to front.
    pub objects: Vec<SkyObject>,
}

impl Pass {
    /// Draw the sky (`docs/specs/render.md` §5.4). Needs a lit pass
    /// ([`Pass::frame_lighting`]); ignored by screen passes.
    pub fn sky(&mut self, sky: SkyDraw) -> &mut Self {
        self.world.sky.push(sky);
        self
    }
}

/// Per-object uniform (must match `SkyObject` in `sky.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct SkyUniform {
    /// Columns of R · scale.
    c0: [f32; 4],
    c1: [f32; 4],
    c2: [f32; 4],
    tint: [f32; 4],
}

/// Pipelines of the sky, built lazily.
#[derive(Debug, Default)]
pub(crate) struct SkyState {
    object_layout: Option<wgpu::BindGroupLayout>,
    shader: Option<wgpu::ShaderModule>,
    layout: Option<wgpu::PipelineLayout>,
    pipelines: Vec<((wgpu::TextureFormat, bool, bool), wgpu::RenderPipeline)>,
}

impl SkyState {
    fn pipeline(
        &mut self,
        ctx: &WorldFrame<'_>,
        models: &mut InstancedState,
        additive: bool,
        masked: bool,
    ) -> Option<(wgpu::RenderPipeline, wgpu::BindGroupLayout)> {
        let device = ctx.device;
        let surface = models.layouts(device);
        let object_layout = self
            .object_layout
            .get_or_insert_with(|| {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("tessera-sky-object-bgl"),
                    entries: &[uniform_entry(
                        0,
                        wgpu::ShaderStages::VERTEX_FRAGMENT,
                        true,
                        size_of::<SkyUniform>(),
                    )],
                })
            })
            .clone();
        let shader = self
            .shader
            .get_or_insert_with(|| {
                device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("tessera-sky.wgsl"),
                    source: wgpu::ShaderSource::Wgsl(sky_source().into()),
                })
            })
            .clone();
        let layout = self
            .layout
            .get_or_insert_with(|| {
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("tessera-sky-layout"),
                    bind_group_layouts: &[
                        Some(ctx.frame_layout),
                        Some(&object_layout),
                        Some(&surface.surface),
                    ],
                    immediate_size: 0,
                })
            })
            .clone();
        let key = (ctx.format, additive, masked);
        if let Some((_, p)) = self.pipelines.iter().find(|(k, _)| *k == key) {
            return Some((p.clone(), object_layout));
        }
        let blend = if additive {
            ADDITIVE
        } else {
            wgpu::BlendState::ALPHA_BLENDING
        };
        let p = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tessera-sky"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_sky"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(mesh_vertex_layout())],
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            // At the far plane (depth 0 under reversed depth): covers only what nothing
            // else has drawn; writes no depth.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::GreaterEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(if masked { "fs_sky_mask" } else { "fs_sky" }),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: ctx.format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.push((key, p.clone()));
        Some((p, object_layout))
    }
}

pub(crate) fn sky_source() -> String {
    lit_source(&[common_source(), SKY_WGSL])
}

#[derive(Debug)]
struct SkyItem {
    model: WorldModel,
    part: usize,
    surface_bg: wgpu::BindGroup,
    offset: u32,
    pipeline: wgpu::RenderPipeline,
}

struct SkyHook {
    items: Vec<SkyItem>,
    objects: u32,
    bind_group: wgpu::BindGroup,
}

impl fmt::Debug for SkyHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SkyHook")
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}

pub(crate) fn build_hook(
    state: &mut SkyState,
    models: &mut InstancedState,
    draws: Vec<SkyDraw>,
    ctx: &WorldFrame<'_>,
) -> Result<Option<Arc<dyn PassHook>>> {
    let total: usize = draws.iter().map(|d| d.objects.len()).sum();
    if total == 0 {
        return Ok(None);
    }
    if total > MAX_SKY_OBJECTS {
        return Err(RenderError::LimitExceeded {
            what: "sky objects per pass",
            requested: total as u64,
            max: MAX_SKY_OBJECTS as u64,
        });
    }
    let stride = uniform_stride(ctx.device, size_of::<SkyUniform>());
    let mut data: Vec<u8> = Vec::with_capacity(total * stride as usize);
    let mut items = Vec::new();
    let mut objects = 0u32;
    let mut object_layout = None;
    let mut warned = false;
    for obj in draws.iter().flat_map(|d| &d.objects) {
        let model = obj.model.gpu();
        if model.owner != ctx.owner {
            if !warned {
                tracing::warn!("sky object model of another renderer skipped");
                warned = true;
            }
            continue;
        }
        let tint = obj.tint.map(|c| if c.is_finite() { c } else { 1.0 });
        if tint[3] <= 0.0 {
            continue;
        }
        let scale = if obj.scale.is_finite() {
            obj.scale
        } else {
            1.0
        };
        let r = sky_rotation(obj.angle_a, obj.angle_b) * scale;
        let u = SkyUniform {
            c0: r.x_axis.extend(0.0).to_array(),
            c1: r.y_axis.extend(0.0).to_array(),
            c2: r.z_axis.extend(0.0).to_array(),
            tint,
        };
        let offset = u32::try_from(data.len()).unwrap_or(u32::MAX);
        data.extend_from_slice(bytemuck::bytes_of(&u));
        data.resize(data.len() + (stride as usize - size_of::<SkyUniform>()), 0);
        objects += 1;
        for (pi, part) in model.parts.iter().enumerate() {
            let Some(surface) = model.surfaces.get(part.surface) else {
                continue;
            };
            let additive = surface.kind == SurfaceKind::Additive;
            let masked = matches!(surface.kind, SurfaceKind::Mask | SurfaceKind::TestBlend);
            let Some((pipeline, layout)) = state.pipeline(ctx, models, additive, masked) else {
                continue;
            };
            object_layout.get_or_insert(layout);
            let surface_layout = models.layouts(ctx.device);
            items.push(SkyItem {
                model: obj.model.clone(),
                part: pi,
                surface_bg: surface.bind_group(ctx.device, &surface_layout.surface, &ctx.textures),
                offset,
                pipeline,
            });
        }
    }
    let Some(layout) = object_layout else {
        return Ok(None);
    };
    let buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tessera-sky-objects"),
            contents: &data,
            usage: wgpu::BufferUsages::UNIFORM,
        });
    let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("tessera-sky-objects"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: wgpu::BufferSize::new(size_of::<SkyUniform>() as u64),
            }),
        }],
    });
    Ok(Some(Arc::new(SkyHook {
        items,
        objects,
        bind_group,
    })))
}

impl PassHook for SkyHook {
    fn record(&self, rp: &mut wgpu::RenderPass<'_>, ctx: &HookRecord<'_>, stats: &mut FrameStats) {
        if ctx.stage != HookStage::Opaque {
            return;
        }
        let Some((group0, offset)) = ctx.group0 else {
            return;
        };
        rp.set_bind_group(0, group0, &[offset]);
        for item in &self.items {
            let model = item.model.gpu();
            let Some(part) = model.parts.get(item.part) else {
                continue;
            };
            rp.set_pipeline(&item.pipeline);
            rp.set_vertex_buffer(0, model.vertices.slice(..));
            rp.set_index_buffer(model.indices.slice(..), wgpu::IndexFormat::Uint32);
            rp.set_bind_group(1, &self.bind_group, &[item.offset]);
            rp.set_bind_group(2, &item.surface_bg, &[]);
            rp.draw_indexed(part.indices.clone(), 0, 0..1);
            stats.draw_calls = stats.draw_calls.saturating_add(1);
        }
        stats.sky_objects = stats.sky_objects.saturating_add(self.objects);
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;

    #[test]
    fn sky_shader_validates() {
        let src = sky_source();
        let module = wgpu::naga::front::wgsl::parse_str(&src)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
    }

    #[test]
    fn uniform_is_four_vectors() {
        assert_eq!(size_of::<SkyUniform>(), 64);
    }

    #[test]
    fn rotation_follows_the_decision() {
        // The function `SkyObject::rotation` and the hook both use.
        let r = sky_rotation;
        // A turns +y (north) toward +z (up).
        let v = r(90.0, 0.0) * Vec3::Y;
        assert!(v.abs_diff_eq(Vec3::Z, 1e-6), "{v}");
        // B turns north toward east, like a heading.
        let v = r(0.0, 90.0) * Vec3::Y;
        assert!(v.abs_diff_eq(Vec3::X, 1e-6), "{v}");
        // Heading 90 and pitch 30 of a sun drawn on the +y axis.
        let v = r(30.0, 90.0) * Vec3::Y;
        let e = 30f32.to_radians();
        assert!(v.abs_diff_eq(Vec3::new(e.cos(), 0.0, e.sin()), 1e-6), "{v}");
        // Non-finite angles count as 0.
        assert!(r(f32::NAN, f32::INFINITY).abs_diff_eq(Mat3::IDENTITY, 0.0));
        assert!(r(f32::NAN, 90.0).abs_diff_eq(r(0.0, 90.0), 0.0));
    }
}
