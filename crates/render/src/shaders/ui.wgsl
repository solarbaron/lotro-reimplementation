// SPDX-License-Identifier: GPL-3.0-or-later
// UI composition (docs/specs/render.md §6.7): one quad per entry, texels read
// with textureLoad only, premultiplied filtering done here, clip chain tested
// per fragment, premultiplied source-over blend (UI_BLEND_* in ui/mod.rs).
//
// Every map is affine, so the vertex stage passes the unrotated box position
// q and each clip item's tested position. For an entry with no rotation the
// screen is the root's unrotated space, and the fragment uses the exact pixel
// centre instead of the interpolated q; Nearest then picks texel floor(c) by
// integer arithmetic (Fit) or with an exact correction step (Tile), so a c
// that lands on an integer never reads the previous texel.

const FLAG_TILE: u32 = 1u;
const FLAG_NEAREST: u32 = 2u;
const FLAG_PREMULTIPLIED: u32 = 4u;
const FLAG_ONE_TO_ONE: u32 = 8u;
const FLAG_LINEARIZE: u32 = 16u;
const FLAG_UNROTATED: u32 = 32u;
// An untextured fill (UiFill): the premultiplied colour is in tile_scale.
const FLAG_FILL: u32 = 64u;

// Must match `EntryUniform` in ui/state.rs (496 bytes).
struct Entry {
    // Clip-space corners: (x0, y0, x1, y1) of corners 0, 1 and of corners 2, 3;
    // corner i is (i & 1, i >> 1) of the displayed box.
    ndc01: vec4<f32>,
    ndc23: vec4<f32>,
    // Displayed box (x, y, w, h).
    dest: vec4<f32>,
    // Box origin (x, y), canvas size (cw, ch).
    idest: vec4<i32>,
    // Texture offset (ox, oy), flags, number of clip items.
    misc: vec4<i32>,
    // Canvas texels per box pixel (x, y), opacity, alpha threshold.
    params: vec4<f32>,
    // Tile node scale (sx, sy), unused (Fit: zero); for a fill, its premultiplied colour.
    tile_scale: vec4<f32>,
    // Clip rectangles (x0, y0, x1, y1), half-open.
    clip_rects: array<vec4<f32>, 8>,
    // Two rows per item: tested position = (dot(row0.xyz, (q, 1)), dot(row1.xyz, (q, 1))).
    clip_maps: array<vec4<f32>, 16>,
};

@group(0) @binding(0) var<uniform> e: Entry;
@group(1) @binding(0) var t_canvas: texture_2d<f32>;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) q: vec2<f32>,
    @location(1) c01: vec4<f32>,
    @location(2) c23: vec4<f32>,
    @location(3) c45: vec4<f32>,
    @location(4) c67: vec4<f32>,
};

fn clip_pos(i: u32, q: vec2<f32>) -> vec2<f32> {
    let p = vec3<f32>(q, 1.0);
    return vec2<f32>(dot(e.clip_maps[2u * i].xyz, p), dot(e.clip_maps[2u * i + 1u].xyz, p));
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    var ndc: vec2<f32>;
    switch vi {
        case 0u: { ndc = e.ndc01.xy; }
        case 1u: { ndc = e.ndc01.zw; }
        case 2u: { ndc = e.ndc23.xy; }
        default: { ndc = e.ndc23.zw; }
    }
    let q = e.dest.xy + corner * e.dest.zw;
    var out: VsOut;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.q = q;
    out.c01 = vec4<f32>(clip_pos(0u, q), clip_pos(1u, q));
    out.c23 = vec4<f32>(clip_pos(2u, q), clip_pos(3u, q));
    out.c45 = vec4<f32>(clip_pos(4u, q), clip_pos(5u, q));
    out.c67 = vec4<f32>(clip_pos(6u, q), clip_pos(7u, q));
    return out;
}

fn outside(i: i32, p: vec2<f32>) -> bool {
    if i >= e.misc.w {
        return false;
    }
    let r = e.clip_rects[i];
    return !(p.x >= r.x && p.x < r.z && p.y >= r.y && p.y < r.w);
}

// Clamp (Fit) or wrap (Tile) a canvas texel position.
fn address(c: vec2<i32>) -> vec2<i32> {
    let size = e.idest.zw;
    if (u32(e.misc.z) & FLAG_TILE) != 0u {
        return ((c % size) + size) % size;
    }
    return clamp(c, vec2<i32>(0, 0), size - vec2<i32>(1, 1));
}

// Premultiplied texel at an addressed canvas position; transparent black
// outside the stored part.
fn fetch(c: vec2<i32>) -> vec4<f32> {
    let t = c - e.misc.xy;
    let dim = vec2<i32>(textureDimensions(t_canvas));
    if any(t < vec2<i32>(0, 0)) || any(t >= dim) {
        return vec4<f32>(0.0);
    }
    let texel = textureLoad(t_canvas, t, 0);
    if (u32(e.misc.z) & FLAG_PREMULTIPLIED) != 0u {
        return texel;
    }
    return vec4<f32>(texel.rgb * texel.a, texel.a);
}

