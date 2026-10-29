// SPDX-License-Identifier: GPL-3.0-or-later
//! The handshake diagnostics (`super::super::diag`), the send checksum switch and the
//! login's neutral TLS line, on the synthetic service and a virtual clock: the counts are
//! right, and no log line, at any level, names a host, an address or a port.

use super::*;
use crate::connector::diag::{
    AuthMode, BindCase, HANDSHAKE_REPORT_SEC, LocalSource, RejectKind, RemoteForm, RemoteName,
    send_checksum_rule,
};
use crate::connector::login_phase::{ChainStep, PlainSteps, exchange};
use crate::login::NotSentKind;
use crate::net_access::TlsFailure;
use crate::session::{
    DisconnectReason as LinkEnd, SendChecksumRule, SessionConnection, combine_packet_checksum,
    compute_datagram_checksum, encode_datagram_with_rule, send_payload_checksum,
};

/// A log sink for [`logs_of`].
#[derive(Clone, Default)]
struct Buf(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Buf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend_from_slice(b);
        Ok(b.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// What `f` logs on this thread at every level (as `RUST_LOG=trace` would show it).
fn logs_of<T>(f: impl FnOnce() -> T) -> (T, String) {
    let buf = Buf::default();
    let out = buf.clone();
    let sub = tracing_subscriber::fmt()
        .with_writer(move || out.clone())
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .finish();
    let value = tracing::subscriber::with_default(sub, f);
    let bytes = buf
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    (value, String::from_utf8_lossy(&bytes).into_owned())
}

/// No address, port or host of the synthetic service in `logs`.
fn assert_no_endpoint(logs: &str) {
    for needle in [
        "192.0.2.",
        "example.invalid",
        ":9001",
        ":9100",
        ":5000",
        "canary",
        TICKET,
    ] {
        assert!(!logs.contains(needle), "{needle} logged:\n{logs}");
    }
}

/// Steps `d` every 0.5 s from `from` up to `to`.
fn run_until(d: &mut SessionDriver<FakeNet>, from: f64, to: f64) {
    let mut t = from;
    while t <= to {
        d.step(t, true, &go);
        t += 0.5;
    }
}

#[test]
fn a_handshake_timeout_logs_its_counts_and_no_endpoint() {
    let (logs_value, logs) = logs_of(|| {
        let (world, mut d) = driver_at_login(Service::default(), 0);
        run_until(&mut d, 0.0, 21.0);
        let r = d.handshake_report(false, 21.0).expect("login report");
        let sent = world.take_sent().len();
        (r, sent, d.ended())
    });
    let (r, sent, ended) = logs_value;
    assert!(ended);
    assert_eq!(r.state, Some(ConnectionState::SessionAuthSent));
    assert_eq!(r.counts.session_auth, 1);
    // Retransmits every 2.0 s until the 20.0 s timeout: at 2, 4, ..., 18 s.
    assert_eq!(r.counts.auth_retransmits, 9);
    assert_eq!(r.counts.sent, 10);
    assert_eq!(sent, 10);
    assert_eq!(r.counts.send_failures, 0);
    assert_eq!(r.link.received, 0);
    assert_eq!(r.link.rejected.total(), 0);
    assert!(!r.challenge_seen());
    assert_eq!(r.auth, AuthMode::Ticket);
    assert_eq!(r.local, LocalSource::Routed);
    assert_eq!(r.remote, RemoteForm::HostPort);
    assert_eq!(r.remote_name, RemoteName::Resolved);
    assert_eq!(r.send_checksum, SendChecksumRule::PerBuffer);
    // The start line, the waiting lines (debug, every 5 s) and the failure line.
    assert!(
        logs.contains("official session: handshake started"),
        "{logs}"
    );
    assert!(logs.contains("send_checksum=\"per-buffer\""), "{logs}");
    let waiting = logs.matches("official session: handshake waiting").count();
    assert!(waiting >= 3, "{waiting} waiting lines:\n{logs}");
    let failed: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("official session: handshake failed"))
        .collect();
    assert_eq!(failed.len(), 1, "{logs}");
    let line = failed.first().copied().unwrap_or_default();
    for field in [
        " INFO ",
        "reason=HandshakeTimeout",
        "state=Some(SessionAuthSent)",
        "auth_sent=1",
        "auth_retransmits=9",
        "sent=10",
        "received=0",
        "rejected_checksum=0",
        "challenge_seen=false",
        "bind=\"Case C (ephemeral port)\"",
        "local_address=\"routed toward the server\"",
        "remote=\"remoteHost:port (port in -h)\"",
        "remote_name=\"name resolved (first IPv4 address)\"",
        "recv_errors=0",
        "challenge_from_other_address=0",
        "auth=\"authType 0x40000002 authFlags 0x04 (ticket in extraBlob)\"",
    ] {
        assert!(line.contains(field), "{field} missing: {line}");
    }
    assert_no_endpoint(&logs);
}

#[test]
fn the_waiting_report_comes_every_five_seconds() {
    let ((), logs) = logs_of(|| {
        let (_world, mut d) = driver_at_login(Service::default(), 0);
        // One pass per 0.1 s up to just before the timeout.
        for i in 0..=195 {
            d.step(f64::from(i) * 0.1, true, &go);
        }
    });
    // At 5, 10 and 15 s.
    assert_eq!(
        logs.matches("official session: handshake waiting").count(),
        3,
        "{logs}"
    );
    const { assert!(HANDSHAKE_REPORT_SEC == 5.0) };
    assert!(!logs.contains("handshake failed"), "{logs}");
}

#[test]
fn a_challenge_with_a_bad_checksum_is_counted_as_seen() {
    let ((r, ended), logs) = logs_of(|| {
        let (world, mut d) = driver_at_login(Service::default(), 0);
        let login = Server::new(0x0111, 1);
        // Stray traffic from another address is no challenge.
        world.deliver(vec![0u8; 40], WORLD_SERVER_2);
        let mut bad = login.challenge(1);
        // The checksum's last byte (wire prefix 2 + header bytes 12..16).
        if let Some(b) = bad.get_mut(2 + 15) {
            *b ^= 0x5A;
        }
        world.deliver(bad, LOGIN_SERVER);
        // A short datagram, and one from an address no connection has.
        world.deliver(vec![0u8; 4], LOGIN_SERVER);
        world.deliver(login.challenge(1), WORLD_SERVER);
        run_until(&mut d, 0.0, 21.0);
        (d.handshake_report(false, 21.0).expect("report"), d.ended())
    });
    assert!(ended);
    assert_eq!(r.link.received, 2);
    assert_eq!(r.link.rejected.checksum, 1);
    assert_eq!(r.link.rejected.size, 1);
    assert_eq!(r.link.challenge_in_rejected, 1);
    assert_eq!(r.counts.challenges, 0);
    assert!(r.challenge_seen());
    assert_eq!(r.other_address, 2);
    // The challenge from the other address is told apart from stray traffic.
    assert_eq!(r.challenge_from_other_address, 1);
    assert!(logs.contains("challenge_seen=true"), "{logs}");
    assert!(logs.contains("challenge_from_other_address=1"), "{logs}");
    assert!(logs.contains("rejected_checksum=1"), "{logs}");
    assert_no_endpoint(&logs);
}

#[test]
fn a_timeout_in_state_two_names_that_state() {
    let ((r, ended), logs) = logs_of(|| {
        let (world, mut d) = driver_at_login(Service::default(), 0);
        let login = Server::new(0x0111, 1);
        world.deliver(login.challenge(1), LOGIN_SERVER);
        d.step(0.0, true, &go);
        run_until(&mut d, 0.5, 21.0);
        (d.handshake_report(false, 21.0).expect("report"), d.ended())
    });
    assert!(ended);
    assert_eq!(r.state, Some(ConnectionState::ChallengeAcceptSent));
    assert_eq!(r.counts.challenges, 1);
    assert_eq!(r.counts.connect_accept, 1);
    assert!(r.counts.connect_accept_retransmits > 0);
    assert_eq!(r.link.received, 1);
    assert_eq!(r.counts.received_ok, 1);
    assert!(logs.contains("state=Some(ChallengeAcceptSent)"), "{logs}");
    assert_no_endpoint(&logs);
}

#[test]
fn a_connected_session_logs_no_handshake_failure() {
    let ((), logs) = logs_of(|| {
        let (_world, mut d, _login) = driver_at_characters(Service::default(), 0);
        d.begin_shutdown(1.0);
        let mut t = 1.0;
        while !d.shutdown_step(t) && t < 60.0 {
            t += 0.5;
        }
    });
    assert!(logs.contains("handshake started"), "{logs}");
    assert!(!logs.contains("handshake failed"), "{logs}");
    assert_no_endpoint(&logs);
}

/// The client's outgoing ISAAC masks of a connection seeded by [`Server::challenge`]
/// (its `outgoing_seed`), the first `n`.
fn client_masks(n: usize) -> Vec<u32> {
    let mut isaac = IsaacCipher::from_seed(0xCAFE_1234);
    (0..n).map(|_| isaac.next_mask()).collect()
}

/// Which `payloadSum` rules `datagram`'s checksum fits: `(whole payload, per buffer)`. A
/// masked checksum fits a rule when one of `masks` makes it.
fn rules_fitting(datagram: &[u8], masks: &[u32]) -> (bool, bool) {
    let pkt = decode_datagram(datagram, false, None, false).expect("decodes");
    let whole = compute_datagram_checksum(&pkt.raw_payload);
    let per = send_payload_checksum(&pkt.opt_headers, &pkt.fragments).expect("re-encodes");
    let fits = |sum: u32| {
        if pkt.header.flags & crate::session::FLAG_MASKED_CHECKSUM == 0 {
            combine_packet_checksum(pkt.header, sum, None).ok() == Some(pkt.header.checksum)
        } else {
            masks.iter().any(|m| {
                combine_packet_checksum(pkt.header, sum, Some(*m)).ok() == Some(pkt.header.checksum)
            })
        }
    };
    (fits(whole), fits(per))
}

/// A connected session on the send checksum `rule` that sends traffic with two or more
/// buffers: every datagram the client sent to the login server, and the logs.
fn connected_traffic(rule: SendChecksumRule) -> (Vec<Vec<u8>>, String) {
    let mut start = session_start(0, None, Arc::default());
    start.send_checksum = rule;
    let (sent, logs) = logs_of(|| {
        let (world, mut d, mut login) = driver_at_characters_from(Service::default(), start);
        // At least 1,400 bytes received, then a blob sent a second later: it carries a
        // `FlowControlHeader` (6 bytes) before its fragment (`official-session.md` §5.5),
        // two buffers whose sums differ by the two rules.
        for _ in 0..30 {
            world.deliver(login.event(0x6C, &character_list()), LOGIN_SERVER);
        }
        d.step(0.2, true, &go);
        let _ = d.take_events();
        d.command(
            &Command::EnterWorld {
                character: "Aldo".to_owned(),
            },
            1.2,
        )
        .expect("enter");
        let mut t = 1.2;
        while t < 6.0 {
            d.step(t, true, &go);
            let _ = d.take_events();
            t += 0.5;
        }
        world.take_sent()
    });
    let sent = sent
        .into_iter()
        .filter(|(_, to)| *to == LOGIN_SERVER)
        .map(|(d, _)| d)
        .collect();
    (sent, logs)
}

#[test]
fn the_whole_payload_rule_reaches_every_datagram_sent() {
    let masks = client_masks(4096);
    let (whole_run, logs) = connected_traffic(SendChecksumRule::WholePayload);
    assert!(logs.contains("send_checksum=\"whole-payload\""), "{logs}");
    let (per_run, _) = connected_traffic(SendChecksumRule::PerBuffer);
    assert!(whole_run.len() >= 4, "{}", whole_run.len());
    // Every datagram of each run fits its own rule.
    for (i, dgram) in whole_run.iter().enumerate() {
        assert!(rules_fitting(dgram, &masks).0, "whole-payload datagram {i}");
    }
    for (i, dgram) in per_run.iter().enumerate() {
        assert!(rules_fitting(dgram, &masks).1, "per-buffer datagram {i}");
    }
    // And the rules differ for some of them: the switch changed bytes on the wire.
    let differing = whole_run
        .iter()
        .filter(|d| rules_fitting(d, &masks) == (true, false))
        .count();
    assert!(differing > 0, "no datagram where the rules differ");
    let per_differing = per_run
        .iter()
        .filter(|d| rules_fitting(d, &masks) == (false, true))
        .count();
    assert!(per_differing > 0);
}

/// The handshake datagrams of a connection on `rule`: the `SessionAuthHeader`, the
/// `ConnectAcceptHeader` after a challenge and, on a second connection, the
/// `ReferralAuthHeader`.
fn handshake_datagrams(rule: SendChecksumRule) -> Vec<Vec<u8>> {
    let start = session_start(0, None, Arc::default());
    let auth = start.handoff.session_auth_header(1_000).expect("auth");
    let mut out = Vec::new();
    let mut conn = SessionConnection::new(0x0100, LOGIN_SERVER, 0.0);
    conn.set_send_checksum_rule(rule);
    conn.start_login(auth, 0.0).expect("login");
    while let Some(d) = conn.pop_outbound_datagram() {
        out.push(d);
    }
    let login = Server::new(0x0111, 1);
    conn.receive_datagram(&login.challenge(1), LOGIN_SERVER, 0.5, Default::default())
        .expect("challenge");
    while let Some(d) = conn.pop_outbound_datagram() {
        out.push(d);
    }
    let mut world = SessionConnection::new(0x0101, WORLD_SERVER, 0.0);
    world.set_send_checksum_rule(rule);
    world
        .start_world_login(0x0102_0304_0506_0708, 0.0)
        .expect("referral");
    while let Some(d) = world.pop_outbound_datagram() {
        out.push(d);
    }
    out
}

#[test]
fn the_handshake_datagrams_are_the_same_under_both_rules() {
    // One buffer each: the two rules give the same sum, so the switch cannot change a
    // State-1 or State-2 timeout (`super::super::diag` "The send checksum switch").
    let per = handshake_datagrams(SendChecksumRule::PerBuffer);
    let whole = handshake_datagrams(SendChecksumRule::WholePayload);
    assert_eq!(per.len(), 3);
    assert_eq!(per, whole);
    let kinds: Vec<(bool, bool, bool)> = per
        .iter()
        .map(|d| {
            let o = decode_datagram(d, false, None, false)
                .expect("decodes")
                .opt_headers;
            (
                o.session_auth.is_some(),
                o.connect_accept.is_some(),
                o.referral_auth.is_some(),
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [
            (true, false, false),
            (false, true, false),
            (false, false, true)
        ]
    );
}

#[test]
fn the_chat_connection_uses_the_send_checksum_rule() {
    for rule in [SendChecksumRule::WholePayload, SendChecksumRule::PerBuffer] {
        let mut start = session_start(0, Some(CHAT_SERVER), Arc::default());
        start.send_checksum = rule;
        let (world, mut d, mut login) = driver_at_characters_from(
            Service {
                chat_socket: true,
                ..Service::default()
            },
            start,
        );
        d.command(
            &Command::EnterWorld {
                character: "Aldo".to_owned(),
            },
            1.0,
        )
        .expect("enter");
        world.deliver(login.event(0x90, &session_ready()), LOGIN_SERVER);
        d.step(1.1, true, &go);
        let (player, name) = d.wants_chat(1.2).expect("the chat session is due");
        let net = FakeNet(world.clone());
        d.attach_chat(net.chat_binder().expect("binder"), player, &name, 1.2, &go);
        d.step(2.3, true, &go);
        let mut chat = Server::new(0x0111, 0xBEEF_0003);
        world.deliver_chat(chat.challenge(0xBEEF_0003));
        world.deliver_chat(chat.control(OptionalHeaders {
            cumulative_ack: Some(0),
            ..OptionalHeaders::default()
        }));
        let status = crate::chat_session::events::ChatEvent::LoginStatus { status: 0 };
        world.deliver_chat(chat.event(status.opcode(), &status.encode().expect("status")));
        // Rooms worth more than 1,400 bytes, then silence until the heartbeat (15 s idle):
        // the heartbeat's blob carries a `FlowControlHeader` before its fragment.
        for i in 0..60 {
            let add = crate::chat_session::events::ChatEvent::AddRoom {
                notice_flags: 0,
                room: crate::chat_session::wire::ChatRoomDescriptor::new(
                    crate::chat_session::wire::DescriptorKind::UserRoom,
                    0,
                    format!("room{i}"),
                ),
            };
            let inner = add.encode().expect("event");
            world.deliver_chat(chat.event(add.opcode(), &inner));
        }
        let mut t = 2.4;
        while t < 20.0 {
            d.step(t, true, &go);
            let _ = d.take_events();
            t += 0.5;
        }
        let masks = client_masks(4096);
        let chat_sent: Vec<Vec<u8>> = world
            .take_sent()
            .into_iter()
            .filter(|(_, to)| *to == CHAT_SERVER)
            .map(|(d, _)| d)
            .collect();
        assert!(chat_sent.len() > 3, "{}", chat_sent.len());
        let fits: Vec<(bool, bool)> = chat_sent.iter().map(|d| rules_fitting(d, &masks)).collect();
        match rule {
            SendChecksumRule::WholePayload => {
                assert!(fits.iter().all(|f| f.0), "{fits:?}");
                assert!(fits.contains(&(true, false)), "{fits:?}");
            }
            SendChecksumRule::PerBuffer => {
                assert!(fits.iter().all(|f| f.1), "{fits:?}");
                assert!(fits.contains(&(false, true)), "{fits:?}");
            }
        }
    }
}

/// A `ConnectChallengeHeader` naming `codec_uuid`.
fn challenge_with_codec(codec_uuid: [u8; 16]) -> Vec<u8> {
    let opt = OptionalHeaders {
        connect_challenge: Some(ConnectChallengeHeader {
            server_time: 1.0,
            assigned_client_rec_id: 0x0111,
            server_rec_id: 0x0222,
            outgoing_seed: 0xCAFE_1234,
            incoming_seed: 1,
            reserved_dword_14: 0,
            client_conn_idx: 4,
            reserved_word_1a: 0,
            codec_uuid,
        }),
        ..OptionalHeaders::default()
    };
    encode_datagram(
        DatagramHeader {
            rec_id: 0x0111,
            ..DatagramHeader::default()
        },
        &opt,
        &[],
        None,
        None,
    )
    .expect("challenge")
}

#[test]
fn an_unsupported_codec_challenge_is_counted_once() {
    let ((r, ended), logs) = logs_of(|| {
        let (world, mut d) = driver_at_login(Service::default(), 0);
        world.deliver(challenge_with_codec([7u8; 16]), LOGIN_SERVER);
        d.step(0.0, true, &go);
        (d.handshake_report(false, 0.0).expect("report"), d.ended())
    });
    assert!(ended);
    assert_eq!(r.link.received, 1);
    assert_eq!(r.counts.challenges, 1);
    assert_eq!(r.counts.received_ok, 0);
    assert_eq!(r.link.rejected.codec, 1);
    assert_eq!(r.link.rejected.total(), 1);
    assert_eq!(r.link.challenge_in_rejected, 0);
    assert!(r.challenge_seen());
    let failed: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("official session: handshake failed"))
        .collect();
    assert_eq!(failed.len(), 1, "{logs}");
    let line = failed.first().copied().unwrap_or_default();
    for field in [
        "reason=UnsupportedCodec",
        "received_ok=0",
        "rejected_codec=1",
        "rejected_other=0",
        "challenges=1",
        "challenge_in_rejected=0",
    ] {
        assert!(line.contains(field), "{field} missing: {line}");
    }
    assert_no_endpoint(&logs);
}

#[test]
fn a_cancel_during_the_handshake_logs_its_counts_once() {
    let ((), logs) = logs_of(|| {
        let (_world, mut d) = driver_at_login(Service::default(), 0);
        run_until(&mut d, 0.0, 10.0);
        d.begin_shutdown(10.0);
        let mut t = 10.0;
        while d.shutdown_step(t) && t < 40.0 {
            t += 0.5;
        }
        d.begin_shutdown(t);
    });
    let failed: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("official session: handshake failed"))
        .collect();
    assert_eq!(failed.len(), 1, "{logs}");
    let line = failed.first().copied().unwrap_or_default();
    for field in [
        " INFO ",
        "reason=cancelled",
        "state=Some(SessionAuthSent)",
        "auth_retransmits=5",
    ] {
        assert!(line.contains(field), "{field} missing: {line}");
    }
    assert_no_endpoint(&logs);
}

#[test]
fn a_revoked_right_during_the_handshake_logs_its_counts_once() {
    let ((), logs) = logs_of(|| {
        let (_world, mut d) = driver_at_login(Service::default(), 0);
        run_until(&mut d, 0.0, 3.0);
        d.abandon(3.0);
        d.abandon(3.5);
    });
    let failed: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("official session: handshake failed"))
        .collect();
    assert_eq!(failed.len(), 1, "{logs}");
    assert!(
        failed
            .first()
            .is_some_and(|l| l.contains("reason=network right revoked")),
        "{logs}"
    );
    assert_no_endpoint(&logs);
}

