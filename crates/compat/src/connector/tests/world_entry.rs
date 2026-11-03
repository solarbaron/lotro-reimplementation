// SPDX-License-Identifier: GPL-3.0-or-later
//! The road into the world and the phase table of `official-session.md` §9.3.8, at driver level
//! on a virtual clock, on a network with no source for readiness, like every production path.

use super::*;
use crate::connector::session_phase::{
    ENTER_WORLD_TIMEOUT_SEC, ENTRY_REFUSED, EntryWait, SELECTION_DESTINATION,
    UNSUPPORTED_ENTRY_ANSWER,
};
use crate::session::{
    DisconnectReason as LinkEnd, FLAG_HAS_FRAGMENTED_BLOBS, FLAG_MASKED_CHECKSUM,
    TransportErrorHeader, combine_packet_checksum, compute_datagram_checksum,
};
use crate::world::{SubEventBatch, SubEventFrame};

fn no_readiness() -> Service {
    Service {
        no_readiness: true,
        ..Service::default()
    }
}

const ENTER: fn() -> Command = || Command::EnterWorld {
    character: "Aldo".to_owned(),
};

fn sent_packets(world: &World) -> Vec<crate::session::DecodedPacket> {
    world
        .take_sent()
        .iter()
        .filter_map(|(d, _)| decode_client_datagram(d, false).ok())
        .collect()
}

fn opcodes(packets: &[crate::session::DecodedPacket]) -> Vec<u32> {
    packets
        .iter()
        .flat_map(|p| &p.fragments)
        .filter_map(|f| f.header.blob_header_word)
        .collect()
}

/// A session past the referral with its world connection in State 5.
fn at_session_start(now: f64) -> (World, SessionDriver<FakeNet>, Server, Server) {
    let (world, mut d, login) = driver_at_characters(no_readiness(), 0);
    d.command(&ENTER(), now).expect("enter");
    d.step(now + 0.1, true, &go);
    world.deliver(
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
        LOGIN_SERVER,
    );
    d.step(now + 0.2, true, &go);
    let server = Server::new(0x0222, 0xBEEF_0002);
    server.connect_to(&world, 0xBEEF_0002, WORLD_SERVER);
    d.step(now + 0.3, true, &go);
    world.take_sent();
    let _ = d.take_events();
    (world, d, login, server)
}

fn timed_out_with(events: &[Event], text: &str) -> bool {
    events.iter().any(|e| {
        matches!(e, Event::Disconnected { reason: DisconnectReason::TimedOutLocally(t) } if t == text)
    })
}

/// §9.3.8: `0x6F` goes on queue 2 to the reserved destination with the empty table, and neither
/// `0x19E` nor `0x13D` goes with it.
#[test]
fn the_selection_is_sent_and_nothing_else() {
    let (world, mut d, _login) = driver_at_characters(no_readiness(), 0);
    world.take_sent();
    d.command(&ENTER(), 1.0).expect("enter");
    d.step(1.1, true, &go);
    let sent = sent_packets(&world);
    assert_eq!(opcodes(&sent), [0x6F]);
    let frag = sent
        .iter()
        .flat_map(|p| &p.fragments)
        .next()
        .expect("the fragment");
    assert_eq!(frag.header.queue_id, 2);
    assert_eq!(frag.header.recipient_id, SELECTION_DESTINATION);
    assert_eq!(frag.header.blob_flags & 0x40, 0, "explicit recipient");
    assert_eq!(frag.header.ext_word, None);
    let mut want = vec![0x00];
    want.extend_from_slice(&0x0100_0000_0000_0042u64.to_le_bytes());
    want.push(0);
    assert_eq!(frag.payload.len(), want.len());
    assert_eq!(frag.payload[9], 0, "the empty table");
}

