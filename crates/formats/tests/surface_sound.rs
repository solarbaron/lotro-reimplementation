// SPDX-License-Identifier: GPL-3.0-or-later
//! Render surfaces, BCn decoding and sound records (spec §5, §9).

mod common;

use common::B;
use tessera_formats::bcn::{BcFormat, decode_bcn};
use tessera_formats::{
    AudioContainer, Did, FormatError, Record, SoundRecord, Surface, SurfaceFormat, WaveInfo,
};

fn surface(fmt: u32, w: u32, h: u32, data: &[u8]) -> Vec<u8> {
    B::new()
        .u32(0x4100_0001)
        .u32(15)
        .u32(w)
        .u32(h)
        .u32(fmt)
        .u32(data.len() as u32)
        .bytes(data)
        .build()
}

fn px(img: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let o = ((y * w + x) * 4) as usize;
    img[o..o + 4].try_into().unwrap()
}

#[test]
fn format_codes() {
    assert_eq!(SurfaceFormat::CODE_DXT1, 0x3154_5844);
    assert_eq!(SurfaceFormat::CODE_DXT3, 0x3354_5844);
    assert_eq!(SurfaceFormat::CODE_DXT5, 0x3554_5844);
    for f in [
        SurfaceFormat::R8G8B8,
        SurfaceFormat::A8R8G8B8,
        SurfaceFormat::A8,
        SurfaceFormat::Jpeg,
        SurfaceFormat::Dxt1,
        SurfaceFormat::Dxt3,
        SurfaceFormat::Dxt5,
    ] {
        assert_eq!(SurfaceFormat::from_code(f.code()), Some(f));
    }
    assert_eq!(SurfaceFormat::from_code(22), None);
}

#[test]
fn uncompressed_formats_convert_to_rgba() {
    // 2x1 R8G8B8, stored B, G, R.
    let data = surface(20, 2, 1, &[1, 2, 3, 4, 5, 6]);
    let s = Surface::parse(&data).unwrap();
    assert_eq!(s.did, Did(0x4100_0001));
    assert_eq!(s.category, 15);
    assert_eq!(s.format, SurfaceFormat::R8G8B8);
    let img = s.to_rgba8().unwrap();
    assert_eq!((img.width, img.height), (2, 1));
    assert_eq!(img.pixels, vec![3, 2, 1, 255, 6, 5, 4, 255]);

    // 1x2 A8R8G8B8, stored B, G, R, A; top row first.
    let data = surface(21, 1, 2, &[10, 20, 30, 0, 40, 50, 60, 255]);
    let img = Surface::parse(&data).unwrap().to_rgba8().unwrap();
    assert_eq!(img.pixels, vec![30, 20, 10, 0, 60, 50, 40, 255]);

    let data = surface(28, 2, 1, &[0, 200]);
    let img = Surface::parse(&data).unwrap().to_rgba8().unwrap();
    assert_eq!(img.pixels, vec![255, 255, 255, 0, 255, 255, 255, 200]);
}

#[test]
fn wrong_length_and_unknown_format_are_rejected() {
    let data = surface(21, 2, 2, &[0; 15]);
    assert!(matches!(
        Surface::parse(&data),
        Err(FormatError::SurfaceLength {
            expected: 16,
            actual: 15,
            ..
        })
    ));
    let data = surface(SurfaceFormat::CODE_DXT1, 8, 4, &[0; 8]);
    assert!(matches!(
        Surface::parse(&data),
        Err(FormatError::SurfaceLength { expected: 16, .. })
    ));
    let data = surface(0x3254_5844, 4, 4, &[0; 16]); // "DXT2"
    assert_eq!(
        Surface::parse(&data).unwrap_err(),
        FormatError::UnsupportedSurfaceFormat { code: 0x3254_5844 }
    );
    // Header length larger than the payload.
    let mut data = surface(28, 2, 2, &[0; 4]);
    data.truncate(data.len() - 1);
    assert!(Surface::parse(&data).is_err());
    // Payload longer than the header says (RS1).
    let mut data = surface(28, 2, 2, &[0; 4]);
    data.push(0);
    assert!(matches!(
        Surface::parse(&data),
        Err(FormatError::TrailingBytes { .. })
    ));
    // Huge dimensions do not overflow.
    let data = surface(21, u32::MAX, u32::MAX, &[]);
    assert!(Surface::parse(&data).is_err());
}

#[test]
fn hand_built_surface_struct_with_short_data_is_rejected() {
    let s = Surface {
        did: Did(0x4100_0000),
        category: 0,
        width: 4,
        height: 4,
        format: SurfaceFormat::A8R8G8B8,
        data: &[0; 4],
    };
    assert!(s.to_rgba8().is_err());
}

const RED: u16 = 0xF800;
const BLUE: u16 = 0x001F;