#[test]
fn socket_read_errors_are_counted() {
    let ((), logs) = logs_of(|| {
        let (world, mut d) = driver_at_login(Service::default(), 0);
        world.fail_reads(2);
        d.step(0.5, true, &go);
        d.step(1.0, true, &go);
        d.step(1.5, true, &go);
        let r = d.handshake_report(false, 1.5).expect("report");
        assert_eq!(r.recv_errors, 2);
        run_until(&mut d, 2.0, 21.0);
    });
    assert!(logs.contains("recv_errors=2"), "{logs}");
}

#[test]
fn errors_and_links_print_no_address_in_debug() {
    let texts = [
        format!("{:?}", SessionError::UnknownRecipient { rec_id: 0x100 }),
        format!(
            "{:?}",
            SessionError::SourceAddressMismatch { rec_id: 0x100 }
        ),
        format!("{:?}", SessionConnection::new(0x0100, LOGIN_SERVER, 0.0)),
    ];
    for t in &texts {
        assert!(!t.contains("192.0.2."), "{t}");
        assert!(!t.contains("9001"), "{t}");
    }
    let (_world, d) = driver_at_login(Service::default(), 0);
    let t = format!("{d:?}");
    assert!(!t.contains("192.0.2."), "{t}");
}

#[test]
fn the_two_rules_differ_only_in_the_checksum() {
    // Two optional header buffers, the first not a multiple of 4 bytes long: the rules
    // give different sums (`send_payload_checksum`).
    let opt = OptionalHeaders {
        flow_control: Some(crate::session::FlowControlHeader {
            bytes_received_delta: 0x0102_0304,
            interval_ms: 0x0506,
        }),
        clock_sync: Some(12.5),
        ..OptionalHeaders::default()
    };
    let hdr = DatagramHeader {
        rec_id: 0x0222,
        ..DatagramHeader::default()
    };
    let per = encode_datagram_with_rule(hdr, &opt, &[], None, None, SendChecksumRule::PerBuffer)
        .expect("per-buffer");
    let whole =
        encode_datagram_with_rule(hdr, &opt, &[], None, None, SendChecksumRule::WholePayload)
            .expect("whole-payload");
    assert_eq!(
        per,
        encode_datagram(hdr, &opt, &[], None, None).expect("default")
    );
    assert_eq!(per.len(), whole.len());
    let differing: Vec<usize> = per
        .iter()
        .zip(&whole)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert!(!differing.is_empty());
    // Only the checksum field (wire prefix 2 + header bytes 12..16).
    assert!(
        differing.iter().all(|i| (14..18).contains(i)),
        "{differing:?}"
    );
    // The receiver's rule accepts the whole-payload sum and refuses the per-buffer one.
    assert!(decode_datagram(&whole, true, None, true).is_ok());
    assert!(matches!(
        decode_datagram(&per, true, None, true),
        Err(SessionError::ChecksumMismatch { .. })
    ));
}

