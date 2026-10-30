// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic tests for `tessera_compat::world` (`docs/specs/official-messages-world.md`
//! §2–§5, §7). Every input is built here from the spec's tables and vectors; no captured
//! traffic or install data is used. Schemas stand in for the run-time `0x34000000` and
//! `0x07000029` lookups.

use std::collections::BTreeMap;

use tessera_compat::session::{
    ENVELOPE_FLAG_HAS_ROUTING_HEADER, MessageRoutingHeader, SessionError, encode_message,
    encode_message_with_routing,
};
use tessera_compat::world::{
    AccountEvictionNotice, ActionCancel, ActionChainCompletion, ActionChainSync, ActionLayerEntry,
    ActionPayload, ActionQueueEnable, ActionStart, ActionStateReset, ActionStepComplete,
    ActionSubEntry, AppearanceTint, AttachmentExtension, CharacterCreateAck, CharacterCreateReject,
    CharacterCreateRequest, CharacterDeleteRequest, CharacterDeleteStatus,
    CharacterEnterWorldRequest, CharacterListResponse, CharacterNameCheckRequest,
    CharacterOperationError, CharacterSummaryEntry, ClientActionCommand, ClientPhysicsSync,
    CompactDid, CompactPid, DEFAULT_PORTAL_STATE_SCHEMA_ID, DisplayLocalizedText,
    EncodedPropertyEntry, EncodedPropertyTable, EncodedPropertyValue, EnterPortalSpace,
    EntityAnimLayerState, EntityAppearanceDesc, EntityCreate, EntityDespawn, EntityPlacement,
    EntityStringEvent, LandblockPropertyEvent, LandblockPropertyOp, LocalizedTokenText,
    LocalizedTokenTextArg, LocalizedTokenTextArgPayload, LocalizedTokenTextContent, LoginFinished,
    MAX_RECURSION_DEPTH, MissingEntityQuery, NetworkClock, PID_AGENT_CLASS, PID_AGENT_SPECIES,
    PID_CURRENT_MORALE, PID_ENTITY_DISPLAY_NAME, PartEntry, PlatformAccountIdReport,
    PlayerSessionReady, PortalPositionSync, PortalReadyAck, PortalStateNotice, PortalStateSchema,
    PortalTransitionComplete, PositionUpdate, PredictedPropertyAck, PropertyRemoval, PropertyScope,
    PropertyTableUpdate, PropertyType, ServerShutdownText, SinglePropertyUpdate, SubEventBatch,
    SubEventFrame, SubPartEntry, SubPartTable, TIMESTAMP_NO_TIME, TextSource,
    VisualDescriptionUpdate, WireBitfield, WirePosition, WireReader, WireWriter,
    WorldCalendarConfig, WorldClock, WorldError, WorldNameAnnouncement, active_target,
    decode_event_envelope, decode_heading_u8, decode_signed_normalized, decode_vle3, decode_vle4,
    decode_vle16, encode_event_envelope, encode_heading_u8, encode_signed_normalized, encode_vle3,
    encode_vle4, encode_vle16, wire_int_is_32bit,
};

/// Synthetic dictionary of `property-system.md` §14.1 (Vector 4) plus one PID per type.
fn schema() -> BTreeMap<u32, PropertyType> {
    let mut map = BTreeMap::new();
    map.insert(0x1000_0BBB, PropertyType::Int);
    map.insert(0x1000_0AAA, PropertyType::Struct);
    map.insert(0x1000_0CCC, PropertyType::Boolean);
    map.insert(PID_ENTITY_DISPLAY_NAME, PropertyType::LocalizedTokenText);
    map.insert(PID_AGENT_SPECIES, PropertyType::Enum);
    map.insert(PID_AGENT_CLASS, PropertyType::Enum);
    map.insert(PID_CURRENT_MORALE, PropertyType::Float);
    for code in 1u32..=22 {
        map.insert(0x0000_0100 + code, PropertyType::from_code(code).unwrap());
    }
    map
}

// ---------------------------------------------------------------------------
// §7.2 Vectors 1–6: value encodings
// ---------------------------------------------------------------------------

#[test]
fn vector_1_vle3_and_variable_u32() {
    for (value, bytes) in [
        (0x7Fu32, &[0x7F][..]),
        (0x100, &[0x81, 0x00][..]),
        (0x0001_2345, &[0xC0, 0x01, 0x45, 0x23][..]),
    ] {
        assert_eq!(encode_vle3(value).unwrap(), bytes);
        assert_eq!(encode_vle4(value), bytes);
        assert_eq!(decode_vle3(bytes).unwrap(), (value, bytes.len()));
        assert_eq!(decode_vle4(bytes).unwrap(), (value, bytes.len()));
    }
    // Signed -1 in a Type 15 Int on the wire.
    assert_eq!(encode_vle4(0xFFFF_FFFF), [0xE0, 0xFF, 0xFF, 0xFF, 0xFF]);
    let mut w = WireWriter::new();
    EncodedPropertyValue::Int(-1).encode(&mut w).unwrap();
    assert_eq!(w.as_slice(), &[0xE0, 0xFF, 0xFF, 0xFF, 0xFF]);
    // 3-tier VLE cannot hold 0x40000000; VariableU32 first bytes 0xE1..=0xFF are invalid.
    assert_eq!(
        encode_vle3(0x4000_0000),
        Err(WorldError::Vle3Overflow(0x4000_0000))
    );
    for b0 in [0xE1u8, 0xF0, 0xFF] {
        assert_eq!(
            decode_vle4(&[b0, 0, 0, 0, 0]),
            Err(WorldError::InvalidVariableU32Prefix(b0))
        );
    }
}

#[test]
fn sixteen_bit_vle_tiers() {
    assert_eq!(encode_vle16(0x7F), [0x7F]);
    assert_eq!(encode_vle16(0x3FFF), [0xBF, 0xFF]);
    assert_eq!(encode_vle16(0x4000), [0xC0, 0x00, 0x00, 0x40]);
    assert_eq!(
        decode_vle16(&[0xC5, 0x99, 0x34, 0x12]).unwrap(),
        (0x1234, 4)
    );
    assert_eq!(decode_vle16(&[0xE0, 0x34, 0x12]).unwrap(), (0x1234, 3));
}

#[test]
fn vector_2_compact_pid_and_hostile_first_bytes() {
    for (pid, bytes) in [
        (0x0000_002Au32, &[0x00, 0x2A][..]),
        (0x1000_0BBB, &[0x4B, 0xBB][..]),
        (0x1000_4567, &[0x80, 0x67, 0x45, 0x00, 0x10][..]),
        (0, &[0x00, 0x00][..]),
    ] {
        assert_eq!(CompactPid(pid).to_bytes(), bytes);
        assert_eq!(CompactPid::from_bytes(bytes).unwrap(), CompactPid(pid));
    }
    // property-system.md §10.2: first bytes 0x81..=0xFF are malformed.
    for b0 in [0x81u8, 0xBF, 0xC0, 0xFF] {
        assert_eq!(
            CompactPid::from_bytes(&[b0, 0, 0, 0, 0]),
            Err(WorldError::InvalidCompactPid(b0))
        );
    }
    // The rule fires inside a property table too.
    assert_eq!(
        EncodedPropertyTable::from_bytes(&[0x01, 0x81, 0x00], &schema()),
        Err(WorldError::InvalidCompactPid(0x81))
    );
}

#[test]
fn vector_3_compact_did() {
    assert_eq!(CompactDid(0x2500_0011).to_bytes().unwrap(), [0x25, 0x11]);
    assert_eq!(
        CompactDid(0x0000_1000).to_bytes().unwrap(),
        [0x00, 0x90, 0x00]
    );
    assert_eq!(
        CompactDid::from_bytes(&[0x00, 0x90, 0x00]).unwrap(),
        CompactDid(0x1000)
    );
}

