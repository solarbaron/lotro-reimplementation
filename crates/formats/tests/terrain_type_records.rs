// SPDX-License-Identifier: GPL-3.0-or-later
//! Terrain type tables (prefix 0x19, `docs/specs/terrain-texturing.md` §3) on synthetic
//! payloads: the worked example of §14, the decoder invariants of §3.5 and hostile input.

mod common;

use common::B;
use tessera_formats::terrain_type::{
    Argb, CURVE_PARAMS, CurveKey, FloatCurve, MIN_TERRAIN_TYPE_ENTRY_LEN, TerrainType,
    TerrainTypeTable, TextureProjection, WaveformMode, decode_terrain_type_table,
};
use tessera_formats::{Did, FormatError, Record};

const DID: u32 = 0x1900_0042;

/// One entry with both curves in mode 1 (`terrain-texturing.md` §3.3), as in §14.
struct E {
    key: u32,
    label: &'static str,
    draw_order: u32,
    diffuse: u32,
    scale: f32,
    rotation: f32,
    opacity: bool,
    threshold: f32,
    blend_max: f32,
    diffuse_scale: [u8; 4],
}

fn entry(b: B, e: &E) -> B {
    b.u32(e.key)
        .pascal(e.label)
        .u32(e.draw_order)
        .u32(e.diffuse)
        .f32(e.scale)
        .f32(e.scale)
        .f32(e.rotation)
        .u32(1)
        .f32(0.0) // u_offset, mode 1
        .u32(1)
        .f32(0.0) // v_offset, mode 1
        .u32(3) // projection XY
        .u8(u8::from(e.opacity))
        .u8(0)
        .u8(0)
        .u8(0)
        .u8(0)
        .f32(e.threshold)
        .f32(e.blend_max)
        .u32(128)
        .u32(0x7800_0001)
        .u8(1) // procedural
        .bytes(&e.diffuse_scale)
        .u32(0x4000_0010) // normal
        .u32(0)
        .u32(0)
        .u32(0xFF00_0000)
        .u32(0xFFFF_FFFF)
        .f32(8.0)
}

/// The two entries of §14, stored key 1 first (a hash order).
fn worked_example() -> Vec<u8> {
    let e0 = E {
        key: 0,
        label: "grass1",
        draw_order: 10,
        diffuse: 0x4000_0001,
        scale: 0.1,
        rotation: 0.0,
        opacity: false,
        threshold: 0.0,
        blend_max: 0.0,
        diffuse_scale: [0xFF; 4],
    };
    let e1 = E {
        key: 1,
        label: "rocks1",
        draw_order: 20,
        diffuse: 0x4000_0002,
        scale: 0.2,
        rotation: 90.0,
        opacity: true,
        threshold: 0.1,
        blend_max: 0.25,
        diffuse_scale: [0x20, 0x40, 0x80, 0xFF],
    };
    entry(entry(B::new().u32(DID).u32(2), &e1), &e0).build()
}

#[test]
fn worked_example_decodes() {
    let data = worked_example();
    assert_eq!(data.len(), 210, "§14: 8 + 2 · 101 bytes");
    let t = TerrainTypeTable::parse_expecting(&data, DID).unwrap();
    assert_eq!(t.did, Did(DID));
    assert_eq!(t.len(), 2);
    // Stored order is kept; lookup is by key.
    assert_eq!(t.entries()[0].key, 1);
    let e0 = t.get(0).unwrap();
    let e1 = t.get(1).unwrap();
    assert!(t.get(2).is_none());
    assert_eq!(e0.label, "grass1");
    assert_eq!((e0.draw_order, e1.draw_order), (10, 20));
    assert_eq!(e1.diffuse, Did(0x4000_0002));
    assert_eq!((e1.u_scale, e1.v_scale, e1.rotation), (0.2, 0.2, 90.0));
    assert_eq!(e1.u_offset.constant(), Some(0.0));
    assert_eq!(e1.projection, TextureProjection::Xy);
    assert!(e1.use_opacity_mask && !e0.use_opacity_mask);
    assert_eq!((e1.alpha_test_threshold, e1.alpha_blend_max), (0.1, 0.25));
    assert_eq!(e1.mask_resolution, 128);
    assert_eq!(e1.physics_material, Did(0x7800_0001));
    assert!(e1.procedural);
    assert_eq!(e1.normal, Did(0x4000_0010));
    assert_eq!(e1.specular_power, 8.0);
    assert_eq!(t.allows_procedural(1), Some(true));
    assert_eq!(t.allows_procedural(9), None);
    // §3.3 colour example: bytes 0x20, 0x40, 0x80, 0xFF.
    let c = e1.diffuse_scale;
    assert_eq!(c, Argb(0xFF80_4020));
    assert_eq!(
        (c.red(), c.green(), c.blue(), c.alpha()),
        (0x80, 0x40, 0x20, 0xFF)
    );
    let [r, g, b, a] = c.to_rgba_f32();
    assert!((r - 0.502).abs() < 1e-3 && (g - 0.251).abs() < 1e-3);
    assert!((b - 0.125).abs() < 1e-3 && a == 1.0);
    assert_eq!(e1.glow_colour.alpha(), 0xFF);
    // Key order iteration and re-encoding.
    let keys: Vec<u32> = t.iter_by_key().map(|e| e.key).collect();
    assert_eq!(keys, [0, 1]);
    assert_eq!(t.to_bytes(), data);
    assert_eq!(decode_terrain_type_table(&data).unwrap(), t);
}

