// SPDX-License-Identifier: GPL-3.0-or-later
//! Instanced scenery and frills (M4.5, `docs/specs/scenery.md` §8.3, §8.5, §9;
//! `docs/specs/render.md` §5.5).
//!
//! A [`WorldModel`] is a mesh with one [`WorldSurface`] per submesh, uploaded once with
//! [`crate::Renderer::upload_world_model`] into buffers of its own. [`Pass::instanced`]
//! draws it any number of times per frame from an [`InstanceBatch`]: one instanced draw
//! per submesh and batch, with per-instance transforms, tints and terrain colours in a
//! vertex buffer. Batches are lit like lit meshes (`lighting.md` §5: ambient and
//! hemispheric terms, the sun with its shadow, up to eight table lights per instance, fog).
//!
//! A batch with [`FrillParams`] follows the frill engine of `scenery.md` §8.3 and §8.5:
//! it is skipped beyond the far distance *D*, thinned by distance reduction, sways by a
//! per-vertex weight from the model's bounding box, blends toward the terrain colour
//! beneath it, and scales out over the fade length *F* (Tessera Decisions where the
//! original's shader formulas are OPEN; see `docs/specs/render.md` §5.5).
//!
//! Sky objects ([`crate::SkyObject`]) draw [`WorldModel`]s too.

use std::fmt;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;

use crate::arena::MeshConfig;
use crate::error::{RenderError, Result};
use crate::frame::{Pass, Rgba};
use crate::lighting::{FRAME_SHADING_WGSL, FRAME_WGSL, MAX_DRAW_LIGHTS};
use crate::lit_pipelines::LightTarget;
use crate::lit_pipelines::SHADOW_FORMAT;
use crate::material::{AlphaMode, FixedBlend, FogRule, LightSelection, Specular};
use crate::mesh::{GpuVertex, MeshData, MeshInfo};
use crate::pass_hook::{HookRecord, HookStage, PassHook};
use crate::pipelines::{DEPTH_COMPARE, DEPTH_FORMAT};
use crate::renderer::{FrameStats, Renderer, ResolvedTexture, TextureResolver, WorldFrame};
use crate::texture::TextureId;

const INSTANCED_WGSL: &str = include_str!("shaders/instanced.wgsl");
const INSTANCED_COMMON_WGSL: &str = include_str!("shaders/instanced_common.wgsl");

/// Largest number of instances one pass may draw per frame (all batches together), so the
/// per-frame instance buffer stays bounded (96 bytes each: 96 MiB).
pub const MAX_INSTANCES_PER_PASS: usize = 1 << 20;

/// Largest number of surfaces of one [`WorldModel`] (the mesh submesh limit).
pub const MAX_MODEL_SURFACES: usize = 4096;

/// Frill sector size in world units, 160 / 16 (`scenery.md` §8.2, §8.3).
pub const FRILL_SECTOR: f32 = 10.0;

/// Duration of a frill group's fade-in in seconds (`scenery.md` §8.3 rule 5).
pub const FRILL_FADE_IN_SECONDS: f32 = 1.5;

/// Length bound of the sway direction (sin, ½ sin, 0) of the sway Decision
/// (`docs/specs/render.md` §5.5): √(1 + ¼). The largest sway displacement in model space is
/// intensity × *h* × this.
const SWAY_REACH: f32 = 1.118_034;

/// Animation intensities at or below this value do not sway (`scenery.md` §8.3 rule 6,
/// decision 0012).
pub const FRILL_SWAY_MIN: f32 = 0.0002;

// ---------------------------------------------------------------------------------------
// Public input types
// ---------------------------------------------------------------------------------------

/// The surface of one submesh of a [`WorldModel`]: the lit-pass subset of [`crate::Material`]
/// and [`crate::LitSurface`] that scenery, frills and sky objects use.
///
/// Colours are in the original's stored-value space (`lighting.md` §5.1). A texture
/// uploaded with [`crate::ColorSpace::Srgb`] is re-encoded after sampling, as in the lit
/// mesh pipeline (`docs/specs/render.md` §4.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldSurface {
    /// Material colour *M* (RGBA).
    pub base_color: Rgba,
    /// Albedo texture *C* (`None`: white). Its sampler (filter, wrap) is the upload's.
    pub base_texture: Option<TextureId>,
    /// Tangent-space normal map (`lighting.md` §5.2.4 sign convention). Used only when
    /// the mesh has tangents and the texture resolves (an unknown or destroyed id, or a
    /// render target, draws as if there were no normal map).
    pub normal_texture: Option<TextureId>,
    /// Alpha test and blend state, as for lit meshes (`docs/specs/render.md` §4.3); the
    /// opacity is the base texture's alpha, or 1 when that alpha is the gloss
    /// ([`WorldSurface::gloss_from_base_alpha`]; `material.md` §11.2–§11.3).
    pub alpha: AlphaMode,
    /// Draw back faces too (lit with the front normal, `material.md` §11.4).
    pub two_sided: bool,
    /// Specular colour *Ms* and power *p* (`lighting.md` §5.8; below 0.5: none). The
    /// mask texture is not used.
    pub specular: Option<Specular>,
    /// Gloss *g* from the base texture's alpha (else 1). That alpha is then not an
    /// opacity: the surface's alpha test keeps every texel and its output alpha is *M*.alpha
    /// (`material.md` §11.3: a gloss alpha never reaches the output).
    pub gloss_from_base_alpha: bool,
    /// Gloss *g* from the normal texture's alpha (`lighting.md` §5.2 item 5, a
    /// normal-and-gloss texture); used when the normal texture resolves, whether or not the
    /// mesh has tangents. Ignored when [`WorldSurface::gloss_from_base_alpha`] is set.
    pub gloss_from_normal_alpha: bool,
    /// Multiply *M* by the vertex colour (`lighting.md` §5.13).
    pub vertex_colors: bool,
    /// Unlit: texture × *M* (× vertex colour), fogged (`lighting.md` §7, code 0x00).
    pub unlit: bool,
}

impl Default for WorldSurface {
    fn default() -> Self {
        Self {
            base_color: [1.0; 4],
            base_texture: None,
            normal_texture: None,
            alpha: AlphaMode::Opaque,
            two_sided: false,
            specular: None,
            gloss_from_base_alpha: false,
            gloss_from_normal_alpha: false,
            vertex_colors: false,
            unlit: false,
        }
    }
}

impl WorldSurface {
    /// An opaque surface with a base texture.
    #[must_use]
    pub fn textured(texture: TextureId) -> Self {
        Self {
            base_texture: Some(texture),
            ..Self::default()
        }
    }
}

/// A mesh uploaded for instanced drawing and sky objects
/// ([`crate::Renderer::upload_world_model`]). Cheap to clone (shared handle); the GPU
/// buffers live as long as any clone. A texture it was built with that is later destroyed
/// ([`crate::Renderer::destroy_texture`]) is dropped from its surfaces when the model is
/// next drawn: it then draws as if that texture were absent (white base, no normal map).
#[derive(Clone)]
pub struct WorldModel(Arc<ModelGpu>);

impl fmt::Debug for WorldModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorldModel")
            .field("label", &self.0.label)
            .field("vertices", &self.0.info.vertex_count)
            .field("indices", &self.0.info.index_count)
            .field("surfaces", &self.0.surfaces.len())
            .finish_non_exhaustive()
    }
}

impl WorldModel {
    /// Bounds and stream summary of the mesh.
    #[must_use]
    pub fn info(&self) -> &MeshInfo {
        &self.0.info
    }

    /// Number of surfaces (one per submesh).
    #[must_use]
    pub fn surface_count(&self) -> usize {
        self.0.surfaces.len()
    }

    /// GPU bytes of the model's vertex and index buffers.
    #[must_use]
    pub fn gpu_bytes(&self) -> u64 {
        self.0.vertices.size() + self.0.indices.size()
    }

    pub(crate) fn gpu(&self) -> &ModelGpu {
        &self.0
    }
}

