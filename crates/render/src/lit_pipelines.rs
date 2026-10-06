// SPDX-License-Identifier: GPL-3.0-or-later
//! The lit mesh pipeline (M4.5) and the sun's shadow-map pass: layouts, the lit fields of
//! the draw uniform, automatic light selection and the lazily built variants
//! (kind × cull × target format × static or skinned).
//!
//! Group 0 is the pass's frame bind group ([`crate::lighting`]): the [`FrameUniform`]
//! at the pass's dynamic offset, the shadow map and its comparison sampler. The shadow
//! pass binds a frame group without the shadow map (it renders into it). Group 1 is the
//! draw bind group shared with the M2 mesh pipelines (draw uniform + skinning palette).
//! Group 2 holds the seven material textures of [`LIT_TEXTURES`].

use glam::{Mat4, Vec3};

use crate::lighting::{FRAME_SHADING_WGSL, FRAME_WGSL, FrameUniform, MAX_DRAW_LIGHTS};
use crate::material::{Gloss, LightSelection, LitSurface, Material, Opacity};
use crate::mesh::GpuVertex;
use crate::mesh_pipelines::{Cull, DrawUniform, MeshLayouts};
use crate::pipelines::{DEPTH_COMPARE, DEPTH_FORMAT};
use crate::skin::GpuSkin;

const LIT_COMMON_WGSL: &str = include_str!("shaders/lit_common.wgsl");
const LIT_WGSL: &str = include_str!("shaders/lit.wgsl");
const SHADOW_WGSL: &str = include_str!("shaders/shadow.wgsl");

/// Material textures of the lit pipeline, in binding order (samplers follow at
/// `binding + LIT_TEXTURES`).
pub(crate) const LIT_TEXTURES: usize = 7;

/// Depth format of the sun's shadow map (standard depth: 0 nearest the light).
pub(crate) const SHADOW_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Lit flags of `DrawUniform::lit_lights[3]` (must match `LIT_*` in `lit_common.wgsl`).
pub(crate) mod lit_flag {
    pub const VERTEX_COLORS: u32 = 1;
    pub const FOG: u32 = 2;
    pub const EMISSIVE: u32 = 4;
    pub const GLOW: u32 = 8;
    pub const SPECULAR: u32 = 16;
    pub const GLOSS_SHIFT: u32 = 5;
    pub const OPACITY_SHIFT: u32 = 7;
    /// sRGB re-encode bit of material texture `i` (binding order).
    pub const SRGB_SHIFT: u32 = 9;
}

/// The lit shader source.
pub(crate) fn lit_shader_source() -> String {
    format!("{FRAME_WGSL}\n{FRAME_SHADING_WGSL}\n{LIT_COMMON_WGSL}\n{LIT_WGSL}")
}

/// The shadow-pass shader source.
pub(crate) fn shadow_shader_source() -> String {
    format!("{FRAME_WGSL}\n{LIT_COMMON_WGSL}\n{SHADOW_WGSL}")
}

/// The textures one lit draw samples, resolved (missing ones are the white texture).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct LitTextures(pub [crate::TextureId; LIT_TEXTURES]);

/// Inputs to [`lit_fields`] beyond the material.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LitContext {
    /// Which material textures resolved (binding order).
    pub present: [bool; LIT_TEXTURES],
    /// Which resolved material textures decode sRGB when sampled.
    pub srgb: [bool; LIT_TEXTURES],
}

/// Specular powers below this upload a black specular colour (`lighting.md` §5.8).
const MIN_SPECULAR_POWER: f32 = 0.5;
/// Largest specular power accepted (larger values only sharpen a highlight that is
/// already sub-pixel).
const MAX_SPECULAR_POWER: f32 = 4096.0;

/// `DrawUniform::flags[2]` value of a dye masked by the base texture's alpha (must match
/// `DYE_BASE_ALPHA` in `lit_common.wgsl` and the M2 dye source).
const DYE_SOURCE_BASE_ALPHA: u32 = 2;

