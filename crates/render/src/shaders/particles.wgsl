// SPDX-License-Identifier: GPL-3.0-or-later
// Particle draws (`docs/specs/particle-runtime.md` §9.1): world-space vertices with a
// particle colour and atlas texture coordinates, the texture of the system's surface, and a
// colour multiplier (1, 1, 1, opacity) per draw. Colours are in stored-value space: a texture
// uploaded sRGB is re-encoded after sampling, and an sRGB target is written the decoded
// value so that it stores the result unchanged (`docs/specs/render.md` §4.1).

struct ParticleDraw {
    view_proj: mat4x4<f32>,
    tint: vec4<f32>,
    // x: alpha-test cutoff (fragments with alpha <= cutoff are discarded when flags.z = 1).
    params: vec4<f32>,
    // x: texture is sRGB, y: target is sRGB, z: alpha test.
    flags: vec4<u32>,
};

@group(0) @binding(0) var<uniform> draw: ParticleDraw;
@group(1) @binding(0) var t_particle: texture_2d<f32>;
@group(1) @binding(1) var s_particle: sampler;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
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
    return out;
}

@fragment
fn fs_particle(in: VsOut) -> @location(0) vec4<f32> {
    var tex = textureSample(t_particle, s_particle, in.uv);
    if (draw.flags.x != 0u) {
        tex = vec4<f32>(particle_linear_to_srgb(tex.rgb), tex.a);
    }
    let c = tex * in.color * draw.tint;
    if (draw.flags.z != 0u && c.a <= draw.params.x) {
        discard;
    }
    var rgb = clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    if (draw.flags.y != 0u) {
        rgb = particle_srgb_to_linear(rgb);
    }
    return vec4<f32>(rgb, clamp(c.a, 0.0, 1.0));
}
