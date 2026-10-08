// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests for the M6 sound records (`docs/specs/sound.md` §3, §4, §9, §14.1–§14.2;
//! `instruments.md` §14.1): sound infos (0x2A), the payload trailer (0x0A) and the typed
//! Sound and SoundEnvironment script nodes. Every buffer is built here; values are invented.

mod common;

use common::B;
use tessera_formats::script_table::{NodeKind, SoundNode};
use tessera_formats::{
    AnyRecord, AudioContainer, Did, DidKind, Evidence, FormatError, NoSchema, NodeData, Record,
    ScriptTable, Scriptlet, SoundInfo, SoundInfoFlags, SoundInfoIssue, SoundIssue, SoundRecord,
    StorageFormat, WaveInfo, decode_script_table, decode_sound, decode_sound_info,
};

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

/// `sound.md` §14.1, byte for byte.
const SPEC_SOUND_INFO: &str = "34 12 00 2A 05 00 00 00 00 00 80 BF 00 00 80 BF 00 00 00 3F \
                               00 00 40 40 00 00 20 42 78 56 00 0A 03 61 62 63";

/// `instruments.md` §14.1, byte for byte (34 bytes, one-byte name).
const INSTRUMENT_SOUND_INFO: &str = "34 12 00 2A 04 00 00 00 00 00 80 BF 00 00 80 BF \
                                     00 00 80 3F 00 00 40 40 00 00 F0 41 78 56 00 0A 01 78";

/// `sound.md` §14.2, byte for byte (64 bytes).
const SPEC_PAYLOAD: &str = "78 56 00 0A 30 00 00 00 \
                            52 49 46 46 28 00 00 00 57 41 56 45 \
                            66 6D 74 20 10 00 00 00 \
                            01 00 01 00 22 56 00 00 44 AC 00 00 02 00 10 00 \
                            64 61 74 61 04 00 00 00 E8 03 18 FC \
                            03 00 00 00 00 00 00 00";

fn sound_info(did: u32, flags: u32, payload: u32, name: &[u8]) -> Vec<u8> {
    B::new()
        .u32(did)
        .u32(flags)
        .f32(-1.0)
        .f32(-1.0)
        .f32(1.0)
        .f32(1.0)
        .f32(32.0)
        .u32(payload)
        .vle(name.len() as u32)
        .bytes(name)
        .build()
}

fn payload(did: u32, data: &[u8], format: u32, reserved: u32) -> Vec<u8> {
    B::new()
        .u32(did)
        .u32(data.len() as u32)
        .bytes(data)
        .u32(format)
        .u32(reserved)
        .build()
}

// ------------------------------------------------------------------ sound info (0x2A)

#[test]
fn spec_sound_info_example_decodes() {
    let data = hex(SPEC_SOUND_INFO);
    assert_eq!(data.len(), 36);
    let s = SoundInfo::parse_expecting(&data, 0x2A00_1234u32).unwrap();
    assert_eq!(s.did, Did(0x2A00_1234));
    assert_eq!(s.flags, SoundInfoFlags(0x05));
    assert!(s.is_loop());
    assert!(s.is_positional());
    assert!(!s.flags.is_attenuated_centred());
    assert!(s.flags.uses_distance_law());
    assert_eq!(s.unknown_flags(), 0);
    assert_eq!(s.unused_a, -1.0);
    assert_eq!(s.unused_b, -1.0);
    assert_eq!(s.priority, 0.5);
    assert_eq!(s.min_distance, 3.0);
    assert_eq!(s.max_distance, 40.0);
    assert_eq!(s.payload, Did(0x0A00_5678));
    assert!(s.payload_is_sound());
    assert_eq!(s.name, "abc");
    assert_eq!(decode_sound_info(&data).unwrap(), s);
}

#[test]
fn instrument_sound_info_example_decodes() {
    let mut data = hex(INSTRUMENT_SOUND_INFO);
    assert_eq!(data.len(), 34);
    let s = decode_sound_info(&data).unwrap();
    assert!(!s.is_loop());
    assert!(s.is_positional());
    assert_eq!(s.priority, 1.0);
    assert_eq!(s.min_distance, 3.0);
    assert_eq!(s.max_distance, 30.0);
    assert_eq!(s.name, "x");
    data[4] = 5;
    assert!(decode_sound_info(&data).unwrap().is_loop());
    // Truncating anywhere before the last byte is a decode error (instruments.md §14.1).
    for cut in 0..data.len() {
        assert!(decode_sound_info(&data[..cut]).is_err(), "cut at {cut}");
    }
}

