// SPDX-License-Identifier: GPL-3.0-or-later
//! Crate-internal extension point for pipelines outside the mesh path that draw into world
//! passes: the render-world modules (terrain layers, water, sky, instanced scenery,
//! `docs/specs/render.md` §4.2).
//!
//! A module adds an [`Arc<dyn PassHook>`](PassHook) to a [`Pass`](crate::Pass) through
//! `Pass::hook` (usually from a public `impl Pass { pub fn water(..) }` block in its own
//! file). `Renderer::end_frame` then
//!
//! 1. calls [`PassHook::prepare`] once per pass during the gather phase, after the pass's
//!    frame uniform is known (build pipelines for the target format, upload per-frame
//!    data), and
//! 2. calls [`PassHook::record`] in each [`HookStage`] of the pass: in the sun's
//!    shadow-map pass ([`HookStage::Shadow`], only for lit passes with a shadow), after
//!    the opaque and alpha-tested mesh surfaces ([`HookStage::Opaque`]), after the
//!    overlays, before the back-to-front blended mesh draws ([`HookStage::Translucent`]),
//!    and after those blended draws ([`HookStage::Post`], the pass's last stage).
//!
//! A hook sets its own pipeline and every bind group it uses, group 0 included: the
//! context hands it the pass's group 0 for the stage with its dynamic offset. The renderer
//! re-binds its own state after the hook returns.

use std::fmt;

use glam::{Mat4, Vec3};

use crate::error::Result;
use crate::lighting::FrameUniform;
use crate::material::MeshShading;
use crate::renderer::FrameStats;

/// Where in a world pass a hook records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum HookStage {
    /// The sun's shadow-map pass of a lit pass (depth only, `SHADOW_FORMAT` attachment,
    /// no colour target). Group 0 is the shadow-pass frame group (binding 0 only).
    Shadow,
    /// After the opaque and alpha-tested mesh surfaces, before quads and lines.
    Opaque,
    /// After the overlays, before the sorted blended mesh draws.
    Translucent,
    /// After the sorted blended mesh draws: the last stage of a world pass, after every
    /// other draw of it (the far view and the translucent stage included). Only the
    /// particle systems of the pass after post-processing record here
    /// (`particle-runtime.md` §12 rule 16, [`crate::Pass::post_particles`]).
    Post,
}

/// Inputs of [`PassHook::prepare`].
#[derive(Debug)]
// Some fields serve only the render-world modules that are still to come.
#[allow(dead_code)]
pub(crate) struct HookPrepare<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    /// Colour target format of the frame.
    pub format: wgpu::TextureFormat,
    /// Group-0 layout of lit colour passes (frame uniform, shadow map, sampler).
    pub frame_layout: &'a wgpu::BindGroupLayout,
    /// Group-0 layout of shadow-map passes (frame uniform only).
    pub shadow_frame_layout: &'a wgpu::BindGroupLayout,
    /// World → clip of the pass (reversed depth).
    pub view_proj: Mat4,
    /// Camera position of the pass.
    pub eye: Vec3,
    /// The pass's frame uniform when the pass is lit (its `has_shadow` says whether a
    /// shadow-map pass will be recorded).
    pub frame: Option<&'a FrameUniform>,
    /// The pass's mesh shading (debug views draw meshes through the M2 pipelines; a hook
    /// may follow suit).
    pub shading: MeshShading,
}

/// Inputs of [`PassHook::record`].
#[derive(Debug)]
// Some fields serve only the render-world modules that are still to come.
#[allow(dead_code)]
pub(crate) struct HookRecord<'a> {
    pub stage: HookStage,
    /// Group 0 for this stage with the pass's dynamic offset: the frame bind group in the
    /// colour stages, the shadow-pass frame group in [`HookStage::Shadow`]. `None` in
    /// passes without frame lighting. In a lit pass with a debug shading the frame
    /// uniform has no shadow (quality 0), so nothing samples the shadow map.
    pub group0: Option<(&'a wgpu::BindGroup, u32)>,
    /// Colour target format of the frame.
    pub format: wgpu::TextureFormat,
    /// World → clip of the pass.
    pub view_proj: Mat4,
    /// Camera position of the pass.
    pub eye: Vec3,
    /// The pass's mesh shading.
    pub shading: MeshShading,
}