// Fit, Nearest, no rotation: canvas texel floor((k + 1/2) * cw / n) of box
// pixel k (0 <= k < n <= 2^24, cw <= 16,384) computed exactly as
// floor((2k + 1) * cw / (2n)). The product needs up to 39 bits, so it is
// divided by long division in base 64: the remainder stays below 2n <= 2^25,
// and remainder * 64 + digit * cw below 2^32.
fn fit_texel(k: i32, n: u32, cw: u32) -> i32 {
    let a = 2u * u32(clamp(k, 0, i32(n) - 1)) + 1u;
    let d = 2u * n;
    var q = 0u;
    var r = 0u;
    for (var shift = 24i; shift >= 0; shift = shift - 6) {
        let t = r * 64u + ((a >> u32(shift)) & 63u) * cw;
        q = q * 64u + t / d;
        r = t % d;
    }
    return i32(q);
}

// Tile, Nearest, no rotation: floor((k + 1/2) / s), from the estimate with the
// reciprocal r = 1/s corrected by one step each way (products are correctly
// rounded, the quotient need not be).
fn tile_texel(k: i32, s: f32, r: f32) -> f32 {
    let p = f32(k) + 0.5;
    var i = floor(p * r);
    if (i + 1.0) * s <= p {
        i = i + 1.0;
    } else if i * s > p {
        i = i - 1.0;
    }
    return i;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if outside(0, in.c01.xy) || outside(1, in.c01.zw) || outside(2, in.c23.xy)
        || outside(3, in.c23.zw) || outside(4, in.c45.xy) || outside(5, in.c45.zw)
        || outside(6, in.c67.xy) || outside(7, in.c67.zw) {
        discard;
    }
    let flags = u32(e.misc.z);
    var s: vec4<f32>;
    if (flags & FLAG_FILL) != 0u {
        s = e.tile_scale;
    } else if (flags & FLAG_ONE_TO_ONE) != 0u {
        let pix = vec2<i32>(floor(in.position.xy));
        s = fetch(address(pix - e.idest.xy));
    } else if (flags & (FLAG_UNROTATED | FLAG_NEAREST)) == (FLAG_UNROTATED | FLAG_NEAREST) {
        let k = vec2<i32>(floor(in.position.xy)) - e.idest.xy;
        var t: vec2<i32>;
        if (flags & FLAG_TILE) != 0u {
            let size = vec2<f32>(e.idest.zw);
            let i = vec2<f32>(
                tile_texel(k.x, e.tile_scale.x, e.params.x),
                tile_texel(k.y, e.tile_scale.y, e.params.y),
            );
            // Integer-valued: exact while below 2^24; `address` wraps again.
            t = vec2<i32>(i - floor(i / size) * size);
        } else {
            t = vec2<i32>(
                fit_texel(k.x, u32(e.dest.z), u32(e.idest.z)),
                fit_texel(k.y, u32(e.dest.w), u32(e.idest.w)),
            );
        }
        s = fetch(address(t));
    } else {
        let size = vec2<f32>(e.idest.zw);
        var q = in.q;
        if (flags & FLAG_UNROTATED) != 0u {
            q = in.position.xy;
        }
        var c = (q - e.dest.xy) * e.params.xy;
        if (flags & FLAG_TILE) != 0u {
            c = c - floor(c / size) * size;
        }
        if (flags & FLAG_NEAREST) != 0u {
            s = fetch(address(vec2<i32>(floor(c))));
        } else {
            let u = c - vec2<f32>(0.5);
            let f0 = floor(u);
            let f = u - f0;
            let i0 = vec2<i32>(f0);
            let a = fetch(address(i0));
            let b = fetch(address(i0 + vec2<i32>(1, 0)));
            let cc = fetch(address(i0 + vec2<i32>(0, 1)));
            let d = fetch(address(i0 + vec2<i32>(1, 1)));
            s = mix(mix(a, b, f.x), mix(cc, d, f.x), f.y);
        }
    }
    // Threshold on the texel alpha, before opacity (render.md §6.7 step 4).
    if s.a < e.params.w {
        discard;
    }
    s = s * e.params.z;
    // Fallback only (render.md §6.4): divide out alpha, decode, multiply alpha back in;
    // alpha is not converted, and alpha 0 gives 0 (also for premultiplied external
    // content whose colour is not 0 there).
    if (flags & FLAG_LINEARIZE) != 0u {
        if s.a > 0.0 {
            s = vec4<f32>(srgb_to_linear(clamp(s.rgb / s.a, vec3<f32>(0.0), vec3<f32>(1.0))) * s.a, s.a);
        } else {
            s = vec4<f32>(0.0);
        }
    }
    return s;
}