/// Fill the lit fields of `u` (glow, power, flags, normal-map sign) for `material` and
/// `lit`; `lights` are the light-table indices chosen for the draw. `u` must come from
/// `DrawUniform::new` for the same material (its dye source decides the opacity source).
pub(crate) fn lit_fields(
    u: &mut DrawUniform,
    material: &Material,
    lit: &LitSurface,
    ctx: &LitContext,
    lights: &[u8],
) {
    let f = |v: f32| if v.is_finite() { v } else { 0.0 };
    let mut flags = 0u32;
    if lit.vertex_colors {
        flags |= lit_flag::VERTEX_COLORS;
    }
    if lit.fog {
        flags |= lit_flag::FOG;
    }
    if lit.emissive.is_some() && ctx.present[4] {
        flags |= lit_flag::EMISSIVE;
    }
    if lit.glow_mask.is_some() && ctx.present[5] {
        flags |= lit_flag::GLOW;
    }
    let mut power = 0.0;
    if let Some(s) = material.specular {
        let p = f(s.shininess);
        if p >= MIN_SPECULAR_POWER {
            flags |= lit_flag::SPECULAR;
            power = p.min(MAX_SPECULAR_POWER);
        }
        // Black below 0.5 (lighting.md §5.8): no specular flag, so no term.
    }
    let gloss = match lit.gloss {
        Gloss::One => 0,
        Gloss::BaseAlpha => 1,
        Gloss::NormalAlpha => 2,
        Gloss::Texture(_) if ctx.present[3] => 3,
        Gloss::Texture(_) => 0,
    };
    // A base alpha that masks the dye is not an opacity (material.md §11.2: testing the dye
    // mask would punch holes in dyeable cloth), so it neither tests nor reaches the output.
    let dye_in_base_alpha = u.flags[2] == DYE_SOURCE_BASE_ALPHA;
    let opacity = match lit.opacity {
        Opacity::One => 0,
        Opacity::BaseAlpha if dye_in_base_alpha => 0,
        Opacity::BaseAlpha => 1,
        Opacity::EmissiveAlpha if ctx.present[4] => 2,
        Opacity::EmissiveAlpha => 0,
        Opacity::Texture(_) if ctx.present[6] => 3,
        Opacity::Texture(_) => 0,
    };
    flags |= gloss << lit_flag::GLOSS_SHIFT;
    flags |= opacity << lit_flag::OPACITY_SHIFT;
    for (i, &srgb) in ctx.srgb.iter().enumerate() {
        if srgb {
            flags |= 1 << (lit_flag::SRGB_SHIFT + i as u32);
        }
    }
    let mut packed = [0u32; 2];
    let count = lights.len().min(MAX_DRAW_LIGHTS);
    for (i, &idx) in lights.iter().take(count).enumerate() {
        packed[i / 4] |= u32::from(idx) << ((i % 4) * 8);
    }
    let glow = lit.glow.map(f);
    // Normal-map green sign of lighting.md §5.2.4 (N = n.x T - n.y B with B = +V): positive
    // green tilts the normal towards the top of the image whatever `normal_green_up` says
    // (that flag only serves the M2 pipelines).
    u.params[1] = -1.0;
    u.lit = [glow[0], glow[1], glow[2], power];
    u.lit_lights = [packed[0], packed[1], count as u32, flags];
}

/// A point light of the table, for [`LightSelector`].
#[derive(Debug, Clone, Copy)]
struct PointEntry {
    position: Vec3,
    range: f32,
    index: u8,
}

/// Per-pass light selection: the table's directional lights and its usable point lights
/// (sorted along x), built once per lit pass so each draw only tests the point lights whose
/// x range can reach it and allocates nothing.
#[derive(Debug, Default)]
pub(crate) struct LightSelector {
    /// Directional table indices, in table order.
    directional: Vec<u8>,
    /// Point lights with a positive range, sorted by `position.x`.
    points: Vec<PointEntry>,
    /// Largest point-light range (for the x window).
    max_range: f32,
    /// Lights per draw: [`MAX_DRAW_LIGHTS`], minus one when the frame has a main light.
    limit: usize,
    /// Valid table entries.
    table_len: usize,
}

