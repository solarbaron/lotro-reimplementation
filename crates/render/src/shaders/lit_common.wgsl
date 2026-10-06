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
@group(2) @binding(7) var s_base: sampler;
@group(2) @binding(8) var s_normal: sampler;
@group(2) @binding(9) var s_dye: sampler;
@group(2) @binding(10) var s_gloss: sampler;
@group(2) @binding(11) var s_emissive: sampler;
@group(2) @binding(12) var s_glow: sampler;
@group(2) @binding(13) var s_opacity: sampler;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) color: vec4<f32>,
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

// The opacity texel alpha (material.md §11.2 source): 1, base alpha, emissive alpha or
// the opacity texture's alpha. Used by the alpha test (also in the shadow pass); only the
// base-alpha source also reaches the output alpha (lighting.md §5.12).
fn lit_opacity(base: vec4<f32>, emissive: vec4<f32>, opacity: vec4<f32>) -> f32 {
    switch (lit_source(LIT_OPACITY_SHIFT)) {
        case 1u: { return base.a; }
        case 2u: { return emissive.a; }
        case 3u: { return opacity.a; }
        default: { return 1.0; }
    }
}
