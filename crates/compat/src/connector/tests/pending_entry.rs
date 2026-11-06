// SPDX-License-Identifier: GPL-3.0-or-later
//! `official-session.md` §9.3.9's entry order at driver level: the once-unresolved forms of the
//! observed world stream through the real decoders, the pending placement and the client's
//! `WorldLoaded`; where a production path stops; and, on a test's stand-in for the server gate, the
//! order beyond it: the one `0xF4`, `WorldEntered`, and the release of the pending state.

use super::service_support::{
    ObservedMessage, observed_buffers, observed_raw_datagram, observed_world_stream,
};
use super::*;
use crate::connector::pending_world::{DEFAULT_RECIPIENT, MAX_PENDING_ENTITIES, withheld_summary};
use crate::world::{
    EncodedPropertyEntry, EncodedPropertyTable, EncodedPropertyValue, EntityCreate,
    PID_WORLD_TIME_OFFSET, PlayerSessionReady,
};
use tessera_net::WorldEntryStage;

const PLAYER: u64 = 0x0100_0000_0000_0007;

fn progress_stages(events: &[Event]) -> Vec<WorldEntryStage> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::WorldEntryProgress { stage } => Some(*stage),
            _ => None,
        })
        .collect()
}

fn sent_opcodes(world: &World) -> Vec<u32> {
    world
        .take_sent()
        .iter()
        .filter_map(|(d, _)| decode_client_datagram(d, false).ok())
        .flat_map(|p| p.fragments)
        .filter_map(|f| f.header.blob_header_word)
        .collect()
}

/// A session at its world connection's State 5, on `service`.
fn at_session_start(service: Service) -> (World, SessionDriver<FakeNet>, Server) {
    let (world, mut d, login) = driver_at_characters(service, 0);
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    d.step(1.1, true, &go);
    world.deliver(
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
        LOGIN_SERVER,
    );
    d.step(1.2, true, &go);
    let server = Server::new(0x0222, 0xBEEF_0002);
    server.connect_to(&world, 0xBEEF_0002, WORLD_SERVER);
    d.step(1.3, true, &go);
    world.take_sent();
    let _ = d.take_events();
    (world, d, server)
}

/// Every production path: no source for readiness or for the exact placement (OPEN-WORLD-9).
fn production() -> Service {
    Service {
        no_readiness: true,
        no_placement_source: true,
        ..Service::default()
    }
}

/// A test's scripted backend: an exact placement, and still no server gate.
fn scripted() -> Service {
    Service {
        no_readiness: true,
        ..Service::default()
    }
}

fn gated() -> Service {
    Service {
        no_readiness: true,
        server_gate: Some(0x7C),
        ..Service::default()
    }
}

fn ready_of(player: u64) -> Vec<u8> {
    PlayerSessionReady {
        player_id: player,
        session_props: EncodedPropertyTable::new(),
    }
    .to_bytes()
    .expect("0x90")
}

/// The datagram of `messages` from `server`, in their own forms.
fn datagram(server: &mut Server, messages: &[ObservedMessage]) -> Vec<u8> {
    server.seq += 1;
    let word = u16::try_from(server.seq & 0xFFFF).expect("small");
    let buffers: Vec<(Vec<u8>, Vec<u8>)> =
        messages.iter().map(|m| observed_buffers(m, word)).collect();
    let mask = server.isaac.next_mask();
    observed_raw_datagram(server.rec_id, server.seq, mask, &buffers, None)
}

/// The observed world stream: every once-unresolved form verifies and is taken, `0x19A`'s attachment
/// and siblings too; nothing is read as readiness. Its `0xCE`s go to the default recipient, so they
/// are kept under the raw key 2 and none is the player's placement (§9.3.9, §9.3.12 Question 1); a
/// scripted `0xCE` addressed to the player is what starts the scene.
#[test]
fn the_observed_world_stream_reads_to_the_pending_placement_and_the_readiness_wait() {
    let (world, mut d, mut server) = at_session_start(scripted());
    let mut t = 1.4;
    for messages in observed_world_stream() {
        world.deliver(datagram(&mut server, &messages), WORLD_SERVER);
        d.step(t, true, &go);
        t += 0.01;
    }
    assert_eq!(
        d.counters().refused_datagrams,
        0,
        "every form verifies with the same masks and dictionaries"
    );
    assert_eq!(d.counters().reassembly.empty_controls, 1);
    let p = d.entry_progress();
    assert!(p.session_started && !p.placed && !p.loaded);
    assert_eq!((p.batches, p.sub_events), (1, 2));
    assert_eq!(p.creations, 2, "the `0x98` and its paired `0x52`");
    assert_eq!(p.appearances, 2, "the announced entity's, and key 2's");
    assert_eq!(
        p.default_target, 5,
        "the first `0x89` and the three `0xCE`s, kept under key 2, and the default-recipient \
         `0x40`, skipped"
    );
    assert_eq!(
        p.actions, 1,
        "`0xA8` is counted until its variant is rechecked"
    );
    assert_eq!(p.positions, 3, "all three under the raw key 2");
    assert_eq!(p.readiness_seen, 2, "`0x5E` and `0x7C`: seen, not acted on");
    assert_eq!(p.unsupported_batches, 1);
    assert_eq!(p.unread, 3, "the opaque 0x88 and the two opaque 0x92");
    assert_eq!(d.pending_sizes().0, 2, "the other entity and the raw key 2");
    assert!(
        !d.take_events()
            .iter()
            .any(|e| matches!(e, Event::WorldPlacementPending { .. })),
        "a default-recipient 0xCE is not the player's"
    );
    world.deliver(
        server.event_to(0xCE, PLAYER, &service_support::player_position()),
        WORLD_SERVER,
    );
    d.step(t, true, &go);
    world.deliver(
        server.event_to(0xCE, PLAYER, &service_support::player_position()),
        WORLD_SERVER,
    );
    d.step(t + 0.01, true, &go);
    assert!(d.entry_progress().placed);
    let events = d.take_events();
    assert_eq!(progress_stages(&events), [WorldEntryStage::Placement]);
    let placed: Vec<&Event> = events
        .iter()
        .filter(|e| matches!(e, Event::WorldPlacementPending { .. }))
        .collect();
    assert_eq!(placed.len(), 1, "the same placement again is no new one");
    let Event::WorldPlacementPending { character, .. } = placed[0] else {
        unreachable!("matched")
    };
    assert_eq!(character, "Aldo");
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::WorldEntered { .. } | Event::EntityCreated { .. })),
        "nothing is published: {events:?}"
    );
}

/// A scripted placement with no server gate, as a production path has none: the client loads the
/// placement, `WorldLoaded` sets the local gate, no `0xF4` goes out, and the readiness wait ends in `UnsupportedTimeout`, naming the stage and what was
/// withheld in the log. One `WorldLoaded` per placement: another is counted. No loading time-out
/// makes readiness.
#[test]
fn a_production_entry_stops_at_the_readiness_wait_and_withholds_every_unsourced_send() {
    let ((events, sent), logs) = super::diagnostics::logs_of(|| {
        let (world, mut d, mut server) = at_session_start(scripted());
        world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
        d.step(2.0, true, &go);
        let at = service_support::player_position();
        world.deliver(server.event_to(0xCE, PLAYER, &at), WORLD_SERVER);
        d.step(2.1, true, &go);
        d.command(
            &Command::WorldLoaded {
                generation: Some(1),
            },
            2.2,
        )
        .expect("loaded");
        assert!(d.entry_progress().loaded);
        d.command(
            &Command::WorldLoaded {
                generation: Some(1),
            },
            2.3,
        )
        .expect_err("a duplicate is refused");
        assert_eq!(d.counters().stray_loaded, 1);
        // A server message cannot set the gate here: no source (OPEN-WORLD-9).
        world.deliver(server.event(0x7C, &[0x00]), WORLD_SERVER);
        d.step(29.0, true, &go);
        assert!(!d.in_world() && !d.ended());
        let sent = sent_opcodes(&world);
        d.step(32.1, false, &go);
        (d.take_events(), sent)
    });
    assert!(!sent.contains(&0xF4), "{sent:x?}");
    for family in [0x30u32, 0x24, 0x19A, 0x3000_0035, 0x13D, 0x19E] {
        assert!(!sent.contains(&family), "{sent:x?}");
    }
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Disconnected {
                reason: DisconnectReason::UnsupportedTimeout
            }
        )),
        "{events:?}"
    );
    let line = logs
        .lines()
        .find(|l| l.contains("official world entry: stopped"))
        .unwrap_or_else(|| panic!("{logs}"));
    assert!(line.contains("waiting for world readiness"), "{line}");
    assert!(line.contains(&withheld_summary()), "{line}");
}

/// The stage line of a stopped entry carries what was withheld, on every road out. A selection
/// refused before the referral is where the omissions matter most (decision 0046 items 5 and 9).
#[test]
fn a_refused_selection_writes_the_stage_line_with_what_was_withheld() {
    let logs = super::diagnostics::logs_of(|| {
        let (world, mut d, mut login) = driver_at_characters(production(), 0);
        d.command(
            &Command::EnterWorld {
                character: "Aldo".to_owned(),
            },
            1.0,
        )
        .expect("enter");
        d.step(1.1, true, &go);
        world.deliver(login.event(0x6D, &[0x00, 4, 3, 2, 1]), LOGIN_SERVER);
        d.step(1.2, true, &go);
        let events = d.take_events();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::EnterWorldFailed { .. })),
            "{events:?}"
        );
    })
    .1;
    let line = logs
        .lines()
        .find(|l| l.contains("official world entry: stopped"))
        .unwrap_or_else(|| panic!("{logs}"));
    assert!(line.contains("selection refused"), "{line}");
    assert!(line.contains(&withheld_summary()), "{line}");
}

/// The network right revoked during an entry: nothing is sent, and the log still has the line.
#[test]
fn a_revoked_entry_writes_the_stage_line_and_sends_nothing() {
    let logs = super::diagnostics::logs_of(|| {
        let (world, mut d, mut server) = at_session_start(production());
        world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
        d.step(2.0, true, &go);
        world.take_sent();
        d.abandon(2.1);
        assert!(world.take_sent().is_empty());
    })
    .1;
    let lines: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("official world entry: stopped"))
        .collect();
    assert_eq!(lines.len(), 1, "{logs}");
    assert!(lines[0].contains("network right revoked"), "{logs}");
    assert!(lines[0].contains(&withheld_summary()), "{logs}");
}