#[test]
fn sound_info_truncation_and_trailing_bytes() {
    let data = hex(SPEC_SOUND_INFO);
    for cut in 0..data.len() {
        assert!(SoundInfo::parse(&data[..cut]).is_err(), "cut at {cut}");
    }
    let mut long = data.clone();
    long.push(0);
    assert!(matches!(
        SoundInfo::parse(&long),
        Err(FormatError::TrailingBytes {
            offset: 36,
            count: 1,
            ..
        })
    ));
    // A name length that runs past the end, including the largest VLE forms.
    for len in [4u32, 0x3FFF, 0x3FFF_FFFF, u32::MAX] {
        let bad = B::new().bytes(&data[..32]).vle(len).bytes(b"abc").build();
        assert!(
            matches!(SoundInfo::parse(&bad), Err(FormatError::Truncated { .. })),
            "name length {len}"
        );
    }
}

#[test]
fn sound_info_prefix_and_did_checks() {
    let data = hex(SPEC_SOUND_INFO);
    let mut wrong = data.clone();
    wrong[3] = 0x0A;
    assert!(matches!(
        SoundInfo::parse(&wrong),
        Err(FormatError::WrongRecordType { .. })
    ));
    assert!(matches!(
        SoundInfo::parse_expecting(&data, 0x2A00_1235u32),
        Err(FormatError::DidMismatch { .. })
    ));
}

#[test]
fn sound_info_keeps_unknown_flags_and_latin1_names() {
    let data = sound_info(0x2A00_0001, 0x8000_0148, 0x0A00_0001, &[b'z', 0xE9]);
    let s = decode_sound_info(&data).unwrap();
    assert_eq!(s.flags.bits(), 0x8000_0148);
    assert_eq!(s.unknown_flags(), 0x8000_0108);
    assert!(s.flags.is_attenuated_centred());
    assert!(!s.is_positional());
    assert!(s.flags.uses_distance_law());
    assert!(!s.flags.is_non_positional());
    assert_eq!(s.name, "z\u{e9}");

    let all = SoundInfoFlags(SoundInfoFlags::KNOWN);
    assert!(all.is_loop() && all.selects_voice_mode_4() && all.has_unread_0x10());
    assert_eq!(all.unknown_bits(), 0);
    assert!(SoundInfoFlags(0).is_non_positional());
    assert!(SoundInfoFlags(0x10).is_non_positional());

    // An empty name is the shortest record: 33 bytes.
    let empty = sound_info(0x2A00_0002, 0, 0x0A00_0002, b"");
    assert_eq!(empty.len(), 33);
    assert_eq!(decode_sound_info(&empty).unwrap().name, "");
}

#[test]
fn sound_info_unloaded_defaults() {
    let s = SoundInfo::unloaded(Did(0x2A00_0003));
    assert_eq!(s.flags.bits(), 0);
    assert_eq!((s.unused_a, s.unused_b), (-1.0, -1.0));
    assert_eq!(s.priority, 0.0);
    assert_eq!((s.min_distance, s.max_distance), (1.0, 50.0));
    assert!(s.payload.is_null());
}

