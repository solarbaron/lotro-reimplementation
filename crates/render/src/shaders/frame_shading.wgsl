// SPDX-License-Identifier: GPL-3.0-or-later
// Shared lighting functions of the lit pipelines (M4.5), appended after frame.wgsl.
// Written for Tessera from the equations of docs/specs/lighting.md (§5, §8.3, §9, §10);
// `crate::lighting::reference` is the CPU twin used by the tests. Every name starts with
// `frame_` so pipelines that include this file keep their own namespace.

@group(0) @binding(1) var frame_shadow_map: texture_depth_2d;
@group(0) @binding(2) var frame_shadow_sampler: sampler_comparison;

// Diffuse and specular contribution of one light, before the material colours.
struct FrameLightTerm {
    diffuse: vec3<f32>,
    specular: vec3<f32>,
};

fn frame_normalize(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let l2 = dot(v, v);
    if (l2 > 1e-24) {
        return v * inverseSqrt(l2);
    }
    return fallback;
}

fn frame_srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let x = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(pow((x + 0.055) / 1.055, vec3<f32>(2.4)), x / 12.92, x <= vec3<f32>(0.04045));
}

fn frame_linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let x = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(1.055 * pow(x, vec3<f32>(1.0 / 2.4)) - 0.055, x * 12.92,
        x <= vec3<f32>(0.0031308));
}

// Final colour (stored-value space) -> what the fragment writes: an sRGB target encodes on
// write, so the value is decoded first and the stored result equals `c`.
fn frame_output(c: vec3<f32>) -> vec3<f32> {
    if ((frame.params.y & FRAME_SRGB_TARGET) != 0u) {
        return frame_srgb_to_linear(c);
    }
    return c;
}

// Distance fog factor f of a world position (lighting.md §10); evaluated per vertex.
fn frame_fog_factor(world_pos: vec3<f32>) -> f32 {
    if (frame.fog_color.w == 0.0) {
        return 0.0;
    }
    let d = length(frame.eye.xyz - world_pos);
    return clamp((d - frame.fog.x) * frame.fog.y, 0.0, 1.0);
}

// Base passes blend towards the fog colour (lighting.md §5.11).
fn frame_apply_fog(c: vec3<f32>, f: f32) -> vec3<f32> {
    return mix(c, frame.fog_color.rgb, f);
}