#[test]
fn vector_4_encoded_property_table_wire_form() {
    let mut inner = EncodedPropertyTable::new();
    inner.push(0x1000_0CCC, EncodedPropertyValue::Boolean(true));
    let mut table = EncodedPropertyTable::new();
    table.push(0x1000_0BBB, EncodedPropertyValue::Int(7));
    table.push(0x1000_0AAA, EncodedPropertyValue::Struct(inner));
    let bytes = table.to_bytes().unwrap();
    assert_eq!(
        bytes,
        [0x02, 0x4B, 0xBB, 0x07, 0x4A, 0xAA, 0x01, 0x4C, 0xCC, 0x01]
    );
    assert_eq!(
        EncodedPropertyTable::from_bytes(&bytes, &schema()).unwrap(),
        table
    );
    assert_eq!(EncodedPropertyTable::new().to_bytes().unwrap(), [0x00]);
}

#[test]
fn vector_5_wire_and_stored_position() {
    let pos = WirePosition::from_components(
        1,
        0x10,
        0x20,
        0,
        0,
        [10.0, 20.0, 30.5],
        [1.0, 0.0, 0.0, 0.0],
    );
    assert_eq!(pos.flags, 0xDB);
    let mut wire = WireWriter::new();
    pos.encode_wire(&mut wire);
    assert_eq!(
        wire.as_slice(),
        [
            0xDB, 0x10, 0x20, 0x00, 0x00, 0x20, 0x41, 0x00, 0x00, 0xA0, 0x41, 0x00, 0x00, 0xF4,
            0x41
        ]
    );
    let mut stored = WireWriter::new();
    pos.encode_stored(&mut stored);
    assert_eq!(
        stored.as_slice(),
        [
            0xDB, 0x01, 0x10, 0x20, 0x00, 0x00, 0x00, 0x00, 0x20, 0x41, 0x00, 0x00, 0xA0, 0x41,
            0x00, 0x00, 0xF4, 0x41
        ]
    );
    let back = WirePosition::decode_wire(&mut WireReader::new(wire.as_slice())).unwrap();
    assert_eq!(back.region, 1);
    assert_eq!(back.cell, 0);
    assert_eq!(back.offset, [10.0, 20.0, 30.5]);
}

#[test]
fn vector_6_localized_token_text_wire_and_stored() {
    let text = LocalizedTokenText::reference(0x11, 0x2500_0001);
    let wire = text.to_bytes().unwrap();
    assert_eq!(
        wire,
        [
            0x00, 0x11, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x25, 0x00, 0x00
        ]
    );
    let mut stored = WireWriter::new();
    text.encode_stored(&mut stored).unwrap();
    assert_eq!(
        stored.as_slice(),
        [
            0x00, 0x11, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x25, 0x00, 0x01, 0x00
        ]
    );
    assert_eq!(LocalizedTokenText::from_bytes(&wire).unwrap(), text);

    // The stored worked example of property-system.md §17.4.1: one kind-4 argument, 32 bytes.
    let mut with_arg = text.clone();
    with_arg.arguments.push(LocalizedTokenTextArg {
        key: 0xAABB_CCDD,
        kind: 4,
        header_kind: 4,
        var_id: 0xAABB_CCDD,
        payload: Some(LocalizedTokenTextArgPayload::Int {
            is_32bit: false,
            value: 7,
        }),
    });
    let mut stored_arg = WireWriter::new();
    with_arg.encode_stored(&mut stored_arg).unwrap();
    assert_eq!(stored_arg.len(), 32);
    let back =
        LocalizedTokenText::decode_stored(&mut WireReader::new(stored_arg.as_slice())).unwrap();
    assert_eq!(back, with_arg);

    // Case B (§7.2 Vector 6): one kind-6 argument for variable 0x9A8B7C6D holding 42, 18
    // bytes on the wire: the 5-byte (u8 kind, u32 var_id) header, then is_32bit and a
    // 1-byte VariableU32.
    let mut case_b = text.clone();
    case_b.arguments.push(LocalizedTokenTextArg::new(
        0x9A8B_7C6D,
        6,
        LocalizedTokenTextArgPayload::int(42),
    ));
    let wire_b = case_b.to_bytes().unwrap();
    assert_eq!(
        wire_b,
        [
            0x00, 0x11, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x25, 0x00, 0x01, 0x06, 0x6D, 0x7C,
            0x8B, 0x9A, 0x01, 0x2A
        ]
    );
    let back_b = LocalizedTokenText::from_bytes(&wire_b).unwrap();
    assert_eq!(back_b, case_b);
    // The variable ID is the table key.
    let arg = back_b.argument(0x9A8B_7C6D).expect("the argument");
    assert_eq!(
        (arg.key, arg.kind, arg.var_id),
        (0x9A8B_7C6D, 6, 0x9A8B_7C6D)
    );
    assert_eq!(
        arg.payload,
        Some(LocalizedTokenTextArgPayload::Int {
            is_32bit: true,
            value: 42
        })
    );

    // Kinds 0 and 8..=255 fail the stream.
    for bad in [0u8, 8, 0xFF] {
        let mut b = wire_b.clone();
        b[11] = bad;
        assert_eq!(
            LocalizedTokenText::from_bytes(&b),
            Err(WorldError::InvalidStringArgKind(u16::from(bad)))
        );
    }
    // arg_count above remaining / 5 is refused before anything is read.
    let mut b = wire.clone();
    *b.last_mut().unwrap() = 0x02;
    b.extend_from_slice(&[0x06, 0, 0, 0, 0, 0x01, 0x00]);
    assert!(matches!(
        LocalizedTokenText::from_bytes(&b),
        Err(WorldError::CountExceedsRemaining { count: 2, .. })
    ));
    // A stored-only argument (kind outside 1..=7) or a payload of another kind cannot go
    // on the wire.
    assert_eq!(
        with_arg.clone().to_bytes().map(|_| ()),
        Ok(()),
        "a stored kind-4 argument writes as a wire kind-4"
    );
    let mut odd = text.clone();
    odd.arguments.push(LocalizedTokenTextArg {
        key: 1,
        kind: 9,
        header_kind: 0,
        var_id: 1,
        payload: None,
    });
    assert_eq!(odd.to_bytes(), Err(WorldError::InvalidStringArgKind(9)));
    let mut mismatched = text.clone();
    mismatched.arguments.push(LocalizedTokenTextArg::new(
        1,
        2,
        LocalizedTokenTextArgPayload::int(1),
    ));
    assert!(matches!(
        mismatched.to_bytes(),
        Err(WorldError::InvalidTextArgument(_))
    ));

    // Every kind round-trips on the wire; kinds 4 to 7 use the compact integer, whose
    // 64-bit form is taken at 0x7FFFFFFF and above.
    let payloads = [
        (
            1u16,
            LocalizedTokenTextArgPayload::Nested(Box::new(LocalizedTokenText::literal("x"))),
        ),
        (
            2,
            LocalizedTokenTextArgPayload::Float {
                value: 1.5,
                decimal_places: 1,
            },
        ),
        (
            3,
            LocalizedTokenTextArgPayload::FloatFormatted {
                value: -2.0,
                decimal_places: 0,
                format_flags: 1,
                radix: 0xFFFF,
            },
        ),
        (4, LocalizedTokenTextArgPayload::int(0x7FFF_FFFF)),
        (
            5,
            LocalizedTokenTextArgPayload::IntFormatted {
                is_32bit: true,
                value: 3,
                format_flags: 2,
                radix: 10,
            },
        ),
        (6, LocalizedTokenTextArgPayload::int(u64::MAX)),
        (
            7,
            LocalizedTokenTextArgPayload::IntFormatted {
                is_32bit: false,
                value: 1 << 40,
                format_flags: 0,
                radix: 16,
            },
        ),
    ];
    let mut all = LocalizedTokenText::reference(0x22, 0x2500_0002);
    for (i, (kind, p)) in payloads.into_iter().enumerate() {
        all.arguments
            .push(LocalizedTokenTextArg::new(0x100 + i as u32, kind, p));
    }
    assert_eq!(
        LocalizedTokenText::from_bytes(&all.to_bytes().unwrap()).unwrap(),
        all
    );
    assert_eq!(
        LocalizedTokenTextArgPayload::int(0x7FFF_FFFE),
        LocalizedTokenTextArgPayload::Int {
            is_32bit: true,
            value: 0x7FFF_FFFE
        }
    );
    assert!(!wire_int_is_32bit(0x7FFF_FFFF));
    // A 32-bit form whose value does not fit 32 bits is not written.
    let mut w = WireWriter::new();
    assert!(matches!(
        LocalizedTokenTextArgPayload::Int {
            is_32bit: true,
            value: 1 << 33
        }
        .encode(&mut w, true),
        Err(WorldError::InvalidTextArgument(_))
    ));

    // The compact integer (wire difference #3).
    let mut w = WireWriter::new();
    LocalizedTokenTextArgPayload::Int {
        is_32bit: true,
        value: 0x4000,
    }
    .encode(&mut w, true)
    .unwrap();
    assert_eq!(w.as_slice(), [0x01, 0xC0, 0x00, 0x00, 0x40]);
    let decoded =
        LocalizedTokenTextArgPayload::decode(&mut WireReader::new(w.as_slice()), 6, true).unwrap();
    assert_eq!(
        decoded,
        LocalizedTokenTextArgPayload::Int {
            is_32bit: true,
            value: 0x4000
        }
    );
    assert_eq!(
        LocalizedTokenTextArgPayload::decode(&mut WireReader::new(&[0x02]), 6, true),
        Err(WorldError::InvalidBool8(2))
    );
}

