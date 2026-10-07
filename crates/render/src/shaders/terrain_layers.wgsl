// SPDX-License-Identifier: GPL-3.0-or-later
// The terrain layer pipeline (M4.5). Written for Tessera from docs/specs/terrain-texturing.md
// §5–§10 (opacity §7, blending §8, layer colour §9) and lighting.md §5; layout in
// docs/specs/render.md §5.2. Appended after frame.wgsl and frame_shading.wgsl. One
// pipeline serves every layer: the feature switches of §8.1 are uniform flags.

struct TerrainDraw {
    model: mat4x4<f32>,
    // Normal matrix columns.
    n0: vec4<f32>,
    n1: vec4<f32>,
    n2: vec4<f32>,
    // x, y = light-table indices (four u8 each, low byte first), z = count.
    lights: vec4<u32>,
};

struct TerrainLayer {
    // rgb = material colour M (2000), w = alpha_blend_max (1203).
    diffuse: vec4<f32>,
    // rgb = specular colour Ms (2001), w = specular power p (< 0: no specular).
    specular: vec4<f32>,
    // rgb = glow colour G (2014), w = alpha-test threshold tau.
    glow: vec4<f32>,
    // Texture matrix rows (§6.3): u = uv_u.x p + uv_u.y q + uv_u.z, likewise v.
    uv_u: vec4<f32>,
    uv_v: vec4<f32>,
    // Normalised texture-matrix columns c0.xy, c1.xy (§6.4).
    normal_cols: vec4<f32>,
    // x = coordinate set (0-2), y = opacity rule (OP_*), z = flags (TL_*),
    // w = sRGB re-encode bits (TS_*).
    params: vec4<u32>,
};

const OP_MASKED_DIFFUSE: u32 = 1u;
const OP_MASK: u32 = 2u;
const OP_DIFFUSE: u32 = 3u;

const TL_NORMAL_MAP: u32 = 1u;
const TL_SPECULAR: u32 = 2u;
const TL_GLOSS_NORMAL: u32 = 4u;
const TL_BAKED: u32 = 8u;
const TL_EMISSIVE: u32 = 16u;
const TL_GLOW: u32 = 32u;
const TL_FIRST: u32 = 64u;
const TL_MASK: u32 = 128u;

const TS_DIFFUSE: u32 = 1u;
const TS_NORMAL: u32 = 2u;
const TS_EMISSIVE: u32 = 4u;
const TS_GLOW: u32 = 8u;
const TS_MASK: u32 = 16u;
const TS_LIGHT: u32 = 32u;

@group(1) @binding(0) var<uniform> draw: TerrainDraw;

@group(2) @binding(0) var<uniform> layer: TerrainLayer;
@group(2) @binding(1) var t_diffuse: texture_2d<f32>;
@group(2) @binding(2) var t_normal: texture_2d<f32>;
@group(2) @binding(3) var t_emissive: texture_2d<f32>;
@group(2) @binding(4) var t_glow: texture_2d<f32>;
@group(2) @binding(5) var t_mask: texture_2d<f32>;
@group(2) @binding(6) var t_light: texture_2d<f32>;
// Layer textures: bilinear with mipmaps, wrap (§6.3).
@group(2) @binding(7) var s_repeat: sampler;
// Masks and the light map: bilinear, clamp (§5.2, §10).
@group(2) @binding(8) var s_clamp: sampler;

struct TerrainIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) binormal: vec3<f32>,
    @location(4) set0: vec2<f32>,
    @location(5) set1: vec2<f32>,
    @location(6) set2: vec2<f32>,
    @location(7) set3: vec2<f32>,
};

struct TerrainOut {
    // Every layer draws the same triangles: keep their depth bit-identical.
    @builtin(position) @invariant clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) binormal: vec3<f32>,
    // Layer texture coordinates (selected set, transformed).
    @location(4) uv: vec2<f32>,
    // Set 3: masks and the light map.
    @location(5) block_uv: vec2<f32>,
    @location(6) shadow: vec3<f32>,
    // x = fog factor (per vertex, lighting.md §10), y = view depth.
    @location(7) fog_depth: vec2<f32>,
};

fn layer_flag(bit: u32) -> bool {
    return (layer.params.z & bit) != 0u;
}

fn to_stored(c: vec4<f32>, bit: u32) -> vec4<f32> {
    if ((layer.params.w & bit) == 0u) {
        return c;
    }
    return vec4<f32>(frame_linear_to_srgb(c.rgb), c.a);
}

@vertex
fn vs_terrain(in: TerrainIn) -> TerrainOut {
    let world = draw.model * vec4<f32>(in.position, 1.0);
    let m3 = mat3x3<f32>(draw.model[0].xyz, draw.model[1].xyz, draw.model[2].xyz);
    let nm = mat3x3<f32>(draw.n0.xyz, draw.n1.xyz, draw.n2.xyz);
    var pq = in.set0;
    if (layer.params.x == 1u) {
        pq = in.set1;
    } else if (layer.params.x == 2u) {
        pq = in.set2;
    }
    let h = vec3<f32>(pq, 1.0);
    var out: TerrainOut;
    out.clip = frame.view_proj * world;
    out.world_pos = world.xyz;
    out.normal = nm * in.normal;
    out.tangent = m3 * in.tangent;
    out.binormal = m3 * in.binormal;
    out.uv = vec2<f32>(dot(layer.uv_u.xyz, h), dot(layer.uv_v.xyz, h));
    out.block_uv = in.set3;
    out.shadow = frame_shadow_coords(world.xyz);
    out.fog_depth = vec2<f32>(frame_fog_factor(world.xyz), out.clip.w);
    return out;
}

