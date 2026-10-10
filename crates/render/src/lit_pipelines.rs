// SPDX-License-Identifier: GPL-3.0-or-later
//! The lit mesh pipeline (M4.5) and the sun's shadow-map pass: layouts, the lit fields of
//! the draw uniform, automatic light selection and the lazily built variants
//! (kind × cull × target format × static or skinned).
//!
//! Group 0 is the pass's frame bind group ([`crate::lighting`]): the [`FrameUniform`]
//! at the pass's dynamic offset, the shadow map and its comparison sampler. The shadow
//! pass binds a frame group without the shadow map (it renders into it). Group 1 is the
//! draw bind group shared with the M2 mesh pipelines (draw uniform + skinning palette).
//! Group 2 holds the eight material textures of [`LIT_TEXTURES`].

use glam::{Mat4, Vec3};

use crate::depth_bias::BiasKey;
use crate::lighting::{
    FRAME_SHADING_WGSL, FRAME_WGSL, FrameUniform, LightReach, MAX_DRAW_LIGHTS, reference,
};
use crate::material::{
    FixedAlpha, FixedBlend, FogRule, Gloss, LightSelection, LitSurface, Material, Opacity,
    SecondOpacity,
};
use crate::mesh::GpuVertex;
use crate::mesh_pipelines::{Cull, DrawUniform, MeshLayouts};
use crate::pipelines::{DEPTH_COMPARE, DEPTH_FORMAT};
use crate::skin::GpuSkin;

const LIT_COMMON_WGSL: &str = include_str!("shaders/lit_common.wgsl");
const LIT_WGSL: &str = include_str!("shaders/lit.wgsl");
const SHADOW_WGSL: &str = include_str!("shaders/shadow.wgsl");

/// Material textures of the lit pipeline, in binding order (samplers follow at
/// `binding + LIT_TEXTURES`).
pub(crate) const LIT_TEXTURES: usize = 8;

/// Binding index of the second colour layer among the material textures.
pub(crate) const LAYER2_TEXTURE: usize = 7;

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
    /// The output alpha is the opacity source alone ([`crate::AlphaMode::Fixed`],
    /// `material.md` §11.3).
    pub const FACTOR_OPACITY: u32 = 1 << 16;
    /// A second colour layer is drawn (`material.md` §10.3).
    pub const SECOND_LAYER: u32 = 1 << 17;
    /// The second layer's RGB is added (else multiplied).
    pub const LAYER_ADD: u32 = 1 << 18;
    /// The emissive texture reads texture matrix 1 (`material.md` §8.4).
    pub const EMISSIVE_UV1: u32 = 1 << 19;
    /// sRGB re-encode bit of the second colour layer (the shift above covers textures 0–6).
    pub const SRGB_LAYER2: u32 = 1 << 20;
    /// The second layer's alpha multiplies the base alpha ([`crate::LayerAlpha::Multiplied`]).
    pub const LAYER_ALPHA: u32 = 1 << 21;
    /// Base RGB alone, alpha 2 × both alphas ([`crate::LayerAlpha::OpacityOnly`]).
    pub const LAYER_OPACITY_ONLY: u32 = 1 << 22;
    /// Second opacity source of [`crate::Opacity::BaseWith`] (two bits: 0 none, 2 the
    /// emissive alpha, 3 the opacity texture's alpha).
    pub const OPACITY2_SHIFT: u32 = 23;
    /// The two opacity sources are added (else multiplied).
    pub const OPACITY_SUM: u32 = 1 << 25;
    /// The second layer's alpha is added to the base alpha ([`crate::LayerAlpha::Summed`]).
    pub const LAYER_SUM: u32 = 1 << 26;
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
    // The lit base pass always contains distance fog (lighting.md §2.1, §5.11); the switch
    // serves unlit surfaces only (§7 code 0x00).
    if lit.fog || !material.unlit {
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
    let base_code = if dye_in_base_alpha { 0 } else { 1 };
    // A source whose texture did not resolve contributes 1 (the white texture's alpha).
    let second_code = |second: SecondOpacity| match second {
        SecondOpacity::EmissiveAlpha if ctx.present[4] => 2,
        SecondOpacity::Texture(_) if ctx.present[6] => 3,
        _ => 0,
    };
    let (opacity, opacity2, sum) = match lit.opacity {
        Opacity::One => (0, 0, false),
        Opacity::BaseAlpha => (base_code, 0, false),
        Opacity::EmissiveAlpha => (second_code(SecondOpacity::EmissiveAlpha), 0, false),
        Opacity::Texture(id) => (second_code(SecondOpacity::Texture(id)), 0, false),
        // material.md §11.2 table: the base alpha with a second alpha, multiplied or added.
        Opacity::BaseWith { second, sum } => (base_code, second_code(second), sum),
    };
    flags |= gloss << lit_flag::GLOSS_SHIFT;
    flags |= opacity << lit_flag::OPACITY_SHIFT;
    flags |= opacity2 << lit_flag::OPACITY2_SHIFT;
    if sum && opacity2 != 0 {
        flags |= lit_flag::OPACITY_SUM;
    }
    // material.md §11.3, rule for unlit records: "opacity" in a blend factor is the §11.2
    // opacity source, or 1 without one (not × M.alpha).
    if matches!(material.alpha, crate::AlphaMode::Fixed(_)) {
        flags |= lit_flag::FACTOR_OPACITY;
    }
    // The blended fragment stages run the alpha test too (render.md R-D11, material.md
    // §11.2): a fixed blend with its record's threshold, every other blended kind with −1
    // (no fragment discarded).
    match material.alpha {
        crate::AlphaMode::Fixed(b) => u.params[0] = b.cutoff(),
        crate::AlphaMode::Blend | crate::AlphaMode::Additive => u.params[0] = -1.0,
        _ => {}
    }
    for (i, &srgb) in ctx.srgb.iter().enumerate() {
        if srgb {
            flags |= if i == LAYER2_TEXTURE {
                lit_flag::SRGB_LAYER2
            } else {
                1 << (lit_flag::SRGB_SHIFT + i as u32)
            };
        }
    }
    // material.md §10.3: the second colour layer, when its texture resolved.
    if let Some(l) = lit.second_layer
        && ctx.present[LAYER2_TEXTURE]
    {
        flags |= lit_flag::SECOND_LAYER;
        if l.add {
            flags |= lit_flag::LAYER_ADD;
        }
        flags |= match l.alpha {
            crate::LayerAlpha::Ignored => 0,
            crate::LayerAlpha::Multiplied => lit_flag::LAYER_ALPHA,
            crate::LayerAlpha::OpacityOnly => lit_flag::LAYER_OPACITY_ONLY,
            crate::LayerAlpha::Summed => lit_flag::LAYER_SUM,
        };
    }
    if lit.emissive_uv1 {
        flags |= lit_flag::EMISSIVE_UV1;
    }
    u.uv = crate::material::UvTransform::pack(lit.uv);
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

/// Directional table lights one draw receives with [`LightSelection::Auto`]: the single
/// extra directional slot of `lighting.md` §5.6 and §8.2 (count 0 or 1).
pub(crate) const MAX_EXTRA_DIRECTIONAL: usize = 1;

/// A point light of the table, for [`LightSelector`].
#[derive(Debug, Clone, Copy)]
struct PointEntry {
    position: Vec3,
    /// Selection radius ρ (`lighting.md` §8.5 step 1).
    reach: f32,
    index: u8,
    key: bool,
    baked: bool,
    parent: Option<u64>,
    only_parent: bool,
    never_parent: bool,
}

/// What a draw is, for light selection (`lighting.md` §8.5 steps 2–3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LightTarget {
    /// World-space axis-aligned bounding box; `None` when it is not finite (no point light
    /// reaches the draw).
    pub bounds: Option<(Vec3, Vec3)>,
    /// A terrain or ceiling chunk: lights flagged as baked into the landscape skip it.
    pub landscape: bool,
    /// The caller's object handle ([`crate::MeshDraw::light_object`]), for the parent
    /// switches.
    pub object: Option<u64>,
}