fn color_block(c0: u16, c1: u16, idx: [u8; 4]) -> Vec<u8> {
    B::new().u16(c0).u16(c1).bytes(&idx).build()
}

#[test]
fn dxt1_four_colour_and_punchthrough() {
    let block = color_block(RED, BLUE, [0xE4, 0, 0, 0]); // texels 0..3 = idx 0,1,2,3
    let data = surface(SurfaceFormat::CODE_DXT1, 4, 4, &block);
    let img = Surface::parse(&data).unwrap().to_rgba8().unwrap();
    assert_eq!(px(&img.pixels, 4, 0, 0), [255, 0, 0, 255]);
    assert_eq!(px(&img.pixels, 4, 1, 0), [0, 0, 255, 255]);
    assert_eq!(px(&img.pixels, 4, 2, 0), [170, 0, 85, 255]);
    assert_eq!(px(&img.pixels, 4, 3, 0), [85, 0, 170, 255]);
    assert_eq!(px(&img.pixels, 4, 3, 3), [255, 0, 0, 255]);

    let block = color_block(BLUE, RED, [0xE4, 0, 0, 0]);
    let img = decode_bcn(BcFormat::Bc1, 4, 4, &block).unwrap();
    assert_eq!(px(&img, 4, 2, 0), [128, 0, 128, 255]);
    assert_eq!(px(&img, 4, 3, 0), [0, 0, 0, 0]);
}

#[test]
fn dxt3_explicit_alpha() {
    let mut block = B::new().u8(0x0F).u8(0x80).bytes(&[0; 6]).build();
    // Colour part with c0 <= c1 must still use four-colour mode in DXT3.
    block.extend(color_block(BLUE, RED, [0x03, 0, 0, 0]));
    let data = surface(SurfaceFormat::CODE_DXT3, 4, 4, &block);
    let img = Surface::parse(&data).unwrap().to_rgba8().unwrap();
    assert_eq!(px(&img.pixels, 4, 0, 0), [170, 0, 85, 255]);
    assert_eq!(px(&img.pixels, 4, 1, 0)[3], 0);
    assert_eq!(px(&img.pixels, 4, 3, 0)[3], 0x88);
}

#[test]
fn dxt5_interpolated_alpha_both_modes() {
    // a0 > a1: indices 0, 1, 2, 7 for the first four texels.
    let mut block = B::new()
        .u8(255)
        .u8(0)
        .bytes(&[0x88, 0x0E, 0, 0, 0, 0])
        .build();
    block.extend(color_block(RED, RED, [0; 4]));
    let data = surface(SurfaceFormat::CODE_DXT5, 4, 4, &block);
    let img = Surface::parse(&data).unwrap().to_rgba8().unwrap();
    let alphas: Vec<u8> = (0..4).map(|x| px(&img.pixels, 4, x, 0)[3]).collect();
    assert_eq!(alphas, vec![255, 0, 219, 36]);

    // a0 <= a1: index 6 = 0, index 7 = 255, index 2 = (4*a0 + a1) / 5.
    let idx: u64 = 6 | (7 << 3) | (2 << 6);
    let ib = idx.to_le_bytes();
    let mut block = B::new().u8(0).u8(255).bytes(&ib[..6]).build();
    block.extend(color_block(RED, RED, [0; 4]));
    let img = decode_bcn(BcFormat::Bc3, 4, 4, &block).unwrap();
    let alphas: Vec<u8> = (0..3).map(|x| px(&img, 4, x, 0)[3]).collect();
    assert_eq!(alphas, vec![0, 255, 51]);
}

#[test]
fn dxt_non_multiple_of_four_dimensions() {
    let mut blocks = color_block(RED, RED, [0; 4]);
    blocks.extend(color_block(BLUE, BLUE, [0; 4]));
    let data = surface(SurfaceFormat::CODE_DXT1, 5, 3, &blocks);
    let s = Surface::parse(&data).unwrap();
    let img = s.to_rgba8().unwrap();
    assert_eq!(img.pixels.len(), 5 * 3 * 4);
    assert_eq!(px(&img.pixels, 5, 3, 2), [255, 0, 0, 255]);
    assert_eq!(px(&img.pixels, 5, 4, 2), [0, 0, 255, 255]);
    assert!(decode_bcn(BcFormat::Bc1, 5, 3, &blocks[..15]).is_none());
}

