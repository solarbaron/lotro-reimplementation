// SPDX-License-Identifier: GPL-3.0-or-later
// The water pipeline (M4.5; own-program surfaces 2026-10-09). Written for Tessera from
// docs/specs/water.md §8.3–§8.7 and the Tessera rules there: the plain water family
// (§8.3–§8.4), plain water on mesh UV and older bump water (§8.5.6), flow-map water
// (§8.6.2, colour by the §8.6.4 Tessera rule), lava (§8.6.5), the mirror record of puddles
// (§8.5.4) and the liquid-distortion records (§8.5.5). Layout in docs/specs/render.md §5.3.
// Appended after frame.wgsl and frame_shading.wgsl.

struct Water {
    // Mesh draws: model -> world (identity for landscape quads).
    model: mat4x4<f32>,
    // x = wave speed, y = wave frequency, z = wave amplitude, w = normal layer count.
    waves: vec4<f32>,
    // Normal layer speeds: layer 0 (u, v), layer 1 (u, v). Flow map and lava: layer 0 holds
    // the phase rates 1057, 1058.
    speed01: vec4<f32>,
    // Layer 2 speed (u, v), layer 0 scale, layer 1 scale.
    speed2_scale01: vec4<f32>,
    // x = layer 2 scale, y = reflection distortion, z = refraction distortion,
    // w = Fresnel bias.
    misc0: vec4<f32>,
    // x = Fresnel exponent, y = specular power, z = overbright, w = sparkle threshold.
    misc1: vec4<f32>,
    // x = fog amount, y = scene copy bound (1/0), z = normal map (0 none, 1 linear,
    // 2 uploaded sRGB: re-encode after sampling), w = grid cells per quad side.
    misc2: vec4<f32>,
    specular_color: vec4<f32>,
    water_color: vec4<f32>,
    fog_color: vec4<f32>,
    shoreline_color: vec4<f32>,
    // x = 1 / target width, y = 1 / target height, zw = block offset O.
    screen: vec4<f32>,
    // x = far-draw fade start s, y = inverse range r (0: no fade); far-view.md §7.2 and
    // the §8 OPEN (Tessera applies the far terrain's fade to the sector water).
    fade: vec4<f32>,
    // x = family (FAM_*), y = textures present (T_*), z = textures uploaded sRGB (T_*),
    // w = flags (FL_*).
    kind: vec4<u32>,
    // Flow map and lava: foam offset (1182, 1183), foam tiling (1184, 1185).
    flow0: vec4<f32>,
    // Flow map and lava: depth offset (1186, 1187), rotation 1188, unused.
    flow1: vec4<f32>,
    // Older bump water: x = 1055, y = 1056. Mirror: x = 1100, y = 1101, z = 1102, w = 1103.
    extra0: vec4<f32>,
    // Mirror: x, y = 1104, 1105 (evaluated), z = 1106. Liquid: x = alpha-test threshold.
    extra1: vec4<f32>,
    // Mirror and liquid: the material colour 2000 (RGBA).
    material: vec4<f32>,
    // Liquid: texture matrix 0, rows (a, b, c, 0) and (d, e, f, 0).
    uv_row0: vec4<f32>,
    uv_row1: vec4<f32>,
    // Lit liquid: light-table indices (x, y: four u8 each, low byte first), z = count.
    lights: vec4<u32>,
};

const FAM_PLAIN: u32 = 0u;
const FAM_PLAIN_UV: u32 = 1u;
const FAM_BUMP: u32 = 2u;
const FAM_FLOW: u32 = 3u;
const FAM_LAVA: u32 = 4u;
const FAM_MIRROR: u32 = 5u;
const FAM_LIQUID: u32 = 6u;
const FAM_LIQUID_LIT: u32 = 7u;

const T_NORMAL: u32 = 1u;
const T_FLOW: u32 = 2u;
const T_FOAM: u32 = 4u;
const T_NOISE: u32 = 8u;
const T_COLOR0: u32 = 16u;
const T_COLOR1: u32 = 32u;
const T_EMISSIVE: u32 = 64u;

// The liquid layers read the world position B instead of the mesh UV (0x2B000442).
const FL_WORLD_UV: u32 = 1u;
// The liquid's alpha test is on.
const FL_ALPHA_TEST: u32 = 2u;
// Flow-map water with the colour list of 0x2B0007FD (water.md §8.6.4 steps 1-6: Fresnel
// bias x (...)^(1 / exponent), tint toward Lc^2 x WaterFogColor, specular x Lc); else the
// list of 0x2B0005EC (steps 1-5 of its own list: Fresnel of §8.4 step 5, tint toward
// WaterFogColor itself, no Lc anywhere; W42).
const FL_FLOW_FAKE_LIGHT: u32 = 4u;
// The liquid's output is multiplied by the vertex colour (0x2B00063E, §8.5.5 step 5).
const FL_VERTEX_COLOR: u32 = 8u;

