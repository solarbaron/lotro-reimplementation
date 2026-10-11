// SPDX-License-Identifier: GPL-3.0-or-later
// Draw uniform, skinning and material textures of the lit mesh pipeline and its shadow
// pass (M4.5). Written for Tessera. Appended after frame.wgsl; the draw uniform matches
// `DrawUniform` in crates/render/src/mesh_pipelines.rs (the M2 fields are shared with
// mesh.wgsl, the `lit*` fields are read only here).

struct Draw {
    model: mat4x4<f32>,
    normal0: vec4<f32>,
    normal1: vec4<f32>,
    normal2: vec4<f32>,
    // Material colour M (rgba).
    base_color: vec4<f32>,
    // rgb = dye colour.
    dye: vec4<f32>,
    // rgb = material specular colour Ms, w = M2 shininess (unused here).
    specular: vec4<f32>,
    // x = alpha-test threshold, y = normal-map green sign, z, w = overlay values.
    params: vec4<f32>,
    overlay_color: vec4<f32>,
    normal_color: vec4<f32>,
    // x = feature bits (F_*), y = shading mode, z = dye mask source.
    flags: vec4<u32>,
    // rgb = glow colour G, w = specular power p.
    lit: vec4<f32>,
    // x, y = light-table indices (four u8 each, low byte first), z = light count,
    // w = lit flags (LIT_*).
    lit_lights: vec4<u32>,
    // Texture matrices 0 and 1 (material.md §8.4), rows ((a, b, c), (d, e, f)) packed as
    // (a0, b0, c0, d0), (e0, f0, a1, b1), (c1, d1, e1, f1).
    uv0: vec4<f32>,
    uv1: vec4<f32>,
    uv2: vec4<f32>,
};

const F_NORMAL_MAP: u32 = 1u;
const F_DYE: u32 = 2u;
const F_UNLIT: u32 = 16u;
const F_MIRRORED: u32 = 64u;
const DYE_BASE_ALPHA: u32 = 2u;

const LIT_VERTEX_COLORS: u32 = 1u;
const LIT_FOG: u32 = 2u;
const LIT_EMISSIVE: u32 = 4u;
const LIT_GLOW: u32 = 8u;
const LIT_SPECULAR: u32 = 16u;
const LIT_GLOSS_SHIFT: u32 = 5u;
const LIT_OPACITY_SHIFT: u32 = 7u;
// Texture uploaded sRGB: sampling decodes it, re-encode to the stored value.
const LIT_SRGB_BASE: u32 = 512u;
const LIT_SRGB_NORMAL: u32 = 1024u;
const LIT_SRGB_DYE: u32 = 2048u;
const LIT_SRGB_GLOSS: u32 = 4096u;
const LIT_SRGB_EMISSIVE: u32 = 8192u;
const LIT_SRGB_GLOW: u32 = 16384u;
const LIT_SRGB_OPACITY: u32 = 32768u;
// A fixed-function blend (AlphaMode::Fixed): the output alpha a blend factor reads is the
// tested opacity alone, not x M.alpha (material.md §11.3, rule for unlit records).
const LIT_FACTOR_OPACITY: u32 = 65536u;
// A second colour layer (material.md §10.3): sampled at texture matrix 1, its RGB added
// (LIT_LAYER_ADD) or multiplied; its alpha multiplies the base alpha only with
// LIT_LAYER_ALPHA (role ColorAndOpacity2, MA54); LIT_LAYER_OPACITY_ONLY: the ColorTexture
// record (§20.1), base RGB alone and alpha 2 x both alphas.
const LIT_SECOND_LAYER: u32 = 131072u;
const LIT_LAYER_ADD: u32 = 262144u;
// The emissive texture reads texture matrix 1 (material.md §8.4).
const LIT_EMISSIVE_UV1: u32 = 524288u;
const LIT_SRGB_LAYER2: u32 = 1048576u;
const LIT_LAYER_ALPHA: u32 = 2097152u;
const LIT_LAYER_OPACITY_ONLY: u32 = 4194304u;
// A second opacity source (Opacity::BaseWith, material.md §11.2 table): two bits, 0 the
// constant 1 (a texture that is 0 or did not resolve, material.md §18 Decision), 2 the
// emissive alpha, 3 the opacity texture's alpha; LIT_OPACITY_SUM adds the two alphas
// (else they are multiplied), so with source 0 a sum is the first alpha + 1. LIT_LAYER_SUM: the second colour layer's alpha is added to
// the base alpha (the sum records with ColorAndOpacity2).
const LIT_OPACITY2_SHIFT: u32 = 23u;
const LIT_OPACITY_SUM: u32 = 33554432u;
const LIT_LAYER_SUM: u32 = 67108864u;

