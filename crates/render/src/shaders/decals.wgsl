// SPDX-License-Identifier: GPL-3.0-or-later
// Placed decal draws (`docs/specs/decals.md` §6): world-space triangles with the per-decal
// colour V of §6.2 and the texture coordinates of §4.5, the surface texture T and per batch
// M = material colour 2000. Colour = T x V x M, alpha = T.a x V.a x M.a (V left out when
// the surface's program has no colour input, §6.1), then fogged when the batch is fogged.
// The alpha test reads the texture's alpha against the per-draw reference. Colours are in
// stored-value space: a texture uploaded sRGB is re-encoded after sampling, and an sRGB
// target is written the decoded value so that it stores the result unchanged
// (`docs/specs/render.md` §4.1).

// The alpha-test slack of frame.wgsl (this shader does not prepend it): see the comment there.
const ALPHA_TEST_SLACK: f32 = 9.5367431640625e-7;

struct DecalDraw {
    view_proj: mat4x4<f32>,
    // M: material colour 2000.
    material: vec4<f32>,
    // x: alpha-test reference (fragments whose texture alpha is <= x are discarded when
    // flags.z = 1), y: fog near, z: 1 / (fog far - fog near).
    params: vec4<f32>,
    // xyz: eye position (world).
    eye: vec4<f32>,
    // rgb: fog colour, w: 1 when this batch is fogged.
    fog_color: vec4<f32>,
    // x: texture is sRGB, y: target is sRGB, z: alpha test, w: vertex colour is read.
    flags: vec4<u32>,
};

// Set for a fixed blend that draws opaque in a premultiplied pass (`render.md` §6.10, R12):
// the fragment writes alpha 1 and its colour times the source factor (1 = ZERO, 2 = ONE,
// 3 = SRCALPHA, 4 = INVSRCALPHA); 0 = the colour and alpha computed.
override FIXED_OPAQUE: u32 = 0u;

@group(0) @binding(0) var<uniform> draw: DecalDraw;
@group(1) @binding(0) var t_decal: texture_2d<f32>;
@group(1) @binding(1) var s_decal: sampler;

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

fn decal_srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let x = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(pow((x + 0.055) / 1.055, vec3<f32>(2.4)), x / 12.92, x <= vec3<f32>(0.04045));
}

fn decal_linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let x = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(1.055 * pow(x, vec3<f32>(1.0 / 2.4)) - 0.055, x * 12.92,
        x <= vec3<f32>(0.0031308));
}

@vertex
fn vs_decal(v: VsIn) -> VsOut {
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

@fragment
fn fs_decal(in: VsOut) -> @location(0) vec4<f32> {
    var tex = textureSample(t_decal, s_decal, in.uv);
    if (draw.flags.x != 0u) {
        tex = vec4<f32>(decal_linear_to_srgb(tex.rgb), tex.a);
    }
    if (draw.flags.z != 0u && tex.a <= draw.params.x + ALPHA_TEST_SLACK) {
        discard;
    }
    var v = vec4<f32>(1.0);
    if (draw.flags.w != 0u) {
        v = in.color;
    }
    var c = tex.rgb * v.rgb * draw.material.rgb;
    if (draw.fog_color.w != 0.0) {
        c = mix(c, draw.fog_color.rgb, in.fog);
    }
    var rgb = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    if (draw.flags.y != 0u) {
        rgb = decal_srgb_to_linear(rgb);
    }
    let a = clamp(tex.a * v.a * draw.material.a, 0.0, 1.0);
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