@group(1) @binding(0) var<uniform> water: Water;
@group(1) @binding(1) var t_normal: texture_2d<f32>;
@group(1) @binding(2) var t_scene: texture_2d<f32>;
@group(1) @binding(3) var s_repeat: sampler;
@group(1) @binding(4) var s_clamp: sampler;
@group(1) @binding(5) var t_flow: texture_2d<f32>;
@group(1) @binding(6) var t_foam: texture_2d<f32>;
@group(1) @binding(7) var t_noise: texture_2d<f32>;
@group(1) @binding(8) var t_color0: texture_2d<f32>;
@group(1) @binding(9) var t_color1: texture_2d<f32>;
@group(1) @binding(10) var t_emissive: texture_2d<f32>;
@group(1) @binding(11) var s_color0: sampler;
@group(1) @binding(12) var s_color1: sampler;
@group(1) @binding(13) var s_emissive: sampler;

struct QuadIn {
    // origin.xy, size.xy
    @location(0) rect: vec4<f32>,
    // x = height
    @location(1) height: vec4<f32>,
    // u, v at the south-west corner, du, dv (water.md §8.1).
    @location(2) uv: vec4<f32>,
};

struct MeshIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) color: vec4<f32>,
    @location(13) binormal: vec4<f32>,
};

struct WaterOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    // B = (P.xy - O) / 160 (water.md §8.3).
    @location(1) block: vec2<f32>,
    // The mesh's UV set 0 (landscape quads: the sector coordinates are not used).
    @location(2) uv: vec2<f32>,
    // Wave slopes along the world x and y (plain water).
    @location(3) slope: vec2<f32>,
    // x = fog factor (clamped), y = fog factor (unclamped above), z = view depth.
    @location(4) fog: vec3<f32>,
    // The surface frame of mesh draws (world space).
    @location(5) normal: vec3<f32>,
    @location(6) tangent: vec3<f32>,
    @location(7) binormal: vec3<f32>,
    // The mesh's vertex colour (white on landscape quads).
    @location(8) color: vec4<f32>,
};

const PI: f32 = 3.14159265358979;
const TAU: f32 = 6.28318530717959;

// s reduced to [-pi, pi) before the sine (water.md §8.3).
fn wrap_pi(s: f32) -> f32 {
    return s - TAU * floor((s + PI) / TAU);
}

struct Waves {
    h: f32,
    dx: f32,
    dy: f32,
};

// Vertex waves (§8.3): sum over k = 0, 1 of A_k sin s_k at model (x, y).
fn waves_at(xy: vec2<f32>, t: f32) -> Waves {
    var w: Waves;
    w.h = 0.0;
    w.dx = 0.0;
    w.dy = 0.0;
    if (water.waves.z != 0.0) {
        for (var k = 0; k < 2; k = k + 1) {
            let kf = f32(k);
            let a = water.waves.z * (1.0 - 0.5 * kf);
            let fx = (-1.0 + 0.3 * kf) * water.waves.y * (kf + 1.0);
            let fy = 0.7 * kf * water.waves.y * (kf + 1.0);
            let s = wrap_pi(fx * xy.x + fy * xy.y + water.waves.x * t * (0.5 + 0.8 * kf));
            w.h = w.h + a * sin(s);
            w.dx = w.dx + a * cos(s) * fx;
            w.dy = w.dy + a * cos(s) * fy;
        }
    }
    return w;
}

// Distance fog without the upper clamp (the mirror programs do not clamp it, §8.5.4).
fn fog_unclamped(world_pos: vec3<f32>) -> f32 {
    if (frame.fog_color.w == 0.0) {
        return 0.0;
    }
    let d = length(frame.eye.xyz - world_pos);
    return max((d - frame.fog.x) * frame.fog.y, 0.0);
}

fn finish_vertex(out: ptr<function, WaterOut>, world: vec3<f32>) {
    (*out).clip = frame.view_proj * vec4<f32>(world, 1.0);
    (*out).world_pos = world;
    (*out).block = (world.xy - water.screen.zw) / 160.0;
    (*out).fog = vec3<f32>(frame_fog_factor(world), fog_unclamped(world), (*out).clip.w);
}