#[test]
fn the_switch_reads_two_names_and_refuses_the_rest() {
    use std::ffi::OsStr;
    let ((), logs) = logs_of(|| {
        assert_eq!(send_checksum_rule(None), SendChecksumRule::PerBuffer);
        assert_eq!(
            send_checksum_rule(Some(OsStr::new("per-buffer"))),
            SendChecksumRule::PerBuffer
        );
        assert_eq!(
            send_checksum_rule(Some(OsStr::new("whole-payload"))),
            SendChecksumRule::WholePayload
        );
        // Exactly the two names: no other case, no surrounding spaces.
        for bad in [
            "",
            "whole",
            "wholepayload",
            "1",
            "per_buffer",
            " whole-payload",
            "Whole-Payload",
            "PER-BUFFER",
        ] {
            assert_eq!(
                send_checksum_rule(Some(OsStr::new(bad))),
                SendChecksumRule::PerBuffer,
                "{bad}"
            );
        }
    });
    assert_eq!(
        logs.matches("unknown send checksum rule refused").count(),
        8,
        "{logs}"
    );
    assert!(logs.contains(" WARN "), "{logs}");
    assert!(
        !logs.contains("per_buffer") && !logs.contains("PER-BUFFER"),
        "the value is not logged: {logs}"
    );
}