/// A placement outside what a location can be is no exact placement: no progress, no scene to load,
/// and a kept placement and its answer stay. Nothing reaches the `Session`'s invalid-event path.
#[test]
fn an_out_of_range_placement_is_no_placement() {
    let (world, mut d, mut server) = at_session_start(gated());
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    d.step(2.0, true, &go);
    let _ = d.take_events();
    let far = service_support::position(PLAYER, [1e30, 0.0, 0.0], 0, [0.0; 3]);
    world.deliver(server.event(0xCE, &far), WORLD_SERVER);
    d.step(2.1, true, &go);
    assert!(!d.entry_progress().placed);
    assert_eq!(d.entry_progress().unread, 1);
    assert_eq!(d.entry_progress().positions, 0);
    assert!(!d.take_events().iter().any(|e| matches!(
        e,
        Event::WorldPlacementPending { .. } | Event::WorldEntryProgress { .. }
    )));
    // A valid one, answered: the same message after it changes nothing.
    world.deliver(
        server.event_to(0xCE, PLAYER, &service_support::player_position()),
        WORLD_SERVER,
    );
    d.step(2.2, true, &go);
    assert!(d.entry_progress().placed);
    let _ = d.take_events();
    d.command(
        &Command::WorldLoaded {
            generation: Some(1),
        },
        2.3,
    )
    .expect("loaded");
    world.deliver(server.event(0xCE, &far), WORLD_SERVER);
    d.step(2.4, true, &go);
    assert_eq!(d.entry_progress().unread, 2);
    assert!(
        !d.take_events()
            .iter()
            .any(|e| matches!(e, Event::WorldPlacementPending { .. }))
    );
    // The local gate stands, so the stand-in for the server gate finishes the entry.
    world.deliver(server.event(0x7C, &[0x00]), WORLD_SERVER);
    d.step(2.5, true, &go);
    assert!(d.in_world());
}

/// The same for the `0xCC` form, which has its own decoder.
#[test]
fn an_out_of_range_portal_placement_is_no_placement() {
    let (world, mut d, mut server) = at_session_start_with_schema();
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    d.step(2.0, true, &go);
    let _ = d.take_events();
    // The `WirePosition`'s offset in the message: its bytes follow the valid form's, so the
    // message is rebuilt with the first offset's bytes replaced.
    let good = checked_message(0xCC);
    let needle = 1.0f32.to_le_bytes();
    let offset_at = good
        .windows(4)
        .position(|w| w == needle)
        .expect("the offset's first coordinate");
    let mut far = good.clone();
    far[offset_at..offset_at + 4].copy_from_slice(&1e30f32.to_le_bytes());
    world.deliver(
        server.on_queue(3, 1, 0, Some(PLAYER), 0xCC, &far),
        WORLD_SERVER,
    );
    d.step(2.1, true, &go);
    assert!(!d.entry_progress().placed);
    assert_eq!(d.entry_progress().unread, 1);
    assert!(
        !d.take_events()
            .iter()
            .any(|e| matches!(e, Event::WorldPlacementPending { .. }))
    );
}

/// A batch's `0x52` paired with a `0x98` that did not decode made no entity. It is counted and does
/// not reach the batch's target, here the local player, whose own table stays.
#[test]
fn the_initial_properties_of_an_undecodable_creation_are_nobodys() {
    let (world, mut d, mut server) = at_session_start_with_schema();
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    d.step(2.0, true, &go);
    let own = checked_message(0x52);
    world.deliver(
        server.on_queue(2, 1, 1, Some(PLAYER), 0x52, &own),
        WORLD_SERVER,
    );
    d.step(2.1, true, &go);
    let before = d.entry_progress();
    assert_eq!(before.creations, 1);
    let kept = d.pending_sizes();
    let batch = crate::world::SubEventBatch {
        event_id: crate::world::SubEventBatch::EVENT_ID_SUB_EVENT,
        frames: vec![
            crate::world::SubEventFrame::Event {
                sub_event_id: 0x98,
                payload: vec![0x00],
            },
            crate::world::SubEventFrame::Event {
                sub_event_id: 0x52,
                payload: own.clone(),
            },
        ],
        entry_batch_tail_flag: None,
    }
    .to_bytes()
    .expect("batch");
    world.deliver(
        server.on_queue(2, 2, 2, Some(PLAYER), 0x4C, &batch),
        WORLD_SERVER,
    );
    d.step(2.2, true, &go);
    let p = d.entry_progress();
    assert_eq!(p.orphaned_initial, 1);
    assert_eq!(p.unread, before.unread + 1, "the 0x98 itself");
    assert_eq!(p.creations, before.creations);
    assert_eq!(d.pending_sizes(), kept);
    assert!(!d.ended());
}

/// `official-messages-play.md` (P10-10H): a chat-server route on a session with no chat connection is
/// a local `InvalidCommand` in these words, and the session goes on.
#[test]
fn a_chat_server_route_fails_locally_in_the_specified_words() {
    let (world, mut d, mut server) = at_session_start(Service {
        no_chat_codec: true,
        ..gated()
    });
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    world.deliver(
        server.event_to(0xCE, PLAYER, &service_support::player_position()),
        WORLD_SERVER,
    );
    d.step(2.0, true, &go);
    d.command(
        &Command::WorldLoaded {
            generation: Some(1),
        },
        2.1,
    )
    .expect("loaded");
    world.deliver(server.event(0x7C, &[0x00]), WORLD_SERVER);
    d.step(2.2, true, &go);
    assert!(d.in_world());
    world.take_sent();
    let refused = d.command(
        &Command::Social(tessera_net::SocialCommand::JoinChannel { name: "x".into() }),
        2.3,
    );
    assert!(
        matches!(
            refused,
            Err(tessera_net::NetError::InvalidCommand(
                "Chat server connection is not supported by this version of Tessera yet."
            ))
        ),
        "{refused:?}"
    );
    assert!(d.in_world() && !d.ended());
}

/// A malformed `0x98` is a checked message that did not read, not a routing problem.
#[test]
fn a_malformed_creation_is_unread() {
    let (world, mut d, mut server) = at_session_start(production());
    world.deliver(server.event(0x98, &[0x00]), WORLD_SERVER);
    d.step(1.5, true, &go);
    let p = d.entry_progress();
    assert_eq!((p.unread, p.no_target, p.default_target), (1, 0, 0));
}

/// A placement in the default-recipient form names no entity (OPEN-WORLD-9): it is counted, and no
/// placement is guessed. An explicit recipient that is the session's ID is exact.
#[test]
fn a_default_recipient_placement_is_not_the_local_player() {
    let (world, mut d, mut server) = at_session_start_with_schema();
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    d.step(2.0, true, &go);
    let sync = checked_message(0xCC);
    world.deliver(server.on_queue(3, 1, 0, None, 0xCC, &sync), WORLD_SERVER);
    d.step(2.1, true, &go);
    assert_eq!(d.entry_progress().default_target, 1);
    assert!(!d.entry_progress().placed);
    let events = d.take_events();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::WorldPlacementPending { .. })),
        "{events:?}"
    );
    world.deliver(
        server.on_queue(3, 2, 0, Some(PLAYER), 0xCC, &sync),
        WORLD_SERVER,
    );
    d.step(2.2, true, &go);
    assert!(d.entry_progress().placed);
    assert!(
        d.take_events()
            .iter()
            .any(|e| matches!(e, Event::WorldPlacementPending { .. }))
    );
}

/// Only the world peer's data populates the pending state: the retiring login connection's messages
/// and its close concern nothing any more; a duplicate referral creates no connection.
#[test]
fn the_retiring_login_connection_concerns_the_session_no_more() {
    let (world, mut d, login) = driver_at_characters(production(), 0);
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    d.step(1.1, true, &go);
    world.deliver(
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
        LOGIN_SERVER,
    );
    d.step(1.2, true, &go);
    assert_eq!(
        progress_stages(&d.take_events()),
        [WorldEntryStage::Referral]
    );
    let mut login = login;
    let server = Server::new(0x0222, 0xBEEF_0002);
    server.connect_to(&world, 0xBEEF_0002, WORLD_SERVER);
    d.step(1.3, true, &go);
    assert_eq!(
        progress_stages(&d.take_events()),
        [WorldEntryStage::WorldHandshake]
    );
    assert_eq!(
        d.connection_states().0,
        None,
        "retired at the world's State 5"
    );
    world.take_sent();
    world.deliver(
        login.event(
            0x98,
            &EntityCreate {
                template_did: 1,
                entity_id: 0x0300_0000_0000_0101,
            }
            .to_bytes()
            .expect("0x98"),
        ),
        LOGIN_SERVER,
    );
    world.deliver(login.event(0xB3, &[0x00, 1, 0, 0, 0]), LOGIN_SERVER);
    world.deliver(login.event(0x6D, &[0x00, 1, 0, 0, 0]), LOGIN_SERVER);
    world.deliver(
        login.referral(0x1122_3344_5566_7700, WORLD_SERVER),
        LOGIN_SERVER,
    );
    d.step(1.4, true, &go);
    assert!(!d.ended());
    assert_eq!(d.pending_sizes().0, 0);
    assert_eq!(d.counters().stray_referrals, 0, "no connection takes it");
    assert!(!d.take_events().iter().any(|e| matches!(
        e,
        Event::EnterWorldFailed { .. } | Event::Disconnected { .. }
    )));
    // A second referral made no connection.
    assert!(
        world
            .take_sent()
            .iter()
            .filter_map(|(p, _)| decode_client_datagram(p, false).ok())
            .all(|p| p.opt_headers.referral_auth.is_none())
    );
    // The login's own `Disconnect` does not end the session.
    world.deliver(
        login.control(OptionalHeaders {
            disconnect: true,
            ..OptionalHeaders::default()
        }),
        LOGIN_SERVER,
    );
    d.step(1.5, true, &go);
    assert!(!d.ended());
}