#[test]
fn localized_token_text_modes_choose_their_source() {
    // Mode 1: the literal; mode 2: the second authoring string (empty without them), never
    // the table; any other mode: the table reference (§2.6 row 1).
    assert_eq!(
        LocalizedTokenText::literal("A").source(),
        TextSource::Literal("A")
    );
    let mut m2 = LocalizedTokenText::reference(5, 0x2500_0001);
    if let LocalizedTokenTextContent::Reference { mode, .. } = &mut m2.content {
        *mode = 2;
    }
    assert_eq!(m2.source(), TextSource::AuthoringSource(""));
    m2.authoring_strings = Some(["tok".into(), "Source text".into(), "note".into()]);
    assert_eq!(m2.source(), TextSource::AuthoringSource("Source text"));
    // Mode 2 still carries its token and table on the wire and round-trips.
    let bytes = m2.to_bytes().unwrap();
    assert_eq!(bytes[0], 2);
    assert_eq!(LocalizedTokenText::from_bytes(&bytes).unwrap(), m2);
    for mode in [0u8, 3, 0xFF] {
        let mut t = LocalizedTokenText::reference(5, 0x2500_0001);
        if let LocalizedTokenTextContent::Reference { mode: m, .. } = &mut t.content {
            *m = mode;
        }
        assert_eq!(
            t.source(),
            TextSource::Table {
                token_id: 5,
                table_did: 0x2500_0001
            }
        );
    }
}

#[test]
fn signed_normalized_byte_and_heading() {
    assert_eq!(decode_signed_normalized(-128), -1.0);
    assert_eq!(decode_signed_normalized(-127), -1.0);
    assert_eq!(decode_signed_normalized(0), 0.0);
    assert_eq!(decode_signed_normalized(127), 1.0);
    assert_eq!(encode_signed_normalized(1.0), 127);
    assert_eq!(encode_signed_normalized(-2.0), -127);
    assert_eq!(encode_signed_normalized(0.5), 63);
    assert_eq!(encode_signed_normalized(f32::NAN), 0);
    for raw in [0u8, 1, 64, 128, 255] {
        assert_eq!(encode_heading_u8(decode_heading_u8(raw)), raw);
    }
}

#[test]
fn all_22_property_types_round_trip() {
    let mut nested = EncodedPropertyTable::new();
    nested.push(0x1000_0CCC, EncodedPropertyValue::Boolean(false));
    let values = [
        EncodedPropertyValue::String("abc".to_owned()),
        EncodedPropertyValue::StringToken(7),
        EncodedPropertyValue::Waveform(tessera_compat::world::WireWaveform::Constant(1.5)),
        EncodedPropertyValue::Timestamp(TIMESTAMP_NO_TIME),
        EncodedPropertyValue::TriState(255),
        EncodedPropertyValue::Vector3([1.0, 2.0, 3.0]),
        EncodedPropertyValue::InstanceId(0x0102_0304_0506_0708),
        EncodedPropertyValue::Enum(0x1000),
        EncodedPropertyValue::Float(2.5),
        EncodedPropertyValue::PropertyId(0x1000_0BBB),
        EncodedPropertyValue::Struct(nested),
        EncodedPropertyValue::Array(vec![
            EncodedPropertyEntry::new(0x1000_0BBB, EncodedPropertyValue::Int(-5)),
            EncodedPropertyEntry::null(),
        ]),
        EncodedPropertyValue::LocalizedTokenText(LocalizedTokenText::literal("hi")),
        EncodedPropertyValue::Bitfield64(u64::MAX),
        EncodedPropertyValue::Int(i32::MIN),
        EncodedPropertyValue::Color([1, 2, 3, 4]),
        EncodedPropertyValue::Position(WirePosition::default()),
        EncodedPropertyValue::Bitfield32(0xDEAD_BEEF),
        EncodedPropertyValue::Int64(-9),
        EncodedPropertyValue::DidRef(0x7000_0001),
        EncodedPropertyValue::Boolean(true),
        EncodedPropertyValue::Bitfield {
            bit_count: 10,
            bytes: vec![0xFF, 0x03],
        },
    ];
    let mut table = EncodedPropertyTable::new();
    for (code, value) in (1u32..=22).zip(values) {
        assert_eq!(u32::from(value.property_type().code()), code);
        table.push(0x0000_0100 + code, value);
    }
    let bytes = table.to_bytes().unwrap();
    assert_eq!(
        EncodedPropertyTable::from_bytes(&bytes, &schema()).unwrap(),
        table
    );
}

// ---------------------------------------------------------------------------
// §7.1 Hostile-input bounds
// ---------------------------------------------------------------------------

#[test]
fn hostile_counts_depth_bools_and_unknown_pids() {
    let s = schema();
    // EncodedPropertyTable count > remaining / 2.
    assert!(matches!(
        EncodedPropertyTable::from_bytes(&[0x05, 0x00, 0x00], &s),
        Err(WorldError::CountExceedsRemaining { .. })
    ));
    // Array count > remaining / 2.
    let mut w = WireWriter::new();
    w.write_vle3(1).unwrap();
    CompactPid(0x0000_010C).encode(&mut w);
    w.write_u32_le(1_000_000);
    assert!(matches!(
        EncodedPropertyTable::from_bytes(w.as_slice(), &s),
        Err(WorldError::CountExceedsRemaining { .. })
    ));
    // UTF-16 string length > remaining / 2.
    assert!(matches!(
        WireReader::new(&[0x10, 0x41, 0x00]).read_utf16le_string(),
        Err(WorldError::CountExceedsRemaining { .. })
    ));
    // Bitfield bytes > remaining.
    assert!(matches!(
        WireBitfield::decode(&mut WireReader::new(&[0x81, 0x00])),
        Err(WorldError::CountExceedsRemaining { .. })
    ));
    // Unknown PID fails the frame (§7.1 item 4); bool8 above 1 fails (item 3).
    assert_eq!(
        EncodedPropertyTable::from_bytes(&[0x01, 0x3F, 0xFF, 0x00], &s),
        Err(WorldError::UnknownPropertyId(0x3FFF))
    );
    assert_eq!(
        EncodedPropertyTable::from_bytes(&[0x01, 0x4C, 0xCC, 0x02], &s),
        Err(WorldError::InvalidBool8(2))
    );
    // Nesting deeper than 16 levels of Struct (§7.1 item 2).
    let mut deep = [0x01, 0x4A, 0xAA].repeat(MAX_RECURSION_DEPTH + 2);
    deep.push(0x00);
    assert!(matches!(
        EncodedPropertyTable::from_bytes(&deep, &s),
        Err(WorldError::MaxRecursionDepthExceeded { .. })
    ));
}

// ---------------------------------------------------------------------------
// §1.1 Event envelope and §7.2 Vector 7
// ---------------------------------------------------------------------------

