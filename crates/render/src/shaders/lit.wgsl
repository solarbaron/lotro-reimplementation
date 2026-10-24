// SPDX-License-Identifier: GPL-3.0-or-later
// The lit mesh pipeline (M4.5): the lit base pass of docs/specs/lighting.md §5 with all
// lights in one pass (§2.1), the unlit pass (§7, code 0x00) and the blend variants.
// Written for Tessera. Appended after frame.wgsl, frame_shading.wgsl and lit_common.wgsl.

struct LitVsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) color: vec4<f32>,
    // Shadow-map coordinates (u, v, depth), lighting.md §9 step 2.
    @location(5) shadow: vec3<f32>,
    // x = fog factor (per vertex, §10), y = view depth (clip w, §9 step 6).
    @location(6) fog_depth: vec2<f32>,
    // The stored binormal after the transform (zero when the mesh has none).
    @location(7) binormal: vec3<f32>,
};

fn normal_matrix() -> mat3x3<f32> {
    return mat3x3<f32>(draw.normal0.xyz, draw.normal1.xyz, draw.normal2.xyz);
}

fn lit_vertex(in: VsIn, position: vec3<f32>, normal: vec3<f32>, tangent: vec3<f32>,
    binormal: vec3<f32>) -> LitVsOut {
    let world = draw.model * vec4<f32>(position, 1.0);
    let m3 = mat3x3<f32>(draw.model[0].xyz, draw.model[1].xyz, draw.model[2].xyz);
    var sign = 1.0;
    if ((draw.flags.x & F_MIRRORED) != 0u) {
        sign = -1.0;
    }
    var out: LitVsOut;
    out.clip = frame.view_proj * world;
    out.world_pos = world.xyz;
    out.normal = normal_matrix() * normal;
    out.tangent = vec4<f32>(m3 * tangent, in.tangent.w * sign);
    // The stored binormal is a direction like the tangent: a mirrored transform mirrors
    // it with the texture, so it needs no sign flip.
    out.binormal = m3 * binormal;
    out.uv = in.uv;
    out.color = in.color;
    out.shadow = frame_shadow_coords(world.xyz);
    out.fog_depth = vec2<f32>(frame_fog_factor(world.xyz), out.clip.w);
    return out;
}

@vertex
fn vs_lit(in: VsIn) -> LitVsOut {
    return lit_vertex(in, in.position, in.normal.xyz, in.tangent.xyz, in.binormal.xyz);
}

@vertex
fn vs_lit_skinned(in: VsIn, skin: SkinIn) -> LitVsOut {
    return lit_vertex(in, skin_point(skin, in.position), skin_dir(skin, in.normal.xyz),
        skin_dir(skin, in.tangent.xyz), skin_dir(skin, in.binormal.xyz));
}

fn light_index(i: u32) -> u32 {
    let word = select(draw.lit_lights.x, draw.lit_lights.y, i >= 4u);
    return (word >> ((i & 3u) * 8u)) & 255u;
}

struct Shaded {
    // Colour in stored-value space, fog applied as for a base pass.
    color: vec3<f32>,
    // Output alpha (material.md §11.3, MA61; lighting.md §5.12, L41): the tested opacity x
    // M.alpha (x vertex alpha where read), 1 x M.alpha without an opacity-bearing role, the
    // same as the original's base and additive light passes; for a fixed blend the tested
    // opacity alone. Clamped by the fragment stages.
    alpha: f32,
    // The tested opacity for the alpha test (material.md §11.2; before vertex colours).
    opacity: f32,
    // Unfogged colour and fog factor (additive passes scale by 1 - f instead).
    unfogged: vec3<f32>,
    fog: f32,
};