/// One instance of an [`InstanceBatch`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Instance {
    /// Model → world transform (affine; the bottom row is ignored). For frills this is
    /// position · rotation · per-axis scale (`scenery.md` §8.5 step 5).
    pub transform: Mat4,
    /// The per-object tint: multiplies the material colour *M* (RGB and alpha) and the
    /// specular colour *Ms* (RGB), as the client does with its per-object tint
    /// (`material.md` §8.3).
    pub tint: Rgba,
    /// Terrain colour beneath the instance, stored-value RGB (frills only, §8.3 rule 7).
    /// Frill groups blend their albedo toward it by the terrain-colour intensity (default
    /// 1), so a frill instance must carry the colour of the ground beneath it.
    pub terrain_color: [f32; 3],
    /// The caller's object handle of this instance, for the parent switches of the light
    /// selection (as [`crate::MeshDraw::light_object`]); `None`: none.
    pub light_object: Option<u64>,
}

impl Instance {
    /// An untinted instance at `transform`, with a white terrain colour. For a frill
    /// group, set [`Instance::terrain_color`] to the ground beneath the instance (or set
    /// [`FrillParams::terrain_color_intensity`] to 0): with white, the base of a clump
    /// turns white.
    #[must_use]
    pub fn new(transform: Mat4) -> Self {
        Self {
            transform,
            tint: [1.0; 4],
            terrain_color: [1.0; 3],
            light_object: None,
        }
    }
}

/// Distance reduction of a frill group (`scenery.md` §8.3 rule 4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrillReduction {
    /// *Rd*: distance where reduction starts (`FrillEngine.ReductionDistance` × 10 units;
    /// 40 by default).
    pub distance: f32,
    /// Instance granule *g*: the instance count must be a multiple of it, else the group
    /// is not reduced.
    pub granule: u32,
}

/// Frill-engine parameters of a batch (`scenery.md` §8.3, §8.5). A batch with them is one
/// frill *group*.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrillParams {
    /// Far distance *D*: a group whose bounding box is at least this far from the camera
    /// is not drawn (rule 2; 97 units at the default level).
    pub far_distance: f32,
    /// Fade length *F* (rule 3): frills scale out between *D* − *F* and *D*.
    pub fade_length: f32,
    /// Distance reduction (rule 4), `None` when the object kind does not allow it or the
    /// engine option is off.
    pub reduction: Option<FrillReduction>,
    /// Fade-in factor 0–1 of a group that has just appeared ([`frill_fade_in`], rule 5).
    pub fade_in: f32,
    /// `Render_Frill_AnimationIntensity` (0–1; sway above [`FRILL_SWAY_MIN`]).
    pub animation_intensity: f32,
    /// `Render_Frill_TerrainColorIntensity` (0–1; default 1 by rule 7).
    pub terrain_color_intensity: f32,
    /// `Render_Frill_IsOpaque` (default true). A non-opaque group draws its opaque and
    /// alpha-tested surfaces blended with depth write, in the translucent stage.
    pub opaque: bool,
    /// z_min and z_max of the vertex block's **stored** bounding box (`scenery.md` §8.5
    /// step 5, `mesh.md` §6.1), for the sway weight. `None`: the box computed from the
    /// model's vertices (equal to the stored one on all observed data, `mesh.md` M7).
    pub sway_z_range: Option<[f32; 2]>,
}

impl Default for FrillParams {
    /// The engine defaults: radius level 2 (10 sectors, *D* = 97, *F* = 30), depth
    /// transitions on, fade distance 3 sectors, reduction from 4 sectors with granule 1,
    /// fully faded in, no sway, terrain colour intensity 1, opaque, the computed box
    /// (`scenery.md` §8.3, §8.4).
    fn default() -> Self {
        let (far_distance, fade_length) =
            frill_distances(frill_radius_sectors(2, false), true, 3.0);
        Self {
            far_distance,
            fade_length,
            reduction: Some(FrillReduction {
                distance: 4.0 * FRILL_SECTOR,
                granule: 1,
            }),
            fade_in: 1.0,
            animation_intensity: 0.0,
            terrain_color_intensity: 1.0,
            opaque: true,
            sway_z_range: None,
        }
    }
}

/// Frill radius in sectors for `Render.FrillDistance` level `level` (`scenery.md` §8.3
/// rule 1): 0, 8, 10, 12, 14, 16 for levels 0–5, else 0; halved (integer division) when
/// `dial_back` (`Render.DialBackGraphics`) is on.
#[must_use]
pub fn frill_radius_sectors(level: i32, dial_back: bool) -> u32 {
    let r = match level {
        1 => 8,
        2 => 10,
        3 => 12,
        4 => 14,
        5 => 16,
        _ => 0,
    };
    if dial_back { r / 2 } else { r }
}

/// The far distance *D* and fade length *F* in units for a frill radius in sectors
/// (`scenery.md` §8.3 rules 2–3): *D* = (radius − 0.3) × 10 with depth transitions, else
/// 1,000; *F* = min(*D*, `depth_fade_sectors` × 10). Negative or non-finite results
/// become 0.
#[must_use]
pub fn frill_distances(
    radius_sectors: u32,
    depth_transitions: bool,
    depth_fade_sectors: f32,
) -> (f32, f32) {
    let d = if depth_transitions {
        (radius_sectors as f32 - 0.3) * FRILL_SECTOR
    } else {
        1_000.0
    };
    let d = if d.is_finite() { d.max(0.0) } else { 0.0 };
    let fade = depth_fade_sectors * FRILL_SECTOR;
    let f = if fade.is_finite() {
        fade.clamp(0.0, d)
    } else {
        0.0
    };
    (d, f)
}

/// Fade-in opacity of a frill group `age_seconds` after it appeared (`scenery.md` §8.3
/// rule 5): 3*t*² − 2*t*³ with *t* = age / 1.5 s, clamped to 0–1.
#[must_use]
pub fn frill_fade_in(age_seconds: f32) -> f32 {
    let t = if age_seconds.is_finite() {
        (age_seconds / FRILL_FADE_IN_SECONDS).clamp(0.0, 1.0)
    } else {
        1.0
    };
    t * t * (3.0 - 2.0 * t)
}

