// SPDX-License-Identifier: GPL-3.0-or-later
//! Writes synthetic seed corpora for the `havok_*` fuzz targets, built by the testkit only.
//!
//! Does nothing unless `TESSERA_HAVOK_SEED_DIR` names a directory (normally `fuzz/corpus`,
//! which is git-ignored): `TESSERA_HAVOK_SEED_DIR=fuzz/corpus cargo test -p tessera-havok
//! --test fuzz_seeds`. Each file starts with the target's mode byte (see the targets' docs).
//! The test also checks that every raw-mode seed decodes.

use std::path::PathBuf;

use tessera_havok::testkit::{
    AnimationKindSpec, AnimationSpec, ClipRecordSpec, SkeletonSpec, encode_block,
    example_interleaved_12_6, example_record_12_8, example_reference_frame_12_7,
    example_rich_record, example_skeleton_12_1, example_spline_12_3,
};
use tessera_havok::{Clip, Skeleton, Tagfile};

fn with_mode(mode: u8, body: &[u8]) -> Vec<u8> {
    let mut v = vec![mode];
    v.extend_from_slice(body);
    v
}

fn seeds() -> Vec<(&'static str, Vec<u8>)> {
    let spline = ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Spline(
        example_spline_12_3(),
    )));
    let interleaved = ClipRecordSpec::new(AnimationSpec::new(AnimationKindSpec::Interleaved(
        example_interleaved_12_6(),
    )));
    let rich = example_rich_record();
    let tagfiles = [
        example_skeleton_12_1(),
        SkeletonSpec::chain(5).to_tagfile(),
        spline.animation.to_tagfile(),
        interleaved.animation.to_tagfile(),
        rich.animation.to_tagfile(),
        example_reference_frame_12_7(3).to_tagfile(),
    ];
    let mut out = Vec::new();
    for t in &tagfiles {
        assert!(Tagfile::parse(t).is_ok());
        out.push(("havok_tagfile", with_mode(0, t)));
    }
    for k in 0..6u8 {
        out.push(("havok_tagfile", vec![1, k, 3, 1, 2, 3, 4]));
    }
    out.push((
        "havok_tagfile",
        vec![2, 1, 0, 1, 2, 1, 25, 2, 3, 0, 1, 1, 1, 1, 7, 1],
    ));

    for t in [
        example_skeleton_12_1(),
        SkeletonSpec::chain(12).to_tagfile(),
    ] {
        assert!(Skeleton::parse(&t).is_ok());
        out.push(("havok_skeleton", with_mode(0, &t)));
    }
    for k in [3u8, 20, 39] {
        out.push((
            "havok_skeleton",
            vec![1, k, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        ));
    }

    for r in [&spline, &interleaved, &rich, &example_record_12_8()] {
        let b = r.to_bytes();
        assert!(Clip::parse(&b).is_ok());
        out.push(("havok_clip_record", with_mode(0, &b)));
    }
    for k in 0..5u8 {
        out.push((
            "havok_clip_record",
            vec![1, k, 30, 2, 3, 1, 9, 9, 9, 0, 0, 0],
        ));
    }

    let block = encode_block(&example_spline_12_3().blocks[0]);
    let mut raw = vec![0u8, 0, 1, 0];
    raw.extend_from_slice(&5u16.to_le_bytes());
    raw.extend_from_slice(&(block.len() as u16).to_le_bytes());
    raw.extend_from_slice(&block);
    out.push(("havok_spline_block", raw));
    if let AnimationKindSpec::Spline(sp) = &rich.animation.kind {
        for (b, bs) in sp.blocks.iter().enumerate() {
            let bytes = encode_block(bs);
            let mut raw = vec![0u8, b as u8, bs.tracks.len() as u8, 0];
            raw.extend_from_slice(&(sp.block_frames(b) as u16).to_le_bytes());
            raw.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            raw.extend_from_slice(&bytes);
            out.push(("havok_spline_block", raw));
        }
    }
    out.push((
        "havok_spline_block",
        vec![1, 60, 3, 2, 2, 2, 1, 7, 7, 7, 2, 2, 2, 5, 5, 5],
    ));
    out.push((
        "havok_spline_block",
        vec![2, 3, 0, 4, 0, 0, 0, 0, 2, 2, 2, 4, 4, 4, 4],
    ));
    out
}

#[test]
fn write_seed_corpora() {
    let seeds = seeds();
    let Some(dir) = std::env::var_os("TESSERA_HAVOK_SEED_DIR").map(PathBuf::from) else {
        return;
    };
    for (i, (target, bytes)) in seeds.iter().enumerate() {
        let d = dir.join(target);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(format!("testkit-{i:03}")), bytes).unwrap();
    }
    eprintln!("wrote {} seeds under {}", seeds.len(), dir.display());
}