@vertex
fn vs_terrain_shadow(in: TerrainIn) -> @builtin(position) vec4<f32> {
    let world = draw.model * vec4<f32>(in.position, 1.0);
    return frame.shadow_view_proj * world;
}

fn light_index(i: u32) -> u32 {
    let word = select(draw.lights.x, draw.lights.y, i >= 4u);
    return (word >> ((i & 3u) * 8u)) & 255u;
}

@fragment
fn fs_terrain(in: TerrainOut) -> @location(0) vec4<f32> {
    // Sample everything first (uniform control flow).
    let c_tex = to_stored(textureSample(t_diffuse, s_repeat, in.uv), TS_DIFFUSE);
    let n_tex = to_stored(textureSample(t_normal, s_repeat, in.uv), TS_NORMAL);
    let em = to_stored(textureSample(t_emissive, s_repeat, in.uv), TS_EMISSIVE);
    let gl = to_stored(textureSample(t_glow, s_repeat, in.uv), TS_GLOW);
    let mask_tex = to_stored(textureSample(t_mask, s_clamp, in.block_uv), TS_MASK);
    let light = to_stored(textureSample(t_light, s_clamp, in.block_uv), TS_LIGHT);

    // Opacity (§7). The mask value is the texture's alpha (A8 decodes to (1, 1, 1, a)); a
    // layer without a mask reads 1.
    let m = select(1.0, clamp(mask_tex.a, 0.0, 1.0), layer_flag(TL_MASK));
    let a = clamp(c_tex.a, 0.0, 1.0);
    var alpha = 1.0;
    switch (layer.params.y) {
        case 1u: { alpha = clamp(a * m + m * m * layer.diffuse.w, 0.0, 1.0); }
        case 2u: { alpha = m; }
        case 3u: { alpha = a; }
        default: {}
    }
    // Discarded when alpha <= tau; the first drawn layer never is (§7 step 1).
    if (!layer_flag(TL_FIRST) && alpha <= layer.glow.w) {
        discard;
    }

    // Shading normal (§6.4, §9.2): n' = the texel turned by the texture transform, in the
    // frame (T, -B, N).
    let n0 = frame_normalize(in.normal, vec3<f32>(0.0, 0.0, 1.0));
    var n = n0;
    // Without a normal map the ambient (hemispheric) term and the sun's N . L0 take the
    // stored vertex normal as it is, not normalised (its length is at most 1 where the
    // triangles around a sample differ, §4.10); the table lights and the highlight use the
    // unit normal.
    // (Both terms are clamped to [0, 1], so a longer normal cannot overflow them.)
    var n_stored = in.normal;
    if (layer_flag(TL_NORMAL_MAP)) {
        let t = n_tex.xyz * 2.0 - 1.0;
        let c = layer.normal_cols;
        let r = frame_normalize(vec3<f32>(t.x * c.x + t.y * c.z, t.x * c.y + t.y * c.w, t.z),
            vec3<f32>(0.0, 0.0, 1.0));
        n = frame_normalize(r.x * in.tangent - r.y * in.binormal + r.z * n0, n0);
        n_stored = n;
    }

    // Gloss (§8.1, §9.4) and specular power.
    var gloss = a;
    if (layer_flag(TL_GLOSS_NORMAL)) {
        gloss = n_tex.a;
    }
    var power = -1.0;
    if (layer_flag(TL_SPECULAR)) {
        power = layer.specular.w;
    }

    // Light sum (§9.1): the light map, ambient with the hemispheric term, the sun with its
    // shadow, the table lights; M pre-multiplied into every light term.
    let mc = layer.diffuse.rgb;
    let v = frame_normalize(frame.eye.xyz - in.world_pos, n);
    var sum = frame_ambient(n_stored, mc);
    if (layer_flag(TL_BAKED)) {
        sum = sum + light.rgb;
    }
    let s = frame_shadow_factor(in.shadow, in.fog_depth.y);
    // Diffuse with the stored normal; the highlight (§9.4: g sat(n . H)^p F0 S, not
    // multiplied by N . L, the albedo or 2) with the unit normal.
    let sun = frame_sun(n, v, gloss, power, s);
    let sun_diffuse = frame_sun(n_stored, v, gloss, -1.0, s).diffuse;
    sum = sum + sun_diffuse * mc;
    let count = min(draw.lights.z, 8u);
    for (var i = 0u; i < count; i = i + 1u) {
        let idx = light_index(i);
        if (idx < frame.params.x) {
            let lt = frame_light(idx, in.world_pos, n, v, gloss, -1.0);
            sum = sum + lt.diffuse * mc;
        }
    }
    // colour = 2 C Sigma + the sun's specular (§9.4, after the factor 2), then emissive and
    // glow (§9.5), then fog (§9.3).
    var color = 2.0 * c_tex.rgb * sum + sun.specular * layer.specular.rgb;
    if (layer_flag(TL_EMISSIVE)) {
        color = color + em.rgb * layer.glow.rgb;
    }
    if (layer_flag(TL_GLOW)) {
        color = color + gl.rgb * layer.glow.rgb * mc;
    }
    color = frame_apply_fog(color, clamp(in.fog_depth.x, 0.0, 1.0));
    return vec4<f32>(frame_output(color), alpha);
}
