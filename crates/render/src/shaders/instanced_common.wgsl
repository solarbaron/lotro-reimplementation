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
const SF_SRGB_BASE: u32 = 1u;
const SF_SRGB_NORMAL: u32 = 2u;

@group(2) @binding(0) var<uniform> surface: Surface;
@group(2) @binding(1) var t_base: texture_2d<f32>;
@group(2) @binding(2) var t_normal: texture_2d<f32>;
@group(2) @binding(3) var s_base: sampler;
@group(2) @binding(4) var s_normal: sampler;

struct ModelIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) color: vec4<f32>,
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

fn surface_base(uv: vec2<f32>) -> vec4<f32> {
    return surface_to_stored(textureSample(t_base, s_base, uv), SF_SRGB_BASE);
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