impl LightTarget {
    /// An object with the world box (`min`, `max`).
    pub(crate) fn object(min: Vec3, max: Vec3, object: Option<u64>) -> Self {
        let bounds =
            (min.is_finite() && max.is_finite() && min.cmple(max).all()).then_some((min, max));
        Self {
            bounds,
            landscape: false,
            object,
        }
    }

    /// The model-space box `aabb` placed with `transform`.
    pub(crate) fn placed(aabb: crate::Aabb, transform: Mat4, object: Option<u64>) -> Self {
        match aabb.transformed(transform) {
            Some(b) => Self::object(b.min, b.max, object),
            None => Self {
                bounds: None,
                landscape: false,
                object,
            },
        }
    }

    /// The same target as a terrain or ceiling chunk.
    pub(crate) fn landscape(mut self) -> Self {
        self.landscape = true;
        self
    }
}

/// Per-pass light selection: the table's directional lights and its usable point lights
/// (sorted along x), built once per lit pass so each draw only tests the point lights whose
/// x range can reach it and allocates nothing.
#[derive(Debug, Default)]
pub(crate) struct LightSelector {
    /// Directional table indices without a position (they reach every draw, `render.md`
    /// §4.3 Decision), in table order.
    directional: Vec<u8>,
    /// Directional table lights with a position and a selection radius (`lighting.md` §8.5,
    /// L39: tested like point lights, ordered by step 4), in table order.
    placed_directional: Vec<PointEntry>,
    /// The first directional table light without a position with `KeyLight` (a per-pixel
    /// candidate, `lighting.md` §8.5 step 2).
    key_directional: Option<u8>,
    /// Key flags of the table entries (for explicit lists).
    key: Vec<bool>,
    /// Directional flags of the table entries (for explicit lists).
    is_directional: Vec<bool>,
    /// Point lights with a positive range and selection radius, sorted by `position.x`.
    points: Vec<PointEntry>,
    /// Largest selection radius (for the x window).
    max_reach: f32,
    /// Point lights per draw: [`MAX_DRAW_LIGHTS`] minus the light-0 slot of the main light
    /// or, without one, of the default directional light (`lighting.md` §8.5 Decision).
    limit: usize,
    /// The frame has a main light.
    has_sun: bool,
    /// Valid table entries.
    table_len: usize,
}

/// The step-4 rank of a reaching light (`lighting.md` §8.5): key lights first, then by
/// squared box distance (Tessera Decision of §8.5), then by table index.
type Rank = (bool, f32, u8);

/// Whether rank `a` comes before rank `b`.
fn rank_less(a: Rank, b: Rank) -> bool {
    b.0.cmp(&a.0)
        .then(a.1.total_cmp(&b.1))
        .then(a.2.cmp(&b.2))
        .is_lt()
}