#[test]
fn every_truncation_and_extension_is_rejected() {
    let data = worked_example();
    for len in 0..data.len() {
        assert!(
            TerrainTypeTable::parse(&data[..len]).is_err(),
            "prefix of {len} bytes accepted"
        );
    }
    let mut long = data.clone();
    long.push(0);
    assert!(matches!(
        TerrainTypeTable::parse(&long),
        Err(FormatError::TrailingBytes { count: 1, .. })
    ));
}

#[test]
fn wrong_prefix_and_did_mismatch() {
    let mut data = worked_example();
    data[3] = 0x18;
    assert!(matches!(
        TerrainTypeTable::parse(&data),
        Err(FormatError::WrongRecordType { .. })
    ));
    let data = worked_example();
    assert!(matches!(
        TerrainTypeTable::parse_expecting(&data, 0x1900_0043u32),
        Err(FormatError::DidMismatch { .. })
    ));
}

/// Offset of entry `i`'s field at entry offset `off` in the worked example (label 6 bytes).
fn at(i: usize, off: usize) -> usize {
    8 + i * 101 + off
}

/// Entry offset of `b` (start of `projection`) with V = 1, L = 6 and mode-1 curves.
const B_OFF: usize = 24 + 1 + 6 + 8 + 8;

#[test]
fn keys_must_be_exactly_0_to_n_minus_1() {
    // Duplicate key: reported at the start of the entry that repeats it.
    let mut data = worked_example();
    data.copy_within(at(0, 0)..at(0, 4), at(1, 0));
    let err = TerrainTypeTable::parse(&data).unwrap_err();
    assert!(
        matches!(err, FormatError::Malformed { offset, .. } if offset == at(1, 0)),
        "{err}"
    );
    // Key outside 0 … n − 1: reported at the start of the offending entry.
    for i in 0..2 {
        let mut data = worked_example();
        data[at(i, 0)..at(i, 4)].copy_from_slice(&2u32.to_le_bytes());
        let err = TerrainTypeTable::parse(&data).unwrap_err();
        assert!(
            matches!(
                err,
                FormatError::IndexOutOfRange {
                    index: 2,
                    limit: 2,
                    offset,
                    ..
                } if offset == at(i, 0)
            ),
            "entry {i}: {err}"
        );
    }
    // The constructor applies the same rule.
    let e = TerrainType {
        key: 1,
        ..TerrainType::default()
    };
    assert!(TerrainTypeTable::new(Did(DID), vec![e.clone()]).is_err());
    assert!(TerrainTypeTable::new(Did(0x1800_0000), vec![]).is_err());
    let ok = TerrainTypeTable::new(
        Did(DID),
        vec![
            e,
            TerrainType {
                key: 0,
                ..TerrainType::default()
            },
        ],
    )
    .unwrap();
    assert_eq!(TerrainTypeTable::parse(&ok.to_bytes()).unwrap(), ok);
}

#[test]
fn projection_bools_and_curve_modes_are_checked() {
    for bad in [0u32, 4] {
        let mut data = worked_example();
        data[at(0, B_OFF)..at(0, B_OFF + 4)].copy_from_slice(&bad.to_le_bytes());
        assert!(
            matches!(
                TerrainTypeTable::parse(&data),
                Err(FormatError::Unsupported { value, .. }) if value == bad
            ),
            "projection {bad}"
        );
    }
    // Each bool8 (b + 4 … b + 8 and b + 25) rejects 2.
    for off in [4, 5, 6, 7, 8, 25] {
        let mut data = worked_example();
        data[at(1, B_OFF + off)] = 2;
        assert!(
            matches!(
                TerrainTypeTable::parse(&data),
                Err(FormatError::InvalidBool { value: 2, .. })
            ),
            "bool at b + {off}"
        );
    }
    // Curve modes outside 1–10 make the length unknown.
    for bad in [0u32, 11, 0xFFFF_FFFF] {
        let mut data = worked_example();
        data[at(0, 31)..at(0, 35)].copy_from_slice(&bad.to_le_bytes());
        assert!(matches!(
            TerrainTypeTable::parse(&data),
            Err(FormatError::Unsupported {
                what: "float curve mode",
                ..
            })
        ));
    }
}