#[test]
fn the_bind_case_is_read_from_the_bound_port() {
    let ip = Ipv4Addr::new(192, 0, 2, 1);
    let at = |port| SocketAddrV4::new(ip, port);
    assert_eq!(
        BindCase::of(ip, 4000, true, at(4000)),
        BindCase::ExplicitBase
    );
    assert_eq!(
        BindCase::of(ip, 4000, true, at(4011)),
        BindCase::ExplicitBase
    );
    assert_eq!(BindCase::of(ip, 4000, true, at(4012)), BindCase::Ephemeral);
    let unique = crate::session::candidate_bind_ports(ip, 0, true);
    let first = unique.first().copied().expect("a Case B port");
    assert_eq!(
        BindCase::of(ip, 0, true, at(first)),
        BindCase::ComputedUnique
    );
    assert_eq!(BindCase::of(ip, 0, false, at(first)), BindCase::Ephemeral);
    assert_eq!(BindCase::of(ip, 0, true, at(1)), BindCase::Ephemeral);
    assert_eq!(
        BindCase::of(ip, 0, true, SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, first)),
        BindCase::EphemeralAnyAddress
    );
    let any = Ipv4Addr::UNSPECIFIED;
    assert_eq!(
        BindCase::of(any, 0, true, SocketAddrV4::new(any, 40000)),
        BindCase::Ephemeral
    );
    for case in [
        BindCase::ExplicitBase,
        BindCase::ComputedUnique,
        BindCase::Ephemeral,
        BindCase::EphemeralAnyAddress,
    ] {
        assert!(case.label().starts_with("Case "));
    }
    assert!(BindCase::Unknown.label().starts_with("unknown"));
    assert_eq!(RemoteForm::of_host(true, true), RemoteForm::LocalHostPort);
    assert_eq!(RemoteForm::of_host(false, true), RemoteForm::HostPort);
    assert_eq!(
        RemoteForm::of_host(false, false),
        RemoteForm::HostWithPortOption
    );
    assert_eq!(RemoteName::of_host("192.0.2.10"), RemoteName::LiteralIpv4);
    assert_eq!(
        RemoteName::of_host("login.example.invalid"),
        RemoteName::Resolved
    );
}