@vertex
fn vs_water(@builtin(vertex_index) vi: u32, q: QuadIn) -> WaterOut {
    let n = max(u32(water.misc2.w), 1u);
    let cell = vi / 6u;
    let corner = vi % 6u;
    // Two counter-clockwise triangles (seen from above) per grid cell.
    var c = vec2<f32>(0.0, 0.0);
    switch (corner) {
        case 1u: { c = vec2<f32>(1.0, 0.0); }
        case 2u, 4u: { c = vec2<f32>(1.0, 1.0); }
        case 5u: { c = vec2<f32>(0.0, 1.0); }
        default: {}
    }
    let g = (vec2<f32>(f32(cell % n), f32(cell / n)) + c) / f32(n);
    let xy = q.rect.xy + g * q.rect.zw;
    let w = waves_at(xy, frame.eye.w);
    var out: WaterOut;
    finish_vertex(&out, vec3<f32>(xy, q.height.x + w.h));
    // The quad's mesh UV (water.md §8.1: the sector coordinates), read by the flow-map
    // and lava families.
    out.uv = q.uv.xy + g * q.uv.zw;
    out.slope = vec2<f32>(w.dx, w.dy);
    out.normal = vec3<f32>(0.0, 0.0, 1.0);
    out.tangent = vec3<f32>(1.0, 0.0, 0.0);
    out.binormal = vec3<f32>(0.0, 1.0, 0.0);
    out.color = vec4<f32>(1.0);
    return out;
}

@vertex
fn vs_water_mesh(v: MeshIn) -> WaterOut {
    let fam = water.kind.x;
    var p = v.position;
    var w: Waves;
    w.h = 0.0;
    w.dx = 0.0;
    w.dy = 0.0;
    // Vertex waves on the model's x and y (§8.3; §8.5.5 step 2 for the unlit liquids).
    if (fam == FAM_PLAIN || fam == FAM_LIQUID) {
        w = waves_at(p.xy, frame.eye.w);
        p.z = p.z + w.h;
    }
    let world = (water.model * vec4<f32>(p, 1.0)).xyz;
    let m3 = mat3x3<f32>(water.model[0].xyz, water.model[1].xyz, water.model[2].xyz);
    var out: WaterOut;
    finish_vertex(&out, world);
    out.uv = v.uv;
    out.slope = vec2<f32>(w.dx, w.dy);
    out.normal = frame_normalize(m3 * v.normal.xyz, vec3<f32>(0.0, 0.0, 1.0));
    out.tangent = m3 * v.tangent.xyz * select(1.0, -1.0, v.tangent.w < 0.0);
    out.binormal = m3 * v.binormal.xyz;
    out.color = v.color;
    return out;
}

fn has(bit: u32) -> bool {
    return (water.kind.y & bit) != 0u;
}

// A texel uploaded sRGB decodes when sampled: back to the stored value (render.md §5.1).
fn stored(c: vec4<f32>, bit: u32) -> vec4<f32> {
    if ((water.kind.z & bit) != 0u) {
        return vec4<f32>(frame_linear_to_srgb(c.rgb), c.a);
    }
    return c;
}

fn scene_stored(c: vec4<f32>) -> vec3<f32> {
    // The copy has the target's format: an sRGB target decodes on sampling.
    if ((frame.params.y & FRAME_SRGB_TARGET) != 0u) {
        return frame_linear_to_srgb(c.rgb);
    }
    return c.rgb;
}

// x^e for x clamped to [0, 1] and e >= 0, with 0^0 = 1 and 0^e = 0 otherwise.
fn water_pow(x: f32, e: f32) -> f32 {
    let s = clamp(x, 0.0, 1.0);
    if (e <= 0.0) {
        return 1.0;
    }
    if (s <= 0.0) {
        return 0.0;
    }
    return exp2(e * log2(s));
}

// The fake light (lighting.md E18): travel direction L, colour and specular colour; the
// main light's when there is one, else straight down (render.md §5.3 Decision), grey 0.6
// and a black specular colour.
struct FakeLight {
    travel: vec3<f32>,
    color: vec3<f32>,
    specular: vec3<f32>,
};

fn fake_light() -> FakeLight {
    var l: FakeLight;
    l.travel = vec3<f32>(0.0, 0.0, -1.0);
    l.color = vec3<f32>(0.6);
    l.specular = vec3<f32>(0.0);
    if (frame.sun_dir.w > 0.0) {
        l.travel = -frame.sun_dir.xyz;
        l.color = frame.sun_color.rgb;
        l.specular = frame.sun_specular.rgb;
    }
    return l;
}

// The normal-layer coordinates of the plain-water families (§8.3, §8.5.6).
struct Layers {
    c0: vec2<f32>,
    c1: vec2<f32>,
    c2: vec2<f32>,
};