#[test]
fn sound_info_validate_reports_each_invariant() {
    let good = SoundInfo::parse(&hex(SPEC_SOUND_INFO)).unwrap();
    assert_eq!(good.validate(), Ok(()));
    let with = |f: &dyn Fn(&mut SoundInfo)| {
        let mut s = good.clone();
        f(&mut s);
        s.validate()
    };
    assert_eq!(
        with(&|s| s.payload = Did(0x2A00_0001)),
        Err(SoundInfoIssue::PayloadNotSound(Did(0x2A00_0001)))
    );
    assert_eq!(
        with(&|s| s.unused_b = 0.0),
        Err(SoundInfoIssue::UnusedFloats {
            unused_a: -1.0,
            unused_b: 0.0
        })
    );
    assert!(matches!(
        with(&|s| s.unused_a = f32::NAN),
        Err(SoundInfoIssue::UnusedFloats { .. })
    ));
    assert_eq!(
        with(&|s| s.priority = 1.5),
        Err(SoundInfoIssue::Priority(1.5))
    );
    assert_eq!(
        with(&|s| s.priority = -0.1),
        Err(SoundInfoIssue::Priority(-0.1))
    );
    assert!(matches!(
        with(&|s| s.priority = f32::NAN),
        Err(SoundInfoIssue::Priority(p)) if p.is_nan()
    ));
    assert_eq!(with(&|s| s.priority = 0.0), Ok(()));
    assert_eq!(with(&|s| s.priority = 1.0), Ok(()));
    assert_eq!(
        with(&|s| s.max_distance = 0.0),
        Err(SoundInfoIssue::MaxDistance(0.0))
    );
    assert!(matches!(
        with(&|s| s.max_distance = f32::NAN),
        Err(SoundInfoIssue::MaxDistance(d)) if d.is_nan()
    ));
    // min > max is allowed (17 records, §4.1).
    assert_eq!(with(&|s| s.min_distance = 100.0), Ok(()));
    assert_eq!(
        with(&|s| s.flags = SoundInfoFlags(0x05 | 0x08 | 0x100)),
        Err(SoundInfoIssue::UnknownFlags(0x108))
    );
    // Every known bit together is fine.
    assert_eq!(
        with(&|s| s.flags = SoundInfoFlags(SoundInfoFlags::KNOWN)),
        Ok(())
    );
    // The constructor defaults have no payload, so they are not a valid record.
    assert_eq!(
        SoundInfo::unloaded(Did(0x2A00_0001)).validate(),
        Err(SoundInfoIssue::PayloadNotSound(Did(0)))
    );
}

#[test]
fn sound_info_routes_through_any_record() {
    assert_eq!(Did(0x2A00_1234).kind(), DidKind::SoundInfo);
    assert_eq!(DidKind::SoundInfo.evidence(), Evidence::Decoded);
    assert_eq!(DidKind::SoundInfo.name(), "sound info");
    assert!(AnyRecord::is_decodable(0x2A00_1234u32));
    let data = hex(SPEC_SOUND_INFO);
    let rec = AnyRecord::decode(0x2A00_1234u32, &data).unwrap().unwrap();
    assert_eq!(rec.did(), Did(0x2A00_1234));
    assert_eq!(rec.type_name(), "sound info");
    match rec {
        AnyRecord::SoundInfo(s) => assert_eq!(s.name, "abc"),
        r => panic!("{r:?}"),
    }
    assert!(AnyRecord::decode(0x2A00_9999u32, &data).is_err());
}

// ------------------------------------------------------------------ payload (0x0A)

#[test]
fn spec_payload_example_decodes() {
    let data = hex(SPEC_PAYLOAD);
    assert_eq!(data.len(), 64);
    let s = SoundRecord::parse_expecting(&data, 0x0A00_5678u32).unwrap();
    assert_eq!(s.audio.len(), 48);
    assert_eq!(s.record_len(), 64);
    assert_eq!(s.container(), AudioContainer::Wave);
    assert_eq!(s.storage_format(), StorageFormat::Pcm);
    assert_eq!(s.trailer_u32, 3);
    assert_eq!(s.reserved(), 0);
    assert_eq!(s.validate(0x0A00_5678u32, data.len()), Ok(()));
    let w = WaveInfo::parse(s.audio).unwrap();
    assert_eq!(
        (w.format_tag, w.channels, w.sample_rate, w.byte_rate),
        (1, 1, 22_050, 44_100)
    );
    assert_eq!((w.block_align, w.bits_per_sample), (2, 16));
    assert_eq!(w.data, &[0xE8, 0x03, 0x18, 0xFC]);
    let samples: Vec<i16> = w
        .data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| i16::from_le_bytes(*c))
        .collect();
    assert_eq!(samples, [1000, -1000]);
}