/// §9.3.9's bound: past 256 target keys a new key is counted and skipped, and the entry goes on: an
/// overflow never ends a healthy entry, sends nothing and keeps the local identity and placement.
#[test]
fn a_full_pending_store_counts_and_skips_and_the_entry_goes_on() {
    let (world, mut d, mut server) = at_session_start(scripted());
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    d.step(1.4, true, &go);
    let _ = d.take_events();
    world.take_sent();
    for i in 0..=MAX_PENDING_ENTITIES as u64 {
        let create = EntityCreate {
            template_did: 1,
            entity_id: 0x0300_0000_0001_0000 + i,
        }
        .to_bytes()
        .expect("0x98");
        world.deliver(server.event(0x98, &create), WORLD_SERVER);
        d.step(1.5, true, &go);
    }
    assert!(!d.ended());
    assert_eq!(d.pending_sizes().0, MAX_PENDING_ENTITIES);
    assert_eq!(d.entry_progress().target_cap_hits, 1);
    assert!(d.take_events().is_empty(), "no terminal result");
    assert!(
        !world
            .take_sent()
            .iter()
            .filter_map(|(p, _)| decode_client_datagram(p, false).ok())
            .any(|p| p.opt_headers.disconnect)
    );
    // The player's placement has no room in the store, and still reaches the client.
    world.deliver(
        server.event_to(0xCE, PLAYER, &service_support::player_position()),
        WORLD_SERVER,
    );
    d.step(1.6, true, &go);
    assert!(
        d.take_events()
            .iter()
            .any(|e| matches!(e, Event::WorldPlacementPending { generation: 1, .. }))
    );
    assert_eq!(d.entry_progress().target_cap_hits, 2);
    // It ends at the readiness wait like any other.
    d.step(32.0, false, &go);
    assert!(d.take_events().iter().any(|e| matches!(
        e,
        Event::Disconnected {
            reason: DisconnectReason::UnsupportedTimeout
        }
    )));
}

/// A test's stand-in for the server gate (OPEN-WORLD-9), to the end of the order: the one `0xF4` in
/// Vector 18's form, then `WorldEntered`, then the pending state released, spawn first. The client's
/// `WorldLoaded` alone does nothing, and the gate alone does nothing.
#[test]
fn both_gates_make_one_ready_message_then_world_entered_then_the_release() {
    let (world, mut d, mut server) = at_session_start(gated());
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    let npc = service_support::observed_npc();
    let create = EntityCreate {
        template_did: service_support::observed_npc_template(),
        entity_id: npc,
    }
    .to_bytes()
    .expect("0x98");
    world.deliver(
        server.event_to(0xCE, npc, &service_support::observed_npc_position()),
        WORLD_SERVER,
    );
    world.deliver(server.event(0x98, &create), WORLD_SERVER);
    world.deliver(
        server.event_to(0xCE, npc, &service_support::observed_npc_position()),
        WORLD_SERVER,
    );
    world.deliver(
        server.event_to(0xCE, PLAYER, &service_support::player_position()),
        WORLD_SERVER,
    );
    d.step(2.0, true, &go);
    let events = d.take_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::WorldPlacementPending { .. })),
        "{events:?}"
    );
    assert_eq!(d.pending_sizes().0, 2);
    // The server gate alone.
    world.deliver(server.event(0x7C, &[0x00]), WORLD_SERVER);
    d.step(2.1, true, &go);
    assert!(!d.in_world());
    assert!(!sent_opcodes(&world).contains(&0xF4));
    // And the client's: the entry ends here, in this order.
    d.command(
        &Command::WorldLoaded {
            generation: Some(1),
        },
        2.2,
    )
    .expect("loaded");
    d.step(2.3, true, &go);
    assert!(d.in_world());
    let sent = world.take_sent();
    let packets: Vec<_> = sent
        .iter()
        .filter_map(|(p, _)| decode_client_datagram(p, false).ok())
        .collect();
    let f4: Vec<_> = packets
        .iter()
        .flat_map(|p| &p.fragments)
        .filter(|f| f.header.blob_header_word == Some(0xF4))
        .collect();
    assert_eq!(f4.len(), 1);
    assert_eq!(f4[0].payload, [0x00]);
    assert_eq!(f4[0].header.queue_id, 1);
    assert_eq!(f4[0].header.recipient_id, 2);
    assert_eq!(f4[0].header.blob_flags & 0x40, 0x40, "the default form");
    assert_eq!(f4[0].header.ext_word, None);
    let events = d.take_events();
    let entered = events
        .iter()
        .position(|e| matches!(e, Event::WorldEntered { .. }))
        .expect("entered");
    let Event::WorldEntered { player, name, .. } = &events[entered] else {
        unreachable!("matched")
    };
    assert_eq!((player.0, name.as_str()), (PLAYER, "Aldo"));
    assert_eq!(d.pending_sizes(), (0, 0), "released");
    // What was kept reaches the feed after `WorldEntered`, not before.
    assert!(
        !events[..entered].iter().any(|e| matches!(
            e,
            Event::EntityCreated { .. } | Event::LocationReport { .. }
        )),
        "{events:?}"
    );
    // A later `WorldLoaded` ends no portal space: there is none, and no second `0xF4` follows.
    let _ = d.command(
        &Command::WorldLoaded {
            generation: Some(1),
        },
        2.4,
    );
    d.step(2.5, true, &go);
    assert!(!sent_opcodes(&world).contains(&0xF4));
}

/// Vector 20: a base-model `0x89`, direct or nested, is counted and skipped; its siblings are kept,
/// and nothing ends.
#[test]
fn an_unsupported_appearance_is_counted_without_losing_its_siblings() {
    let (world, mut d, mut server) = at_session_start(production());
    let npc = service_support::observed_npc();
    let create = EntityCreate {
        template_did: service_support::observed_npc_template(),
        entity_id: npc,
    }
    .to_bytes()
    .expect("0x98");
    world.deliver(server.event(0x98, &create), WORLD_SERVER);
    let mut base_model = service_support::observed_appearance().to_vec();
    base_model[1] |= 0x01;
    world.deliver(
        server.on_queue(4, 1, 0, Some(npc), 0x89, &base_model),
        WORLD_SERVER,
    );
    let batch = crate::world::SubEventBatch {
        event_id: crate::world::SubEventBatch::EVENT_ID_SUB_EVENT,
        frames: vec![
            crate::world::SubEventFrame::Event {
                sub_event_id: 0x89,
                payload: base_model.clone(),
            },
            crate::world::SubEventFrame::Event {
                sub_event_id: 0x89,
                payload: service_support::observed_appearance().to_vec(),
            },
        ],
        entry_batch_tail_flag: None,
    }
    .to_bytes()
    .expect("batch");
    world.deliver(
        server.on_queue(2, 2, 2, Some(npc), 0x4C, &batch),
        WORLD_SERVER,
    );
    d.step(2.0, true, &go);
    assert!(!d.ended());
    let p = d.entry_progress();
    assert_eq!((p.unread, p.appearances), (2, 1));
}

/// `docs/m8-remediation.md` §27 fix 6: an entry has one placement. Later `0xCE` and `0xCC` updates are
/// kept, ask for no second load and do not clear the gate, so `WorldEntered` needs no second signal.
#[test]
fn a_later_position_does_not_ask_for_another_load() {
    let (world, mut d, mut server) = at_session_start(gated());
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    world.deliver(
        server.event_to(0xCE, PLAYER, &service_support::player_position()),
        WORLD_SERVER,
    );
    d.step(2.0, true, &go);
    d.command(
        &Command::WorldLoaded {
            generation: Some(1),
        },
        2.1,
    )
    .expect("loaded");
    world.deliver(
        server.event_to(
            0xCE,
            PLAYER,
            &service_support::position(PLAYER, [9.0, 9.0, 9.0], 0, [0.0; 3]),
        ),
        WORLD_SERVER,
    );
    d.step(2.2, true, &go);
    assert_eq!(
        d.take_events()
            .iter()
            .filter(|e| matches!(e, Event::WorldPlacementPending { .. }))
            .count(),
        1
    );
    assert_eq!(d.entry_progress().positions, 2, "the update is kept");
    world.deliver(server.event(0x7C, &[0x00]), WORLD_SERVER);
    d.step(2.3, true, &go);
    assert!(d.in_world(), "the one signal is enough");
    assert_eq!(d.counters().stray_loaded, 0);
}

/// `docs/m8-remediation.md` §27 fix 6, on a production path: nothing names the exact placement yet,
/// so no `WorldPlacementPending` goes out, for a `0xCE` naming the `0x90`'s ID, its `0xCC`, or one
/// that came before the `0x90`; the data is kept and `WorldLoaded` has nothing to answer.
#[test]
fn no_production_message_is_the_exact_placement() {
    let (world, mut d, mut server) = at_session_start(production());
    let position = service_support::player_position();
    world.deliver(server.event(0xCE, &position), WORLD_SERVER);
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    world.deliver(server.event(0xCE, &position), WORLD_SERVER);
    d.step(2.0, true, &go);
    let events = d.take_events();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::WorldPlacementPending { .. })),
        "{events:?}"
    );
    assert_eq!(progress_stages(&events), [WorldEntryStage::Identity]);
    let p = d.entry_progress();
    assert!(p.session_started && !p.placed);
    assert_eq!(
        p.positions, 2,
        "both of the player's own positions are kept"
    );
    d.command(
        &Command::WorldLoaded {
            generation: Some(1),
        },
        2.1,
    )
    .expect_err("before any placement");
    assert!(!d.entry_progress().loaded);
    assert_eq!(d.counters().stray_loaded, 1);
    // Its deadline is the readiness wait's, with Tessera's own reason.
    d.step(32.1, false, &go);
    assert!(d.take_events().iter().any(|e| matches!(
        e,
        Event::Disconnected {
            reason: DisconnectReason::UnsupportedTimeout
        }
    )));
}

/// `docs/m8-remediation.md` §27 fix 16: `0x40` removes the pending entity, nothing is published
/// before `WorldEntered`, and a later update does not bring the entity back. Nothing of it reaches
/// the client at the release.
#[test]
fn a_pending_despawn_removes_the_entity_and_nothing_brings_it_back() {
    const OTHER: u64 = 0x0100_0000_0000_0123;
    let (world, mut d, mut server) = at_session_start(gated());
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    world.deliver(
        server.event_to(0xCE, PLAYER, &service_support::player_position()),
        WORLD_SERVER,
    );
    let create = EntityCreate {
        template_did: 0x7000_0001,
        entity_id: OTHER,
    }
    .to_bytes()
    .expect("0x98");
    world.deliver(server.event(0x98, &create), WORLD_SERVER);
    d.step(2.0, true, &go);
    assert_eq!(d.pending_sizes().0, 2);
    world.deliver(server.event_to(0x40, OTHER, &[0x00]), WORLD_SERVER);
    d.step(2.1, true, &go);
    assert_eq!(d.pending_sizes().0, 2, "its record stays");
    assert_eq!(d.entry_progress().despawns, 1);
    let events = d.take_events();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::EntityDespawned { .. })),
        "{events:?}"
    );
    // A later update of it is refused by the record.
    let positions = d.entry_progress().positions;
    world.deliver(
        server.event_to(
            0xCE,
            OTHER,
            &service_support::position(OTHER, [1.0; 3], 0, [0.0; 3]),
        ),
        WORLD_SERVER,
    );
    world.deliver(server.event_to(0x40, OTHER, &[0x00]), WORLD_SERVER);
    d.step(2.2, true, &go);
    assert_eq!(d.pending_sizes().0, 2, "no resurrection");
    assert_eq!(d.entry_progress().positions, positions, "nothing kept");
    assert_eq!(d.entry_progress().despawns, 1, "nothing left to remove");
    assert_eq!(d.entry_progress().despawn_blocked, 2);

    d.command(
        &Command::WorldLoaded {
            generation: Some(1),
        },
        2.3,
    )
    .expect("loaded");
    world.deliver(server.event(0x7C, &[0x00]), WORLD_SERVER);
    d.step(2.4, true, &go);
    assert!(d.in_world());
    let events = d.take_events();
    assert!(
        !events.iter().any(|e| matches!(
            e,
            Event::EntityCreated { id, .. } if id.0 == OTHER
        )),
        "{events:?}"
    );
}

