// SPDX-License-Identifier: GPL-3.0-or-later
//! Mip-chain uploads (`docs/specs/texture-list.md` §5.5): every stored level lands in its
//! mip, the GPU picks the mip by on-screen size, a non-halving level is copied clipped into
//! its slot, and hostile chains are rejected or cut. Textures are generated here.

#[path = "support/gpu.rs"]
mod gpu;

use tessera_render::{
    ColorSpace, Filter, MipChainUpload, MipLevel, OffscreenFormat, PassSpace, Quad, RenderError,
    Renderer, TextureId, Wrap,
};

fn solid(size: u32, rgba: [u8; 4]) -> Vec<u8> {
    rgba.repeat((size * size) as usize)
}

/// `size × size` level whose top-left `half × half` quadrant is `a` and the rest `b`.
fn quadrant(size: u32, half: u32, a: [u8; 4], b: [u8; 4]) -> Vec<u8> {
    (0..size * size)
        .flat_map(|i| {
            if i % size < half && i / size < half {
                a
            } else {
                b
            }
        })
        .collect()
}

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

fn upload(r: &mut Renderer<'static>, levels: &[MipLevel<'_>]) -> Result<TextureId, RenderError> {
    r.upload_texture_mips(&MipChainUpload {
        color_space: ColorSpace::Linear,
        filter: Filter::Nearest,
        wrap: Wrap::Clamp,
        ..MipChainUpload::new(levels)
    })
}

/// Draw `tex` as screen quads of 64, 32 and 16 pixels and return each quad's centre pixel.
fn sizes(r: &mut Renderer<'static>, tex: TextureId) -> Result<[[u8; 4]; 3], RenderError> {
    let target = r.create_offscreen_target(128, 64, OffscreenFormat::Rgba8Unorm)?;
    let mut frame = r.begin_offscreen_frame(&target)?;
    frame
        .pass("mips", PassSpace::Screen)
        .clear([0.5, 0.5, 0.5, 1.0])
        .quad(Quad::screen_rect(0.0, 0.0, 64.0, 64.0).with_texture(tex))
        .quad(Quad::screen_rect(64.0, 0.0, 32.0, 32.0).with_texture(tex))
        .quad(Quad::screen_rect(96.0, 0.0, 16.0, 16.0).with_texture(tex));
    r.end_frame(frame)?;
    let img = r.read_pixels(&target)?;
    let px = |x, y| img.pixel(x, y).unwrap_or([0; 4]);
    Ok([px(32, 32), px(80, 16), px(104, 8)])
}

#[test]
fn the_gpu_picks_the_stored_mip_by_size() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let (a, b, c) = (solid(64, RED), solid(32, GREEN), solid(16, BLUE));
    let levels = [
        MipLevel {
            width: 64,
            height: 64,
            rgba: &a,
        },
        MipLevel {
            width: 32,
            height: 32,
            rgba: &b,
        },
        MipLevel {
            width: 16,
            height: 16,
            rgba: &c,
        },
    ];
    let tex = upload(&mut r, &levels)?;
    assert_eq!(r.texture_mip_levels(tex), Some(3));
    assert_eq!(r.texture_size(tex), Some((64, 64)));
    assert_eq!(sizes(&mut r, tex)?, [RED, GREEN, BLUE]);
    // Mip textures are not updated in place.
    assert!(matches!(
        r.update_texture(tex, &a),
        Err(RenderError::InvalidTexture(_))
    ));

    // A short chain (no level below 32): sampling smaller stays on the last level.
    let short = upload(&mut r, &levels[..2])?;
    assert_eq!(r.texture_mip_levels(short), Some(2));
    assert_eq!(sizes(&mut r, short)?, [RED, GREEN, GREEN]);
    Ok(())
}

#[test]
fn a_non_halving_level_is_copied_clipped() -> Result<(), RenderError> {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    // texture-list.md §4.2: two 64 × 64 levels. The second goes into the 32 × 32 slot,
    // clipped to its top-left part, which is all yellow.
    let a = solid(64, RED);
    let b = quadrant(64, 32, YELLOW, BLACK);
    let tex = upload(
        &mut r,
        &[
            MipLevel {
                width: 64,
                height: 64,
                rgba: &a,
            },
            MipLevel {
                width: 64,
                height: 64,
                rgba: &b,
            },
        ],
    )?;
    let [big, half, _] = sizes(&mut r, tex)?;
    assert_eq!((big, half), (RED, YELLOW));
    Ok(())
}

#[test]
fn hostile_chains_are_rejected() {
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return;
    };
    let a = solid(4, RED);
    let short = vec![0u8; 5];
    let bad = [
        MipLevel {
            width: 4,
            height: 4,
            rgba: &a,
        },
        MipLevel {
            width: 2,
            height: 2,
            rgba: &short,
        },
    ];
    assert!(matches!(
        upload(&mut r, &bad),
        Err(RenderError::InvalidTexture(_))
    ));
    assert!(matches!(
        upload(&mut r, &[]),
        Err(RenderError::InvalidTexture(_))
    ));
    let huge = MipLevel {
        width: u32::MAX,
        height: 1,
        rgba: &a,
    };
    assert!(matches!(
        upload(&mut r, &[huge]),
        Err(RenderError::InvalidTexture(_))
    ));
    // 1,000 levels of a 4 × 4 texture: cut at the full pyramid (3 levels).
    let one = solid(1, GREEN);
    let mut levels = vec![MipLevel {
        width: 4,
        height: 4,
        rgba: &a,
    }];
    levels.extend(std::iter::repeat_n(
        MipLevel {
            width: 1,
            height: 1,
            rgba: &one,
        },
        1000,
    ));
    let tex = upload(&mut r, &levels).expect("long chain is cut, not rejected");
    assert_eq!(r.texture_mip_levels(tex), Some(3));
}