/// §9.3.8: the four waits, one after the other: each ends only at its next stage, other traffic
/// resets none of them, and each has its own locally worded end.
#[test]
fn each_wait_on_the_road_has_its_own_named_end() {
    // Referral.
    let (world, mut d, mut login) = driver_at_characters(no_readiness(), 0);
    d.command(&ENTER(), 1.0).expect("enter");
    for t in [5.0, 15.0, 25.0] {
        world.deliver(login.event(0x30, &[0x00, 0x7F]), LOGIN_SERVER);
        d.step(t, true, &go);
        assert!(!d.ended());
    }
    assert_eq!(d.skip_counters().count(0x30), 3, "counted, never held");
    d.step(1.0 + ENTER_WORLD_TIMEOUT_SEC, false, &go);
    let events = d.take_events();
    assert!(
        timed_out_with(&events, EntryWait::Referral.deadline_text()),
        "{events:?}"
    );

    // World State 5.
    let (world, mut d, login) = driver_at_characters(no_readiness(), 0);
    d.command(&ENTER(), 1.0).expect("enter");
    world.deliver(
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
        LOGIN_SERVER,
    );
    d.step(10.0, true, &go);
    let _ = d.take_events();
    d.step(29.0, false, &go);
    assert!(!d.ended());
    // A world server that never answers the handshake is ordinary silence (§6.1's 20 s, shorter
    // than this stage's wait): no Account event, no entry refusal.
    d.step(30.5, false, &go);
    d.step(30.6, false, &go);
    let events = d.take_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Disconnected {
                reason: DisconnectReason::TimedOut
            }
        )),
        "{events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::Account(_) | Event::EnterWorldFailed { .. }))
    );

    // Complete `0x90`: traffic on the world connection does not reset it.
    let (world, mut d, _login, mut server) = at_session_start(1.0);
    world.deliver(server.event(0x149, &[0x00, 0x7F]), WORLD_SERVER);
    d.step(20.0, true, &go);
    assert!(!d.ended());
    d.step(1.3 + ENTER_WORLD_TIMEOUT_SEC, false, &go);
    let events = d.take_events();
    assert!(
        timed_out_with(&events, EntryWait::SessionStart.deadline_text()),
        "{events:?}"
    );
}

/// §9.3.8: `0x90` alone is not `WorldEntered`. The checked messages are read with their one shared
/// byte and counted, nothing is published, no readiness or `0xF4` is made up, and the entry ends
/// at the readiness wait with Tessera's words.
#[test]
fn the_session_start_is_not_world_entry_and_the_stage_is_named() {
    let (world, mut d, _login, mut server) = at_session_start(1.0);
    let ready = PlayerSessionReady {
        player_id: 0x0100_0000_0000_0007,
        session_props: EncodedPropertyTable::new(),
    }
    .to_bytes()
    .expect("encodes");
    world.deliver(server.event(0x90, &ready), WORLD_SERVER);
    d.step(2.0, true, &go);
    assert!(!d.in_world());
    let batch = SubEventBatch {
        event_id: SubEventBatch::EVENT_ID_SUB_EVENT,
        frames: vec![SubEventFrame::Event {
            sub_event_id: 0x98,
            payload: vec![0x00, 1, 2],
        }],
        entry_batch_tail_flag: None,
    }
    .to_bytes()
    .expect("encodes");
    world.deliver(server.event(0x4C, &batch), WORLD_SERVER);
    // The historical two-byte form of `0x4C` does not read as a batch.
    let mut shifted = vec![0x00];
    shifted.extend_from_slice(&batch);
    world.deliver(server.blob(0x4C, &shifted), WORLD_SERVER);
    world.deliver(server.event(0x7C, &[0x00]), WORLD_SERVER);
    world.deliver(server.event(0x0D, &[0x00]), WORLD_SERVER);
    world.deliver(server.event(0xCC, &[0x00, 0x00]), WORLD_SERVER);
    d.step(3.0, true, &go);
    let progress = d.entry_progress();
    assert!(progress.referred && progress.world_connected && progress.session_started);
    assert_eq!(progress.batches, 1);
    assert_eq!(progress.creations, 1);
    assert_eq!(progress.unread, 1, "the shifted batch");
    assert_eq!(progress.readiness_seen, 1, "seen, not acted on");
    assert_eq!(progress.unsupported_batches, 1);
    assert_eq!(progress.placements, 1);
    let events = d.take_events();
    assert!(events.is_empty(), "nothing is published: {events:?}");
    let sent = opcodes(&sent_packets(&world));
    assert!(!sent.contains(&0xF4), "{sent:x?}");
    assert!(!sent.contains(&0x30), "{sent:x?}");
    // The client's loading signal changes nothing either.
    let _ = d.command(&Command::WorldLoaded, 4.0);
    d.step(4.1, true, &go);
    assert!(!opcodes(&sent_packets(&world)).contains(&0xF4));
    // No chat connection is due.
    assert_eq!(d.wants_chat(5.0), None);
    d.step(32.0, false, &go);
    let events = d.take_events();
    assert!(
        timed_out_with(&events, EntryWait::Readiness.deadline_text()),
        "{events:?}"
    );
    assert!(
        EntryWait::Readiness
            .deadline_text()
            .contains("Tessera limit")
    );
    assert!(d.ended());
    let sent = sent_packets(&world);
    assert_eq!(
        sent.iter().filter(|p| p.opt_headers.disconnect).count(),
        1,
        "one Disconnect"
    );
    assert!(!opcodes(&sent).contains(&0xF4));
}