#[test]
fn modes_and_refusals_are_grouped() {
    use crate::session::{AUTH_TYPE_ACCOUNT, AUTH_TYPE_TICKET};
    assert_eq!(AuthMode::of(AUTH_TYPE_TICKET, 0x04), AuthMode::Ticket);
    assert_eq!(
        AuthMode::of(AUTH_TYPE_ACCOUNT, 0x02),
        AuthMode::TicketInSecondString
    );
    assert_eq!(AuthMode::of(AUTH_TYPE_ACCOUNT, 0), AuthMode::AccountOnly);
    assert_eq!(AuthMode::of(7, 0), AuthMode::Other);
    let kind = |e: SessionError| RejectKind::of(&e);
    assert_eq!(
        kind(SessionError::ChecksumMismatch {
            expected: 1,
            actual: 2
        }),
        RejectKind::Checksum
    );
    assert_eq!(
        kind(SessionError::DatagramTooShort { len: 3 }),
        RejectKind::Size
    );
    assert_eq!(
        kind(SessionError::UnknownRecipient { rec_id: 0x100 }),
        RejectKind::Routing
    );
    assert_eq!(
        kind(SessionError::UnsupportedCodecUuid { uuid: [1; 16] }),
        RejectKind::Codec
    );
    assert_eq!(
        kind(SessionError::InvalidHeaderFlags { flags: 1 }),
        RejectKind::Header
    );
    assert_eq!(
        kind(SessionError::DuplicateSequence {
            seq_num: 1,
            expected: 2
        }),
        RejectKind::Other
    );
    let _ = LinkEnd::HandshakeTimeout;
}

