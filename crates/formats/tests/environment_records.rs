// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests for the environment records: sky description (0x15,
//! `docs/specs/environment.md` §5.1), day descriptions (0x1A, §5.2–§5.4), game time (0x35,
//! §6), the LOD-class table (0x32, §9.1) and camera states (0x08, `docs/specs/walker.md`
//! §11.5). Every fixture is built here; no game data.

use tessera_formats::environment::camera::{CAMERA_HEADER_LEN, TARGET_BLOCK_LEN};
use tessera_formats::environment::day::{MIN_KEYFRAME_LEN, MIN_SKY_OBJECT_LEN};
use tessera_formats::environment::{
    Argb, CameraBody, CameraState, CinematicBody, CurveKey, DayDescription, DayGroup, DayPeriod,
    FirstPersonBody, FlightBody, FloatCurve, GameTime, Keyframe, KeyframeEffect,
    KeyframeObjectEntry, ListRef, LodClass, LodClassTable, Month, SkyDescription, SkyEntry,
    SkyLayer, SkyObject, SkyObjectLayers, TargetBlock, ThirdPersonBody, Weighted, WeightedList,
    decode_camera_state, decode_day_description, decode_game_time, decode_lod_class_table,
    decode_sky_description,
};
use tessera_formats::{Did, FormatError, Record};

fn list<T>(key: u32, pairs: &[(T, f32)]) -> WeightedList<T>
where
    T: Copy,
{
    WeightedList {
        key,
        pairs: pairs
            .iter()
            .map(|&(value, weight)| Weighted { value, weight })
            .collect(),
    }
}

fn entry(sky_key: u32, dids: &[(u32, f32)]) -> SkyEntry {
    let pairs: Vec<(Did, f32)> = dids.iter().map(|&(d, w)| (Did(d), w)).collect();
    SkyEntry {
        sky_key,
        list: ListRef::Inline(list(0, &pairs)),
    }
}

fn sky() -> SkyDescription {
    SkyDescription {
        version: 1,
        did: Did(0x1500_0007),
        light_interval_min: 1.0,
        light_interval_max: 1.0,
        sky_interval_min: 180.0,
        sky_interval_max: 180.0,
        game_time: Did(0x3500_0000),
        groups: vec![
            DayGroup {
                day_type: 0x1000_0001,
                chance: 15.0,
                follow: ListRef::Inline(list(0, &[(0x1000_0001, 15.0), (0x1000_0004, 1.0)])),
                entries: vec![entry(2, &[(0x1A00_0001, 10.0)]), entry(0x1000_0100, &[])],
            },
            DayGroup {
                day_type: 0x1000_0004,
                chance: 2.0,
                follow: ListRef::Named(0x1000_0003),
                entries: vec![SkyEntry {
                    sky_key: 0x1000_0200,
                    list: ListRef::Named(0x1000_0001),
                }],
            },
        ],
        named_follow_lists: vec![list(
            0x1000_0003,
            &[(0x1000_0007, 10.0), (0x1000_0002, 10.0)],
        )],
        named_sky_lists: vec![list(
            0x1000_0001,
            &[(Did(0x1A00_002B), 5.0), (Did(0x1A00_0000), 5.0)],
        )],
        local_defaults: vec![
            entry(1, &[(0x1A00_0003, 10.0)]),
            entry(2, &[(0x1A00_0004, 2.0), (0x1A00_0005, 1.0)]),
        ],
    }
}

/// Every proper prefix of `bytes` must fail without panicking.
fn every_prefix_fails<T>(bytes: &[u8], decode: impl Fn(&[u8]) -> Result<T, FormatError>) {
    for n in 0..bytes.len() {
        assert!(decode(&bytes[..n]).is_err(), "prefix of {n} bytes decoded");
    }
}

/// A trailing byte must fail with `TrailingBytes`.
fn trailing_fails<T: std::fmt::Debug>(
    bytes: &[u8],
    decode: impl Fn(&[u8]) -> Result<T, FormatError>,
) {
    let mut b = bytes.to_vec();
    b.push(0);
    assert!(matches!(
        decode(&b),
        Err(FormatError::TrailingBytes { count: 1, .. })
    ));
}

#[test]
fn sky_description_round_trip() {
    let s = sky();
    let bytes = s.to_bytes();
    let back = SkyDescription::parse_expecting(&bytes, 0x1500_0007).unwrap();
    assert_eq!(back, s);
    assert_eq!(back.to_bytes(), bytes);
    every_prefix_fails(&bytes, decode_sky_description);
    trailing_fails(&bytes, decode_sky_description);
}

#[test]
fn sky_description_lookups() {
    let s = sky();
    // §11.2 step 1: the day group's entry.
    let e = s.sky_entry(0x1000_0001, 2).unwrap();
    let l = s.resolve_sky_list(&e.list).unwrap();
    assert_eq!(l.values().collect::<Vec<_>>(), [Did(0x1A00_0001)]);
    // Step 2: no group for the day type (2, the client's initial value) → local default.
    let e = s.sky_entry(2, 2).unwrap();
    let l = s.resolve_sky_list(&e.list).unwrap();
    assert_eq!(l.total_weight(), 3.0);
    // Group exists but has no entry for the key → local default.
    let e = s.sky_entry(0x1000_0001, 1).unwrap();
    assert_eq!(
        s.resolve_sky_list(&e.list).unwrap().pairs[0].value,
        Did(0x1A00_0003)
    );
    // Step 3: neither.
    assert!(s.sky_entry(0x1000_0001, 99).is_none());
    // Named references resolve; a dangling one does not.
    let named = s.sky_entry(0x1000_0004, 0x1000_0200).unwrap();
    assert_eq!(s.resolve_sky_list(&named.list).unwrap().pairs.len(), 2);
    assert!(s.resolve_sky_list(&ListRef::Named(7)).is_none());
    let g = s.group(0x1000_0004).unwrap();
    assert_eq!(s.resolve_follow_list(&g.follow).unwrap().key, 0x1000_0003);
    assert!(s.resolve_follow_list(&ListRef::Named(1)).is_none());
    // Every DID (inline group, inline default, named list), in stored order.
    let all: Vec<u32> = s.day_descriptions().map(|d| d.0).collect();
    assert_eq!(
        all,
        [
            0x1A00_0001,
            0x1A00_0003,
            0x1A00_0004,
            0x1A00_0005,
            0x1A00_002B,
            0x1A00_0000
        ]
    );
}