#[test]
fn spec_payload_with_junk_chunk_decodes() {
    // The same WAV with a 1-byte JUNK chunk (plus pad byte) before `fmt ` (§14.2).
    let spec = hex(SPEC_PAYLOAD);
    let wav = &spec[8..56];
    let body = B::new()
        .bytes(b"WAVE")
        .bytes(b"JUNK")
        .u32(1)
        .u8(0x5A)
        .u8(0)
        .bytes(&wav[12..])
        .build();
    let riff = B::new()
        .bytes(b"RIFF")
        .u32(body.len() as u32)
        .bytes(&body)
        .build();
    let data = payload(0x0A00_5678, &riff, 3, 0);
    let s = decode_sound(&data).unwrap();
    assert_eq!(s.validate(0x0A00_5678u32, data.len()), Ok(()));
    let w = WaveInfo::parse(s.audio).unwrap();
    assert_eq!(w.sample_rate, 22_050);
    assert_eq!(w.data, &[0xE8, 0x03, 0x18, 0xFC]);
}

#[test]
fn storage_formats_classify() {
    for (raw, f) in [
        (2, StorageFormat::Stored),
        (3, StorageFormat::Pcm),
        (7, StorageFormat::NotAudio),
        (5, StorageFormat::Other(5)),
    ] {
        assert_eq!(StorageFormat::from_raw(raw), f);
        assert_eq!(f.raw(), raw);
    }
}

#[test]
fn payload_validation_reports_each_invariant() {
    const DID: u32 = 0x0A00_0001;
    let ogg = b"OggS\0\x02rest";
    let dmap = b"RIFF\x04\0\0\0DMAP";
    let check = |data: &[u8], expected: u32| {
        decode_sound(data)
            .unwrap()
            .validate(expected, data.len())
            .err()
    };
    assert_eq!(check(&payload(DID, ogg, 2, 0), DID), None);
    assert_eq!(check(&payload(DID, dmap, 7, 0), DID), None);
    assert_eq!(
        check(&payload(DID, ogg, 2, 0), DID + 1),
        Some(SoundIssue::Did {
            expected: Did(DID + 1),
            found: Did(DID)
        })
    );
    assert_eq!(
        check(&payload(DID, ogg, 2, 0x3F80_0000), DID),
        Some(SoundIssue::Reserved(0x3F80_0000))
    );
    assert_eq!(
        check(&payload(DID, b"ID3\x03", 2, 0), DID),
        Some(SoundIssue::UnknownContainer)
    );
    assert_eq!(
        check(&payload(DID, ogg, 5, 0), DID),
        Some(SoundIssue::StorageFormat(5))
    );
    assert!(matches!(
        check(&payload(DID, ogg, 7, 0), DID),
        Some(SoundIssue::FormatMismatch { format: 7, .. })
    ));
    assert!(matches!(
        check(&payload(DID, dmap, 3, 0), DID),
        Some(SoundIssue::FormatMismatch { format: 3, .. })
    ));
    let data = payload(DID, ogg, 2, 0);
    let s = decode_sound(&data).unwrap();
    assert_eq!(
        s.validate(DID, data.len() + 1),
        Err(SoundIssue::Length {
            record_len: data.len() + 1,
            audio_len: ogg.len()
        })
    );
    // NaN bit patterns in the reserved word (quiet and signalling) survive unchanged: the
    // word is read as an integer, never round-tripped through a float.
    for bits in [0x7FC0_1234u32, 0x7F80_0001, 0xFF80_0001, 0xFFFF_FFFF] {
        let nan = payload(DID, ogg, 2, bits);
        let s = decode_sound(&nan).unwrap();
        assert_eq!(s.reserved(), bits);
        assert_eq!(s.validate(DID, nan.len()), Err(SoundIssue::Reserved(bits)));
    }
}

/// A RIFF WAVE stream: `fmt ` (PCM, `channels`, 16-bit) then a `data` chunk that declares
/// `declared` bytes and holds `held`.
fn wav(channels: u16, bits: u16, declared: u32, held: &[u8]) -> Vec<u8> {
    let align = channels * bits / 8;
    let body = B::new()
        .bytes(b"WAVE")
        .bytes(b"fmt ")
        .u32(16)
        .u16(1)
        .u16(channels)
        .u32(22_050)
        .u32(22_050 * u32::from(align))
        .u16(align)
        .u16(bits)
        .bytes(b"data")
        .u32(declared)
        .bytes(held)
        .build();
    B::new()
        .bytes(b"RIFF")
        .u32(body.len() as u32)
        .bytes(&body)
        .build()
}