#[cfg(feature = "jpeg")]
#[test]
fn embedded_jpeg_decodes() {
    use image::codecs::jpeg::JpegEncoder;
    let mut jpeg = Vec::new();
    let rgb: Vec<u8> = std::iter::repeat_n([200u8, 100, 50], 16 * 8)
        .flatten()
        .collect();
    JpegEncoder::new_with_quality(&mut jpeg, 95)
        .encode(&rgb, 16, 8, image::ExtendedColorType::Rgb8)
        .unwrap();
    let data = surface(500, 0, 0, &jpeg);
    let s = Surface::parse(&data).unwrap();
    assert!(s.is_jpeg());
    let img = s.to_rgba8().unwrap();
    assert_eq!((img.width, img.height), (16, 8));
    let p = px(&img.pixels, 16, 5, 5);
    assert!(p[0].abs_diff(200) < 8 && p[1].abs_diff(100) < 8 && p[2].abs_diff(50) < 8);
    assert_eq!(p[3], 255);

    let bad = surface(500, 0, 0, &[0xFF, 0xD8, 0xFF, 0x00, 0x01]);
    assert!(Surface::parse(&bad).unwrap().to_rgba8().is_err());
    let not_jpeg = surface(500, 0, 0, b"GIF89a");
    assert!(Surface::parse(&not_jpeg).unwrap().to_rgba8().is_err());
}

// ---------------------------------------------------------------- sound

fn sound(audio: &[u8]) -> Vec<u8> {
    B::new()
        .u32(0x0A00_0001)
        .u32(audio.len() as u32)
        .bytes(audio)
        .u32(3)
        .f32(0.0)
        .build()
}

fn wav() -> Vec<u8> {
    let fmt = B::new()
        .u16(1)
        .u16(2)
        .u32(22_050)
        .u32(88_200)
        .u16(4)
        .u16(16)
        .build();
    let body = B::new()
        .bytes(b"WAVE")
        .bytes(b"JUNK")
        .u32(3)
        .bytes(&[0, 0, 0, 0]) // odd size, padded
        .bytes(b"fmt ")
        .u32(fmt.len() as u32)
        .bytes(&fmt)
        .bytes(b"data")
        .u32(4)
        .bytes(&[1, 2, 3, 4])
        .build();
    B::new()
        .bytes(b"RIFF")
        .u32(body.len() as u32)
        .bytes(&body)
        .build()
}

#[test]
fn sound_containers() {
    let ogg = sound(b"OggS\0\x02rest");
    let s = SoundRecord::parse(&ogg).unwrap();
    assert_eq!(s.container(), AudioContainer::Ogg);
    assert_eq!(s.trailer_u32, 3);
    assert_eq!(s.container().extension(), Some("ogg"));

    let w = sound(&wav());
    let s = SoundRecord::parse(&w).unwrap();
    assert_eq!(s.container(), AudioContainer::Wave);
    let info = WaveInfo::parse(s.audio).unwrap();
    assert_eq!(info.format_tag, 1);
    assert_eq!(info.channels, 2);
    assert_eq!(info.sample_rate, 22_050);
    assert_eq!(info.bits_per_sample, 16);
    assert_eq!(info.data, &[1, 2, 3, 4]);

    let dmap = sound(b"RIFF\x04\0\0\0DMAP");
    assert_eq!(
        SoundRecord::parse(&dmap).unwrap().container(),
        AudioContainer::DirectMusicAudioPath
    );
    let unk = sound(b"ID3\x03");
    assert_eq!(
        SoundRecord::parse(&unk).unwrap().container(),
        AudioContainer::Unknown
    );
    assert!(WaveInfo::parse(b"RIFF\x04\0\0\0DMAP").is_err());
}

#[test]
fn sound_truncated_and_trailing() {
    let data = sound(b"OggS");
    for cut in 0..data.len() {
        assert!(SoundRecord::parse(&data[..cut]).is_err(), "cut at {cut}");
    }
    let mut long = data.clone();
    long.push(0);
    assert!(matches!(
        SoundRecord::parse(&long),
        Err(FormatError::TrailingBytes { .. })
    ));
    let mut wrong = data;
    wrong[3] = 0x41;
    assert!(matches!(
        SoundRecord::parse(&wrong),
        Err(FormatError::WrongRecordType { .. })
    ));
}

/// A short `fmt ` chunk is reported at its offset in the audio buffer, not relative to the
/// chunk body.
#[test]
fn short_fmt_chunk_reports_absolute_offset() {
    let body = B::new()
        .bytes(b"WAVE")
        .bytes(b"JUNK")
        .u32(100)
        .bytes(&[0; 100])
        .bytes(b"fmt ")
        .u32(4)
        .u16(1)
        .u16(2)
        .bytes(b"data")
        .u32(0)
        .build();
    let audio = B::new()
        .bytes(b"RIFF")
        .u32(body.len() as u32)
        .bytes(&body)
        .build();
    // RIFF header 12 + JUNK header 8 + 100 + fmt header 8: the body starts at 128.
    assert_eq!(
        WaveInfo::parse(&audio),
        Err(FormatError::Truncated {
            offset: 128,
            needed: 16,
            available: 4,
        })
    );
}
