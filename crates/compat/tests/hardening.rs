// SPDX-License-Identifier: GPL-3.0-or-later
//! Regression tests for the hostile-input fixes of the compat cleanup: each
//! test drives one input that used to panic, spin or grow without bound.

use std::net::{Ipv4Addr, SocketAddrV4};

use tessera_compat::login::{LauncherConfig, LoginError, MAX_XML_INPUT_BYTES, UserPreferences};
use tessera_compat::session::{
    BitTreeCodec, BlobFragmentHeader, ConnectChallengeHeader, ConnectionState, DatagramHeader,
    InMemoryUdpTransport, MAX_CONTAINER_DEPTH, MAX_NAK_ENTRIES, MAX_PENDING_OUT_OF_ORDER_ACKS,
    MAX_UNPACKED_MESSAGES, OPCODE_BATCH_CONTAINER, OPCODE_SINGLE_WRAPPER, OptionalHeaders,
    RuntimeBitTreeTables, SessionConnection, SessionError, SessionMultiplexer, dispatch_message,
    encode_batch_container, encode_datagram, encode_message, encode_single_wrapper,
    encode_variable_u32,
};

fn peer(last: u8) -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, last), 9000)
}

fn unsequenced(opt: &OptionalHeaders) -> Vec<u8> {
    encode_datagram(DatagramHeader::default(), opt, &[], None, None).expect("encode datagram")
}

/// A caller-built table whose fast-lookup entry is `i16::MIN` used to negate
/// out of range; it must now fail cleanly.
#[test]
fn bit_tree_table_with_minimum_entry_does_not_panic() {
    let mut tables = RuntimeBitTreeTables::synthetic_canonical();
    tables.fast_lookup.fill(i16::MIN);
    let mut out = Vec::new();
    assert!(tables.decode(&[0xFF, 0x00, 0x12], &mut out, 64).is_err());

    // Every 9-bit prefix leads into the tree, whose only node holds
    // `i16::MIN` leaves (codes longer than 9 bits); those used to be negated
    // out of range and are now refused as symbols outside `0..=0x100`.
    let tables = RuntimeBitTreeTables {
        codewords: [0; 257],
        bit_lengths: [10; 257],
        fast_lookup: [1; 512],
        tree_transitions: vec![[i16::MAX, i16::MAX], [i16::MIN, i16::MIN]],
    };
    let mut out = Vec::new();
    assert_eq!(
        tables.decode(&[0xFF; 8], &mut out, 64),
        Err(SessionError::WireDecompressionFailed)
    );
    assert!(out.is_empty());
}

/// A sequence group wider than its 16-bit field used to be truncated silently.
#[test]
fn oversized_sequence_group_is_rejected() {
    // queueId 3, default recipient (0x40) and no chunk information (0x80).
    let mut bytes = vec![0x03, 0xC0];
    encode_variable_u32(0x0001_0000, &mut bytes);
    bytes.extend_from_slice(&[0x00, 0x00]); // orderNum
    bytes.extend_from_slice(&[0x00; 4]);
    assert_eq!(
        BlobFragmentHeader::decode(&bytes).map(|_| ()),
        Err(SessionError::SequenceGroupOutOfRange { value: 0x0001_0000 })
    );

    let mut ok = vec![0x03, 0xC0];
    encode_variable_u32(0xFFFF, &mut ok);
    ok.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    let (hdr, _) = BlobFragmentHeader::decode(&ok).expect("16-bit group decodes");
    assert_eq!(hdr.seq_group, 0xFFFF);
}