impl LightSelector {
    /// The selector of a pass with frame uniform `frame`.
    pub(crate) fn new(frame: &FrameUniform) -> Self {
        let table_len = (frame.params[0] as usize).min(frame.lights.len());
        let mut directional = Vec::new();
        let mut points = Vec::new();
        let mut max_range = 0.0f32;
        for (i, l) in frame.lights.iter().take(table_len).enumerate() {
            let Ok(index) = u8::try_from(i) else { break };
            if l.specular_kind[3] > 0.5 {
                directional.push(index);
                continue;
            }
            let r2 = l.color_r2[3];
            let position = Vec3::from_slice(&l.position_k[..3]);
            // `FrameUniform::new` leaves only finite positions and ranges; check anyway.
            if r2.is_nan() || r2 <= 0.0 || !position.is_finite() {
                continue;
            }
            let range = r2.sqrt();
            max_range = max_range.max(range);
            points.push(PointEntry {
                position,
                range,
                index,
            });
        }
        points.sort_by(|a, b| a.position.x.total_cmp(&b.position.x));
        // lighting.md §8.5: at most 8 lights in all, the main light included.
        let has_sun = frame.sun_dir[3] > 0.5;
        let limit = MAX_DRAW_LIGHTS - usize::from(has_sun);
        Self {
            directional,
            points,
            max_range,
            limit,
            table_len,
        }
    }

    /// Lights a draw receives from the table (`MAX_DRAW_LIGHTS` minus the main light).
    #[cfg(test)]
    pub(crate) fn limit(&self) -> usize {
        self.limit
    }

    /// The light-table indices a draw with bounding sphere (`center`, `radius`) receives,
    /// at most [`Self::limit`] of them.
    ///
    /// [`LightSelection::Auto`] (Tessera Decision of `lighting.md` §8.5): point lights whose
    /// range sphere touches the bounding sphere, nearest centre first (ties by table
    /// index), then directional lights in table order. Explicit lists keep their order and
    /// drop indices past the table.
    pub(crate) fn select(
        &self,
        selection: LightSelection,
        center: Vec3,
        radius: f32,
        out: &mut Vec<u8>,
    ) {
        out.clear();
        match selection {
            LightSelection::None => {}
            LightSelection::List { indices, count: n } => {
                out.extend(
                    indices
                        .iter()
                        .take(usize::from(n).min(MAX_DRAW_LIGHTS))
                        .copied()
                        .filter(|&i| usize::from(i) < self.table_len)
                        .take(self.limit),
                );
            }
            LightSelection::Auto => self.select_auto(center, radius, out),
        }
    }

    fn select_auto(&self, center: Vec3, radius: f32, out: &mut Vec<u8>) {
        let center = if center.is_finite() {
            center
        } else {
            Vec3::ZERO
        };
        let radius = if radius.is_finite() {
            radius.max(0.0)
        } else {
            0.0
        };
        // The nearest `limit` touching lights, kept sorted by (distance, index).
        let mut best = [(0.0f32, 0u8); MAX_DRAW_LIGHTS];
        let mut len = 0usize;
        if self.limit > 0 && !self.points.is_empty() {
            let reach = radius + self.max_range;
            let (lo, hi) = (center.x - reach, center.x + reach);
            let start = self.points.partition_point(|p| p.position.x < lo);
            for p in self.points.get(start..).unwrap_or(&[]) {
                if p.position.x > hi {
                    break;
                }
                let d = p.position.distance(center);
                if d.is_nan() || d >= p.range + radius {
                    continue;
                }
                let key = (d, p.index);
                let less =
                    |a: (f32, u8), b: (f32, u8)| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).is_lt();
                if len == self.limit && !less(key, best[len - 1]) {
                    continue;
                }
                let mut j = len.min(self.limit - 1);
                while j > 0 && less(key, best[j - 1]) {
                    best[j] = best[j - 1];
                    j -= 1;
                }
                best[j] = key;
                len = (len + 1).min(self.limit);
            }
        }
        out.extend(best[..len].iter().map(|&(_, i)| i));
        let room = self.limit.saturating_sub(out.len());
        out.extend(self.directional.iter().copied().take(room));
    }
}

/// What a lit pipeline draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum LitKind {
    Opaque,
    Mask,
    TestBlend,
    Blend,
    Additive,
    /// Shadow-map depth of an opaque caster.
    Shadow,
    /// Shadow-map depth of an alpha-tested caster.
    ShadowMask,
}