fn plain_layers(in: WaterOut) -> Layers {
    let t = frame.eye.w;
    var l: Layers;
    let fam = water.kind.x;
    if (fam == FAM_BUMP) {
        // Older bump water (§8.5.6): U = UV x 1055, b = 1056 T.
        let u = in.uv * water.extra0.x;
        let b = water.extra0.y * t;
        l.c0 = u + vec2<f32>(b, b);
        l.c1 = 2.0 * u + vec2<f32>(4.0 * b, 4.0 * b);
        l.c2 = 4.0 * u + vec2<f32>(8.0 * b, 8.0 * b);
        return l;
    }
    var base = in.block;
    if (fam == FAM_PLAIN_UV) {
        base = in.uv;
    }
    l.c0 = base * water.speed2_scale01.z + water.speed01.xy * t;
    l.c1 = base * water.speed2_scale01.w + water.speed01.zw * t;
    l.c2 = base * water.misc0.x + water.speed2_scale01.xy * t;
    return l;
}

// The refraction of the scene drawn so far at screen position `s` offset by `off`, as
// stored values; the water fog colour without a scene copy.
fn refraction_at(s: vec2<f32>, off: vec2<f32>) -> vec3<f32> {
    let scene = textureSample(t_scene, s_clamp, s + off);
    if (water.misc2.y > 0.5) {
        return scene_stored(scene);
    }
    return water.fog_color.rgb;
}

// Plain water, plain water on mesh UV and older bump water: §8.4 steps 1-7.
fn shade_plain(in: WaterOut) -> vec3<f32> {
    let l = plain_layers(in);
    // 1. Normal: the three layers mapped to 2 (rgb - 0.5), summed, normalised.
    let s0 = stored(textureSample(t_normal, s_repeat, l.c0), T_NORMAL).rgb;
    let s1 = stored(textureSample(t_normal, s_repeat, l.c1), T_NORMAL).rgb;
    let s2 = stored(textureSample(t_normal, s_repeat, l.c2), T_NORMAL).rgb;
    let layers = water.waves.w;
    var sum = 2.0 * (s0 - 0.5);
    if (layers >= 2.0) {
        sum = sum + 2.0 * (s1 - 0.5);
    }
    if (layers >= 3.0) {
        sum = sum + 2.0 * (s2 - 0.5);
    }
    var nt = vec3<f32>(0.0, 0.0, 1.0);
    if (has(T_NORMAL)) {
        nt = frame_normalize(sum, vec3<f32>(0.0, 0.0, 1.0));
    }
    // Surface frame of the (waved) plane: T along +x, B along +y, N up; the texel is
    // turned into it with the sign convention of lighting.md §5.2.4 (N = n.x T - n.y B +
    // n.z N0; see docs/specs/render.md §5.3).
    let tang = frame_normalize(vec3<f32>(1.0, 0.0, in.slope.x), vec3<f32>(1.0, 0.0, 0.0));
    let bit = frame_normalize(vec3<f32>(0.0, 1.0, in.slope.y), vec3<f32>(0.0, 1.0, 0.0));
    let n0 = frame_normalize(cross(tang, bit), vec3<f32>(0.0, 0.0, 1.0));
    let n = frame_normalize(nt.x * tang - nt.y * bit + nt.z * n0, n0);

    // 2. Screen coordinates (the fragment's position is in framebuffer pixels).
    let s = in.clip.xy * water.screen.xy;
    // 3. Refraction tinted toward the under-water fog colour, blended toward the shoreline
    // tint near the camera.
    let f = refraction_at(s, n.xy * water.misc0.z);
    let u = f + water.misc2.x * (water.fog_color.rgb - f);
    let to_eye = frame.eye.xyz - in.world_pos;
    let dist = length(to_eye);
    let w = clamp((dist - 5.0) * 0.1, 0.0, 1.0);
    let shore = f * water.shoreline_color.rgb;
    let refraction = shore + w * (u - shore);
    // 4. Reflection: the unmodified refraction sample (Tessera rule).
    let reflection = f;
    // 5. Fresnel.
    let v = frame_normalize(to_eye, vec3<f32>(0.0, 0.0, 1.0));
    let fr = water.misc0.w + (1.0 - water.misc0.w)
        * water_pow(1.0 - abs(dot(v, n)), water.misc1.x);
    var color = refraction + fr * (reflection - refraction);
    // 6. Specular with the sparkle of the tangent-space normal.
    let light = fake_light();
    let h = frame_normalize(v - light.travel, n);
    var spec = water_pow(dot(n, h), water.misc1.y);
    if (has(T_NORMAL)) {
        spec = spec + 4.0
            * (clamp(nt.x - water.misc1.w, 0.0, 1.0) + clamp(nt.y - water.misc1.w, 0.0, 1.0));
    }
    // 7. Output: water colour and overbright, specular, then distance fog.
    color = color * water.water_color.rgb * water.misc1.z
        + spec * light.specular * water.specular_color.rgb;
    return frame_apply_fog(color, in.fog.x);
}