#[test]
fn sky_description_hostile() {
    let bytes = sky().to_bytes();
    // §13: version other than 1.
    let mut b = bytes.clone();
    b[0] = 2;
    assert!(matches!(
        decode_sky_description(&b),
        Err(FormatError::Unsupported {
            value: 2,
            offset: 0,
            ..
        })
    ));
    // Wrong record type.
    let mut b = bytes.clone();
    b[7] = 0x1A;
    assert!(matches!(
        decode_sky_description(&b),
        Err(FormatError::WrongRecordType { .. })
    ));
    // `follow_is_named` ≥ 2 (first group: offset 4 + 4 + 32 + 4 + 4 + 4 + 4 = 56).
    let mut b = bytes.clone();
    assert_eq!(b[56], 0);
    b[56] = 2;
    assert!(matches!(
        decode_sky_description(&b),
        Err(FormatError::InvalidBool { offset: 56, .. })
    ));
    // A group count larger than the bytes left.
    let mut b = bytes.clone();
    b[44..48].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        decode_sky_description(&b),
        Err(FormatError::Truncated { .. })
    ));
    // DID mismatch.
    assert!(matches!(
        SkyDescription::parse_expecting(&bytes, 0x1500_0001),
        Err(FormatError::DidMismatch { .. })
    ));
}

#[test]
fn sky_description_vle_counts() {
    // Named-list counts are VLEs: a non-shortest 0xE0 form decodes too, and re-encodes
    // shorter.
    let mut s = sky();
    s.named_follow_lists.clear();
    s.named_sky_lists.clear();
    let short = s.to_bytes();
    let mut long = Vec::new();
    // Header and groups up to the first VLE count: everything before the two 0 bytes that
    // precede the default count.
    let defaults_len = 4 + s
        .local_defaults
        .iter()
        .map(|e| {
            let ListRef::Inline(l) = &e.list else {
                unreachable!()
            };
            4 + 1 + 4 + 4 + 8 * l.pairs.len()
        })
        .sum::<usize>();
    let split = short.len() - defaults_len - 2;
    assert_eq!(&short[split..split + 2], &[0, 0]);
    long.extend_from_slice(&short[..split]);
    long.extend_from_slice(&[0xE0, 0, 0, 0, 0, 0x80, 0x00]);
    long.extend_from_slice(&short[split + 2..]);
    let back = decode_sky_description(&long).unwrap();
    assert_eq!(back, s);
    assert_eq!(back.to_bytes(), short);
}