/// Instances drawn of a reducible frill group of `count` instances at squared distance
/// `distance_sq` from the camera (`scenery.md` §8.3 rule 4): with
/// *f* = clamp((*d*² − *Rd*²) / (*D*² − *Rd*²), 0, 1), *g* × max(1, trunc((1 − *f*) ×
/// *n* / *g*)). No reduction (all `count`) when 2 · *Rd* > *D*, the granule is 0 or does
/// not divide `count`.
#[must_use]
pub fn frill_reduced_count(
    count: usize,
    distance_sq: f32,
    far_distance: f32,
    reduction: FrillReduction,
) -> usize {
    let g = reduction.granule as usize;
    let rd = reduction.distance;
    if count == 0
        || g == 0
        || !count.is_multiple_of(g)
        || !matches!(
            (2.0 * rd).partial_cmp(&far_distance),
            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
        )
        || !distance_sq.is_finite()
    {
        return count;
    }
    let (d2, rd2) = (far_distance * far_distance, rd * rd);
    let span = d2 - rd2;
    let f = if span > 0.0 {
        ((distance_sq - rd2) / span).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let groups = ((1.0 - f) * (count / g) as f32).trunc();
    let groups = if groups.is_finite() {
        (groups as usize).max(1)
    } else {
        1
    };
    (g * groups).min(count)
}

/// Instances of one [`WorldModel`] drawn in a pass ([`Pass::instanced`]).
#[derive(Debug, Clone)]
pub struct InstanceBatch {
    /// The model.
    pub model: WorldModel,
    /// The instances (shared, so a batch can be queued on several passes for free).
    pub instances: Arc<[Instance]>,
    /// Lights from the pass's table: with [`LightSelection::Auto`] selected for each
    /// instance against its own world box (`lighting.md` §8.5, one list per object); an
    /// explicit list or `None` applies to every instance.
    pub lights: LightSelection,
    /// Opaque and alpha-tested surfaces cast the sun's shadow.
    pub cast_shadow: bool,
    /// Instances whose bounding sphere starts farther than this from the camera are not
    /// drawn (`scenery.md` §9.1 Tessera Decision: `d0` of the LOD class, scaled by the
    /// draw-distance setting). `None`: no limit.
    pub max_distance: Option<f32>,
    /// Frill-engine behaviour (`None`: ordinary scenery).
    pub frill: Option<FrillParams>,
}

impl InstanceBatch {
    /// A scenery batch: automatic lights, shadow casting, no distance limit, no frill.
    #[must_use]
    pub fn new(model: WorldModel, instances: impl Into<Arc<[Instance]>>) -> Self {
        Self {
            model,
            instances: instances.into(),
            lights: LightSelection::Auto,
            cast_shadow: true,
            max_distance: None,
            frill: None,
        }
    }

    /// A frill group (no shadow casting). Its instances should carry the terrain colour
    /// beneath them ([`Instance::terrain_color`]).
    #[must_use]
    pub fn frills(
        model: WorldModel,
        instances: impl Into<Arc<[Instance]>>,
        params: FrillParams,
    ) -> Self {
        Self {
            cast_shadow: false,
            frill: Some(params),
            ..Self::new(model, instances)
        }
    }
}

impl Pass {
    /// Queue instanced scenery or a frill group (`docs/specs/render.md` §5.5). Needs a lit
    /// pass ([`Pass::frame_lighting`]); ignored by screen passes.
    pub fn instanced(&mut self, batch: InstanceBatch) -> &mut Self {
        self.world.instanced.push(batch);
        self
    }
}

impl Renderer<'_> {
    /// Upload a mesh with one surface per submesh for instanced drawing and sky objects
    /// (`docs/specs/render.md` §5.5). Submesh *i* uses `surfaces[min(i, len − 1)]`; no
    /// surfaces means the default surface. Textures are resolved now: unknown ones and
    /// registered render targets are treated as absent (logged): a white base texture, no
    /// normal map.
    ///
    /// # Errors
    ///
    /// [`RenderError::InvalidMesh`] and [`RenderError::LimitExceeded`] as for
    /// [`Renderer::upload_mesh`] (the mesh is validated the same way), and when more than
    /// [`MAX_MODEL_SURFACES`] surfaces are given; [`RenderError::Gpu`] when the GPU
    /// rejects the buffers.
    pub fn upload_world_model(
        &mut self,
        data: &MeshData,
        surfaces: &[WorldSurface],
    ) -> Result<WorldModel> {
        if surfaces.len() > MAX_MODEL_SURFACES {
            return Err(RenderError::LimitExceeded {
                what: "world model surfaces",
                requested: surfaces.len() as u64,
                max: MAX_MODEL_SURFACES as u64,
            });
        }
        let up = self.world_upload();
        // Validated and packed as for `upload_mesh` (render.md §5.5): the renderer's own
        // mesh limits, without wireframe edges.
        let prepared = data.prepare(&MeshConfig {
            wireframe_edges: false,
            ..up.mesh_config
        })?;
        let device = up.device;
        let max = device.limits().max_buffer_size;
        let vbytes: &[u8] = bytemuck::cast_slice(&prepared.vertices);
        let ibytes: &[u8] = bytemuck::cast_slice(&prepared.indices);
        for (what, len) in [
            ("model vertex bytes", vbytes.len()),
            ("model index bytes", ibytes.len()),
        ] {
            if len as u64 > max {
                return Err(RenderError::LimitExceeded {
                    what,
                    requested: len as u64,
                    max,
                });
            }
        }
        let layouts = up.instanced.layouts(device);
        let surface_layout = layouts.surface.clone();
        let (model, gpu_error) =
            crate::renderer::with_error_scope(device, || {
                let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("tessera-model-vertices"),
                    contents: vbytes,
                    usage: wgpu::BufferUsages::VERTEX,
                });
                let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("tessera-model-indices"),
                    contents: ibytes,
                    usage: wgpu::BufferUsages::INDEX,
                });
                let has_tangents = prepared.info.has_tangents;
                let defaults = [WorldSurface::default()];
                let list = if surfaces.is_empty() {
                    &defaults[..]
                } else {
                    surfaces
                };
                let mut warned = false;
                let white = up.textures.white();
                let gpu_surfaces: Vec<SurfaceGpu> = list
                .iter()
                .map(|s| {
                    for id in [s.base_texture, s.normal_texture] {
                        if up.textures.is_missing(id) && !warned {
                            tracing::warn!(
                                ?id,
                                "world model texture unknown or a render target; treated as absent"
                            );
                            warned = true;
                        }
                    }
                    let set = [up.textures.get(s.base_texture), up.textures.get(s.normal_texture)];
                    SurfaceGpu::new(device, &surface_layout, *s, has_tangents, set, &white)
                })
                .collect();
                let parts = prepared
                    .submeshes
                    .iter()
                    .enumerate()
                    .map(|(i, s)| ModelPart {
                        indices: s.first_index..s.first_index.saturating_add(s.index_count),
                        surface: i.min(gpu_surfaces.len() - 1),
                    })
                    .collect();
                ModelGpu {
                    owner: up.owner,
                    label: data.label.clone(),
                    vertices,
                    indices,
                    info: prepared.info.clone(),
                    parts,
                    surfaces: gpu_surfaces,
                }
            });
        if let Some(e) = gpu_error {
            return Err(RenderError::Gpu(e));
        }
        Ok(WorldModel(Arc::new(model)))
    }
}

// ---------------------------------------------------------------------------------------
// GPU-side model
// ---------------------------------------------------------------------------------------

#[derive(Debug)]
pub(crate) struct ModelPart {
    pub indices: Range<u32>,
    pub surface: usize,
}

/// What pipeline family a surface uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SurfaceKind {
    Opaque,
    Mask,
    TestBlend,
    Blend,
    /// Additive and the other fixed blends ([`AlphaMode::Fixed`]).
    Fixed(FixedBlend),
}

impl SurfaceKind {
    fn from_alpha(alpha: AlphaMode) -> Self {
        match alpha {
            AlphaMode::Opaque => Self::Opaque,
            AlphaMode::Mask { .. } => Self::Mask,
            AlphaMode::TestBlend { .. } => Self::TestBlend,
            AlphaMode::Blend => Self::Blend,
            AlphaMode::Additive => Self::Fixed(FixedBlend::ADDITIVE),
            AlphaMode::Fixed(b) => Self::Fixed(b),
        }
    }

    /// Drawn in the translucent stage.
    pub(crate) fn translucent(self) -> bool {
        matches!(self, Self::TestBlend | Self::Blend | Self::Fixed(_))
    }

    /// Casts the sun's shadow (opaque and cut-out surfaces, as lit meshes).
    fn casts(self) -> bool {
        matches!(self, Self::Opaque | Self::Mask | Self::TestBlend)
    }
}

/// Surface uniform (must match `Surface` in `instanced_common.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct SurfaceUniform {
    pub color: [f32; 4],
    /// rgb = Ms, w = power p (< 0: no specular).
    pub specular: [f32; 4],
    /// x = alpha-test threshold (< -0.5: no test), yzw unused.
    pub params: [f32; 4],
    /// x = flags (`SF_*`), y = sRGB re-encode bits, zw unused.
    pub flags: [u32; 4],
}

pub(crate) mod sf {
    pub const NORMAL_MAP: u32 = 1;
    pub const VERTEX_COLORS: u32 = 2;
    pub const UNLIT: u32 = 4;
    pub const GLOSS_BASE_ALPHA: u32 = 8;
    /// Opaque surface: the base texture's alpha is not an opacity.
    pub const OPAQUE: u32 = 16;
    pub const GLOSS_NORMAL_ALPHA: u32 = 32;
    /// The base texture's alpha is the gloss, not an opacity: opacity 1.
    pub const OPACITY_ONE: u32 = 64;
    pub const SRGB_BASE: u32 = 1;
    pub const SRGB_NORMAL: u32 = 2;
}

fn finite_or(v: f32, d: f32) -> f32 {
    if v.is_finite() { v } else { d }
}

