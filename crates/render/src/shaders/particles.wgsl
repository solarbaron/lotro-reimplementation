// SPDX-License-Identifier: GPL-3.0-or-later
// Particle draws (`docs/specs/particle-runtime.md` §9.1, §9.1.1): world-space vertices with
// a particle colour and atlas texture coordinates, the texture(s) of the system's surface,
// and per draw M = material colour 2000 x the draw's tint (1, 1, 1, opacity). The programs
// of §9.1.1: the alpha test reads the texture's opacity T.a alone (two layers: the product
// of both alphas); colour = T x V x M (doubled for some records, then fogged for the
// alpha-blended ones); output alpha = T.a x V.a x M.a, with the texture alpha, the vertex
// colour or M left out where the program says so. Colours are in stored-value space: a
// texture uploaded sRGB is re-encoded after sampling, and an sRGB target is written the
// decoded value so that it stores the result unchanged (`docs/specs/render.md` §4.1).

struct ParticleDraw {
    view_proj: mat4x4<f32>,
    // M: material colour 2000 x the draw's tint (white when the program reads no M).
    material: vec4<f32>,
    // x: alpha-test reference (fragments whose tested opacity is <= x are discarded when
    // flags.z = 1), y: fog near, z: 1 / (fog far - fog near).
    params: vec4<f32>,
    // xyz: eye position (world).
    eye: vec4<f32>,
    // rgb: fog colour, w: 1 when this draw is fogged.
    fog_color: vec4<f32>,
    // x: texture is sRGB, y: target is sRGB, z: alpha test, w: program bits
    // (1 texture alpha, 2 vertex colour, 4 doubled).
    flags: vec4<u32>,
    // x: second-layer texture is sRGB, y: layers (0 one, 1 product, 2 sum).
    layers: vec4<u32>,
    // Texture matrices 0 and 1 (`material.md` §8.4) packed as (a0, b0, c0, d0),
    // (e0, f0, a1, b1), (c1, d1, e1, f1) for rows ((a, b, c), (d, e, f)); the identity for
    // one-layer programs.
    uv0: vec4<f32>,
    uv1: vec4<f32>,
    uv2: vec4<f32>,
};

const PROGRAM_TEXTURE_ALPHA: u32 = 1u;
const PROGRAM_VERTEX_COLOUR: u32 = 2u;
const PROGRAM_DOUBLED: u32 = 4u;

// Set for a fixed blend that draws opaque in a premultiplied pass (`render.md` §6.10, R12):
// the fragment writes alpha 1 and its colour times the source factor (1 = ZERO, 2 = ONE,
// 3 = SRCALPHA, 4 = INVSRCALPHA); 0 = the colour and alpha computed.
override FIXED_OPAQUE: u32 = 0u;

@group(0) @binding(0) var<uniform> draw: ParticleDraw;
@group(1) @binding(0) var t_particle: texture_2d<f32>;
@group(1) @binding(1) var s_particle: sampler;
@group(1) @binding(2) var t_layer2: texture_2d<f32>;
@group(1) @binding(3) var s_layer2: sampler;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    // Distance fog factor f (`lighting.md` §10, per vertex).
    @location(2) fog: f32,
};

fn particle_srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let x = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(pow((x + 0.055) / 1.055, vec3<f32>(2.4)), x / 12.92, x <= vec3<f32>(0.04045));
}

fn particle_linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let x = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(1.055 * pow(x, vec3<f32>(1.0 / 2.4)) - 0.055, x * 12.92,
        x <= vec3<f32>(0.0031308));
}

@vertex
fn vs_particle(v: VsIn) -> VsOut {
    var out: VsOut;
    out.position = draw.view_proj * vec4<f32>(v.position, 1.0);
    out.color = v.color;
    out.uv = v.uv;
    var f = 0.0;
    if (draw.fog_color.w != 0.0) {
        let d = length(draw.eye.xyz - v.position);
        f = clamp((d - draw.params.y) * draw.params.z, 0.0, 1.0);
    }
    out.fog = f;
    return out;
}

fn particle_uv_matrix0(uv: vec2<f32>) -> vec2<f32> {
    let m = draw.uv0;
    return vec2<f32>(m.x * uv.x + m.y * uv.y + m.z, m.w * uv.x + draw.uv1.x * uv.y + draw.uv1.y);
}

fn particle_uv_matrix1(uv: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(draw.uv1.z * uv.x + draw.uv1.w * uv.y + draw.uv2.x,
        draw.uv2.y * uv.x + draw.uv2.z * uv.y + draw.uv2.w);
}

@fragment
fn fs_particle(in: VsOut) -> @location(0) vec4<f32> {
    // Two-layer programs read layer 1 at texture matrix 0 and layer 2 at texture matrix 1,
    // both applied to the vertex UV pair (`particle-runtime.md` §9.1.1, RT27); one-layer
    // programs get identity matrices.
    var tex = textureSample(t_particle, s_particle, particle_uv_matrix0(in.uv));
    if (draw.flags.x != 0u) {
        tex = vec4<f32>(particle_linear_to_srgb(tex.rgb), tex.a);
    }
    var l2 = textureSample(t_layer2, s_layer2, particle_uv_matrix1(in.uv));
    if (draw.layers.x != 0u) {
        l2 = vec4<f32>(particle_linear_to_srgb(l2.rgb), l2.a);
    }
    if (draw.layers.y == 1u) {
        tex = vec4<f32>(tex.rgb * l2.rgb, tex.a * l2.a);
    } else if (draw.layers.y == 2u) {
        tex = vec4<f32>(tex.rgb + l2.rgb, tex.a * l2.a);
    }
    // The alpha test reads the texture's opacity alone (§9.1.1 rule 1).
    if (draw.flags.z != 0u && tex.a <= draw.params.x) {
        discard;
    }
    let program = draw.flags.w;
    var v = vec4<f32>(1.0);
    if ((program & PROGRAM_VERTEX_COLOUR) != 0u) {
        v = in.color;
    }
    var ta = 1.0;
    if ((program & PROGRAM_TEXTURE_ALPHA) != 0u) {
        ta = tex.a;
    }
    var c = tex.rgb * v.rgb * draw.material.rgb;
    if ((program & PROGRAM_DOUBLED) != 0u) {
        c = 2.0 * c;
    }
    if (draw.fog_color.w != 0.0) {
        c = mix(c, draw.fog_color.rgb, in.fog);
    }
    var rgb = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    if (draw.flags.y != 0u) {
        rgb = particle_srgb_to_linear(rgb);
    }
    let a = clamp(ta * v.a * draw.material.a, 0.0, 1.0);
    switch FIXED_OPAQUE {
        case 1u: {
            return vec4<f32>(vec3<f32>(0.0), 1.0);
        }
        case 2u: {
            return vec4<f32>(rgb, 1.0);
        }
        case 3u: {
            return vec4<f32>(rgb * a, 1.0);
        }
        case 4u: {
            return vec4<f32>(rgb * (1.0 - a), 1.0);
        }
        default: {
            return vec4<f32>(rgb, a);
        }
    }
}