#[test]
fn vector_7_enter_world_delete_and_ready_ack() {
    let enter = CharacterEnterWorldRequest::new(0x0102_0304_0506_0708);
    let env = enter.to_envelope_bytes().unwrap();
    assert_eq!(
        env,
        [
            0x00, 0x00, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x00
        ]
    );
    let delete = CharacterDeleteRequest {
        character_id: 0x0102_0304_0506_0708,
        monster_play: false,
    };
    assert_eq!(delete.event_id(), 0x7A);
    assert_eq!(
        delete.to_bytes().unwrap(),
        [0x00, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
    );
    assert_eq!(
        CharacterDeleteRequest::from_bytes(&delete.to_bytes().unwrap(), 0xD1)
            .unwrap()
            .event_id(),
        0xD1
    );
    assert_eq!(PortalReadyAck.to_bytes().unwrap(), [0x00]);

    let decoded = decode_event_envelope(0x6F, 2, &env, 1024).unwrap();
    assert_eq!(decoded.target_entity_id, 2);
    assert_eq!(
        CharacterEnterWorldRequest::from_bytes(&decoded.inner_payload, &schema()).unwrap(),
        enter
    );
}

#[test]
fn event_envelope_zlib_routing_and_base_flags() {
    let inner = MissingEntityQuery { entity_id: 9 }.to_bytes().unwrap();
    let zipped = encode_message(0, &inner, true).unwrap();
    let env = decode_event_envelope(0xD0, 0x0100_0000_0000_0001, &zipped, 1024).unwrap();
    assert_eq!(env.inner_payload, inner);
    assert_eq!(env.target_entity_id, 0x0100_0000_0000_0001);

    let routing = MessageRoutingHeader {
        field0: Some(1),
        ..MessageRoutingHeader::default()
    };
    let routed = encode_message_with_routing(
        ENVELOPE_FLAG_HAS_ROUTING_HEADER,
        Some(&routing),
        &inner,
        false,
    )
    .unwrap();
    let env = decode_event_envelope(0xD0, 2, &routed, 1024).unwrap();
    assert_eq!(env.routing_header, Some(routing));
    assert_eq!(env.inner_payload, inner);
    // routingField0 overrides recipientId as the target (§1.1.2 item 2).
    assert_eq!(env.target_entity_id, 1);
    assert_eq!(active_target(0x0100_0000_0000_0005, Some(&routing)), 1);
    // Without routingField0 (or without a routing header) the target is recipientId.
    let other = MessageRoutingHeader {
        field2: Some(0x0100_0000_0000_0009),
        ..MessageRoutingHeader::default()
    };
    assert_eq!(active_target(7, Some(&other)), 7);
    assert_eq!(active_target(7, None), 7);
    let routed = encode_message_with_routing(
        ENVELOPE_FLAG_HAS_ROUTING_HEADER,
        Some(&other),
        &inner,
        false,
    )
    .unwrap();
    let env = decode_event_envelope(0xD0, 0x0100_0000_0000_0001, &routed, 1024).unwrap();
    assert_eq!(env.target_entity_id, 0x0100_0000_0000_0001);

    assert_eq!(
        decode_event_envelope(0xD0, 2, &[0x00, 0x05], 1024),
        Err(WorldError::InvalidBaseFlags(0x05))
    );
    assert_eq!(
        decode_event_envelope(0xD0, 2, &[0x04, 0x00], 1024),
        Err(WorldError::Envelope(SessionError::InvalidEnvelopeFlags {
            flags: 0x04
        }))
    );
    assert_eq!(encode_event_envelope(&[0x00, 0x01]), [0x00, 0x00, 0x01]);
}

// ---------------------------------------------------------------------------
// §2.8 EntityAppearanceDesc and §3 character screen
// ---------------------------------------------------------------------------

fn sample_appearance() -> EntityAppearanceDesc {
    EntityAppearanceDesc {
        base_model: None,
        secondary_did: Some(0x2000_0001),
        scale: Some([1.0, 1.1, 1.2]),
        param_a: Some(0.5),
        tint_entries: Some(vec![AppearanceTint {
            slot: 1,
            key: 2,
            color_bgra: 0xFF00_00FF,
            field_d: 0,
            field_e: 0,
        }]),
        primary_part: Some(PartEntry {
            part_ids: [Some(1), None, Some(3), None],
            sub_map: Some(vec![(
                7,
                SubPartTable {
                    hint: 1,
                    entries: vec![SubPartEntry { k: 1, v1: 2, v2: 3 }],
                },
            )]),
        }),
        part_map: Some((1, vec![(9, PartEntry::default())])),
        param_b: Some(2.0),
    }
}

#[test]
fn entity_appearance_desc_layout_and_open_gate() {
    let desc = sample_appearance();
    assert_eq!(desc.flags(), 0x07E2);
    let mut w = WireWriter::new();
    desc.encode(&mut w).unwrap();
    let back = EntityAppearanceDesc::decode(&mut WireReader::new(w.as_slice())).unwrap();
    assert_eq!(back, desc);

    // Bit 0 (base_model) is one unconditional u32 (§2.8), whatever its high byte.
    for model in [0x3100_0001u32, 0x3200_0002, 0x0100_0001] {
        let with_model = EntityAppearanceDesc {
            base_model: Some(model),
            scale: Some([2.0, 2.0, 2.0]),
            ..EntityAppearanceDesc::default()
        };
        let mut w = WireWriter::new();
        with_model.encode(&mut w).unwrap();
        let mut expected = 0x0021u32.to_le_bytes().to_vec();
        expected.extend_from_slice(&model.to_le_bytes());
        for _ in 0..3 {
            expected.extend_from_slice(&2.0f32.to_le_bytes());
        }
        assert_eq!(w.as_slice(), expected.as_slice());
        assert_eq!(
            EntityAppearanceDesc::decode(&mut WireReader::new(w.as_slice())).unwrap(),
            with_model
        );
    }
    // A full descriptor with a base model round-trips.
    let full = EntityAppearanceDesc {
        base_model: Some(0x3100_0010),
        ..sample_appearance()
    };
    assert_eq!(full.flags(), 0x07E3);
    let mut w = WireWriter::new();
    full.encode(&mut w).unwrap();
    assert_eq!(
        EntityAppearanceDesc::decode(&mut WireReader::new(w.as_slice())).unwrap(),
        full
    );
    // A truncated base model is a framing error, not a refusal.
    assert!(matches!(
        EntityAppearanceDesc::decode(&mut WireReader::new(&[0x01, 0, 0, 0, 0x31])),
        Err(WorldError::UnexpectedEof { .. })
    ));
    // Undefined flag bits are refused.
    assert_eq!(
        EntityAppearanceDesc::decode(&mut WireReader::new(&0x0800u32.to_le_bytes())),
        Err(WorldError::InvalidAppearanceFlags(0x0800))
    );
}

fn sample_character() -> CharacterSummaryEntry {
    let mut public_props = EncodedPropertyTable::new();
    public_props.push(PID_AGENT_SPECIES, EncodedPropertyValue::Enum(0x17));
    let mut private_props = EncodedPropertyTable::new();
    private_props.push(PID_AGENT_CLASS, EncodedPropertyValue::Enum(0x18));
    CharacterSummaryEntry {
        character_id: 0x0300_0000_0000_0042,
        name: "Synthetic".to_owned(),
        status_or_world: 1,
        public_props,
        private_props,
        metadata_a: 0,
        visual_desc: sample_appearance(),
        metadata_b: 0,
        metadata_c: 0xFFFF_FFFF,
        metadata_d: 0,
        metadata_e: 0xFFFF_FFFF,
        timestamp_or_meta: 5,
    }
}

#[test]
fn character_screen_messages_round_trip() {
    let s = schema();
    let entry = sample_character();
    assert_eq!(
        entry.property(PID_AGENT_CLASS),
        Some(&EncodedPropertyValue::Enum(0x18))
    );
    let list = CharacterListResponse {
        characters: vec![entry.clone(), entry.clone()],
        account_props: EncodedPropertyTable::new(),
    };
    let bytes = list.to_bytes().unwrap();
    assert_eq!(&bytes[..5], &[0x00, 0x02, 0x00, 0x00, 0x00]);
    assert_eq!(CharacterListResponse::from_bytes(&bytes, &s).unwrap(), list);
    // char_count above what the bytes can hold is refused before allocating.
    let mut huge = vec![0x00];
    huge.extend_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        CharacterListResponse::from_bytes(&huge, &s),
        Err(WorldError::CountExceedsRemaining { .. })
    ));

    let ack = CharacterCreateAck { character: entry };
    assert_eq!(
        CharacterCreateAck::from_bytes(&ack.to_bytes().unwrap(), &s).unwrap(),
        ack
    );
    let check = CharacterNameCheckRequest {
        name: "Name".to_owned(),
    };
    assert_eq!(
        CharacterNameCheckRequest::from_bytes(&check.to_bytes().unwrap()).unwrap(),
        check
    );
    let create = CharacterCreateRequest {
        creation_props: EncodedPropertyTable::new(),
        name: "Name".to_owned(),
        char_gen_type: 1,
        flag_a: true,
        flag_b: false,
        creation_param: 0,
    };
    let mut create_bytes = create.to_bytes().unwrap();
    assert_eq!(
        CharacterCreateRequest::from_bytes(&create_bytes, &s).unwrap(),
        create
    );
    let flag_a_at = create_bytes.len() - 6;
    create_bytes[flag_a_at] = 2;
    assert_eq!(
        CharacterCreateRequest::from_bytes(&create_bytes, &s),
        Err(WorldError::InvalidBool8(2))
    );

    let reject = CharacterCreateReject {
        result_code: 3,
        name_error_code: 4,
    };
    assert_eq!(reject.to_bytes().unwrap().len(), 9);
    let status = CharacterDeleteStatus {
        character_id: 1,
        result_code: 0,
    };
    assert_eq!(status.to_bytes().unwrap().len(), 13);
    let op_err = CharacterOperationError { error_code: 1 };
    assert_eq!(op_err.to_bytes().unwrap().len(), 5);
    let platform = PlatformAccountIdReport { platform_id: 77 };
    assert_eq!(platform.to_bytes().unwrap().len(), 9);
    let evict = AccountEvictionNotice {
        boot_reason: 2,
        reason_text: LocalizedTokenText::literal("bye"),
    };
    assert_eq!(
        AccountEvictionNotice::from_bytes(&evict.to_bytes().unwrap()).unwrap(),
        evict
    );
    assert_eq!(
        CharacterDeleteRequest::from_bytes(&[0x00; 9], 0x15),
        Err(WorldError::WrongEventId(0x15))
    );
}