/// `docs/m8-remediation.md` §27 fix 16: a server `0x30`, `0x60` or `0x19A` before `WorldEntered` is
/// counted and discarded: nothing runs, nothing is answered and nothing is kept.
#[test]
fn server_rpcs_before_the_world_are_counted_and_discarded() {
    let (world, mut d, mut server) = at_session_start(production());
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    d.step(2.0, true, &go);
    let _ = world.take_sent();
    for opcode in [0x30u32, 0x60, 0x19A] {
        world.deliver(server.event(opcode, &[0x00, 1, 2, 3]), WORLD_SERVER);
    }
    d.step(2.1, true, &go);
    assert_eq!(d.entry_progress().other, 3);
    assert_eq!(d.pending_sizes(), (0, 0));
    assert!(
        !sent_opcodes(&world)
            .iter()
            .any(|o| *o == 0x30 || *o == 0xD0)
    );
}

/// `docs/m8-remediation.md` §27 fix 7: a NAK of the `0xF4` sends it again, the same bytes, on the
/// default-recipient form; the entry is not lost to an out-of-order answer.
#[test]
fn the_ready_message_is_kept_and_sent_again_on_a_nak() {
    let (world, mut d, mut server) = at_session_start(gated());
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    world.deliver(
        server.event_to(0xCE, PLAYER, &service_support::player_position()),
        WORLD_SERVER,
    );
    d.step(2.0, true, &go);
    d.command(
        &Command::WorldLoaded {
            generation: Some(1),
        },
        2.1,
    )
    .expect("loaded");
    let _ = world.take_sent();
    world.deliver(server.event(0x7C, &[0x00]), WORLD_SERVER);
    d.step(2.2, true, &go);
    assert!(d.in_world());
    let ready = world
        .take_sent()
        .into_iter()
        .find_map(|(bytes, _)| {
            let packet = decode_client_datagram(&bytes, false).ok()?;
            packet
                .fragments
                .iter()
                .any(|f| f.header.blob_header_word == Some(0xF4))
                .then_some((bytes, packet.header.seq_num))
        })
        .expect("the one 0xF4");
    world.deliver(
        server.control(OptionalHeaders {
            sequence_nak: Some(vec![ready.1]),
            ..OptionalHeaders::default()
        }),
        WORLD_SERVER,
    );
    d.step(2.3, true, &go);
    let again: Vec<Vec<u8>> = world
        .take_sent()
        .into_iter()
        .map(|(bytes, _)| bytes)
        .filter(|bytes| *bytes == ready.0)
        .collect();
    assert_eq!(again.len(), 1, "the same bytes, once");
}

/// `docs/m8-remediation.md` §27 fix 5: while `0x24` is held a `Command::Move` in the world sends
/// nothing, not even the historical 38-byte form, and the session goes on.
#[test]
fn a_move_in_the_world_is_held() {
    let (world, mut d, mut server) = at_session_start(gated());
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    world.deliver(
        server.event_to(0xCE, PLAYER, &service_support::player_position()),
        WORLD_SERVER,
    );
    d.step(2.0, true, &go);
    d.command(
        &Command::WorldLoaded {
            generation: Some(1),
        },
        2.1,
    )
    .expect("loaded");
    world.deliver(server.event(0x7C, &[0x00]), WORLD_SERVER);
    d.step(2.2, true, &go);
    assert!(d.in_world());
    let _ = world.take_sent();
    for i in 0..3 {
        d.command(
            &Command::Move {
                location: tessera_net::Location {
                    region: tessera_net::RegionId(1),
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                heading: 0.0,
                velocity: tessera_net::Vec3::new(1.0, 0.0, 0.0),
                on_ground: true,
            },
            2.3 + f64::from(i) / 10.0,
        )
        .expect_err("held: a local `InvalidCommand`");
        d.step(2.3 + f64::from(i) / 10.0, true, &go);
    }
    assert_eq!(d.counters().moves_held, 3);
    assert!(!sent_opcodes(&world).contains(&0x24));
    assert!(d.in_world());
}

/// The evidence target's lines and the other lines `docs/DEVELOPMENT.md` tells the owner to paste, by
/// the very pattern it gives.
fn paste_lines(logs: &str) -> Vec<&str> {
    logs.lines()
        .filter(|l| {
            [
                "official_evidence",
                "official session:",
                "official world entry:",
                "official login:",
            ]
            .iter()
            .any(|needle| l.contains(needle))
        })
        .collect()
}

/// The lines under the documented `RUST_LOG=info,official_evidence=debug`: anything at `info` or above,
/// and the evidence target at `debug`.
fn lines_of_the_documented_filter(logs: &str) -> Vec<&str> {
    logs.lines()
        .filter(|l| {
            let level = l.trim_start();
            ["ERROR", "WARN", "INFO"]
                .iter()
                .any(|w| level.starts_with(w))
                || (level.starts_with("DEBUG") && level.contains("official_evidence:"))
        })
        .collect()
}

/// No address, port, account, ticket, cookie, name, ID or identifier in any of `lines`, in any form
/// the fixtures use.
fn assert_nothing_private(lines: &[&str]) {
    let peer = |a: SocketAddrV4| [a.ip().to_string(), a.port().to_string()];
    for line in lines {
        // These labels of the auth mode and the bind case name no value.
        let line = &line
            .replace("ticket text in extraBlob", "")
            .replace("seed from the routed address", "")
            .replace("(referral cookie)", "");
        for private in peer(WORLD_SERVER).into_iter().chain(peer(LOGIN_SERVER)) {
            assert!(!line.contains(&private), "{private} in {line}");
        }
        for private in [
            "Aldo",
            "acct",
            "ticket",
            "cookie",
            "seed",
            "0x100000000000007",
            "72057594037927943",
            "1122334455667788",
            "1234605616436508552",
            "password",
            "didver",
            "netver:",
        ] {
            assert!(!line.contains(private), "{private} in {line}");
        }
    }
}

/// `docs/DEVELOPMENT.md` "Reading a live official run", `official-session.md` §9.3.9 "Evidence for
/// transport producers": a run's evidence lines name their stage, opcode, queue, sequence, local word
/// and flow counters with times since the stage's start, keep the entry's two gates apart at its
/// deadline, and carry no address, port, account, ticket, cookie, name or ID.
#[test]
fn the_evidence_lines_say_what_ran_and_nothing_private() {
    let (_, logs) = super::diagnostics::logs_of(|| {
        let (world, mut d, mut server) = at_session_start(scripted());
        world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
        world.deliver(
            server.event_to(0xCE, PLAYER, &service_support::player_position()),
            WORLD_SERVER,
        );
        d.step(2.0, true, &go);
        d.step(7.5, true, &go);
        d.command(
            &Command::WorldLoaded {
                generation: Some(1),
            },
            8.0,
        )
        .expect("loaded");
        d.step(8.1, true, &go);
        d.step(32.5, false, &go);
    });
    let evidence: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains(crate::session::EVIDENCE_TARGET))
        .collect();
    let all = evidence.join("\n");
    for needle in [
        "official evidence: stage",
        "stage=\"waiting for the referral\"",
        "stage=\"connecting to the world server\"",
        "stage=\"waiting for world readiness\"",
        "official evidence: packet",
        "dir=\"send\"",
        "kind=\"new\"",
        // The handshake's packets and the one close of a local terminal end, with their words: the
        // accept is the one step from 0 to 1.
        "kind=\"auth\"",
        "kind=\"accept\"",
        "kind=\"disconnect\"",
        "word_before=0 word_after=1 word_input=\"accept-initial\"",
        "word_input=\"handshake-unchanged\"",
        "word_input=\"control-unchanged\"",
        "opcode=0x6f",
        "queue=2",
        "word_before=",
        "word_after=",
        "word_input=\"sequenced-plus-one\"",
        "flow_event=",
        "flow_accumulated=",
        "flow_scope=\"udp-payload-of-accepted-datagrams\"",
        "official evidence: blob",
        "opcode=0x90",
        "first_of_opcode=true",
        "official evidence: datagram",
        "official evidence: counts after the session start",
        "positions=1",
        "official evidence: client loading done",
        "load_ms=",
        "readiness_clock_ms=",
        "official evidence: entry deadline",
        "server_gate=\"no source (OPEN-WORLD-9)\"",
        "local_gate=\"open\"",
        "limit_ms=30000",
        "ms=",
    ] {
        assert!(all.contains(needle), "{needle} not logged:\n{all}");
    }
    let accept = evidence
        .iter()
        .find(|l| l.contains("kind=\"accept\""))
        .expect("the accept");
    assert!(accept.contains("seq=0 "), "{accept}");
    let close = evidence
        .iter()
        .find(|l| l.contains("kind=\"disconnect\"") && l.contains("role=\"world\""))
        .unwrap_or_else(|| panic!("the world's Disconnect:\n{all}"));
    assert!(
        close.contains("word_input=\"control-unchanged\""),
        "{close}"
    );
    assert_nothing_private(&paste_lines(&logs));
    // Bounded per connection, and a connection's totals are there when it goes.
    assert!(evidence.len() < 2000, "{}", evidence.len());
}