impl LitKind {
    pub(crate) fn is_shadow(self) -> bool {
        matches!(self, Self::Shadow | Self::ShadowMask)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct LitKey {
    /// Colour target format (ignored for shadow kinds).
    pub format: wgpu::TextureFormat,
    pub kind: LitKind,
    pub cull: Cull,
    pub skinned: bool,
}

/// Layouts and shaders of the lit and shadow pipelines.
#[derive(Debug)]
pub(crate) struct LitLayouts {
    /// Group 0 of lit passes: frame uniform, shadow map, comparison sampler.
    pub frame: wgpu::BindGroupLayout,
    /// Group 0 of the shadow pass: the frame uniform only.
    pub shadow_frame: wgpu::BindGroupLayout,
    /// Group 2: the material textures and samplers.
    pub material: wgpu::BindGroupLayout,
    lit_layout: wgpu::PipelineLayout,
    shadow_layout: wgpu::PipelineLayout,
    lit_shader: wgpu::ShaderModule,
    shadow_shader: wgpu::ShaderModule,
}

const VERTEX_ATTRS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    0 => Float32x3, 1 => Snorm16x4, 2 => Snorm16x4, 3 => Float32x2, 4 => Unorm8x4
];
const SKIN_ATTRS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
    5 => Uint8x4, 6 => Unorm16x4
];

/// Slope-scaled depth bias of the shadow pass (Tessera's choice; the constant part is
/// [`crate::SunShadow::depth_bias`]).
const SHADOW_SLOPE_BIAS: f32 = 1.5;

impl LitLayouts {
    pub(crate) fn new(device: &wgpu::Device, mesh: &MeshLayouts) -> Self {
        let frame = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tessera-frame-lighting-bgl"),
            entries: &crate::lighting::frame_layout_entries(),
        });
        let [uniform_entry, ..] = crate::lighting::frame_layout_entries();
        let shadow_frame = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tessera-shadow-frame-bgl"),
            entries: &[uniform_entry],
        });
        let mut entries = Vec::with_capacity(2 * LIT_TEXTURES);
        for binding in 0..LIT_TEXTURES as u32 {
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
        for binding in LIT_TEXTURES as u32..2 * LIT_TEXTURES as u32 {
            entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });
        }
        let material = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tessera-lit-material-bgl"),
            entries: &entries,
        });
        let lit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tessera-lit-layout"),
            bind_group_layouts: &[Some(&frame), Some(&mesh.draw), Some(&material)],
            immediate_size: 0,
        });
        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tessera-shadow-layout"),
            bind_group_layouts: &[Some(&shadow_frame), Some(&mesh.draw), Some(&material)],
            immediate_size: 0,
        });
        let lit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tessera-lit.wgsl"),
            source: wgpu::ShaderSource::Wgsl(lit_shader_source().into()),
        });
        let shadow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tessera-shadow.wgsl"),
            source: wgpu::ShaderSource::Wgsl(shadow_shader_source().into()),
        });
        Self {
            frame,
            shadow_frame,
            material,
            lit_layout,
            shadow_layout,
            lit_shader,
            shadow_shader,
        }
    }

    fn build(&self, device: &wgpu::Device, key: LitKey) -> wgpu::RenderPipeline {
        let vertex_buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<GpuVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &VERTEX_ATTRS,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<GpuSkin>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &SKIN_ATTRS,
            }),
        ];
        let buffers = if key.skinned {
            &vertex_buffers[..]
        } else {
            &vertex_buffers[..1]
        };
        let cull_mode = match key.cull {
            Cull::None => None,
            Cull::Back => Some(wgpu::Face::Back),
            Cull::Front => Some(wgpu::Face::Front),
        };
        let label = format!(
            "tessera-lit-{:?}-{:?}{}",
            key.kind,
            key.cull,
            if key.skinned { "-skinned" } else { "" }
        );
        let primitive = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode,
            ..Default::default()
        };
        if key.kind.is_shadow() {
            let fragment = (key.kind == LitKind::ShadowMask).then(|| wgpu::FragmentState {
                module: &self.shadow_shader,
                entry_point: Some("fs_shadow_mask"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[],
            });
            return device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&label),
                layout: Some(&self.shadow_layout),
                vertex: wgpu::VertexState {
                    module: &self.shadow_shader,
                    entry_point: Some(if key.skinned {
                        "vs_shadow_skinned"
                    } else {
                        "vs_shadow"
                    }),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers,
                },
                primitive,
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: SHADOW_FORMAT,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState {
                        constant: 0,
                        slope_scale: SHADOW_SLOPE_BIAS,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState::default(),
                fragment,
                multiview_mask: None,
                cache: None,
            });
        }
        let (fs, blend, depth_write) = match key.kind {
            LitKind::Opaque => ("fs_lit_opaque", None, true),
            LitKind::Mask => ("fs_lit_mask", None, true),
            LitKind::TestBlend => (
                "fs_lit_test_blend",
                Some(wgpu::BlendState::ALPHA_BLENDING),
                true,
            ),
            LitKind::Blend => (
                "fs_lit_blend",
                Some(wgpu::BlendState::ALPHA_BLENDING),
                false,
            ),
            _ => (
                "fs_lit_additive",
                Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Zero,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                false,
            ),
        };
        let targets = [Some(wgpu::ColorTargetState {
            format: key.format,
            blend,
            write_mask: wgpu::ColorWrites::ALL,
        })];
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&label),
            layout: Some(&self.lit_layout),
            vertex: wgpu::VertexState {
                module: &self.lit_shader,
                entry_point: Some(if key.skinned {
                    "vs_lit_skinned"
                } else {
                    "vs_lit"
                }),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers,
            },
            primitive,
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(depth_write),
                depth_compare: Some(DEPTH_COMPARE),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.lit_shader,
                entry_point: Some(fs),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &targets,
            }),
            multiview_mask: None,
            cache: None,
        })
    }
}