impl SurfaceUniform {
    /// `normal_resolved`: the normal texture resolved (else the normal-map switch is off).
    fn new(s: &WorldSurface, has_tangents: bool, srgb: [bool; 2], normal_resolved: bool) -> Self {
        let color = s.base_color.map(|c| finite_or(c, 1.0));
        let (spec, power) = match s.specular {
            Some(sp) if finite_or(sp.shininess, 0.0) >= 0.5 => (
                sp.color.map(|c| finite_or(c, 0.0)),
                sp.shininess.min(4096.0),
            ),
            _ => ([0.0; 3], -1.0),
        };
        let cutoff = match s.alpha {
            AlphaMode::Mask { cutoff } | AlphaMode::TestBlend { cutoff } => finite_or(cutoff, 0.5),
            _ => -1.0,
        };
        let mut flags = 0;
        if has_tangents && normal_resolved {
            flags |= sf::NORMAL_MAP;
        }
        if s.vertex_colors {
            flags |= sf::VERTEX_COLORS;
        }
        if s.unlit {
            flags |= sf::UNLIT;
        }
        if s.gloss_from_base_alpha {
            // A gloss alpha never reaches the output alpha nor the alpha test (material.md
            // §11.2, §11.3): the surface has no opacity source.
            flags |= sf::GLOSS_BASE_ALPHA | sf::OPACITY_ONE;
        } else if s.gloss_from_normal_alpha && normal_resolved {
            flags |= sf::GLOSS_NORMAL_ALPHA;
        }
        if s.alpha == AlphaMode::Opaque {
            flags |= sf::OPAQUE;
        }
        let mut srgb_bits = 0;
        if srgb[0] {
            srgb_bits |= sf::SRGB_BASE;
        }
        if srgb[1] {
            srgb_bits |= sf::SRGB_NORMAL;
        }
        Self {
            color,
            specular: [spec[0], spec[1], spec[2], power],
            params: [cutoff, 0.0, 0.0, 0.0],
            flags: [flags, srgb_bits, 0, 0],
        }
    }
}

/// A surface's retained bind group and the textures it was built with: the ids that
/// resolved (an id that did not is bound as white and never re-resolved).
#[derive(Debug)]
struct SurfaceBind {
    bind_group: wgpu::BindGroup,
    textures: [Option<TextureId>; 2],
}

#[derive(Debug)]
pub(crate) struct SurfaceGpu {
    bind: Mutex<SurfaceBind>,
    surface: WorldSurface,
    has_tangents: bool,
    pub kind: SurfaceKind,
    pub two_sided: bool,
}

impl SurfaceGpu {
    /// `set`: the resolved base and normal textures (`None`: absent or unusable).
    fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        s: WorldSurface,
        has_tangents: bool,
        set: [Option<ResolvedTexture>; 2],
        white: &ResolvedTexture,
    ) -> Self {
        let ids = [s.base_texture, s.normal_texture];
        let bind = make_surface_bind(device, layout, &s, has_tangents, ids, &set, white);
        Self {
            bind: Mutex::new(bind),
            kind: SurfaceKind::from_alpha(s.alpha),
            two_sided: s.two_sided,
            surface: s,
            has_tangents,
        }
    }

    /// The surface's bind group. When a texture it was built with has been destroyed
    /// since, the bind group is rebuilt first without it (a destroyed texture may not be
    /// bound: the GPU would reject the whole submission), as if it had never resolved.
    pub(crate) fn bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        textures: &TextureResolver<'_>,
    ) -> wgpu::BindGroup {
        let mut b = self.bind.lock().unwrap_or_else(PoisonError::into_inner);
        let alive = |id: &Option<TextureId>| id.is_none() || textures.get(*id).is_some();
        if !b.textures.iter().all(alive) {
            tracing::debug!(
                textures = ?b.textures,
                "world model texture destroyed; surface rebuilt without it"
            );
            let ids = b.textures;
            let set = ids.map(|id| id.and_then(|_| textures.get(id)));
            *b = make_surface_bind(
                device,
                layout,
                &self.surface,
                self.has_tangents,
                ids,
                &set,
                &textures.white(),
            );
        }
        b.bind_group.clone()
    }
}

fn make_surface_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    s: &WorldSurface,
    has_tangents: bool,
    ids: [Option<TextureId>; 2],
    set: &[Option<ResolvedTexture>; 2],
    white: &ResolvedTexture,
) -> SurfaceBind {
    let base = set[0].as_ref().unwrap_or(white);
    let normal = set[1].as_ref().unwrap_or(white);
    let uniform = SurfaceUniform::new(s, has_tangents, [base.srgb, normal.srgb], set[1].is_some());
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("tessera-model-surface"),
        contents: bytemuck::bytes_of(&uniform),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("tessera-model-surface"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&base.view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&normal.view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&base.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(&normal.sampler),
            },
        ],
    });
    SurfaceBind {
        bind_group,
        textures: [
            ids[0].filter(|_| set[0].is_some()),
            ids[1].filter(|_| set[1].is_some()),
        ],
    }
}

pub(crate) struct ModelGpu {
    pub owner: u64,
    pub label: Option<String>,
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub info: MeshInfo,
    pub parts: Vec<ModelPart>,
    pub surfaces: Vec<SurfaceGpu>,
}

// ---------------------------------------------------------------------------------------
// Shared helpers of the render-world modules
// ---------------------------------------------------------------------------------------

/// The six planes of a reversed-depth `view_proj`, for sphere culling.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    pub(crate) fn new(view_proj: Mat4) -> Self {
        let r = view_proj.transpose();
        let (x, y, z, w) = (r.x_axis, r.y_axis, r.z_axis, r.w_axis);
        let planes = [w + x, w - x, w + y, w - y, z, w - z].map(|p| {
            let len = p.truncate().length();
            if len > 0.0 && len.is_finite() {
                p / len
            } else {
                Vec4::ZERO
            }
        });
        Self { planes }
    }

    /// Whether a sphere may be visible (conservative; non-finite input counts as visible).
    pub(crate) fn sphere_visible(&self, center: Vec3, radius: f32) -> bool {
        if !center.is_finite() || !radius.is_finite() {
            return true;
        }
        self.planes
            .iter()
            .all(|p| p.truncate().dot(center) + p.w >= -radius)
    }
}

/// The dynamic-offset stride of a uniform of `size` bytes.
pub(crate) fn uniform_stride(device: &wgpu::Device, size: usize) -> u64 {
    let a = u64::from(device.limits().min_uniform_buffer_offset_alignment).max(16);
    (size as u64).div_ceil(a) * a
}

/// Light-table indices packed as the lit shaders read them: x, y = four indices each (low
/// byte first), z = count.
pub(crate) fn light_words(lights: &[u8]) -> [u32; 3] {
    let mut words = [0u32; 2];
    for (i, &l) in lights.iter().take(MAX_DRAW_LIGHTS).enumerate() {
        words[i / 4] |= u32::from(l) << ((i % 4) * 8);
    }
    [words[0], words[1], lights.len().min(MAX_DRAW_LIGHTS) as u32]
}

/// A dynamic-offset uniform binding entry.
pub(crate) fn uniform_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    dynamic: bool,
    size: usize,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic,
            min_binding_size: wgpu::BufferSize::new(size as u64),
        },
        count: None,
    }
}

/// A filterable 2-D texture binding entry (fragment stage).
pub(crate) fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// A filtering sampler binding entry (fragment stage).
pub(crate) fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

/// Depth state of a world colour pipeline (reversed depth, [`DEPTH_COMPARE`]).
pub(crate) fn world_depth(write: bool) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(write),
        depth_compare: Some(DEPTH_COMPARE),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

/// Depth state of a sun shadow-map pipeline (standard depth, same bias as lit meshes).
pub(crate) fn shadow_depth() -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: SHADOW_FORMAT,
        depth_write_enabled: Some(true),
        depth_compare: Some(wgpu::CompareFunction::LessEqual),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState {
            constant: 0,
            slope_scale: 1.5,
            clamp: 0.0,
        },
    }
}

/// Shader source with the frame declarations and shading functions prepended.
pub(crate) fn lit_source(parts: &[&str]) -> String {
    let mut s = String::with_capacity(
        FRAME_WGSL.len()
            + FRAME_SHADING_WGSL.len()
            + parts.iter().map(|p| p.len()).sum::<usize>()
            + 8,
    );
    s.push_str(FRAME_WGSL);
    s.push('\n');
    s.push_str(FRAME_SHADING_WGSL);
    for p in parts {
        s.push('\n');
        s.push_str(p);
    }
    s
}

pub(crate) fn instanced_source() -> String {
    lit_source(&[INSTANCED_COMMON_WGSL, INSTANCED_WGSL])
}

pub(crate) fn common_source() -> &'static str {
    INSTANCED_COMMON_WGSL
}

