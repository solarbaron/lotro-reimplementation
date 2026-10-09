// SPDX-License-Identifier: GPL-3.0-or-later
// Distant sprites (imposters) of the distant-landscape sectors (docs/specs/far-view.md
// §9.3, §9.6; misc-records.md §5.2–§5.4). Written for Tessera from the spec's prose.
// Appended after frame.wgsl and frame_shading.wgsl; `SpriteDraw` matches
// `SpriteDrawUniform` in crates/render/src/far_view.rs.

struct SpriteDraw {
    model: mat4x4<f32>,
    // x = distance scale c, y = alpha-test threshold, z = fade start s, w = inverse range r.
    params: vec4<f32>,
    // x = flags (SP_*), y = sRGB bits (SS_*), z, w unused.
    flags: vec4<u32>,
};

const SP_NORMAL_ATLAS: u32 = 1u;
const SS_COLOUR: u32 = 1u;
const SS_NORMAL: u32 = 2u;

@group(1) @binding(0) var<uniform> sd: SpriteDraw;
@group(1) @binding(1) var t_atlas: texture_2d<f32>;
@group(1) @binding(2) var t_normals: texture_2d<f32>;
@group(1) @binding(3) var s_atlas: sampler;

struct SpriteIn {
    // xyz = anchor, w = size S.
    @location(0) pos_size: vec4<f32>,
    // x = base B, y = distance D, z = mirror (0 or 1).
    @location(1) base_dist: vec4<f32>,
    // left u, right u, top v, bottom v.
    @location(2) uv: vec4<f32>,
};

struct SpriteOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    // x = fog f, y = opacity factor, z = mirror sign (1 or -1).
    @location(1) terms: vec3<f32>,
    @location(2) right: vec3<f32>,
    @location(3) up: vec3<f32>,
    @location(4) back: vec3<f32>,
};

fn sprite_stored(c: vec4<f32>, bit: u32) -> vec4<f32> {
    if ((sd.flags.y & bit) == 0u) {
        return c;
    }
    return vec4<f32>(frame_linear_to_srgb(c.rgb), c.a);
}

@vertex
fn vs_sprite(@builtin(vertex_index) vi: u32, s: SpriteIn) -> SpriteOut {
    var out: SpriteOut;
    let p = (sd.model * vec4<f32>(s.pos_size.xyz, 1.0)).xyz;
    let size = s.pos_size.w;
    let base = s.base_dist.x;
    // misc-records.md §5.2: d from the anchor to the eye, R horizontal, U = d x R.
    let to_eye = frame.eye.xyz - p;
    let dist = length(to_eye);
    let d = frame_normalize(to_eye, vec3<f32>(0.0, 0.0, 1.0));
    let r = frame_normalize(vec3<f32>(-d.y, d.x, 0.0), vec3<f32>(1.0, 0.0, 0.0));
    let u = frame_normalize(cross(d, r), vec3<f32>(0.0, 0.0, 1.0));
    // Two triangles: (TL, BL, TR), (TR, BL, BR).
    var corner = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0), vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0),
    );
    let c = corner[vi % 6u];
    let side = c.x - 0.5;
    let world = p + (side * size) * r + (base + c.y * size) * u;
    out.clip = frame.view_proj * vec4<f32>(world, 1.0);
    out.uv = vec2<f32>(mix(s.uv.x, s.uv.y, c.x), mix(s.uv.w, s.uv.z, c.y));
    // far-view.md §9.3: sat((d - (D c - 25)) / 10) times the far-draw fade.
    let fade_in = clamp((dist - (s.base_dist.y * sd.params.x - 25.0)) / 10.0, 0.0, 1.0);
    let fade_out = 1.0 - clamp((dist - sd.params.z) * sd.params.w, 0.0, 1.0);
    let mirror = select(1.0, -1.0, s.base_dist.z > 0.5);
    out.terms = vec3<f32>(frame_fog_factor(p), fade_in * fade_out, mirror);
    out.right = r;
    out.up = u;
    out.back = -d;
    return out;
}

@fragment
fn fs_sprite(v: SpriteOut) -> @location(0) vec4<f32> {
    let albedo = sprite_stored(textureSample(t_atlas, s_atlas, v.uv), SS_COLOUR);
    let nt = sprite_stored(textureSample(t_normals, s_atlas, v.uv), SS_NORMAL).rgb;
    let coverage = albedo.a * v.terms.y;
    if (coverage < sd.params.y + 0.001) {
        discard;
    }
    // misc-records.md §5.3: R along R (along -R mirrored), G along U, B along -d.
    var n = v.up;
    if ((sd.flags.x & SP_NORMAL_ATLAS) != 0u) {
        let t = 2.0 * nt - vec3<f32>(1.0);
        n = frame_normalize(t.x * v.terms.z * v.right + t.y * v.up + t.z * v.back, v.up);
    }
    // The fake directional light: the main light when it is directional, else a grey of
    // 0.6 from straight above (lighting.md E18; the default direction is Tessera's).
    var l = vec3<f32>(0.0, 0.0, 1.0);
    var lc = vec3<f32>(0.6);
    if (frame.sun_dir.w > 0.5) {
        l = frame.sun_dir.xyz;
        lc = frame.sun_color.rgb;
    }
    let ambient = clamp(frame.ambient.rgb + dot(n, v.up) * frame.hemispheric.rgb,
        vec3<f32>(0.0), vec3<f32>(1.0));
    let light = ambient + clamp(dot(n, l), 0.0, 1.0) * lc;
    let c = 2.0 * albedo.rgb * light;
    let rgb = (1.0 - v.terms.x) * c + v.terms.x * frame.fog_color.rgb;
    return vec4<f32>(frame_output(rgb), coverage);
}