/// A recorder of draws into world passes (see the module docs).
pub(crate) trait PassHook: fmt::Debug + Send + Sync {
    /// Gather phase, once per pass that holds the hook. An error aborts `end_frame`.
    fn prepare(&self, ctx: &HookPrepare<'_>) -> Result<()> {
        let _ = ctx;
        Ok(())
    }

    /// Record the hook's draws for `ctx.stage`.
    fn record(&self, rp: &mut wgpu::RenderPass<'_>, ctx: &HookRecord<'_>, stats: &mut FrameStats);
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, PoisonError};

    use super::*;
    use crate::{
        Camera, FrameLighting, MeshDebug, OffscreenFormat, Renderer, RendererConfig, SunLight,
        SunShadow,
    };

    /// Draws a full-screen triangle whose colour is the frame's sun colour (read through
    /// group 0 at the pass's offset), and counts the stages it was called in.
    #[derive(Debug, Default)]
    struct SunColourHook {
        pipelines: Mutex<Option<(wgpu::RenderPipeline, wgpu::RenderPipeline)>>,
        calls: Mutex<Vec<(HookStage, bool)>>,
    }

    const HOOK_WGSL: &str = r"
@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    // Behind everything (reversed depth: 0 is far).
    return vec4<f32>(p, 0.0, 1.0);
}

@fragment
fn fs() -> @location(0) vec4<f32> {
    return vec4<f32>(frame.sun_color.rgb, 1.0);
}

