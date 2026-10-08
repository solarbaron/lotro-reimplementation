// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data check of mip-chain uploads (`docs/specs/texture-list.md` §5.4–§5.5): texture
//! lists of the user's install are resolved to their active chain at the default texture
//! detail (3) without the highres tier, every level is decoded and the chain is uploaded
//! with `Renderer::upload_texture_mips`. Skipped unless `TESSERA_GAME_DIR` is set; the
//! install is opened read-only and nothing derived from it is written anywhere. Only counts
//! are printed.

#[path = "support/gpu.rs"]
mod gpu;

use tessera_dat::{ArchiveSet, SetOptions};
use tessera_formats::{Record, Surface, TextureList, TextureListKind};
use tessera_render::{MipChainUpload, MipLevel, RenderError};

const LISTS: std::ops::Range<u32> = 0x4000_0000..0x4100_0000;
/// Mip chains to upload.
const WANT: usize = 200;
/// Directory entries examined at most.
const MAX_ATTEMPTS: usize = 20_000;
/// Skip chains whose active level 0 has more texels than this (keeps the test quick).
const MAX_TEXELS: u64 = 1024 * 1024;

#[test]
fn real_mip_chains_upload() -> Result<(), RenderError> {
    let Some(dir) = std::env::var_os("TESSERA_GAME_DIR") else {
        eprintln!("skipping: TESSERA_GAME_DIR is not set");
        return Ok(());
    };
    let Some(mut r) = gpu::headless_renderer([0.0, 0.0, 0.0, 1.0]) else {
        return Ok(());
    };
    let set = ArchiveSet::open(&dir, SetOptions::default()).expect("open archive set");
    let routed = set.candidates(LISTS.start);
    let order: Vec<usize> = routed
        .iter()
        .copied()
        .chain((0..set.members().len()).filter(|i| !routed.contains(i)))
        .collect();
    let (mut attempts, mut uploaded, mut levels_total, mut multi) = (0usize, 0usize, 0usize, 0);
    let (mut missing, mut too_big, mut not_halving) = (0usize, 0usize, 0usize);
    'outer: for i in order {
        let Some(m) = set.get(i) else { continue };
        for entry in m.archive.entries_range(LISTS) {
            let Ok(entry) = entry else { break };
            attempts += 1;
            if attempts > MAX_ATTEMPTS || uploaded >= WANT {
                break 'outer;
            }
            let Ok(bytes) = m.archive.read_entry(&entry) else {
                continue;
            };
            let list = TextureList::parse_expecting(&bytes, entry.did).expect("texture list");
            if list.kind != TextureListKind::MipChain {
                continue;
            }
            // texture-list.md §5.4 at detail 3 (drop one level unless flags bit 0 or n ≤ 1).
            let n = list.surfaces.len();
            let drop = if list.flags & 1 == 1 || n <= 1 { 0 } else { 1 };
            let active = TextureList {
                surfaces: list.surfaces[drop..].to_vec(),
                ..list
            };
            let chain = active.effective_chain(|did| {
                let bytes = set.read(did.0).ok()?;
                Surface::parse_expecting(&bytes, did.0)
                    .and_then(|s| s.to_rgba8())
                    .ok()
            });
            let Some(first) = chain.first() else {
                missing += 1;
                continue;
            };
            if u64::from(first.level.width) * u64::from(first.level.height) > MAX_TEXELS {
                too_big += 1;
                continue;
            }
            let (w0, h0) = (first.level.width, first.level.height);
            not_halving += chain
                .iter()
                .enumerate()
                .filter(|(k, l)| {
                    (l.level.width, l.level.height) != ((w0 >> k).max(1), (h0 >> k).max(1))
                })
                .count();
            let levels: Vec<MipLevel<'_>> = chain
                .iter()
                .map(|l| MipLevel {
                    width: l.level.width,
                    height: l.level.height,
                    rgba: &l.level.pixels,
                })
                .collect();
            let tex = r.upload_texture_mips(&MipChainUpload::new(&levels))?;
            let full = 32 - w0.max(h0).leading_zeros();
            assert_eq!(
                r.texture_mip_levels(tex),
                Some(u32::try_from(levels.len()).unwrap_or(u32::MAX).min(full))
            );
            if levels.len() > 1 {
                multi += 1;
            }
            levels_total += levels.len();
            uploaded += 1;
            r.destroy_texture(tex);
        }
    }
    eprintln!(
        "mip chains: {uploaded} uploaded ({multi} with several levels, {levels_total} levels), \
         {not_halving} non-halving levels, {missing} without a resolving level, {too_big} \
         skipped as large, {attempts} entries examined"
    );
    assert!(uploaded > 0, "no texture list could be uploaded");
    assert!(multi > 0, "no multi-level chain found");
    Ok(())
}
