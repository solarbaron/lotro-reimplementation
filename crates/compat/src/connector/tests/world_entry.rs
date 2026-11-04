// SPDX-License-Identifier: GPL-3.0-or-later
//! The road into the world and the phase table of `official-session.md` §9.3.8, at driver level
//! on a virtual clock, on a network with no source for readiness, like every production path.

use super::*;
use crate::connector::data::{InstallData, SchemaView};
use crate::connector::session_phase::{
    ENTER_WORLD_TIMEOUT_SEC, ENTRY_REFUSED, SELECTION_DESTINATION,
};
use crate::initial::{AdvertisedArchive, InstallUnsupported};
use crate::session::{
    DisconnectReason as LinkEnd, FLAG_HAS_FRAGMENTED_BLOBS, FLAG_MASKED_CHECKSUM,
    TransportErrorHeader, combine_packet_checksum, compute_datagram_checksum,
};
use crate::world::{
    CompactDid, DEFAULT_PORTAL_STATE_SCHEMA_ID, EnterPortalSpace, EntityCreate, PortalPositionSync,
    PortalStateSchema, PropertyScope, PropertyTableUpdate, PropertyType, SubEventBatch,
    SubEventFrame, WirePosition,
};
use std::collections::BTreeMap;

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

fn timed_out_locally(events: &[Event]) -> bool {
    events.iter().any(|e| {
        matches!(
            e,
            Event::Disconnected {
                reason: DisconnectReason::UnsupportedTimeout
            }
        )
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
    assert!(timed_out_locally(&events), "{events:?}");

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
    assert!(timed_out_locally(&events), "{events:?}");
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
            payload: checked_message(0x98),
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
    // Without its schema and to exact exhaustion, `0xCC` does not count as a placement.
    world.deliver(server.event(0xCC, &[0x00, 0x00]), WORLD_SERVER);
    d.step(3.0, true, &go);
    let progress = d.entry_progress();
    assert!(progress.referred && progress.world_connected && progress.session_started);
    assert_eq!(progress.batches, 1);
    assert_eq!(progress.creations, 1);
    assert_eq!(progress.unread, 2, "the shifted batch and the short `0xCC`");
    assert_eq!(progress.readiness_seen, 1, "seen, not acted on");
    assert_eq!(progress.unsupported_batches, 1);
    assert_eq!(progress.placements, 0);
    let events = d.take_events();
    assert!(
        events
            .iter()
            .all(|e| matches!(e, Event::WorldEntryProgress { .. })),
        "nothing is published: {events:?}"
    );
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
    assert!(timed_out_locally(&events), "{events:?}");
    assert!(d.ended());
    let sent = sent_packets(&world);
    assert_eq!(
        sent.iter().filter(|p| p.opt_headers.disconnect).count(),
        1,
        "one Disconnect"
    );
    assert!(!opcodes(&sent).contains(&0xF4));
}

/// A valid complete message of each checked opcode, with its one shared byte.
pub(super) fn checked_message(opcode: u32) -> Vec<u8> {
    match opcode {
        0x98 => EntityCreate {
            template_did: 0x7000_0123,
            entity_id: 0x0300_0000_0000_0101,
        }
        .to_bytes()
        .expect("0x98"),
        0x52 => {
            let mut w = crate::world::WireWriter::new();
            PropertyTableUpdate {
                scope: PropertyScope::Entity,
                properties: EncodedPropertyTable::new(),
            }
            .encode(&mut w)
            .expect("0x52");
            w.into_vec()
        }
        0xCE => service_support::position(0x0300_0000_0000_0101, [1.0, 2.0, 3.0], 0, [0.0; 3]),
        0xCC => PortalPositionSync {
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
            linear_velocity: [None, None, None],
            angular_velocity: [None, None, None],
            anim_or_context_did: CompactDid(0x0900_0001),
            state_schema_id: DEFAULT_PORTAL_STATE_SCHEMA_ID,
            bool_states: Vec::new(),
            enum_states: Vec::new(),
            extra_blob: Vec::new(),
        }
        .to_bytes(&BTreeMap::from([(
            DEFAULT_PORTAL_STATE_SCHEMA_ID,
            PortalStateSchema::default(),
        )]))
        .expect("0xCC"),
        0x5E => EnterPortalSpace {
            stamp: 1,
            scene_id: 2,
        }
        .to_bytes()
        .expect("0x5E"),
        0x89 => service_support::observed_appearance().to_vec(),
        _ => unreachable!("a checked opcode"),
    }
}

/// The synthetic install with one `0xCC` state schema and the service's dictionary.
#[derive(Debug)]
struct WithSchema(Records, PortalStateSchema);

impl InstallData for WithSchema {
    fn record(&self, did: u32, max_bytes: usize) -> Option<Vec<u8>> {
        self.0.record(did, max_bytes)
    }
    fn property_type(&self, pid: u32) -> Option<PropertyType> {
        service_support::DICTIONARY
            .iter()
            .find(|(p, _)| *p == pid)
            .map(|(_, t)| *t)
    }
    fn portal_state_schema(&self, id: u32) -> Option<&PortalStateSchema> {
        (id == DEFAULT_PORTAL_STATE_SCHEMA_ID).then_some(&self.1)
    }
    fn selected_language_key(&self) -> Result<u32, InstallUnsupported> {
        self.0.selected_language_key()
    }
    fn check_name_table(&self) -> Result<(), InstallUnsupported> {
        self.0.check_name_table()
    }
    fn name_rule_languages(&self, rule_key: u32) -> Result<Vec<u32>, InstallUnsupported> {
        self.0.name_rule_languages(rule_key)
    }
    fn archive_inventory(&self) -> Result<Vec<AdvertisedArchive>, InstallUnsupported> {
        self.0.archive_inventory()
    }
}

fn with_schema() -> SessionStart {
    let mut start = session_start(0, None, Arc::default());
    start.schema = SchemaView(Arc::new(WithSchema(
        records(),
        PortalStateSchema::default(),
    )));
    start
}

/// [`at_session_start`] on an install with the service's dictionary and a `0xCC` schema.
pub(super) fn at_session_start_with_schema() -> (World, SessionDriver<FakeNet>, Server) {
    let (world, mut d, login) = driver_at_characters_from(no_readiness(), with_schema());
    d.command(&ENTER(), 1.0).expect("enter");
    d.step(1.1, true, &go);
    world.deliver(
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
        LOGIN_SERVER,
    );
    d.step(1.2, true, &go);
    let server = Server::new(0x0222, 0xBEEF_0002);
    server.connect_to(&world, 0xBEEF_0002, WORLD_SERVER);
    d.step(1.3, true, &go);
    (world, d, server)
}

/// §9.3.9: each checked message is kept only when it decodes to exact exhaustion, top level and
/// embedded in a `0x4C`, with its one shared byte consumed once; a second byte does not read. Its
/// target is the entity a `0x98` announced.
#[test]
fn checked_messages_count_only_when_they_decode() {
    const TARGET: u64 = 0x0300_0000_0000_0101;
    type Counted = fn(&crate::connector::session_phase::EntryProgress) -> u64;
    let cases: [(u32, Counted); 4] = [
        (0x52, |p| p.creations),
        (0xCE, |p| p.positions),
        (0xCC, |p| p.placements),
        (0x89, |p| p.appearances),
    ];
    let embed = |opcode: u32, payload: Vec<u8>| {
        SubEventBatch {
            event_id: SubEventBatch::EVENT_ID_SUB_EVENT,
            frames: vec![SubEventFrame::Event {
                sub_event_id: opcode,
                payload,
            }],
            entry_batch_tail_flag: None,
        }
        .to_bytes()
        .expect("batch")
    };
    let doubled = |data: &[u8]| {
        let mut out = vec![0x00];
        out.extend_from_slice(data);
        out
    };
    for (opcode, counted) in cases {
        let (world, mut d, mut server) = at_session_start_with_schema();
        let mut word = 100u16;
        let mut to = |server: &mut Server, opcode: u32, data: &[u8]| {
            word += 1;
            server.on_queue(1, word, 0, Some(TARGET), opcode, data)
        };
        world.deliver(to(&mut server, 0x98, &checked_message(0x98)), WORLD_SERVER);
        d.step(1.9, true, &go);
        let before = counted(&d.entry_progress());
        let base = before;
        let data = checked_message(opcode);
        world.deliver(to(&mut server, opcode, &data), WORLD_SERVER);
        d.step(2.0, true, &go);
        let p = d.entry_progress();
        assert_eq!(
            (counted(&p) - base, p.unread),
            (1, 0),
            "top level {opcode:#x}"
        );

        world.deliver(to(&mut server, opcode, &doubled(&data)), WORLD_SERVER);
        d.step(2.1, true, &go);
        let p = d.entry_progress();
        assert_eq!(
            (counted(&p) - base, p.unread),
            (1, 1),
            "doubled {opcode:#x}"
        );

        world.deliver(
            to(&mut server, 0x4C, &embed(opcode, data.clone())),
            WORLD_SERVER,
        );
        d.step(2.2, true, &go);
        let p = d.entry_progress();
        assert_eq!(
            (counted(&p) - base, p.unread),
            (2, 1),
            "embedded {opcode:#x}"
        );

        world.deliver(
            to(&mut server, 0x4C, &embed(opcode, doubled(&data))),
            WORLD_SERVER,
        );
        d.step(2.3, true, &go);
        let p = d.entry_progress();
        assert_eq!(
            (counted(&p) - base, p.unread, p.batches, p.sub_events),
            (2, 2, 2, 2),
            "embedded doubled {opcode:#x}"
        );

        let cut = &data[..data.len() - 1];
        world.deliver(to(&mut server, opcode, cut), WORLD_SERVER);
        d.step(2.4, true, &go);
        let p = d.entry_progress();
        assert_eq!((counted(&p) - base, p.unread), (2, 3), "cut {opcode:#x}");
        assert!(
            d.take_events()
                .iter()
                .all(|e| matches!(e, Event::WorldEntryProgress { .. })),
            "nothing is published"
        );
        assert!(!d.ended());
    }
}

/// Session §5.6.4 rule 3, §9.3.9: the recipient is part of the identity on every queue. Two blobs
/// with the same word and blob ID to two entities are two fresh identities, both delivered. Another
/// body for the same recipient is delivered once with an anomaly count, and nothing ends.
#[test]
fn another_recipient_is_another_identity() {
    const A: u64 = 0x0300_0000_0000_0101;
    const B: u64 = 0x0300_0000_0000_0102;
    for queue in [2u8, 3, 4] {
        let (world, mut d, _login, mut server) = at_session_start(1.0);
        let body = |t: u8| [0x00, t];
        world.deliver(
            server.on_queue(queue, 77, 9, Some(A), 0x149, &body(1)),
            WORLD_SERVER,
        );
        world.deliver(
            server.on_queue(queue, 77, 9, Some(B), 0x149, &body(2)),
            WORLD_SERVER,
        );
        d.step(2.0, true, &go);
        assert!(!d.ended(), "queue {queue}");
        assert_eq!(d.entry_progress().other, 2, "both delivered, queue {queue}");
        let counts = d.counters().reassembly;
        assert_eq!(counts.duplicates, 0);
        assert_eq!(
            counts.queue3_equal, 0,
            "each recipient has its own baseline"
        );
        // An exact repeat for B is ignored.
        world.deliver(
            server.on_queue(queue, 77, 9, Some(B), 0x149, &body(2)),
            WORLD_SERVER,
        );
        d.step(2.1, true, &go);
        assert_eq!(d.counters().reassembly.duplicates, 1, "queue {queue}");
        assert!(!d.ended());
        // Another complete body for A: delivered once, an anomaly.
        world.deliver(
            server.on_queue(queue, 77, 9, Some(A), 0x149, &body(3)),
            WORLD_SERVER,
        );
        d.step(2.2, true, &go);
        assert!(!d.ended(), "queue {queue}");
        assert_eq!(d.entry_progress().other, 3, "delivered, queue {queue}");
        let counts = d.counters().reassembly;
        assert_eq!(counts.reused_identities, 1, "queue {queue}");
        if queue == 3 {
            assert_eq!(counts.queue3_equal, 1, "the equal word of a different body");
        }
        // The same body again is the exact repeat of what was delivered.
        world.deliver(
            server.on_queue(queue, 77, 9, Some(A), 0x149, &body(3)),
            WORLD_SERVER,
        );
        d.step(2.3, true, &go);
        assert_eq!(d.entry_progress().other, 3, "queue {queue}");
        assert_eq!(d.counters().reassembly.duplicates, 2, "queue {queue}");
    }
}

/// An unmasked packet's checksum has no key: its fragment on queues 2–4 is dropped and leaves no
/// identity behind, so the masked blob after it is no reuse (engineering choice, session.rs).
#[test]
fn an_unmasked_fragment_leaves_no_identity_behind() {
    let (world, mut d, _login, mut server) = at_session_start(1.0);
    let frags = split_blob(2, 500, 500, 0, None, 0x149, &[0x00, 1]).expect("split");
    let unmasked = encode_datagram(
        crate::session::DatagramHeader {
            rec_id: server.rec_id,
            seq_num: server.seq,
            ..crate::session::DatagramHeader::default()
        },
        &OptionalHeaders::default(),
        &frags,
        None,
        None,
    )
    .expect("datagram");
    world.deliver(unmasked, WORLD_SERVER);
    d.step(2.0, true, &go);
    assert!(!d.ended());
    assert_eq!(d.entry_progress().other, 0, "dropped");
    world.deliver(
        server.on_queue(2, 500, 500, None, 0x149, &[0x00, 2]),
        WORLD_SERVER,
    );
    d.step(2.1, true, &go);
    assert!(!d.ended());
    assert_eq!(d.entry_progress().other, 1);
}

/// §9.3.8: the wait from `0x6F` is stopped only by a valid referral or a terminal or refusal result,
/// and the `0x90` wait starts at the world connection's State 5. A `0x90` on the login connection
/// is counted and skipped: no stage moves, no clock restarts, and a malformed one ends nothing.
#[test]
fn a_session_start_on_the_login_connection_stops_no_wait() {
    let ready = PlayerSessionReady {
        player_id: 0x0100_0000_0000_0007,
        session_props: EncodedPropertyTable::new(),
    }
    .to_bytes()
    .expect("encodes");

    // The referral wait still ends on its own clock, with its own text.
    let (world, mut d, mut login) = driver_at_characters(no_readiness(), 0);
    d.command(&ENTER(), 1.0).expect("enter");
    d.step(1.1, true, &go);
    world.deliver(login.event(0x90, &ready), LOGIN_SERVER);
    world.deliver(login.event(0x90, &[0x00, 1, 2]), LOGIN_SERVER);
    d.step(29.0, true, &go);
    assert!(!d.ended(), "a malformed one is only counted");
    assert_eq!(d.skip_counters().count(0x90), 2);
    assert!(!d.entry_progress().session_started);
    d.step(1.0 + ENTER_WORLD_TIMEOUT_SEC, false, &go);
    assert!(timed_out_locally(&d.take_events()));

    // A real referral after it still moves the wait, and only the world's `0x90` starts the last.
    let (world, mut d, mut login) = driver_at_characters(no_readiness(), 0);
    d.command(&ENTER(), 1.0).expect("enter");
    d.step(1.1, true, &go);
    world.deliver(login.event(0x90, &ready), LOGIN_SERVER);
    d.step(20.0, true, &go);
    world.deliver(
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
        LOGIN_SERVER,
    );
    d.step(21.0, true, &go);
    assert!(d.entry_progress().referred);
    let mut server = Server::new(0x0222, 0xBEEF_0002);
    server.connect_to(&world, 0xBEEF_0002, WORLD_SERVER);
    d.step(22.0, true, &go);
    assert!(d.entry_progress().world_connected);
    assert!(!d.entry_progress().session_started);
    world.deliver(server.event(0x90, &ready), WORLD_SERVER);
    d.step(23.0, true, &go);
    assert!(d.entry_progress().session_started);
    // A second `0x90`, malformed or not, moves nothing: the readiness clock runs from the first.
    world.deliver(server.event(0x90, &[0x00, 1, 2]), WORLD_SERVER);
    world.deliver(server.event(0x90, &ready), WORLD_SERVER);
    d.step(30.0, true, &go);
    assert!(!d.ended());
    assert_eq!(
        d.entry_progress().other,
        2,
        "counted with the other traffic"
    );
    d.step(23.0 + ENTER_WORLD_TIMEOUT_SEC, false, &go);
    assert!(timed_out_locally(&d.take_events()));
}

/// §9.3.8: a local terminal close sends at most one standalone Disconnect, so what the same pass
/// queued stays unsent, here the `0xEC` that a `0xDD` asked for just before a `0xB3`.
#[test]
fn nothing_queued_goes_out_after_a_terminal_decision() {
    use super::initial_negotiation::{RULE, connected, install_request, queue1};
    let (world, mut d, mut login) = connected();
    world.deliver(
        queue1(&mut login, 0xDD, &install_request(RULE, &[])),
        LOGIN_SERVER,
    );
    world.deliver(login.event(0xB3, &[0x00, 4, 3, 2, 1]), LOGIN_SERVER);
    world.take_sent();
    d.step(0.1, true, &go);
    assert!(d.ended());
    let sent = sent_packets(&world);
    assert!(opcodes(&sent).is_empty(), "{:x?}", opcodes(&sent));
    assert_eq!(
        sent.iter().filter(|p| p.opt_headers.disconnect).count(),
        0,
        "§9.3.9: a server's close gets no closing reply"
    );
}

/// §9.3.9: a valid `0x6D` after the referral is counted and skipped: there is no list to go back to,
/// and the entry's clock goes on.
#[test]
fn a_refusal_after_the_referral_is_counted() {
    let (world, mut d, _login, mut server) = at_session_start(1.0);
    world.deliver(server.event(0x6D, &[0x00, 4, 3, 2, 1]), WORLD_SERVER);
    d.step(2.0, true, &go);
    let events = d.take_events();
    assert!(
        !events.iter().any(|e| matches!(
            e,
            Event::EnterWorldFailed { .. } | Event::Disconnected { .. }
        )),
        "{events:?}"
    );
    assert_eq!(d.counters().refusals_skipped, 1);
    assert!(!d.ended());
}

/// World §6's row of `0x6C`: a nonempty list reaches the screens with each entry's body, class and
/// level, and the account table triggers nothing. No spec maps the appearance to a look yet.
#[test]
fn a_nonempty_list_connects_with_names_only() {
    let (world, mut d, mut login) = {
        use super::initial_negotiation::connected_over;
        connected_over(Arc::new(WithSchema(
            records(),
            PortalStateSchema::default(),
        )))
    };
    world.deliver(
        login.event(0x6C, &service_support::character_list()),
        LOGIN_SERVER,
    );
    world.take_sent();
    d.step(0.1, true, &go);
    let events = d.take_events();
    let at = events
        .iter()
        .position(|e| matches!(e, Event::Connected { .. }))
        .expect("the list decodes to exact exhaustion");
    // §9.3.9's first-screen contract: names only, with no attribute guessed.
    assert_eq!(events[at + 1..], [], "{events:?}");
    let Event::Connected { characters } = &events[at] else {
        unreachable!("matched")
    };
    assert_eq!(characters[0].name, service_support::CHARACTER);
    assert!(opcodes(&sent_packets(&world)).is_empty());
}

/// §9.3.8's historical rows at driver level: repeats restart no list wait and emit no progress, and
/// a queued status with no position words still shows the queue.
#[test]
fn historical_repeats_have_one_clock() {
    use super::initial_negotiation::connected;
    use tessera_net::account::LoginStage;

    let status = |value: u32| {
        crate::world::encode_event_envelope(
            &crate::session::VersionStatusMessage {
                status: value,
                server_version_info: "x".to_owned(),
            }
            .encode()
            .expect("encodes"),
        )
    };
    let progress = |events: &[Event]| {
        events.iter().any(|e| {
            matches!(
                e,
                Event::Account(AccountEvent::LoginProgress {
                    stage: LoginStage::JoiningWorld
                })
            )
        })
    };
    let (world, mut d, mut login) = connected();
    world.deliver(login.event(0x12F, &[]), LOGIN_SERVER);
    d.step(0.1, true, &go);
    world.deliver(login.blob(0x12D, &status(1)), LOGIN_SERVER);
    d.step(1.0, true, &go);
    assert!(progress(&d.take_events()), "the real stage change");
    for t in [10.0, 20.0, 30.0] {
        world.deliver(login.blob(0x12D, &status(1)), LOGIN_SERVER);
        d.step(t, true, &go);
        assert!(!progress(&d.take_events()), "a repeat at {t}");
        assert!(!d.ended());
    }
    d.step(31.0, false, &go);
    let events = d.take_events();
    assert!(d.ended(), "{events:?}");
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Disconnected {
                reason: DisconnectReason::UnsupportedTimeout
            }
        )),
        "{events:?}"
    );

    // An unsolicited status 1 replaces no running `0xDD` wait.
    let (world, mut d, mut login) = connected();
    world.deliver(login.blob(0x12D, &status(1)), LOGIN_SERVER);
    d.step(29.0, true, &go);
    assert!(!progress(&d.take_events()));
    d.step(31.0, false, &go);
    assert!(d.take_events().iter().any(|e| matches!(
        e,
        Event::Disconnected {
            reason: DisconnectReason::UnsupportedTimeout
        }
    )));

    // Queued, with its text and no position: the queue panel with nothing known, on its limit.
    let (world, mut d, mut login) = connected();
    let mut queued = crate::session::LOGON_RESULT_QUEUED.to_le_bytes().to_vec();
    queued.push(0);
    world.deliver(
        login.blob(0x130, &crate::world::encode_event_envelope(&queued)),
        LOGIN_SERVER,
    );
    d.step(1.0, true, &go);
    assert!(d.take_events().iter().any(|e| matches!(
        e,
        Event::Account(AccountEvent::LoginQueueChanged {
            position: None,
            length: None,
            wait_seconds: None,
        })
    )));
    d.step(200.0, false, &go);
    assert!(!d.ended(), "the queue's limit, not the `0xDD` wait");
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
    assert_eq!(d.counters().refusals_skipped, 1);
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

    // Malformed `0x6D`: counted, with no result.
    let (world, mut d, mut login) = driver_at_characters(no_readiness(), 0);
    world.deliver(login.event(0x6D, &[0x00, 4, 3]), LOGIN_SERVER);
    d.step(1.0, true, &go);
    assert!(d.take_events().is_empty());
    assert_eq!(d.counters().refusals_skipped, 1);

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

    // Unverified bit 0x20: its framing is unproven; dropped, whatever its checksum says.
    let undefined = [0x02u8, 0x60, 1, 0, 1, 0, 0x10, 0x01];
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
            Event::Disconnected {
                reason: DisconnectReason::Unsupported
            }
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