@vertex
fn vs_shadow(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    return frame.shadow_view_proj * vec4<f32>(p * 1e-3, 0.0, 1.0);
}
";

    impl PassHook for SunColourHook {
        fn prepare(&self, ctx: &HookPrepare<'_>) -> Result<()> {
            let mut p = self
                .pipelines
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if p.is_some() {
                return Ok(());
            }
            let src = format!("{}\n{}", crate::FRAME_WGSL, HOOK_WGSL);
            let module = ctx
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("hook-test"),
                    source: wgpu::ShaderSource::Wgsl(src.into()),
                });
            let layout = |bgl: &wgpu::BindGroupLayout| {
                ctx.device
                    .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: None,
                        bind_group_layouts: &[Some(bgl)],
                        immediate_size: 0,
                    })
            };
            let colour_layout = layout(ctx.frame_layout);
            let shadow_layout = layout(ctx.shadow_frame_layout);
            let depth = |format| {
                Some(wgpu::DepthStencilState {
                    format,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                })
            };
            let colour = ctx
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("hook-test-colour"),
                    layout: Some(&colour_layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("vs"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: depth(crate::DEPTH_FORMAT),
                    multisample: wgpu::MultisampleState::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some("fs"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(ctx.format.into())],
                    }),
                    multiview_mask: None,
                    cache: None,
                });
            let shadow = ctx
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("hook-test-shadow"),
                    layout: Some(&shadow_layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("vs_shadow"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: depth(crate::lit_pipelines::SHADOW_FORMAT),
                    multisample: wgpu::MultisampleState::default(),
                    fragment: None,
                    multiview_mask: None,
                    cache: None,
                });
            *p = Some((colour, shadow));
            Ok(())
        }

        fn record(
            &self,
            rp: &mut wgpu::RenderPass<'_>,
            ctx: &HookRecord<'_>,
            stats: &mut FrameStats,
        ) {
            self.calls
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((ctx.stage, ctx.group0.is_some()));
            let (Some((bg, offset)), Some((colour, shadow))) = (
                ctx.group0,
                &*self
                    .pipelines
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner),
            ) else {
                return;
            };
            match ctx.stage {
                HookStage::Opaque => rp.set_pipeline(colour),
                HookStage::Shadow => rp.set_pipeline(shadow),
                HookStage::Translucent | HookStage::Post => return,
            }
            rp.set_bind_group(0, bg, &[offset]);
            rp.draw(0..3, 0..1);
            stats.draw_calls = stats.draw_calls.saturating_add(1);
        }
    }

    fn renderer() -> Option<Renderer<'static>> {
        let config = RendererConfig {
            context: crate::ContextOptions {
                allow_software: true,
                ..crate::ContextOptions::default()
            },
            ..RendererConfig::default()
        };
        match Renderer::new_headless(&config) {
            Ok(r) => Some(r),
            Err(e) => {
                eprintln!("SKIPPED: no GPU renderer: {e}");
                None
            }
        }
    }

    fn lighting(colour: f32, shadow: bool) -> FrameLighting {
        FrameLighting {
            sun: Some(SunLight {
                direction: Vec3::Z,
                color: [colour; 3],
                specular: [0.0; 3],
            }),
            shadow: shadow.then(|| SunShadow::fit(Vec3::Z, Vec3::ZERO, 4.0, 50.0, 256)),
            ..FrameLighting::default()
        }
    }

    #[test]
    fn hooks_get_group0_at_the_pass_offset_in_every_stage() -> Result<()> {
        let Some(mut r) = renderer() else {
            return Ok(());
        };
        let target = r.create_offscreen_target(8, 8, OffscreenFormat::Rgba8Unorm)?;
        let hook = Arc::new(SunColourHook::default());
        let mut frame = r.begin_offscreen_frame(&target)?;
        let camera = Camera::default();
        // Three lit passes with different sun colours: the hook must see each pass's own
        // uniform (the last pass wins the pixel). The second pass has no shadow, the
        // third a debug shading (no frame group, no shadow pass).
        frame
            .world_pass("a", &camera)
            .frame_lighting(lighting(0.2, true))
            .hook(hook.clone());
        frame
            .world_pass("b", &camera)
            .frame_lighting(lighting(0.6, false))
            .hook(hook.clone());
        frame.world_pass("m2", &camera).hook(hook.clone());
        let stats = r.end_frame(frame)?;
        assert_eq!(stats.shadow_passes, 1);
        let px = r.read_pixels(&target)?.pixel(4, 4);
        // Pass "m2" has no frame lighting: the hook gets no group 0 and draws nothing.
        assert_eq!(px, Some([153, 153, 153, 255]));
        let calls = std::mem::take(&mut *hook.calls.lock().unwrap_or_else(PoisonError::into_inner));
        assert_eq!(
            calls,
            vec![
                (HookStage::Shadow, true),
                (HookStage::Opaque, true),
                (HookStage::Translucent, true),
                (HookStage::Post, true),
                (HookStage::Opaque, true),
                (HookStage::Translucent, true),
                (HookStage::Post, true),
                (HookStage::Opaque, false),
                (HookStage::Translucent, false),
                (HookStage::Post, false),
            ]
        );

        // A lit pass with a debug shading: no shadow pass, the frame group without a
        // shadow.
        let mut frame = r.begin_offscreen_frame(&target)?;
        frame
            .world_pass("debug", &camera)
            .frame_lighting(lighting(0.2, true))
            .mesh_debug(MeshDebug {
                shading: MeshShading::Normals,
                ..MeshDebug::default()
            })
            .hook(hook.clone());
        let stats = r.end_frame(frame)?;
        assert_eq!(stats.shadow_passes, 0);
        let calls = std::mem::take(&mut *hook.calls.lock().unwrap_or_else(PoisonError::into_inner));
        assert_eq!(
            calls,
            vec![
                (HookStage::Opaque, true),
                (HookStage::Translucent, true),
                (HookStage::Post, true)
            ]
        );
        Ok(())
    }
}