/// `sound.md` §12.2 step 5: a `data` chunk shorter than its declared size is rejected.
#[test]
fn wave_data_shorter_than_declared_is_rejected() {
    for declared in [5u32, 100, 1000, u32::MAX] {
        let audio = wav(1, 16, declared, &[1, 2, 3, 4]);
        assert_eq!(
            WaveInfo::parse(&audio),
            Err(FormatError::Truncated {
                offset: 44,
                needed: usize::try_from(declared).unwrap(),
                available: 4,
            }),
            "declared {declared}"
        );
    }
    // Exactly the declared size still parses.
    let exact = wav(1, 16, 4, &[1, 2, 3, 4]);
    let w = WaveInfo::parse(&exact).unwrap();
    assert_eq!(w.data, &[1, 2, 3, 4]);
}

/// `sound.md` §12.2 step 5: a `data` length that is not a multiple of the block align is
/// rejected.
#[test]
fn wave_data_not_a_multiple_of_block_align_is_rejected() {
    // Mono 16-bit (align 2) with 3 bytes (plus the pad byte of an odd chunk).
    let mut audio = wav(1, 16, 3, &[1, 2, 3]);
    audio.push(0);
    let err = WaveInfo::parse(&audio).unwrap_err();
    assert!(
        matches!(err, FormatError::Malformed { offset: 44, .. }),
        "{err:?}"
    );
    // Stereo 16-bit (align 4) with 6 bytes.
    let err = WaveInfo::parse(&wav(2, 16, 6, &[0; 6])).unwrap_err();
    assert!(matches!(err, FormatError::Malformed { .. }), "{err:?}");
    // Stereo 24-bit (align 6) with 12 bytes is fine; an empty `data` chunk too.
    assert_eq!(
        WaveInfo::parse(&wav(2, 24, 12, &[0; 12]))
            .unwrap()
            .data
            .len(),
        12
    );
    assert!(
        WaveInfo::parse(&wav(1, 16, 0, &[]))
            .unwrap()
            .data
            .is_empty()
    );
}

/// A WAV inside a payload record whose `data` chunk is short fails as a WAV, while the
/// payload envelope itself still decodes (its own length identity holds).
#[test]
fn short_wave_data_inside_a_payload() {
    let audio = wav(1, 16, 1000, &[0; 4]);
    let data = payload(0x0A00_0002, &audio, 3, 0);
    let s = decode_sound(&data).unwrap();
    assert_eq!(s.container(), AudioContainer::Wave);
    assert!(matches!(
        WaveInfo::parse(s.audio),
        Err(FormatError::Truncated { .. })
    ));
}

#[test]
fn payload_hostile_lengths() {
    let good = payload(0x0A00_0001, b"OggS", 2, 0);
    // S larger than the data: never an over-read or a huge allocation.
    for s in [5u32, 0x7FFF_FFFF, u32::MAX] {
        let mut bad = good.clone();
        bad[4..8].copy_from_slice(&s.to_le_bytes());
        assert!(
            matches!(decode_sound(&bad), Err(FormatError::Truncated { .. })),
            "S = {s:#X}"
        );
    }
    // S one short: the record has a trailing byte.
    let mut short = good.clone();
    short[4..8].copy_from_slice(&3u32.to_le_bytes());
    assert!(decode_sound(&short).is_err());
    let mut long = good;
    long.extend_from_slice(&[0, 0, 0, 0]);
    assert!(matches!(
        decode_sound(&long),
        Err(FormatError::TrailingBytes { .. })
    ));
}

// ------------------------------------------------------------------ script nodes (§9)

fn base(id: u32) -> B {
    B::new()
        .tsize(1, 0)
        .tsize(1, 0)
        .u8(0)
        .u8(1)
        .f32(0.8)
        .f32(0.0)
        .f32(1.0)
        .u32(id)
}