/// Sequence NAKs for packets never sent used to queue out-of-order
/// acknowledgements without bound.
#[test]
fn out_of_order_ack_queue_is_capped_and_counted() {
    let server = peer(1);
    let mut conn = SessionConnection::new(0x0100, server, 0.0);
    let per_packet = u32::try_from(MAX_NAK_ENTRIES).unwrap();
    let packets = u32::try_from(MAX_PENDING_OUT_OF_ORDER_ACKS / MAX_NAK_ENTRIES + 2).unwrap();
    for p in 0..packets {
        let first = 1_000 + p * per_packet;
        let opt = OptionalHeaders {
            sequence_nak: Some((first..first + per_packet).collect()),
            ..OptionalHeaders::default()
        };
        conn.receive_datagram(&unsequenced(&opt), server, 0.1, None)
            .expect("NAK datagram accepted");
    }
    let sent = usize::try_from(packets * per_packet).unwrap();
    assert_eq!(
        conn.pending_out_of_order_acks(),
        MAX_PENDING_OUT_OF_ORDER_ACKS
    );
    assert_eq!(
        conn.dropped_out_of_order_acks(),
        u64::try_from(sent - MAX_PENDING_OUT_OF_ORDER_ACKS).unwrap()
    );
}

fn challenge(assigned: u16) -> Vec<u8> {
    unsequenced(&OptionalHeaders {
        connect_challenge: Some(ConnectChallengeHeader {
            assigned_client_rec_id: assigned,
            server_rec_id: 0x0222,
            ..ConnectChallengeHeader::default()
        }),
        ..OptionalHeaders::default()
    })
}

/// A peer that assigned a recipient ID held by another connection used to
/// replace (drop) that connection.
#[test]
fn recipient_id_collision_never_replaces_another_connection() {
    let mut mux = SessionMultiplexer::new();
    let a = mux.create_connection(peer(1), 0.0).unwrap();
    let b = mux.create_connection(peer(2), 0.0).unwrap();
    assert_ne!(a, b);

    let err = mux
        .receive_from_peer(&challenge(b), peer(1), 0.1, None)
        .unwrap_err();
    assert_eq!(err, SessionError::RecipientIdCollision { rec_id: b });
    assert_eq!(mux.connection(b).unwrap().remote_addr(), peer(2));

    // Connection `a` was refused before anything changed.
    let conn_a = mux.connection(a).unwrap();
    assert_eq!(conn_a.remote_addr(), peer(1));
    assert_eq!(conn_a.local_rec_id(), a);
    assert_eq!(conn_a.server_rec_id(), 0);
    assert_eq!(conn_a.state(), ConnectionState::Idle);
    let mut transport = InMemoryUdpTransport::new(peer(9));
    assert_eq!(
        mux.flush_outbound(&mut transport).unwrap(),
        0,
        "no ConnectAccept queued"
    );

    // A datagram for `b` still reaches `b`, not `a`.
    let (id, _) = mux
        .receive_from_peer(&challenge(0), peer(2), 0.2, None)
        .expect("b accepts its own challenge");
    assert_eq!(id, b);
}

/// Assigned recipient IDs above the local range are ignored.
#[test]
fn out_of_range_assigned_recipient_id_is_ignored() {
    let mut mux = SessionMultiplexer::new();
    let a = mux.create_connection(peer(1), 0.0).unwrap();
    let (id, _) = mux
        .receive_from_peer(&challenge(0xFFF0), peer(1), 0.1, None)
        .expect("challenge accepted");
    assert_eq!(id, a);
    assert_eq!(mux.connection(a).unwrap().local_rec_id(), a);
}

/// A full recipient-ID range used to make allocation spin forever.
#[test]
fn recipient_id_allocation_is_bounded() {
    let mut mux = SessionMultiplexer::new();
    let range = usize::from(0xFEFFu16 - 0x0100) + 1;
    for _ in 0..range {
        mux.create_connection(peer(1), 0.0)
            .expect("free recipient ID");
    }
    assert_eq!(
        mux.create_connection(peer(1), 0.0),
        Err(SessionError::RecipientIdsExhausted)
    );
}