/// The server's close: its codes are in the lines of the documented command, with no address, and the
/// connection's end and totals follow.
#[test]
fn the_server_close_lines_say_the_code_and_nothing_private() {
    for (fatal, reason) in [
        (false, "server transport error"),
        (true, "server fatal error"),
    ] {
        let (_, logs) = super::diagnostics::logs_of(|| {
            let (world, mut d, mut server) = at_session_start(scripted());
            world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
            d.step(2.0, true, &go);
            let error = crate::session::TransportErrorHeader {
                error_code: 0x0001_2345,
                error_subcode: 2,
            };
            let headers = if fatal {
                OptionalHeaders {
                    transport_fatal_error: Some(error),
                    ..OptionalHeaders::default()
                }
            } else {
                OptionalHeaders {
                    transport_error: Some(error),
                    ..OptionalHeaders::default()
                }
            };
            world.deliver(server.control(headers), WORLD_SERVER);
            d.step(3.0, true, &go);
            assert!(d.ended());
        });
        let filtered = lines_of_the_documented_filter(&logs);
        let all = filtered.join("\n");
        for needle in [
            "transport error from server",
            "error_code=0x00012345",
            "error_subcode=0x00000002",
            "official evidence: connection closed",
            "official evidence: connection totals",
            reason,
        ] {
            assert!(all.contains(needle), "{needle} not logged:\n{all}");
        }
        let code_at = paste_lines(&logs)
            .iter()
            .position(|l| l.contains("error_code=0x00012345"))
            .unwrap_or_else(|| panic!("the code is not in the paste:\n{logs}"));
        let closed_at = paste_lines(&logs)
            .iter()
            .position(|l| l.contains("official evidence: connection closed"))
            .expect("closed");
        assert!(code_at < closed_at, "{logs}");
        assert_nothing_private(&paste_lines(&logs));
    }
}

/// `docs/m8-remediation.md` §27 "Builder": the first batch, creation, position and placement after
/// `0x90` have a line each, once, in the order they came, with the time since that `0x90`, and no ID.
#[test]
fn the_first_of_each_kind_after_the_session_start_is_logged_in_order() {
    let (_, logs) = super::diagnostics::logs_of(|| {
        let (world, mut d, mut server) = at_session_start_with_schema();
        // Not after the `0x90` yet: no line for this one.
        world.deliver(
            server.on_queue(3, 1, 0, Some(PLAYER), 0xCE, &checked_message(0xCE)),
            WORLD_SERVER,
        );
        d.step(1.5, true, &go);
        world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
        d.step(2.0, true, &go);
        let batch = crate::world::SubEventBatch {
            event_id: crate::world::SubEventBatch::EVENT_ID_SUB_EVENT,
            frames: vec![
                crate::world::SubEventFrame::Event {
                    sub_event_id: 0x98,
                    payload: checked_message(0x98),
                },
                crate::world::SubEventFrame::Event {
                    sub_event_id: 0xCE,
                    payload: checked_message(0xCE),
                },
            ],
            entry_batch_tail_flag: None,
        }
        .to_bytes()
        .expect("batch");
        world.deliver(server.on_queue(2, 2, 2, None, 0x4C, &batch), WORLD_SERVER);
        d.step(2.25, true, &go);
        // Seconds later, and whose it is has no source: it has its line all the same.
        world.deliver(
            server.on_queue(3, 2, 0, None, 0xCC, &checked_message(0xCC)),
            WORLD_SERVER,
        );
        d.step(3.5, true, &go);
        // The second of each kind has none.
        world.deliver(
            server.on_queue(3, 3, 0, Some(PLAYER), 0xCC, &checked_message(0xCC)),
            WORLD_SERVER,
        );
        world.deliver(server.on_queue(2, 3, 3, None, 0x4C, &batch), WORLD_SERVER);
        d.step(4.0, true, &go);
    });
    let firsts: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("official evidence: first after the session start"))
        .collect();
    let order: Vec<&str> = firsts
        .iter()
        .map(|l| {
            ["batch", "creation", "position", "placement"]
                .into_iter()
                .find(|w| l.contains(&format!("first=\"{w}\"")))
                .unwrap_or_else(|| panic!("{l}"))
        })
        .collect();
    assert_eq!(
        order,
        ["batch", "creation", "position", "placement"],
        "{logs}"
    );
    assert!(firsts[0].contains("since_session_start_ms=250"), "{logs}");
    assert!(firsts[1].contains("in_batch=true"), "{logs}");
    assert!(firsts[3].contains("since_session_start_ms=1500"), "{logs}");
    assert!(firsts[3].contains("in_batch=false"), "{logs}");
    assert_nothing_private(&firsts);
}

/// The bounds of the evidence log: a chatty connection stays within them, however long it talks, and
/// what came just before its end is there. `tail=true` lines hold the last of it.
#[test]
fn the_evidence_lines_are_bounded_and_keep_the_end() {
    use crate::session::MAX_EVIDENCE_LINES_PER_CONNECTION;
    let (_, logs) = super::diagnostics::logs_of(|| {
        let (world, mut d, mut server) = at_session_start(production());
        let mut t = 1.5;
        for i in 0..2000u32 {
            world.deliver(server.event(0x4000 + (i % 100), &[0x00]), WORLD_SERVER);
            d.step(t, true, &go);
            t += 0.001;
        }
        // One opcode more, long past the budget of the receive lines.
        world.deliver(server.event(0x5555, &[0x00]), WORLD_SERVER);
        d.step(t, true, &go);
        d.begin_shutdown(t + 0.1);
    });
    let world_lines: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains(crate::session::EVIDENCE_TARGET) && l.contains("role=\"world\""))
        .collect();
    assert!(
        world_lines.len() <= MAX_EVIDENCE_LINES_PER_CONNECTION,
        "{} of at most {MAX_EVIDENCE_LINES_PER_CONNECTION}",
        world_lines.len()
    );
    let totals = world_lines
        .iter()
        .find(|l| l.contains("official evidence: connection totals"))
        .unwrap_or_else(|| panic!("no totals:\n{logs}"));
    assert!(totals.contains("datagrams="), "{totals}");
    assert!(totals.contains("disconnects=1"), "{totals}");
    assert!(!totals.contains("lines_not_shown=0"), "{totals}");
    assert!(
        world_lines
            .iter()
            .any(|l| l.contains("opcode=0x5555") && l.contains("first_of_opcode=true")),
        "the new opcode:\n{logs}"
    );
    let tail: Vec<&&str> = world_lines
        .iter()
        .filter(|l| l.contains("tail=true"))
        .collect();
    assert!(!tail.is_empty(), "{logs}");
    assert!(tail.len() <= crate::session::MAX_EVIDENCE_TAIL_LINES);
    // The goodbye has its own budget, so it is no tail line, and it is the last send before the end.
    assert!(
        world_lines
            .iter()
            .any(|l| l.contains("kind=\"disconnect\"") && l.contains("tail=false")),
        "{logs}"
    );
}

/// EV-8: whatever a hostile or broken server sends, the lines of the documented command stay bounded:
/// nothing at `info` and nothing on the evidence target grows with the messages.
#[test]
fn a_hostile_run_stays_within_the_documented_filter_bound() {
    use crate::session::MAX_EVIDENCE_LINES_PER_CONNECTION;
    let run = |rounds: u32| {
        super::diagnostics::logs_of(|| {
            let (world, mut d, mut server) = at_session_start(gated());
            world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
            world.deliver(
                server.event_to(0xCE, PLAYER, &service_support::player_position()),
                WORLD_SERVER,
            );
            d.step(2.0, true, &go);
            d.command(
                &Command::WorldLoaded {
                    generation: Some(1),
                },
                2.1,
            )
            .expect("loaded");
            world.deliver(server.event(0x7C, &[0x00]), WORLD_SERVER);
            d.step(2.2, true, &go);
            assert!(d.in_world());
            let mut t = 2.3;
            for i in 0..rounds {
                let hostile = [
                    // An envelope that does not decode, and bodies of each kind that do not read.
                    server.blob(0x4000 + (i % 7), &[0x80, 0, 0, 0]),
                    server.blob(crate::session::OPCODE_LOGIN_QUEUE_STATUS, &[0x00]),
                    server.event_to(0x40, PLAYER, &[1]),
                    server.event_to(0x98, PLAYER, &[1, 2, 3]),
                    server.event(0x4C, &[0xFF, 0xFF]),
                    server.event(0x0D, &[0xFF, 0xFF]),
                    server.event(0xCE, &[0x00, 0x01]),
                ];
                for datagram in hostile {
                    world.deliver(datagram, WORLD_SERVER);
                }
                // A datagram that does not qualify at all.
                world.deliver(vec![0, 0, 1, 2, 3], WORLD_SERVER);
                d.step(t, true, &go);
                t += 0.01;
                let _ = world.take_sent();
                let _ = d.take_events();
                assert!(d.in_world());
            }
        })
        .1
    };
    let short = run(200);
    let long = run(1200);
    let lines = lines_of_the_documented_filter(&long);
    // Two connections' evidence and what a session writes once, no more.
    let bound = 2 * MAX_EVIDENCE_LINES_PER_CONNECTION + 64;
    assert!(lines.len() <= bound, "{} of at most {bound}", lines.len());
    // The lines of one refused message are limited in time, not in number of messages.
    let refused = |logs: &str| {
        logs.lines()
            .filter(|l| l.contains("official session: message refused"))
            .count()
    };
    let (few, many) = (refused(&short), refused(&long));
    assert!((1..=4).contains(&few), "{few}");
    assert!(many <= few + 2, "{many} against {few}");
    assert!(
        long.lines()
            .any(|l| l.contains("official session: message refused") && l.contains("skipped=")),
        "{long}"
    );
}

/// The user's close while a placement is pending: exactly one Disconnect, a local close at once,
/// and no drain; the store is discarded.
#[test]
fn a_user_close_at_the_pending_placement_says_one_disconnect_and_is_done() {
    let (world, mut d, mut server) = at_session_start(production());
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    world.deliver(
        server.event_to(0xCE, PLAYER, &service_support::player_position()),
        WORLD_SERVER,
    );
    d.step(2.0, true, &go);
    world.take_sent();
    d.begin_shutdown(2.1);
    assert!(!d.shutdown_step(2.2), "no drain");
    assert_eq!(d.pending_sizes(), (0, 0));
    let closes = world
        .take_sent()
        .iter()
        .filter_map(|(p, dst)| {
            decode_client_datagram(p, false)
                .ok()
                .filter(|p| p.opt_headers.disconnect)
                .map(|_| *dst)
        })
        .collect::<Vec<_>>();
    assert_eq!(closes, [WORLD_SERVER]);
    let mut t = 2.3;
    while t < 30.0 {
        d.shutdown_step(t);
        t += 1.0;
    }
    assert!(world.take_sent().is_empty(), "no retransmitted Disconnect");
}