fn lit_shade(in: LitVsOut) -> Shaded {
    // Sample everything up front (uniform control flow). Texture matrix 0 for every
    // texture but the second colour layer (matrix 1) and, where the record says so, the
    // emissive texture (material.md §8.4).
    let uv0 = lit_uv0(in.uv);
    let uv1 = lit_uv1(in.uv);
    var uv_em = uv0;
    if (lit_flag(LIT_EMISSIVE_UV1)) {
        uv_em = uv1;
    }
    let layer2 = lit_to_stored(textureSample(t_layer2, s_layer2, uv1), LIT_SRGB_LAYER2);
    let base = lit_layered(lit_to_stored(textureSample(t_base, s_base, uv0), LIT_SRGB_BASE),
        layer2);
    let normal_tex = lit_to_stored(textureSample(t_normal, s_normal, uv0), LIT_SRGB_NORMAL);
    let dye_tex = lit_to_stored(textureSample(t_dye, s_dye, uv0), LIT_SRGB_DYE);
    let gloss_tex = lit_to_stored(textureSample(t_gloss, s_gloss, uv0), LIT_SRGB_GLOSS);
    let emissive = lit_to_stored(textureSample(t_emissive, s_emissive, uv_em),
        LIT_SRGB_EMISSIVE);
    let glow = lit_to_stored(textureSample(t_glow, s_glow, uv0), LIT_SRGB_GLOW);
    let opacity_tex = lit_to_stored(textureSample(t_opacity, s_opacity, uv0),
        LIT_SRGB_OPACITY);
    let f = draw.flags.x;

    var out: Shaded;
    out.opacity = lit_opacity(base, emissive, opacity_tex);

    // Material colour M, multiplied by the vertex colour where the record reads it (§5.13).
    var m = draw.base_color;
    if (lit_flag(LIT_VERTEX_COLORS)) {
        m = m * in.color;
    }
    // The tested opacity reaches the output alpha whatever its source (material.md §11.3,
    // revised 2026-10-10, MA60).
    out.alpha = out.opacity * m.a;
    // A fixed blend's factors read the §11.2 tested opacity, or 1 without an
    // opacity-bearing role (material.md §11.3, rule for unlit records).
    if (lit_flag(LIT_FACTOR_OPACITY)) {
        out.alpha = out.opacity;
    }

    // Albedo C after dye (§5.2 items 2-3; renderer mask convention, 1 = dyed).
    var c = base.rgb;
    if ((f & F_DYE) != 0u) {
        var mask = dye_tex.r;
        if (draw.flags.z == DYE_BASE_ALPHA) {
            mask = base.a;
        }
        c = c * mix(vec3<f32>(1.0), draw.dye.rgb, clamp(mask, 0.0, 1.0));
    }

    var fog = 0.0;
    if (lit_flag(LIT_FOG)) {
        fog = clamp(in.fog_depth.x, 0.0, 1.0);
    }
    out.fog = fog;

    if ((f & F_UNLIT) != 0u) {
        // Unlit pass (§7, code 0x00): texture x material colour.
        out.unfogged = c * m.rgb;
        out.color = frame_apply_fog(out.unfogged, fog);
        return out;
    }

    // Shading normal (§5.2.4). Back faces keep the front normal (material.md §11.4).
    var n = frame_normalize(in.normal, vec3<f32>(0.0, 0.0, 1.0));
    if ((f & F_NORMAL_MAP) != 0u) {
        // The tangent as given (normalised, not made orthogonal to N0) and the stored
        // binormal, or the derived one without it (frame_binormal).
        let b = frame_binormal(n, in.tangent, in.binormal);
        if (dot(b, b) > 0.0) {
            let t = normalize(in.tangent.xyz);
            var tn = normal_tex.xyz * 2.0 - 1.0;
            tn.y = tn.y * draw.params.y;
            n = frame_normalize(t * tn.x + b * tn.y + n * tn.z, n);
        }
    }

    // Gloss g (§5.2 item 5).
    var gloss = 1.0;
    switch (lit_source(LIT_GLOSS_SHIFT)) {
        case 1u: { gloss = base.a; }
        case 2u: { gloss = normal_tex.a; }
        case 3u: { gloss = gloss_tex.a; }
        default: {}
    }
    var power = -1.0;
    if (lit_flag(LIT_SPECULAR)) {
        power = draw.lit.w;
    }
    let v = frame_normalize(frame.eye.xyz - in.world_pos, n);

    // Light sum (§5.3-§5.7): ambient, the sun with its shadow, the listed lights.
    var sum = frame_ambient(n, m.rgb);
    var spec = vec3<f32>(0.0);
    let s = frame_shadow_factor(in.shadow, in.fog_depth.y);
    let sun = frame_sun(n, v, gloss, power, s);
    sum = sum + sun.diffuse * m.rgb;
    spec = spec + sun.specular * draw.specular.rgb;
    let count = min(draw.lit_lights.z, 8u);
    for (var i = 0u; i < count; i = i + 1u) {
        let idx = light_index(i);
        if (idx < frame.params.x) {
            let t = frame_light(idx, in.world_pos, n, v, gloss, power);
            sum = sum + t.diffuse * m.rgb;
            spec = spec + t.specular * draw.specular.rgb;
        }
    }

    var color = 2.0 * c * sum + spec;
    // Emissive (§5.9) and glow (Tessera Decision: the glow mask x G added like emissive).
    if (lit_flag(LIT_EMISSIVE)) {
        color = color + emissive.rgb * draw.lit.rgb;
    }
    if (lit_flag(LIT_GLOW)) {
        color = color + glow.rgb * draw.lit.rgb;
    }
    out.unfogged = color;
    out.color = frame_apply_fog(color, fog);
    return out;
}

