// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic integration tests for official world-session messages and compact
//! wire value encodings (`docs/specs/official-messages-world.md` §1–§7).

use std::collections::BTreeMap;
use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use tessera_compat::world::MAX_WORLD_COORDINATE_M;
use tessera_compat::world::{
    AccountBooted, ActionErrorResponse, ActionPayload, ActionStatusResponse, ActionSubEntry,
    AppearanceAuxEntry, AppearanceEquipmentEntry, AppearanceMorphEntry, CharacterCreateError,
    CharacterCreateSuccess, CharacterDeletedResult, CharacterError, CharacterSet,
    CharacterSetExtraEntry, CharacterSummaryEntry, CheckCharacterName, ClientActionCommand,
    ClientMoveCommand, CompactDid, CompactPid, CreateCharacter, DEFAULT_PORTAL_STATE_BOOL_PIDS,
    DEFAULT_PORTAL_STATE_ENUM_PIDS, DEFAULT_PORTAL_STATE_SCHEMA_ID, DeleteCharacter, DeleteEntity,
    DisplayLocalizedText, ENVELOPE_FLAG_HAS_ROUTING_HEADER, ENVELOPE_FLAG_ZLIB_COMPRESSED,
    EncodedPropertyEntry, EncodedPropertyTable, EncodedPropertyValue, EntityActionEvent,
    EntityAppearanceDesc, EntityAppearanceDescUpdate, EntityBatchSubEvents, EntityCreate,
    EntityInitialProperties, EntityPlacement, EntityPositionUpdate, MAX_RECURSION_DEPTH,
    MOUNTED_PORTAL_STATE_SCHEMA_ID_1, MOUNTED_PORTAL_STATE_SCHEMA_ID_2, NetworkClockSync,
    PID_ADVANCEMENT_LEVEL, PID_AGENT_SPECIES, PID_CHARGEN_CHARACTER_LEVEL,
    PID_COMBAT_ANIMATION_SET, PID_NAME, PID_WEENIE_TYPE, POS_FLAG_BLOCK, POS_FLAG_CELL,
    POS_FLAG_CELL_IS_ZERO, POS_FLAG_OFFSET, POS_FLAG_ORIENTATION, POS_FLAG_REGION,
    POS_FLAG_REGION_IS_ONE, PlayerSessionStart, PortalPlacement, PortalSpaceExit, PortalSpaceReady,
    PortalSpaceState, PropertyRemove, PropertyType, QueryMissingEntity, RoutingDestinationEntry,
    RoutingHeader, SelectCharacter, SendSteamId, ServerLastWords, SinglePropertyUpdate,
    SubEventFrame, TIMESTAMP_NO_TIME, WORLD_CALENDAR_CONFIG_DID, WireLocalizedText,
    WireLocalizedTextArg, WireLocalizedTextArgPayload, WirePosition, WireReader, WireWaveform,
    WireWriter, WorldCalendarConfig, WorldError, WorldName, decode_blob_payload, decode_heading_u8,
    decode_vle3, decode_vle4, decode_vle16, encode_heading_u8, encode_vle3, encode_vle4,
    encode_vle16,
};

fn sample_schema() -> BTreeMap<u32, PropertyType> {
    let mut map = BTreeMap::new();
    map.insert(PID_NAME, PropertyType::LocalizedText);
    map.insert(PID_ADVANCEMENT_LEVEL, PropertyType::Int);
    map.insert(PID_CHARGEN_CHARACTER_LEVEL, PropertyType::Int);
    map.insert(PID_AGENT_SPECIES, PropertyType::Enum);
    map.insert(0x0000_0001, PropertyType::String);
    map.insert(0x0000_0002, PropertyType::StringToken);
    map.insert(0x0000_0003, PropertyType::Waveform);
    map.insert(0x0000_0004, PropertyType::Timestamp);
    map.insert(0x0000_0005, PropertyType::TriState);
    map.insert(0x0000_0006, PropertyType::Vector3);
    map.insert(0x0000_0007, PropertyType::InstanceId);
    map.insert(0x0000_0008, PropertyType::Enum);
    map.insert(0x0000_0009, PropertyType::Float);
    map.insert(0x0000_000A, PropertyType::PropertyId);
    map.insert(0x0000_000B, PropertyType::Struct);
    map.insert(0x0000_000C, PropertyType::Array);
    map.insert(0x0000_000D, PropertyType::LocalizedText);
    map.insert(0x0000_000E, PropertyType::Bitfield64);
    map.insert(0x0000_000F, PropertyType::Int);
    map.insert(0x0000_0010, PropertyType::Color);
    map.insert(0x0000_0011, PropertyType::Position);
    map.insert(0x0000_0012, PropertyType::Bitfield32);
    map.insert(0x0000_0013, PropertyType::Int64);
    map.insert(0x0000_0014, PropertyType::DidRef);
    map.insert(0x0000_0015, PropertyType::Boolean);
    map.insert(0x0000_0016, PropertyType::Bitfield);
    map
}

// ---------------------------------------------------------------------------
// §7.2 Synthetic Test Vectors (Vectors 1–8)
// ---------------------------------------------------------------------------

#[test]
fn vector_1_three_tier_vle() {
    // 1. 0x0000_0042 (66) -> 42 (1 byte)
    let b1 = encode_vle3(0x0000_0042).unwrap();
    assert_eq!(b1, [0x42]);
    assert_eq!(decode_vle3(&b1).unwrap(), (0x0000_0042, 1));

    // 2. 0x0000_05A3 (1443) -> 85 A3 (2 bytes)
    let b2 = encode_vle3(0x0000_05A3).unwrap();
    assert_eq!(b2, [0x85, 0xA3]);
    assert_eq!(decode_vle3(&b2).unwrap(), (0x0000_05A3, 2));

    // 3. 0x0012_3456 (1193046) -> C0 12 56 34 (4 bytes)
    let b3 = encode_vle3(0x0012_3456).unwrap();
    assert_eq!(b3, [0xC0, 0x12, 0x56, 0x34]);
    assert_eq!(decode_vle3(&b3).unwrap(), (0x0012_3456, 4));

    // Boundary and overflow check
    let b_max = encode_vle3(0x3FFF_FFFF).unwrap();
    assert_eq!(b_max, [0xFF, 0xFF, 0xFF, 0xFF]);
    assert_eq!(decode_vle3(&b_max).unwrap(), (0x3FFF_FFFF, 4));
    assert_eq!(
        encode_vle3(0x4000_0000),
        Err(WorldError::Vle3Overflow(0x4000_0000))
    );
}