// ---------------------------------------------------------------------------
// §4 World entry, portal space, clocks
// ---------------------------------------------------------------------------

#[test]
fn world_entry_messages_round_trip() {
    let s = schema();
    let name = WorldNameAnnouncement {
        world_name: LocalizedTokenText::reference(1, 0x2500_0002),
    };
    assert_eq!(
        WorldNameAnnouncement::from_bytes(&name.to_bytes().unwrap()).unwrap(),
        name
    );
    let display = DisplayLocalizedText {
        target_or_channel_mask: 0xFFFF,
        message: LocalizedTokenText::literal("x"),
    };
    assert_eq!(
        DisplayLocalizedText::from_bytes(&display.to_bytes().unwrap()).unwrap(),
        display
    );
    let shutdown = ServerShutdownText {
        last_words: LocalizedTokenText::literal("later"),
    };
    assert_eq!(
        ServerShutdownText::from_bytes(&shutdown.to_bytes().unwrap()).unwrap(),
        shutdown
    );
    let ready = PlayerSessionReady {
        player_id: 0x0100_0000_0000_0007,
        session_props: EncodedPropertyTable::new(),
    };
    assert_eq!(
        PlayerSessionReady::from_bytes(&ready.to_bytes().unwrap(), &s).unwrap(),
        ready
    );
    for bytes in [
        PortalTransitionComplete.to_bytes().unwrap(),
        PortalStateNotice.to_bytes().unwrap(),
        PortalReadyAck.to_bytes().unwrap(),
    ] {
        assert_eq!(bytes, [0x00]);
    }
    assert_eq!(PortalTransitionComplete::EVENT_ID, 0xC7);
    assert_eq!(PortalStateNotice::EVENT_ID, 0x153);
    assert_eq!(
        PortalReadyAck::from_bytes(&[0x01]),
        Err(WorldError::InvalidBaseFlags(1))
    );

    // `0x7C` (login_finished) is one byte; `0x5E` (enter_portal_space) six: base_flags,
    // the expected stamp and the u32 scene_id (§4.3.1).
    assert_eq!(LoginFinished::EVENT_ID, 0x7C);
    assert_eq!(LoginFinished.to_bytes().unwrap(), [0x00]);
    assert_eq!(EnterPortalSpace::EVENT_ID, 0x5E);
    let enter = EnterPortalSpace {
        stamp: 0xFE,
        scene_id: 0x0A0B_0C0D,
    };
    let bytes = enter.to_bytes().unwrap();
    assert_eq!(bytes, [0x00, 0xFE, 0x0D, 0x0C, 0x0B, 0x0A]);
    assert_eq!(EnterPortalSpace::from_bytes(&bytes).unwrap(), enter);
    assert!(matches!(
        EnterPortalSpace::from_bytes(&bytes[..5]),
        Err(WorldError::UnexpectedEof { .. })
    ));
    assert_eq!(
        EnterPortalSpace::from_bytes(&[0x00, 1, 0, 0, 0, 0, 9]),
        Err(WorldError::TrailingBytes(1))
    );
}

/// The schema the run-time construction of §4.3.1 yields for `0x0C`; its PIDs are the
/// spec's verification table.
fn portal_schemas() -> BTreeMap<u32, PortalStateSchema> {
    let mut map = BTreeMap::new();
    map.insert(
        DEFAULT_PORTAL_STATE_SCHEMA_ID,
        PortalStateSchema {
            bool_pids: vec![
                0x0000_068A,
                0x1000_3EA0,
                0x1000_6676,
                0x1000_4157,
                0x1000_0ED8,
                0x1000_1A27,
                0x1000_0ED7,
                0x1000_0443,
            ],
            enum_pids: vec![
                0x0000_0126,
                0x0000_0201,
                0x0000_0687,
                0x1000_136E,
                0x1000_00FC,
                0x1000_0469,
            ],
        },
    );
    map.insert(0x1000_0E50, PortalStateSchema::default());
    map
}

#[test]
fn vector_8_portal_position_sync_state_section() {
    let schemas = portal_schemas();
    let sync = PortalPositionSync {
        sync_bit_0: false,
        sequence_or_reason: 3,
        position: WirePosition::from_components(
            1,
            0x10,
            0x20,
            0,
            0,
            [1.0, 2.0, 3.0],
            [1.0, 0.0, 0.0, 0.0],
        ),
        heading_raw: 64,
        movement_mode: 1,
        linear_velocity: [Some(1.0), None, Some(-1.0)],
        angular_velocity: [None, None, None],
        anim_or_context_did: CompactDid(0x0900_0001),
        state_schema_id: DEFAULT_PORTAL_STATE_SCHEMA_ID,
        bool_states: vec![true, false, false, false, true, false, false, false],
        enum_states: vec![2, 1, 3, 0, 4, 1],
        extra_blob: Vec::new(),
    };
    let bytes = sync.to_bytes(&schemas).unwrap();
    assert_eq!(
        &bytes[bytes.len() - 11..],
        &[
            0x11, 0x02, 0x01, 0x03, 0x00, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00
        ]
    );
    let back = PortalPositionSync::from_bytes(&bytes, &schemas).unwrap();
    assert_eq!(back, sync);
    // sync_flags bit 0 clear: sequence_or_reason is the acknowledged step.
    assert!(!back.is_forced());
    assert_eq!(
        (back.ack_step_seq(), back.teleport_reason()),
        (Some(3), None)
    );
    let mut forced = sync.clone();
    forced.sync_bit_0 = true;
    let fb = forced.to_bytes(&schemas).unwrap();
    assert_eq!(fb[1] & 0x01, 0x01);
    let forced = PortalPositionSync::from_bytes(&fb, &schemas).unwrap();
    assert!(forced.is_forced());
    assert_eq!(
        (forced.ack_step_seq(), forced.teleport_reason()),
        (None, Some(3))
    );
    let default = &schemas[&DEFAULT_PORTAL_STATE_SCHEMA_ID];
    assert_eq!(back.bool_state_for_pid(default, 0x1000_0ED8), Some(true));
    assert_eq!(back.enum_state_for_pid(default, 0x1000_00FC), Some(4));

    // An explicit schema id sets bit 7; an unknown one is refused.
    let mut mounted = sync.clone();
    mounted.state_schema_id = 0x1000_0E50;
    mounted.bool_states.clear();
    mounted.enum_states.clear();
    let bytes = mounted.to_bytes(&schemas).unwrap();
    assert_eq!(bytes[1] & 0x80, 0x80);
    assert_eq!(
        PortalPositionSync::from_bytes(&bytes, &schemas).unwrap(),
        mounted
    );
    assert_eq!(
        PortalPositionSync::from_bytes(&bytes, &BTreeMap::new()),
        Err(WorldError::UnknownPortalStateSchema(0x1000_0E50))
    );
}

