// SPDX-License-Identifier: GPL-3.0-or-later
// The water pipeline (M4.5). Written for Tessera from docs/specs/water.md §8.3–§8.4 (the
// water permutations of shader record 0x2B0000A8) and the Tessera rules there; layout in
// docs/specs/render.md §5.3. Appended after frame.wgsl and frame_shading.wgsl.

struct Water {
    // x = wave speed, y = wave frequency, z = wave amplitude, w = normal layer count.
    waves: vec4<f32>,
    // Normal layer scroll speeds: layer 0 (u, v), layer 1 (u, v).
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
};

@group(1) @binding(0) var<uniform> water: Water;
@group(1) @binding(1) var t_normal: texture_2d<f32>;
@group(1) @binding(2) var t_scene: texture_2d<f32>;
@group(1) @binding(3) var s_repeat: sampler;
@group(1) @binding(4) var s_clamp: sampler;

struct QuadIn {
    // origin.xy, size.xy
    @location(0) rect: vec4<f32>,
    // x = height
    @location(1) height: vec4<f32>,
};

struct WaterOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    // Normal-layer coordinates 0 and 1 (xy, zw) and 2 (xy).
    @location(1) uv01: vec4<f32>,
    @location(2) uv2: vec2<f32>,
    // Surface frame from the waves: tangent (+x) and bitangent (+y) slopes.
    @location(3) slope: vec2<f32>,
    @location(4) fog: f32,
};

const PI: f32 = 3.14159265358979;
const TAU: f32 = 6.28318530717959;

// s reduced to [-pi, pi) before the sine (water.md §8.3).
fn wrap_pi(s: f32) -> f32 {
    return s - TAU * floor((s + PI) / TAU);
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
    let t = frame.eye.w;
    // Vertex waves (§8.3): sum over k = 0, 1 of A_k sin s_k.
    var h = 0.0;
    var dx = 0.0;
    var dy = 0.0;
    if (water.waves.z != 0.0) {
        for (var k = 0; k < 2; k = k + 1) {
            let kf = f32(k);
            let a = water.waves.z * (1.0 - 0.5 * kf);
            let fx = (-1.0 + 0.3 * kf) * water.waves.y * (kf + 1.0);
            let fy = 0.7 * kf * water.waves.y * (kf + 1.0);
            let s = wrap_pi(fx * xy.x + fy * xy.y + water.waves.x * t * (0.5 + 0.8 * kf));
            h = h + a * sin(s);
            dx = dx + a * cos(s) * fx;
            dy = dy + a * cos(s) * fy;
        }
    }
    let world = vec3<f32>(xy, q.height.x + h);
    // Normal layers (§8.3): B = (P.xy - O) / 160, layer k at B * Scale_k + Speed_k * T.
    let b = (xy - water.screen.zw) / 160.0;
    var out: WaterOut;
    out.clip = frame.view_proj * vec4<f32>(world, 1.0);
    out.world_pos = world;
    out.uv01 = vec4<f32>(b * water.speed2_scale01.z + water.speed01.xy * t,
        b * water.speed2_scale01.w + water.speed01.zw * t);
    out.uv2 = b * water.misc0.x + water.speed2_scale01.xy * t;
    out.slope = vec2<f32>(dx, dy);
    out.fog = frame_fog_factor(world);
    return out;
}

fn scene_stored(c: vec4<f32>) -> vec3<f32> {
    // The copy has the target's format: an sRGB target decodes on sampling.
    if ((frame.params.y & FRAME_SRGB_TARGET) != 0u) {
        return frame_linear_to_srgb(c.rgb);
    }
    return c.rgb;
}

@fragment
fn fs_water(in: WaterOut) -> @location(0) vec4<f32> {
    // 1. Normal: the three layers mapped to 2 (rgb - 0.5), summed, normalised.
    var s0 = textureSample(t_normal, s_repeat, in.uv01.xy).rgb;
    var s1 = textureSample(t_normal, s_repeat, in.uv01.zw).rgb;
    var s2 = textureSample(t_normal, s_repeat, in.uv2).rgb;
    // A map uploaded sRGB decodes on sampling: back to the stored texels (render.md §5.1).
    if (water.misc2.z > 1.5) {
        s0 = frame_linear_to_srgb(s0);
        s1 = frame_linear_to_srgb(s1);
        s2 = frame_linear_to_srgb(s2);
    }
    let layers = water.waves.w;
    var sum = 2.0 * (s0 - 0.5);
    if (layers >= 2.0) {
        sum = sum + 2.0 * (s1 - 0.5);
    }
    if (layers >= 3.0) {
        sum = sum + 2.0 * (s2 - 0.5);
    }
    var nt = vec3<f32>(0.0, 0.0, 1.0);
    if (water.misc2.z > 0.5) {
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
    // 3. Refraction of the scene drawn so far, tinted toward the under-water fog colour,
    // blended toward the shoreline tint near the camera.
    let scene = textureSample(t_scene, s_clamp, s + n.xy * water.misc0.z);
    var f = water.fog_color.rgb;
    if (water.misc2.y > 0.5) {
        f = scene_stored(scene);
    }
    let u = f + water.misc2.x * (water.fog_color.rgb - f);
    let to_eye = frame.eye.xyz - in.world_pos;
    let dist = length(to_eye);
    let w = clamp((dist - 5.0) * 0.1, 0.0, 1.0);
    let shore = f * water.shoreline_color.rgb;
    let refraction = shore + w * (u - shore);
    // 4. Reflection: the permutation without a reflection texture uses the unmodified
    // refraction sample (Tessera rule).
    let reflection = f;
    // 5. Fresnel.
    let v = frame_normalize(to_eye, vec3<f32>(0.0, 0.0, 1.0));
    let fr = water.misc0.w + (1.0 - water.misc0.w)
        * pow(clamp(1.0 - abs(dot(v, n)), 0.0, 1.0), water.misc1.x);
    var color = refraction + fr * (reflection - refraction);
    // 6. Specular: the fake light's travel direction L (the main light's, else straight
    // down), plus the sparkle of the tangent-space normal.
    var travel = vec3<f32>(0.0, 0.0, -1.0);
    var fake_specular = vec3<f32>(0.0);
    if (frame.sun_dir.w > 0.0) {
        travel = -frame.sun_dir.xyz;
        fake_specular = frame.sun_specular.rgb;
    }
    let h = frame_normalize(v - travel, n);
    let spec = frame_spec_power(dot(n, h), water.misc1.y)
        + 4.0 * (clamp(nt.x - water.misc1.w, 0.0, 1.0) + clamp(nt.y - water.misc1.w, 0.0, 1.0));
    // 7. Output: water colour and overbright, specular, then distance fog.
    color = color * water.water_color.rgb * water.misc1.z
        + spec * fake_specular * water.specular_color.rgb;
    color = frame_apply_fog(color, clamp(in.fog, 0.0, 1.0));
    return vec4<f32>(frame_output(color), 1.0);
}