/// §9.3.8's rows after `Connected`: a closing header gets no reply and no Account event, `0x6D`
/// at the idle list is counted, and the malformed rows end with Tessera's text.
#[test]
fn the_rows_after_connected_have_no_account_event() {
    let closing = |opt: OptionalHeaders| {
        let (world, mut d, login) = driver_at_characters(no_readiness(), 0);
        world.take_sent();
        world.deliver(login.control(opt), LOGIN_SERVER);
        d.step(1.0, true, &go);
        (world, d)
    };
    for opt in [
        OptionalHeaders {
            disconnect: true,
            ..OptionalHeaders::default()
        },
        OptionalHeaders {
            transport_error: Some(TransportErrorHeader {
                error_code: 0,
                error_subcode: 0,
            }),
            ..OptionalHeaders::default()
        },
        OptionalHeaders {
            transport_error: Some(TransportErrorHeader {
                error_code: 0x00A7_E948,
                error_subcode: 0,
            }),
            ..OptionalHeaders::default()
        },
        OptionalHeaders {
            transport_error: Some(TransportErrorHeader {
                error_code: 0x1234_5678,
                error_subcode: 9,
            }),
            ..OptionalHeaders::default()
        },
        OptionalHeaders {
            transport_fatal_error: Some(TransportErrorHeader {
                error_code: 7,
                error_subcode: 0,
            }),
            ..OptionalHeaders::default()
        },
    ] {
        let (world, mut d) = closing(opt);
        let events = d.take_events();
        assert!(
            matches!(
                events.as_slice(),
                [Event::Disconnected { reason: DisconnectReason::ServerClosed(text) }] if text.is_empty()
            ),
            "{events:?}"
        );
        assert!(
            sent_packets(&world)
                .iter()
                .all(|p| !p.opt_headers.disconnect),
            "no reply"
        );
    }

    // Valid `0x6D` at the idle list: counted, no event.
    let (world, mut d, mut login) = driver_at_characters(no_readiness(), 0);
    world.deliver(login.event(0x6D, &[0x00, 4, 3, 2, 1]), LOGIN_SERVER);
    d.step(1.0, true, &go);
    assert!(d.take_events().is_empty());
    assert_eq!(d.counters().idle_refusals, 1);
    assert!(!d.ended());
    // Valid `0xB3`: `ServerClosed`, its text not dispatched.
    world.deliver(login.event(0xB3, &[0x00, 4, 3, 2, 1, 0xFF]), LOGIN_SERVER);
    d.step(1.1, true, &go);
    let events = d.take_events();
    assert!(
        matches!(
            events.as_slice(),
            [Event::Disconnected { reason: DisconnectReason::ServerClosed(text) }] if text.is_empty()
        ),
        "{events:?}"
    );

    // Malformed `0x6D`: Tessera's text.
    let (world, mut d, mut login) = driver_at_characters(no_readiness(), 0);
    world.deliver(login.event(0x6D, &[0x00, 4, 3]), LOGIN_SERVER);
    d.step(1.0, true, &go);
    let events = d.take_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Disconnected { reason: DisconnectReason::TransportFailed(text) }
                if text == UNSUPPORTED_ENTRY_ANSWER
        )),
        "{events:?}"
    );

    // Valid `0x6D` while selecting: back to the list, and the character can be chosen again.
    let (world, mut d, mut login) = driver_at_characters(no_readiness(), 0);
    d.command(&ENTER(), 1.0).expect("enter");
    d.step(1.1, true, &go);
    world.deliver(login.event(0x6D, &[0x00, 4, 3, 2, 1]), LOGIN_SERVER);
    d.step(1.2, true, &go);
    let events = d.take_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::EnterWorldFailed { character, reason } if character == "Aldo" && reason == ENTRY_REFUSED
        )),
        "{events:?}"
    );
    assert!(!d.ended());
    d.command(&ENTER(), 2.0).expect("again");
}