fn keyframe(time: f32, effect: u32) -> Keyframe {
    Keyframe {
        time,
        fog_near: 200.0,
        fog_far: 2500.0,
        fog_extra: 0.5,
        fog_colour: Argb(0xFF80_90A0),
        sun_brightness: 0.55,
        sun_heading: 15.0,
        sun_pitch: 150.0,
        sun_colour: Argb(0xFFFF_EEDD),
        sun_specular: Argb(0xFF80_8080),
        ambient_brightness: 0.2,
        ambient_colour_1: Argb(0xFF40_4040),
        ambient_colour_2: Argb(0xFF40_4040),
        colour_3: Argb(0),
        object_entries: vec![KeyframeObjectEntry {
            object_index: 3,
            replacement: Did(0),
            angle_b_offset: 0.0,
            angle_a_offset: 0.0,
            tint: Argb(0x80FF_FFFF),
        }],
        curves: [
            FloatCurve::Constant(1.0),
            FloatCurve::waveform(4, [0.5; 10]).unwrap(),
            FloatCurve::Keyframed {
                w: [0.0; 10],
                period: 10.0,
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
            FloatCurve::Constant(0.0),
        ],
        values_a: [0.033, 1.0, 2.0, 0.07, 0.6, 0.03],
        value_b: 0.0,
        effect: (effect != 0).then_some(KeyframeEffect {
            object: Did(effect),
            extra: 0,
        }),
        colour_4: Argb(0xFFFF_FFFF),
    }
}

fn sky_object(index: u32, layers: usize) -> SkyObject {
    SkyObject {
        index,
        object: Did(0x4700_0100 + index),
        begin_time: 0.0,
        end_time: 1.0,
        angle_a_begin: -90.0,
        angle_a_end: 270.0,
        angle_b_begin: 75.0,
        angle_b_end: 75.0,
        reverse: index.is_multiple_of(2),
        layers: SkyObjectLayers {
            value_a: if layers == 0 { 1.0 } else { 6.0 },
            value_b: if layers == 0 { 1.0 } else { 0.6 },
            layers: (0..layers)
                .map(|i| SkyLayer {
                    key: -1.0 + 0.2 * i as f32,
                    texture: Did(0x4000_0000 + i as u32),
                    value: 0.5,
                })
                .collect(),
            flag: true,
            value: 0.06,
        },
        flag_31: true,
        ..SkyObject::default()
    }
}

fn day() -> DayDescription {
    DayDescription {
        unknown0: 0,
        did: Did(0x1A00_0042),
        keyframes: vec![
            keyframe(0.0, 0),
            keyframe(0.2, 0x4700_0500),
            keyframe(0.31, 0),
            keyframe(1.0, 0),
        ],
        objects: vec![sky_object(3, 0), sky_object(4, 9)],
        transition_seconds: 2.0,
    }
}

#[test]
fn day_description_round_trip() {
    let d = day();
    let bytes = d.to_bytes();
    let back = DayDescription::parse_expecting(&bytes, 0x1A00_0042).unwrap();
    assert_eq!(back, d);
    assert_eq!(back.to_bytes(), bytes);
    assert!(back.keyframes_ascending());
    assert!(back.object_indices_unique());
    assert_eq!(back.object(4).unwrap().layers.layers.len(), 9);
    assert!(back.object(5).is_none());
    assert_eq!(
        back.keyframes[0].object_entry(3).unwrap().tint.alpha(),
        0x80
    );
    // EV4: the extra u32 follows exactly the non-zero effect.
    assert_eq!(
        back.keyframes[1].effect,
        Some(KeyframeEffect {
            object: Did(0x4700_0500),
            extra: 0
        })
    );
    assert_eq!(back.keyframes[0].effect, None);
    every_prefix_fails(&bytes, decode_day_description);
    trailing_fails(&bytes, decode_day_description);
}

#[test]
fn day_description_minimum_sizes() {
    let mut k = keyframe(0.5, 0);
    k.object_entries.clear();
    k.curves = Default::default();
    let d = DayDescription {
        keyframes: vec![k],
        objects: vec![sky_object(1, 0)],
        did: Did(0x1A00_0000),
        ..DayDescription::default()
    };
    let bytes = d.to_bytes();
    assert_eq!(
        bytes.len(),
        8 + 4 + MIN_KEYFRAME_LEN + 4 + MIN_SKY_OBJECT_LEN + 4
    );
    assert_eq!(decode_day_description(&bytes).unwrap(), d);
}

#[test]
fn day_description_hostile() {
    let d = DayDescription {
        did: Did(0x1A00_0001),
        keyframes: vec![keyframe(0.0, 0)],
        ..DayDescription::default()
    };
    let bytes = d.to_bytes();
    // The first curve's mode is at 12 + 56 + 4 + 20 = 92.
    let curve_at = 12 + 56 + 4 + 20;
    assert_eq!(&bytes[curve_at..curve_at + 4], &1u32.to_le_bytes());
    for mode in [0u32, 11, u32::MAX] {
        let mut b = bytes.clone();
        b[curve_at..curve_at + 4].copy_from_slice(&mode.to_le_bytes());
        assert!(matches!(
            decode_day_description(&b),
            Err(FormatError::Unsupported { value, .. }) if value == mode
        ));
    }
    // A keyframe count larger than the bytes left.
    let mut b = bytes.clone();
    b[8..12].copy_from_slice(&1000u32.to_le_bytes());
    assert!(matches!(
        decode_day_description(&b),
        Err(FormatError::Truncated { .. })
    ));
    // A sky-object flag ≥ 2.
    let d = DayDescription {
        did: Did(0x1A00_0001),
        objects: vec![sky_object(1, 0)],
        ..DayDescription::default()
    };
    let bytes = d.to_bytes();
    // flag_25 at 16 + 4 + 4 + 24 + 4 = 52.
    let mut b = bytes.clone();
    b[52] = 2;
    assert!(matches!(
        decode_day_description(&b),
        Err(FormatError::InvalidBool { offset: 52, .. })
    ));
    // Wrong prefix.
    let mut b = bytes;
    b[7] = 0x15;
    assert!(matches!(
        decode_day_description(&b),
        Err(FormatError::WrongRecordType { .. })
    ));
}

#[test]
fn day_description_invariant_helpers() {
    let mut d = day();
    d.keyframes.swap(1, 2);
    assert!(!d.keyframes_ascending());
    d.objects[1].index = 3;
    assert!(!d.object_indices_unique());
}

fn game_time() -> GameTime {
    GameTime {
        did: Did(0x3500_0000),
        day_length: 11_160.0,
        days_per_year: 360,
        zero_offset: 3600.0,
        zero_year: 34,
        value_54: 0.8,
        value_58: 0.25,
        periods: [0.0, 0.25, 0.5, 0.75]
            .iter()
            .enumerate()
            .map(|(i, &start)| DayPeriod {
                start,
                name: 100 + i as u32,
            })
            .collect(),
        weekdays: vec![200],
        months: (0..12)
            .map(|i| Month {
                start_day: 30 * i,
                name: 300 + i,
            })
            .collect(),
        unknown_last: 7,
    }
}

#[test]
fn game_time_round_trip() {
    let g = game_time();
    let bytes = g.to_bytes();
    // §6: 180 bytes with 4 periods, 1 weekday and 12 months.
    assert_eq!(bytes.len(), 180);
    let back = GameTime::parse_expecting(&bytes, 0x3500_0000).unwrap();
    assert_eq!(back, g);
    assert!(back.calendar_is_ordered());
    every_prefix_fails(&bytes, decode_game_time);
    trailing_fails(&bytes, decode_game_time);

    let mut bad = g.clone();
    bad.periods[3].start = 1.0;
    assert!(!bad.calendar_is_ordered());
    let mut bad = g;
    bad.months.swap(0, 1);
    assert!(!bad.calendar_is_ordered());
}

#[test]
fn lod_class_table_round_trip() {
    let t = LodClassTable {
        did: Did(0x3200_0000),
        entries: (0..32)
            .map(|i| LodClass {
                class: 0x1000_0000 + i,
                values: [
                    25.0 * (i + 1) as f32,
                    2.0,
                    2.0,
                    80.0,
                    1000.0,
                    1000.0,
                    -1.0,
                    1100.0,
                ],
            })
            .collect(),
    };
    let bytes = t.to_bytes();
    // §9.1: 4 + 4 + 32 × 36.
    assert_eq!(bytes.len(), 1160);
    let back = LodClassTable::parse_expecting(&bytes, 0x3200_0000).unwrap();
    assert_eq!(back, t);
    assert_eq!(back.len(), 32);
    assert_eq!(back.get(0x1000_0001).unwrap().max_distance(), 50.0);
    assert!(back.get(7).is_none());
    every_prefix_fails(&bytes, decode_lod_class_table);
    trailing_fails(&bytes, decode_lod_class_table);
    let mut b = bytes;
    b[4..8].copy_from_slice(&33u32.to_le_bytes());
    assert!(matches!(
        decode_lod_class_table(&b),
        Err(FormatError::Truncated { .. })
    ));
}

fn target() -> TargetBlock {
    TargetBlock {
        unresolved: [0; 8],
        holding_location: 0,
        bone: 0,
        offset: [0.0, 0.2, 0.75],
        yaw_offset: 0.0,
        pitch_offset: 0.0,
        height_factor: 0.925,
        height_factor_under_water: 0.0,
    }
}

fn camera(body: CameraBody) -> CameraState {
    CameraState {
        entity: Did(0x4700_0002),
        collision_smoothing: 0.15,
        near_clip: 0.1,
        far_clip: 2000.0,
        uses_physics: true,
        uses_water_physics: false,
        follows_under_water: true,
        auto_transitions_disabled: false,
        body,
    }
}

#[test]
fn camera_states_round_trip() {
    let bodies = [
        (
            CameraBody::ThirdPerson(ThirdPersonBody {
                angle_smoothing: 0.08,
                zoom_smoothing: 0.4,
                target_smoothing: 0.08,
                unnamed: [0.07, 0.0],
                target: target(),
                constrain_pitch: true,
                pitch_high: 60.0,
                pitch_low: -80.0,
                min_distance: 1.0,
                max_distance: 20.0,
                fov: 45.0,
                initial_pitch: -15.0,
                initial_distance: 6.0,
                align_to_slope: true,
                target_fade_distance: 0.7,
                target_fade_opacity: 0.1,
                look_at_smoothing: 0.5,
                ..ThirdPersonBody::default()
            }),
            161,
        ),
        (
            CameraBody::FirstPerson(FirstPersonBody {
                fov: 60.0,
                constrain_pitch: true,
                pitch_high: 90.0,
                pitch_low: -90.0,
                target: target(),
                initial_yaw: 0.0,
                look_from_smoothing: 0.5,
            }),
            89,
        ),
        (
            CameraBody::Cinematic(CinematicBody {
                fov: 65.0,
                target_smoothing: 0.04,
                angle_smoothing: 0.22,
            }),
            36,
        ),
        (
            CameraBody::Flight(FlightBody {
                fov: 50.0,
                constrain_pitch: true,
                pitch_high: 90.0,
                pitch_low: -90.0,
                constrain_roll: true,
                roll_left: -89.0,
                roll_right: 89.0,
                target: target(),
                ..FlightBody::default()
            }),
            94,
        ),
    ];
    for (i, (body, size)) in bodies.into_iter().enumerate() {
        let c = camera(body);
        let bytes = c.to_bytes();
        assert_eq!(bytes.len(), size, "type {}", i + 1);
        assert_eq!(c.encoded_len(), size);
        assert_eq!(&bytes[20..24], &(i as u32 + 1).to_le_bytes());
        let back = CameraState::parse(&bytes).unwrap();
        assert_eq!(back, c);
        assert_eq!(back.body.controller_type(), i as u32 + 1);
        assert_eq!(back.body.target().is_some(), i != 2);
        every_prefix_fails(&bytes, decode_camera_state);
        trailing_fails(&bytes, decode_camera_state);
    }
    assert_eq!(CAMERA_HEADER_LEN, 24);
    assert_eq!(TARGET_BLOCK_LEN, 44);
}

#[test]
fn camera_state_hostile() {
    let bytes = camera(CameraBody::Cinematic(CinematicBody::default())).to_bytes();
    for t in [0u32, 5, u32::MAX] {
        let mut b = bytes.clone();
        b[20..24].copy_from_slice(&t.to_le_bytes());
        assert!(matches!(
            decode_camera_state(&b),
            Err(FormatError::Unsupported { value, offset: 20, .. }) if value == t
        ));
    }
    let mut b = bytes;
    b[17] = 2;
    assert!(matches!(
        decode_camera_state(&b),
        Err(FormatError::InvalidBool { offset: 17, .. })
    ));
}

// ---------------------------------------------------------------------------------------
// Hand-built records in spec order. The round trips above cannot see a symmetric reorder of
// same-typed fields in a reader and its writer; these fixtures are written byte by byte from
// the spec tables, with a distinct value per field, and every decoded field is asserted.
// ---------------------------------------------------------------------------------------

/// Little-endian byte builder that checks each field starts at the offset the spec gives.
#[derive(Default)]
struct Bytes(Vec<u8>);

impl Bytes {
    fn at(&mut self, at: usize) -> &mut Self {
        assert_eq!(self.0.len(), at, "field offset");
        self
    }
    fn u32(&mut self, at: usize, v: u32) -> &mut Self {
        self.at(at).0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn f32(&mut self, at: usize, v: f32) -> &mut Self {
        self.at(at).0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn f64(&mut self, at: usize, v: f64) -> &mut Self {
        self.at(at).0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn u8(&mut self, at: usize, v: u8) -> &mut Self {
        self.at(at).0.push(v);
        self
    }
    fn raw(&mut self, at: usize, v: &[u8]) -> &mut Self {
        self.at(at).0.extend_from_slice(v);
        self
    }
}

/// Bit `k` of `mask` as a stored bool byte. Every hand-built test runs once with all flags
/// clear and once per flag with only that flag set, so that two swapped flags show up.
fn flag(mask: u32, k: u32) -> u8 {
    u8::from((mask >> k) & 1 == 1)
}

fn one_hot(n: u32) -> impl Iterator<Item = u32> {
    std::iter::once(0).chain((0..n).map(|k| 1 << k))
}

#[test]
fn sky_description_default_encodes() {
    // A description built from `Default` carries version 1 (§5.1.1), so its bytes decode.
    let s = SkyDescription {
        did: Did(0x1500_0007),
        ..SkyDescription::default()
    };
    assert_eq!(s.version, 1);
    let back = decode_sky_description(&s.to_bytes()).unwrap();
    assert_eq!(back, s);
}

#[test]
fn sky_description_hand_built() {
    // §5.1.1–§5.1.4 in stored order: one group with an inline follow list and a named sky
    // entry, one named follow list, one named sky list, one local default with an inline
    // list.
    let mut b = Bytes::default();
    b.u32(0, 1)
        .u32(4, 0x1500_0007)
        .f64(8, 1.5)
        .f64(16, 2.5)
        .f64(24, 180.0)
        .f64(32, 181.0)
        .u32(40, 0x3500_0000)
        .u32(44, 1)
        // Day group.
        .u32(48, 0x1000_0001)
        .f32(52, 15.0)
        .u8(56, 0)
        .u32(57, 0x77)
        .u32(61, 1)
        .u32(65, 0x1000_0004)
        .f32(69, 3.0)
        .u32(73, 1)
        .u32(77, 0x1000_0200)
        .u8(81, 1)
        .u32(82, 0x1000_0001)
        // Named follow lists (VLE count 1).
        .u8(86, 1)
        .u32(87, 0x1000_0003)
        .u32(91, 1)
        .u32(95, 0x1000_0007)
        .f32(99, 10.0)
        // Named sky lists.
        .u8(103, 1)
        .u32(104, 0x1000_0001)
        .u32(108, 1)
        .u32(112, 0x1A00_002B)
        .f32(116, 5.0)
        // Local defaults.
        .u32(120, 1)
        .u32(124, 2)
        .u8(128, 0)
        .u32(129, 0)
        .u32(133, 1)
        .u32(137, 0x1A00_0004)
        .f32(141, 2.0);
    let bytes = b.0;
    assert_eq!(bytes.len(), 145);
    let s = decode_sky_description(&bytes).unwrap();
    assert_eq!(s.version, 1);
    assert_eq!(s.did, Did(0x1500_0007));
    assert_eq!(
        (
            s.light_interval_min,
            s.light_interval_max,
            s.sky_interval_min,
            s.sky_interval_max
        ),
        (1.5, 2.5, 180.0, 181.0)
    );
    assert_eq!(s.game_time, Did(0x3500_0000));
    assert_eq!(s.groups.len(), 1);
    let g = &s.groups[0];
    assert_eq!((g.day_type, g.chance), (0x1000_0001, 15.0));
    assert_eq!(g.follow, ListRef::Inline(list(0x77, &[(0x1000_0004, 3.0)])));
    assert_eq!(
        g.entries,
        [SkyEntry {
            sky_key: 0x1000_0200,
            list: ListRef::Named(0x1000_0001)
        }]
    );
    assert_eq!(
        s.named_follow_lists,
        [list(0x1000_0003, &[(0x1000_0007, 10.0)])]
    );
    assert_eq!(
        s.named_sky_lists,
        [list(0x1000_0001, &[(Did(0x1A00_002B), 5.0)])]
    );
    assert_eq!(s.local_defaults, [entry(2, &[(0x1A00_0004, 2.0)])]);
    assert_eq!(s.to_bytes(), bytes);
}

#[test]
fn sky_description_hostile_counts_and_flags() {
    let s = sky();
    let bytes = s.to_bytes();
    // First group: follow flag at 56, inline key 57, count 61, two pairs 65..81, entry count
    // 81, first sky entry key 85, its `list_is_named` at 89.
    assert_eq!(&bytes[81..85], &2u32.to_le_bytes());
    assert_eq!(bytes[89], 0);
    let mut b = bytes.clone();
    b[89] = 2;
    assert!(matches!(
        decode_sky_description(&b),
        Err(FormatError::InvalidBool { offset: 89, .. })
    ));
    // An over-large sky-entry count in the first group.
    let mut b = bytes.clone();
    b[81..85].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        decode_sky_description(&b),
        Err(FormatError::Truncated { .. })
    ));
    // An over-large inline weighted-list count (the first follow list's).
    let mut b = bytes.clone();
    b[61..65].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        decode_sky_description(&b),
        Err(FormatError::Truncated { .. })
    ));
    // An over-large local-default count: the last 4 + 21 + 29 bytes are the count and the
    // two default entries of `sky()`.
    let at = bytes.len() - 54;
    assert_eq!(&bytes[at..at + 4], &2u32.to_le_bytes());
    let mut b = bytes.clone();
    b[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        decode_sky_description(&b),
        Err(FormatError::Truncated { .. })
    ));

    // An over-large VLE named-list count (0xE0 form, u32::MAX), for each of the two tables.
    let mut s = sky();
    s.named_follow_lists.clear();
    s.named_sky_lists.clear();
    let short = s.to_bytes();
    let split = short.len() - 54 - 2;
    assert_eq!(&short[split..split + 2], &[0, 0]);
    for which in 0..2 {
        let mut b = short[..split + which].to_vec();
        b.extend_from_slice(&[0xE0, 0xFF, 0xFF, 0xFF, 0xFF]);
        b.extend_from_slice(&short[split + which + 1..]);
        assert!(
            matches!(
                decode_sky_description(&b),
                Err(FormatError::Truncated { .. })
            ),
            "named-list table {which}"
        );
    }
}