// Ambient with the hemispheric term (lighting.md §5.3) for material colour `m`:
// sat(m * ambient + (N . U) * m * hemispheric).
fn frame_ambient(n: vec3<f32>, m: vec3<f32>) -> vec3<f32> {
    let a0 = m * frame.ambient.rgb;
    let h0 = m * frame.hemispheric.rgb;
    return clamp(a0 + dot(n, frame.up.xyz) * h0, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Point-light attenuation with the newer back end's smooth fade (lighting.md §8.3).
fn frame_attenuation(d2: f32, r2: f32, k: f32) -> f32 {
    if (d2 >= r2) {
        return 0.0;
    }
    var a = 1.0 / (1.0 + k * d2);
    if (d2 > 0.75 * r2) {
        let s = (r2 - d2) / (0.25 * r2);
        a = a * s * s * (3.0 - 2.0 * s);
    }
    return a;
}

// sat(x)^p as exp2(p * log2(x)) (lighting.md §5.8); 0 for x <= 0.
fn frame_spec_power(x: f32, p: f32) -> f32 {
    let s = clamp(x, 0.0, 1.0);
    if (s <= 0.0) {
        return 0.0;
    }
    return exp2(p * log2(s));
}

// Shadow-map coordinates (u, v, depth) of a world position (lighting.md §9 step 2).
fn frame_shadow_coords(world_pos: vec3<f32>) -> vec3<f32> {
    let c = frame.shadow_view_proj * vec4<f32>(world_pos, 1.0);
    let p = c.xyz / max(abs(c.w), 1e-20) * sign(c.w);
    return vec3<f32>(0.5 * p.x + 0.5, 0.5 - 0.5 * p.y, p.z);
}

fn frame_shadow_tap(uv: vec2<f32>, z: f32) -> f32 {
    return textureSampleCompareLevel(frame_shadow_map, frame_shadow_sampler, uv, z);
}

// Visibility s in [0, 1] (lighting.md §9 step 4): one tap at quality 1, otherwise five taps
// (centre 0.36, one texel left, right, up and down 0.16 each). Outside the map: lit.
fn frame_shadow_visibility(coords: vec3<f32>) -> f32 {
    if (any(coords.xy < vec2<f32>(0.0)) || any(coords.xy > vec2<f32>(1.0)) || coords.z > 1.0
        || coords.z < 0.0) {
        return 1.0;
    }
    let z = coords.z - frame.shadow_color.w;
    let t = frame.shadow.w;
    if (frame.shadow.x < 2.0) {
        return frame_shadow_tap(coords.xy, z);
    }
    return 0.36 * frame_shadow_tap(coords.xy, z)
        + 0.16 * (frame_shadow_tap(coords.xy + vec2<f32>(t, 0.0), z)
        + frame_shadow_tap(coords.xy - vec2<f32>(t, 0.0), z)
        + frame_shadow_tap(coords.xy + vec2<f32>(0.0, t), z)
        + frame_shadow_tap(coords.xy - vec2<f32>(0.0, t), z));
}

// Shadow factor S per channel (lighting.md §9 steps 5-7); 1 without shadows. `view_depth`
// is the clip w of the fragment.
fn frame_shadow_factor(coords: vec3<f32>, view_depth: f32) -> vec3<f32> {
    let q = frame.shadow.x;
    let phi = clamp(q, 0.0, 1.0)
        * clamp(1.0 - (view_depth - 0.1 - frame.shadow.y) * frame.shadow.z, 0.0, 1.0);
    if (phi <= 0.0) {
        return vec3<f32>(1.0);
    }
    let s = frame_shadow_visibility(coords);
    let shadow_color = frame.shadow_color.rgb + frame.shadow_range.rgb * s;
    return vec3<f32>(1.0) + phi * (shadow_color - vec3<f32>(1.0));
}

// The main directional light (lighting.md §5.4, §5.8): diffuse = sat(N . L0) S E0,
// specular = g sat(N . H)^p S Fs. `power < 0` means no specular.
fn frame_sun(n: vec3<f32>, v: vec3<f32>, gloss: f32, power: f32, s: vec3<f32>) -> FrameLightTerm {
    var t: FrameLightTerm;
    t.diffuse = vec3<f32>(0.0);
    t.specular = vec3<f32>(0.0);
    if (frame.sun_dir.w == 0.0) {
        return t;
    }
    let l = frame.sun_dir.xyz;
    t.diffuse = clamp(dot(n, l), 0.0, 1.0) * s * frame.sun_color.rgb;
    if (power >= 0.0) {
        let h = frame_normalize(v + l, n);
        t.specular = gloss * frame_spec_power(dot(n, h), power) * s * frame.sun_specular.rgb;
    }
    return t;
}

// Entry `i` of the light table at world position `p` (lighting.md §5.6 per-pixel form,
// §8.3): diffuse = sat(N . L) a E; specular = clamp(g a sat(N . H)^p, 0, 1) F.
fn frame_light(i: u32, p: vec3<f32>, n: vec3<f32>, v: vec3<f32>, gloss: f32,
    power: f32) -> FrameLightTerm {
    let light = frame.lights[min(i, FRAME_MAX_LIGHTS - 1u)];
    var l = vec3<f32>(0.0, 0.0, 1.0);
    var a = 1.0;
    if (light.specular_kind.w > 0.5) {
        l = frame_normalize(light.position_k.xyz, l);
    } else {
        let d = light.position_k.xyz - p;
        let d2 = dot(d, d);
        a = frame_attenuation(d2, light.color_r2.w, light.position_k.w);
        l = frame_normalize(d, n);
    }
    var t: FrameLightTerm;
    t.diffuse = clamp(dot(n, l), 0.0, 1.0) * a * light.color_r2.rgb;
    t.specular = vec3<f32>(0.0);
    if (power >= 0.0 && a > 0.0) {
        let h = frame_normalize(v + l, n);
        t.specular = clamp(gloss * a * frame_spec_power(dot(n, h), power), 0.0, 1.0)
            * light.specular_kind.rgb;
    }
    return t;
}

// The binormal B of a normal map (lighting.md §5.2.4): the stored binormal as given (only
// normalised; render.md §4.8 OPEN on departure R-D10), or, where the mesh has none or it is
// (near) zero, w x cross(N0, T) of the mesh convention (render.md §2.1). Zero when neither
// gives a direction, in which case the caller keeps N0. `t` is the interpolated tangent
// with its sign in w, `stored` the interpolated stored binormal, both after the transform.
fn frame_binormal(n: vec3<f32>, t: vec4<f32>, stored: vec3<f32>) -> vec3<f32> {
    let t_len2 = dot(t.xyz, t.xyz);
    if (t_len2 <= 1e-12) {
        return vec3<f32>(0.0);
    }
    if (dot(stored, stored) > 1e-6 * t_len2) {
        return normalize(stored);
    }
    let b_raw = cross(n, t.xyz);
    if (dot(b_raw, b_raw) <= 1e-12 * t_len2) {
        return vec3<f32>(0.0);
    }
    return normalize(b_raw) * select(-1.0, 1.0, t.w >= 0.0);
}