#[test]
fn network_clock_snap_slew_and_boost() {
    let mut clock = NetworkClock::new(0.0);
    clock.on_forced_sync(100.0, 0.0);
    assert_eq!(clock.update(1.0), 101.0);
    // Behind the server: snap forward.
    clock.on_clock_sync(200.0, 1.0);
    assert_eq!(clock.update(1.0), 200.0);
    // Ahead by 0.05 s: no backward jump, tier 0.0025.
    clock.on_clock_sync(199.95, 1.0);
    assert_eq!(clock.update(1.0), 200.0);
    assert_eq!(clock.slew_rate(), 0.0025);
    assert!((clock.slew_debt() - 0.05).abs() < 1e-9);
    // Time keeps moving forward while the debt is bled off.
    let t = clock.update(11.0);
    assert!(t > 200.0 && t < 210.0);
    for (excess, rate) in [
        (0.0, 0.0),
        (0.015, 0.00025),
        (0.03, 0.0025),
        (0.06, 0.01),
        (0.12, 0.025),
        (0.24, 0.05),
    ] {
        assert_eq!(NetworkClock::base_slew_rate_for_excess(excess), rate);
    }
    // Relative timestamps follow synced time; "no time" passes through.
    assert_eq!(
        clock.decode_wire_timestamp(TIMESTAMP_NO_TIME, false),
        TIMESTAMP_NO_TIME
    );
    let synced = clock.synced_time();
    assert_eq!(clock.decode_wire_timestamp(1.0, false), synced + 1.0);
    assert_eq!(clock.decode_wire_timestamp(1.0, true), 1.0);
}

/// A synthetic calendar record DID (the run-time directory lookup gives the real one).
const CALENDAR_DID: u32 = 0x3500_0000;

fn synthetic_calendar() -> WorldCalendarConfig {
    WorldCalendarConfig {
        record_did: CALENDAR_DID,
        day_length_seconds: 1000.0,
        days_per_year: 10,
        calendar_epoch_offset: 0.0,
        base_year: 5,
        dusk_threshold: 0.75,
        dawn_threshold: 0.25,
        tod_phases: vec![(0.0, 1), (0.5, 2)],
        aux_tokens: vec![3],
        months: vec![(0, 4), (5, 5)],
        era_string_token: 6,
    }
}

#[test]
fn world_calendar_record_and_clock() {
    let cal = synthetic_calendar();
    let bytes = cal.to_bytes().unwrap();
    assert_eq!(
        WorldCalendarConfig::from_bytes(&bytes, CALENDAR_DID).unwrap(),
        cal
    );
    // §4.4.2: the record must carry the DID its directory key resolved to, whatever it is.
    let mut other = cal.clone();
    other.record_did = 0x3500_0007;
    assert_eq!(
        WorldCalendarConfig::from_bytes(&other.to_bytes().unwrap(), 0x3500_0007).unwrap(),
        other
    );
    assert!(matches!(
        WorldCalendarConfig::from_bytes(&bytes, 0x3500_0007),
        Err(WorldError::InvalidWorldCalendar(_))
    ));
    let mut bad = bytes.clone();
    bad[4..8].copy_from_slice(&0.0f32.to_le_bytes());
    assert!(matches!(
        WorldCalendarConfig::from_bytes(&bad, CALENDAR_DID),
        Err(WorldError::InvalidWorldCalendar(_))
    ));
    let mut bad_did = bytes;
    bad_did[0] = 1;
    assert!(matches!(
        WorldCalendarConfig::from_bytes(&bad_did, CALENDAR_DID),
        Err(WorldError::InvalidWorldCalendar(_))
    ));

    let mut clock = WorldClock::new(cal);
    let first = clock.tick(10_500.0);
    assert_eq!(first.calendar_year, 6);
    assert_eq!(first.day_of_year, 0);
    assert!((first.present_time_of_day - 0.5).abs() < 1e-6);
    assert!(!first.is_night);
    let later = clock.tick(10_900.0);
    assert!(later.is_night);
    let next_day = clock.tick(11_100.0);
    assert_eq!(next_day.day_of_year, 1);
    clock.set_time_dilation(0.0);
    assert_eq!(clock.tick(20_000.0).day_of_year, 1);
}

// ---------------------------------------------------------------------------
// §5 Object lifecycle, movement, actions, properties
// ---------------------------------------------------------------------------

#[test]
fn sub_event_batches_tail_null_frames_and_bounds() {
    let create = EntityCreate {
        template_did: 0x7000_0001,
        entity_id: 0x0100_0000_0000_0009,
    };
    assert_eq!(create.to_bytes().unwrap().len(), 13);
    let frames = vec![
        SubEventFrame::Event {
            sub_event_id: EntityCreate::EVENT_ID,
            payload: create.to_bytes().unwrap(),
        },
        SubEventFrame::Null {
            field_a: 0x4000,
            field_b: 7,
        },
    ];
    let aoi = SubEventBatch {
        event_id: SubEventBatch::EVENT_ID_AOI_ENTER,
        frames: frames.clone(),
        entry_batch_tail_flag: Some(1),
    };
    let bytes = aoi.to_bytes().unwrap();
    assert_eq!(*bytes.last().unwrap(), 1);
    assert_eq!(SubEventBatch::from_bytes(&bytes, 0x0D).unwrap(), aoi);

    let update = SubEventBatch {
        event_id: SubEventBatch::EVENT_ID_SUB_EVENT,
        frames,
        entry_batch_tail_flag: None,
    };
    let bytes = update.to_bytes().unwrap();
    assert_eq!(bytes.len(), aoi.to_bytes().unwrap().len() - 1);
    assert_eq!(SubEventBatch::from_bytes(&bytes, 0x4C).unwrap(), update);

    // sub_event_count above remaining / 4, payload_size above remaining, wrong ID.
    assert!(matches!(
        SubEventBatch::from_bytes(&[0x00, 0x05, 0x01, 0x00, 0x00], 0x4C),
        Err(WorldError::CountExceedsRemaining { .. })
    ));
    let mut over = vec![0x00, 0x01, 0x52];
    over.extend_from_slice(&100u32.to_le_bytes());
    assert!(matches!(
        SubEventBatch::from_bytes(&over, 0x4C),
        Err(WorldError::CountExceedsRemaining { .. })
    ));
    assert_eq!(
        SubEventBatch::from_bytes(&[0x00, 0x00], 0x52),
        Err(WorldError::WrongEventId(0x52))
    );
    assert_eq!(EntityDespawn.to_bytes().unwrap(), [0x00]);
    assert_eq!(
        MissingEntityQuery { entity_id: 1 }
            .to_bytes()
            .unwrap()
            .len(),
        9
    );
}

fn ce_with_heading() -> PositionUpdate {
    PositionUpdate {
        entity_id: 0x0100_0000_0000_0003,
        seq_byte: None,
        velocity: [None, None, None],
        anim_state: None,
        placement: EntityPlacement::WorldWithHeading {
            position: WirePosition::from_components(
                1,
                1,
                2,
                0,
                0,
                [5.0, 6.0, 7.0],
                [1.0, 0.0, 0.0, 0.0],
            ),
            heading_raw: 128,
        },
        interp_param_a: None,
        interp_param_b: None,
    }
}