/// An HTTP client whose HTTPS requests fail with `kind` and whose plain requests answer 200;
/// counts its requests. The failure is in the handshake (not reached, as the login client
/// reports it) unless `after_connect` is set (a final failure once connected).
struct TlsFails(TlsFailure, usize, bool);

impl HttpClient for TlsFails {
    fn execute(&mut self, request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        self.1 += 1;
        if is_https(&request.url) {
            let reason = self.0.reason().to_owned();
            if self.2 {
                Err(LoginError::ServiceUnavailable { reason })
            } else {
                Err(LoginError::NotSent {
                    kind: NotSentKind::Unreached,
                    reason,
                })
            }
        } else {
            Ok(HttpResponse {
                status_code: 200,
                body: Vec::new(),
            })
        }
    }
}

fn request(url: &str) -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Get,
        url: url.to_owned(),
        headers: Vec::new(),
        body: Vec::new(),
        timeout_sec: 10,
        require_tls_validation: true,
    }
}

#[test]
fn a_tls_failure_logs_the_step_and_kind_once_and_no_host() {
    let ((plain, calls), logs) = logs_of(|| {
        let plain = PlainSteps::default();
        let mut http = TlsFails(TlsFailure::NameMismatch, 0, false);
        for _ in 0..2 {
            let mut req = request("");
            let resp = exchange(
                &mut http,
                ChainStep::DatacenterDocument,
                "http://gls.example.invalid:8080/GLS.DataCenterServer/Service.asmx",
                &mut req,
                &plain,
            )
            .expect("plain answer");
            assert_eq!(resp.status_code, 200);
        }
        // An `https://` address with no fallback logs too.
        let mut req = request("");
        let _ = exchange(
            &mut http,
            ChainStep::StatusUrl,
            "https://status.example.invalid/status",
            &mut req,
            &plain,
        );
        (plain, http.1)
    });
    // HTTPS form and plain request, then plain at once (the host was not reached over
    // HTTPS earlier in this login), then the `https://` address.
    assert_eq!(calls, 4);
    assert!(plain.tls_failed(ChainStep::DatacenterDocument));
    assert!(plain.tls_failed(ChainStep::StatusUrl));
    assert!(!plain.tls_failed(ChainStep::AuthServer));
    let lines: Vec<&str> = logs
        .lines()
        .filter(|l| {
            l.contains("official login: HTTPS certificate not valid for the step's address")
        })
        .collect();
    assert_eq!(lines.len(), 2, "{logs}");
    assert!(
        lines
            .iter()
            .any(|l| l.contains("step=\"datacenter document\"")
                && l.contains("kind=\"name mismatch\"")
                && l.contains(" INFO ")),
        "{logs}"
    );
    assert!(
        lines.iter().any(|l| l.contains("step=\"status URL\"")),
        "{logs}"
    );
    assert_no_endpoint(&logs);
    assert!(!logs.contains("8080"), "{logs}");
    assert!(!logs.contains("GLS.DataCenterServer"), "{logs}");
}