pub(crate) const MESH_VERTEX_ATTRS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    0 => Float32x3, 1 => Snorm16x4, 2 => Snorm16x4, 3 => Float32x2, 4 => Unorm8x4
];

pub(crate) fn mesh_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: size_of::<GpuVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &MESH_VERTEX_ATTRS,
    }
}

// ---------------------------------------------------------------------------------------
// Pipelines
// ---------------------------------------------------------------------------------------

/// Per-instance vertex data (must match `InstIn` in `instanced.wgsl`). 96 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct GpuInstance {
    /// Rows of the affine model → world matrix.
    rows: [[f32; 4]; 3],
    tint: [f32; 4],
    /// rgb = terrain colour, w = sway phase.
    terrain: [f32; 4],
    /// The instance's light list: x, y = light-table indices (four u8 each, low byte
    /// first), z = count, w unused.
    lights: [u32; 4],
}

const INSTANCE_ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
    7 => Float32x4, 8 => Float32x4, 9 => Float32x4, 10 => Float32x4, 11 => Float32x4,
    12 => Uint32x4
];

/// Batch uniform (must match `Batch` in `instanced.wgsl`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct BatchUniform {
    /// x, y, z unused (the light lists are per instance), w = flags (`BF_*`).
    lights: [u32; 4],
    /// D, F, sway amplitude, terrain colour intensity.
    frill: [f32; 4],
    /// z_min, sway height h, fade-in, unused.
    sway: [f32; 4],
}

mod bf {
    pub const FRILL: u32 = 1;
    pub const SWAY: u32 = 2;
    /// The surfaces are drawn blended (a non-opaque frill group).
    pub const BLENDED: u32 = 4;
}

/// Bind group layouts shared by the instanced and sky pipelines.
#[derive(Debug)]
pub(crate) struct ModelLayouts {
    /// Group 1 of the instanced pipelines: the batch uniform (dynamic offset).
    pub batch: wgpu::BindGroupLayout,
    /// Group 2: surface uniform, base and normal textures, their samplers.
    pub surface: wgpu::BindGroupLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PipeKey {
    format: wgpu::TextureFormat,
    kind: SurfaceKind,
    two_sided: bool,
    /// Instances with a negative determinant: the front face is clockwise, so the other
    /// face is culled (`docs/specs/render.md` §2.3 Mirrored transforms; unused when
    /// two-sided).
    mirrored: bool,
}

/// Pipelines and layouts of the instanced path, built lazily, kept for the renderer's
/// life.
#[derive(Debug, Default)]
pub(crate) struct InstancedState {
    layouts: Option<Arc<ModelLayouts>>,
    shader: Option<wgpu::ShaderModule>,
    colour_layout: Option<wgpu::PipelineLayout>,
    shadow_layout: Option<wgpu::PipelineLayout>,
    pipelines: Vec<(PipeKey, wgpu::RenderPipeline)>,
    /// Shadow pipelines by alpha test (shadows are drawn without face culling).
    shadow: Vec<(bool, wgpu::RenderPipeline)>,
}

impl InstancedState {
    pub(crate) fn layouts(&mut self, device: &wgpu::Device) -> Arc<ModelLayouts> {
        if let Some(l) = &self.layouts {
            return l.clone();
        }
        let batch = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tessera-instanced-batch-bgl"),
            entries: &[uniform_entry(
                0,
                wgpu::ShaderStages::VERTEX_FRAGMENT,
                true,
                size_of::<BatchUniform>(),
            )],
        });
        let surface = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tessera-model-surface-bgl"),
            entries: &[
                uniform_entry(
                    0,
                    wgpu::ShaderStages::VERTEX_FRAGMENT,
                    false,
                    size_of::<SurfaceUniform>(),
                ),
                texture_entry(1),
                texture_entry(2),
                sampler_entry(3),
                sampler_entry(4),
            ],
        });
        let l = Arc::new(ModelLayouts { batch, surface });
        self.layouts = Some(l.clone());
        l
    }

    fn ensure_layouts(&mut self, ctx: &WorldFrame<'_>) {
        let layouts = self.layouts(ctx.device);
        if self.shader.is_none() {
            self.shader = Some(
                ctx.device
                    .create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("tessera-instanced.wgsl"),
                        source: wgpu::ShaderSource::Wgsl(instanced_source().into()),
                    }),
            );
        }
        if self.colour_layout.is_none() {
            self.colour_layout = Some(ctx.device.create_pipeline_layout(
                &wgpu::PipelineLayoutDescriptor {
                    label: Some("tessera-instanced-layout"),
                    bind_group_layouts: &[
                        Some(ctx.frame_layout),
                        Some(&layouts.batch),
                        Some(&layouts.surface),
                    ],
                    immediate_size: 0,
                },
            ));
        }
        if self.shadow_layout.is_none() {
            self.shadow_layout = Some(ctx.device.create_pipeline_layout(
                &wgpu::PipelineLayoutDescriptor {
                    label: Some("tessera-instanced-shadow-layout"),
                    bind_group_layouts: &[
                        Some(ctx.shadow_frame_layout),
                        Some(&layouts.batch),
                        Some(&layouts.surface),
                    ],
                    immediate_size: 0,
                },
            ));
        }
    }

    fn pipeline(&mut self, device: &wgpu::Device, key: PipeKey) -> Option<wgpu::RenderPipeline> {
        if let Some((_, p)) = self.pipelines.iter().find(|(k, _)| *k == key) {
            return Some(p.clone());
        }
        let (shader, layout) = (self.shader.as_ref()?, self.colour_layout.as_ref()?);
        let (fs, blend, write) = match key.kind {
            SurfaceKind::Opaque => ("fs_opaque", None, true),
            SurfaceKind::Mask => ("fs_mask", None, true),
            SurfaceKind::TestBlend => (
                "fs_test_blend",
                Some(wgpu::BlendState::ALPHA_BLENDING),
                true,
            ),
            SurfaceKind::Blend => ("fs_blend", Some(wgpu::BlendState::ALPHA_BLENDING), false),
            SurfaceKind::Fixed(b) => (
                match b.fog_rule() {
                    FogRule::Lerp => "fs_blend",
                    FogRule::FadeOut => "fs_additive",
                    FogRule::TowardWhite => "fs_modulate",
                },
                Some(b.blend_state()),
                b.depth_write,
            ),
        };
        let buffers = [
            Some(mesh_vertex_layout()),
            Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<GpuInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &INSTANCE_ATTRS,
            }),
        ];
        let p = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tessera-instanced"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &buffers,
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: match (key.two_sided, key.mirrored) {
                    (true, _) => None,
                    (false, false) => Some(wgpu::Face::Back),
                    (false, true) => Some(wgpu::Face::Front),
                },
                ..Default::default()
            },
            depth_stencil: Some(world_depth(write)),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(fs),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: key.format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.push((key, p.clone()));
        Some(p)
    }

    fn shadow_pipeline(
        &mut self,
        device: &wgpu::Device,
        masked: bool,
    ) -> Option<wgpu::RenderPipeline> {
        let key = masked;
        if let Some((_, p)) = self.shadow.iter().find(|(k, _)| *k == key) {
            return Some(p.clone());
        }
        let (shader, layout) = (self.shader.as_ref()?, self.shadow_layout.as_ref()?);
        let buffers = [
            Some(mesh_vertex_layout()),
            Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<GpuInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &INSTANCE_ATTRS,
            }),
        ];
        let fragment = masked.then(|| wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_shadow_mask"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[],
        });
        let p = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tessera-instanced-shadow"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_shadow"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &buffers,
            },
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(shadow_depth()),
            multisample: wgpu::MultisampleState::default(),
            fragment,
            multiview_mask: None,
            cache: None,
        });
        self.shadow.push((key, p.clone()));
        Some(p)
    }
}

// ---------------------------------------------------------------------------------------
// Per-frame hook
// ---------------------------------------------------------------------------------------

/// One instanced draw: a submesh of a batch's model with a range of the pass's instances.
#[derive(Debug)]
struct Item {
    model: WorldModel,
    part: usize,
    surface_bg: wgpu::BindGroup,
    batch_offset: u32,
    /// The instances drawn in colour (inside the view).
    instances: Range<u32>,
    /// The instances drawn into the shadow map: the colour range followed by the casters
    /// outside the view but inside the shadow map.
    shadow_instances: Range<u32>,
    /// The first item of its batch: it counts the batch's instances in the statistics.
    counts_instances: bool,
    pipeline: wgpu::RenderPipeline,
    translucent: bool,
    shadow: Option<wgpu::RenderPipeline>,
}