/// §9.3.8: after `Connected` a stale `0xDD` or `0xF5`, malformed or not, is counted and cannot close
/// a healthy list; so is a `0x12F`.
#[test]
fn stale_negotiation_messages_cannot_close_a_healthy_list() {
    let (world, mut d, mut login) = driver_at_characters(no_readiness(), 0);
    let before = d.initial_stages().ignored;
    for (op, data) in [
        (0xDDu32, vec![0x00, 0xFF]),
        (0xF5, vec![0x01]),
        (0xF5, vec![0x00]),
        (0xDD, vec![0x00; 13]),
    ] {
        world.deliver(login.blob(op, &data), LOGIN_SERVER);
    }
    world.deliver(login.event(0x12F, &[]), LOGIN_SERVER);
    world.take_sent();
    d.step(1.0, true, &go);
    assert!(!d.ended());
    assert_eq!(d.initial_stages().ignored, before + 3);
    assert!(opcodes(&sent_packets(&world)).is_empty());
}

/// §9.3.8: a chat request reports local unavailability and the primary session goes on: no chat
/// handshake, no ticket to a chat server.
#[test]
fn chat_requests_fail_locally_and_keep_the_primary_session() {
    let (_world, mut d, _login) = driver_at_characters(
        Service {
            no_chat_codec: true,
            ..no_readiness()
        },
        0,
    );
    let refused = d.command(
        &Command::Social(tessera_net::SocialCommand::JoinChannel { name: "x".into() }),
        1.0,
    );
    assert!(matches!(
        refused,
        Err(tessera_net::NetError::InvalidCommand(_))
    ));
    assert!(!d.ended());
    assert_eq!(d.wants_chat(2.0), None);
}

/// §9.3.8: the withheld commands fail locally as `InvalidCommand` with no packet.
#[test]
fn withheld_character_commands_fail_locally_with_no_packet() {
    let (world, mut d, _login) = driver_at_characters(no_readiness(), 0);
    world.take_sent();
    let delete = Command::Character(tessera_net::CharacterCommand::DeleteCharacter {
        name: "Aldo".to_owned(),
    });
    assert!(matches!(
        d.command(&delete, 1.0),
        Err(tessera_net::NetError::InvalidCommand(_))
    ));
    d.step(1.1, true, &go);
    assert!(opcodes(&sent_packets(&world)).is_empty());
    assert!(!d.ended());
}

/// A masked packet with one hand-made fragment header: its checksum is right when `good`.
fn masked_raw(server: &mut Server, header_bytes: &[u8], data: &[u8], good: bool) -> Vec<u8> {
    server.seq += 1;
    let mut payload = header_bytes.to_vec();
    payload.extend_from_slice(data);
    let mut hdr = crate::session::DatagramHeader {
        rec_id: server.rec_id,
        flags: FLAG_HAS_FRAGMENTED_BLOBS | FLAG_MASKED_CHECKSUM,
        seq_num: server.seq,
        pak_size: u16::try_from(payload.len()).expect("small"),
        ..crate::session::DatagramHeader::default()
    };
    let mask = server.isaac.next_mask();
    let sum = compute_datagram_checksum(header_bytes).wrapping_add(compute_datagram_checksum(data));
    hdr.checksum = combine_packet_checksum(hdr, sum, Some(mask)).expect("sum");
    if !good {
        hdr.checksum ^= 1;
    }
    let mut dgram = vec![0u8, 0];
    dgram.extend_from_slice(&hdr.to_wire_bytes());
    dgram.extend_from_slice(&payload);
    dgram
}

/// §9.3.8's transport table: bit `0x02` on queues 2–4 with a valid checksum is one terminal
/// unsupported result without an Ack; a bad checksum or an undefined bit is only dropped and
/// counted, and cannot close anything.
#[test]
fn the_transport_forms_follow_their_table() {
    // queue 2, flags 0x42, order 1, blob 1, opcode, length, data.
    let header = [0x02u8, 0x42, 1, 0, 1, 0, 0x10, 0x01];
    let (world, mut d, mut login) = driver_at_characters(no_readiness(), 0);
    world.take_sent();
    world.deliver(
        masked_raw(&mut login, &header, &[0x00], false),
        LOGIN_SERVER,
    );
    d.step(1.0, true, &go);
    assert!(!d.ended(), "unverifiable: dropped");
    assert_eq!(d.counters().refused_datagrams, 1);

    // Undefined bit 0x04: its framing is unproven; dropped, whatever its checksum says.
    let undefined = [0x02u8, 0x44, 1, 0, 1, 0, 0x10, 0x01];
    let (world, mut d, mut login) = driver_at_characters(no_readiness(), 0);
    world.take_sent();
    world.deliver(
        masked_raw(&mut login, &undefined, &[0x00], true),
        LOGIN_SERVER,
    );
    d.step(1.0, true, &go);
    assert!(!d.ended());

    // Valid checksum.
    let (world, mut d, mut login) = driver_at_characters(no_readiness(), 0);
    world.take_sent();
    world.deliver(masked_raw(&mut login, &header, &[0x00], true), LOGIN_SERVER);
    d.step(1.0, true, &go);
    assert!(d.ended());
    let events = d.take_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Disconnected { reason: DisconnectReason::TransportFailed(text) } if !text.is_empty()
        )),
        "{events:?}"
    );
    let sent = sent_packets(&world);
    assert_eq!(sent.iter().filter(|p| p.opt_headers.disconnect).count(), 1);
    assert!(
        sent.iter().all(|p| p.opt_headers.cumulative_ack.is_none()),
        "no Ack"
    );
    let _ = LinkEnd::UnsupportedTraffic;
}