#[test]
fn position_update_seq_byte_follows_header_flags_only() {
    // header_flags bit 3 set: seq_byte present.
    let mut ce = ce_with_heading();
    ce.seq_byte = Some(0x2A);
    let bytes = ce.to_bytes().unwrap();
    assert_eq!(bytes[1], 0x08);
    assert_eq!(bytes[11], 0x2A);
    assert_eq!(PositionUpdate::from_bytes(&bytes).unwrap(), ce);

    // mode_flags bit 3 set (extended attachment) with header_flags bit 3 clear: no seq_byte.
    let attached = PositionUpdate {
        placement: EntityPlacement::Attachment {
            parent_id: 0x0100_0000_0000_0004,
            attach_slot: 3,
            extension: Some(AttachmentExtension {
                attach_param: 9,
                attach_sub: 1,
                attach_flag: true,
            }),
        },
        ..ce_with_heading()
    };
    let bytes = attached.to_bytes().unwrap();
    assert_eq!(bytes[1], 0x00);
    assert_eq!(bytes[10], 0x0C);
    // The byte after mode_flags is the parent ID, not a sequence stamp.
    assert_eq!(&bytes[11..19], &0x0100_0000_0000_0004u64.to_le_bytes());
    assert_eq!(PositionUpdate::from_bytes(&bytes).unwrap(), attached);
}