struct InstancedHook {
    items: Vec<Item>,
    instances: wgpu::Buffer,
    batch_bg: wgpu::BindGroup,
    culled: u32,
    /// Whether `culled` was already added to the frame's statistics (once per pass).
    counted: AtomicBool,
}

impl fmt::Debug for InstancedHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstancedHook")
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}

/// Model-space bounds of the sway weight (`scenery.md` §8.5 step 5): z_min and
/// h = max(0.0002, z_max − z_min), with h = 0 (no sway) when z_max − z_min ≤ 0.01. The
/// z range is the stored box's when given (and finite), else the model's computed box.
fn sway_bounds(info: &MeshInfo, stored: Option<[f32; 2]>) -> (f32, f32) {
    let (lo, hi) = match stored {
        Some([lo, hi]) if lo.is_finite() && hi.is_finite() => (lo, hi),
        _ => (info.aabb.min.z, info.aabb.max.z),
    };
    let span = hi - lo;
    if !span.is_finite() || span <= 0.01 {
        (finite_or(lo, 0.0), 0.0)
    } else {
        (lo, span.max(FRILL_SWAY_MIN))
    }
}

/// The model sphere placed with an instance transform: centre and radius (scaled by the
/// largest axis scale), or `None` when the transform or the result is not finite (such an
/// instance is dropped; unlike [`crate::BoundingSphere::transformed`] there is no fallback
/// to the model-space sphere).
fn placed_sphere(sphere: crate::BoundingSphere, m: Mat4) -> Option<(Vec3, f32)> {
    affine_rows(m)?;
    let center = m.transform_point3(sphere.center);
    let scale = [m.x_axis, m.y_axis, m.z_axis]
        .iter()
        .map(|a| a.truncate().length())
        .fold(0.0f32, f32::max);
    let radius = sphere.radius * scale;
    let ok = (center - radius).is_finite() && (center + radius).is_finite();
    ok.then_some((center, radius))
}

fn affine_rows(m: Mat4) -> Option<[[f32; 4]; 3]> {
    let r = m.transpose();
    let rows = [
        r.x_axis.to_array(),
        r.y_axis.to_array(),
        r.z_axis.to_array(),
    ];
    rows.iter().flatten().all(|v| v.is_finite()).then_some(rows)
}

/// Squared distance from `p` to the box `[lo, hi]`.
fn box_distance_sq(p: Vec3, lo: Vec3, hi: Vec3) -> f32 {
    let d = (lo - p).max(Vec3::ZERO).max(p - hi);
    d.length_squared()
}

/// The largest byte size of a per-pass uniform buffer bound with dynamic offsets: the
/// device's buffer limit, and offsets must fit in 32 bits.
pub(crate) fn max_uniform_bytes(device: &wgpu::Device) -> u64 {
    device.limits().max_buffer_size.min(u64::from(u32::MAX))
}

