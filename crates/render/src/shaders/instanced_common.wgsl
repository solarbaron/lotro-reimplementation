// SPDX-License-Identifier: GPL-3.0-or-later
// Surfaces of world models (instanced scenery, frills and sky objects; M4.5). Written for
// Tessera. Appended after frame.wgsl and frame_shading.wgsl; `Surface` matches
// `SurfaceUniform` in crates/render/src/instanced.rs (docs/specs/render.md §5.5).

struct Surface {
    // Material colour M (rgba).
    color: vec4<f32>,
    // rgb = material specular colour Ms, w = specular power p (< 0: no specular).
    specular: vec4<f32>,
    // x = alpha-test threshold (< -0.5: no test).
    params: vec4<f32>,
    // x = flags (SF_*), y = sRGB re-encode bits (SF_SRGB_*).
    flags: vec4<u32>,
    // Texture matrices 0 and 1 (material.md §8.4), rows ((a, b, c), (d, e, f)) packed as
    // (a0, b0, c0, d0), (e0, f0, a1, b1), (c1, d1, e1, f1).
    uv0: vec4<f32>,
    uv1: vec4<f32>,
    uv2: vec4<f32>,
};

const SF_NORMAL_MAP: u32 = 1u;
const SF_VERTEX_COLORS: u32 = 2u;
const SF_UNLIT: u32 = 4u;
const SF_GLOSS_BASE_ALPHA: u32 = 8u;
// Opaque surface: the base texture's alpha is not an opacity.
const SF_OPAQUE: u32 = 16u;
// Gloss from the normal texture's alpha (a normal-and-gloss texture).
const SF_GLOSS_NORMAL_ALPHA: u32 = 32u;
// The base texture's alpha is not an opacity (it is the gloss): the opacity is 1 for the
// alpha test and the output alpha (material.md §11.2, §11.3).
const SF_OPACITY_ONE: u32 = 64u;
// An unlit surface drawn without distance fog (WorldSurfaceOptions::fog off; lighting.md §7
// code 0x00).
const SF_NO_FOG: u32 = 128u;
// A fixed-function blend (AlphaMode::Fixed): the opacity a blend factor reads is the opacity
// texel alone, not x M.alpha (material.md §11.3, rule for unlit records).
const SF_FACTOR_OPACITY: u32 = 256u;
// A second colour layer (material.md §10.3): RGB added (SF_LAYER_ADD) or multiplied; its
// alpha multiplies the base alpha only with SF_LAYER_ALPHA (role ColorAndOpacity2, MA54);
// SF_LAYER_OPACITY_ONLY: the ColorTexture record (§20.1), base RGB alone and alpha
// 2 x both alphas.
const SF_SECOND_LAYER: u32 = 512u;
const SF_LAYER_ADD: u32 = 1024u;
const SF_LAYER_ALPHA: u32 = 2048u;
const SF_LAYER_OPACITY_ONLY: u32 = 4096u;
const SF_SRGB_BASE: u32 = 1u;
const SF_SRGB_NORMAL: u32 = 2u;
const SF_SRGB_LAYER2: u32 = 4u;

@group(2) @binding(0) var<uniform> surface: Surface;
@group(2) @binding(1) var t_base: texture_2d<f32>;
@group(2) @binding(2) var t_normal: texture_2d<f32>;
@group(2) @binding(3) var s_base: sampler;
@group(2) @binding(4) var s_normal: sampler;
@group(2) @binding(5) var t_layer2: texture_2d<f32>;
@group(2) @binding(6) var s_layer2: sampler;

// A texture-coordinate pair through packed matrix k (0 or 1) of `p0`..`p2`.
fn uv_through(p0: vec4<f32>, p1: vec4<f32>, p2: vec4<f32>, k: u32, uv: vec2<f32>) -> vec2<f32> {
    if (k == 0u) {
        return vec2<f32>(p0.x * uv.x + p0.y * uv.y + p0.z, p0.w * uv.x + p1.x * uv.y + p1.y);
    }
    return vec2<f32>(p1.z * uv.x + p1.w * uv.y + p2.x, p2.y * uv.x + p2.z * uv.y + p2.w);
}

struct ModelIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) color: vec4<f32>,
    // The stored binormal (+V), zero when the mesh has none (GpuVertex::binormal).
    @location(13) binormal: vec4<f32>,
};

fn surface_flag(bit: u32) -> bool {
    return (surface.flags.x & bit) != 0u;
}

// A texture uploaded sRGB decodes when sampled; re-encode it to the stored value
// (lighting.md §5.1, docs/specs/render.md §4.1).
fn surface_to_stored(c: vec4<f32>, srgb_bit: u32) -> vec4<f32> {
    if ((surface.flags.y & srgb_bit) == 0u) {
        return c;
    }
    return vec4<f32>(frame_linear_to_srgb(c.rgb), c.a);
}

// The base colour at `uv0` with the second colour layer at `uv1` applied (material.md
// §10.3).
fn surface_base_at(uv0: vec2<f32>, uv1: vec2<f32>) -> vec4<f32> {
    let base = surface_to_stored(textureSample(t_base, s_base, uv0), SF_SRGB_BASE);
    let layer2 = surface_to_stored(textureSample(t_layer2, s_layer2, uv1), SF_SRGB_LAYER2);
    if (!surface_flag(SF_SECOND_LAYER)) {
        return base;
    }
    if (surface_flag(SF_LAYER_OPACITY_ONLY)) {
        // 2 x both layer alphas, unclamped: the material alpha multiplies it next and only
        // the output (the 8-bit target) clamps (material.md §20.1, MA46).
        return vec4<f32>(base.rgb, 2.0 * base.a * layer2.a);
    }
    var rgb = base.rgb * layer2.rgb;
    if (surface_flag(SF_LAYER_ADD)) {
        rgb = base.rgb + layer2.rgb;
    }
    var a = base.a;
    if (surface_flag(SF_LAYER_ALPHA)) {
        a = a * layer2.a;
    }
    return vec4<f32>(rgb, a);
}

// The base colour through the surface's own texture matrices.
fn surface_base(uv: vec2<f32>) -> vec4<f32> {
    return surface_base_at(uv_through(surface.uv0, surface.uv1, surface.uv2, 0u, uv),
        uv_through(surface.uv0, surface.uv1, surface.uv2, 1u, uv));
}

fn surface_normal_texel(uv: vec2<f32>) -> vec4<f32> {
    return surface_to_stored(textureSample(t_normal, s_normal, uv), SF_SRGB_NORMAL);
}

// The opacity texel of a surface: the base texture's alpha, or 1 when that alpha is not an
// opacity (SF_OPACITY_ONE). Drives the alpha test (also in the shadow pass).
fn surface_opacity_texel(base: vec4<f32>) -> f32 {
    return select(base.a, 1.0, surface_flag(SF_OPACITY_ONE));
}

// The opacity a surface's base texel contributes to the output alpha.
fn surface_opacity(base: vec4<f32>) -> f32 {
    return select(surface_opacity_texel(base), 1.0, surface_flag(SF_OPAQUE));
}

// The alpha test of lit surfaces (lighting.md §5.12): discarded when opacity <= threshold.
fn surface_alpha_test(opacity: f32) -> bool {
    return surface.params.x >= -0.5 && opacity <= surface.params.x;
}
