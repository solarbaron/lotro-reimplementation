// SPDX-License-Identifier: GPL-3.0-or-later
// The sky (M4.5; docs/specs/environment.md §12, docs/specs/render.md §5.4). Written for
// Tessera. Appended after frame.wgsl, frame_shading.wgsl and instanced_common.wgsl.
// Sky objects are drawn around the camera, unlit and unfogged (Tessera Decision), tinted
// by their keyframe tint, at the far plane.

struct SkyObject {
    // Columns of R_Y(-A) R_Z(-B) (environment.md §11.4 step 5) times the object's scale.
    c0: vec4<f32>,
    c1: vec4<f32>,
    c2: vec4<f32>,
    // Keyframe tint; a = opacity.
    tint: vec4<f32>,
};

@group(1) @binding(0) var<uniform> sky: SkyObject;

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_sky(v: ModelIn) -> SkyOut {
    let r = mat3x3<f32>(sky.c0.xyz, sky.c1.xyz, sky.c2.xyz);
    let world = frame.eye.xyz + r * v.position;
    var out: SkyOut;
    let clip = frame.view_proj * vec4<f32>(world, 1.0);
    // The far plane under reversed depth.
    out.clip = vec4<f32>(clip.xy, 0.0, clip.w);
    out.uv = v.uv;
    out.color = v.color;
    return out;
}

struct SkyShaded {
    color: vec3<f32>,
    alpha: f32,
    opacity: f32,
};

fn sky_shade(in: SkyOut) -> SkyShaded {
    let base = surface_base(in.uv);
    var m = surface.color * sky.tint;
    if (surface_flag(SF_VERTEX_COLORS)) {
        m = m * in.color;
    }
    var out: SkyShaded;
    out.color = base.rgb * m.rgb;
    out.opacity = surface_opacity_texel(base);
    // A fixed blend reads the opacity source, not x M.alpha (material.md §11.3); the
    // keyframe opacity (tint alpha, Render_Opacity) still scales it.
    let a = select(surface_opacity(base) * m.a, out.opacity * sky.tint.a,
        surface_flag(SF_FACTOR_OPACITY));
    out.alpha = clamp(a, 0.0, 1.0);
    return out;
}

@fragment
fn fs_sky(in: SkyOut) -> @location(0) vec4<f32> {
    let s = sky_shade(in);
    return vec4<f32>(frame_output(s.color), s.alpha);
}

@fragment
fn fs_sky_mask(in: SkyOut) -> @location(0) vec4<f32> {
    let s = sky_shade(in);
    if (surface_alpha_test(s.opacity)) {
        discard;
    }
    return vec4<f32>(frame_output(s.color), s.alpha);
}