pub(crate) fn build_hook(
    state: &mut InstancedState,
    batches: Vec<InstanceBatch>,
    ctx: &WorldFrame<'_>,
) -> Result<Option<Arc<dyn PassHook>>> {
    if batches.is_empty() {
        return Ok(None);
    }
    state.ensure_layouts(ctx);
    let layouts = state.layouts(ctx.device);
    let frustum = Frustum::new(ctx.view_proj);
    let stride = uniform_stride(ctx.device, size_of::<BatchUniform>());
    // One uniform slot per queued batch at most: checked up front, so the outcome does not
    // depend on the view.
    let max_uniform = max_uniform_bytes(ctx.device);
    let uniform_bytes = (batches.len() as u64).saturating_mul(stride);
    if uniform_bytes > max_uniform {
        return Err(RenderError::LimitExceeded {
            what: "instanced batch uniform bytes per pass",
            requested: uniform_bytes,
            max: max_uniform,
        });
    }
    let max_instances =
        usize::try_from(ctx.device.limits().max_buffer_size / size_of::<GpuInstance>() as u64)
            .unwrap_or(usize::MAX)
            .min(MAX_INSTANCES_PER_PASS);
    let casts = ctx.frame.has_shadow();
    // Casters outside the view still cast into it (render.md §4.4): they are tested against
    // the shadow map's volume instead.
    let shadow_frustum = Frustum::new(Mat4::from_cols_array_2d(&ctx.frame.shadow_view_proj));
    let time_phase = |o: Vec3| (o.x * 0.37 + o.y * 0.21).rem_euclid(std::f32::consts::TAU);

    let mut uniforms: Vec<u8> = Vec::new();
    let mut gpu: Vec<GpuInstance> = Vec::new();
    let mut shadow_only: Vec<GpuInstance> = Vec::new();
    let mut mirrored: Vec<GpuInstance> = Vec::new();
    let mut items = Vec::new();
    let mut culled = 0u32;
    let mut dropped = 0u32;
    let mut chosen = Vec::with_capacity(MAX_DRAW_LIGHTS);
    let mut warned_foreign = false;
    for batch in batches {
        let model = batch.model.gpu();
        if model.owner != ctx.owner {
            if !warned_foreign {
                tracing::warn!("world model of another renderer skipped");
                warned_foreign = true;
            }
            continue;
        }
        let sphere = model.info.sphere;
        let start = gpu.len();
        let frill = batch.frill;
        // Frill group culling and reduction (scenery.md §8.3 rules 2 and 4).
        let mut take = batch.instances.len();
        if let Some(fp) = frill {
            let far = finite_or(fp.far_distance, 0.0).max(0.0);
            // The group's box encloses the instances that can be drawn: a non-finite
            // transform is dropped below and must not stretch the box (its sphere would
            // fall back to the model-space sphere at the origin).
            let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
            for inst in batch.instances.iter() {
                if let Some((c, r)) = placed_sphere(sphere, inst.transform) {
                    lo = lo.min(c - r);
                    hi = hi.max(c + r);
                }
            }
            // No drawable instance: nothing to draw, the group is culled whole.
            let d2 = if lo.is_finite() && hi.is_finite() {
                box_distance_sq(ctx.eye, lo, hi)
            } else {
                f32::INFINITY
            };
            if d2 >= far * far {
                culled = culled.saturating_add(u32::try_from(take).unwrap_or(u32::MAX));
                continue;
            }
            if let Some(red) = fp.reduction {
                take = frill_reduced_count(take, d2, far, red);
            }
        }
        culled = culled.saturating_add(u32::try_from(batch.instances.len() - take).unwrap_or(0));
        let casting = batch.cast_shadow && casts && model.surfaces.iter().any(|s| s.kind.casts());
        let max_d = batch.max_distance.filter(|d| d.is_finite());
        let (z_min, h) = sway_bounds(&model.info, frill.and_then(|f| f.sway_z_range));
        let intensity = frill.map_or(0.0, |fp| {
            finite_or(fp.animation_intensity, 0.0).clamp(0.0, 1.0)
        });
        let sways = frill.is_some() && intensity > FRILL_SWAY_MIN && h > 0.0;
        // The culling sphere (render.md §5.5 Culling, with the implementer note there): the
        // model sphere grown by the largest sway displacement and, for frills, enlarged to
        // hold the model origin, toward which the distance scale shrinks the vertices.
        let cull_sphere = if frill.is_some() {
            let sway = if sways {
                intensity * h * SWAY_REACH
            } else {
                0.0
            };
            let r = sphere.radius + sway;
            crate::BoundingSphere {
                center: sphere.center,
                radius: if r.is_finite() {
                    r.max(sphere.center.length())
                } else {
                    sphere.radius
                },
            }
        } else {
            sphere
        };
        // One light selection for the whole batch when it cannot depend on the instance.
        let uniform_lights = ctx.lights.is_uniform(batch.lights);
        if uniform_lights {
            ctx.lights.select(
                batch.lights,
                &LightTarget::object(Vec3::ZERO, Vec3::ZERO, None),
                &mut chosen,
            );
        }
        let batch_lights = light_words(&chosen);
        mirrored.clear();
        shadow_only.clear();
        for inst in batch.instances.iter().take(take) {
            let Some(rows) = affine_rows(inst.transform) else {
                culled = culled.saturating_add(1);
                continue;
            };
            let Some((center, radius)) = placed_sphere(cull_sphere, inst.transform) else {
                culled = culled.saturating_add(1);
                continue;
            };
            if let Some(d) = max_d {
                let start_d = placed_sphere(sphere, inst.transform)
                    .map_or(f32::INFINITY, |(c, r)| c.distance(ctx.eye) - r);
                if start_d > d {
                    culled = culled.saturating_add(1);
                    continue;
                }
            }
            let in_view = frustum.sphere_visible(center, radius);
            if !in_view {
                // Not drawn in colour; kept as a caster when its shadow may reach the map.
                culled = culled.saturating_add(1);
                if !(casting && shadow_frustum.sphere_visible(center, radius)) {
                    continue;
                }
            }
            if gpu.len() + mirrored.len() + shadow_only.len() >= max_instances {
                // Over the per-pass limit: dropped (and counted as culled), not an error,
                // so a dense view cannot fail the frame (render.md §5.5).
                if in_view {
                    culled = culled.saturating_add(1);
                }
                dropped = dropped.saturating_add(1);
                continue;
            }
            let origin = Vec3::new(rows[0][3], rows[1][3], rows[2][3]);
            let t = inst.terrain_color.map(|c| finite_or(c, 1.0));
            // The instance's own light list (lighting.md §8.5, row L32: one list per
            // object, against its own world box), unless every instance gets the same.
            let lights = if !in_view {
                [0; 3]
            } else if uniform_lights {
                batch_lights
            } else {
                ctx.lights.select(
                    batch.lights,
                    &LightTarget::placed(model.info.aabb, inst.transform, inst.light_object),
                    &mut chosen,
                );
                light_words(&chosen)
            };
            let g = GpuInstance {
                rows,
                tint: inst.tint.map(|c| finite_or(c, 1.0)),
                terrain: [t[0], t[1], t[2], time_phase(origin)],
                lights: [lights[0], lights[1], lights[2], 0],
            };
            if !in_view {
                shadow_only.push(g);
            } else if Mat3::from_mat4(inst.transform).determinant() < 0.0 {
                // Mirrored: drawn with the culled face swapped (render.md §2.3).
                mirrored.push(g);
            } else {
                gpu.push(g);
            }
        }
        let split = gpu.len();
        gpu.append(&mut mirrored);
        let mid = gpu.len();
        gpu.append(&mut shadow_only);
        if gpu.len() == start {
            continue;
        }
        let mut flags = 0;
        let mut u = BatchUniform {
            lights: [0; 4],
            frill: [0.0; 4],
            sway: [z_min, 0.0, 1.0, 0.0],
        };
        if let Some(fp) = frill {
            flags |= bf::FRILL;
            if sways {
                flags |= bf::SWAY;
            }
            if !fp.opaque {
                flags |= bf::BLENDED;
            }
            u.frill = [
                finite_or(fp.far_distance, 0.0).max(0.0),
                finite_or(fp.fade_length, 0.0).max(0.0),
                intensity,
                finite_or(fp.terrain_color_intensity, 1.0).clamp(0.0, 1.0),
            ];
            u.sway = [z_min, h, finite_or(fp.fade_in, 1.0).clamp(0.0, 1.0), 0.0];
        }
        u.lights[3] = flags;
        let offset = uniforms.len() as u64;
        let offset32 = u32::try_from(offset).map_err(|_| RenderError::LimitExceeded {
            what: "instanced batch uniform bytes per pass",
            requested: offset,
            max: u64::from(u32::MAX),
        })?;
        uniforms.extend_from_slice(bytemuck::bytes_of(&u));
        uniforms.resize((offset + stride) as usize, 0);
        let as_u32 = |i: usize| u32::try_from(i).unwrap_or(u32::MAX);
        let colour_range = as_u32(start)..as_u32(mid);
        // Upright and mirrored instances (one-sided surfaces cull opposite faces).
        let groups = [
            (as_u32(start)..as_u32(split), false),
            (as_u32(split)..as_u32(mid), true),
        ];
        let shadow_range = as_u32(start)..as_u32(gpu.len());
        let mut counted_part = None;
        for (pi, part) in model.parts.iter().enumerate() {
            let Some(surface) = model.surfaces.get(part.surface) else {
                continue;
            };
            let mut kind = surface.kind;
            if frill.is_some_and(|f| !f.opaque) && !kind.translucent() {
                kind = SurfaceKind::TestBlend;
            }
            let shadow = if casting && surface.kind.casts() {
                state.shadow_pipeline(ctx.device, surface.kind != SurfaceKind::Opaque)
            } else {
                None
            };
            // Two-sided surfaces cull nothing: one draw covers both groups.
            let draws: &[(Range<u32>, bool)] = if surface.two_sided {
                &[(colour_range.clone(), false)][..]
            } else {
                &groups[..]
            };
            let mut shadow = shadow;
            for (range, mirrored) in draws {
                let Some(pipeline) = state.pipeline(
                    ctx.device,
                    PipeKey {
                        format: ctx.format,
                        kind,
                        two_sided: surface.two_sided,
                        mirrored: *mirrored && !surface.two_sided,
                    },
                ) else {
                    continue;
                };
                // The shadow pass (no culling) draws the part once, with the first item.
                let item_shadow = shadow.take();
                if range.is_empty() && item_shadow.is_none() {
                    continue;
                }
                // The first drawn part counts the batch's instances (once per batch).
                let counts = *counted_part.get_or_insert(pi) == pi;
                items.push(Item {
                    model: batch.model.clone(),
                    part: pi,
                    surface_bg: surface.bind_group(ctx.device, &layouts.surface, &ctx.textures),
                    batch_offset: offset32,
                    instances: range.clone(),
                    shadow_instances: shadow_range.clone(),
                    counts_instances: counts,
                    pipeline,
                    translucent: kind.translucent(),
                    shadow: item_shadow,
                });
            }
        }
    }
    if dropped > 0 {
        tracing::warn!(
            dropped,
            max = max_instances,
            "instances over the per-pass limit dropped"
        );
    }
    if items.is_empty() && culled == 0 {
        return Ok(None);
    }
    if gpu.is_empty() {
        // Nothing to draw; the hook only reports what was culled. A vertex buffer may not
        // be empty.
        gpu.push(GpuInstance::zeroed());
    }
    if uniforms.is_empty() {
        uniforms.resize(stride as usize, 0);
    }
    let instances = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tessera-instances"),
            contents: bytemuck::cast_slice(&gpu),
            usage: wgpu::BufferUsages::VERTEX,
        });
    let batch_buffer = ctx
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tessera-instanced-batches"),
            contents: &uniforms,
            usage: wgpu::BufferUsages::UNIFORM,
        });
    let batch_bg = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("tessera-instanced-batches"),
        layout: &layouts.batch,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &batch_buffer,
                offset: 0,
                size: wgpu::BufferSize::new(size_of::<BatchUniform>() as u64),
            }),
        }],
    });
    Ok(Some(Arc::new(InstancedHook {
        items,
        instances,
        batch_bg,
        culled,
        counted: AtomicBool::new(false),
    })))
}