impl LightSelector {
    /// The selector of a pass with frame uniform `frame`; `reach[i]` describes table entry
    /// *i* ([`LightReach::default`] for entries past its end).
    pub(crate) fn new(frame: &FrameUniform, reach: &[LightReach]) -> Self {
        let table_len = (frame.params[0] as usize).min(frame.lights.len());
        let mut directional = Vec::new();
        let mut placed_directional = Vec::new();
        let mut key_directional = None;
        let mut key = Vec::with_capacity(table_len);
        let mut is_directional = Vec::with_capacity(table_len);
        let mut points = Vec::new();
        let mut max_reach = 0.0f32;
        let entry = |info: LightReach, position: Vec3, reach: f32, index: u8| PointEntry {
            position,
            reach,
            index,
            key: info.key_light,
            baked: info.baked_landscape,
            parent: info.parent,
            only_parent: info.only_influence_parent,
            never_parent: info.never_influence_parent,
        };
        for (i, l) in frame.lights.iter().take(table_len).enumerate() {
            let Ok(index) = u8::try_from(i) else { break };
            let info = reach.get(i).copied().unwrap_or_default();
            let is_key = info.key_light;
            key.push(is_key);
            let directional_light = l.specular_kind[3] > 0.5;
            is_directional.push(directional_light);
            if directional_light {
                // L39: a directional light with a position and ρ is walked like a point
                // light; without them it reaches every draw (the Decision).
                let rho = info.radius.filter(|r| r.is_finite() && *r >= 0.0);
                match (info.position.filter(|p| p.is_finite()), rho) {
                    (Some(p), Some(r)) => {
                        if r > 0.0 {
                            placed_directional.push(entry(info, p, r, index));
                        }
                    }
                    _ => {
                        directional.push(index);
                        if is_key && key_directional.is_none() {
                            key_directional = Some(index);
                        }
                    }
                }
                continue;
            }
            let r2 = l.color_r2[3];
            let position = Vec3::from_slice(&l.position_k[..3]);
            // `FrameUniform::new` leaves only finite positions and ranges; check anyway.
            if r2.is_nan() || r2 <= 0.0 || !position.is_finite() {
                continue;
            }
            let reach = info
                .radius
                .filter(|r| r.is_finite() && *r >= 0.0)
                .unwrap_or_else(|| r2.sqrt());
            if reach.is_nan() || reach <= 0.0 {
                // ρ = 0: the strict test `d² < ρ²` never passes. (An infinite range,
                // whose square overflowed, reaches everything.)
                continue;
            }
            max_reach = max_reach.max(reach);
            points.push(entry(info, position, reach, index));
        }
        points.sort_by(|a, b| a.position.x.total_cmp(&b.position.x));
        // lighting.md §8.5: at most 8 lights in all, the main light included. Without a
        // main light the default directional light stands in for it (Tessera Decision of
        // §8.5): it holds the light-0 slot and counts toward the 8, unless a key
        // directional light of the list takes that slot.
        let has_sun = frame.sun_dir[3] > 0.5;
        Self {
            directional,
            placed_directional,
            key_directional,
            key,
            is_directional,
            points,
            max_reach,
            limit: MAX_DRAW_LIGHTS - 1,
            has_sun,
            table_len,
        }
    }

    /// Point lights a draw receives from the table at most (`MAX_DRAW_LIGHTS` minus the
    /// light-0 slot).
    #[cfg(test)]
    pub(crate) fn limit(&self) -> usize {
        self.limit
    }

    /// Whether table entry `i` is a key directional light, which takes the light-0 slot in
    /// a frame without a main light.
    fn takes_light0(&self, i: u8) -> bool {
        let i = usize::from(i);
        !self.has_sun && self.is_directional.get(i) == Some(&true) && self.key.get(i) == Some(&true)
    }

    /// Whether [`Self::select`] gives every target the same lights for `selection`: an
    /// explicit list, none, or automatic selection in a pass without usable point lights
    /// or walked directional lights (only the directional lights that reach every draw).
    pub(crate) fn is_uniform(&self, selection: LightSelection) -> bool {
        match selection {
            LightSelection::None | LightSelection::List { .. } => true,
            LightSelection::Auto => self.points.is_empty() && self.placed_directional.is_empty(),
        }
    }

    /// The light-table indices `target` receives: 7, the eighth slot being light 0 of the
    /// main light or, without one, of the default directional light (`lighting.md` §8.5
    /// Decision), or 8 when a key directional table light takes that slot (no main light).
    ///
    /// [`LightSelection::Auto`] (`lighting.md` §8.5, `render.md` §4.3): the point lights
    /// whose squared distance to the target's box is below ρ², without the baked-landscape
    /// lights on terrain and ceiling chunks and with the parent switches applied; key lights
    /// first, each group nearest first (Tessera Decision of §8.5; ties by table index), at
    /// most 7. Directional table lights: one with a position and ρ is walked by the same
    /// test and ranked by the same order (L39); one without reaches every draw and follows
    /// in table order (the Decision). Then, without a main light, the first key directional
    /// light that reaches the target (light 0, not counted in the 7); then the first other
    /// directional light that reaches it (the extra directional slot, §5.6, §8.2) if fewer
    /// than 7 point lights were selected. Explicit lists keep their order and drop indices
    /// past the table.
    pub(crate) fn select(
        &self,
        selection: LightSelection,
        target: &LightTarget,
        out: &mut Vec<u8>,
    ) {
        out.clear();
        match selection {
            LightSelection::None => {}
            LightSelection::List { indices, count: n } => {
                let list = indices
                    .get(..usize::from(n).min(MAX_DRAW_LIGHTS))
                    .unwrap_or(&[]);
                let valid = || {
                    list.iter()
                        .copied()
                        .filter(|&i| usize::from(i) < self.table_len)
                };
                // 8 in all with the light-0 slot: 7, or 8 when a key directional light of
                // the list holds that slot (no main light, §8.5 Decision).
                let cap = self.limit + usize::from(valid().any(|i| self.takes_light0(i)));
                out.extend(valid().take(cap));
            }
            LightSelection::Auto => self.select_auto(target, out),
        }
    }