/// §9.3.9's first-screen contract: an invalid name, a repeated ID or name, or a list over its cap
/// fails the whole list; no name is cleaned into a valid one; an empty list is valid.
#[test]
fn the_first_list_is_all_valid_or_unsupported() {
    let list_of = character_list_of;
    let ok = [(1u64, "Aldo"), (2, "Bera")];
    for (entries, connects) in [
        (&ok[..], true),
        (&[][..], true),
        (&[(1, "")][..], false),
        (&[(1, "   ")][..], false),
        (&[(1, "\u{7}\u{7}")][..], false),
        (&[(1, "Aldo"), (1, "Bera")][..], false),
        (&[(1, "Aldo"), (2, "Aldo")][..], false),
        (&[(1, "Al\u{7}do")][..], false),
    ] {
        let (world, mut d, mut login) = super::initial_negotiation::connected();
        world.deliver(login.event(0x6C, &list_of(entries)), LOGIN_SERVER);
        d.step(0.5, true, &go);
        let events = d.take_events();
        let connected = events.iter().find_map(|e| match e {
            Event::Connected { characters } => Some(characters.clone()),
            _ => None,
        });
        assert_eq!(connected.is_some(), connects, "{entries:?} {events:?}");
        if connects {
            assert!(
                !events.iter().any(|e| matches!(e, Event::Character(_))),
                "{events:?}"
            );
            let names = connected.unwrap_or_default();
            assert_eq!(names.len(), entries.len());
            for c in &names {
                assert!(tessera_net::model::is_valid_name(&c.name), "{c:?}");
            }
        } else {
            assert!(
                events.iter().any(|e| matches!(
                    e,
                    Event::Disconnected {
                        reason: DisconnectReason::Unsupported
                    }
                )),
                "{entries:?} {events:?}"
            );
        }
    }
    // Over 64 UTF-8 bytes, and over the cap.
    let long = "x".repeat(65);
    let (world, mut d, mut login) = super::initial_negotiation::connected();
    world.deliver(
        login.event(0x6C, &list_of(&[(1, long.as_str())])),
        LOGIN_SERVER,
    );
    d.step(0.5, true, &go);
    assert!(d.ended());
    let names: Vec<String> = (0..257).map(|i| format!("Char{i}")).collect();
    let many: Vec<(u64, &str)> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (i as u64 + 1, n.as_str()))
        .collect();
    let (world, mut d, mut login) = super::initial_negotiation::connected();
    let frags = split_blob(1, 0, 9, 0, None, 0x6C, &list_of(&many)).expect("split");
    for part in frags.chunks(20) {
        login.seq += 1;
        let mask = login.isaac.next_mask();
        world.deliver(
            encode_datagram(
                DatagramHeader {
                    rec_id: login.rec_id,
                    seq_num: login.seq,
                    ..DatagramHeader::default()
                },
                &OptionalHeaders::default(),
                part,
                Some(mask),
                None,
            )
            .expect("datagram"),
            LOGIN_SERVER,
        );
    }
    d.step(0.5, true, &go);
    let events = d.take_events();
    assert!(d.ended());
    assert!(
        !events.iter().any(|e| matches!(e, Event::Connected { .. })),
        "over the cap, no partial list"
    );
}

/// §9.3.9: every later `0x6C` is counted and skipped in every phase, valid or not, and the first list
/// and its IDs stay.
#[test]
fn every_later_list_is_counted_and_skipped() {
    let (world, mut d, mut login) = driver_at_characters(production(), 0);
    world.deliver(login.event(0x6C, &[0x00, 0x01]), LOGIN_SERVER);
    d.step(0.5, true, &go);
    assert!(!d.ended());
    assert_eq!(d.counters().later_lists_skipped, 1);
    assert!(d.take_events().is_empty());
    world.deliver(
        login.event(0x6C, &character_list_of(&[(5, "Cora")])),
        LOGIN_SERVER,
    );
    d.step(0.6, true, &go);
    assert_eq!(d.counters().later_lists_skipped, 2);
    assert!(d.take_events().is_empty());
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        0.7,
    )
    .expect("enter");
    d.step(0.8, true, &go);
    world.deliver(
        login.event(0x6C, &character_list_of(&[(6, "Dara")])),
        LOGIN_SERVER,
    );
    d.step(0.9, true, &go);
    assert_eq!(d.counters().later_lists_skipped, 3);
    assert!(!d.ended());
}

/// §9.3.9's notices: in every phase plain text, and past 256 bytes dropped, not cut.
#[test]
fn notices_are_shown_in_every_phase_and_never_cut() {
    let notification = |text: &str| {
        let mut body = Vec::new();
        crate::session::encode_variable_u32(
            u32::try_from(text.encode_utf16().count()).expect("small"),
            &mut body,
        )
        .expect("count");
        for u in text.encode_utf16() {
            body.extend_from_slice(&u.to_le_bytes());
        }
        encode_event_envelope(&body)
    };
    let (world, mut d, mut login) = super::initial_negotiation::connected();
    world.deliver(login.blob(0x12E, &notification("first")), LOGIN_SERVER);
    world.deliver(
        login.blob(0x12E, &notification(&"x".repeat(257))),
        LOGIN_SERVER,
    );
    d.step(0.5, true, &go);
    assert!(d.take_events().iter().any(|e| matches!(
        e,
        Event::Account(AccountEvent::LoginNotice { text }) if text == "first"
    )));
    assert_eq!(d.counters().notices_dropped, 1);
    assert!(!d.ended());
    let (world, mut d, mut server) = at_session_start(production());
    world.deliver(server.blob(0x12E, &notification("second")), WORLD_SERVER);
    d.step(1.5, true, &go);
    assert!(d.take_events().iter().any(|e| matches!(
        e,
        Event::Account(AccountEvent::LoginNotice { text }) if text == "second"
    )));
}

/// §9.3.9, `docs/m8-remediation.md` §27 fix 9: a checksum-valid challenge with a zero header recipient
/// or zero payload bytes 18-19 is unsupported at once, retries stop, and no reply. The header's opaque
/// peer word is not tested: it goes through like any other.
#[test]
fn a_zero_recipient_challenge_ends_the_attempt_with_no_reply() {
    let (world, mut d) = driver_at_login(Service::default(), 0);
    world.take_sent();
    world.deliver(
        challenge_datagram(0, 0x0222, 0x3344, 1, 0xCAFE_1234, INCOMING_UUID),
        LOGIN_SERVER,
    );
    d.step(0.1, true, &go);
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
    let mut t = 0.2;
    while t < 25.0 {
        d.step(t, true, &go);
        t += 1.0;
    }
    assert!(world.take_sent().is_empty(), "no ticket, no accept");

    let (world, mut d) = driver_at_login(Service::default(), 0);
    world.take_sent();
    world.deliver(
        challenge_datagram(0x0111, 0, 0x3344, 1, 0xCAFE_1234, INCOMING_UUID),
        LOGIN_SERVER,
    );
    d.step(0.1, true, &go);
    assert!(d.ended(), "zero payload bytes 18-19");
}

#[test]
fn a_zero_peer_word_challenge_is_answered_like_any_other() {
    let (world, mut d) = driver_at_login(Service::default(), 0);
    world.take_sent();
    world.deliver(
        challenge_datagram(0x0111, 0x0222, 0, 1, 0xCAFE_1234, INCOMING_UUID),
        LOGIN_SERVER,
    );
    d.step(0.1, true, &go);
    assert!(!d.ended());
    assert!(!world.take_sent().is_empty(), "the accept");
}

/// [`at_session_start`] on an install with a `0xCC` schema.
fn at_session_start_with_schema() -> (World, SessionDriver<FakeNet>, Server) {
    super::world_entry::at_session_start_with_schema()
}

/// A valid complete message of `opcode`.
fn checked_message(opcode: u32) -> Vec<u8> {
    super::world_entry::checked_message(opcode)
}

/// How many Disconnects each of `to` got, oldest first.
fn disconnects(world: &World, to: &[std::net::SocketAddrV4]) -> Vec<usize> {
    let sent = world.take_sent();
    to.iter()
        .map(|peer| {
            sent.iter()
                .filter(|(bytes, dst)| {
                    dst == peer
                        && decode_client_datagram(bytes, false)
                            .is_ok_and(|p| p.opt_headers.disconnect)
                })
                .count()
        })
        .collect()
}

/// A selection with its referral taken, the world connection in its handshake.
fn at_referral(service: Service) -> (World, SessionDriver<FakeNet>) {
    let (world, mut d, login) = driver_at_characters(service, 0);
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    d.step(1.1, true, &go);
    world.deliver(
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
        LOGIN_SERVER,
    );
    d.step(1.2, true, &go);
    assert!(d.entry_progress().referred);
    world.take_sent();
    let _ = d.take_events();
    (world, d)
}

/// §9.3.9 "Referral endpoint validity": a zero address or port for the pending selection is required
/// progress: one close on the login connection, no world connection and nothing sent to it. With no
/// selection pending it is one more stray referral.
#[test]
fn a_referral_with_no_endpoint_is_one_close_and_no_world_connection() {
    let zero_port = std::net::SocketAddrV4::new(*WORLD_SERVER.ip(), 0);
    let zero_ip = std::net::SocketAddrV4::new(std::net::Ipv4Addr::UNSPECIFIED, WORLD_SERVER.port());
    for target in [zero_port, zero_ip] {
        let (world, mut d, login) = driver_at_characters(production(), 0);
        d.command(
            &Command::EnterWorld {
                character: "Aldo".to_owned(),
            },
            1.0,
        )
        .expect("enter");
        d.step(1.1, true, &go);
        world.take_sent();
        world.deliver(login.referral(7, target), LOGIN_SERVER);
        d.step(1.2, true, &go);
        assert!(d.ended());
        assert!(d.take_events().iter().any(|e| matches!(
            e,
            Event::Disconnected {
                reason: DisconnectReason::Unsupported
            }
        )));
        assert_eq!(d.connection_states().1, None, "no world connection");
        let sent = world.take_sent();
        assert!(sent.iter().all(|(_, dst)| *dst == LOGIN_SERVER), "{sent:?}");
        assert_eq!(
            sent.iter()
                .filter(|(b, _)| decode_client_datagram(b, false)
                    .is_ok_and(|p| p.opt_headers.disconnect))
                .count(),
            1
        );
        // No selection pending: counted.
        let (world, mut d, login) = driver_at_characters(production(), 0);
        world.deliver(login.referral(7, target), LOGIN_SERVER);
        d.step(0.5, true, &go);
        assert!(!d.ended());
        assert_eq!(d.counters().stray_referrals, 1);
    }
}

/// §9.3.9's close rows: an established connection's silence sends exactly one Disconnect.
#[test]
fn established_silence_sends_exactly_one_disconnect() {
    let (world, mut d, _login) = driver_at_characters(production(), 0);
    world.take_sent();
    d.step(61.5, true, &go);
    d.step(61.6, true, &go);
    assert!(d.take_events().iter().any(|e| matches!(
        e,
        Event::Disconnected {
            reason: DisconnectReason::TimedOut
        }
    )));
    assert_eq!(disconnects(&world, &[LOGIN_SERVER]), [1]);
}

