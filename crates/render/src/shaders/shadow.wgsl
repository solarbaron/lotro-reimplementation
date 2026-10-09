// SPDX-License-Identifier: GPL-3.0-or-later
// The sun's shadow-map pass (lighting.md §7 code 0x0A, §9 step 1): depth only, alpha-tested
// casters keep their cut-outs. Written for Tessera. Appended after frame.wgsl and
// lit_common.wgsl (group 0 here holds only the frame uniform: the shadow map is the
// attachment of this pass).

struct ShadowVsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn shadow_vertex(in: VsIn, position: vec3<f32>) -> ShadowVsOut {
    let world = draw.model * vec4<f32>(position, 1.0);
    var out: ShadowVsOut;
    out.clip = frame.shadow_view_proj * world;
    out.uv = in.uv;
    return out;
}

@vertex
fn vs_shadow(in: VsIn) -> ShadowVsOut {
    return shadow_vertex(in, in.position);
}

@vertex
fn vs_shadow_skinned(in: VsIn, skin: SkinIn) -> ShadowVsOut {
    return shadow_vertex(in, skin_point(skin, in.position));
}

@fragment
fn fs_shadow_mask(in: ShadowVsOut) {
    let uv0 = lit_uv0(in.uv);
    let uv1 = lit_uv1(in.uv);
    var uv_em = uv0;
    if (lit_flag(LIT_EMISSIVE_UV1)) {
        uv_em = uv1;
    }
    let base = lit_layered(textureSample(t_base, s_base, uv0),
        textureSample(t_layer2, s_layer2, uv1));
    let emissive = textureSample(t_emissive, s_emissive, uv_em);
    let opacity = textureSample(t_opacity, s_opacity, uv0);
    // Alpha is never sRGB-encoded, so the samples need no conversion here.
    if (lit_opacity(base, emissive, opacity) <= draw.params.x) {
        discard;
    }
}