fn alpha_test(s: Shaded) {
    // Kept only when the tested opacity is greater than the reference R (material.md §11.2):
    // discarded when opacity <= R. R = -1 keeps everything, R = 0 drops opacity 0 only.
    if (frame_alpha_discards(s.opacity, draw.params.x)) {
        discard;
    }
}

@fragment
fn fs_lit_opaque(in: LitVsOut) -> @location(0) vec4<f32> {
    let s = lit_shade(in);
    return vec4<f32>(frame_output(s.color), 1.0);
}

@fragment
fn fs_lit_mask(in: LitVsOut) -> @location(0) vec4<f32> {
    let s = lit_shade(in);
    alpha_test(s);
    return vec4<f32>(frame_output(s.color), 1.0);
}

@fragment
fn fs_lit_test_blend(in: LitVsOut) -> @location(0) vec4<f32> {
    let s = lit_shade(in);
    alpha_test(s);
    return vec4<f32>(frame_output(s.color), clamp(s.alpha, 0.0, 1.0));
}

// Blended kinds: the threshold is -1 except for a fixed blend with an alpha test
// (render.md §4.3 `Fixed`, the caller's reference R), so only those discard.
@fragment
fn fs_lit_blend(in: LitVsOut) -> @location(0) vec4<f32> {
    let s = lit_shade(in);
    alpha_test(s);
    return frame_blend_output(frame_output(s.color), clamp(s.alpha, 0.0, 1.0));
}

// Additive (SRCALPHA / ONE): fogged distance fades the contribution out (§6, §10).
@fragment
fn fs_lit_additive(in: LitVsOut) -> @location(0) vec4<f32> {
    let s = lit_shade(in);
    alpha_test(s);
    return vec4<f32>(frame_output(s.unfogged * (1.0 - s.fog)), clamp(s.alpha, 0.0, 1.0));
}

// Modulating fixed blends (ZERO / SRCCOLOR, DESTCOLOR / ZERO, MIN): fogged distance fades
// the colour toward white, which leaves the destination unchanged (render.md §4.3).
@fragment
fn fs_lit_modulate(in: LitVsOut) -> @location(0) vec4<f32> {
    let s = lit_shade(in);
    alpha_test(s);
    return vec4<f32>(frame_output(mix(s.unfogged, vec3<f32>(1.0), s.fog)), clamp(s.alpha, 0.0, 1.0));
}