fn node(kind: u8, data: &[u8], id: u32) -> Vec<u8> {
    B::new()
        .u8(kind)
        .bytes(data)
        .bytes(&base(id).build())
        .u32(0)
        .u32(0)
        .build()
}

/// A Sound node's 43 data bytes with out-of-range values kept as stored (§9.1).
fn sound_data(sound: u32) -> Vec<u8> {
    B::new()
        .u32(sound)
        .u8(0) // FireAndForget
        .u8(1) // MoveWithEntity
        .u8(1) // UseCameraPosition
        .f32(70.0) // MinRandomDistance > Max
        .f32(60.0)
        .f32(0.5)
        .f32(-1.5)
        .f32(2.0)
        .u32(2) // Channel
        .f32(15.0) // RampOutTime beyond 4
        .f32(-8.0) // Pitch beyond -1
        .f32(-0.25) // PitchVariance negative
        .build()
}

fn table(roots: &[Vec<u8>]) -> Vec<u8> {
    let mut b = B::new()
        .u32(1)
        .vle(0)
        .tsize(1, 1)
        .u32(5)
        .vle(roots.len() as u32);
    for r in roots {
        b = b.bytes(r);
    }
    b.tsize(1, 0).tsize(1, 0).build()
}

#[test]
fn sound_node_fields_are_typed_and_unclamped() {
    let data = sound_data(0x2A00_0010);
    assert_eq!(data.len(), SoundNode::SIZE);
    let env = B::new().u32(7).u32((-3i32) as u32).build();
    let t = table(&[node(48, &data, 1), node(62, &env, 2)]);
    let t = decode_script_table(&t, &NoSchema).unwrap();
    let roots = &t.scripts[0].roots;
    assert_eq!(roots[0].kind, NodeKind::SOUND);
    let s = roots[0].sound().expect("sound node");
    assert_eq!(s.sound, Did(0x2A00_0010));
    assert!(!s.fire_and_forget);
    assert!(s.move_with_entity);
    assert!(s.use_camera_position);
    assert_eq!(s.min_random_distance, 70.0);
    assert_eq!(s.max_random_distance, 60.0);
    assert_eq!(s.offset, [0.5, -1.5, 2.0]);
    assert_eq!(s.channel, 2);
    assert_eq!(s.ramp_out_time, 15.0);
    assert_eq!(s.pitch, -8.0);
    assert_eq!(s.pitch_variance, -0.25);
    assert!(!s.stored_sound_is_null());
    assert!(!roots[0].is_sound_placeholder());
    assert_eq!(s.random_distance_range(), (60.0, 60.0));
    assert_eq!(roots[0].base.intensity_multiplier, 0.8);

    assert_eq!(roots[1].kind, NodeKind::SOUND_ENVIRONMENT);
    assert_eq!(
        roots[1].data,
        NodeData::SoundEnvironment {
            name: 7,
            priority: -3
        }
    );
    assert!(roots[1].sound().is_none());
}

#[test]
fn sound_node_in_a_scriptlet_and_placeholder() {
    let mut b = B::new().vle(1);
    b = b.bytes(&node(48, &sound_data(0), 9));
    let data = b.u32(0).build();
    let s = Scriptlet::parse_expecting(&data, 0x0C00_0001u32, &NoSchema).unwrap();
    let n = s.roots[0].sound().unwrap();
    assert!(n.stored_sound_is_null());
    assert!(s.roots[0].is_sound_placeholder());
}