#[test]
fn every_curve_mode_round_trips() {
    let curves = [
        FloatCurve::Constant(0.5),
        FloatCurve::waveform(2, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.25, 0.0, 0.0, 0.0]).unwrap(),
        FloatCurve::Waveform {
            mode: WaveformMode::new(9).unwrap(),
            w: [1.0; 10],
        },
        FloatCurve::Keyframed {
            w: [0.0; 10],
            period: 4.0,
            looping: true,
            keys: vec![
                CurveKey {
                    value: 0.0,
                    position: 0.0,
                },
                CurveKey {
                    value: 1.0,
                    position: 100.0,
                },
            ],
        },
    ];
    let entries: Vec<TerrainType> = curves
        .iter()
        .enumerate()
        .map(|(i, c)| TerrainType {
            key: i as u32,
            label: "t".into(),
            u_offset: c.clone(),
            v_offset: curves[curves.len() - 1 - i].clone(),
            projection: TextureProjection::Yz,
            ..TerrainType::default()
        })
        .collect();
    let t = TerrainTypeTable::new(Did(DID), entries).unwrap();
    let bytes = t.to_bytes();
    let back = TerrainTypeTable::parse(&bytes).unwrap();
    assert_eq!(back, t);
    assert_eq!(back.get(1).unwrap().u_offset.mode(), 2);
    assert_eq!(back.get(3).unwrap().u_offset.mode(), 10);
    assert_eq!(back.get(3).unwrap().u_offset.encoded_len(), 53 + 16);
    let expected: usize = 8 + t
        .entries()
        .iter()
        .map(|e| 4 + 2 + 20 + e.u_offset.encoded_len() + e.v_offset.encoded_len() + 54)
        .sum::<usize>();
    assert_eq!(bytes.len(), expected);
    // A mode-10 loop byte other than 0 or 1 is rejected.
    let pos = bytes
        .windows(4)
        .position(|w| w == 10u32.to_le_bytes())
        .unwrap();
    let mut bad = bytes.clone();
    bad[pos + 4 + 40 + 4] = 2;
    assert!(matches!(
        TerrainTypeTable::parse(&bad),
        Err(FormatError::InvalidBool { value: 2, .. })
    ));
}

#[test]
fn long_labels_use_the_vle_length() {
    let label: String = std::iter::repeat_n('a', 200).collect();
    let t = TerrainTypeTable::new(
        Did(DID),
        vec![TerrainType {
            label: label.clone(),
            ..TerrainType::default()
        }],
    )
    .unwrap();
    let bytes = t.to_bytes();
    assert_eq!(&bytes[12..14], &[0x80, 200], "two-byte VLE");
    assert_eq!(
        TerrainTypeTable::parse(&bytes)
            .unwrap()
            .get(0)
            .unwrap()
            .label,
        label
    );
    // A label length running past the end is truncation, not a panic.
    let mut short = bytes.clone();
    short.truncate(20);
    assert!(matches!(
        TerrainTypeTable::parse(&short),
        Err(FormatError::Truncated { .. })
    ));
}

#[test]
fn hostile_counts_fail_before_allocating() {
    for count in [u32::MAX, 0x0100_0000, 3] {
        let data = B::new().u32(DID).u32(count).bytes(&[0; 200]).build();
        assert!(matches!(
            TerrainTypeTable::parse(&data),
            Err(FormatError::Truncated { .. })
        ));
    }
    // Mode-10 keyframe counts are bounded by the remaining input too.
    let data = B::new()
        .u32(DID)
        .u32(1)
        .u32(0)
        .pascal("x")
        .u32(0)
        .u32(0)
        .f32(1.0)
        .f32(1.0)
        .f32(0.0)
        .u32(10)
        .bytes(&[0; 44])
        .u8(0)
        .u32(u32::MAX)
        .bytes(&[0; 64])
        .build();
    assert!(matches!(
        TerrainTypeTable::parse(&data),
        Err(FormatError::Truncated { .. })
    ));
    assert_eq!(MIN_TERRAIN_TYPE_ENTRY_LEN, 95);
    // An empty table is valid.
    let empty = B::new().u32(DID).u32(0).build();
    assert!(TerrainTypeTable::parse(&empty).unwrap().is_empty());
}

#[test]
fn waveform_modes_are_only_2_to_9() {
    // Regression: a public `mode: u32` let a hand-built table encode bytes that its own
    // decoder rejects or misreads (mode 1 read 4 bytes and took w[1] as the next field).
    for bad in [0u32, 1, 10, 11, 42, u32::MAX] {
        assert_eq!(WaveformMode::new(bad), None, "mode {bad}");
        assert_eq!(
            FloatCurve::waveform(bad, [0.5; CURVE_PARAMS]),
            None,
            "mode {bad}"
        );
    }
    for mode in 2..=9 {
        let c = FloatCurve::waveform(mode, [0.5; CURVE_PARAMS]).unwrap();
        assert_eq!(c.mode(), mode);
        assert_eq!(c.encoded_len(), 44);
        let mut bytes = Vec::new();
        c.write(&mut bytes);
        assert_eq!(bytes.len(), c.encoded_len());
        // Every constructible curve survives a table round trip.
        let t = TerrainTypeTable::new(
            Did(DID),
            vec![TerrainType {
                u_offset: c.clone(),
                v_offset: c.clone(),
                ..TerrainType::default()
            }],
        )
        .unwrap();
        assert_eq!(TerrainTypeTable::parse(&t.to_bytes()).unwrap(), t);
    }
}
