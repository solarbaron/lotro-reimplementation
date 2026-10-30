// SPDX-License-Identifier: GPL-3.0-or-later
//! The session's first datagrams and its handshake as the driver runs them
//! (`official-session.md` §3.2, §5.6.2, §6.2–§6.4, §9.3) on the synthetic network and a
//! virtual clock: the bind plan, the `ConnectAcceptHeader` rate, the delivery queues of the
//! client's messages and the `0x130` outcomes.

use super::*;
use crate::login::NetPreferences;
use crate::session::{
    BindPlan, BindPorts, LOGON_RESULT_QUEUED, LoginQueuePosition, decode_datagram,
};

/// The bind plans the synthetic network was asked for.
fn plans(world: &World) -> Vec<BindPlan> {
    world.shared.lock().expect("shared").bind_plans.clone()
}

/// A driver started from `start` (its first datagram queued).
fn started(start: SessionStart) -> (World, SessionDriver<FakeNet>) {
    let world = new_world(Service::default());
    let mut net = FakeNet(world.clone());
    let d = SessionDriver::start(&mut net, start, 0.0, &go).expect("start");
    (world, d)
}

#[test]
fn the_socket_binds_the_wildcard_with_case_b_by_default() {
    let (world, d) = started(session_start(0, None, Arc::default()));
    // §3.2: no interface selection binds 0.0.0.0; the first local address seeds Case B.
    assert_eq!(
        plans(&world),
        vec![BindPlan {
            address: Ipv4Addr::UNSPECIFIED,
            ports: BindPorts::Unique {
                seed_ip: *LOCAL.ip()
            },
        }]
    );
    assert_eq!(d.first_local_ipv4(), Some(*LOCAL.ip()));
    // The first datagram is the §6.2 one: `seqNum` 1, `time` 0, the primary mode.
    let sent = world.take_sent();
    let (first, to) = sent.first().expect("first datagram");
    assert_eq!(*to, LOGIN_SERVER);
    let pkt = decode_datagram(first, false, None, true).expect("decodes");
    assert_eq!(
        (pkt.header.rec_id, pkt.header.seq_num, pkt.header.time),
        (0, 1, 0)
    );
    let auth = pkt.opt_headers.session_auth.expect("SessionAuthHeader");
    assert_eq!(
        auth.net_version,
        "180804_netver:9999; didver:00000000-0000-0000-0000-000000000000"
    );
    assert_eq!((auth.auth_type, auth.auth_flags), (0x0000_0001, 0x04));
    assert_eq!(auth.credentials.ticket.as_deref(), Some(TICKET));
}

#[test]
fn the_net_preferences_and_the_launch_values_choose_the_bind() {
    // `Net.BindInterface` selects the address; `Net.UserSpecifiedPort` is Case A.
    let mut start = session_start(0, None, Arc::default());
    start.net = NetPreferences {
        bind_interface: "192.0.2.77".to_owned(),
        user_specified_port: 4100,
        ..NetPreferences::default()
    };
    let (world, _d) = started(start);
    assert_eq!(
        plans(&world),
        vec![BindPlan {
            address: Ipv4Addr::new(192, 0, 2, 77),
            ports: BindPorts::Explicit(4100),
        }]
    );

    // A non-zero `--outport` overrides `Net.UserSpecifiedPort`, and the two-colon `-h`
    // form's `localHost` overrides `Net.BindInterface`.
    let mut start = session_start(0, None, Arc::default());
    let version = start.handoff.version_info.clone();
    start.handoff = SessionHandoff::new(
        "192.0.2.5:login.example.invalid:9001",
        5200,
        "SUB0000000000000000000000",
        TICKET,
        version,
        "English",
        "",
        "https://auth.example.invalid/login",
        0,
    )
    .expect("handoff");
    start.net = NetPreferences {
        bind_interface: "192.0.2.77".to_owned(),
        user_specified_port: 4100,
        ..NetPreferences::default()
    };
    let (world, _d) = started(start);
    assert_eq!(
        plans(&world),
        vec![BindPlan {
            address: Ipv4Addr::new(192, 0, 2, 5),
            ports: BindPorts::Explicit(5200),
        }]
    );

    // `Net.ComputeUniquePort = 0`: Case C.
    let mut start = session_start(0, None, Arc::default());
    start.net = NetPreferences {
        compute_unique_port: false,
        ..NetPreferences::default()
    };
    let (world, _d) = started(start);
    assert_eq!(
        plans(&world),
        vec![BindPlan {
            address: Ipv4Addr::UNSPECIFIED,
            ports: BindPorts::Ephemeral,
        }]
    );
}

/// The `ConnectAcceptHeader` the driver sends after `login`'s challenge.
fn accept_after_challenge(start: SessionStart) -> crate::session::ConnectAcceptHeader {
    let (world, mut d) = started(start);
    world.take_sent();
    let login = Server::new(0x0111, 0xBEEF_0001);
    world.deliver(login.challenge(0xBEEF_0001), LOGIN_SERVER);
    d.step(0.1, true, &go);
    let sent = world.take_sent();
    let (accept, _) = sent.first().expect("accept");
    let pkt = decode_datagram(accept, false, None, true).expect("decodes");
    // §6.4 step 4: `recId` is the server's, `seqNum` 1, `time` 0.
    assert_eq!(
        (pkt.header.rec_id, pkt.header.seq_num, pkt.header.time),
        (0x0222, 1, 0)
    );
    assert_eq!(d.connection_ids().0, 0x0111);
    pkt.opt_headers.connect_accept.expect("ConnectAcceptHeader")
}

