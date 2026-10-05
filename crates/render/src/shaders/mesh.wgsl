// SPDX-License-Identifier: GPL-3.0-or-later
// Mesh surfaces (opaque / alpha-test / alpha-blend entry points), wireframe and
// vertex-normal overlays, static and skinned. Written for Tessera; feature switches are
// uniforms.

struct Globals {
    view_proj: mat4x4<f32>,
    // xyz = camera position (world)
    eye: vec4<f32>,
    // xyz = normalised direction towards the light
    light_dir: vec4<f32>,
    light_color: vec4<f32>,
    ambient: vec4<f32>,
};

struct Draw {
    model: mat4x4<f32>,
    // Columns of the normal matrix (inverse transpose of the model's 3x3).
    normal0: vec4<f32>,
    normal1: vec4<f32>,
    normal2: vec4<f32>,
    base_color: vec4<f32>,
    // rgb = dye colour
    dye: vec4<f32>,
    // rgb = specular colour, w = shininess
    specular: vec4<f32>,
    // x = alpha cutoff, y = normal-map green sign, z = normal line length,
    // w = fraction of the eye distance overlay lines are pulled forward
    params: vec4<f32>,
    // wireframe colour
    overlay_color: vec4<f32>,
    // vertex-normal line colour
    normal_color: vec4<f32>,
    // x = feature bits, y = shading mode, z = dye mask source
    flags: vec4<u32>,
};

const F_NORMAL_MAP: u32 = 1u;
const F_DYE: u32 = 2u;
const F_SPECULAR: u32 = 4u;
const F_SPECULAR_TEX: u32 = 8u;
const F_UNLIT: u32 = 16u;
const F_TWO_SIDED: u32 = 32u;
const F_MIRRORED: u32 = 64u;

const MODE_LIT: u32 = 0u;
const MODE_BASE: u32 = 1u;
const MODE_NORMALS: u32 = 2u;
const MODE_UV: u32 = 3u;

const DYE_BASE_ALPHA: u32 = 2u;

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var<uniform> draw: Draw;

// Skinning palette of the draw: 256 affine matrices, three rows each (row-major 3 x 4),
// entry j = rows 3j .. 3j+2. Bound at the draw's dynamic offset; static draws ignore it.
struct Palette {
    rows: array<vec4<f32>, 768>,
};
@group(1) @binding(1) var<uniform> palette: Palette;
@group(2) @binding(0) var t_base: texture_2d<f32>;
@group(2) @binding(1) var t_normal: texture_2d<f32>;
@group(2) @binding(2) var t_dye: texture_2d<f32>;
@group(2) @binding(3) var t_spec: texture_2d<f32>;
@group(2) @binding(4) var s_base: sampler;
@group(2) @binding(5) var s_normal: sampler;
@group(2) @binding(6) var s_dye: sampler;
@group(2) @binding(7) var s_spec: sampler;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) color: vec4<f32>,
};

// Second vertex stream of skinned meshes: four bone indices (u8) and four weights
// (unorm16, summing to 1).
struct SkinIn {
    @location(5) joints: vec4<u32>,
    @location(6) weights: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) color: vec4<f32>,
};

fn normal_matrix() -> mat3x3<f32> {
    return mat3x3<f32>(draw.normal0.xyz, draw.normal1.xyz, draw.normal2.xyz);
}

fn safe_normalize(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let l2 = dot(v, v);
    if (l2 > 1e-24) {
        return v * inverseSqrt(l2);
    }
    return fallback;
}

// Shared by the static and skinned entry points, so a skinned vertex at its bind pose
// takes exactly the static path.
fn surface_vertex(in: VsIn, position: vec3<f32>, normal: vec3<f32>, tangent: vec3<f32>) -> VsOut {
    let world = draw.model * vec4<f32>(position, 1.0);
    let m3 = mat3x3<f32>(draw.model[0].xyz, draw.model[1].xyz, draw.model[2].xyz);
    var sign = 1.0;
    if ((draw.flags.x & F_MIRRORED) != 0u) {
        sign = -1.0;
    }
    var out: VsOut;
    out.clip = globals.view_proj * world;
    out.world_pos = world.xyz;
    out.normal = normal_matrix() * normal;
    out.tangent = vec4<f32>(m3 * tangent, in.tangent.w * sign);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    return surface_vertex(in, in.position, in.normal.xyz, in.tangent.xyz);
}

// ---- skinning ------------------------------------------------------------------------
//
// Linear blend skinning written as v + sum_k w_k * (M_k v - v). With weights summing to 1
// this equals sum_k w_k * M_k v; with identity matrices every term is exactly zero, so the
// bind pose reproduces the static path bit for bit. Directions (normal, tangent) use the
// matrices without translation; the fragment stage renormalises them.

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

@vertex
fn vs_skinned(in: VsIn, skin: SkinIn) -> VsOut {
    return surface_vertex(in, skin_point(skin, in.position), skin_dir(skin, in.normal.xyz),
        skin_dir(skin, in.tangent.xyz));
}

// 8 x 8 cells per UV unit, tinted by the UV value so orientation is visible.
fn uv_checker(uv: vec2<f32>) -> vec3<f32> {
    let cell = floor(uv * 8.0);
    let odd = ((i32(cell.x) + i32(cell.y)) & 1) == 1;
    let tint = vec3<f32>(fract(uv.x), fract(uv.y), 0.5);
    let level = select(0.9, 0.25, odd);
    return level * (0.35 + 0.65 * tint);
}

