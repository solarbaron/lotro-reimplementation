// SPDX-License-Identifier: GPL-3.0-or-later
// Instanced textured quad. The unit quad spans -0.5..0.5 in local X/Y and is
// generated from the vertex index (triangle strip, 4 vertices).

struct Globals {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var t_color: texture_2d<f32>;
@group(1) @binding(1) var s_color: sampler;

struct InstanceIn {
    @location(0) m0: vec4<f32>,
    @location(1) m1: vec4<f32>,
    @location(2) m2: vec4<f32>,
    @location(3) m3: vec4<f32>,
    @location(4) color: vec4<f32>,
    // xy = uv offset, zw = uv scale
    @location(5) uv_rect: vec4<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: InstanceIn) -> VsOut {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let local = vec4<f32>(corner - vec2<f32>(0.5, 0.5), 0.0, 1.0);
    let model = mat4x4<f32>(inst.m0, inst.m1, inst.m2, inst.m3);
    var out: VsOut;
    out.position = globals.view_proj * (model * local);
    // v = 0 at the top edge (local +Y) of the quad.
    out.uv = inst.uv_rect.xy + vec2<f32>(corner.x, 1.0 - corner.y) * inst.uv_rect.zw;
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(t_color, s_color, in.uv) * in.color;
}