/// Lazily built lit and shadow pipeline variants.
#[derive(Debug, Default)]
pub(crate) struct LitPipelineCache {
    pipelines: Vec<(LitKey, wgpu::RenderPipeline)>,
}

impl LitPipelineCache {
    pub(crate) fn ensure(&mut self, device: &wgpu::Device, layouts: &LitLayouts, key: LitKey) {
        let key = normalize(key);
        if self.lookup(key).is_none() {
            tracing::debug!(?key, "building lit pipeline");
            self.pipelines.push((key, layouts.build(device, key)));
        }
    }

    pub(crate) fn lookup(&self, key: LitKey) -> Option<&wgpu::RenderPipeline> {
        let key = normalize(key);
        self.pipelines
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, p)| p)
    }
}

/// Shadow pipelines do not depend on the colour format or culling.
fn normalize(key: LitKey) -> LitKey {
    if key.kind.is_shadow() {
        LitKey {
            format: wgpu::TextureFormat::R8Unorm,
            cull: Cull::None,
            ..key
        }
    } else {
        key
    }
}

/// The world-space bounding sphere of a draw (for light selection).
pub(crate) fn draw_sphere(sphere: crate::BoundingSphere, transform: Mat4) -> (Vec3, f32) {
    let s = sphere.transformed(transform);
    (s.center, s.radius)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::{FrameLighting, Light, LightKind};
    use crate::material::Specular;

    fn validate(src: &str) {
        let module = wgpu::naga::front::wgsl::parse_str(src)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(src)));
        let mut v = wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        );
        if let Err(e) = v.validate(&module) {
            panic!("{}", e.emit_to_string(src));
        }
        // The draw uniform the shader declares matches the Rust struct.
        let draw = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Draw"))
            .map(|(_, t)| t.inner.size(module.to_ctx()));
        assert_eq!(draw, Some(size_of::<DrawUniform>() as u32));
    }

    #[test]
    fn lit_and_shadow_shaders_validate() {
        validate(&lit_shader_source());
        validate(&shadow_shader_source());
    }

    fn ctx() -> LitContext {
        LitContext {
            present: [true; LIT_TEXTURES],
            srgb: [false; LIT_TEXTURES],
        }
    }

    #[test]
    fn lit_fields_pack_flags_and_lights() {
        let mut u = bytemuck::Zeroable::zeroed();
        let m = Material {
            specular: Some(Specular {
                color: [1.0; 3],
                shininess: 0.4,
                texture: None,
            }),
            ..Material::default()
        };
        let lit = LitSurface {
            glow: [0.5, f32::NAN, 1.0],
            gloss: Gloss::NormalAlpha,
            opacity: Opacity::One,
            vertex_colors: true,
            ..LitSurface::default()
        };
        let mut c = ctx();
        c.srgb[0] = true;
        lit_fields(&mut u, &m, &lit, &c, &[3, 1, 4, 1, 5, 9, 2, 6, 5]);
        assert_eq!(u.lit, [0.5, 0.0, 1.0, 0.0]);
        let flags = u.lit_lights[3];
        assert_eq!(
            flags & lit_flag::SPECULAR,
            0,
            "power below 0.5 has no highlight"
        );
        assert_ne!(flags & lit_flag::VERTEX_COLORS, 0);
        assert_ne!(flags & lit_flag::FOG, 0);
        assert_eq!((flags >> lit_flag::GLOSS_SHIFT) & 3, 2);
        assert_eq!((flags >> lit_flag::OPACITY_SHIFT) & 3, 0);
        assert_ne!(flags & (1 << lit_flag::SRGB_SHIFT), 0);
        assert_eq!(u.lit_lights[2], 8);
        assert_eq!(u.lit_lights[0], 0x0104_0103);
        assert_eq!(u.lit_lights[1], 0x0602_0905);

        // A gloss texture that did not resolve falls back to g = 1.
        let lit = LitSurface {
            gloss: Gloss::Texture(crate::TextureId::dangling()),
            ..LitSurface::default()
        };
        let mut c = ctx();
        c.present[3] = false;
        let m = Material {
            specular: Some(Specular {
                color: [1.0; 3],
                shininess: 20.0,
                texture: None,
            }),
            ..Material::default()
        };
        lit_fields(&mut u, &m, &lit, &c, &[]);
        assert_eq!((u.lit_lights[3] >> lit_flag::GLOSS_SHIFT) & 3, 0);
        assert_ne!(u.lit_lights[3] & lit_flag::SPECULAR, 0);
        assert_eq!(u.lit[3], 20.0);
    }

    fn frame_with(lights: Vec<Light>, sun: bool) -> FrameUniform {
        let l = FrameLighting {
            lights,
            ..FrameLighting::default()
        };
        let l = if sun {
            l
        } else {
            FrameLighting { sun: None, ..l }
        };
        FrameUniform::new(&l, Mat4::IDENTITY, Vec3::ZERO, false)
    }

    #[test]
    fn auto_selection_is_nearest_first() {
        let mut lights: Vec<Light> = (0..12)
            .map(|i| Light::point(Vec3::new(i as f32 * 3.0, 0.0, 0.0), 5.0, 0.1, [1.0; 3]))
            .collect();
        lights.push(Light {
            kind: LightKind::Directional { direction: Vec3::Z },
            color: [1.0; 3],
            specular: [1.0; 3],
        });
        // FrameLighting::default() has a sun: 7 table lights at most (lighting.md §8.5).
        let sel = LightSelector::new(&frame_with(lights.clone(), true));
        assert_eq!(sel.limit(), MAX_DRAW_LIGHTS - 1);
        let mut out = Vec::new();
        // Sphere at x = 10 with radius 1: lights within 6 of the centre touch it.
        sel.select(
            LightSelection::Auto,
            Vec3::new(10.0, 0.0, 0.0),
            1.0,
            &mut out,
        );
        assert_eq!(out, vec![3, 4, 2, 5, 12]);
        // Far away: only the directional light.
        sel.select(LightSelection::Auto, Vec3::splat(1e6), 1.0, &mut out);
        assert_eq!(out, vec![12]);
        // A huge sphere touches everything: 7 at most with the sun, nearest first.
        sel.select(LightSelection::Auto, Vec3::ZERO, 1e5, &mut out);
        assert_eq!(out, vec![0, 1, 2, 3, 4, 5, 6]);
        // Without a main light the draw takes 8.
        let no_sun = LightSelector::new(&frame_with(lights.clone(), false));
        no_sun.select(LightSelection::Auto, Vec3::ZERO, 1e5, &mut out);
        assert_eq!(out, vec![0, 1, 2, 3, 4, 5, 6, 7]);
        // Explicit lists drop indices past the table and keep the cap.
        sel.select(
            LightSelection::List {
                indices: [200, 1, 0, 0, 0, 0, 0, 0],
                count: 2,
            },
            Vec3::ZERO,
            1.0,
            &mut out,
        );
        assert_eq!(out, vec![1]);
        sel.select(
            LightSelection::List {
                indices: [1, 2, 3, 4, 5, 6, 7, 8],
                count: 8,
            },
            Vec3::ZERO,
            1.0,
            &mut out,
        );
        assert_eq!(out, vec![1, 2, 3, 4, 5, 6, 7]);
        no_sun.select(
            LightSelection::List {
                indices: [1, 2, 3, 4, 5, 6, 7, 8],
                count: u8::MAX,
            },
            Vec3::ZERO,
            1.0,
            &mut out,
        );
        assert_eq!(out, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        sel.select(LightSelection::None, Vec3::ZERO, 1.0, &mut out);
        assert!(out.is_empty());
        // Hostile spheres select without panicking.
        sel.select(LightSelection::Auto, Vec3::NAN, f32::INFINITY, &mut out);
        sel.select(
            LightSelection::Auto,
            Vec3::splat(f32::MAX),
            f32::MAX,
            &mut out,
        );
    }

    /// The bounded, windowed selection equals a brute-force sort of every touching light.
    #[test]
    fn auto_selection_matches_brute_force() {
        // Deterministic pseudo-random lights (xorshift), some with equal distances.
        let mut state = 0x9E37_79B9u32;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state % 1000) as f32 / 10.0
        };
        let lights: Vec<Light> = (0..200)
            .map(|i| {
                if i % 37 == 0 {
                    Light {
                        kind: LightKind::Directional { direction: Vec3::X },
                        color: [1.0; 3],
                        specular: [1.0; 3],
                    }
                } else {
                    let p = Vec3::new(next().round(), next().round() * 0.1, 0.0);
                    Light::point(p, 1.0 + next() * 0.1, 0.1, [1.0; 3])
                }
            })
            .collect();
        for sun in [true, false] {
            let frame = frame_with(lights.clone(), sun);
            let sel = LightSelector::new(&frame);
            let mut out = Vec::new();
            for c in 0..60 {
                let center = Vec3::new(c as f32 * 1.7, 2.0, 0.5);
                let radius = (c % 5) as f32;
                sel.select(LightSelection::Auto, center, radius, &mut out);
                let mut near: Vec<(f32, u8)> = Vec::new();
                let mut dir = Vec::new();
                for (i, l) in frame
                    .lights
                    .iter()
                    .take(frame.params[0] as usize)
                    .enumerate()
                {
                    let i = i as u8;
                    if l.specular_kind[3] > 0.5 {
                        dir.push(i);
                    } else {
                        let d = Vec3::from_slice(&l.position_k[..3]).distance(center);
                        if d < l.color_r2[3].sqrt() + radius {
                            near.push((d, i));
                        }
                    }
                }
                near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
                let mut expected: Vec<u8> = near.iter().map(|&(_, i)| i).collect();
                expected.extend(dir);
                expected.truncate(sel.limit());
                assert_eq!(out, expected, "centre {center}, radius {radius}, sun {sun}");
            }
        }
    }

    #[test]
    fn dye_in_base_alpha_is_not_opacity() {
        use crate::material::{Dye, DyeMask};
        let m = Material {
            dye: Some(Dye {
                color: [1.0, 0.0, 0.0],
                mask: DyeMask::BaseAlpha,
            }),
            normal_green_up: false,
            ..Material::default()
        };
        let mut u = DrawUniform::new(
            Mat4::IDENTITY,
            &m,
            &crate::mesh_pipelines::DrawContext {
                shading: crate::MeshShading::Lit,
                normal_map: false,
                dye_mask: false,
                specular_tex: false,
                overlay_color: [1.0; 4],
                normal_color: [1.0; 4],
                normal_length: 0.0,
            },
        );
        assert_eq!(u.params[1], 1.0, "the M2 sign follows normal_green_up");
        lit_fields(&mut u, &m, &LitSurface::default(), &ctx(), &[]);
        assert_eq!((u.lit_lights[3] >> lit_flag::OPACITY_SHIFT) & 3, 0);
        // lighting.md §5.2.4: green tilts towards the image top in lit passes.
        assert_eq!(u.params[1], -1.0);
        // Without the dye the base alpha is the opacity.
        let plain = Material::default();
        u.flags[2] = 0;
        lit_fields(&mut u, &plain, &LitSurface::default(), &ctx(), &[]);
        assert_eq!((u.lit_lights[3] >> lit_flag::OPACITY_SHIFT) & 3, 1);
    }
}