// The two flow phases of §8.6.2 steps 1-4 (shared with lava, §8.6.5).
struct Phases {
    flow: vec2<f32>,
    c1: vec2<f32>,
    c2: vec2<f32>,
    // Blend weight toward the second phase.
    w: f32,
    // k = 2 |F|^2.
    k: f32,
};

fn rotate(c: vec2<f32>, theta: f32) -> vec2<f32> {
    let co = cos(theta);
    let si = sin(theta);
    return vec2<f32>(c.x * co + c.y * si, -c.x * si + c.y * co);
}

fn flow_phases(in: WaterOut, flow_texel: vec4<f32>, noise_texel: vec4<f32>) -> Phases {
    let t = frame.eye.w;
    var p: Phases;
    // 1. Flow vector F = 2 x (the flow map's first two channels at UV) - 1.
    p.flow = vec2<f32>(0.0);
    if (has(T_FLOW)) {
        p.flow = 2.0 * flow_texel.rg - 1.0;
    }
    // 2. Phase offset q = 1/2 x the noise's first channel at B / 2.
    var q = 0.0;
    if (has(T_NOISE)) {
        q = 0.5 * noise_texel.r;
    }
    // 3. Two phases.
    let p1 = q + water.speed01.x * t;
    let p2 = q + water.speed01.y * t + 0.5;
    let f1 = fract(p1);
    let f2 = fract(p2);
    let i1 = p1 - f1;
    let i2 = p2 - f2;
    // 4. Layer-0 coordinates of each phase, turned by 1188.
    let b = in.block * water.speed2_scale01.z;
    p.c1 = rotate(b + 0.7 * vec2<f32>(i1, i1) + p.flow * f1, water.flow1.z);
    p.c2 = rotate(b + 0.7 * vec2<f32>(i2, i2) + p.flow * f2, water.flow1.z);
    // 5. Blend weight w = 2 |1/2 - f1| toward the second sample.
    p.w = 2.0 * abs(0.5 - f1);
    p.k = 2.0 * dot(p.flow, p.flow);
    return p;
}

