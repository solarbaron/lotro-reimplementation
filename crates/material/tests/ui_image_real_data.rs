// SPDX-License-Identifier: GPL-3.0-or-later
//! Real-data tests of the UI image decoders against a user's own install (read-only),
//! skipped unless `TESSERA_GAME_DIR` is set:
//!
//! - every loose `.jpg`, `.tga` and `.dds` file in the install folder tree (at most 8
//!   levels deep) is probed from its first 64 bytes and decoded as a skin image; the
//!   probe's format and size must agree with the decode, and JPEG files must decode the
//!   same through the plugin path;
//! - a sample of render surfaces (0x41) is converted with `from_surface`; the tag must
//!   follow the surface format and the pixels must equal `Surface::to_rgba8` (black for
//!   A8); JPEG surfaces must also decode the same with the file decoder `decode_jpeg`.
//!
//! Run with `TESSERA_GAME_DIR=… cargo test -p tessera-material --release --test
//! ui_image_real_data -- --nocapture`. `TESSERA_SAMPLE` overrides the surface sample size
//! and `TESSERA_SEED` the seed. Only counts and sizes are printed; nothing is written.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use tessera_formats::{Record, Surface, SurfaceFormat};
use tessera_material::ui_image::{
    ImageOrigin, PROBE_LEN, SourceTag, decode_file, decode_jpeg, format_for_path, from_surface,
    probe_header,
};
use tessera_props::{Did, GameData};

fn game_dir() -> Option<PathBuf> {
    match std::env::var_os("TESSERA_GAME_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => {
            eprintln!("TESSERA_GAME_DIR not set; skipping");
            None
        }
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Image files below `dir`, at most `depth` levels down, sorted. Symbolic links are not
/// followed.
fn image_files(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let Ok(ft) = e.file_type() else { continue };
        let path = e.path();
        if ft.is_dir() && depth > 0 {
            image_files(&path, depth - 1, out);
        } else if ft.is_file() && format_for_path(&path, ImageOrigin::Skin).is_ok() {
            out.push(path);
        }
    }
}

#[test]
fn loose_image_files_probe_and_decode() {
    let Some(dir) = game_dir() else { return };
    let mut files = Vec::new();
    image_files(&dir, 8, &mut files);
    eprintln!("{} loose image files", files.len());
    let mut by_format: BTreeMap<&str, usize> = BTreeMap::new();
    let mut cropped = 0usize;
    for path in &files {
        let mut head = Vec::with_capacity(PROBE_LEN);
        std::fs::File::open(path)
            .expect("open")
            .take(PROBE_LEN as u64)
            .read_to_end(&mut head)
            .expect("read header");
        let probe = probe_header(path, ImageOrigin::Skin, &head)
            .unwrap_or_else(|e| panic!("probe {}: {e}", path.display()));
        let bytes = std::fs::read(path).expect("read");
        let img = decode_file(path, ImageOrigin::Skin, &bytes)
            .unwrap_or_else(|e| panic!("decode {}: {e}", path.display()));
        if let Some((w, h)) = probe.dimensions {
            assert_eq!(
                (w, h),
                (img.source_width(), img.source_height()),
                "{}",
                path.display()
            );
        }
        assert_eq!(
            img.pixels().len() as u64,
            u64::from(img.width()) * u64::from(img.height())
        );
        cropped += usize::from(img.is_cropped());
        *by_format.entry(probe.format.name()).or_default() += 1;
        if probe.format.name() == "JPEG" {
            assert_eq!(img.tag(), SourceTag::NoAlpha);
            assert!(img.pixels().iter().all(|&p| p >> 24 == 0xFF));
            let plugin = decode_file(path, ImageOrigin::Plugin, &bytes).expect("plugin path");
            assert_eq!(plugin, img);
        }
    }
    eprintln!("decoded by format: {by_format:?}; cropped: {cropped}");
}

/// xorshift64*: a small deterministic generator for sampling.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

#[test]
fn render_surfaces_take_the_canonical_form() {
    let Some(dir) = game_dir() else { return };
    let gd = GameData::open(&dir).expect("open install");
    let mut dids = std::collections::BTreeSet::new();
    for member in gd.source().members() {
        for entry in member.archive.entries_range(0x4100_0000..0x4200_0000) {
            dids.insert(entry.expect("directory walk").did);
        }
    }
    let dids: Vec<u32> = dids.into_iter().collect();
    assert!(!dids.is_empty(), "no render surfaces");
    let sample = env_u64("TESSERA_SAMPLE", 400) as usize;
    let mut rng = Rng(env_u64("TESSERA_SEED", 0x0115) | 1);
    let mut by_format: BTreeMap<String, usize> = BTreeMap::new();
    let mut failed = 0usize;
    for _ in 0..sample {
        let did = dids[(rng.next() % dids.len() as u64) as usize];
        let bytes = gd.read(Did(did)).expect("read").expect("listed DID");
        let Ok(surface) = Surface::parse_expecting(&bytes, did) else {
            failed += 1;
            continue;
        };
        let Ok(rgba) = surface.to_rgba8() else {
            // Same failure must come out of from_surface.
            assert!(from_surface(&surface).is_err());
            failed += 1;
            continue;
        };
        let img = from_surface(&surface).unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
        let expected_tag = match surface.format {
            SurfaceFormat::A8R8G8B8 => SourceTag::Argb32,
            SurfaceFormat::A8 => SourceTag::AlphaOnly,
            SurfaceFormat::R8G8B8 | SurfaceFormat::Jpeg => SourceTag::NoAlpha,
            _ => SourceTag::Other,
        };
        assert_eq!(img.tag(), expected_tag, "{did:#010x}");
        assert_eq!(
            (img.source_width(), img.source_height()),
            (rgba.width, rgba.height)
        );
        for y in 0..img.height() {
            for x in 0..img.width() {
                let i = ((y * rgba.width + x) * 4) as usize;
                let p = &rgba.pixels[i..i + 4];
                let want = if surface.format == SurfaceFormat::A8 {
                    u32::from(p[3]) << 24
                } else {
                    u32::from_be_bytes([p[3], p[0], p[1], p[2]])
                };
                assert_eq!(img.pixel(x, y), Some(want), "{did:#010x} ({x}, {y})");
            }
        }
        if surface.format == SurfaceFormat::Jpeg {
            // The file decoder must agree with the embedded-surface decoder.
            let own = decode_jpeg(surface.data).unwrap_or_else(|e| panic!("{did:#010x}: {e}"));
            let diff = own
                .pixels()
                .iter()
                .zip(img.pixels())
                .flat_map(|(a, b)| a.to_be_bytes().into_iter().zip(b.to_be_bytes()))
                .map(|(a, b)| a.abs_diff(b))
                .max()
                .unwrap_or(0);
            assert_eq!((own.width(), own.height()), (img.width(), img.height()));
            assert!(diff <= 2, "{did:#010x}: JPEG decoders differ by {diff}");
        }
        *by_format
            .entry(format!("{:?}", surface.format))
            .or_default() += 1;
    }
    eprintln!("surfaces by format: {by_format:?}; not decodable: {failed}");
    assert!(by_format.values().sum::<usize>() > 0);
}