impl PassHook for InstancedHook {
    fn record(&self, rp: &mut wgpu::RenderPass<'_>, ctx: &HookRecord<'_>, stats: &mut FrameStats) {
        let Some((group0, offset)) = ctx.group0 else {
            return;
        };
        if !self.counted.swap(true, Ordering::Relaxed) {
            stats.instances_culled = stats.instances_culled.saturating_add(self.culled);
        }
        let shadow = ctx.stage == HookStage::Shadow;
        let want_translucent = ctx.stage == HookStage::Translucent;
        let mut bound = false;
        let mut current: Option<&wgpu::RenderPipeline> = None;
        let mut current_model: Option<*const ModelGpu> = None;
        for item in &self.items {
            let (pipeline, range) = if shadow {
                match &item.shadow {
                    Some(p) => (p, &item.shadow_instances),
                    None => continue,
                }
            } else if item.translucent == want_translucent {
                (&item.pipeline, &item.instances)
            } else {
                continue;
            };
            if range.is_empty() {
                continue;
            }
            let model = item.model.gpu();
            let Some(part) = model.parts.get(item.part) else {
                continue;
            };
            if !bound {
                rp.set_bind_group(0, group0, &[offset]);
                rp.set_vertex_buffer(1, self.instances.slice(..));
                bound = true;
            }
            if current != Some(pipeline) {
                rp.set_pipeline(pipeline);
                current = Some(pipeline);
            }
            let ptr: *const ModelGpu = model;
            if current_model != Some(ptr) {
                rp.set_vertex_buffer(0, model.vertices.slice(..));
                rp.set_index_buffer(model.indices.slice(..), wgpu::IndexFormat::Uint32);
                current_model = Some(ptr);
            }
            rp.set_bind_group(1, &self.batch_bg, &[item.batch_offset]);
            rp.set_bind_group(2, &item.surface_bg, &[]);
            rp.draw_indexed(part.indices.clone(), 0, range.clone());
            stats.draw_calls = stats.draw_calls.saturating_add(1);
            let n = range.end - range.start;
            if shadow {
                stats.shadow_draws = stats.shadow_draws.saturating_add(1);
            } else {
                // Instances once per batch; triangles per submesh.
                if item.counts_instances {
                    stats.instances = stats.instances.saturating_add(n);
                }
                stats.triangles = stats.triangles.saturating_add(
                    u64::from(n) * u64::from((part.indices.end - part.indices.start) / 3),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instanced_shader_validates() {
        let src = instanced_source();
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
    fn layouts_match_the_wgsl() {
        assert_eq!(size_of::<GpuInstance>(), 96);
        assert_eq!(size_of::<BatchUniform>(), 48);
        assert_eq!(size_of::<SurfaceUniform>(), 64);
    }

    /// A normal-and-gloss texture's alpha is the gloss (F2 frills drew near-white with an
    /// implied gloss of 1, M4.5 verification); the base alpha wins when both are set.
    #[test]
    fn gloss_comes_from_the_normal_alpha_when_asked() {
        let s = WorldSurface {
            gloss_from_normal_alpha: true,
            ..WorldSurface::default()
        };
        let flags = |s: &WorldSurface, resolved| {
            SurfaceUniform::new(s, true, [false; 2], resolved).flags[0]
        };
        assert_ne!(flags(&s, true) & sf::GLOSS_NORMAL_ALPHA, 0);
        assert_eq!(
            flags(&s, false) & sf::GLOSS_NORMAL_ALPHA,
            0,
            "no normal texture: gloss 1"
        );
        let both = WorldSurface {
            gloss_from_base_alpha: true,
            ..s
        };
        assert_eq!(flags(&both, true) & sf::GLOSS_NORMAL_ALPHA, 0);
        assert_ne!(flags(&both, true) & sf::GLOSS_BASE_ALPHA, 0);
        assert_eq!(
            flags(&WorldSurface::default(), true) & sf::GLOSS_NORMAL_ALPHA,
            0
        );
    }

    #[test]
    fn frill_radius_follows_rule_1() {
        let r: Vec<u32> = (-1..=6).map(|l| frill_radius_sectors(l, false)).collect();
        assert_eq!(r, vec![0, 0, 8, 10, 12, 14, 16, 0]);
        assert_eq!(frill_radius_sectors(5, true), 8);
        assert_eq!(frill_radius_sectors(1, true), 4);
    }

    #[test]
    fn frill_distances_follow_rules_2_and_3() {
        // Default level 2: radius 10 sectors, D = 97, F = 30, fade start 67.
        let (d, f) = frill_distances(10, true, 3.0);
        assert!((d - 97.0).abs() < 1e-4 && (f - 30.0).abs() < 1e-6);
        assert_eq!(frill_distances(10, false, 3.0), (1_000.0, 30.0));
        // F never exceeds D.
        assert_eq!(frill_distances(1, true, 3.0), (7.0, 7.0));
        assert_eq!(frill_distances(0, true, 3.0), (0.0, 0.0));
        assert_eq!(frill_distances(10, true, f32::NAN).1, 0.0);
    }

    #[test]
    fn fade_in_is_smoothstep_over_one_and_a_half_seconds() {
        assert_eq!(frill_fade_in(0.0), 0.0);
        assert_eq!(frill_fade_in(1.5), 1.0);
        assert_eq!(frill_fade_in(9.0), 1.0);
        assert_eq!(frill_fade_in(-1.0), 0.0);
        assert!((frill_fade_in(0.75) - 0.5).abs() < 1e-6);
        assert_eq!(frill_fade_in(f32::NAN), 1.0);
    }

    #[test]
    fn reduction_follows_rule_4() {
        let red = FrillReduction {
            distance: 40.0,
            granule: 4,
        };
        // Inside Rd: everything.
        assert_eq!(frill_reduced_count(40, 30.0 * 30.0, 97.0, red), 40);
        // At D: one granule.
        assert_eq!(frill_reduced_count(40, 97.0 * 97.0, 97.0, red), 4);
        // Half way in d^2: f = 0.5, trunc(0.5 * 10) = 5 granules.
        let mid = (40.0f32 * 40.0 + 97.0 * 97.0) * 0.5;
        assert_eq!(frill_reduced_count(40, mid, 97.0, red), 20);
        // Not a multiple of the granule: no reduction.
        assert_eq!(frill_reduced_count(41, 97.0 * 97.0, 97.0, red), 41);
        // 2 Rd > D: no reduction.
        assert_eq!(frill_reduced_count(40, 97.0 * 97.0, 70.0, red), 40);
        // Hostile values.
        assert_eq!(frill_reduced_count(40, f32::NAN, 97.0, red), 40);
        let zero = FrillReduction {
            distance: 40.0,
            granule: 0,
        };
        assert_eq!(frill_reduced_count(40, 97.0 * 97.0, 97.0, zero), 40);
    }

    #[test]
    fn frustum_culls_spheres_behind_and_beside() {
        let cam = crate::Camera {
            eye: Vec3::ZERO,
            target: Vec3::Y,
            up: Vec3::Z,
            ..crate::Camera::default()
        };
        let f = Frustum::new(cam.view_proj(1.0));
        assert!(f.sphere_visible(Vec3::new(0.0, 10.0, 0.0), 1.0));
        assert!(!f.sphere_visible(Vec3::new(0.0, -10.0, 0.0), 1.0));
        assert!(!f.sphere_visible(Vec3::new(100.0, 10.0, 0.0), 1.0));
        // A big sphere around the camera is visible.
        assert!(f.sphere_visible(Vec3::new(0.0, -10.0, 0.0), 20.0));
        assert!(f.sphere_visible(Vec3::NAN, 1.0));
    }

    #[test]
    fn light_words_pack_bytes() {
        assert_eq!(light_words(&[1, 2, 3, 4, 5]), [0x0403_0201, 5, 5]);
        assert_eq!(light_words(&[]), [0, 0, 0]);
        assert_eq!(light_words(&[9; 12])[2], 8);
    }

    #[test]
    fn sway_bounds_follow_step_5() {
        let mut info = MeshData::plane(1.0, 1.0, 1, 1)
            .validate(&MeshConfig::default())
            .expect("plane");
        info.aabb.min.z = 0.0;
        info.aabb.max.z = 0.005;
        assert_eq!(sway_bounds(&info, None), (0.0, 0.0));
        info.aabb.max.z = 2.0;
        assert_eq!(sway_bounds(&info, None), (0.0, 2.0));
        // The stored box wins when given (scenery.md §8.5 step 5); hostile values fall
        // back to the computed box.
        assert_eq!(sway_bounds(&info, Some([-1.0, 3.0])), (-1.0, 4.0));
        assert_eq!(sway_bounds(&info, Some([0.5, 0.505])), (0.5, 0.0));
        assert_eq!(sway_bounds(&info, Some([f32::NAN, 1.0])), (0.0, 2.0));
    }

    #[test]
    fn frill_defaults_are_level_2() {
        // scenery.md §8.3 rules 1-3, §8.4: Render.FrillDistance level 2 = 10 sectors,
        // D = 97, F = 30 (fade start 67).
        let p = FrillParams::default();
        assert!((p.far_distance - 97.0).abs() < 1e-4, "{p:?}");
        assert!((p.fade_length - 30.0).abs() < 1e-6, "{p:?}");
        assert_eq!(p.sway_z_range, None);
    }
}