#[test]
fn position_update_full_position_defaults_and_hostile_values() {
    let mut full = ce_with_heading();
    full.placement = EntityPlacement::WorldFull {
        position: WirePosition::from_components(
            1,
            1,
            2,
            0,
            0,
            [1.0, 2.0, 3.0],
            [0.0, 1.0, 0.0, 0.0],
        ),
    };
    full.velocity = [Some(1.0), Some(0.0), None];
    full.anim_state = Some(0x1234);
    full.interp_param_b = Some(9);
    let bytes = full.to_bytes().unwrap();
    assert_eq!(bytes[10], 0x10);
    let back = PositionUpdate::from_bytes(&bytes).unwrap();
    assert_eq!(back, full);
    assert_eq!(back.interp_param_a_or_default(), 5);
    assert_eq!(back.interp_param_b_or_default(), 9);

    // Undefined header bit 0x80 and mode bit 0x01 are refused.
    let mut bad = ce_with_heading().to_bytes().unwrap();
    bad[1] = 0x80;
    assert!(matches!(
        PositionUpdate::from_bytes(&bad),
        Err(WorldError::UndefinedFlagBits { .. })
    ));
    let mut bad = ce_with_heading().to_bytes().unwrap();
    bad[10] = 0x01;
    assert!(matches!(
        PositionUpdate::from_bytes(&bad),
        Err(WorldError::UndefinedFlagBits { .. })
    ));
    // Non-finite velocity and out-of-range coordinates (play §9.1 rule 4).
    let mut nan = ce_with_heading();
    nan.velocity = [Some(f32::NAN), None, None];
    assert!(matches!(
        PositionUpdate::from_bytes(&nan.to_bytes().unwrap()),
        Err(WorldError::NonFiniteFloat { .. })
    ));
    let mut far = ce_with_heading();
    far.placement = EntityPlacement::WorldWithHeading {
        position: WirePosition::from_components(
            1,
            1,
            2,
            0,
            0,
            [1.0e6, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
        ),
        heading_raw: 0,
    };
    assert!(matches!(
        PositionUpdate::from_bytes(&far.to_bytes().unwrap()),
        Err(WorldError::CoordinateOutOfRange { .. })
    ));
}

#[test]
fn position_update_refuses_mode_combinations_it_cannot_write_back() {
    // mode_flags byte is at offset 10 (base_flags, header_flags, 8-byte entity ID).
    // 0x08 (extended attachment data) without 0x04 (attachment mode).
    let mut bytes = ce_with_heading().to_bytes().unwrap();
    bytes[10] = 0x08;
    assert_eq!(
        PositionUpdate::from_bytes(&bytes),
        Err(WorldError::UndefinedFlagBits {
            field: "0xCE mode_flags",
            bits: 0x08,
        })
    );
    // 0x10 (full world Position) together with 0x04 (attachment mode).
    let attached = PositionUpdate {
        placement: EntityPlacement::Attachment {
            parent_id: 4,
            attach_slot: 1,
            extension: None,
        },
        ..ce_with_heading()
    };
    let mut bytes = attached.to_bytes().unwrap();
    assert_eq!(bytes[10], 0x04);
    assert_eq!(PositionUpdate::from_bytes(&bytes).unwrap(), attached);
    bytes[10] = 0x14;
    assert_eq!(
        PositionUpdate::from_bytes(&bytes),
        Err(WorldError::UndefinedFlagBits {
            field: "0xCE mode_flags",
            bits: 0x10,
        })
    );
}

#[test]
fn base_position_drops_the_orientation_bit() {
    // A base Position (0xCE world mode with heading) carries no quaternion (§2.5.1 row 7):
    // bit 0x20 in its flags byte is not kept, so a decode / encode round trip is exact.
    let mut bytes = ce_with_heading().to_bytes().unwrap();
    // The Position flags byte follows mode_flags at offset 11.
    bytes[11] |= 0x20;
    let decoded = PositionUpdate::from_bytes(&bytes).unwrap();
    let EntityPlacement::WorldWithHeading { position, .. } = &decoded.placement else {
        panic!("world placement expected");
    };
    assert_eq!(position.flags & 0x20, 0);
    assert_eq!(decoded, ce_with_heading());
    bytes[11] &= !0x20;
    assert_eq!(decoded.to_bytes().unwrap(), bytes);
}

fn full_action_payload() -> ActionPayload {
    ActionPayload {
        action_mode: 10,
        scalar_param: Some(0.5),
        sub_entry: Some(ActionSubEntry {
            sub_action_id: 0x2000_0000,
            sub_entry_id: 0x2000_0001,
            sub_entry_flag: true,
        }),
        target_id_a: Some(1),
        target_id_b: Some(2),
        action_props: None,
        state_code: Some(0x2000_0002),
        speed_or_scale: Some(1.5),
        timestamp: 12.5,
        aux_index: Some(0xFFFF_FFFF),
    }
}

#[test]
fn action_payload_sizes_and_stored_only_bit() {
    let s = schema();
    let minimal = ActionPayload::default();
    let mut w = WireWriter::new();
    minimal.encode(&mut w).unwrap();
    assert_eq!(w.len(), 11);
    let full = full_action_payload();
    let mut w = WireWriter::new();
    full.encode(&mut w).unwrap();
    assert_eq!(w.len(), 56);
    assert_eq!(
        ActionPayload::decode(&mut WireReader::new(w.as_slice()), &s).unwrap(),
        full
    );
    // Bit 2 (0x0004) is set only by stored descriptors; on the wire it is refused.
    let mut stored_bit = vec![0x0A, 0x04, 0x00];
    stored_bit.extend_from_slice(&0f64.to_le_bytes());
    assert!(matches!(
        ActionPayload::decode(&mut WireReader::new(&stored_bit), &s),
        Err(WorldError::UndefinedFlagBits { .. })
    ));
}

#[test]
fn action_events_round_trip() {
    let s = schema();
    let cmd = ClientActionCommand {
        entity_id: 5,
        action_id: CompactDid(0x7000_0010),
        payload: full_action_payload(),
    };
    assert_eq!(
        ClientActionCommand::from_bytes(&cmd.to_bytes().unwrap(), &s).unwrap(),
        cmd
    );
    let start = ActionStart {
        action_id: CompactDid(0x7000_0011),
        action_seq: 4,
        payload: ActionPayload::default(),
        bitfield: None,
    };
    assert_eq!(start.event_id(), 0x5A);
    assert_eq!(
        ActionStart::from_bytes(&start.to_bytes().unwrap(), 0x5A, &s).unwrap(),
        start
    );
    let with_bits = ActionStart {
        bitfield: Some(WireBitfield {
            bit_count: 3,
            bytes: vec![0x05],
        }),
        ..start
    };
    assert_eq!(with_bits.event_id(), 0xA8);
    assert_eq!(
        ActionStart::from_bytes(&with_bits.to_bytes().unwrap(), 0xA8, &s).unwrap(),
        with_bits
    );
    let step = ActionStepComplete {
        action_id: CompactDid(1),
        action_seq: 2,
        completion_timestamp: 3.0,
    };
    assert_eq!(
        ActionStepComplete::from_bytes(&step.to_bytes().unwrap()).unwrap(),
        step
    );
    let cancel = ActionCancel {
        action_id: CompactDid(1),
        action_seq: 2,
        reason_code: 3,
    };
    assert_eq!(
        ActionCancel::from_bytes(&cancel.to_bytes().unwrap()).unwrap(),
        cancel
    );
    let chain = ActionChainSync {
        chain_mode: 1,
        actions: vec![(CompactDid(1), 2, ActionPayload::default())],
        completions: vec![ActionChainCompletion {
            action_id: CompactDid(3),
            action_seq: 4,
            timestamp: 5.0,
            flag: true,
        }],
    };
    assert_eq!(
        ActionChainSync::from_bytes(&chain.to_bytes().unwrap(), &s).unwrap(),
        chain
    );
    let layers = EntityAnimLayerState {
        layers: vec![ActionLayerEntry {
            action_did: CompactDid(9),
            action_seq: 1,
            payload: ActionPayload::default(),
            bitfield: WireBitfield::default(),
        }],
    };
    assert_eq!(
        EntityAnimLayerState::from_bytes(&layers.to_bytes().unwrap(), &s).unwrap(),
        layers
    );
    let enable = ActionQueueEnable { enabled: true };
    assert_eq!(enable.to_bytes().unwrap(), [0x00, 0x01]);
    assert_eq!(
        ActionQueueEnable::from_bytes(&[0x00, 0x02]),
        Err(WorldError::InvalidBool8(2))
    );
    assert_eq!(ActionStateReset.to_bytes().unwrap(), [0x00]);
}

#[test]
fn client_physics_sync_base_length_and_slot_flags() {
    let s = schema();
    let base = ClientPhysicsSync::new_base(
        7,
        0x2A,
        2,
        [0x80, 0x80, 0x87, 0x87],
        [0, 0x02, 0x40, 0x42],
        [[0, 0, 0, 0xC0], [0; 4], [0x10, 0, 0, 0]],
    );
    let bytes = base.to_bytes().unwrap();
    assert_eq!(bytes.len(), ClientPhysicsSync::BASE_BYTE_LEN);
    assert_eq!(bytes[9], 0x2A);
    assert_eq!(&bytes[18..22], &[0x00, 0x02, 0x40, 0x42]);
    assert_eq!(&bytes[22..26], &[0, 0, 0, 0xC0]);
    assert_eq!(base.slot_sequences(), [0, 0, 1, 2]);
    assert_eq!(ClientPhysicsSync::from_bytes(&bytes, &s).unwrap(), base);
    assert!(base.jump(1) && !base.walking(1));
    assert!(base.walking(2) && !base.jump(2));
    assert!(base.jump(3) && base.walking(3));
    assert!(!base.jump(4) && !base.walking(4));

    // A sender writes 0 under mask 0xAC whatever the struct holds; jump and walk stay.
    let mut noisy = base.clone();
    noisy.slot_flags = [0xAC, 0xAE, 0xEC, 0xEE];
    assert_eq!(noisy.to_bytes().unwrap(), bytes);
    // A receiver ignores bits under 0xAC: they frame nothing.
    let mut received = bytes.clone();
    for f in &mut received[18..22] {
        *f |= 0xAC;
    }
    let decoded = ClientPhysicsSync::from_bytes(&received, &s).unwrap();
    assert_eq!(decoded.slot_flags, [0xAC, 0xAE, 0xEC, 0xEE]);
    assert!(decoded.slot_actions.iter().all(Vec::is_empty));
    assert!(decoded.slot_props.iter().all(Option::is_none));

    // Actions in one slot serialize a count for all four; properties only where flagged.
    let mut busy = base;
    busy.slot_actions[2].push((CompactDid(0x7000_0001), ActionPayload::default()));
    busy.slot_props[3] = Some(EncodedPropertyTable::new());
    let bytes = busy.to_bytes().unwrap();
    assert_eq!(&bytes[18..22], &[0x00, 0x02, 0x41, 0x52]);
    busy.slot_flags = busy.encoded_slot_flags();
    assert_eq!(ClientPhysicsSync::from_bytes(&bytes, &s).unwrap(), busy);

    // The u8 action count of a slot cannot exceed 255.
    let mut crowded = busy;
    crowded.slot_actions[0] = vec![(CompactDid(0x7000_0001), ActionPayload::default()); 256];
    assert_eq!(
        crowded.to_bytes(),
        Err(WorldError::CountOverflow { count: 256 })
    );
}

#[test]
fn visual_and_property_events_round_trip() {
    let s = schema();
    let visual = VisualDescriptionUpdate {
        visual_desc: sample_appearance(),
    };
    assert_eq!(
        VisualDescriptionUpdate::from_bytes(&visual.to_bytes().unwrap()).unwrap(),
        visual
    );
    let mut props = EncodedPropertyTable::new();
    props.push(PID_CURRENT_MORALE, EncodedPropertyValue::Float(10.0));
    for (scope, id) in [
        (PropertyScope::Entity, 0x52),
        (PropertyScope::Session, 0xFE),
    ] {
        let update = PropertyTableUpdate {
            scope,
            properties: props.clone(),
        };
        assert_eq!(update.event_id(), id);
        let mut w = WireWriter::new();
        update.encode(&mut w).unwrap();
        assert_eq!(
            PropertyTableUpdate::decode(&mut WireReader::new(w.as_slice()), id, &s).unwrap(),
            update
        );
    }
    for id in [0x88u32, 0x92, 0xB0, 0x149] {
        let single = SinglePropertyUpdate {
            event_id: id,
            entry: EncodedPropertyEntry::new(PID_CURRENT_MORALE, EncodedPropertyValue::Float(1.0)),
        };
        let mut w = WireWriter::new();
        single.encode(&mut w).unwrap();
        assert_eq!(
            SinglePropertyUpdate::decode(&mut WireReader::new(w.as_slice()), id, &s).unwrap(),
            single
        );
    }
    for id in [0x0Eu32, 0xE7, 0x76, 0x14A] {
        let removal = PropertyRemoval {
            event_id: id,
            property_id: 0x1000_0001,
        };
        let mut w = WireWriter::new();
        removal.encode(&mut w).unwrap();
        assert_eq!(w.len(), 5);
        assert_eq!(
            PropertyRemoval::decode(&mut WireReader::new(w.as_slice()), id).unwrap(),
            removal
        );
    }
    assert_eq!(
        PropertyRemoval::decode(&mut WireReader::new(&[0; 5]), 0x88),
        Err(WorldError::WrongEventId(0x88))
    );
    let string_event = EntityStringEvent {
        text: "tag".to_owned(),
    };
    assert_eq!(
        EntityStringEvent::from_bytes(&string_event.to_bytes().unwrap()).unwrap(),
        string_event
    );
    let position =
        WirePosition::from_components(1, 3, 4, 0, 0x0100, [0.0; 3], [1.0, 0.0, 0.0, 0.0]);
    for op in [
        LandblockPropertyOp::Table {
            flag: 1,
            properties: props.clone(),
        },
        LandblockPropertyOp::Single(EncodedPropertyEntry::new(
            PID_CURRENT_MORALE,
            EncodedPropertyValue::Float(2.0),
        )),
        LandblockPropertyOp::Remove(0x1000_0002),
    ] {
        let event = LandblockPropertyEvent { position, op };
        let mut w = WireWriter::new();
        event.encode(&mut w).unwrap();
        assert_eq!(
            LandblockPropertyEvent::decode(
                &mut WireReader::new(w.as_slice()),
                event.event_id(),
                &s
            )
            .unwrap(),
            event
        );
    }
    let ack = PredictedPropertyAck {
        property_id: 1,
        sequence: 0xFFFF_FFFF,
    };
    assert_eq!(ack.to_bytes().unwrap().len(), 9);
    assert_eq!(
        PredictedPropertyAck::from_bytes(&ack.to_bytes().unwrap()).unwrap(),
        ack
    );
}
