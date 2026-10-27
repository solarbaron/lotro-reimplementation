// SPDX-License-Identifier: GPL-3.0-or-later
//! The handshake diagnostics (`super::super::diag`), the send checksum switch and the
//! login's neutral TLS line, on the synthetic service and a virtual clock: the counts are
//! right, and no log line, at any level, names a host, an address or a port.

use super::*;
use crate::connector::diag::{
    AuthMode, BindCase, HANDSHAKE_REPORT_SEC, LocalSource, RejectKind, send_checksum_rule,
};
use crate::connector::login_phase::{ChainStep, PlainSteps, exchange};
use crate::net_access::TlsFailure;
use crate::session::{
    DisconnectReason as LinkEnd, SendChecksumRule, compute_datagram_checksum,
    encode_datagram_with_rule,
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
    assert_eq!(r.other_address, 1);
    assert!(logs.contains("challenge_seen=true"), "{logs}");
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
    assert_eq!(r.counts.accepted, 1);
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

#[test]
fn the_whole_payload_rule_reaches_every_datagram_sent() {
    let mut start = session_start(0, None, Arc::default());
    start.send_checksum = SendChecksumRule::WholePayload;
    let ((world, d), logs) = logs_of(|| {
        let world = new_world(Service::default());
        let mut net = FakeNet(world.clone());
        let mut d = SessionDriver::start(&mut net, start, 0.0, &go).expect("start");
        let login = Server::new(0x0111, 1);
        world.deliver(login.challenge(1), LOGIN_SERVER);
        run_until(&mut d, 0.0, 3.0);
        (world, d)
    });
    let r = d.handshake_report(false, 3.0).expect("report");
    assert_eq!(r.send_checksum, SendChecksumRule::WholePayload);
    assert!(logs.contains("send_checksum=\"whole-payload\""), "{logs}");
    // Every unsequenced datagram the client sent verifies by the receive rule (the
    // whole-payload sum), so the switch is applied on send.
    let sent = world.take_sent();
    assert!(sent.len() >= 3, "{}", sent.len());
    for (dgram, to) in &sent {
        assert_eq!(*to, LOGIN_SERVER);
        let pkt = decode_datagram(dgram, false, None, false).expect("decodes");
        if pkt.header.flags & crate::session::FLAG_MASKED_CHECKSUM == 0 {
            let payload_sum = compute_datagram_checksum(&pkt.raw_payload);
            let expected = crate::session::combine_packet_checksum(pkt.header, payload_sum, None)
                .expect("sum");
            assert_eq!(pkt.header.checksum, expected);
        }
    }
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
            send_checksum_rule(Some(OsStr::new(" Whole-Payload "))),
            SendChecksumRule::WholePayload
        );
        for bad in ["", "whole", "wholepayload", "1", "per_buffer"] {
            assert_eq!(
                send_checksum_rule(Some(OsStr::new(bad))),
                SendChecksumRule::PerBuffer,
                "{bad}"
            );
        }
    });
    assert_eq!(
        logs.matches("unknown send checksum rule refused").count(),
        5,
        "{logs}"
    );
    assert!(logs.contains(" WARN "), "{logs}");
    assert!(
        !logs.contains("per_buffer"),
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
        kind(SessionError::UnknownRecipient {
            rec_id: 0x100,
            source_addr: LOGIN_SERVER
        }),
        RejectKind::Routing
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
    // The two errors that carry addresses name none in their text.
    let texts = [
        SessionError::UnknownRecipient {
            rec_id: 0x100,
            source_addr: LOGIN_SERVER,
        }
        .to_string(),
        SessionError::SourceAddressMismatch {
            rec_id: 0x100,
            expected: LOGIN_SERVER,
            actual: WORLD_SERVER,
        }
        .to_string(),
    ];
    for t in texts {
        assert!(!t.contains("192.0.2."), "{t}");
        assert!(!t.contains("9001") && !t.contains("9100"), "{t}");
    }
    let _ = LinkEnd::HandshakeTimeout;
}

/// An HTTP client whose HTTPS requests fail with `kind` and whose plain requests answer 200.
struct TlsFails(TlsFailure, usize);

impl HttpClient for TlsFails {
    fn execute(&mut self, request: &HttpRequest) -> Result<HttpResponse, LoginError> {
        self.1 += 1;
        if is_https(&request.url) {
            Err(LoginError::ServiceUnavailable {
                reason: self.0.reason().to_owned(),
            })
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
        let mut http = TlsFails(TlsFailure::NameMismatch, 0);
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
    assert_eq!(calls, 5);
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
        let mut http = TlsFails(TlsFailure::Handshake, 0);
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