#[test]
fn the_accept_carries_the_connection_speed_rate() {
    // The default `0.00`: `maxRecvRate` 0.
    let accept = accept_after_challenge(session_start(0, None, Arc::default()));
    assert_eq!(accept.max_recv_rate, 0);
    // `Net.ConnectionSpeed = 1000`: floor(1000 × 102.4).
    let mut start = session_start(0, None, Arc::default());
    start.net.connection_speed_kbps = 1000.0;
    let accept = accept_after_challenge(start);
    assert_eq!(accept.max_recv_rate, 102_400);
}

#[test]
fn the_client_messages_go_on_the_queues_of_the_outbound_table() {
    let (world, mut d, mut login) = driver_at_characters(Service::default(), 0);
    world.take_sent();
    // 0x12C answers 0x12F on queue 4 (ReliableUnordered).
    world.deliver(login.event(0x12F, &[]), LOGIN_SERVER);
    d.step(0.2, true, &go);
    // 0x6F on queue 3, sequence group 0x13.
    d.command(
        &Command::EnterWorld {
            character: "Aldo".to_owned(),
        },
        0.3,
    )
    .expect("enter");
    d.step(0.4, true, &go);
    let frags: Vec<_> = world
        .take_sent()
        .iter()
        .filter_map(|(d, _)| decode_datagram(d, false, None, false).ok())
        .flat_map(|p| p.fragments)
        .map(|f| f.header)
        .collect();
    let version = frags
        .iter()
        .find(|h| h.blob_header_word == Some(0x12C))
        .expect("0x12C");
    assert_eq!(version.queue_id, 4);
    let select = frags
        .iter()
        .find(|h| h.blob_header_word == Some(0x6F))
        .expect("0x6F");
    assert_eq!((select.queue_id, select.seq_group), (3, 0x13));
}

#[test]
fn the_outbound_table_and_the_movement_rule() {
    use super::super::session_phase::{movement_is_discrete, outbound_queue};
    assert_eq!(outbound_queue(0x12C, &[]), (4, 0));
    assert_eq!(outbound_queue(0x19E, &[]), (3, 0x13));
    assert_eq!(outbound_queue(0x6F, &[]), (3, 0x13));
    assert_eq!(outbound_queue(0xF4, &[]), (3, 0x05));
    assert_eq!(outbound_queue(0x30, &[]), (3, 0x0A));
    assert_eq!(outbound_queue(0x28, &[]), (3, 0x04));
    assert_eq!(outbound_queue(0x5F, &[]), (3, 0x08));
    assert_eq!(outbound_queue(0xC9, &[]), (3, 0x0E));
    // Unlisted: queue 4 (engineering choice).
    assert_eq!(outbound_queue(0xD0, &[]), (4, 0));
    // 0x24: continuous movement alone on queue 1, a discrete step on queue 3 group 5.
    let mut step = vec![0u8; 38];
    step[0x0E..0x12].copy_from_slice(&[0x87; 4]);
    assert!(!movement_is_discrete(&step));
    assert_eq!(outbound_queue(0x24, &step), (1, 0));
    let mut changed = step.clone();
    changed[0x11] = 0x80;
    assert!(movement_is_discrete(&changed));
    assert_eq!(outbound_queue(0x24, &changed), (3, 0x05));
    let mut jump = step.clone();
    jump[0x15] = 0x02;
    assert!(movement_is_discrete(&jump));
    let mut action = step.clone();
    action[0x12] = 0x01;
    assert!(movement_is_discrete(&action));
    assert!(movement_is_discrete(&[0u8; 4]));
}

#[test]
fn a_negative_logon_result_ends_the_session_and_zero_is_accepted() {
    // 0x80041464 with its position, then 0: queued, then on.
    let (world, mut d) = driver_at_login(Service::default(), 0);
    let mut login = Server::new(0x0111, 0xBEEF_0001);
    world.deliver(login.challenge(0xBEEF_0001), LOGIN_SERVER);
    let queued = LoginQueueStatusMessage {
        login_result: LOGON_RESULT_QUEUED,
        detail_text: String::new(),
        queue: Some(LoginQueuePosition {
            position: 7,
            total: 40,
        }),
    };
    world.deliver(
        login.blob(0x130, &encode_event_envelope(&queued.encode())),
        LOGIN_SERVER,
    );
    let accepted = LoginQueueStatusMessage {
        login_result: 0,
        detail_text: String::new(),
        queue: None,
    };
    world.deliver(
        login.blob(0x130, &encode_event_envelope(&accepted.encode())),
        LOGIN_SERVER,
    );
    d.step(0.1, true, &go);
    let events = d.take_events();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Account(AccountEvent::LoginQueueChanged {
            position: Some(7),
            length: Some(40),
            ..
        })
    )));
    assert!(!d.ended());

    // Any other negative value: refused, and the session ends.
    let refused = LoginQueueStatusMessage {
        login_result: 0x8000_4005,
        detail_text: "no".to_owned(),
        queue: None,
    };
    world.deliver(
        login.blob(0x130, &encode_event_envelope(&refused.encode())),
        LOGIN_SERVER,
    );
    d.step(0.2, true, &go);
    let events = d.take_events();
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Account(AccountEvent::LoginRefused {
            reason: LoginRefusal::AccountNotAllowed,
            ..
        })
    )));
    assert!(events.iter().any(is_disconnected));
    assert!(d.ended());
}
