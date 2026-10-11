// SPDX-License-Identifier: GPL-3.0-or-later
// Instanced scenery and frills (M4.5; docs/specs/scenery.md §8.3, §8.5, §9 and
// docs/specs/render.md §5.5). Written for Tessera. Appended after frame.wgsl,
// frame_shading.wgsl and instanced_common.wgsl. Lighting as the lit mesh pipeline
// (lighting.md §5); the frill sway, terrain-colour blend and distance scale are Tessera
// Decisions (the original's shader formulas are OPEN, scenery.md §8.3).

struct Batch {
    // x, y, z unused (the light lists are per instance), w = flags (BF_*).
    lights: vec4<u32>,
    // x = far distance D, y = fade length F, z = animation intensity,
    // w = terrain-colour intensity.
    frill: vec4<f32>,
    // x = z_min of the model's bounding box, y = sway height h (0: none),
    // z = fade-in factor.
    sway: vec4<f32>,
    // This frame's texture matrices 0 and 1 when BF_UV is set (packed as Surface.uv*).
    uv0: vec4<f32>,
    uv1: vec4<f32>,
    uv2: vec4<f32>,
};

const BF_FRILL: u32 = 1u;
const BF_SWAY: u32 = 2u;
const BF_UV: u32 = 8u;

@group(1) @binding(0) var<uniform> batch: Batch;

struct InstIn {
    // Rows of the affine model -> world matrix.
    @location(7) r0: vec4<f32>,
    @location(8) r1: vec4<f32>,
    @location(9) r2: vec4<f32>,
    @location(10) tint: vec4<f32>,
    // rgb = terrain colour beneath the instance, w = sway phase.
    @location(11) terrain: vec4<f32>,
    // The instance's light list: x, y = light-table indices (four u8 each, low byte
    // first), z = count (lighting.md §8.5: one list per object).
    @location(12) lights: vec4<u32>,
};

struct InstOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) color: vec4<f32>,
    @location(5) shadow: vec3<f32>,
    // x = fog factor, y = view depth (clip w), z = sway weight w.
    @location(6) fog_depth: vec3<f32>,
    @location(7) tint: vec4<f32>,
    @location(8) terrain: vec3<f32>,
    @location(9) @interpolate(flat) lights: vec4<u32>,
    // The stored binormal after the transform (zero when the model has none).
    @location(10) binormal: vec3<f32>,
};

fn batch_flag(bit: u32) -> bool {
    return (batch.lights.w & bit) != 0u;
}

// Texture coordinates through matrix k: the batch's of this frame, else the surface's own
// (material.md §8.4).
fn inst_uv(k: u32, uv: vec2<f32>) -> vec2<f32> {
    if (batch_flag(BF_UV)) {
        return uv_through(batch.uv0, batch.uv1, batch.uv2, k, uv);
    }
    return uv_through(surface.uv0, surface.uv1, surface.uv2, k, uv);
}

fn safe_unit(v: vec3<f32>) -> vec3<f32> {
    return frame_normalize(v, vec3<f32>(0.0));
}

struct Placed {
    world: vec3<f32>,
    normal: vec3<f32>,
    tangent: vec4<f32>,
    binormal: vec3<f32>,
    weight: f32,
};

