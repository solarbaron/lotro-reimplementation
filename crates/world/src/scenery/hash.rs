// SPDX-License-Identifier: GPL-3.0-or-later
//! The placement hashes and seeds of `scenery.md` §6.
//!
//! All arithmetic wraps modulo 2³². The multipliers, offsets and salts are constants of the
//! original client's placement hash; decision 0012 (`docs/decisions/0012-numeric-constants.md`)
//! allows them in Tessera's code, citing `scenery.md` §6.

/// Seed B offset: B = 17·i + j + this (`scenery.md` §6.3).
pub const SEED_B_OFFSET: u32 = 0x0035_56F7;
/// The scene-seed multiplier, and the top-level incoming seed of a vertex (§6.3).
pub const SCENE_SEED: u32 = 0x0009_23B1;
/// Step between the seeds of consecutive children (§6.3).
pub const CHILD_SEED_STEP: u32 = 0x2D41_BF19;
/// Added to the scene seed S for the frill layer (§6.3).
pub const FRILL_LAYER_SALT: u32 = 0x0380_430A;

/// Salt of the frequency test (§6.2).
pub const SALT_FREQUENCY: u32 = 0x0119_1E67;
/// Salt of displacement x and of the height offset of §7.4 step 11 (§6.2).
pub const SALT_DISP_X: u32 = 0x28D0_642D;
/// Salt of displacement y (§6.2).
pub const SALT_DISP_Y: u32 = 0x062A_DE4F;
/// Salt of displacement z, used only with `height_mode` 9 (§6.2).
pub const SALT_DISP_Z: u32 = 0x0071_D36D;
/// Salt of the heading (§6.2).
pub const SALT_HEADING: u32 = 0x03D6_0BCB;
/// Salt of the second angle (§6.2).
pub const SALT_ANGLE2: u32 = 0x0298_6B4B;
/// Salt of the third angle (§6.2).
pub const SALT_ANGLE3: u32 = 0x0090_B46B;
/// Salt of the uniform scale, `scale_mode` 4 (§6.2).
pub const SALT_SCALE: u32 = 0x0051_5E71;

/// A hash value turned into a fraction: (v ≫ 16) / 65,536, so 0 ≤ u < 1 (§6). Exact in
/// f32.
#[must_use]
pub fn fraction(v: u32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let hi = (v >> 16) as f32;
    hi / 65_536.0
}

/// The raw vertex scene-pick hash of §6.1 for global vertex coordinates gx = 16·X + i and
/// gy = 16·Y + j.
#[must_use]
pub fn vertex_hash(gx: u32, gy: u32) -> u32 {
    gx.wrapping_mul(gx)
        .wrapping_mul(gy)
        .wrapping_mul(0x019A_266F)
        .wrapping_sub(0x4A65_06DB)
        .wrapping_mul(gy)
        .wrapping_sub(gx.wrapping_mul(0x6B3C_5F11))
        .wrapping_sub(0x39B6_25FB)
}

/// The fraction u of the vertex scene pick (§6.1).
#[must_use]
pub fn vertex_fraction(gx: u32, gy: u32) -> f32 {
    fraction(vertex_hash(gx, gy))
}

/// The raw node hash of §6.2 for seeds A, B and s (s = C plus a salt).
#[must_use]
pub fn node_hash(a: u32, b: u32, s: u32) -> u32 {
    let a2b2 = a.wrapping_mul(a).wrapping_mul(b).wrapping_mul(b);
    0x6243_3D7D_u32
        .wrapping_sub(0x2BD3_8901_u32.wrapping_mul(s).wrapping_mul(a2b2))
        .wrapping_mul(s)
        .wrapping_sub(0x6B3C_5F11_u32.wrapping_mul(a))
        .wrapping_sub(0x4A65_06DB_u32.wrapping_mul(b))
}

/// R(A, B, s) of §6.2: the fraction of [`node_hash`].
#[must_use]
pub fn node_random(a: u32, b: u32, s: u32) -> f32 {
    fraction(node_hash(a, b, s))
}

/// Seed A of an outdoor landblock: X + Y + R + 5 (§6.3, Tessera Decision for the
/// landblock-info list that would switch it to 1).
#[must_use]
pub fn seed_a(region: u8, x: u8, y: u8) -> u32 {
    u32::from(x) + u32::from(y) + u32::from(region) + 5
}

/// Seed B of coarse vertex (i, j): 17·i + j + 0x003556F7 (§6.3).
#[must_use]
pub fn seed_b(i: u32, j: u32) -> u32 {
    i.wrapping_mul(17)
        .wrapping_add(j)
        .wrapping_add(SEED_B_OFFSET)
}

/// The scene seed S of a scene instantiated with incoming seed `c`: c · 0x000923B1 (§6.3).
#[must_use]
pub fn scene_seed(c: u32) -> u32 {
    c.wrapping_mul(SCENE_SEED)
}

/// The seed of child `k` of a group or node with seed `base` (§6.3).
#[must_use]
pub fn child_seed(base: u32, k: u32) -> u32 {
    base.wrapping_add(k.wrapping_mul(CHILD_SEED_STEP))
}