/// §9.3.9: only the primary connection gets a Disconnect: a local terminal end while the world is in
/// its handshake sends none at all, to the retiring login connection or the world.
#[test]
fn a_terminal_end_in_the_world_handshake_says_nothing() {
    let (world, mut d) = at_referral(production());
    d.step(21.5, true, &go);
    d.step(21.6, true, &go);
    assert!(d.take_events().iter().any(|e| matches!(
        e,
        Event::Disconnected {
            reason: DisconnectReason::TimedOut
        }
    )));
    assert_eq!(disconnects(&world, &[LOGIN_SERVER, WORLD_SERVER]), [0, 0]);
}

/// §9.3.9's Cancel row: one on the established login connection, none on the world's, and its auth
/// retries end.
#[test]
fn a_user_close_in_the_world_handshake_says_one_on_login_and_none_on_world() {
    let (world, mut d) = at_referral(production());
    d.begin_shutdown(1.3);
    assert!(!d.shutdown_step(1.4), "no drain");
    assert_eq!(disconnects(&world, &[LOGIN_SERVER, WORLD_SERVER]), [1, 0]);
    let mut t = 2.0;
    while t < 30.0 {
        d.shutdown_step(t);
        t += 0.5;
    }
    assert!(world.take_sent().is_empty(), "no auth retry");
}

/// §9.3.9: past the world's State 5 the login connection is retired: a user's close says one on the
/// world and none on login.
#[test]
fn a_user_close_after_the_world_state_5_says_one_on_world_only() {
    let (world, mut d, _server) = at_session_start(production());
    d.begin_shutdown(1.4);
    assert_eq!(disconnects(&world, &[LOGIN_SERVER, WORLD_SERVER]), [0, 1]);
}

/// §9.3.9: a typed logout is the same close: `ClientRequested`, one Disconnect, no RPC and no drain.
#[test]
fn a_typed_logout_closes_with_one_disconnect_and_no_rpc() {
    let (world, mut d, mut server) = at_session_start(gated());
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    world.deliver(
        server.event_to(0xCE, PLAYER, &service_support::player_position()),
        WORLD_SERVER,
    );
    d.step(2.0, true, &go);
    d.command(
        &Command::WorldLoaded {
            generation: Some(1),
        },
        2.1,
    )
    .expect("loaded");
    world.deliver(server.event(0x7C, &[0x00]), WORLD_SERVER);
    d.step(2.2, true, &go);
    assert!(d.in_world());
    let _ = d.take_events();
    world.take_sent();
    d.command(&Command::LeaveWorld, 2.3).expect("closes");
    assert!(d.ended());
    assert!(d.take_events().iter().any(|e| matches!(
        e,
        Event::Disconnected {
            reason: DisconnectReason::ClientRequested
        }
    )));
    let sent = world.take_sent();
    assert!(
        !sent
            .iter()
            .filter_map(|(p, _)| decode_client_datagram(p, false).ok())
            .flat_map(|p| p.fragments)
            .any(|f| f.header.blob_header_word == Some(0x30)),
        "no logout RPC"
    );
    assert_eq!(
        sent.iter()
            .filter(
                |(p, _)| decode_client_datagram(p, false).is_ok_and(|p| p.opt_headers.disconnect)
            )
            .count(),
        1
    );
    // At the list there is nothing to log out of.
    let (_world, mut d, _login) = driver_at_characters(production(), 0);
    assert!(matches!(
        d.command(&Command::LeaveWorld, 0.5),
        Err(tessera_net::NetError::InvalidCommand(_))
    ));
    assert!(!d.ended());
}

/// §9.3.11: the first `0x90`'s ID may be neither zero nor logical 2, either is required progress.
#[test]
fn a_first_session_start_with_no_id_is_unsupported() {
    for id in [0u64, DEFAULT_RECIPIENT] {
        let (world, mut d, mut server) = at_session_start(production());
        world.deliver(server.event(0x90, &ready_of(id)), WORLD_SERVER);
        d.step(2.0, true, &go);
        assert!(d.ended(), "{id}");
        assert!(d.take_events().iter().any(|e| matches!(
            e,
            Event::Disconnected {
                reason: DisconnectReason::Unsupported
            }
        )));
        assert_eq!(disconnects(&world, &[WORLD_SERVER]), [1]);
    }
}

/// §9.3.11: an exact repeat counts and skips, a later different ID is unsupported, no silent rebinding,
/// and a later one with the same ID and another table leaves the first (labelled, §28 fix 20).
#[test]
fn a_later_session_start_repeats_or_ends_the_entry() {
    let (world, mut d, mut server) = at_session_start(production());
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    d.step(2.0, true, &go);
    let _ = d.take_events();
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    d.step(2.1, true, &go);
    assert!(!d.ended());
    assert_eq!(d.counters().session_starts_repeated, 1);
    assert!(d.take_events().is_empty(), "no second stage");
    for other in [PLAYER + 1, 0, DEFAULT_RECIPIENT] {
        let (world, mut d, mut server) = at_session_start(production());
        world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
        d.step(2.0, true, &go);
        world.deliver(server.event(0x90, &ready_of(other)), WORLD_SERVER);
        d.step(2.1, true, &go);
        assert!(d.ended(), "{other}");
        assert!(d.take_events().iter().any(|e| matches!(
            e,
            Event::Disconnected {
                reason: DisconnectReason::Unsupported
            }
        )));
    }
}

/// `official-session.md` §9.3.11's scripted-backend case, with the local gate first. A test's backend
/// stands in for the authoritative placement and the readiness: this tests the interface, not an
/// official receive trigger.
#[test]
fn the_scripted_entry_runs_in_the_specified_order() {
    let loaded = |g| Command::WorldLoaded {
        generation: Some(g),
    };
    let (world, mut d, mut server) = at_session_start(gated());
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    d.step(2.0, true, &go);
    // A `0x90` is identity only.
    let events = d.take_events();
    assert_eq!(progress_stages(&events), [WorldEntryStage::Identity]);
    assert!(!events.iter().any(|e| matches!(
        e,
        Event::WorldPlacementPending { .. } | Event::WorldEntered { .. }
    )));
    // Before the placement: refused.
    assert!(d.command(&loaded(1), 2.05).is_err());
    // The placement: generation 1, and the session stays entering.
    let at = service_support::player_position();
    world.deliver(server.event_to(0xCE, PLAYER, &at), WORLD_SERVER);
    d.step(2.1, true, &go);
    assert!(
        d.take_events()
            .iter()
            .any(|e| matches!(e, Event::WorldPlacementPending { generation: 1, .. }))
    );
    assert!(!d.in_world());
    // The same placement's motion makes no generation.
    world.deliver(server.event_to(0xCE, PLAYER, &at), WORLD_SERVER);
    d.step(2.15, true, &go);
    assert!(
        !d.take_events()
            .iter()
            .any(|e| matches!(e, Event::WorldPlacementPending { .. }))
    );
    // A later sourced placement invalidates a load of the old one.
    d.command(&loaded(1), 2.2).expect("loaded");
    let (location, yaw) = (
        tessera_net::Location::new(tessera_net::RegionId(1), 5.0, 6.0, 7.0),
        0.25,
    );
    d.scripted_new_placement_for_tests(location, yaw, 2.3);
    assert!(
        d.take_events()
            .iter()
            .any(|e| matches!(e, Event::WorldPlacementPending { generation: 2, .. }))
    );
    assert!(!d.entry_progress().loaded, "the old completion is gone");
    assert!(d.command(&loaded(1), 2.31).is_err(), "stale");
    assert!(
        d.command(&Command::WorldLoaded { generation: None }, 2.32)
            .is_err()
    );
    d.command(&loaded(2), 2.33).expect("the new generation");
    assert!(d.command(&loaded(2), 2.34).is_err(), "duplicate");
    assert!(!d.in_world(), "the local gate alone does not enter");
    assert!(!sent_opcodes(&world).contains(&0xF4));
    // Server readiness: one retained `0xF4`, then `WorldEntered`, then the release.
    world.deliver(server.event(0x7C, &[0x00]), WORLD_SERVER);
    d.step(2.4, true, &go);
    assert!(d.in_world());
    assert_eq!(
        sent_opcodes(&world).iter().filter(|o| **o == 0xF4).count(),
        1
    );
    assert!(
        d.take_events()
            .iter()
            .any(|e| matches!(e, Event::WorldEntered { .. }))
    );
}

/// A `0x90` whose table says the world clock's offset is `offset`.
fn ready_with_clock(player: u64, offset: f32) -> Vec<u8> {
    PlayerSessionReady {
        player_id: player,
        session_props: EncodedPropertyTable {
            entries: vec![EncodedPropertyEntry::new(
                PID_WORLD_TIME_OFFSET,
                EncodedPropertyValue::Float(offset),
            )],
        },
    }
    .to_bytes()
    .expect("0x90")
}

/// [`at_session_start`] on an install whose dictionary has the `0x90` table's property.
fn at_session_start_with_clock(service: Service) -> (World, SessionDriver<FakeNet>, Server) {
    let (world, mut d, login) =
        driver_at_characters_from(service, super::world_entry::with_schema());
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    d.step(1.1, true, &go);
    world.deliver(
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
        LOGIN_SERVER,
    );
    d.step(1.2, true, &go);
    let server = Server::new(0x0222, 0xBEEF_0002);
    server.connect_to(&world, 0xBEEF_0002, WORLD_SERVER);
    d.step(1.3, true, &go);
    world.take_sent();
    let _ = d.take_events();
    (world, d, server)
}

