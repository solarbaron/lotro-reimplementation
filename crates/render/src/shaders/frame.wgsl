// SPDX-License-Identifier: GPL-3.0-or-later
// Shared per-pass lighting, fog and shadow uniform of the lit pipelines (M4.5).
// Written for Tessera; layout documented in docs/specs/render.md §4.2 and mirrored by
// `FrameUniform` in crates/render/src/lighting.rs (keep both in sync).
//
// Every pipeline that draws into a lit world pass (meshes, terrain layers, water, sky,
// instanced scenery) prepends this file and binds the pass's frame bind group as group 0.
// All colours are in the original's stored-value space (no sRGB decoding, lighting.md §5.1).

// One entry of the pass's light table.
struct FrameLight {
    // Point light: xyz = world position, w = attenuation factor k.
    // Directional light: xyz = unit vector towards the light, w = 0.
    position_k: vec4<f32>,
    // rgb = diffuse colour (before the material colour), w = squared range R^2 (point).
    color_r2: vec4<f32>,
    // rgb = specular colour (before the material specular), w = kind (0 point, 1 directional).
    specular_kind: vec4<f32>,
};

struct Frame {
    // World -> clip, reversed depth (near plane at z = 1).
    view_proj: mat4x4<f32>,
    // xyz = camera position (world), w = time input in seconds (lighting.md §4.2 E20).
    eye: vec4<f32>,
    // xyz = world up U (unit), w unused.
    up: vec4<f32>,
    // xyz = unit vector towards the main directional light, w = 1 when present else 0.
    sun_dir: vec4<f32>,
    // rgb = main light colour E0 (before the material colour), w unused.
    sun_color: vec4<f32>,
    // rgb = main light specular colour (before the material specular), w unused.
    sun_specular: vec4<f32>,
    // rgb = frame ambient 1/2 (X + Y), w unused.
    ambient: vec4<f32>,
    // rgb = frame hemispheric 1/2 (X - Y), w unused.
    hemispheric: vec4<f32>,
    // rgb = fog colour, w = 1 when fog is on else 0.
    fog_color: vec4<f32>,
    // x = fog near, y = 1 / (far - near), z = fog far, w = far - near.
    fog: vec4<f32>,
    // World -> shadow-map clip (orthographic, depth 0..1, near = 0).
    shadow_view_proj: mat4x4<f32>,
    // x = quality (0 = no shadow, 1 = one tap, >= 2 = five taps), y = fade start,
    // z = fade scale, w = one shadow-map texel in texture coordinates.
    shadow: vec4<f32>,
    // rgb = landscape shadow colour, w = depth bias subtracted from the comparison depth.
    shadow_color: vec4<f32>,
    // rgb = shadow colour range, w unused.
    shadow_range: vec4<f32>,
    // x = number of valid entries in `lights`, y = flags (FRAME_SRGB_TARGET), z, w reserved.
    params: vec4<u32>,
    lights: array<FrameLight, 256>,
};

// The pass renders into an sRGB-encoded target: convert stored values before output.
const FRAME_SRGB_TARGET: u32 = 1u;
const FRAME_MAX_LIGHTS: u32 = 256u;

@group(0) @binding(0) var<uniform> frame: Frame;