#[test]
fn vector_2_four_tier_vle_signed_negative_int() {
    // -1i32 as u32 = 0xFFFF_FFFF -> E0 FF FF FF FF (5 bytes)
    let neg_one = (-1i32) as u32;
    let encoded = encode_vle4(neg_one);
    assert_eq!(encoded, [0xE0, 0xFF, 0xFF, 0xFF, 0xFF]);
    let (decoded_u32, consumed) = decode_vle4(&encoded).unwrap();
    assert_eq!(consumed, 5);
    assert_eq!(decoded_u32 as i32, -1);

    // Additional tier boundaries for 4-tier VLE
    assert_eq!(encode_vle4(0x7F), [0x7F]);
    assert_eq!(encode_vle4(0x3FFF), [0xBF, 0xFF]);
    assert_eq!(encode_vle4(0x1FFF_FFFF), [0xDF, 0xFF, 0xFF, 0xFF]);
    assert_eq!(encode_vle4(0x2000_0000), [0xE0, 0x00, 0x00, 0x00, 0x20]);
    for &val in &[
        0,
        1,
        0x7F,
        0x80,
        0x3FFF,
        0x4000,
        0x1FFF_FFFF,
        0x2000_0000,
        0xFFFF_FFFF,
    ] {
        let bytes = encode_vle4(val);
        assert_eq!(decode_vle4(&bytes).unwrap(), (val, bytes.len()));
    }
}

#[test]
fn sixteen_bit_vle_all_tiers_and_reader_branches() {
    assert_eq!(encode_vle16(0x0042), [0x42]);
    assert_eq!(encode_vle16(0x05A3), [0x85, 0xA3]);
    assert_eq!(encode_vle16(0x4000), [0xC0, 0x00, 0x00, 0x40]);
    assert_eq!(encode_vle16(0xABCD), [0xC0, 0x00, 0xCD, 0xAB]);

    for &val in &[0u16, 0x7F, 0x80, 0x3FFF, 0x4000, 0xABCD, 0xFFFF] {
        let bytes = encode_vle16(val);
        assert_eq!(decode_vle16(&bytes).unwrap(), (val, bytes.len()));
    }
    // Reader 3-byte branch (0xE0..=0xFF reads u16 LE directly without discard byte)
    assert_eq!(decode_vle16(&[0xE0, 0x34, 0x12]).unwrap(), (0x1234, 3));
}

#[test]
fn vector_3_compact_did_encoding() {
    // DID 0x2300018E -> 23 81 8E (3 bytes)
    let did = CompactDid(0x2300_018E);
    let bytes = did.to_bytes().unwrap();
    assert_eq!(bytes, [0x23, 0x81, 0x8E]);
    assert_eq!(CompactDid::from_bytes(&bytes).unwrap(), did);

    // Additional 2-byte and 5-byte cases
    let did_short = CompactDid(0x2300_000A);
    assert_eq!(did_short.to_bytes().unwrap(), [0x23, 0x0A]);
    assert_eq!(CompactDid::from_bytes(&[0x23, 0x0A]).unwrap(), did_short);

    let did_long = CompactDid(0x7001_2345);
    assert_eq!(did_long.to_bytes().unwrap(), [0x70, 0xC0, 0x01, 0x45, 0x23]);
    assert_eq!(
        CompactDid::from_bytes(&[0x70, 0xC0, 0x01, 0x45, 0x23]).unwrap(),
        did_long
    );
}

#[test]
fn vector_4_compact_pid_encoding() {
    // 1. PID 0x00000354 (Name) -> 03 54 (2 bytes BE)
    let p1 = CompactPid(0x0000_0354);
    assert_eq!(p1.to_bytes(), [0x03, 0x54]);
    assert_eq!(CompactPid::from_bytes(&[0x03, 0x54]).unwrap(), p1);

    // 2. PID 0x10001011 (Advancement_Level) -> 50 11 (2 bytes BE with 0x4000 bit)
    let p2 = CompactPid(0x1000_1011);
    assert_eq!(p2.to_bytes(), [0x50, 0x11]);
    assert_eq!(CompactPid::from_bytes(&[0x50, 0x11]).unwrap(), p2);

    // 3. PID 0x1000663B (CharGen_Character_Level) -> 80 3B 66 00 10 (5 bytes)
    let p3 = CompactPid(0x1000_663B);
    assert_eq!(p3.to_bytes(), [0x80, 0x3B, 0x66, 0x00, 0x10]);
    assert_eq!(
        CompactPid::from_bytes(&[0x80, 0x3B, 0x66, 0x00, 0x10]).unwrap(),
        p3
    );
}