// Flow-map water: the normal of §8.6.2 (layers 1 and 2 when the record has them) and the
// colour of the record's own OpenGL reflection-permutation list with the reflection R
// replaced by the untinted refraction sample, alpha 1 (Tessera rule of §8.6.4, revised
// 2026-10-10): 0x2B0007FD steps 1-6 of the first list (FL_FLOW_FAKE_LIGHT), 0x2B0005EC
// steps 1-5 of its own list.
fn shade_flow(in: WaterOut) -> vec3<f32> {
    let t = frame.eye.w;
    let flow_texel = stored(textureSample(t_flow, s_clamp, in.uv), T_FLOW);
    let noise_texel = stored(textureSample(t_noise, s_repeat, 0.5 * in.block), T_NOISE);
    let p = flow_phases(in, flow_texel, noise_texel);
    let n1 = stored(textureSample(t_normal, s_repeat, p.c1), T_NORMAL).rgb;
    let n2 = stored(textureSample(t_normal, s_repeat, p.c2), T_NORMAL).rgb;
    // 7. Layers 1 and 2 with g = clamp(0.0004 x view depth, 1, 10).
    let g = clamp(0.0004 * in.fog.z, 1.0, 10.0);
    let l1 = in.block * water.speed2_scale01.w + g * water.speed01.zw * t;
    let l2 = in.block * water.misc0.x + g * water.speed2_scale01.xy * t;
    let t1 = stored(textureSample(t_normal, s_repeat, l1), T_NORMAL).rgb;
    let t2 = stored(textureSample(t_normal, s_repeat, l2), T_NORMAL).rgb;
    let third = stored(textureSample(t_flow, s_clamp, in.uv + water.flow1.xy), T_FLOW).b;
    let foam = textureSample(t_foam, s_repeat, (in.uv + water.flow0.xy) * water.flow0.zw);
    // 5.-6. The flow normal from the mixed texel.
    var n = vec3<f32>(0.0, 0.0, 1.0);
    if (has(T_NORMAL)) {
        let m = mix(n1, n2, p.w);
        n = vec3<f32>(p.k * (2.0 * m.r - 1.0), p.k * (2.0 * m.g - 1.0),
            1.0 + p.k * (2.0 * m.b - 2.0));
        if (water.waves.w >= 3.0) {
            var phi = 0.0;
            if (has(T_FLOW) && has(T_FOAM)) {
                phi = third * foam.a;
            }
            let weight = (1.0 - length(p.flow)) * (1.0 - phi);
            n = n + ((2.0 * t1 - 1.0) + (2.0 * t2 - 1.0)) * weight;
        }
    }
    // 8. Normalise.
    n = frame_normalize(n, vec3<f32>(0.0, 0.0, 1.0));

    // Colour (§8.6.4, the record's own OpenGL list).
    let layered = (water.kind.w & FL_FLOW_FAKE_LIGHT) != 0u;
    let s = in.clip.xy * water.screen.xy;
    // 1. Refraction offset by the normal x 1048; the reflection (0x2B0007FD undistorted,
    // 0x2B0005EC at N.xy x 1047 + (0, 0.003)) is replaced by the untinted refraction sample
    // (Tessera rule), so its offset has no effect.
    let fr = refraction_at(s, n.xy * water.misc0.z);
    let r = fr;
    // 2. 0x2B0007FD: Lc = clamp(Dl + (1066 - 1)(1 - Dl), 0, 1). 0x2B0005EC never reads the
    // fake light's diffuse colour: no Lc (W42), i.e. Lc = 1 below.
    let light = fake_light();
    var lc = vec3<f32>(1.0);
    if (layered) {
        lc = clamp(light.color + (water.misc1.z - 1.0) * (1.0 - light.color), vec3<f32>(0.0),
            vec3<f32>(1.0));
    }
    // 3. Tint by w = clamp(0.1 x eye distance - 0.5, 0, 1) x 1068 toward Lc^2 x WaterFogColor
    // (0x2B0007FD) or toward WaterFogColor itself (0x2B0005EC).
    let to_eye = frame.eye.xyz - in.world_pos;
    let dist = length(to_eye);
    let wt = clamp(0.1 * dist - 0.5, 0.0, 1.0) * water.misc2.x;
    let tinted = fr + wt * (lc * lc * water.fog_color.rgb - fr);
    // 4. Fresnel: 0x2B0007FD f = 1050 x max(1 - |v . N|, 0)^(1 / 1051); 0x2B0005EC
    // f = 1050 + (1 - 1050) max(1 - |v . N|, 0)^1051 (§8.4 step 5; W42, W43).
    let v = frame_normalize(to_eye, vec3<f32>(0.0, 0.0, 1.0));
    let x = max(1.0 - abs(dot(v, n)), 0.0);
    var f = 0.0;
    if (layered) {
        var inv = 0.0;
        if (water.misc1.x > 0.0) {
            inv = 1.0 / water.misc1.x;
        }
        f = water.misc0.w * water_pow(x, inv);
    } else {
        f = water.misc0.w + (1.0 - water.misc0.w) * water_pow(x, water.misc1.x);
    }
    // 5. Colour and specular (x Lc only for 0x2B0007FD; no sparkle term).
    let h = frame_normalize(v - light.travel, n);
    var color = mix(tinted, r, f) * water.water_color.rgb * water.misc1.z
        + lc * water_pow(dot(n, h), water.misc1.y) * light.specular * water.specular_color.rgb;
    // 6. Fog starting 100 m later.
    var fog = 0.0;
    if (frame.fog_color.w != 0.0) {
        fog = clamp((dist - frame.fog.x - 100.0) * frame.fog.y, 0.0, 1.0);
    }
    color = frame_apply_fog(color, fog);
    return color;
}

// Lava (§8.6.5 steps 1-7, the Direct3D 10/11 form): no light, opaque.
fn shade_lava(in: WaterOut) -> vec3<f32> {
    let flow_texel = stored(textureSample(t_flow, s_clamp, in.uv), T_FLOW);
    let noise_texel = stored(textureSample(t_noise, s_repeat, 0.5 * in.block), T_NOISE);
    let p = flow_phases(in, flow_texel, noise_texel);
    // 2. Flow colour: layer 1 at c1 mixed with layer 2 at c2.
    let a = stored(textureSample(t_color0, s_color0, p.c1), T_COLOR0).rgb;
    let b = stored(textureSample(t_color1, s_color1, p.c2), T_COLOR1).rgb;
    let c = mix(a, b, p.w);
    // 3. Emissive texel at UV + 1063 B (no time term).
    var e = vec4<f32>(0.0);
    let et = stored(textureSample(t_emissive, s_emissive,
        in.uv + water.speed2_scale01.z * in.block), T_EMISSIVE);
    if (has(T_EMISSIVE)) {
        e = et;
    }
    // 4. Emissive weight and base colour.
    let wa = clamp(1.0 + e.a - p.k, 0.0, 1.0);
    let x = c + wa * (e.rgb - c);
    // 5. Brightness.
    let m = 2.0 * water.misc1.z * (1.8 - wa - 0.5 * e.a);
    let y = m * x;
    // 6. Foam.
    let third = stored(textureSample(t_flow, s_clamp, in.uv + water.flow1.xy), T_FLOW).b;
    let foam = stored(textureSample(t_foam, s_repeat, (in.uv + water.flow0.xy) * water.flow0.zw),
        T_FOAM);
    var phi = 0.0;
    if (has(T_FLOW) && has(T_FOAM)) {
        phi = clamp(foam.a * third, 0.0, 1.0);
    }
    let z = y + phi * (foam.rgb - y);
    // 7. Fog, clamped to 0-1.
    return clamp(z + in.fog.x * (frame.fog_color.rgb - z), vec3<f32>(0.0), vec3<f32>(1.0));
}