/// §9.3.8's identities: an exact retransmission is ignored, a conflicting body of a remembered
/// identity terminates after the checksum held, and past 30 s the identity is fresh again.
#[test]
fn a_conflicting_identity_terminates_and_an_exact_repeat_is_ignored() {
    let send = |server: &mut Server, blob: u16, data: &[u8]| {
        server.seq += 1;
        let frags = split_blob(2, blob, blob, 0, None, 0x149, data).expect("split");
        let mask = server.isaac.next_mask();
        encode_datagram(
            crate::session::DatagramHeader {
                rec_id: server.rec_id,
                seq_num: server.seq,
                ..crate::session::DatagramHeader::default()
            },
            &OptionalHeaders::default(),
            &frags,
            Some(mask),
            None,
        )
        .expect("datagram")
    };
    let (world, mut d, mut login) = driver_at_characters(no_readiness(), 0);
    world.deliver(send(&mut login, 500, &[0x00, 1]), LOGIN_SERVER);
    world.deliver(send(&mut login, 500, &[0x00, 1]), LOGIN_SERVER);
    d.step(1.0, true, &go);
    assert_eq!(d.counters().reassembly.duplicates, 1);
    assert!(!d.ended());
    // Past 30 s it is fresh, with another body.
    world.deliver(send(&mut login, 500, &[0x00, 2]), LOGIN_SERVER);
    d.step(32.0, true, &go);
    assert!(!d.ended());
    // Within 30 s another body conflicts.
    world.deliver(send(&mut login, 500, &[0x00, 3]), LOGIN_SERVER);
    d.step(33.0, true, &go);
    assert!(d.ended());
}

/// §9.3.8 and Vector 11's transitions at driver level: the rules of `0xDD` and `0xF5` while the
/// initial stage is active.
mod initial_transitions {
    use super::super::initial_negotiation::{RULE, connected, install_request, queue1};
    use super::*;
    use crate::initial::UnsupportedNegotiation as U;

    fn at_advertisement() -> (World, SessionDriver<FakeNet>, Server) {
        let (world, mut d, mut login) = connected();
        world.deliver(
            queue1(&mut login, 0xDD, &install_request(RULE, &[])),
            LOGIN_SERVER,
        );
        d.step(0.1, true, &go);
        (world, d, login)
    }

    fn refused_with(d: &mut SessionDriver<FakeNet>, why: U) -> bool {
        d.take_events().iter().any(|e| {
            matches!(
                e,
                Event::Account(AccountEvent::LoginRefused {
                    reason: LoginRefusal::UnsupportedStep,
                    note: Some(note),
                    ..
                }) if note == why.detail()
            )
        })
    }