/// Many small compressed sub-messages used to expand to the per-message limit
/// each; the decompressed total is now bounded for the whole container tree.
#[test]
fn nested_decompression_is_budgeted_per_container_tree() {
    let sub = encode_message(0, &vec![0u8; 64 * 1024], true).unwrap();
    assert!(sub.len() < 1024, "zeros compress well");

    let fits: Vec<(u32, Vec<u8>)> = (0..8).map(|i| (0x10 + i, sub.clone())).collect();
    let container = encode_batch_container(&fits, false).unwrap();
    let out = dispatch_message(OPCODE_BATCH_CONTAINER, &container, false, 1024 * 1024, 4).unwrap();
    assert_eq!(out.len(), 8);

    // Sixteen sub-messages use the 1 MiB budget up; the seventeenth may only
    // inflate into what is left (nothing), so it is refused on its declared
    // length and peak memory never exceeds the budget.
    let too_many: Vec<(u32, Vec<u8>)> = (0..32).map(|i| (0x10 + i, sub.clone())).collect();
    let container = encode_batch_container(&too_many, false).unwrap();
    assert_eq!(
        dispatch_message(OPCODE_BATCH_CONTAINER, &container, false, 1024 * 1024, 4),
        Err(SessionError::UnpackBudgetExceeded {
            what: "bytes",
            actual: 17 * 64 * 1024,
            limit: 1024 * 1024,
        })
    );
}

/// A caller-supplied depth is clamped, so a long chain of single-message
/// wrappers cannot recurse deeper than `MAX_CONTAINER_DEPTH`.
#[test]
fn container_depth_is_clamped() {
    let mut payload = encode_message(0, b"leaf", false).unwrap();
    let mut opcode = 0x10u32;
    for _ in 0..(MAX_CONTAINER_DEPTH + 8) {
        payload = encode_single_wrapper(opcode, &payload, false).unwrap();
        opcode = OPCODE_SINGLE_WRAPPER;
    }
    assert_eq!(
        dispatch_message(
            OPCODE_SINGLE_WRAPPER,
            &payload,
            false,
            1024 * 1024,
            usize::MAX
        ),
        Err(SessionError::ContainerRecursionLimitExceeded {
            max_depth: MAX_CONTAINER_DEPTH
        })
    );
}

/// The number of leaf messages of one container tree is capped.
#[test]
fn unpacked_message_count_is_capped() {
    let leaf = encode_message(0, &[], false).unwrap();
    let subs: Vec<(u32, Vec<u8>)> = (0..=MAX_UNPACKED_MESSAGES)
        .map(|_| (0x10, leaf.clone()))
        .collect();
    let container = encode_batch_container(&subs, false).unwrap();
    assert!(matches!(
        dispatch_message(
            OPCODE_BATCH_CONTAINER,
            &container,
            false,
            16 * 1024 * 1024,
            4
        ),
        Err(SessionError::UnpackBudgetExceeded {
            what: "messages",
            ..
        })
    ));
}

/// Local configuration files above the parser limit used to be read into
/// memory whole before the size check.
#[test]
fn oversized_configuration_files_are_refused_before_reading() {
    let dir = tempfile::tempdir().unwrap();
    let big = vec![b' '; MAX_XML_INPUT_BYTES + 1];
    std::fs::write(dir.path().join("lotro.launcherconfig"), &big).unwrap();
    std::fs::write(dir.path().join("UserPreferences.ini"), &big).unwrap();

    // The detail is the file reader's, not the parser's: the file was refused
    // after at most `MAX_XML_INPUT_BYTES + 1` bytes, before parsing.
    let refused = LoginError::InputTooLarge {
        class: tessera_compat::login::LoginFailureClass::F02ConfigInvalid,
        detail: "configuration file exceeds MAX_XML_INPUT_BYTES",
    };
    assert_eq!(LauncherConfig::discover(dir.path()).unwrap_err(), refused);
    assert_eq!(UserPreferences::discover(dir.path()).unwrap_err(), refused);
}