// The tangent frame of a mesh draw (tangent, binormal, normal), with fallbacks for meshes
// without tangents (an axis perpendicular to the normal).
struct Frame3 {
    t: vec3<f32>,
    b: vec3<f32>,
    n: vec3<f32>,
};

fn mesh_frame(in: WaterOut) -> Frame3 {
    var f: Frame3;
    f.n = frame_normalize(in.normal, vec3<f32>(0.0, 0.0, 1.0));
    var t = in.tangent - dot(in.tangent, f.n) * f.n;
    if (dot(t, t) <= 1e-12) {
        let axis = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0),
            abs(f.n.x) > 0.9);
        t = axis - dot(axis, f.n) * f.n;
    }
    f.t = frame_normalize(t, vec3<f32>(1.0, 0.0, 0.0));
    var b = in.binormal;
    if (dot(b, b) <= 1e-12) {
        b = cross(f.n, f.t);
    }
    f.b = frame_normalize(b, cross(f.n, f.t));
    return f;
}

// The mirror record (§8.5.4) with the Tessera rule: the reflection is the distance-fog
// colour times the material colour until a reflection pass exists.
fn shade_mirror(in: WaterOut) -> vec4<f32> {
    let t = frame.eye.w;
    // Normal-map coordinates (UV + S T) x 1103; the diffuse at UV unchanged.
    let nc = (in.uv + water.extra1.xy * t) * water.extra0.w;
    var nm = vec3<f32>(0.5, 0.5, 1.0);
    let ns = stored(textureSample(t_normal, s_repeat, nc), T_NORMAL).rgb;
    if (has(T_NORMAL)) {
        nm = ns;
    }
    var d = vec4<f32>(1.0);
    let ds = stored(textureSample(t_color0, s_color0, in.uv), T_COLOR0);
    if (has(T_COLOR0)) {
        d = ds;
    }
    // 2. Reflection (Tessera rule) times the material colour's RGB.
    let r = frame.fog_color.rgb * water.material.rgb;
    // 4. Fresnel with the tangent-space view vector.
    let fr = mesh_frame(in);
    let v_world = frame_normalize(frame.eye.xyz - in.world_pos, fr.n);
    let v = vec3<f32>(dot(v_world, fr.t), dot(v_world, fr.b), dot(v_world, fr.n));
    let m = vec3<f32>(0.0, 0.0, 1.0)
        + water.extra1.z * vec3<f32>(2.0 * nm.x - 1.0, 2.0 * nm.y - 1.0, 2.0 * nm.z - 2.0);
    let f = water_pow(1.0 - abs(dot(v, m)), water.extra0.y);
    let a = water.extra0.z + (1.0 - water.extra0.z) * f;
    // 5. Colour D x R toward the fog colour by the unclamped factor; alpha a x D.a x M.a.
    let c = d.rgb * r;
    let color = c + in.fog.y * (frame.fog_color.rgb - c);
    return vec4<f32>(color, a * d.a * water.material.a);
}

fn light_index(i: u32) -> u32 {
    let word = select(water.lights.x, water.lights.y, i >= 4u);
    return (word >> ((i & 3u) * 8u)) & 255u;
}