/// §9.3.9's identities: an exact retransmission is ignored, a complete different body of a
/// remembered identity is delivered with an anomaly count, and past 30 s the identity is fresh.
#[test]
fn a_reused_identity_is_delivered_with_an_anomaly_and_an_exact_repeat_is_ignored() {
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
    assert_eq!(d.counters().reassembly.reused_identities, 0);
    // Within 30 s another body is an anomaly, and the session goes on.
    world.deliver(send(&mut login, 500, &[0x00, 3]), LOGIN_SERVER);
    d.step(33.0, true, &go);
    assert!(!d.ended());
    assert_eq!(d.counters().reassembly.reused_identities, 1);
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
        let events = d.take_events();
        let refused = events.iter().any(|e| {
            matches!(
                e,
                Event::Account(AccountEvent::LoginRefused {
                    reason: LoginRefusal::UnsupportedStep,
                    ..
                })
            )
        });
        let ended = if why.is_deadline() {
            DisconnectReason::UnsupportedTimeout
        } else {
            DisconnectReason::Unsupported
        };
        refused
            && events
                .iter()
                .any(|e| matches!(e, Event::Disconnected { reason } if *reason == ended))
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

        // §9.3.9: a later `0xDD` that does not read is counted; flags `02..FF` still fail.
        let (world, mut d, mut login) = at_advertisement();
        world.deliver(queue1(&mut login, 0xDD, &[0x00, 1, 2]), LOGIN_SERVER);
        d.step(0.2, true, &go);
        assert!(!d.ended());
        world.deliver(queue1(&mut login, 0xDD, &[0x04, 1, 2]), LOGIN_SERVER);
        d.step(0.3, true, &go);
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