/// §9.3.9 "keep its identity/properties pending" on the scripted road: a non-empty `0x90` table stays
/// pending through the placement and both gates, a repeat with another table leaves the first
/// (labelled, §28 fix 20), and the release has it. Nothing reads it before `WorldEntered`.
#[test]
fn the_first_session_table_stays_pending_until_the_release() {
    let (world, mut d, mut server) = at_session_start_with_clock(gated());
    world.deliver(
        server.event(0x90, &ready_with_clock(PLAYER, 7.5)),
        WORLD_SERVER,
    );
    d.step(2.0, true, &go);
    assert!(d.entry_progress().session_started);
    assert_eq!(d.session_property(PID_WORLD_TIME_OFFSET), None, "pending");
    world.deliver(
        server.event(0x90, &ready_with_clock(PLAYER, 9.0)),
        WORLD_SERVER,
    );
    world.deliver(
        server.event_to(0xCE, PLAYER, &service_support::player_position()),
        WORLD_SERVER,
    );
    d.step(2.1, true, &go);
    assert_eq!(d.counters().session_starts_repeated, 1);
    d.command(
        &Command::WorldLoaded {
            generation: Some(1),
        },
        2.2,
    )
    .expect("loaded");
    assert_eq!(d.session_property(PID_WORLD_TIME_OFFSET), None, "pending");
    world.deliver(server.event(0x7C, &[0x00]), WORLD_SERVER);
    d.step(2.3, true, &go);
    assert!(d.in_world());
    assert_eq!(
        d.session_property(PID_WORLD_TIME_OFFSET),
        Some(&EncodedPropertyValue::Float(7.5)),
        "the first table, released"
    );
    assert_eq!(d.pending_sizes(), (0, 0));
}

/// §9.3.9: "discard everything on failure": an entry that stops releases no table.
#[test]
fn a_failed_entry_discards_the_session_table() {
    let (world, mut d, mut server) = at_session_start_with_clock(production());
    world.deliver(
        server.event(0x90, &ready_with_clock(PLAYER, 7.5)),
        WORLD_SERVER,
    );
    d.step(2.0, true, &go);
    assert!(d.entry_progress().session_started);
    d.step(
        2.0 + crate::connector::session_phase::ENTER_WORLD_TIMEOUT_SEC,
        true,
        &go,
    );
    assert!(d.ended());
    assert_eq!(d.session_property(PID_WORLD_TIME_OFFSET), None);
}

/// The reasons of the `Disconnected` events in `events`.
fn disconnected(events: &[Event]) -> Vec<&DisconnectReason> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Disconnected { reason } => Some(reason),
            _ => None,
        })
        .collect()
}

/// Runs `d` from `from` to `to` in half seconds.
fn run(d: &mut SessionDriver<FakeNet>, from: f64, to: f64) {
    let mut t = from;
    while t < to {
        d.step(t, true, &go);
        t += 0.5;
    }
}

/// §9.3.9's I/O row after `Connected`: a send the socket refuses is `TransportFailed` with no refusal
/// and no Disconnect, and nothing more is tried, whether the socket stays dead or heals.
#[test]
fn a_failed_send_in_the_session_is_the_io_row() {
    let (world, mut d, _login) = driver_at_characters(production(), 0);
    world.take_sent();
    let _ = d.take_events();
    world.fail_sends(SendFailure::Hard);
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    d.step(1.1, true, &go);
    assert!(d.ended());
    let events = d.take_events();
    assert!(
        matches!(
            disconnected(&events)[..],
            [DisconnectReason::TransportFailed(_)]
        ),
        "{events:?}"
    );
    assert!(!events.iter().any(|e| matches!(e, Event::Account(_))));
    let attempts = world.failed_sends().len();
    assert_eq!(attempts, 1, "the first refused send ends it");
    world.heal_sends();
    run(&mut d, 1.2, 70.0);
    assert!(world.take_sent().is_empty(), "no Disconnect and no retry");
    assert_eq!(world.failed_sends().len(), attempts);
}

/// §9.3.9's I/O row before `Connected`: `LoginRefused { ServiceUnavailable }`, then `TransportFailed`,
/// and no further send of the ticket. Not the silence row's words after 20 s.
#[test]
fn a_failed_send_in_the_handshake_refuses_and_stops_the_ticket() {
    let (world, mut d) = driver_at_login(Service::default(), 0);
    world.take_sent();
    world.fail_sends(SendFailure::Hard);
    run(&mut d, 0.1, 5.0);
    assert!(d.ended(), "well before the handshake's 20 s");
    let events = d.take_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Account(AccountEvent::LoginRefused {
                reason: LoginRefusal::ServiceUnavailable,
                ..
            })
        )),
        "{events:?}"
    );
    assert!(
        matches!(
            disconnected(&events)[..],
            [DisconnectReason::TransportFailed(_)]
        ),
        "{events:?}"
    );
    let attempts = world.failed_sends().len();
    assert_eq!(attempts, 1, "the auth's resend, once");
    run(&mut d, 5.0, 30.0);
    world.heal_sends();
    run(&mut d, 30.0, 40.0);
    assert_eq!(world.failed_sends().len(), attempts);
    assert!(world.take_sent().is_empty(), "no ticket again");
}

/// A socket that is full just now is a count (labelled, engineering choice): the session goes on and
/// its own clocks decide, and it can recover.
#[test]
fn a_full_socket_is_only_counted() {
    let (world, mut d, mut login) = driver_at_characters(production(), 0);
    world.take_sent();
    world.fail_sends(SendFailure::Transient);
    world.deliver(login.event(0x149, &[0x00, 0x7F]), LOGIN_SERVER);
    run(&mut d, 1.0, 10.0);
    assert!(!d.ended());
    assert!(!world.failed_sends().is_empty());
    world.heal_sends();
    world.deliver(login.event(0x149, &[0x00, 0x7F]), LOGIN_SERVER);
    run(&mut d, 10.0, 20.0);
    assert!(!d.ended());
    assert!(!world.take_sent().is_empty(), "it sends again");
}

/// §9.3.9: nothing the retiring login connection does decides the session: a dead route to it is only
/// a count, and the same on the world connection, the primary one by then, is the I/O row, with no
/// Disconnect to either.
#[test]
fn only_the_primary_connections_failed_send_ends_the_session() {
    let (world, mut d, mut login) = driver_at_characters(production(), 0);
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        1.0,
    )
    .expect("enter");
    d.step(1.1, true, &go);
    world.deliver(
        login.referral(0x1122_3344_5566_7788, WORLD_SERVER),
        LOGIN_SERVER,
    );
    d.step(1.2, true, &go);
    assert!(d.entry_progress().referred);
    let _ = d.take_events();
    world.take_sent();
    world.fail_sends(SendFailure::HardTo(LOGIN_SERVER));
    world.deliver(login.event(0x149, &[0x00, 0x7F]), LOGIN_SERVER);
    run(&mut d, 1.3, 4.0);
    assert!(
        world.failed_sends().contains(&LOGIN_SERVER),
        "the retiring connection tried"
    );
    assert!(!d.ended(), "and that decides nothing");

    world.fail_sends(SendFailure::HardTo(WORLD_SERVER));
    world.take_sent();
    run(&mut d, 4.0, 10.0);
    assert!(d.ended());
    let events = d.take_events();
    assert!(
        matches!(
            disconnected(&events)[..],
            [DisconnectReason::TransportFailed(_)]
        ),
        "{events:?}"
    );
    assert_eq!(disconnects(&world, &[LOGIN_SERVER, WORLD_SERVER]), [0, 0]);
    assert_eq!(world.failed_sends().last(), Some(&WORLD_SERVER));
}

/// The same with the server's gate first: the result does not depend on the gates' order, and no
/// `WorldEntered` comes before the client's load.
#[test]
fn the_scripted_entry_takes_the_gates_in_either_order() {
    let (world, mut d, mut server) = at_session_start(gated());
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    world.deliver(server.event(0x7C, &[0x00]), WORLD_SERVER);
    world.deliver(
        server.event_to(0xCE, PLAYER, &service_support::player_position()),
        WORLD_SERVER,
    );
    d.step(2.0, true, &go);
    assert!(!d.in_world(), "the server gate alone does not enter");
    assert!(!sent_opcodes(&world).contains(&0xF4));
    d.command(
        &Command::WorldLoaded {
            generation: Some(1),
        },
        2.1,
    )
    .expect("loaded");
    d.step(2.2, true, &go);
    assert!(d.in_world());
    assert_eq!(
        sent_opcodes(&world).iter().filter(|o| **o == 0xF4).count(),
        1
    );
}

/// §9.3.9 (labelled, §28 fix 28): an inbound `OutOfOrderAck` names the server's own sequences, so one
/// whose number happens to be the `0xF4`'s does not end its retention.
#[test]
fn a_colliding_inbound_out_of_order_ack_keeps_the_ready_message() {
    let (world, mut d, mut server) = at_session_start(gated());
    world.deliver(server.event(0x90, &ready_of(PLAYER)), WORLD_SERVER);
    world.deliver(
        server.event_to(0xCE, PLAYER, &service_support::player_position()),
        WORLD_SERVER,
    );
    d.step(2.0, true, &go);
    d.command(
        &Command::WorldLoaded {
            generation: Some(1),
        },
        2.1,
    )
    .expect("loaded");
    world.take_sent();
    world.deliver(server.event(0x7C, &[0x00]), WORLD_SERVER);
    d.step(2.2, true, &go);
    let ready = world
        .take_sent()
        .into_iter()
        .find_map(|(bytes, _)| {
            let packet = decode_client_datagram(&bytes, false).ok()?;
            packet
                .fragments
                .iter()
                .any(|f| f.header.blob_header_word == Some(0xF4))
                .then_some((bytes, packet.header.seq_num))
        })
        .expect("the one 0xF4");
    world.deliver(
        server.control(OptionalHeaders {
            out_of_order_ack: Some(vec![ready.1]),
            ..OptionalHeaders::default()
        }),
        WORLD_SERVER,
    );
    d.step(2.25, true, &go);
    world.deliver(
        server.control(OptionalHeaders {
            sequence_nak: Some(vec![ready.1]),
            ..OptionalHeaders::default()
        }),
        WORLD_SERVER,
    );
    d.step(2.3, true, &go);
    assert_eq!(
        world
            .take_sent()
            .into_iter()
            .filter(|(bytes, _)| *bytes == ready.0)
            .count(),
        1,
        "the same bytes"
    );
}

/// OE-T20's lone surrogate: the first list fails whole. The reader refuses the name, so nothing can
/// clean it into a valid one.
#[test]
fn a_lone_surrogate_in_the_first_list_fails_it_whole() {
    let mut list = character_list_of(&[(1, "Aldo"), (2, "Bera")]);
    let name: Vec<u8> = "Aldo".encode_utf16().flat_map(u16::to_le_bytes).collect();
    let at = list
        .windows(name.len())
        .position(|w| w == name)
        .expect("the name");
    list.splice(at + 2..at + 4, [0x00, 0xD8]);
    let (world, mut d, mut login) = super::initial_negotiation::connected();
    world.deliver(login.event(0x6C, &list), LOGIN_SERVER);
    d.step(0.5, true, &go);
    let events = d.take_events();
    assert!(d.ended());
    assert!(!events.iter().any(|e| matches!(e, Event::Connected { .. })));
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Disconnected {
            reason: DisconnectReason::Unsupported
        }
    )));
}