/// A Sound node whose stored `SoundScript_SoundID` is 0 but whose ID is driven from an
/// entity property (`sound.md` §9.1: 24 such nodes play) is not a placeholder.
#[test]
fn driven_null_sound_id_is_not_a_placeholder() {
    const SOURCE: u32 = 0x0001_2345; // an invented entity property
    let base = B::new()
        .tsize(1, 1)
        .u32(tessera_formats::script_table::sound_pid::SOUND_ID)
        .u32(SOURCE)
        .tsize(1, 0)
        .u8(0)
        .u8(1)
        .f32(1.0)
        .f32(0.0)
        .f32(1.0)
        .u32(3)
        .build();
    let driven = B::new()
        .u8(48)
        .bytes(&sound_data(0))
        .bytes(&base)
        .u32(0)
        .u32(0)
        .build();
    let data = B::new().vle(1).bytes(&driven).u32(0).build();
    let s = Scriptlet::parse(&data, &NoSchema).unwrap();
    let n = &s.roots[0];
    assert!(n.sound().unwrap().stored_sound_is_null());
    assert_eq!(
        n.base
            .driven_source(tessera_formats::script_table::sound_pid::SOUND_ID),
        Some(SOURCE)
    );
    assert!(!n.is_sound_placeholder());

    // Not a Sound node: never a placeholder.
    let env = table(&[node(62, &[1, 0, 0, 0, 2, 0, 0, 0], 2)]);
    let t = decode_script_table(&env, &NoSchema).unwrap();
    assert!(!t.scripts[0].roots[0].is_sound_placeholder());
}

#[test]
fn sound_node_defaults_and_distance_range() {
    let d = SoundNode::default();
    assert!(d.fire_and_forget && d.move_with_entity && !d.use_camera_position);
    assert!(d.sound.is_null());
    assert_eq!(d.random_distance_range(), (0.0, 0.0));
    let n = SoundNode {
        min_random_distance: -5.0,
        max_random_distance: 275.0,
        ..SoundNode::default()
    };
    assert_eq!(n.random_distance_range(), (0.0, 275.0));
    let n = SoundNode {
        min_random_distance: f32::NAN,
        max_random_distance: -1.0,
        ..SoundNode::default()
    };
    assert_eq!(n.random_distance_range(), (0.0, 0.0));
}

/// Hostile non-finite random distances never yield an infinite range (`sound.md` §9.2 step
/// 5 OPEN): they count as 0, like NaN. Large finite values are kept.
#[test]
fn sound_node_distance_range_is_always_finite() {
    let cases = [
        (f32::INFINITY, f32::INFINITY, (0.0, 0.0)),
        (10.0, f32::INFINITY, (0.0, 0.0)),
        (f32::INFINITY, 20.0, (0.0, 20.0)),
        (f32::NEG_INFINITY, 20.0, (0.0, 20.0)),
        (f32::NAN, f32::NAN, (0.0, 0.0)),
        (1.0, f32::MAX, (1.0, f32::MAX)),
    ];
    for (min, max, want) in cases {
        let n = SoundNode {
            min_random_distance: min,
            max_random_distance: max,
            ..SoundNode::default()
        };
        let (lo, hi) = n.random_distance_range();
        assert_eq!((lo, hi), want, "({min}, {max})");
        assert!(lo.is_finite() && hi.is_finite() && 0.0 <= lo && lo <= hi);
    }
    // Through the decoder: bytes 7..15 of the node data are both +infinity.
    let mut data = sound_data(0x2A00_0010);
    data[7..11].copy_from_slice(&f32::INFINITY.to_le_bytes());
    data[11..15].copy_from_slice(&f32::INFINITY.to_le_bytes());
    let t = decode_script_table(&table(&[node(48, &data, 1)]), &NoSchema).unwrap();
    let s = t.scripts[0].roots[0].sound().unwrap();
    assert_eq!(s.max_random_distance, f32::INFINITY, "stored value kept");
    assert_eq!(s.random_distance_range(), (0.0, 0.0));
}

#[test]
fn sound_nodes_reject_bad_bools_and_truncation() {
    let mut data = sound_data(0x2A00_0010);
    data[5] = 2; // MoveWithEntity
    let t = table(&[node(48, &data, 1)]);
    assert!(matches!(
        ScriptTable::parse(&t, &NoSchema),
        Err(e) if matches!(e.root_cause(), FormatError::InvalidBool { value: 2, .. })
    ));
    let good = table(&[
        node(48, &sound_data(0x2A00_0010), 1),
        node(62, &[1, 0, 0, 0, 2, 0, 0, 0], 2),
    ]);
    assert!(ScriptTable::parse(&good, &NoSchema).is_ok());
    for cut in 0..good.len() {
        assert!(
            ScriptTable::parse(&good[..cut], &NoSchema).is_err(),
            "cut at {cut}"
        );
    }
}