    /// `00` in the window gets one constructed `00` with an explicit recipient 2; a repeat, with
    /// any body, gets a count and no reply; before that reply `01` or an empty body fails.
    #[test]
    fn completion_bodies() {
        let (world, mut d, mut login) = at_advertisement();
        // The `0xEC` went out with the recipient in its explicit field.
        let sent = sent_packets(&world);
        let ec = sent
            .iter()
            .flat_map(|p| &p.fragments)
            .find(|f| f.header.blob_header_word == Some(0xEC))
            .expect("the advertisement");
        assert_eq!(ec.header.blob_flags & 0x40, 0);
        assert_eq!(ec.header.recipient_id, 2);
        world.deliver(queue1(&mut login, 0xF5, &[0]), LOGIN_SERVER);
        d.step(0.2, true, &go);
        let sent = sent_packets(&world);
        let reply = sent
            .iter()
            .flat_map(|p| &p.fragments)
            .find(|f| f.header.blob_header_word == Some(0xF5))
            .expect("the reply");
        assert_eq!(reply.payload, [0]);
        assert_eq!(
            reply.header.blob_flags, 0,
            "explicit, queue-1, no extension"
        );
        assert_eq!(reply.header.recipient_id, 2);
        let before = d.initial_stages().ignored;
        for body in [vec![0u8], vec![1], vec![0, 9]] {
            world.deliver(queue1(&mut login, 0xF5, &body), LOGIN_SERVER);
        }
        d.step(0.3, true, &go);
        assert!(opcodes(&sent_packets(&world)).is_empty());
        assert!(!d.ended());
        assert_eq!(d.initial_stages().ignored, before + 3);

        for body in [vec![0x01u8], vec![]] {
            let (world, mut d, mut login) = at_advertisement();
            world.take_sent();
            world.deliver(queue1(&mut login, 0xF5, &body), LOGIN_SERVER);
            d.step(0.2, true, &go);
            assert!(refused_with(&mut d, U::UnexpectedCompletion), "{body:x?}");
        }
    }

    /// A later well-formed `0xDD`, its array non-empty too, only updates the word and is counted;
    /// a malformed one before the stage ends fails; a triggering one with an array fails.
    #[test]
    fn request_scope() {
        let (world, mut d, mut login) = at_advertisement();
        world.take_sent();
        world.deliver(
            queue1(&mut login, 0xDD, &install_request(RULE, &[9, 9])),
            LOGIN_SERVER,
        );
        d.step(0.2, true, &go);
        assert!(!d.ended());
        assert!(opcodes(&sent_packets(&world)).is_empty());
        assert_eq!(d.initial_stages().ignored, 1);

        let (world, mut d, mut login) = at_advertisement();
        world.deliver(queue1(&mut login, 0xDD, &[0x00, 1, 2]), LOGIN_SERVER);
        d.step(0.2, true, &go);
        assert!(refused_with(&mut d, U::MalformedRequest));

        let (world, mut d, mut login) = connected();
        world.deliver(
            queue1(&mut login, 0xDD, &install_request(RULE, &[1])),
            LOGIN_SERVER,
        );
        d.step(0.1, true, &go);
        assert!(refused_with(&mut d, U::RequestArray));

        // On another queue: counted, nothing changes.
        let (world, mut d, mut login) = connected();
        world.take_sent();
        world.deliver(login.blob(0xDD, &install_request(RULE, &[])), LOGIN_SERVER);
        d.step(0.1, true, &go);
        assert!(!d.ended());
        assert_eq!(d.initial_stages().ignored, 1);
        assert!(!d.initial_stages().ec_sent);
    }

    /// The historical path: a `0x12F` before `0xEC` suppresses it, one after it is unsupported, and
    /// the refusals of `0x12D` and `0x130` have their rows.
    #[test]
    fn historical_rows() {
        let (world, mut d, mut login) = at_advertisement();
        world.deliver(login.event(0x12F, &[]), LOGIN_SERVER);
        d.step(0.2, true, &go);
        assert!(refused_with(&mut d, U::LateVersionRequest));

        let (world, mut d, mut login) = connected();
        world.deliver(login.event(0x12F, &[]), LOGIN_SERVER);
        world.deliver(login.event(0x12F, &[]), LOGIN_SERVER);
        world.deliver(
            queue1(&mut login, 0xDD, &install_request(RULE, &[])),
            LOGIN_SERVER,
        );
        d.step(0.1, true, &go);
        let sent = opcodes(&sent_packets(&world));
        assert_eq!(sent, [0x12C], "one `0x12C` and no `0xEC`");
        assert!(!d.ended());

        let (world, mut d, mut login) = connected();
        let status = crate::session::VersionStatusMessage {
            status: 0,
            server_version_info: "no".to_owned(),
        }
        .encode()
        .expect("encodes");
        world.deliver(
            queue1(
                &mut login,
                0x12D,
                &crate::world::encode_event_envelope(&status),
            ),
            LOGIN_SERVER,
        );
        d.step(0.1, true, &go);
        let events = d.take_events();
        assert!(
            events.iter().any(|e| matches!(
                e,
                Event::Account(AccountEvent::LoginRefused {
                    reason: LoginRefusal::ClientNotAccepted,
                    ..
                })
            )),
            "{events:?}"
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                Event::Disconnected {
                    reason: DisconnectReason::ServerClosed(_)
                }
            )),
            "{events:?}"
        );
    }
}
