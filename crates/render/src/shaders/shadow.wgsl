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
    let base = textureSample(t_base, s_base, in.uv);
    let emissive = textureSample(t_emissive, s_emissive, in.uv);
    let opacity = textureSample(t_opacity, s_opacity, in.uv);
    // Alpha is never sRGB-encoded, so the samples need no conversion here.
    if (lit_opacity(base, emissive, opacity) <= draw.params.x) {
        discard;
    }
}