const SOURCE_ONE: u32 = 0u;
const SOURCE_BASE: u32 = 1u;
const SOURCE_SECOND: u32 = 2u;
const SOURCE_TEXTURE: u32 = 3u;

@group(1) @binding(0) var<uniform> draw: Draw;

struct Palette {
    rows: array<vec4<f32>, 768>,
};
@group(1) @binding(1) var<uniform> palette: Palette;

@group(2) @binding(0) var t_base: texture_2d<f32>;
@group(2) @binding(1) var t_normal: texture_2d<f32>;
@group(2) @binding(2) var t_dye: texture_2d<f32>;
@group(2) @binding(3) var t_gloss: texture_2d<f32>;
@group(2) @binding(4) var t_emissive: texture_2d<f32>;
@group(2) @binding(5) var t_glow: texture_2d<f32>;
@group(2) @binding(6) var t_opacity: texture_2d<f32>;
@group(2) @binding(7) var t_layer2: texture_2d<f32>;
@group(2) @binding(8) var s_base: sampler;
@group(2) @binding(9) var s_normal: sampler;
@group(2) @binding(10) var s_dye: sampler;
@group(2) @binding(11) var s_gloss: sampler;
@group(2) @binding(12) var s_emissive: sampler;
@group(2) @binding(13) var s_glow: sampler;
@group(2) @binding(14) var s_opacity: sampler;
@group(2) @binding(15) var s_layer2: sampler;

// Texture coordinates through texture matrix 0 and 1 (material.md §8.4).
fn lit_uv0(uv: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(draw.uv0.x * uv.x + draw.uv0.y * uv.y + draw.uv0.z,
        draw.uv0.w * uv.x + draw.uv1.x * uv.y + draw.uv1.y);
}

fn lit_uv1(uv: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(draw.uv1.z * uv.x + draw.uv1.w * uv.y + draw.uv2.x,
        draw.uv2.y * uv.x + draw.uv2.z * uv.y + draw.uv2.w);
}

// The base colour with the second colour layer applied (material.md §10.3): RGB added or
// multiplied, the alphas multiplied when the layer's role carries opacity (MA54).
fn lit_layered(base: vec4<f32>, layer2: vec4<f32>) -> vec4<f32> {
    if (!lit_flag(LIT_SECOND_LAYER)) {
        return base;
    }
    if (lit_flag(LIT_LAYER_OPACITY_ONLY)) {
        // 2 x both layer alphas, unclamped: the material alpha multiplies it next and only
        // the output (the 8-bit target) clamps (material.md §20.1, MA46).
        return vec4<f32>(base.rgb, 2.0 * base.a * layer2.a);
    }
    var rgb = base.rgb * layer2.rgb;
    if (lit_flag(LIT_LAYER_ADD)) {
        rgb = base.rgb + layer2.rgb;
    }
    var a = base.a;
    if (lit_flag(LIT_LAYER_ALPHA)) {
        a = a * layer2.a;
    }
    if (lit_flag(LIT_LAYER_SUM)) {
        // Unclamped: the output alpha is clamped (material.md §11.3).
        a = a + layer2.a;
    }
    return vec4<f32>(rgb, a);
}

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) color: vec4<f32>,
    // The stored binormal (+V), zero when the mesh has none (GpuVertex::binormal).
    @location(13) binormal: vec4<f32>,
};