    /// Whether light `p` may reach `target` (§8.5 step 3, row L34): the never-parent
    /// switch for the light's parent, the only-parent switch for **every** other object,
    /// whether or not the light has a parent (so an unattached light with
    /// `Light_OnlyInfluenceParent` reaches nothing). A target without an object handle is
    /// never the parent.
    fn admits(p: &PointEntry, target: &LightTarget) -> bool {
        if p.baked && target.landscape {
            return false;
        }
        match p.parent {
            Some(parent) if target.object == Some(parent) => !p.never_parent,
            _ => !p.only_parent,
        }
    }

    /// The step-4 rank of `p` for `target` when it reaches it (§8.5 steps 2–3).
    fn reaches(p: &PointEntry, target: &LightTarget) -> Option<Rank> {
        let (min, max) = target.bounds?;
        if !Self::admits(p, target) {
            return None;
        }
        let d2 = reference::box_distance2(p.position, min, max);
        (!d2.is_nan() && d2 < p.reach * p.reach).then_some((p.key, d2, p.index))
    }

    /// The first walked directional light (L39) in step-4 order that reaches `target` and
    /// passes `keep`.
    fn best_placed_directional(
        &self,
        target: &LightTarget,
        keep: impl Fn(&PointEntry) -> bool,
    ) -> Option<u8> {
        self.placed_directional
            .iter()
            .filter(|p| keep(p))
            .filter_map(|p| Self::reaches(p, target))
            .fold(None, |best: Option<Rank>, r| match best {
                Some(b) if !rank_less(r, b) => Some(b),
                _ => Some(r),
            })
            .map(|r| r.2)
    }

    fn select_auto(&self, target: &LightTarget, out: &mut Vec<u8>) {
        // The first `limit` reaching lights in (key first, distance², index) order.
        let mut best: [Rank; MAX_DRAW_LIGHTS] = [(false, 0.0, 0); MAX_DRAW_LIGHTS];
        let mut len = 0usize;
        if let Some((min, max)) = target.bounds
            && self.limit > 0
            && !self.points.is_empty()
        {
            let (lo, hi) = (min.x - self.max_reach, max.x + self.max_reach);
            let start = self.points.partition_point(|p| p.position.x < lo);
            for p in self.points.get(start..).unwrap_or(&[]) {
                if p.position.x > hi {
                    break;
                }
                let Some(key) = Self::reaches(p, target) else {
                    continue;
                };
                if len == self.limit && !rank_less(key, best[len - 1]) {
                    continue;
                }
                let mut j = len.min(self.limit - 1);
                while j > 0 && rank_less(key, best[j - 1]) {
                    best[j] = best[j - 1];
                    j -= 1;
                }
                best[j] = key;
                len = (len + 1).min(self.limit);
            }
        }
        out.extend(best[..len].iter().map(|&(_, _, i)| i));
        // Without a main light, a key directional light takes the light-0 slot in place of
        // the default directional light (§8.5 step 2 and Decision): it does not count
        // toward the 7. A walked one that reaches the target comes first (L39).
        let light0 = if self.has_sun {
            None
        } else {
            self.best_placed_directional(target, |p| p.key)
                .or(self.key_directional)
        };
        if let Some(i) = light0 {
            out.push(i);
        }
        // The extra directional slot holds one light (lighting.md §5.6, §8.2: count 0 or
        // 1; §8.5 step 2 sends every directional light that is not light 0 there): the
        // first walked directional light that reaches the target, in step-4 order, else the
        // first directional table light without a position, other than light 0, after the
        // point lights, within the 7.
        if self.limit.saturating_sub(len).min(MAX_EXTRA_DIRECTIONAL) > 0 {
            let extra = self
                .best_placed_directional(target, |p| Some(p.index) != light0)
                .or_else(|| {
                    self.directional
                        .iter()
                        .copied()
                        .find(|&d| Some(d) != light0)
                });
            out.extend(extra);
        }
    }
}

/// What a lit pipeline draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum LitKind {
    Opaque,
    Mask,
    TestBlend,
    Blend,
    /// Additive and the other fixed blends ([`crate::AlphaMode::Fixed`]), with how they
    /// write alpha in the pass ([`FixedBlend::alpha_rule`]).
    Fixed(FixedBlend, FixedAlpha),
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
    /// The entity's depth bias for the colour kinds (`decals.md` §6.5); none for the
    /// shadow kinds, which keep the shadow pass's own bias.
    pub bias: BiasKey,
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