#[test]
fn vector_5_wire_position_vs_stored_position() {
    // region = 1, block = (0x7F, 0x85), instance = 0, cell = 0, offset = (1.0, 2.0, 3.0), identity quaternion
    let pos =
        WirePosition::from_components(1, 0x7F, 0x85, 0, 0, [1.0, 2.0, 3.0], [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(
        pos.flags,
        POS_FLAG_REGION
            | POS_FLAG_BLOCK
            | POS_FLAG_CELL
            | POS_FLAG_OFFSET
            | POS_FLAG_REGION_IS_ONE
            | POS_FLAG_CELL_IS_ZERO
    );
    assert_eq!(pos.flags, 0xDB);

    // Wire encoding (0x800 set): 15 bytes (elides region=1 and cell=0)
    let mut wire_w = WireWriter::new();
    pos.encode_wire(&mut wire_w);
    let expected_wire: [u8; 15] = [
        0xDB, 0x7F, 0x85, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x40, 0x40,
    ];
    assert_eq!(wire_w.as_slice(), expected_wire);

    let mut wire_r = WireReader::new(&expected_wire);
    let decoded_wire = WirePosition::decode_wire(&mut wire_r).unwrap();
    wire_r.finish().unwrap();
    assert_eq!(decoded_wire, pos);

    // Stored .dat encoding (0x800 clear): 18 bytes (includes region=1 and cell=0x0000)
    let mut stored_w = WireWriter::new();
    pos.encode_stored(&mut stored_w);
    let expected_stored: [u8; 18] = [
        0xDB, 0x01, 0x7F, 0x85, 0x00, 0x00, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x00, 0x40, 0x00,
        0x00, 0x40, 0x40,
    ];
    assert_eq!(stored_w.as_slice(), expected_stored);

    let mut stored_r = WireReader::new(&expected_stored);
    let decoded_stored = WirePosition::decode_stored(&mut stored_r).unwrap();
    stored_r.finish().unwrap();
    assert_eq!(decoded_stored, pos);

    // Full Type 17 with non-identity orientation (flag 0x20)
    let pos_orient = WirePosition::from_components(
        2,
        0x10,
        0x20,
        7,
        0x0102,
        [4.0, 5.0, 6.0],
        [0.70710677, 0.0, 0.70710677, 0.0],
    );
    assert_ne!(pos_orient.flags & POS_FLAG_ORIENTATION, 0);
    let mut w2 = WireWriter::new();
    pos_orient.encode_wire(&mut w2);
    let mut r2 = WireReader::new(w2.as_slice());
    assert_eq!(WirePosition::decode_wire(&mut r2).unwrap(), pos_orient);
    r2.finish().unwrap();
}

#[test]
fn vector_6_encoded_property_table_wire_vs_stored() {
    let schema = sample_schema();
    let mut props = EncodedPropertyTable::new();
    props.push(
        PID_NAME,
        EncodedPropertyValue::LocalizedText(WireLocalizedText::literal("Name")),
    );
    props.push(PID_ADVANCEMENT_LEVEL, EncodedPropertyValue::Int(50));
    props.push(PID_CHARGEN_CHARACTER_LEVEL, EncodedPropertyValue::Int(-1));

    let wire_bytes = props.to_bytes().unwrap();
    let expected_wire: [u8; 28] = [
        0x03, // count = 3 (no TSize byte on wire)
        0x03, 0x54, // CompactPID(0x00000354)
        0x01, 0x04, 0x4E, 0x00, 0x61, 0x00, 0x6D, 0x00, 0x65, 0x00, 0x00,
        0x00, // LocalizedText("Name")
        0x50, 0x11, // CompactPID(0x10001011)
        0x32, // 4-tier VLE(50)
        0x80, 0x3B, 0x66, 0x00, 0x10, // CompactPID(0x1000663B)
        0xE0, 0xFF, 0xFF, 0xFF, 0xFF, // 4-tier VLE(-1)
    ];
    assert_eq!(wire_bytes, expected_wire);

    let decoded = EncodedPropertyTable::from_bytes(&expected_wire, &schema).unwrap();
    assert_eq!(decoded, props);

    // Also verify stored LocalizedText (includes TSize=0x01 before arg_count, making LocalizedText("Name") 14 bytes)
    let si = WireLocalizedText::literal("Name");
    let mut stored_si_w = WireWriter::new();
    si.encode_stored(&mut stored_si_w).unwrap();
    assert_eq!(
        stored_si_w.as_slice(),
        [
            0x01, 0x04, 0x4E, 0x00, 0x61, 0x00, 0x6D, 0x00, 0x65, 0x00, 0x00, 0x01, 0x00
        ]
    );
    let mut stored_si_r = WireReader::new(stored_si_w.as_slice());
    assert_eq!(
        WireLocalizedText::decode_stored(&mut stored_si_r).unwrap(),
        si
    );
    stored_si_r.finish().unwrap();
}

#[test]
fn vector_7_select_character_and_portal_space_ready() {
    let schema = sample_schema();
    let select = SelectCharacter::new(0x0200_0000_0000_0001);

    // Inner event payload (10 bytes)
    let inner = select.to_bytes().unwrap();
    assert_eq!(
        inner,
        [0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00]
    );
    let decoded = SelectCharacter::from_bytes(&inner, &schema).unwrap();
    assert_eq!(decoded, select);

    // Uncompressed fragmented blob payload (11 bytes)
    let blob = select.to_blob_bytes().unwrap();
    assert_eq!(
        blob,
        [
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00
        ]
    );
    let unwrapped = decode_blob_payload(&blob, 0).unwrap();
    assert_eq!(unwrapped.envelope_flags, 0x00);
    assert_eq!(unwrapped.inner_payload, inner);

    // 0xF4 PortalSpaceReady (1 byte inner payload)
    let ready = PortalSpaceReady;
    assert_eq!(ready.to_bytes(), [0x00]);
    assert_eq!(PortalSpaceReady::from_bytes(&[0x00]).unwrap(), ready);
}

#[test]
fn vector_8_portal_placement_default_state_section_and_full_packet() {
    let bool_states = vec![true, false, false, false, true, false, false, false];
    let enum_states = vec![2, 1, 1, 3, 0x018E, 5];
    let portal = PortalPlacement {
        pos_sync_flags: 0x00,
        sequence_or_reason: 1,
        position: WirePosition::from_components(
            1,
            0x7F,
            0x85,
            0,
            0,
            [1.0, 2.0, 3.0],
            [1.0, 0.0, 0.0, 0.0],
        ),
        heading_raw: 0x40,
        movement_mode: 0x01,
        linear_velocity: [0.0; 3],
        angular_velocity: [0.0; 3],
        anim_or_context_did: CompactDid(0x2300_018E),
        state_schema_id: DEFAULT_PORTAL_STATE_SCHEMA_ID,
        bool_states: bool_states.clone(),
        enum_states: enum_states.clone(),
        extra_blob: Vec::new(),
    };

    // Verify 12-byte state section (steps 9a, 9b, 10, 11 of §4.3.1)
    let mut state_w = WireWriter::new();
    portal.encode_state_section(&mut state_w).unwrap();
    let expected_state: [u8; 12] = [
        0x11, // bool_state_bytes[0]: bit 0 (0x068A) and bit 4 (0x10000ED8) set
        0x02, 0x01, 0x01, 0x03, 0x81, 0x8E, 0x05, // 6 enum VLEs
        0x00, 0x00, 0x00, 0x00, // extra_blob_size = 0
    ];
    assert_eq!(state_w.as_slice(), expected_state);

    let mut state_r = WireReader::new(&expected_state);
    let (dec_bools, dec_enums, dec_extra) =
        PortalPlacement::decode_state_section(&mut state_r, DEFAULT_PORTAL_STATE_SCHEMA_ID)
            .unwrap();
    state_r.finish().unwrap();
    assert_eq!(dec_bools, bool_states);
    assert_eq!(dec_enums, enum_states);
    assert!(dec_extra.is_empty());

    // Verify PID helper lookups
    assert_eq!(
        portal.bool_state_for_pid(DEFAULT_PORTAL_STATE_BOOL_PIDS[0]),
        Some(true)
    );
    assert_eq!(
        portal.bool_state_for_pid(DEFAULT_PORTAL_STATE_BOOL_PIDS[4]),
        Some(true)
    );
    assert_eq!(
        portal.bool_state_for_pid(DEFAULT_PORTAL_STATE_BOOL_PIDS[1]),
        Some(false)
    );
    assert_eq!(portal.enum_state_for_pid(PID_WEENIE_TYPE), Some(2));
    assert_eq!(portal.enum_state_for_pid(PID_AGENT_SPECIES), Some(0x018E));
    assert_eq!(portal.enum_state_for_pid(PID_COMBAT_ANIMATION_SET), Some(5));
    assert_eq!(DEFAULT_PORTAL_STATE_ENUM_PIDS.len(), 6);

    // Full 0xCC round-trip
    let full_bytes = portal.to_bytes().unwrap();
    let decoded_portal = PortalPlacement::from_bytes(&full_bytes).unwrap();
    assert_eq!(decoded_portal, portal);

    // Mounted schema (0x10000E50 / 0x10000E51 -> 0 bools, 0 enums, bit 0x80 set)
    for schema_id in [
        MOUNTED_PORTAL_STATE_SCHEMA_ID_1,
        MOUNTED_PORTAL_STATE_SCHEMA_ID_2,
    ] {
        let mounted = PortalPlacement {
            pos_sync_flags: 0x80 | 0x02 | 0x10,
            sequence_or_reason: 42,
            position: portal.position,
            heading_raw: 0x80,
            movement_mode: 2,
            linear_velocity: [5.5, 0.0, 0.0],
            angular_velocity: [0.25, 0.0, 0.0],
            anim_or_context_did: CompactDid(0x1000_0001),
            state_schema_id: schema_id,
            bool_states: Vec::new(),
            enum_states: Vec::new(),
            extra_blob: vec![0xAA, 0xBB],
        };
        let bytes = mounted.to_bytes().unwrap();
        assert_eq!(PortalPlacement::from_bytes(&bytes).unwrap(), mounted);
    }
}

// ---------------------------------------------------------------------------
// Heading Quantization, All 22 Property Types, and fragmented blob Envelope
// ---------------------------------------------------------------------------

#[test]
fn quantized_heading_round_trip() {
    assert_eq!(encode_heading_u8(0.0), 0);
    assert!((decode_heading_u8(0) - 0.0).abs() < 1e-6);
    let pi = std::f32::consts::PI;
    assert_eq!(encode_heading_u8(pi), 128);
    assert!((decode_heading_u8(128) - pi).abs() < 1e-5);
    assert_eq!(encode_heading_u8(-pi / 2.0), 192);
}

#[test]
fn all_22_wire_property_types_round_trip() {
    let schema = sample_schema();
    let mut nested = EncodedPropertyTable::new();
    nested.push(
        0x0000_0001,
        EncodedPropertyValue::String("inner".to_string()),
    );

    let mut rich_si = WireLocalizedText::reference(0x0012_3456, 0x2500_0001);
    rich_si.authoring_strings = Some([
        "TOK_TEST".to_string(),
        "Hello {0}".to_string(),
        "comment".to_string(),
    ]);
    rich_si.arguments = vec![
        WireLocalizedTextArg {
            key: 0x1111_2222,
            kind: 1,
            header_kind: 1,
            var_id: 10,
            payload: Some(WireLocalizedTextArgPayload::Nested(Box::new(
                WireLocalizedText::literal("Sub"),
            ))),
        },
        WireLocalizedTextArg {
            key: 0x2222_3333,
            kind: 2,
            header_kind: 2,
            var_id: 11,
            payload: Some(WireLocalizedTextArgPayload::Float {
                value: 3.5,
                decimal_places: 2,
            }),
        },
        WireLocalizedTextArg {
            key: 0x3333_4444,
            kind: 3,
            header_kind: 3,
            var_id: 12,
            payload: Some(WireLocalizedTextArgPayload::FloatFormatted {
                value: 12.75,
                decimal_places: 1,
                format_flags: 0x03,
                radix: 10,
            }),
        },
        WireLocalizedTextArg {
            key: 0x4444_5555,
            kind: 4,
            header_kind: 4,
            var_id: 13,
            payload: Some(WireLocalizedTextArgPayload::FixedInt(-999)),
        },
        WireLocalizedTextArg {
            key: 0x5555_6666,
            kind: 5,
            header_kind: 5,
            var_id: 14,
            payload: Some(WireLocalizedTextArgPayload::FixedIntFormatted {
                value: 123_456,
                format_flags: 1,
                radix: 0xFFFF,
            }),
        },
        WireLocalizedTextArg {
            key: 0x6666_7777,
            kind: 6,
            header_kind: 6,
            var_id: 15,
            payload: Some(WireLocalizedTextArgPayload::CompactInt {
                is_32bit: true,
                value: 42,
            }),
        },
        WireLocalizedTextArg {
            key: 0x7777_8888,
            kind: 7,
            header_kind: 7,
            var_id: 16,
            payload: Some(WireLocalizedTextArgPayload::CompactIntFormatted {
                is_32bit: false,
                value: 0x0102_0304_0506_0708,
                format_flags: 2,
                radix: 16,
            }),
        },
    ];

    let mut ps = EncodedPropertyTable::new();
    ps.push_null();
    ps.push(
        0x0000_0001,
        EncodedPropertyValue::String("Latin1".to_string()),
    );
    ps.push(0x0000_0002, EncodedPropertyValue::StringToken(0x0546_5AB1));
    ps.push(
        0x0000_0003,
        EncodedPropertyValue::Waveform(WireWaveform::Constant(2.5)),
    );
    ps.push(
        0x0000_0003,
        EncodedPropertyValue::Waveform(WireWaveform::Standard {
            subtype: 2,
            coeffs: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0],
        }),
    );
    ps.push(
        0x0000_0003,
        EncodedPropertyValue::Waveform(WireWaveform::Keyframed {
            coeffs: [0.5; 10],
            period: 4.0,
            loop_flag: true,
            pairs: vec![(0.0, 1.0), (2.0, 3.0)],
        }),
    );
    ps.push(0x0000_0004, EncodedPropertyValue::Timestamp(1234.5));
    ps.push(0x0000_0005, EncodedPropertyValue::TriState(255));
    ps.push(0x0000_0006, EncodedPropertyValue::Vector3([1.0, -2.0, 3.5]));
    ps.push(
        0x0000_0007,
        EncodedPropertyValue::InstanceId(0x0200_0000_0000_0099),
    );
    ps.push(0x0000_0008, EncodedPropertyValue::Enum(0x2300_018E));
    ps.push(0x0000_0009, EncodedPropertyValue::Float(99.5));
    ps.push(0x0000_000A, EncodedPropertyValue::PropertyId(0x1000_1011));
    ps.push(0x0000_000B, EncodedPropertyValue::Struct(nested));
    ps.push(
        0x0000_000C,
        EncodedPropertyValue::Array(vec![
            EncodedPropertyEntry::new(0x0000_0009, EncodedPropertyValue::Float(1.25)),
            EncodedPropertyEntry::new(0x0000_000F, EncodedPropertyValue::Int(-7)),
        ]),
    );
    ps.push(0x0000_000D, EncodedPropertyValue::LocalizedText(rich_si));
    ps.push(
        0x0000_000E,
        EncodedPropertyValue::Bitfield64(0xCAFE_BABE_DEAD_BEEF),
    );
    ps.push(0x0000_000F, EncodedPropertyValue::Int(-42));
    ps.push(
        0x0000_0010,
        EncodedPropertyValue::Color([0x10, 0x20, 0x30, 0xFF]),
    );
    ps.push(
        0x0000_0011,
        EncodedPropertyValue::Position(WirePosition::from_components(
            1,
            10,
            20,
            0,
            0,
            [1.0, 2.0, 3.0],
            [1.0, 0.0, 0.0, 0.0],
        )),
    );
    ps.push(0x0000_0012, EncodedPropertyValue::Bitfield32(0x1234_5678));
    ps.push(0x0000_0013, EncodedPropertyValue::Int64(-123_456_789_012));
    ps.push(0x0000_0014, EncodedPropertyValue::DidRef(0x7000_1234));
    ps.push(0x0000_0015, EncodedPropertyValue::Boolean(true));
    ps.push(
        0x0000_0016,
        EncodedPropertyValue::Bitfield {
            bit_count: 19,
            bytes: vec![0xAA, 0x55, 0x03],
        },
    );

    let encoded = ps.to_bytes().unwrap();
    let decoded = EncodedPropertyTable::from_bytes(&encoded, &schema).unwrap();
    assert_eq!(decoded, ps);
}

#[test]
fn blob_envelope_routing_header_and_zlib() {
    let inner = EntityCreate {
        template_did: 0x7000_0001,
        entity_id: 0x0200_0000_0000_0042,
    }
    .to_bytes();

    let rh = RoutingHeader {
        routing_flags: 0x01 | 0x02 | 0x04 | 0x08,
        dest_table: vec![RoutingDestinationEntry {
            prefix: 3,
            word: 0x1234,
            destination_id: 0x0200_0000_0000_0001,
        }],
        sender_addr_block: Some(([1, 2, 3], 4)),
        override_target_id: Some(0x0200_0000_0000_0077),
        extra_words: vec![0xAAAA_BBBB, 0xCCCC_DDDD],
    };

    let mut uncompressed = WireWriter::new();
    rh.encode(&mut uncompressed).unwrap();
    // RoutingHeader must be 2-byte aligned
    assert_eq!(uncompressed.len() % 2, 0);
    uncompressed.write_bytes(&inner);

    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    z.write_all(uncompressed.as_slice()).unwrap();
    let compressed_bytes = z.finish().unwrap();

    let mut blob = WireWriter::new();
    blob.write_u8(ENVELOPE_FLAG_ZLIB_COMPRESSED | ENVELOPE_FLAG_HAS_ROUTING_HEADER);
    blob.write_u32_le(uncompressed.len() as u32);
    blob.write_bytes(&compressed_bytes);

    let decoded = decode_blob_payload(blob.as_slice(), 0x1111).unwrap();
    assert_eq!(decoded.routing_header.as_ref(), Some(&rh));
    assert_eq!(decoded.target_entity_id, 0x0200_0000_0000_0077);
    assert_eq!(decoded.inner_payload, inner);
}

// ---------------------------------------------------------------------------
// §3–§5 Message Codec Round-Trip Tests
// ---------------------------------------------------------------------------

#[test]
fn character_screen_messages_round_trip() {
    let schema = sample_schema();

    // 0x154 CheckCharacterName
    let check = CheckCharacterName {
        name: "Aldarion".to_string(),
    };
    assert_eq!(
        CheckCharacterName::from_bytes(&check.to_bytes().unwrap()).unwrap(),
        check
    );

    // 0xDC CreateCharacter
    let mut create_props = EncodedPropertyTable::new();
    create_props.push(PID_AGENT_SPECIES, EncodedPropertyValue::Enum(0x2300_0001));
    let create = CreateCharacter {
        creation_props: create_props,
        name: "Aldarion".to_string(),
        char_gen_type: 1,
        flag_a: false,
        flag_b: true,
        creation_param: 0,
    };
    assert_eq!(
        CreateCharacter::from_bytes(&create.to_bytes().unwrap(), &schema).unwrap(),
        create
    );

    // 0x13D CharacterSet & 0xDD CharacterCreateSuccess (with EntityAppearanceDesc)
    let mut extra_vprops = EncodedPropertyTable::new();
    extra_vprops.push(0x0000_0009, EncodedPropertyValue::Float(1.0));
    let vdesc = EntityAppearanceDesc {
        equipment: vec![AppearanceEquipmentEntry {
            item_class_did: 0x7000_1111,
            slot_bitmask: 0x0000_0003,
            primary_color: 0x1122_3344,
            secondary_color: 0x5566_7788,
            item_flag_a: true,
            item_flag_b: false,
            item_scalar: 1.5,
        }],
        appearance_entries: vec![AppearanceMorphEntry {
            appearance_key: 0x1000_0001,
            appearance_weight: 0.75,
        }],
        anim_set_did: 0x2300_0055,
        block_3: vec![AppearanceAuxEntry {
            field_0: 1,
            field_4: 2,
            field_8: 3.0,
            field_c: 4.0,
            field_10: 5,
            field_14: 6,
            field_15: 7,
            field_18: 8.0,
        }],
        extra_visual_props: Some(extra_vprops),
        visual_flag: true,
    };
    let mut char_props = EncodedPropertyTable::new();
    char_props.push(PID_ADVANCEMENT_LEVEL, EncodedPropertyValue::Int(20));
    let summary_entry = CharacterSummaryEntry {
        character_id: 0x0200_0000_0000_0001,
        name: "Aldarion".to_string(),
        appearance_desc: vdesc,
        char_properties: char_props,
    };

    let cset = CharacterSet {
        characters: vec![summary_entry.clone()],
        monster_characters: Vec::new(),
        account_props: EncodedPropertyTable::new(),
        status_or_version: 1,
        extra_entries: vec![CharacterSetExtraEntry {
            key_id: 0x0200_0000_0000_0001,
            val_a: 10,
            val_b: 20,
        }],
    };
    assert_eq!(
        CharacterSet::from_bytes(&cset.to_bytes().unwrap(), &schema).unwrap(),
        cset
    );

    let csuccess = CharacterCreateSuccess {
        character: summary_entry,
        is_monster_play: false,
    };
    assert_eq!(
        CharacterCreateSuccess::from_bytes(&csuccess.to_bytes().unwrap(), &schema).unwrap(),
        csuccess
    );

    // 0x79 CharacterCreateError
    let cerr = CharacterCreateError {
        result_code: 4,
        name_error_code: 12,
    };
    assert_eq!(
        CharacterCreateError::from_bytes(&cerr.to_bytes()).unwrap(),
        cerr
    );

    // 0x7A & 0xD1 DeleteCharacter
    let del_fp = DeleteCharacter::free_peoples(0x0200_0000_0000_0001);
    assert_eq!(del_fp.event_id(), DeleteCharacter::EVENT_ID_FREE_PEOPLES);
    assert_eq!(
        DeleteCharacter::from_bytes(&del_fp.to_bytes(), false).unwrap(),
        del_fp
    );
    let del_mp = DeleteCharacter::monster_play(0x0200_0000_0000_0002);
    assert_eq!(del_mp.event_id(), DeleteCharacter::EVENT_ID_MONSTER_PLAY);
    assert_eq!(
        DeleteCharacter::from_bytes(&del_mp.to_bytes(), true).unwrap(),
        del_mp
    );

    // 0x15 CharacterDeletedResult
    let del_res = CharacterDeletedResult {
        character_id: 0x0200_0000_0000_0001,
        result_code: 0,
    };
    assert_eq!(
        CharacterDeletedResult::from_bytes(&del_res.to_bytes()).unwrap(),
        del_res
    );

    // 0x6D CharacterError
    let ch_err = CharacterError { error_code: 7 };
    assert_eq!(
        CharacterError::from_bytes(&ch_err.to_bytes()).unwrap(),
        ch_err
    );

    // 0xB3 AccountBooted
    let booted = AccountBooted {
        boot_reason: 2,
        reason_text: WireLocalizedText::literal("Disconnected"),
    };
    assert_eq!(
        AccountBooted::from_bytes(&booted.to_bytes().unwrap()).unwrap(),
        booted
    );

    // 0x19E SendSteamId
    let steam = SendSteamId {
        steam_id: 0x0110_0001_0000_0001,
    };
    assert_eq!(SendSteamId::from_bytes(&steam.to_bytes()).unwrap(), steam);
}

#[test]
fn world_entry_and_object_lifecycle_messages_round_trip() {
    let schema = sample_schema();

    // 0x43 WorldName, 0xAA DisplayLocalizedText, 0xFA ServerLastWords
    let wname = WorldName {
        world_name: WireLocalizedText::literal("Evernight"),
    };
    assert_eq!(
        WorldName::from_bytes(&wname.to_bytes().unwrap()).unwrap(),
        wname
    );

    let dsi = DisplayLocalizedText {
        target_id: 0x0200_0000_0000_0001,
        message: WireLocalizedText::reference(0x0001_0002, 0x2500_0001),
    };
    assert_eq!(
        DisplayLocalizedText::from_bytes(&dsi.to_bytes().unwrap()).unwrap(),
        dsi
    );

    let last_words = ServerLastWords {
        last_words: WireLocalizedText::literal("Server restarting"),
    };
    assert_eq!(
        ServerLastWords::from_bytes(&last_words.to_bytes().unwrap()).unwrap(),
        last_words
    );

    // 0x90 PlayerSessionStart
    let mut sprops = EncodedPropertyTable::new();
    sprops.push(PID_ADVANCEMENT_LEVEL, EncodedPropertyValue::Int(50));
    let pstart = PlayerSessionStart {
        player_id: 0x0200_0000_0000_0001,
        session_props: sprops,
    };
    assert_eq!(
        PlayerSessionStart::from_bytes(&pstart.to_bytes().unwrap(), &schema).unwrap(),
        pstart
    );

    // 0xC7 PortalSpaceExit & 0x153 PortalSpaceState
    assert_eq!(
        PortalSpaceExit::from_bytes(&PortalSpaceExit.to_bytes()).unwrap(),
        PortalSpaceExit
    );
    assert_eq!(
        PortalSpaceState::from_bytes(&PortalSpaceState.to_bytes()).unwrap(),
        PortalSpaceState
    );

    // 0x98 EntityCreate, 0x52 EntityInitialProperties, 0x89 EntityAppearanceDescUpdate, 0x40 DeleteEntity, 0xD0 QueryMissingEntity
    let ecreate = EntityCreate {
        template_did: 0x7000_1234,
        entity_id: 0x0200_0000_0000_0099,
    };
    assert_eq!(
        EntityCreate::from_bytes(&ecreate.to_bytes()).unwrap(),
        ecreate
    );

    let mut iprops = EncodedPropertyTable::new();
    iprops.push(
        PID_NAME,
        EncodedPropertyValue::LocalizedText(WireLocalizedText::literal("Guard")),
    );
    let einit = EntityInitialProperties { properties: iprops };
    assert_eq!(
        EntityInitialProperties::from_bytes(&einit.to_bytes().unwrap(), &schema).unwrap(),
        einit
    );

    let vupd = EntityAppearanceDescUpdate {
        appearance_desc: EntityAppearanceDesc::default(),
    };
    assert_eq!(
        EntityAppearanceDescUpdate::from_bytes(&vupd.to_bytes().unwrap(), &schema).unwrap(),
        vupd
    );

    assert_eq!(
        DeleteEntity::from_bytes(&DeleteEntity.to_bytes()).unwrap(),
        DeleteEntity
    );

    let qmissing = QueryMissingEntity {
        entity_id: 0x0200_0000_0000_0099,
    };
    assert_eq!(
        QueryMissingEntity::from_bytes(&qmissing.to_bytes()).unwrap(),
        qmissing
    );

    // 0x0D / 0x4C EntityBatchSubEvents
    let batch = EntityBatchSubEvents {
        sub_events: vec![
            SubEventFrame::Event {
                sub_event_id: EntityCreate::EVENT_ID,
                payload: ecreate.to_bytes(),
            },
            SubEventFrame::Null {
                field_a: 0x05A3,
                field_b: 0x1234,
            },
            SubEventFrame::Event {
                sub_event_id: EntityInitialProperties::EVENT_ID,
                payload: einit.to_bytes().unwrap(),
            },
        ],
    };
    assert_eq!(
        EntityBatchSubEvents::from_bytes(&batch.to_bytes().unwrap()).unwrap(),
        batch
    );
}

#[test]
fn entity_position_update_and_movement_actions_round_trip() {
    let schema = sample_schema();
    let pos = WirePosition::from_components(
        1,
        0x7F,
        0x85,
        0,
        0,
        [10.0, 20.0, 30.0],
        [1.0, 0.0, 0.0, 0.0],
    );

    // 0xCE Mode 1: WorldWithHeading + velocity + anim_state + interp params
    let ce1 = EntityPositionUpdate {
        header_flags: 0x01 | 0x02 | 0x04 | 0x08 | 0x10 | 0x20 | 0x40,
        entity_id: 0x0200_0000_0000_0055,
        mode_flags: 0x00,
        seq_byte: Some(7),
        velocity: [1.0, 2.0, 3.0],
        anim_state: 14,
        placement: EntityPlacement::WorldWithHeading {
            position: pos,
            heading_raw: 0x80,
        },
        interp_param_a: 6,
        interp_param_b: 3,
    };
    assert_eq!(
        EntityPositionUpdate::from_bytes(&ce1.to_bytes().unwrap()).unwrap(),
        ce1
    );

    // 0xCE Mode 2: WorldPositionOnly
    let ce2 = EntityPositionUpdate {
        header_flags: 0x00,
        entity_id: 0x0200_0000_0000_0055,
        mode_flags: 0x10,
        seq_byte: None,
        velocity: [0.0; 3],
        anim_state: 0,
        placement: EntityPlacement::WorldPositionOnly { position: pos },
        interp_param_a: 5,
        interp_param_b: 2,
    };
    assert_eq!(
        EntityPositionUpdate::from_bytes(&ce2.to_bytes().unwrap()).unwrap(),
        ce2
    );

    // 0xCE Mode 3: ParentAttachment
    let ce3 = EntityPositionUpdate {
        header_flags: 0x00,
        entity_id: 0x0200_0000_0000_0055,
        mode_flags: 0x04,
        seq_byte: None,
        velocity: [0.0; 3],
        anim_state: 0,
        placement: EntityPlacement::ParentAttachment {
            parent_id: 0x0200_0000_0000_0001,
            attach_slot: 3,
        },
        interp_param_a: 5,
        interp_param_b: 2,
    };
    assert_eq!(
        EntityPositionUpdate::from_bytes(&ce3.to_bytes().unwrap()).unwrap(),
        ce3
    );

    // 0x24 ClientMoveCommand: 38-byte base
    let move_base = ClientMoveCommand::new_base(
        0x0200_0000_0000_0001,
        0x01,
        42,
        [1, 1, 1, 1],
        [128, 128, 128, 128],
        [128, 128, 128, 128],
        [0, 0, 0, 0],
        0x0000_0000,
    );
    let move_base_bytes = move_base.to_bytes().unwrap();
    assert_eq!(move_base_bytes.len(), ClientMoveCommand::BASE_BYTE_LEN);
    assert_eq!(
        ClientMoveCommand::from_bytes(&move_base_bytes, &schema).unwrap(),
        move_base
    );

    // 0x24 ClientMoveCommand with slot actions and slot properties
    let mut act_props = EncodedPropertyTable::new();
    act_props.push(0x0000_0009, EncodedPropertyValue::Float(2.0));
    let apayload = ActionPayload {
        action_mode: 0x0A,
        flags: 0x0040 | 0x2000 | 0x0001 | 0x4000 | 0x0002 | 0x0008 | 0x0010 | 0x0020,
        scalar_param: 1.5,
        sub_entry: Some(ActionSubEntry {
            sub_action_id: 100,
            sub_entry_id: 200,
            sub_entry_flag: true,
        }),
        target_id_a: 0x0200_0000_0000_00AA,
        target_id_b: 0x0200_0000_0000_00BB,
        action_props: Some(act_props.clone()),
        state_code: 4,
        speed_or_scale: 1.25,
        timestamp: 500.0,
        aux_index: 9,
    };
    let mut move_ext = move_base;
    move_ext.slot_flags = [0, 0, 0x10, 0x01];
    move_ext.slot_actions[3].push((CompactDid(0x2300_0010), apayload.clone()));
    move_ext.slot_props[2] = Some(act_props);
    let move_ext_bytes = move_ext.to_bytes().unwrap();
    assert_eq!(
        ClientMoveCommand::from_bytes(&move_ext_bytes, &schema).unwrap(),
        move_ext
    );

    // 0x75 ClientActionCommand & 0x5A/0xA8/0x63 EntityActionEvent & 0x22/0x16C
    let cact = ClientActionCommand {
        action_id: CompactDid(0x2300_0010),
        context_did: CompactDid(0x2300_0001),
        action_payload: apayload.clone(),
    };
    assert_eq!(
        ClientActionCommand::from_bytes(&cact.to_bytes().unwrap(), &schema).unwrap(),
        cact
    );

    let eact = EntityActionEvent {
        action_id: CompactDid(0x2300_0010),
        action_payload: apayload,
    };
    assert_eq!(
        EntityActionEvent::from_bytes(&eact.to_bytes().unwrap(), &schema).unwrap(),
        eact
    );

    let aerr = ActionErrorResponse {
        action_id: CompactDid(0x2300_0010),
        error_did: CompactDid(0x2300_00FF),
        target_id: 0x0200_0000_0000_00AA,
    };
    assert_eq!(
        ActionErrorResponse::from_bytes(&aerr.to_bytes().unwrap()).unwrap(),
        aerr
    );

    let astat = ActionStatusResponse {
        action_id: CompactDid(0x2300_0010),
        status_did: CompactDid(0x2300_0002),
    };
    assert_eq!(
        ActionStatusResponse::from_bytes(&astat.to_bytes().unwrap()).unwrap(),
        astat
    );

    // 0x88/0x92/0xB0/0x147/0x149 SinglePropertyUpdate & 0xB7 PropertyRemove
    let spu = SinglePropertyUpdate::new(PID_ADVANCEMENT_LEVEL, EncodedPropertyValue::Int(51));
    assert_eq!(
        SinglePropertyUpdate::from_bytes(&spu.to_bytes().unwrap(), &schema).unwrap(),
        spu
    );

    let prem = PropertyRemove {
        pid: CompactPid(PID_ADVANCEMENT_LEVEL),
    };
    assert_eq!(PropertyRemove::from_bytes(&prem.to_bytes()).unwrap(), prem);
}

// ---------------------------------------------------------------------------
// §4.4.1 NetworkClockSync and §4.4.2 WorldCalendarConfig
// ---------------------------------------------------------------------------

#[test]
fn smoothed_network_clock_forward_snap_and_five_tier_slew() {
    let mut clock = NetworkClockSync::new(1000.0, 10.0);
    assert!((clock.synced_time() - 1000.0).abs() < 1e-9);

    // 1. Client behind server -> immediate forward snap (with latency compensation)
    clock.stage_clock_sync(1002.0, 11.0, 11.05);
    let t = clock.update_frame(11.10);
    // target = (11.10 - 11.05) + ((11.05 - 11.0) + 1002.0) = 1002.10
    assert!((t - 1002.10).abs() < 1e-9);
    assert_eq!(clock.slew_debt(), 0.0);

    // 2. Client ahead of server by 0.010 s (< 0.015 s tolerance) -> ignored, no backward jump
    clock.stage_clock_sync(1003.09, 12.10, 12.10);
    let t2 = clock.update_frame(12.10);
    assert!((t2 - 1003.10).abs() < 1e-9);
    assert_eq!(clock.slew_rate(), 0.0);

    // 3. Client ahead of server by 0.100 s (in [0.060, 0.120) -> tier 0.01 s/s)
    // At t_now = 13.10, unslewed synced_time is 1004.10; server says 1004.00 -> excess = 0.100
    clock.stage_clock_sync(1004.00, 13.10, 13.10);
    let t3 = clock.update_frame(13.10);
    assert!((t3 - 1004.10).abs() < 1e-9); // monotonic: does not jump backward
    assert!((clock.slew_debt() - 0.100).abs() < 1e-9);
    assert!((clock.slew_rate() - 0.01).abs() < 1e-9);

    // Advance 5.0 seconds -> bleeds 5.0 * 0.01 = 0.05 s of slew_debt
    let t4 = clock.update_frame(18.10);
    assert!((t4 - 1009.05).abs() < 1e-9);
    assert!((clock.slew_debt() - 0.05).abs() < 1e-9);

    // Relative vs absolute Type 4 Timestamp and NO_TIME sentinel
    assert_eq!(
        clock.decode_wire_timestamp(TIMESTAMP_NO_TIME, false),
        TIMESTAMP_NO_TIME
    );
    assert!((clock.decode_wire_timestamp(5.0, false) - (t4 + 5.0)).abs() < 1e-9);
    assert!((clock.encode_wire_timestamp(t4 + 5.0, false) - 5.0).abs() < 1e-9);
}

#[test]
fn world_calendar_config_canonical_180_bytes_and_day_night_evaluation() {
    let rec = WorldCalendarConfig::canonical();
    assert_eq!(rec.did, WORLD_CALENDAR_CONFIG_DID);
    let bytes = rec.to_bytes().unwrap();
    assert_eq!(bytes.len(), 180);
    assert_eq!(WorldCalendarConfig::from_bytes(&bytes).unwrap(), rec);

    // Evaluate at dawn (0.25 of day) -> daytime (is_night == false)
    // With calendar_epoch_offset = 3600.0 and day_length = 11160.0, day 0 starts at dilated_time = -3600.0
    let dawn_dilated = -3600.0 + 0.25 * 11_160.0;
    let snap_dawn = rec.evaluate(dawn_dilated);
    assert!((snap_dawn.present_time_of_day - 0.25).abs() < 1e-5);
    assert!(!snap_dawn.is_night);
    assert_eq!(snap_dawn.calendar_year, 34);
    assert_eq!(snap_dawn.day_of_year, 0);

    // Evaluate at dusk (0.80 of day) -> nighttime (is_night == true)
    let dusk_dilated = -3600.0 + 0.80 * 11_160.0;
    let snap_dusk = rec.evaluate(dusk_dilated);
    assert!((snap_dusk.present_time_of_day - 0.80).abs() < 1e-5);
    assert!(snap_dusk.is_night);
}

// ---------------------------------------------------------------------------
// §7.1 Hostile-Input Invariants
// ---------------------------------------------------------------------------

#[test]
fn hostile_input_bounds_and_validation() {
    let schema = sample_schema();

    // 1. Oversized VLE count in a short buffer -> CountExceedsRemaining without OOM
    let oversized_ps = [0xFF, 0xFF, 0xFF, 0xFF]; // 3-tier VLE = 0x3FFF_FFFF entries, 0 bytes left
    assert!(matches!(
        EncodedPropertyTable::from_bytes(&oversized_ps, &schema),
        Err(WorldError::CountExceedsRemaining { .. })
    ));

    // 2. Recursion depth > 16 rejected in nested Struct EncodedPropertyTable
    let mut deep_w = WireWriter::new();
    for _ in 0..=MAX_RECURSION_DEPTH {
        deep_w.write_vle3(1).unwrap(); // 1 entry
        CompactPid(0x0000_000B).encode(&mut deep_w); // PID for PropertyType::Struct
    }
    deep_w.write_vle3(0).unwrap();
    assert_eq!(
        EncodedPropertyTable::from_bytes(deep_w.as_slice(), &schema),
        Err(WorldError::MaxRecursionDepthExceeded {
            max: MAX_RECURSION_DEPTH
        })
    );

    // 3. Invalid bool8 >= 2 rejected
    let bad_bool_ps = [0x01, 0x00, 0x15, 0x02]; // count=1, PID=0x0015 (Boolean), value=0x02
    assert_eq!(
        EncodedPropertyTable::from_bytes(&bad_bool_ps, &schema),
        Err(WorldError::InvalidBool8(0x02))
    );

    // 4. Unknown PID rejected with UnknownPropertyId
    let unknown_pid_ps = [0x01, 0x09, 0x99, 0x00]; // count=1, PID=0x0999 (not in schema)
    assert_eq!(
        EncodedPropertyTable::from_bytes(&unknown_pid_ps, &schema),
        Err(WorldError::UnknownPropertyId(0x0000_0999))
    );

    // 5. Non-zero base_flags rejected
    assert_eq!(
        DeleteEntity::from_bytes(&[0x01]),
        Err(WorldError::InvalidBaseFlags(0x01))
    );
}

// ---------------------------------------------------------------------------
// Coordinate finiteness and world bounds (`official-messages-play.md` §9.1 rule 4)
// ---------------------------------------------------------------------------

fn portal_with(offset: [f32; 3], linear: [f32; 3], angular: [f32; 3]) -> PortalPlacement {
    PortalPlacement {
        pos_sync_flags: 0x7E,
        sequence_or_reason: 1,
        position: WirePosition::from_components(1, 0x7F, 0x85, 0, 0, offset, [1.0, 0.0, 0.0, 0.0]),
        heading_raw: 0x40,
        movement_mode: 0x01,
        linear_velocity: linear,
        angular_velocity: angular,
        anim_or_context_did: CompactDid(0x2300_018E),
        state_schema_id: DEFAULT_PORTAL_STATE_SCHEMA_ID,
        bool_states: vec![false; 8],
        enum_states: vec![0; 6],
        extra_blob: Vec::new(),
    }
}

#[test]
fn portal_placement_rejects_non_finite_position_and_velocity() {
    let ok = portal_with([1.0, 2.0, 3.0], [1.0, 2.0, 3.0], [0.1, 0.2, 0.3]);
    assert_eq!(
        PortalPlacement::from_bytes(&ok.to_bytes().unwrap()).unwrap(),
        ok
    );

    let cases = [
        (
            portal_with([f32::NAN, 2.0, 3.0], [0.0; 3], [0.0; 3]),
            "pos_x",
        ),
        (
            portal_with([1.0, 2.0, f32::INFINITY], [0.0; 3], [0.0; 3]),
            "pos_z",
        ),
        (
            portal_with([1.0, 2.0, 3.0], [0.0, f32::NEG_INFINITY, 0.0], [0.0; 3]),
            "vel_y",
        ),
        (
            portal_with([1.0, 2.0, 3.0], [0.0; 3], [0.0, 0.0, f32::NAN]),
            "ang_vel_z",
        ),
    ];
    for (portal, field) in cases {
        let bytes = portal.to_bytes().unwrap();
        assert_eq!(
            PortalPlacement::from_bytes(&bytes),
            Err(WorldError::NonFiniteFloat { field })
        );
    }
}

fn ce_world(offset: [f32; 3], velocity: [f32; 3], heading: bool) -> EntityPositionUpdate {
    let position = WirePosition::from_components(1, 0x7F, 0x85, 0, 0, offset, [1.0, 0.0, 0.0, 0.0]);
    EntityPositionUpdate {
        header_flags: 0x10 | 0x20 | 0x40,
        entity_id: 0x0200_0000_0000_0055,
        mode_flags: if heading { 0x00 } else { 0x10 },
        seq_byte: None,
        velocity,
        anim_state: 0,
        placement: if heading {
            EntityPlacement::WorldWithHeading {
                position,
                heading_raw: 0x10,
            }
        } else {
            EntityPlacement::WorldPositionOnly { position }
        },
        interp_param_a: 5,
        interp_param_b: 2,
    }
}

#[test]
fn entity_position_update_rejects_non_finite_and_out_of_range_values() {
    let edge = MAX_WORLD_COORDINATE_M;
    for heading in [true, false] {
        let ok = ce_world([edge, -edge, 0.5], [1.0, -2.0, 3.0], heading);
        assert_eq!(
            EntityPositionUpdate::from_bytes(&ok.to_bytes().unwrap()).unwrap(),
            ok
        );

        let bad_vel = ce_world([1.0, 2.0, 3.0], [f32::NAN, 0.0, 0.0], heading);
        assert_eq!(
            EntityPositionUpdate::from_bytes(&bad_vel.to_bytes().unwrap()),
            Err(WorldError::NonFiniteFloat { field: "vel_x" })
        );

        let bad_pos = ce_world([1.0, f32::INFINITY, 3.0], [0.0; 3], heading);
        assert_eq!(
            EntityPositionUpdate::from_bytes(&bad_pos.to_bytes().unwrap()),
            Err(WorldError::NonFiniteFloat { field: "pos_y" })
        );

        let far = ce_world([1.0, 2.0, 100_001.0], [0.0; 3], heading);
        assert_eq!(
            EntityPositionUpdate::from_bytes(&far.to_bytes().unwrap()),
            Err(WorldError::CoordinateOutOfRange { field: "pos_z" })
        );
    }
}