// Model -> world for one vertex of one instance, with the frill sway and distance scale.
fn place(v: ModelIn, inst: InstIn) -> Placed {
    let c0 = vec3<f32>(inst.r0.x, inst.r1.x, inst.r2.x);
    let c1 = vec3<f32>(inst.r0.y, inst.r1.y, inst.r2.y);
    let c2 = vec3<f32>(inst.r0.z, inst.r1.z, inst.r2.z);
    let origin = vec3<f32>(inst.r0.w, inst.r1.w, inst.r2.w);
    let m3 = mat3x3<f32>(c0, c1, c2);
    var p = v.position;
    var out: Placed;
    out.weight = 0.0;
    var scale = 1.0;
    if (batch_flag(BF_FRILL)) {
        // Sway weight (scenery.md §8.5 step 5): (z - z_min) / h from the model's box.
        if (batch.sway.y > 0.0) {
            out.weight = clamp((v.position.z - batch.sway.x) / batch.sway.y, 0.0, 1.0);
        }
        if (batch_flag(BF_SWAY)) {
            // Tessera Decision (scenery.md §10): a slow two-axis sway of the upper
            // vertices, amplitude = intensity x height, weighted by w^2.
            let t = frame.eye.w;
            let ph = inst.terrain.w;
            let a = batch.frill.z * batch.sway.y * out.weight * out.weight;
            p = p + vec3<f32>(a * sin(ph + 1.7 * t), 0.5 * a * sin(1.3 * ph + 1.1 * t), 0.0);
        }
        // Distance scale (Tessera Decision for scenery.md §8.3 rule 3): full size up to
        // D - F, scaled to nothing at D; times the group's fade-in (rule 5).
        let d = distance(frame.eye.xyz, origin);
        scale = clamp((batch.frill.x - d) / max(batch.frill.y, 1e-3), 0.0, 1.0) * batch.sway.z;
        // Normals are rotated, not scaled (§8.5 step 5). A mirrored instance (negative
        // determinant) flips the bitangent sign, as for scenery (render.md §2.3, §4.1).
        let r = mat3x3<f32>(safe_unit(c0), safe_unit(c1), safe_unit(c2));
        let s = select(1.0, -1.0, dot(c0, cross(c1, c2)) < 0.0);
        out.normal = r * v.normal.xyz;
        out.tangent = vec4<f32>(r * v.tangent.xyz, v.tangent.w * s);
        // The stored binormal turns with the tangent; a mirroring r mirrors it with the
        // texture, so it takes no sign.
        out.binormal = r * v.binormal.xyz;
    } else {
        // Inverse transpose (times the determinant) for non-uniform scales.
        let cof = mat3x3<f32>(cross(c1, c2), cross(c2, c0), cross(c0, c1));
        let det = dot(c0, cross(c1, c2));
        let s = select(1.0, -1.0, det < 0.0);
        out.normal = cof * v.normal.xyz * s;
        out.tangent = vec4<f32>(m3 * v.tangent.xyz, v.tangent.w * s);
        out.binormal = m3 * v.binormal.xyz;
    }
    out.world = origin + scale * (m3 * p);
    return out;
}

@vertex
fn vs_main(v: ModelIn, inst: InstIn) -> InstOut {
    let pl = place(v, inst);
    var out: InstOut;
    out.clip = frame.view_proj * vec4<f32>(pl.world, 1.0);
    out.world_pos = pl.world;
    out.normal = pl.normal;
    out.tangent = pl.tangent;
    out.binormal = pl.binormal;
    out.uv = v.uv;
    out.color = v.color;
    out.shadow = frame_shadow_coords(pl.world);
    out.fog_depth = vec3<f32>(frame_fog_factor(pl.world), out.clip.w, pl.weight);
    out.tint = inst.tint;
    out.terrain = inst.terrain.rgb;
    out.lights = inst.lights;
    return out;
}

fn light_index(lights: vec4<u32>, i: u32) -> u32 {
    let word = select(lights.x, lights.y, i >= 4u);
    return (word >> ((i & 3u) * 8u)) & 255u;
}

struct Shaded {
    color: vec3<f32>,
    alpha: f32,
    opacity: f32,
    unfogged: vec3<f32>,
    fog: f32,
};