// Liquid-distortion records (§8.5.5).
fn shade_liquid(in: WaterOut) -> vec4<f32> {
    let t = frame.eye.w;
    // 1. Coordinates: UV through texture matrix 0, or B.
    var base = vec2<f32>(dot(water.uv_row0.xyz, vec3<f32>(in.uv, 1.0)),
        dot(water.uv_row1.xyz, vec3<f32>(in.uv, 1.0)));
    if ((water.kind.w & FL_WORLD_UV) != 0u) {
        base = in.block;
    }
    let c0 = base * water.speed2_scale01.z + water.speed01.xy * t;
    let c1 = base * water.speed2_scale01.w + water.speed01.zw * t;
    let c2 = base * water.misc0.x + water.speed2_scale01.xy * t;
    // 3. Distortion: the three layer samples mapped to 2 (rgb - 0.5), normalised, x, y x 1048.
    let s0 = stored(textureSample(t_normal, s_repeat, c0), T_NORMAL).rgb;
    let s1 = stored(textureSample(t_normal, s_repeat, c1), T_NORMAL).rgb;
    let s2 = stored(textureSample(t_normal, s_repeat, c2), T_NORMAL).rgb;
    var off = vec2<f32>(0.0);
    if (has(T_NORMAL)) {
        let sum = 2.0 * (s0 - 0.5) + 2.0 * (s1 - 0.5) + 2.0 * (s2 - 0.5);
        off = frame_normalize(sum, vec3<f32>(0.0, 0.0, 1.0)).xy * water.misc0.z;
    }
    // 4. The colour texture at layer 2's coordinates plus the offset.
    var texel = vec4<f32>(1.0);
    let ts = stored(textureSample(t_color0, s_color0, c2 + off), T_COLOR0);
    if (has(T_COLOR0)) {
        texel = ts;
    }
    let m = water.material;
    if (water.kind.x == FAM_LIQUID_LIT) {
        // The lit record tests its opacity texel before any vertex multiply (lighting.md
        // §5.12, §5.13).
        if ((water.kind.w & FL_ALPHA_TEST) != 0u && texel.a <= water.extra1.x) {
            discard;
        }
        let alpha = texel.a * m.a;
        // Lit like a lit ColorAndOpacity surface (lighting.md §5): ambient, the sun with its
        // shadow, the listed lights; no specular.
        let n = frame_normalize(in.normal, vec3<f32>(0.0, 0.0, 1.0));
        var sum = frame_ambient(n, m.rgb);
        let v = frame_normalize(frame.eye.xyz - in.world_pos, n);
        let shadow = frame_shadow_factor(frame_shadow_coords(in.world_pos), in.fog.z);
        sum = sum + frame_sun(n, v, 1.0, -1.0, shadow).diffuse * m.rgb;
        let count = min(water.lights.z, 8u);
        for (var i = 0u; i < count; i = i + 1u) {
            let idx = light_index(i);
            if (idx < frame.params.x) {
                sum = sum + frame_light(idx, in.world_pos, n, v, 1.0, -1.0).diffuse * m.rgb;
            }
        }
        return vec4<f32>(frame_apply_fog(2.0 * texel.rgb * sum, in.fog.x), alpha);
    }
    // 5. Unlit: the texel (RGBA) first multiplied by the vertex colour (0x2B00063E), its
    // alpha then by the material alpha; the alpha test reads that output alpha (discard at
    // or below the reference, W45); colour x material colour x 1066, fogged.
    var vc = vec4<f32>(1.0);
    if ((water.kind.w & FL_VERTEX_COLOR) != 0u) {
        vc = in.color;
    }
    let c = texel * vc;
    let alpha = c.a * m.a;
    if ((water.kind.w & FL_ALPHA_TEST) != 0u && alpha <= water.extra1.x) {
        discard;
    }
    return vec4<f32>(frame_apply_fog(c.rgb * m.rgb * water.misc1.z, in.fog.x), alpha);
}

// Opaque families (plain, plain on UV, older bump, flow map, lava): alpha 1, written with
// source ONE and destination ZERO, with the far-draw fade of landscape quads.
@fragment
fn fs_water(in: WaterOut) -> @location(0) vec4<f32> {
    var color = vec3<f32>(0.0);
    switch (water.kind.x) {
        case 3u: { color = shade_flow(in); }
        case 4u: { color = shade_lava(in); }
        default: { color = shade_plain(in); }
    }
    // Far-draw fade (Tessera, far-view.md §8 OPEN): a = 1 - sat((d - s) r), blended toward
    // the undistorted scene copy; without a copy only a hard cut at a = 0.5.
    let behind = textureSample(t_scene, s_clamp, in.clip.xy * water.screen.xy);
    let dist = length(frame.eye.xyz - in.world_pos);
    let a = 1.0 - clamp((dist - water.fade.x) * water.fade.y, 0.0, 1.0);
    if (water.misc2.y > 0.5) {
        if (a < 0.001) {
            discard;
        }
        color = scene_stored(behind) + a * (color - scene_stored(behind));
    } else if (a < 0.5) {
        discard;
    }
    return vec4<f32>(frame_output(color), 1.0);
}

// Blended families (mirror, liquid distortion): source alpha / inverse source alpha.
@fragment
fn fs_water_blend(in: WaterOut) -> @location(0) vec4<f32> {
    var c = vec4<f32>(0.0);
    switch (water.kind.x) {
        case 5u: { c = shade_mirror(in); }
        default: { c = shade_liquid(in); }
    }
    return vec4<f32>(frame_output(c.rgb), clamp(c.a, 0.0, 1.0));
}
