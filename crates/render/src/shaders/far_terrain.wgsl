// SPDX-License-Identifier: GPL-3.0-or-later
// Far terrain from distant-landscape sectors (docs/specs/far-view.md §6.3, §6.4). Written
// for Tessera from the spec's prose. Appended after frame.wgsl and frame_shading.wgsl;
// `FarDraw` matches `FarDrawUniform` in crates/render/src/far_view.rs.

struct FarDraw {
    model: mat4x4<f32>,
    // x = fade start s, y = inverse range r, z = near-view origin x, w = near-view origin y.
    fade: vec4<f32>,
    // x, y = detail (and normal) repeats per sector, z = flags (FT_*), w = sRGB bits (FS_*).
    params: vec4<f32>,
    // x, y = colour-list repeats per sector (transform set 0 of the normal-mapped template).
    colour: vec4<f32>,
};

const FT_NORMAL_MAP: u32 = 1u;
const FT_NEAR_VIEW: u32 = 2u;
const FS_COLOUR: u32 = 1u;
const FS_DETAIL: u32 = 2u;
const FS_NORMAL: u32 = 4u;

@group(1) @binding(0) var<uniform> fd: FarDraw;
@group(1) @binding(1) var t_colour: texture_2d<f32>;
@group(1) @binding(2) var t_detail: texture_2d<f32>;
@group(1) @binding(3) var t_normal: texture_2d<f32>;
@group(1) @binding(4) var s_clamp: sampler;
@group(1) @binding(5) var s_wrap: sampler;

struct FarIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct FarOut {
    // The seam overlay (far-view.md §6.5) draws the near terrain's own triangles again:
    // its depth must equal the layer pipeline's (terrain_layers.wgsl), so both compute the
    // position the same way and mark it invariant (without the mark the overlay z-fights;
    // crates/render/tests/far_view_golden.rs checks it).
    @builtin(position) @invariant clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // Per-vertex light (plain template and the per-vertex passes).
    @location(1) light: vec3<f32>,
    // x = fog f, y = far-draw fade a, z = coverage w, w = normal strength t.
    @location(2) terms: vec4<f32>,
    @location(3) normal: vec3<f32>,
};

fn far_flag(bit: u32) -> bool {
    return (u32(fd.params.z) & bit) != 0u;
}

fn far_stored(c: vec4<f32>, bit: u32) -> vec4<f32> {
    if ((u32(fd.params.w) & bit) == 0u) {
        return c;
    }
    return vec4<f32>(frame_linear_to_srgb(c.rgb), c.a);
}

// Per-vertex light (§6.3 step 1): sat(A + n_up H) + k sat(n . L) C_D, the normal not
// renormalised (the stored sample normal).
fn far_vertex_light(n: vec3<f32>) -> vec3<f32> {
    let ambient = clamp(frame.ambient.rgb + dot(n, frame.up.xyz) * frame.hemispheric.rgb,
        vec3<f32>(0.0), vec3<f32>(1.0));
    let k = frame.sun_dir.w;
    return ambient + k * clamp(dot(n, frame.sun_dir.xyz), 0.0, 1.0) * frame.sun_color.rgb;
}

@vertex
fn vs_far(v: FarIn) -> FarOut {
    var out: FarOut;
    let world = fd.model * vec4<f32>(v.position, 1.0);
    let n = (fd.model * vec4<f32>(v.normal, 0.0)).xyz;
    out.clip = frame.view_proj * world;
    out.uv = v.uv;
    out.light = far_vertex_light(n);
    out.normal = n;
    let d = length(frame.eye.xyz - world.xyz);
    let fog = frame_fog_factor(world.xyz);
    let fade = 1.0 - clamp((d - fd.fade.x) * fd.fade.y, 0.0, 1.0);
    var w = 1.0;
    var t = 0.0;
    if (far_flag(FT_NORMAL_MAP)) {
        // §6.4: horizontal distance h, normal strength t, near-view mask m, coverage w.
        let h = length(frame.eye.xy - world.xy);
        t = clamp((h - 80.0) / 160.0, 0.0, 1.0);
        var m = 1.0;
        if (far_flag(FT_NEAR_VIEW)) {
            let p = world.x - fd.fade.z;
            let q = world.y - fd.fade.w;
            m = clamp(max(max(-p - 159.0, -q - 159.0), max(p - 319.0, q - 319.0)), 0.0, 1.0);
        }
        let n_up = dot(n, frame.up.xyz);
        w = min(1.0, m + clamp((h - 80.0) / 80.0, 0.0, 1.0) * min(1.0, n_up * n_up));
    }
    out.terms = vec4<f32>(fog, fade, w, t);
    return out;
}

// Pass 0x3F (§6.4 steps 1–3): the sun per pixel through the tiled normal map, in the
// tangent frame of Tessera Decision FD5 (tangent = world east made orthogonal to the
// vertex normal, bitangent = normal x tangent).
fn far_pixel_light(n_vertex: vec3<f32>, normal_texel: vec3<f32>, t: f32) -> vec3<f32> {
    let n = frame_normalize(n_vertex, vec3<f32>(0.0, 0.0, 1.0));
    let east = vec3<f32>(1.0, 0.0, 0.0);
    let tangent = frame_normalize(east - n * dot(east, n), vec3<f32>(0.0, 1.0, 0.0));
    let bitangent = cross(n, tangent);
    let to_tangent = transpose(mat3x3<f32>(tangent, bitangent, n));
    let l = to_tangent * frame.sun_dir.xyz;
    let u = to_tangent * frame.up.xyz;
    let np = t * vec3<f32>(2.0 * normal_texel.r - 1.0, 2.0 * normal_texel.g - 1.0,
        2.0 * normal_texel.b - 2.0) + vec3<f32>(0.0, 0.0, 1.0);
    let e = clamp(dot(np, l), 0.0, 1.0);
    let e1 = clamp(2.0 * e - 0.9, 0.0, 1.0);
    let mu = max(max(l.x, l.y), l.z);
    let e2 = e + mu * (e1 - e);
    let ambient = clamp(frame.ambient.rgb + dot(np, u) * frame.hemispheric.rgb,
        vec3<f32>(0.0), vec3<f32>(1.0));
    return ambient + e2 * frame.sun_color.rgb;
}

@fragment
fn fs_far(v: FarOut) -> @location(0) vec4<f32> {
    // Every sample first, in uniform control flow.
    let colour = far_stored(textureSample(t_colour, s_clamp, v.uv * fd.colour.xy), FS_COLOUR).rgb;
    let detail_uv = v.uv * fd.params.xy;
    let detail = far_stored(textureSample(t_detail, s_wrap, detail_uv), FS_DETAIL).rgb;
    let texel = far_stored(textureSample(t_normal, s_wrap, detail_uv), FS_NORMAL).rgb;
    let a = v.terms.y * v.terms.z;
    if (far_flag(FT_NORMAL_MAP)) {
        if (a < 0.001) {
            discard;
        }
    } else if (v.terms.y < 0.1) {
        discard;
    }
    var light = v.light;
    if (far_flag(FT_NORMAL_MAP) && frame.sun_dir.w > 0.5) {
        light = far_pixel_light(v.normal, texel, v.terms.w);
    }
    let c = (colour + detail - vec3<f32>(0.5)) * light;
    let rgb = (1.0 - v.terms.x) * 2.0 * c + v.terms.x * frame.fog_color.rgb;
    return vec4<f32>(frame_output(rgb), a);
}