fn shade(in: VsOut, front_facing: bool) -> vec4<f32> {
    // Sample everything up front (uniform control flow).
    let base_tex = textureSample(t_base, s_base, in.uv);
    let normal_tex = textureSample(t_normal, s_normal, in.uv);
    let dye_tex = textureSample(t_dye, s_dye, in.uv);
    let spec_tex = textureSample(t_spec, s_spec, in.uv);
    let f = draw.flags.x;
    let mode = draw.flags.y;

    var base = base_tex * draw.base_color * in.color;
    if ((f & F_DYE) != 0u) {
        var mask = dye_tex.r;
        if (draw.flags.z == DYE_BASE_ALPHA) {
            mask = base_tex.a;
            base.a = draw.base_color.a * in.color.a;
        }
        let tint = mix(vec3<f32>(1.0), draw.dye.rgb, clamp(mask, 0.0, 1.0));
        base = vec4<f32>(base.rgb * tint, base.a);
    }

    var facing = front_facing;
    if ((f & F_MIRRORED) != 0u) {
        facing = !facing;
    }
    // Perturb the front-side normal first (the TBN must be built from the
    // unflipped normal), then flip the result for back faces of two-sided
    // materials: the back of a surface is lit with the negated front normal.
    var n = safe_normalize(in.normal, vec3<f32>(0.0, 0.0, 1.0));
    if ((f & F_NORMAL_MAP) != 0u) {
        let t_raw = in.tangent.xyz - n * dot(n, in.tangent.xyz);
        if (dot(t_raw, t_raw) > 1e-12) {
            let t = normalize(t_raw);
            let b = cross(n, t) * select(-1.0, 1.0, in.tangent.w >= 0.0);
            var tn = normal_tex.xyz * 2.0 - 1.0;
            tn.y = tn.y * draw.params.y;
            n = safe_normalize(t * tn.x + b * tn.y + n * tn.z, n);
        }
    }
    if ((f & F_TWO_SIDED) != 0u && !facing) {
        n = -n;
    }

    if (mode == MODE_BASE || (mode == MODE_LIT && (f & F_UNLIT) != 0u)) {
        return base;
    }
    if (mode == MODE_NORMALS) {
        return vec4<f32>(n * 0.5 + 0.5, base.a);
    }
    let l = globals.light_dir.xyz;
    let ndl = max(dot(n, l), 0.0);
    if (mode == MODE_UV) {
        return vec4<f32>(uv_checker(in.uv) * (0.4 + 0.6 * ndl), base.a);
    }
    var color = base.rgb * (globals.ambient.rgb + globals.light_color.rgb * ndl);
    if ((f & F_SPECULAR) != 0u && ndl > 0.0) {
        let v = safe_normalize(globals.eye.xyz - in.world_pos, n);
        let h = safe_normalize(l + v, n);
        var spec = draw.specular.rgb;
        if ((f & F_SPECULAR_TEX) != 0u) {
            spec = spec * spec_tex.rgb;
        }
        color = color + spec * globals.light_color.rgb * pow(max(dot(n, h), 0.0), draw.specular.w);
    }
    return vec4<f32>(color, base.a);
}

@fragment
fn fs_opaque(in: VsOut, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    let c = shade(in, front_facing);
    return vec4<f32>(c.rgb, 1.0);
}

@fragment
fn fs_mask(in: VsOut, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    let c = shade(in, front_facing);
    if (c.a < draw.params.x) {
        discard;
    }
    return vec4<f32>(c.rgb, 1.0);
}

@fragment
fn fs_blend(in: VsOut, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    return shade(in, front_facing);
}

// ---- overlays ----------------------------------------------------------------------

struct LineOut {
    @builtin(position) clip: vec4<f32>,
};

// Pull overlay lines slightly towards the camera so they win the depth test
// against the surface they outline.
fn pulled(world: vec3<f32>) -> vec4<f32> {
    let p = world + (globals.eye.xyz - world) * draw.params.w;
    return globals.view_proj * vec4<f32>(p, 1.0);
}

fn wire_vertex(position: vec3<f32>) -> LineOut {
    let world = (draw.model * vec4<f32>(position, 1.0)).xyz;
    var out: LineOut;
    out.clip = pulled(world);
    return out;
}

@vertex
fn vs_wire(in: VsIn) -> LineOut {
    return wire_vertex(in.position);
}

@vertex
fn vs_wire_skinned(in: VsIn, skin: SkinIn) -> LineOut {
    return wire_vertex(skin_point(skin, in.position));
}

fn normal_line_vertex(vi: u32, position: vec3<f32>, normal: vec3<f32>) -> LineOut {
    let world = (draw.model * vec4<f32>(position, 1.0)).xyz;
    let n = safe_normalize(normal_matrix() * normal, vec3<f32>(0.0));
    var out: LineOut;
    out.clip = pulled(world + n * (f32(vi & 1u) * draw.params.z));
    return out;
}

// Drawn with the mesh's vertex buffer stepped per instance: two vertices per
// mesh vertex, the second one moved along the normal.
@vertex
fn vs_normals(@builtin(vertex_index) vi: u32, in: VsIn) -> LineOut {
    return normal_line_vertex(vi, in.position, in.normal.xyz);
}

// The same with the skin stream stepped per instance too.
@vertex
fn vs_normals_skinned(@builtin(vertex_index) vi: u32, in: VsIn, skin: SkinIn) -> LineOut {
    return normal_line_vertex(vi, skin_point(skin, in.position), skin_dir(skin, in.normal.xyz));
}

@fragment
fn fs_overlay() -> @location(0) vec4<f32> {
    return draw.overlay_color;
}

@fragment
fn fs_normal_lines() -> @location(0) vec4<f32> {
    return draw.normal_color;
}