/// The mesh vertex with the stored binormal at location 13 (`GpuVertex`; the same
/// locations as the instanced pipeline's model vertex).
const VERTEX_ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
    0 => Float32x3, 1 => Snorm16x4, 2 => Snorm16x4, 3 => Float32x2, 4 => Unorm8x4,
    13 => Snorm16x4
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
        let mut constants: &[(&str, f64)] = &[];
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
            LitKind::Fixed(b, alpha) => {
                // A fixed blend that draws opaque (render.md §6.10, R12) has fog rule
                // `Lerp`, so its entry point applies `FIXED_OPAQUE`.
                let fixed = b.pipeline(alpha);
                constants = fixed.constants();
                (
                    match b.fog_rule() {
                        FogRule::Lerp => "fs_lit_blend",
                        FogRule::FadeOut => "fs_lit_additive",
                        FogRule::TowardWhite => "fs_lit_modulate",
                    },
                    fixed.blend,
                    b.depth_write,
                )
            }
            // Shadow kinds returned above.
            LitKind::Shadow | LitKind::ShadowMask => ("fs_lit_opaque", None, true),
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
                bias: key.bias.state(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.lit_shader,
                entry_point: Some(fs),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants,
                    ..Default::default()
                },
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

/// Shadow pipelines do not depend on the colour format, culling or the entity's bias.
fn normalize(key: LitKey) -> LitKey {
    if key.kind.is_shadow() {
        LitKey {
            format: wgpu::TextureFormat::R8Unorm,
            cull: Cull::None,
            bias: BiasKey::NONE,
            ..key
        }
    } else {
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::{FrameLighting, Light, LightFields, LightKind};
    use crate::material::{AlphaMode, Specular};

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

    /// lighting.md §2.1: the lit base pass always contains fog; the switch serves unlit
    /// surfaces (§7 code 0x00).
    #[test]
    fn fog_switch_acts_on_unlit_surfaces_only() {
        let fog = |unlit: bool, switch: bool| {
            let mut u: DrawUniform = bytemuck::Zeroable::zeroed();
            let m = Material {
                unlit,
                ..Material::default()
            };
            let lit = LitSurface {
                fog: switch,
                ..LitSurface::default()
            };
            lit_fields(&mut u, &m, &lit, &ctx(), &[]);
            u.lit_lights[3] & lit_flag::FOG != 0
        };
        assert!(fog(false, true));
        assert!(fog(false, false));
        assert!(fog(true, true));
        assert!(!fog(true, false));
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

    /// material.md §11.3: only fixed blends read the opacity source alone.
    #[test]
    fn fixed_blends_flag_the_factor_opacity() {
        for (alpha, want) in [
            (AlphaMode::Fixed(FixedBlend::ADDITIVE), true),
            (AlphaMode::Additive, false),
            (AlphaMode::Blend, false),
            (AlphaMode::BLEND_DEPTH_WRITE, false),
        ] {
            let mut u = bytemuck::Zeroable::zeroed();
            let m = Material {
                alpha,
                ..Material::default()
            };
            lit_fields(&mut u, &m, &LitSurface::default(), &ctx(), &[]);
            let set = u.lit_lights[3] & lit_flag::FACTOR_OPACITY != 0;
            assert_eq!(set, want, "{alpha:?}");
        }
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

    /// A unit box around `c` (or a point box with `half` 0).
    fn boxed(c: Vec3, half: f32) -> LightTarget {
        LightTarget::object(c - Vec3::splat(half), c + Vec3::splat(half), None)
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
        let sel = LightSelector::new(&frame_with(lights.clone(), true), &[]);
        assert_eq!(sel.limit(), MAX_DRAW_LIGHTS - 1);
        let mut out = Vec::new();
        // Box from x = 9 to 11: without reach entries ρ = R = 5, so lights at x in (4, 16).
        sel.select(
            LightSelection::Auto,
            &boxed(Vec3::new(10.0, 0.0, 0.0), 1.0),
            &mut out,
        );
        assert_eq!(out, vec![3, 4, 2, 5, 12]);
        // Far away: only the directional light.
        sel.select(
            LightSelection::Auto,
            &boxed(Vec3::splat(1e6), 1.0),
            &mut out,
        );
        assert_eq!(out, vec![12]);
        // A huge box contains everything: 7 at most with the sun, by index (all at 0).
        sel.select(LightSelection::Auto, &boxed(Vec3::ZERO, 1e5), &mut out);
        assert_eq!(out, vec![0, 1, 2, 3, 4, 5, 6]);
        // Without a main light the default directional light holds the light-0 slot and
        // counts toward the 8 (lighting.md §8.5 Decision): still 7 table lights.
        let no_sun = LightSelector::new(&frame_with(lights.clone(), false), &[]);
        no_sun.select(LightSelection::Auto, &boxed(Vec3::ZERO, 1e5), &mut out);
        assert_eq!(out, vec![0, 1, 2, 3, 4, 5, 6]);
        // A key directional light takes that slot instead: 7 point lights and it.
        let mut key_dir = vec![LightReach::default(); 13];
        key_dir[12].key_light = true;
        let keyed = LightSelector::new(&frame_with(lights.clone(), false), &key_dir);
        keyed.select(LightSelection::Auto, &boxed(Vec3::ZERO, 1e5), &mut out);
        assert_eq!(out, vec![0, 1, 2, 3, 4, 5, 6, 12]);
        // With a main light the key directional light goes to the extra slot, within 7.
        let keyed_sun = LightSelector::new(&frame_with(lights.clone(), true), &key_dir);
        keyed_sun.select(LightSelection::Auto, &boxed(Vec3::ZERO, 1e5), &mut out);
        assert_eq!(out, vec![0, 1, 2, 3, 4, 5, 6]);
        keyed_sun.select(
            LightSelection::Auto,
            &boxed(Vec3::new(10.0, 0.0, 0.0), 1.0),
            &mut out,
        );
        assert_eq!(out, vec![3, 4, 2, 5, 12]);
        keyed.select(
            LightSelection::List {
                indices: [1, 2, 3, 4, 5, 6, 7, 12],
                count: 8,
            },
            &boxed(Vec3::ZERO, 1.0),
            &mut out,
        );
        assert_eq!(out, vec![1, 2, 3, 4, 5, 6, 7, 12]);
        // Explicit lists drop indices past the table and keep the cap.
        let t = boxed(Vec3::ZERO, 1.0);
        sel.select(
            LightSelection::List {
                indices: [200, 1, 0, 0, 0, 0, 0, 0],
                count: 2,
            },
            &t,
            &mut out,
        );
        assert_eq!(out, vec![1]);
        sel.select(
            LightSelection::List {
                indices: [1, 2, 3, 4, 5, 6, 7, 8],
                count: 8,
            },
            &t,
            &mut out,
        );
        assert_eq!(out, vec![1, 2, 3, 4, 5, 6, 7]);
        no_sun.select(
            LightSelection::List {
                indices: [1, 2, 3, 4, 5, 6, 7, 8],
                count: u8::MAX,
            },
            &t,
            &mut out,
        );
        assert_eq!(out, vec![1, 2, 3, 4, 5, 6, 7]);
        sel.select(LightSelection::None, &t, &mut out);
        assert!(out.is_empty());
        // Hostile boxes select without panicking; a non-finite box gets no point light.
        sel.select(
            LightSelection::Auto,
            &LightTarget::object(Vec3::NAN, Vec3::INFINITY, None),
            &mut out,
        );
        assert_eq!(out, vec![12]);
        sel.select(
            LightSelection::Auto,
            &LightTarget::object(Vec3::splat(-f32::MAX), Vec3::splat(f32::MAX), None),
            &mut out,
        );
        let placed = LightTarget::placed(
            crate::Aabb {
                min: Vec3::ZERO,
                max: Vec3::splat(2.0),
            },
            Mat4::from_scale(Vec3::splat(f32::MAX)),
            None,
        );
        assert_eq!(placed.bounds, None);
    }

    /// lighting.md §8.5 steps 1–2: ρ from Range and Falloff, strict box test.
    #[test]
    fn selection_radius_and_box_test() {
        let at = |x: f32| Light::point(Vec3::new(x, 0.0, 0.0), 10.0, 0.1, [1.0; 3]);
        let lights = vec![at(-8.4), at(-8.6), at(18.0)];
        let fields = |falloff: f32, global: bool| LightFields {
            range: 10.0,
            falloff,
            global_light: global,
            ..LightFields::default()
        };
        // Falloff 1 (and above): ρ = 8.5; Falloff 0.5: 9.25; GlobalLight: 10.
        let reach = [
            LightReach::from_fields(&fields(1.0, false)),
            LightReach::from_fields(&fields(3.0, false)),
            LightReach::from_fields(&fields(0.5, true)),
        ];
        let close = |r: Option<f32>, want: f32| r.is_some_and(|r| (r - want).abs() < 1e-5);
        assert!(close(reach[0].radius, 8.5), "{:?}", reach[0].radius);
        assert!(close(reach[1].radius, 8.5));
        assert!(close(reach[2].radius, 10.0));
        assert!((reference::selection_radius(10.0, 0.5, false) - 9.25).abs() < 1e-6);
        let sel = LightSelector::new(&frame_with(lights, true), &reach);
        let mut out = Vec::new();
        // Box [0, 8] × [-1, 1] × [-1, 1]: light 0 at 8.4 < 8.5 reaches, light 1 at 8.6
        // does not; light 2 at 10 from the box with ρ = 10 does not (strict).
        let t = LightTarget::object(Vec3::new(0.0, -1.0, -1.0), Vec3::new(8.0, 1.0, 1.0), None);
        sel.select(LightSelection::Auto, &t, &mut out);
        assert_eq!(out, vec![0]);
        let t = LightTarget::object(Vec3::new(0.0, -1.0, -1.0), Vec3::new(8.1, 1.0, 1.0), None);
        sel.select(LightSelection::Auto, &t, &mut out);
        assert_eq!(out, vec![0, 2]);
        // The box distance, not the centre distance: a long thin box reaches far lights.
        assert_eq!(
            reference::box_distance2(
                Vec3::new(5.0, 3.0, 0.0),
                Vec3::ZERO,
                Vec3::new(10.0, 1.0, 0.0)
            ),
            4.0
        );
        assert_eq!(
            reference::box_distance2(Vec3::ONE, Vec3::ZERO, Vec3::splat(2.0)),
            0.0
        );
        // Hostile fields fall back to the defaults of a new light.
        assert!(close(
            Some(reference::selection_radius(f32::NAN, f32::NAN, false)),
            25.0 * 0.85
        ));
        assert_eq!(reference::selection_radius(-3.0, 0.0, false), 0.0);
    }

    /// lighting.md §8.5 steps 3–4: key lights first, baked lights skip landscape chunks,
    /// the parent switches.
    #[test]
    fn key_baked_and_parent_rules() {
        let lights: Vec<Light> = (0..10)
            .map(|i| Light::point(Vec3::new(i as f32, 0.0, 0.0), 50.0, 0.1, [1.0; 3]))
            .collect();
        let mut reach = vec![LightReach::default(); 10];
        reach[9].key_light = true;
        reach[7].key_light = true;
        reach[1].baked_landscape = true;
        reach[2] = LightReach {
            only_influence_parent: true,
            ..LightReach::default()
        }
        .with_parent(42);
        reach[3] = LightReach {
            never_influence_parent: true,
            ..LightReach::default()
        }
        .with_parent(42);
        // An unattached light with the only-parent switch reaches no object (row L34).
        reach[4].only_influence_parent = true;
        // An unattached light with the never-parent switch reaches every object.
        reach[5].never_influence_parent = true;
        let sel = LightSelector::new(&frame_with(lights, true), &reach);
        let mut out = Vec::new();
        let at0 = |object| LightTarget::object(Vec3::ZERO, Vec3::ZERO, object);
        // Key lights 7 and 9 first (nearest first), then 0, 1, 3, 5, 6 by distance; 2 only
        // reaches its parent, 4 nothing.
        sel.select(LightSelection::Auto, &at0(None), &mut out);
        assert_eq!(out, vec![7, 9, 0, 1, 3, 5, 6]);
        // Another object with a handle: the same.
        sel.select(LightSelection::Auto, &at0(Some(7)), &mut out);
        assert_eq!(out, vec![7, 9, 0, 1, 3, 5, 6]);
        // The parent: 2 reaches it, 3 does not; 4 still reaches nothing.
        sel.select(LightSelection::Auto, &at0(Some(42)), &mut out);
        assert_eq!(out, vec![7, 9, 0, 1, 2, 5, 6]);
        // A landscape chunk: the baked light 1 is skipped (L30).
        sel.select(LightSelection::Auto, &at0(None).landscape(), &mut out);
        assert_eq!(out, vec![7, 9, 0, 3, 5, 6, 8]);
    }

    /// lighting.md §5.6, §8.2: one extra directional slot per draw (count 0 or 1), filled
    /// with the first directional table light after the point lights (other than a key
    /// directional light holding light 0, §8.5).
    #[test]
    fn one_directional_table_light_per_draw() {
        let mut lights = vec![Light::point(Vec3::ZERO, 50.0, 0.1, [1.0; 3])];
        for _ in 0..3 {
            lights.push(Light {
                kind: LightKind::Directional { direction: Vec3::Z },
                color: [1.0; 3],
                specular: [1.0; 3],
            });
        }
        lights.push(Light::point(Vec3::X, 50.0, 0.1, [1.0; 3]));
        let mut out = Vec::new();
        for sun in [true, false] {
            let sel = LightSelector::new(&frame_with(lights.clone(), sun), &[]);
            let t = LightTarget::object(Vec3::ZERO, Vec3::ZERO, None);
            sel.select(LightSelection::Auto, &t, &mut out);
            assert_eq!(out, vec![0, 4, 1], "sun {sun}");
            // No point light reaches a non-finite box; the directional light still does.
            let t = LightTarget::object(Vec3::NAN, Vec3::NAN, None);
            sel.select(LightSelection::Auto, &t, &mut out);
            assert_eq!(out, vec![1], "sun {sun}");
            // An explicit list is the caller's: kept as given.
            sel.select(
                LightSelection::List {
                    indices: [1, 2, 3, 0, 0, 0, 0, 0],
                    count: 3,
                },
                &t,
                &mut out,
            );
            assert_eq!(out, vec![1, 2, 3]);
        }
        // A key directional light (table entry 2): without a main light it takes light 0
        // and another directional light still fills the extra slot (§8.5 step 2, §8.2), 8
        // in all; with a main light it is only a candidate for the extra slot.
        let mut reach = vec![LightReach::default(); lights.len()];
        reach[2].key_light = true;
        let t = LightTarget::object(Vec3::ZERO, Vec3::ZERO, None);
        let keyed = LightSelector::new(&frame_with(lights.clone(), false), &reach);
        keyed.select(LightSelection::Auto, &t, &mut out);
        assert_eq!(out, vec![0, 4, 2, 1]);
        let keyed_sun = LightSelector::new(&frame_with(lights.clone(), true), &reach);
        keyed_sun.select(LightSelection::Auto, &t, &mut out);
        assert_eq!(out, vec![0, 4, 1]);
        // The key directional light alone: no other directional light to add.
        reach = vec![LightReach::default(); 3];
        reach[1].key_light = true;
        let lone = LightSelector::new(&frame_with(lights[..2].to_vec(), false), &reach);
        lone.select(LightSelection::Auto, &t, &mut out);
        assert_eq!(out, vec![0, 1]);
        // Seven point lights fill the 7: the key directional light takes light 0 (8 in
        // all) and the extra slot stays empty.
        let mut many: Vec<Light> = (0..7)
            .map(|i| Light::point(Vec3::new(i as f32, 0.0, 0.0), 50.0, 0.1, [1.0; 3]))
            .collect();
        many.extend(lights[1..4].iter().copied());
        let mut reach = vec![LightReach::default(); many.len()];
        reach[8].key_light = true;
        let full = LightSelector::new(&frame_with(many, false), &reach);
        full.select(LightSelection::Auto, &t, &mut out);
        assert_eq!(out, vec![0, 1, 2, 3, 4, 5, 6, 8]);
    }

    /// `lighting.md` §8.5, L39: a directional table light with a position and ρ is walked
    /// like a point light (box distance below ρ², parent switches) and ordered by step 4
    /// (key first, nearest first) for the light-0 and extra directional slots; one without
    /// a position still reaches every draw.
    #[test]
    fn walked_directional_lights_follow_l39() {
        let dir = || Light {
            kind: LightKind::Directional { direction: Vec3::Z },
            color: [1.0; 3],
            specular: [1.0; 3],
        };
        // 0 point, 1 directional without a position, 2 and 3 directional with positions.
        let lights = vec![
            Light::point(Vec3::ZERO, 50.0, 0.1, [1.0; 3]),
            dir(),
            dir(),
            dir(),
        ];
        let placed = |x: f32, rho: f32| LightReach {
            radius: Some(rho),
            position: Some(Vec3::new(x, 0.0, 0.0)),
            ..LightReach::default()
        };
        let reach = vec![
            LightReach::default(),
            LightReach::default(),
            placed(20.0, 10.0),
            placed(5.0, 10.0),
        ];
        let sel = LightSelector::new(&frame_with(lights.clone(), true), &reach);
        assert!(!sel.is_uniform(LightSelection::Auto));
        let mut out = Vec::new();
        let near = LightTarget::object(Vec3::ZERO, Vec3::ZERO, None);
        // Entry 3 reaches (distance 5 < 10) and fills the extra slot before the
        // position-less entry 1; entry 2 (distance 20) does not reach.
        sel.select(LightSelection::Auto, &near, &mut out);
        assert_eq!(out, vec![0, 3]);
        // Out of reach of both walked ones: the position-less directional light.
        let far = LightTarget::object(
            Vec3::new(-100.0, 0.0, 0.0),
            Vec3::new(-99.0, 0.0, 0.0),
            None,
        );
        sel.select(LightSelection::Auto, &far, &mut out);
        assert_eq!(out, vec![1]);
        // Both walked ones reach a box spanning them: the nearer first.
        let wide = LightTarget::object(Vec3::new(14.0, 0.0, 0.0), Vec3::new(15.0, 0.0, 0.0), None);
        sel.select(LightSelection::Auto, &wide, &mut out);
        assert_eq!(out, vec![0, 2]);
        // Key first (step 4): a key walked light wins the extra slot over a nearer one, and
        // without a main light takes light 0, the nearer one then filling the extra slot.
        let mut keyed = reach.clone();
        keyed[2].key_light = true;
        let sel = LightSelector::new(&frame_with(lights.clone(), true), &keyed);
        sel.select(LightSelection::Auto, &wide, &mut out);
        assert_eq!(out, vec![0, 2]);
        let sel = LightSelector::new(&frame_with(lights.clone(), false), &keyed);
        sel.select(LightSelection::Auto, &wide, &mut out);
        assert_eq!(out, vec![0, 2, 3]);
        // The parent switches apply as for point lights.
        let mut only = reach;
        only[3].only_influence_parent = true;
        let sel = LightSelector::new(&frame_with(lights, true), &only);
        sel.select(LightSelection::Auto, &near, &mut out);
        assert_eq!(out, vec![0, 1]);
    }

    /// The bounded, windowed selection equals a brute-force sort of every reaching light.
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
        let mut reach = Vec::new();
        let lights: Vec<Light> = (0..200)
            .map(|i| {
                reach.push(LightReach {
                    radius: (i % 3 == 0).then(|| next() * 0.08),
                    key_light: i % 11 == 0,
                    baked_landscape: i % 4 == 0,
                    ..LightReach::default()
                });
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
            let sel = LightSelector::new(&frame, &reach);
            let mut out = Vec::new();
            for c in 0..120 {
                let center = Vec3::new(c as f32 * 0.85, 2.0, 0.5);
                let half = Vec3::new((c % 5) as f32, (c % 3) as f32 * 0.5, 0.25);
                let landscape = c % 2 == 0;
                let mut t = LightTarget::object(center - half, center + half, None);
                if landscape {
                    t = t.landscape();
                }
                sel.select(LightSelection::Auto, &t, &mut out);
                let mut near: Vec<(bool, f32, u8)> = Vec::new();
                let mut dir = Vec::new();
                for (i, l) in frame
                    .lights
                    .iter()
                    .take(frame.params[0] as usize)
                    .enumerate()
                {
                    if l.specular_kind[3] > 0.5 {
                        dir.push(i as u8);
                        continue;
                    }
                    let r = reach[i];
                    if r.baked_landscape && landscape {
                        continue;
                    }
                    let rho = r.radius.unwrap_or(l.color_r2[3].sqrt());
                    let p = Vec3::from_slice(&l.position_k[..3]);
                    let d2 = reference::box_distance2(p, center - half, center + half);
                    if d2 < rho * rho {
                        near.push((r.key_light, d2, i as u8));
                    }
                }
                near.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
                let mut expected: Vec<u8> = near.iter().map(|&(_, _, i)| i).collect();
                expected.truncate(sel.limit());
                let key_dir = dir
                    .iter()
                    .copied()
                    .find(|&i| reach[usize::from(i)].key_light);
                let room = sel.limit() - expected.len();
                let light0 = key_dir.filter(|_| !sun);
                // No main light: the key directional light holds light 0 (§8.5).
                expected.extend(light0);
                // One extra directional slot (§5.6, §8.2), after the point lights.
                expected.extend(
                    dir.into_iter()
                        .filter(|&d| Some(d) != light0)
                        .take(room.min(1)),
                );
                assert_eq!(out, expected, "centre {center}, half {half}, sun {sun}");
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