#[test]
fn game_time_hostile() {
    let bytes = game_time().to_bytes();
    // §6: period count at 32, weekday count at 32 + 4 + 8·4 = 68, month count at 68 + 4 + 4
    // = 76.
    for (at, n) in [(32usize, 4u32), (68, 1), (76, 12)] {
        assert_eq!(&bytes[at..at + 4], &n.to_le_bytes(), "count at {at}");
        let mut b = bytes.clone();
        b[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(
            matches!(decode_game_time(&b), Err(FormatError::Truncated { .. })),
            "count at {at}"
        );
    }
    // Wrong prefix (0x15 is the sky description).
    let mut b = bytes.clone();
    b[3] = 0x15;
    assert!(matches!(
        decode_game_time(&b),
        Err(FormatError::WrongRecordType { .. })
    ));
    assert!(matches!(
        GameTime::parse_expecting(&bytes, 0x3500_0001),
        Err(FormatError::DidMismatch { .. })
    ));
}

#[test]
fn game_time_hand_built() {
    // §6 in stored order, one value per field.
    let mut b = Bytes::default();
    b.u32(0, 0x3500_0000)
        .f32(4, 11_160.0)
        .u32(8, 360)
        .f64(12, 3600.5)
        .u32(20, 34)
        .f32(24, 0.8)
        .f32(28, 0.25)
        .u32(32, 2)
        .f32(36, 0.0)
        .u32(40, 101)
        .f32(44, 0.5)
        .u32(48, 102)
        .u32(52, 1)
        .u32(56, 201)
        .u32(60, 2)
        .u32(64, 0)
        .u32(68, 301)
        .u32(72, 30)
        .u32(76, 302)
        .u32(80, 77);
    let bytes = b.0;
    let g = decode_game_time(&bytes).unwrap();
    assert_eq!(g.day_length, 11_160.0);
    assert_eq!(g.days_per_year, 360);
    assert_eq!(g.zero_offset, 3600.5);
    assert_eq!(g.zero_year, 34);
    assert_eq!((g.value_54, g.value_58), (0.8, 0.25));
    assert_eq!(
        g.periods,
        [
            DayPeriod {
                start: 0.0,
                name: 101
            },
            DayPeriod {
                start: 0.5,
                name: 102
            }
        ]
    );
    assert_eq!(g.weekdays, [201]);
    assert_eq!(
        g.months,
        [
            Month {
                start_day: 0,
                name: 301
            },
            Month {
                start_day: 30,
                name: 302
            }
        ]
    );
    assert_eq!(g.unknown_last, 77);
    assert_eq!(g.to_bytes(), bytes);
}

/// A day description with one keyframe and one sky object, written from the tables of
/// §5.2–§5.4 with a distinct value per field; the sky object's six flags follow `mask`.
fn hand_built_day(mask: u32) -> Vec<u8> {
    let mut b = Bytes::default();
    // §5.2 fields 1–3.
    b.u32(0, 0).u32(4, 0x1A00_0042).u32(8, 1);
    // §5.3 keyframe at 12.
    let k = 12;
    b.f32(k, 0.25)
        .f32(k + 4, 1.0)
        .f32(k + 8, 2.0)
        .f32(k + 12, 3.0)
        .u32(k + 16, 0xFF01_0203)
        .f32(k + 20, 4.0)
        .f32(k + 24, 5.0)
        .f32(k + 28, 6.0)
        .u32(k + 32, 0xFF04_0506)
        .u32(k + 36, 0xFF07_0809)
        .f32(k + 40, 7.0)
        .u32(k + 44, 0xFF0A_0B0C)
        .u32(k + 48, 0xFF0D_0E0F)
        .u32(k + 52, 0x1011_1213)
        .u32(k + 56, 1)
        // §5.3.1 object entry.
        .u32(k + 60, 9)
        .u32(k + 64, 0x4700_0077)
        .f32(k + 68, 8.0)
        .f32(k + 72, 9.0)
        .u32(k + 76, 0x8014_1516);
    // Four mode-1 curves (`material.md` §5.3: u32 mode, f32 value).
    let mut at = k + 80;
    for v in [10.0, 11.0, 12.0, 13.0] {
        b.u32(at, 1).f32(at + 4, v);
        at += 8;
    }
    for (i, v) in [14.0, 15.0, 16.0, 17.0, 18.0, 19.0].into_iter().enumerate() {
        b.f32(at + 4 * i, v);
    }
    at += 24;
    b.f32(at, 0.5)
        .u32(at + 4, 0x4700_0500)
        .u32(at + 8, 21)
        .u32(at + 12, 0xFFFF_FFFE);
    at += 16;
    assert_eq!(at, 12 + MIN_KEYFRAME_LEN + 20 + 4);
    // §5.2 field 5 and the §5.4 sky object.
    b.u32(at, 1);
    let o = at + 4;
    b.u32(o, 3)
        .u32(o + 4, 0x4700_0103)
        .f32(o + 8, 0.125)
        .f32(o + 12, 0.875)
        .f32(o + 16, 30.0)
        .f32(o + 20, 31.0)
        .f32(o + 24, 32.0)
        .f32(o + 28, 33.0)
        .u32(o + 32, 34)
        .u8(o + 36, flag(mask, 0))
        .u8(o + 37, flag(mask, 1))
        .u8(o + 38, flag(mask, 2))
        .f32(o + 39, 35.0)
        .f32(o + 43, 36.0)
        .u8(o + 47, flag(mask, 3))
        .f32(o + 48, 6.0)
        .f32(o + 52, 0.6)
        .u32(o + 56, 1)
        .f32(o + 60, -1.0)
        .u32(o + 64, 0x4000_0001)
        .f32(o + 68, 37.0)
        .u8(o + 72, flag(mask, 4))
        .f32(o + 73, 0.06)
        .u8(o + 77, flag(mask, 5));
    assert_eq!(o + 78, o + MIN_SKY_OBJECT_LEN + 12);
    // §5.2 field 7.
    b.f32(o + 78, 2.0);
    b.0
}

#[test]
fn day_description_hand_built_field_order() {
    for mask in one_hot(6) {
        let bytes = hand_built_day(mask);
        let d = decode_day_description(&bytes).unwrap();
        assert_eq!(d.unknown0, 0);
        assert_eq!(d.did, Did(0x1A00_0042));
        assert_eq!(d.transition_seconds, 2.0);
        assert_eq!(d.keyframes.len(), 1);
        let k = &d.keyframes[0];
        assert_eq!(k.time, 0.25);
        assert_eq!(k.fog_near, 1.0);
        assert_eq!(k.fog_far, 2.0);
        assert_eq!(k.fog_extra, 3.0);
        assert_eq!(k.fog_colour, Argb(0xFF01_0203));
        assert_eq!(k.sun_brightness, 4.0);
        assert_eq!(k.sun_heading, 5.0);
        assert_eq!(k.sun_pitch, 6.0);
        assert_eq!(k.sun_colour, Argb(0xFF04_0506));
        assert_eq!(k.sun_specular, Argb(0xFF07_0809));
        assert_eq!(k.ambient_brightness, 7.0);
        assert_eq!(k.ambient_colour_1, Argb(0xFF0A_0B0C));
        assert_eq!(k.ambient_colour_2, Argb(0xFF0D_0E0F));
        assert_eq!(k.colour_3, Argb(0x1011_1213));
        assert_eq!(
            k.object_entries,
            [KeyframeObjectEntry {
                object_index: 9,
                replacement: Did(0x4700_0077),
                angle_b_offset: 8.0,
                angle_a_offset: 9.0,
                tint: Argb(0x8014_1516),
            }]
        );
        assert_eq!(
            k.curves,
            [
                FloatCurve::Constant(10.0),
                FloatCurve::Constant(11.0),
                FloatCurve::Constant(12.0),
                FloatCurve::Constant(13.0)
            ]
        );
        assert_eq!(k.values_a, [14.0, 15.0, 16.0, 17.0, 18.0, 19.0]);
        assert_eq!(k.value_b, 0.5);
        assert_eq!(
            k.effect,
            Some(KeyframeEffect {
                object: Did(0x4700_0500),
                extra: 21
            })
        );
        assert_eq!(k.colour_4, Argb(0xFFFF_FFFE));

        assert_eq!(d.objects.len(), 1);
        let o = &d.objects[0];
        assert_eq!(o.index, 3);
        assert_eq!(o.object, Did(0x4700_0103));
        assert_eq!((o.begin_time, o.end_time), (0.125, 0.875));
        assert_eq!((o.angle_a_begin, o.angle_a_end), (30.0, 31.0));
        assert_eq!((o.angle_b_begin, o.angle_b_end), (32.0, 33.0));
        assert_eq!(o.unknown_20, 34);
        let flags = [
            o.flag_25,
            o.flag_26,
            o.flag_30,
            o.reverse,
            o.layers.flag,
            o.flag_31,
        ];
        for (k, f) in flags.into_iter().enumerate() {
            assert_eq!(f, flag(mask, k as u32) == 1, "flag {k}, mask {mask:#b}");
        }
        assert_eq!((o.value_28, o.value_2c), (35.0, 36.0));
        assert_eq!((o.layers.value_a, o.layers.value_b), (6.0, 0.6));
        assert_eq!(
            o.layers.layers,
            [SkyLayer {
                key: -1.0,
                texture: Did(0x4000_0001),
                value: 37.0
            }]
        );
        assert_eq!(o.layers.value, 0.06);
        assert_eq!(d.to_bytes(), bytes);
    }
}

#[test]
fn keyframe_effect_with_zero_object_encodes_as_none() {
    // EV4: a zero effect DID has no extra value. An effect whose object is 0 cannot be
    // stored, so it is written as "no effect" and the bytes still decode.
    let mut k = keyframe(0.5, 0);
    k.effect = Some(KeyframeEffect {
        object: Did(0),
        extra: 5,
    });
    let d = DayDescription {
        did: Did(0x1A00_0001),
        keyframes: vec![k],
        ..DayDescription::default()
    };
    let back = decode_day_description(&d.to_bytes()).unwrap();
    assert_eq!(back.keyframes[0].effect, None);
    let mut expected = d;
    expected.keyframes[0].effect = None;
    assert_eq!(back, expected);
}

/// Writes a target block (§11.5.2) at `at` with values derived from `seed`; returns the
/// expected decode.
fn hand_built_target(b: &mut Bytes, at: usize, seed: f32, zero_head: bool) -> TargetBlock {
    let head: [u8; 8] = if zero_head {
        [0; 8]
    } else {
        [1, 2, 3, 4, 5, 6, 7, 8]
    };
    b.raw(at, &head)
        .u32(at + 8, 11)
        .u32(at + 12, 12)
        .f32(at + 16, seed + 0.25)
        .f32(at + 20, seed + 0.5)
        .f32(at + 24, seed + 0.75)
        .f32(at + 28, seed + 1.0)
        .f32(at + 32, seed + 1.25)
        .f32(at + 36, seed + 1.5)
        .f32(at + 40, seed + 1.75);
    TargetBlock {
        unresolved: head,
        holding_location: 11,
        bone: 12,
        offset: [seed + 0.25, seed + 0.5, seed + 0.75],
        yaw_offset: seed + 1.0,
        pitch_offset: seed + 1.25,
        height_factor: seed + 1.5,
        height_factor_under_water: seed + 1.75,
    }
}

/// The 24-byte header of §11.5.1; its four flags follow bits 0–3 of `mask`.
fn hand_built_camera_header(b: &mut Bytes, mask: u32, controller: u32) {
    b.u32(0, 0x4700_0002)
        .f32(4, 0.15)
        .f32(8, 0.1)
        .f32(12, 2000.0)
        .u8(16, flag(mask, 0))
        .u8(17, flag(mask, 1))
        .u8(18, flag(mask, 2))
        .u8(19, flag(mask, 3))
        .u32(20, controller);
}

fn check_camera_header(c: &CameraState, mask: u32) {
    assert_eq!(c.entity, Did(0x4700_0002));
    assert_eq!(
        (c.collision_smoothing, c.near_clip, c.far_clip),
        (0.15, 0.1, 2000.0)
    );
    let flags = [
        c.uses_physics,
        c.uses_water_physics,
        c.follows_under_water,
        c.auto_transitions_disabled,
    ];
    for (k, f) in flags.into_iter().enumerate() {
        assert_eq!(
            f,
            flag(mask, k as u32) == 1,
            "header flag {k}, mask {mask:#b}"
        );
    }
}

#[test]
fn camera_state_worked_example() {
    // walker.md §11.12: a synthetic type-3 record of 36 bytes.
    let mut b = Bytes::default();
    b.u32(0, 0x4700_0010)
        .f32(4, 0.05)
        .f32(8, 0.2)
        .f32(12, 1500.0)
        .raw(16, &[0, 0, 0, 0])
        .u32(20, 3)
        .f32(24, 50.0)
        .f32(28, 0.1)
        .f32(32, 0.2);
    let bytes = b.0;
    assert_eq!(bytes.len(), 36);
    let c = decode_camera_state(&bytes).unwrap();
    assert_eq!(c.entity, Did(0x4700_0010));
    assert_eq!(c.collision_smoothing, 0.05);
    assert_eq!(c.near_clip, 0.2);
    assert_eq!(c.far_clip, 1500.0);
    assert!(!c.uses_physics);
    assert!(!c.uses_water_physics);
    assert!(!c.follows_under_water);
    assert!(!c.auto_transitions_disabled);
    assert_eq!(c.body.controller_type(), 3);
    assert_eq!(
        c.body,
        CameraBody::Cinematic(CinematicBody {
            fov: 50.0,
            target_smoothing: 0.1,
            angle_smoothing: 0.2,
        })
    );
    assert_eq!(c.to_bytes(), bytes);
}

#[test]
fn camera_state_third_person_hand_built() {
    // §11.5.3 type 1: 137-byte body at 24; the header flags are bits 0–3 of the mask, the
    // body flags bits 4–8.
    for mask in one_hot(9) {
        let mut b = Bytes::default();
        hand_built_camera_header(&mut b, mask, 1);
        let o = CAMERA_HEADER_LEN;
        b.f32(o, 1.5)
            .f32(o + 4, 2.5)
            .f32(o + 8, 3.5)
            .f32(o + 12, 4.5)
            .f32(o + 16, 5.5);
        let target = hand_built_target(&mut b, o + 20, 100.0, false);
        b.u8(o + 64, flag(mask, 4))
            .f32(o + 65, 45.0)
            .f32(o + 69, -80.0)
            .u8(o + 73, flag(mask, 5))
            .f32(o + 74, 13.5)
            .f32(o + 78, 14.5)
            .f32(o + 82, 15.5)
            .f32(o + 86, 16.5)
            .f32(o + 90, 17.5)
            .u8(o + 94, flag(mask, 6))
            .u8(o + 95, flag(mask, 7))
            .f32(o + 96, 18.5)
            .f32(o + 100, 19.5)
            .f32(o + 104, 20.5)
            .f32(o + 108, 21.5)
            .f32(o + 112, 22.5)
            .f32(o + 116, 23.5)
            .u8(o + 120, flag(mask, 8))
            .f32(o + 121, 24.5)
            .f32(o + 125, 25.5)
            .f32(o + 129, 26.5)
            .f32(o + 133, 27.5);
        let bytes = b.0;
        assert_eq!(bytes.len(), 161);
        let c = decode_camera_state(&bytes).unwrap();
        check_camera_header(&c, mask);
        let f = |k| flag(mask, k) == 1;
        assert_eq!(
            c.body,
            CameraBody::ThirdPerson(ThirdPersonBody {
                angle_smoothing: 1.5,
                zoom_smoothing: 2.5,
                target_smoothing: 3.5,
                unnamed: [4.5, 5.5],
                target,
                constrain_pitch: f(4),
                pitch_high: 45.0,
                pitch_low: -80.0,
                constrain_yaw: f(5),
                offset_range: [13.5, 14.5, 15.5],
                yaw_high: 16.5,
                yaw_low: 17.5,
                mouse_look_relative: f(6),
                detached_mouse: f(7),
                min_distance: 18.5,
                max_distance: 19.5,
                fov: 20.5,
                initial_pitch: 21.5,
                initial_yaw: 22.5,
                initial_distance: 23.5,
                align_to_slope: f(8),
                min_target_height: 24.5,
                target_fade_distance: 25.5,
                target_fade_opacity: 26.5,
                look_at_smoothing: 27.5,
            }),
            "mask {mask:#b}"
        );
        assert_eq!(c.to_bytes(), bytes);
    }
}

#[test]
fn camera_state_first_person_hand_built() {
    // §11.5.3 type 2: 65-byte body; body flag is bit 4.
    for mask in one_hot(5) {
        let mut b = Bytes::default();
        hand_built_camera_header(&mut b, mask, 2);
        let o = CAMERA_HEADER_LEN;
        b.f32(o, 60.0)
            .u8(o + 4, flag(mask, 4))
            .f32(o + 5, 90.0)
            .f32(o + 9, -85.0);
        let target = hand_built_target(&mut b, o + 13, 200.0, true);
        b.f32(o + 57, 7.0).f32(o + 61, 0.5);
        let bytes = b.0;
        assert_eq!(bytes.len(), 89);
        let c = decode_camera_state(&bytes).unwrap();
        check_camera_header(&c, mask);
        assert_eq!(
            c.body,
            CameraBody::FirstPerson(FirstPersonBody {
                fov: 60.0,
                constrain_pitch: flag(mask, 4) == 1,
                pitch_high: 90.0,
                pitch_low: -85.0,
                target,
                initial_yaw: 7.0,
                look_from_smoothing: 0.5,
            }),
            "mask {mask:#b}"
        );
        assert_eq!(c.to_bytes(), bytes);
    }
}

#[test]
fn camera_state_flight_hand_built() {
    // §11.5.3 type 4: 70-byte body; body flags are bits 4–5.
    for mask in one_hot(6) {
        let mut b = Bytes::default();
        hand_built_camera_header(&mut b, mask, 4);
        let o = CAMERA_HEADER_LEN;
        b.f32(o, 50.0)
            .f32(o + 4, 3.0)
            .u8(o + 8, flag(mask, 4))
            .f32(o + 9, 90.0)
            .f32(o + 13, -85.0)
            .u8(o + 17, flag(mask, 5))
            .f32(o + 18, -89.0)
            .f32(o + 22, 88.0);
        let target = hand_built_target(&mut b, o + 26, 300.0, true);
        let bytes = b.0;
        assert_eq!(bytes.len(), 94);
        let c = decode_camera_state(&bytes).unwrap();
        check_camera_header(&c, mask);
        assert_eq!(
            c.body,
            CameraBody::Flight(FlightBody {
                fov: 50.0,
                initial_yaw: 3.0,
                constrain_pitch: flag(mask, 4) == 1,
                pitch_high: 90.0,
                pitch_low: -85.0,
                constrain_roll: flag(mask, 5) == 1,
                roll_left: -89.0,
                roll_right: 88.0,
                target,
            }),
            "mask {mask:#b}"
        );
        assert_eq!(c.to_bytes(), bytes);
    }
}