struct SkinIn {
    @location(5) joints: vec4<u32>,
    @location(6) weights: vec4<f32>,
};

fn lit_flag(bit: u32) -> bool {
    return (draw.lit_lights.w & bit) != 0u;
}

fn lit_source(shift: u32) -> u32 {
    return (draw.lit_lights.w >> shift) & 3u;
}

fn lit_to_stored(c: vec4<f32>, srgb_bit: u32) -> vec4<f32> {
    if (!lit_flag(srgb_bit)) {
        return c;
    }
    let x = clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let e = select(1.055 * pow(x, vec3<f32>(1.0 / 2.4)) - 0.055, x * 12.92,
        x <= vec3<f32>(0.0031308));
    return vec4<f32>(e, c.a);
}

// Linear blend skinning, as mesh.wgsl: v + sum_k w_k (M_k v - v).
fn bone_point(j: u32, p: vec3<f32>) -> vec3<f32> {
    let b = min(j, 255u) * 3u;
    let h = vec4<f32>(p, 1.0);
    return vec3<f32>(dot(palette.rows[b], h), dot(palette.rows[b + 1u], h),
        dot(palette.rows[b + 2u], h));
}

fn bone_dir(j: u32, d: vec3<f32>) -> vec3<f32> {
    let b = min(j, 255u) * 3u;
    return vec3<f32>(dot(palette.rows[b].xyz, d), dot(palette.rows[b + 1u].xyz, d),
        dot(palette.rows[b + 2u].xyz, d));
}

fn skin_point(s: SkinIn, p: vec3<f32>) -> vec3<f32> {
    var acc = p;
    if (s.weights.x > 0.0) { acc = acc + s.weights.x * (bone_point(s.joints.x, p) - p); }
    if (s.weights.y > 0.0) { acc = acc + s.weights.y * (bone_point(s.joints.y, p) - p); }
    if (s.weights.z > 0.0) { acc = acc + s.weights.z * (bone_point(s.joints.z, p) - p); }
    if (s.weights.w > 0.0) { acc = acc + s.weights.w * (bone_point(s.joints.w, p) - p); }
    return acc;
}

fn skin_dir(s: SkinIn, d: vec3<f32>) -> vec3<f32> {
    var acc = d;
    if (s.weights.x > 0.0) { acc = acc + s.weights.x * (bone_dir(s.joints.x, d) - d); }
    if (s.weights.y > 0.0) { acc = acc + s.weights.y * (bone_dir(s.joints.y, d) - d); }
    if (s.weights.z > 0.0) { acc = acc + s.weights.z * (bone_dir(s.joints.z, d) - d); }
    if (s.weights.w > 0.0) { acc = acc + s.weights.w * (bone_dir(s.joints.w, d) - d); }
    return acc;
}

fn lit_opacity_part(code: u32, base: vec4<f32>, emissive: vec4<f32>, opacity: vec4<f32>) -> f32 {
    switch (code) {
        case 1u: { return base.a; }
        case 2u: { return emissive.a; }
        case 3u: { return opacity.a; }
        default: { return 1.0; }
    }
}

// The tested opacity (material.md §11.2 table): 1, the base alpha, the emissive alpha or the
// opacity texture's alpha, or the base alpha times (or plus) a second one. Used by the alpha
// test (also in the shadow pass) and, times M.alpha, as the output alpha (§11.3).
fn lit_opacity(base: vec4<f32>, emissive: vec4<f32>, opacity: vec4<f32>) -> f32 {
    let a = lit_opacity_part(lit_source(LIT_OPACITY_SHIFT), base, emissive, opacity);
    // Source 0 reads 1: a product keeps the first alpha, a sum adds 1.
    let b = lit_opacity_part(lit_source(LIT_OPACITY2_SHIFT), base, emissive, opacity);
    if (lit_flag(LIT_OPACITY_SUM)) {
        return a + b;
    }
    return a * b;
}