fn shade(in: InstOut) -> Shaded {
    let uv0 = inst_uv(0u, in.uv);
    let base = surface_base_at(uv0, inst_uv(1u, in.uv));
    let ntex = surface_normal_texel(uv0);
    var out: Shaded;
    out.opacity = surface_opacity_texel(base);
    var m = surface.color * in.tint;
    if (surface_flag(SF_VERTEX_COLORS)) {
        m = m * in.color;
    }
    // Output alpha (lighting.md §5.12); for a fixed blend the tested opacity alone
    // (material.md §11.3).
    out.alpha = select(surface_opacity(base) * m.a, out.opacity, surface_flag(SF_FACTOR_OPACITY));
    var c = base.rgb;
    if (batch_flag(BF_FRILL)) {
        // Tessera Decision (scenery.md §8.3 rule 7): the frill's base blends toward the
        // terrain colour beneath it, its tips keep their own colour.
        c = mix(c, in.terrain, batch.frill.w * (1.0 - in.fog_depth.z));
    }
    // SF_NO_FOG is set only on unlit surfaces: the lit base pass is always fogged.
    out.fog = select(clamp(in.fog_depth.x, 0.0, 1.0), 0.0, surface_flag(SF_NO_FOG));
    if (surface_flag(SF_UNLIT)) {
        out.unfogged = c * m.rgb;
        out.color = frame_apply_fog(out.unfogged, out.fog);
        return out;
    }
    var n = frame_normalize(in.normal, vec3<f32>(0.0, 0.0, 1.0));
    if (surface_flag(SF_NORMAL_MAP)) {
        // The tangent as given (normalised, not made orthogonal to N0) and the stored
        // binormal, or the derived one without it (frame_binormal, render.md §4.1, §4.8).
        let b = frame_binormal(n, in.tangent, in.binormal);
        if (dot(b, b) > 0.0) {
            let t = normalize(in.tangent.xyz);
            let tn = ntex.xyz * 2.0 - 1.0;
            // lighting.md §5.2.4: N = n.x T - n.y B + n.z N0.
            n = frame_normalize(t * tn.x - b * tn.y + n * tn.z, n);
        }
    }
    var gloss = 1.0;
    if (surface_flag(SF_GLOSS_BASE_ALPHA)) {
        gloss = base.a;
    } else if (surface_flag(SF_GLOSS_NORMAL_ALPHA)) {
        gloss = ntex.a;
    }
    let power = surface.specular.w;
    let v = frame_normalize(frame.eye.xyz - in.world_pos, n);
    var sum = frame_ambient(n, m.rgb);
    var spec = vec3<f32>(0.0);
    let s = frame_shadow_factor(in.shadow, in.fog_depth.y);
    let sun = frame_sun(n, v, gloss, power, s);
    // Ms is the specular colour times the same per-object tint as M (material.md §8.3).
    let ms = surface.specular.rgb * in.tint.rgb;
    sum = sum + sun.diffuse * m.rgb;
    spec = spec + sun.specular * ms;
    let count = min(in.lights.z, 8u);
    for (var i = 0u; i < count; i = i + 1u) {
        let idx = light_index(in.lights, i);
        if (idx < frame.params.x) {
            let t = frame_light(idx, in.world_pos, n, v, gloss, power);
            sum = sum + t.diffuse * m.rgb;
            spec = spec + t.specular * ms;
        }
    }
    out.unfogged = 2.0 * c * sum + spec;
    out.color = frame_apply_fog(out.unfogged, out.fog);
    return out;
}

@fragment
fn fs_opaque(in: InstOut) -> @location(0) vec4<f32> {
    let s = shade(in);
    return vec4<f32>(frame_output(s.color), 1.0);
}

@fragment
fn fs_mask(in: InstOut) -> @location(0) vec4<f32> {
    let s = shade(in);
    if (surface_alpha_test(s.opacity)) {
        discard;
    }
    return vec4<f32>(frame_output(s.color), 1.0);
}

@fragment
fn fs_test_blend(in: InstOut) -> @location(0) vec4<f32> {
    let s = shade(in);
    if (surface_alpha_test(s.opacity)) {
        discard;
    }
    return vec4<f32>(frame_output(s.color), clamp(s.alpha, 0.0, 1.0));
}

// Blended kinds: the surface threshold is -1 except for a fixed blend with an alpha test
// (render.md §4.3 `Fixed`), so only those discard.
@fragment
fn fs_blend(in: InstOut) -> @location(0) vec4<f32> {
    let s = shade(in);
    if (surface_alpha_test(s.opacity)) {
        discard;
    }
    return frame_blend_output(frame_output(s.color), clamp(s.alpha, 0.0, 1.0));
}

@fragment
fn fs_additive(in: InstOut) -> @location(0) vec4<f32> {
    let s = shade(in);
    if (surface_alpha_test(s.opacity)) {
        discard;
    }
    return vec4<f32>(frame_output(s.unfogged * (1.0 - s.fog)), clamp(s.alpha, 0.0, 1.0));
}

// Modulating fixed blends: fog fades the colour toward white (render.md §4.3).
@fragment
fn fs_modulate(in: InstOut) -> @location(0) vec4<f32> {
    let s = shade(in);
    if (surface_alpha_test(s.opacity)) {
        discard;
    }
    return vec4<f32>(frame_output(mix(s.unfogged, vec3<f32>(1.0), s.fog)), clamp(s.alpha, 0.0, 1.0));
}

struct ShadowOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_shadow(v: ModelIn, inst: InstIn) -> ShadowOut {
    let pl = place(v, inst);
    var out: ShadowOut;
    out.clip = frame.shadow_view_proj * vec4<f32>(pl.world, 1.0);
    out.uv = v.uv;
    return out;
}

// Cut-outs cast cut-out shadows (lighting.md §7, code 0x0A).
@fragment
fn fs_shadow_mask(in: ShadowOut) {
    let a = surface_opacity_texel(surface_base_at(inst_uv(0u, in.uv), inst_uv(1u, in.uv)));
    if (surface_alpha_test(a)) {
        discard;
    }
}