#[test]
fn a_handshake_failure_of_tls_has_its_own_line() {
    let ((), logs) = logs_of(|| {
        let plain = PlainSteps::default();
        let mut http = TlsFails(TlsFailure::Handshake, 0, false);
        let mut req = request("");
        let _ = exchange(
            &mut http,
            ChainStep::AuthServer,
            "http://auth.example.invalid/x",
            &mut req,
            &plain,
        );
    });
    assert!(
        logs.contains("official login: HTTPS handshake failed for the step's address"),
        "{logs}"
    );
    assert!(logs.contains("kind=\"handshake\""), "{logs}");
    assert_no_endpoint(&logs);
}

#[test]
fn a_tls_failure_once_connected_is_logged_and_final() {
    // Decision 0044 as amended: a failure after the connection was made is never asked
    // again over plain HTTP; its TLS kind is still logged once, without the host.
    let ((plain, calls, result), logs) = logs_of(|| {
        let plain = PlainSteps::default();
        let mut http = TlsFails(TlsFailure::Expired, 0, true);
        let mut req = request("");
        let result = exchange(
            &mut http,
            ChainStep::AuthServer,
            "http://auth.example.invalid/x",
            &mut req,
            &plain,
        );
        (plain, http.1, result)
    });
    assert_eq!(calls, 1);
    assert!(matches!(result, Err(LoginError::ServiceUnavailable { .. })));
    assert!(plain.tls_failed(ChainStep::AuthServer));
    assert!(!plain.went_plain(ChainStep::AuthServer));
    assert!(
        logs.contains("official login: HTTPS certificate not valid for the step's address"),
        "{logs}"
    );
    assert!(logs.contains("kind=\"expired\""), "{logs}");
    assert_no_endpoint(&logs);
}
