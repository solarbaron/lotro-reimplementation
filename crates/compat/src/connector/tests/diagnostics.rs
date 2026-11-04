// SPDX-License-Identifier: GPL-3.0-or-later
//! The handshake diagnostics (`super::super::diag`), the per-buffer send checksum and the
//! login's neutral TLS line, on the synthetic service and a virtual clock: the counts are
//! right, and no log line, at any level, names a host, an address or a port.

use super::*;
use crate::connector::diag::{
    AuthMode, BindCase, HANDSHAKE_REPORT_SEC, LocalSource, RejectKind, RemoteForm, RemoteName,
};
use crate::connector::login_phase::{ChainStep, PlainSteps, exchange};
use crate::login::NotSentKind;
use crate::net_access::TlsFailure;
use crate::session::{
    DisconnectReason as LinkEnd, SessionConnection, combine_packet_checksum,
    compute_datagram_checksum, guid_fields_swapped, payload_checksum,
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

/// What `f` logs on this thread at every level (as `RUST_LOG=trace` would show it), without the
/// time of day: its microseconds would match a test's numeric needle by chance.
pub(super) fn logs_of<T>(f: impl FnOnce() -> T) -> (T, String) {
    let buf = Buf::default();
    let out = buf.clone();
    let sub = tracing_subscriber::fmt()
        .with_writer(move || out.clone())
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .without_time()
        .finish();
    // With a single registered subscriber `tracing` asks only the current thread's subscriber
    // about a new callsite, so a callsite another test's thread reaches first would stay
    // disabled here; a second one, never used, turns that shortcut off.
    let _second = tracing::Dispatch::new(tracing_subscriber::fmt().finish());
    let value = tracing::subscriber::with_default(sub, || {
        tracing::callsite::rebuild_interest_cache();
        f()
    });
    let bytes = buf
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    (value, String::from_utf8_lossy(&bytes).into_owned())
}

/// A numeric needle over [`logs_of`] finds a logged value and nothing else: the helper's own text
/// has no digit, so the tests' checks cannot match by chance and do match what is really logged.
#[test]
fn the_log_capture_shows_values_and_no_clock() {
    let ((), logs) = logs_of(|| tracing::warn!(flags = 132, "refused"));
    assert!(logs.contains("flags=132"), "{logs}");
    let rest = logs.replace("flags=132", "");
    assert!(!rest.contains(|c: char| c.is_ascii_digit()), "{logs}");
}

/// No address, port or host of the synthetic service in `logs`.
pub(super) fn assert_no_endpoint(logs: &str) {
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
pub(super) fn run_until(d: &mut SessionDriver<FakeNet>, from: f64, to: f64) {
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
    assert_eq!(r.local, LocalSource::Wildcard);
    assert_eq!(r.remote, RemoteForm::HostPort);
    assert_eq!(r.remote_name, RemoteName::Resolved);
    // The start line, the waiting lines (debug, every 5 s) and the failure line.
    assert!(
        logs.contains("official session: handshake started"),
        "{logs}"
    );
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
        "reason=handshake timeout",
        "state=Some(SessionAuthSent)",
        "auth_sent=1",
        "auth_retransmits=9",
        "sent=10",
        "received=0",
        "rejected_checksum=0",
        "challenge_seen=false",
        "bind=\"Case B (computed unique port, seed from the routed address)\"",
        "local_address=\"wildcard (0.0.0.0)\"",
        "remote=\"remoteHost:port (port in -h)\"",
        "remote_name=\"name resolved (first IPv4 address)\"",
        "recv_errors=0",
        "challenge_from_other_address=0",
        "auth=\"authType 0x00000001 authFlags 0x04 (ticket text in extraBlob)\"",
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

/// Which `payloadSum` rules `datagram`'s checksum fits: `(whole payload, per buffer)`: the
/// withdrawn whole-payload sum and the §4.4.2 per-buffer sum. A masked checksum fits a rule
/// when one of `masks` makes it.
fn rules_fitting(datagram: &[u8], masks: &[u32]) -> (bool, bool) {
    let pkt = decode_client_datagram(datagram, false).expect("decodes");
    let whole = compute_datagram_checksum(&pkt.raw_payload);
    let per = payload_checksum(&pkt.opt_headers, &pkt.fragments).expect("re-encodes");
    assert_eq!(
        per, pkt.payload_sum,
        "the received buffers are the sent ones"
    );
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

/// A connected session that sends traffic with two or more buffers: every datagram the
/// client sent to the login server.
fn connected_traffic() -> Vec<Vec<u8>> {
    let start = session_start(0, None, Arc::default());
    // Datagrams received, then a blob sent in answer in the same pass (the `0x12C` that
    // answers a `0x12F`): it carries the pending `FlowControlHeader` (6 bytes) before its
    // fragment (`official-session.md` §5.5 P10-22-POLICY), buffers whose per-buffer sum
    // differs from the whole-payload one.
    let (world, mut d, _login) =
        driver_at_characters_after(Service::default(), start, |world, login, d| {
            for _ in 0..30 {
                world.deliver(login.event(0x149, &[0x00, 0x7F]), LOGIN_SERVER);
            }
            world.deliver(login.event(0x12F, &[]), LOGIN_SERVER);
            d.step(0.05, true, &go);
        });
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
    world
        .take_sent()
        .into_iter()
        .filter(|(_, to)| *to == LOGIN_SERVER)
        .map(|(d, _)| d)
        .collect()
}

#[test]
fn every_datagram_sent_fits_the_per_buffer_rule_and_not_the_withdrawn_one() {
    // `official-session.md` §4.4.2: one per-buffer rule on send and on receive; the
    // whole-payload rule is withdrawn and must fail a multi-buffer datagram.
    let masks = client_masks(4096);
    let run = connected_traffic();
    assert!(run.len() >= 4, "{}", run.len());
    for (i, dgram) in run.iter().enumerate() {
        assert!(rules_fitting(dgram, &masks).1, "per-buffer datagram {i}");
    }
    let differing = run
        .iter()
        .filter(|d| rules_fitting(d, &masks) == (false, true))
        .count();
    assert!(differing > 0, "no datagram where the rules differ");
}

/// An independent check of the client's sent checksums (`official-session.md` §4.4, §5.1,
/// §5.6.3, §7.2), written from the spec and sharing no checksum code with the sender: the
/// §4.4.1 sum, the §5.6.3 fragment header lengths from the parsed fields, the per-buffer
/// payload sum of §4.4.2 over the raw payload split at those boundaries, and one outgoing
/// ISAAC word per sequenced `seqNum` in order (`seqNum` 2 takes the first), a resend
/// reusing its word.
struct SendOracle {
    isaac: IsaacCipher,
    words: std::collections::BTreeMap<u32, u32>,
    next_seq: u32,
    checked: usize,
    /// Masked datagrams whose `seqNum` had been seen (resends).
    resent: usize,
}

impl SendOracle {
    fn new(outgoing_seed: u32) -> Self {
        Self {
            isaac: IsaacCipher::from_seed(outgoing_seed),
            words: std::collections::BTreeMap::new(),
            next_seq: 2,
            checked: 0,
            resent: 0,
        }
    }

    /// §4.4.1, from the spec's table: the length × 65,536 for every buffer, big-endian
    /// words, the tail in the high positions.
    fn sum(b: &[u8]) -> u32 {
        let len = u32::try_from(b.len()).expect("short");
        let mut acc = len.wrapping_mul(65_536);
        let whole = b.len() / 4 * 4;
        for w in b[..whole].as_chunks::<4>().0 {
            acc = acc.wrapping_add(u32::from_be_bytes(*w));
        }
        let tail = &b[whole..];
        let mut t = 0u32;
        for (j, byte) in tail.iter().enumerate() {
            t |= u32::from(*byte) << (24 - 8 * j);
        }
        acc.wrapping_add(t)
    }

    /// §5.6.1 sizes: three tiers.
    fn vlen(v: u32) -> usize {
        match v {
            0..=0x7F => 1,
            0x80..=0x3FFF => 2,
            _ => 4,
        }
    }

    /// §4.3: the byte length of each optional header the client sends, in ascending flag
    /// order; a standalone `SessionAuthHeader` is the whole `opt_len`.
    fn opt_lens(o: &OptionalHeaders, opt_len: usize) -> Vec<usize> {
        let list = |s: &Option<Vec<u32>>| s.as_ref().map(|v| 4 + 4 * v.len());
        [
            list(&o.sequence_nak),
            list(&o.out_of_order_ack),
            o.cumulative_ack.map(|_| 4),
            o.session_auth.as_ref().map(|_| opt_len),
            o.referral_auth.map(|_| 8),
            o.connect_accept.map(|_| 28),
            o.clock_sync.map(|_| 8),
            o.flow_control.map(|_| 6),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// §5.6.3: the even-length fragment header of `h` (fields 1–9; no attachment).
    fn header_len(h: &crate::session::BlobFragmentHeader) -> usize {
        assert!(h.attachment.is_none(), "the client sends no attachment");
        let q = h.queue_id;
        let fragmented = h.blob_flags & 0x10 != 0;
        let mut n = 2;
        if q == 3 {
            n += Self::vlen(u32::from(h.seq_group));
        }
        if (2..=4).contains(&q) {
            n += 2;
        }
        // Field 4: never on queue 3 (§5.6.2, §5.6.3).
        if q == 2 || q == 4 || (q == 1 && fragmented) {
            n += 2;
        }
        if fragmented {
            n += Self::vlen(h.frag_index) + Self::vlen(h.num_frags);
        }
        if h.ext_word.is_some() {
            n += 2;
        }
        if h.blob_flags & 0x40 == 0 {
            n += 8;
        }
        if h.blob_flags & 0x80 == 0 {
            if h.frag_index == 0 {
                n += Self::vlen(h.blob_header_word.expect("fragment 0 has its word"));
            }
            n += Self::vlen(h.frag_data_len);
        }
        n + n % 2
    }

    /// Checks one uncompressed datagram the client sent; returns whether it was masked.
    fn check(&mut self, datagram: &[u8]) -> bool {
        assert_eq!(&datagram[..2], &[0, 0], "uncompressed");
        let pkt = decode_client_datagram(datagram, false).expect("decodes");
        let payload = &pkt.raw_payload;
        // The payload split (§4.4.2 step 2): each optional header, then each fragment's
        // header and its data, every one its own buffer.
        let frag_bytes: usize = pkt
            .fragments
            .iter()
            .map(|f| Self::header_len(&f.header) + f.payload.len())
            .sum();
        let opt_len = payload.len() - frag_bytes;
        let mut sum = 0u32;
        let mut at = 0usize;
        for n in Self::opt_lens(&pkt.opt_headers, opt_len) {
            sum = sum.wrapping_add(Self::sum(&payload[at..at + n]));
            at += n;
        }
        assert_eq!(at, opt_len, "the optional headers cover their part");
        for f in &pkt.fragments {
            let h = Self::header_len(&f.header);
            sum = sum.wrapping_add(Self::sum(&payload[at..at + h]));
            at += h;
            assert_eq!(&payload[at..at + f.payload.len()], f.payload.as_slice());
            sum = sum.wrapping_add(Self::sum(&payload[at..at + f.payload.len()]));
            at += f.payload.len();
        }
        assert_eq!(at, payload.len(), "the split covers the payload");
        // §4.4.2 step 1: the header with `BA DD 70 DD` in its checksum field.
        let mut header = datagram[2..22].to_vec();
        let wire = u32::from_be_bytes([header[12], header[13], header[14], header[15]]);
        header[12..16].copy_from_slice(&[0xBA, 0xDD, 0x70, 0xDD]);
        let hdr_sum = Self::sum(&header);
        let masked = pkt.header.flags & 0x0000_0002 != 0;
        let payload_sum = if masked {
            let seq = pkt.header.seq_num;
            let word = match self.words.get(&seq) {
                Some(w) => {
                    self.resent += 1;
                    *w
                }
                None => {
                    assert_eq!(seq, self.next_seq, "one word per seqNum, in order");
                    self.next_seq += 1;
                    let w = self.isaac.next_mask();
                    self.words.insert(seq, w);
                    w
                }
            };
            wire.wrapping_sub(hdr_sum) ^ word
        } else {
            wire.wrapping_sub(hdr_sum)
        };
        assert_eq!(payload_sum, sum, "seqNum {}", pkt.header.seq_num);
        self.checked += 1;
        masked
    }
}

#[test]
fn every_datagram_sent_fits_an_independent_checksum_oracle() {
    // The login connection from its first datagram: the `SessionAuthHeader`, the
    // `ConnectAcceptHeader`, the `0x12C` (queue 4) after the version request, acks with a
    // `FlowControlHeader`, and the `0x6F` (queue 2, implicit group 0), resends included.
    let start = session_start(0, None, Arc::default());
    let world = new_world(Service::default());
    let mut net = FakeNet(world.clone());
    let mut d = SessionDriver::start(&mut net, start, 0.0, &go).expect("start");
    let mut login = Server::new(0x0111, 0xBEEF_0001);
    world.deliver(login.challenge(0xBEEF_0001), LOGIN_SERVER);
    d.step(0.0, true, &go);
    // A first server packet confirms State 5; the tick sends the FlowControl-only packet
    // (sequence 2, §6.4.1), so the `0x12C` answering the `0x12F` that follows carries the
    // next report with its fragment (§5.5 P10-22-POLICY).
    world.deliver(
        login.control(OptionalHeaders {
            cumulative_ack: Some(0),
            ..OptionalHeaders::default()
        }),
        LOGIN_SERVER,
    );
    d.step(0.05, true, &go);
    world.deliver(login.event(0x12F, &[]), LOGIN_SERVER);
    d.step(0.1, true, &go);
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
    while t < 8.0 {
        d.step(t, true, &go);
        let _ = d.take_events();
        t += 0.5;
    }
    let mut sent = world.take_sent();
    // The server asks again for the first blob datagram, sequence 3 (the FlowControl-only
    // sequence 2 has nothing to resend, §5.3): its resend reuses the word of its `seqNum`
    // (§7.2).
    world.deliver(
        login.control(OptionalHeaders {
            sequence_nak: Some(vec![3]),
            ..OptionalHeaders::default()
        }),
        LOGIN_SERVER,
    );
    d.step(t, true, &go);
    sent.extend(world.take_sent());
    let mut oracle = SendOracle::new(0xCAFE_1234);
    let (mut masked, mut flow_and_fragment, mut opcodes) = (0, 0, Vec::new());
    for (dgram, to) in sent {
        assert_eq!(to, LOGIN_SERVER);
        if oracle.check(&dgram) {
            masked += 1;
        }
        let pkt = decode_client_datagram(&dgram, false).expect("decodes");
        if pkt.opt_headers.flow_control.is_some() && !pkt.fragments.is_empty() {
            flow_and_fragment += 1;
        }
        opcodes.extend(pkt.fragments.iter().filter_map(|f| {
            f.header
                .blob_header_word
                .map(|w| (w, f.header.queue_id, f.header.seq_group))
        }));
    }
    assert!(
        oracle.checked >= 4 && masked >= 2,
        "{} / {masked}",
        oracle.checked
    );
    assert!(
        flow_and_fragment >= 1,
        "a FlowControlHeader before a fragment"
    );
    assert!(oracle.resent >= 1, "a resend reuses its word");
    assert!(opcodes.contains(&(0x12C, 4, 0)), "{opcodes:?}");
    assert!(opcodes.contains(&(0x6F, 2, 0)), "{opcodes:?}");
}

/// The handshake datagrams of a connection: the `SessionAuthHeader`, the
/// `ConnectAcceptHeader` after a challenge and, on a second connection, the
/// `ReferralAuthHeader`.
fn handshake_datagrams() -> Vec<Vec<u8>> {
    let start = session_start(0, None, Arc::default());
    let auth = start.handoff.session_auth_header(1_000).expect("auth");
    let mut out = Vec::new();
    let mut conn = SessionConnection::new(LOGIN_SERVER, 0.0);
    conn.set_codec_uuids(SYNTHETIC_CODEC_UUIDS);
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
    let mut world = SessionConnection::new(WORLD_SERVER, 0.0);
    world.set_codec_uuids(SYNTHETIC_CODEC_UUIDS);
    world
        .start_world_login([1, 2, 3, 4, 5, 6, 7, 8], 0.0)
        .expect("referral");
    while let Some(d) = world.pop_outbound_datagram() {
        out.push(d);
    }
    out
}

#[test]
fn the_handshake_datagrams_are_one_buffer_each() {
    // One buffer each, so the per-buffer and the withdrawn whole-payload sums agree; the
    // auth, the accept and the referral carry sequence 0 (`official-session.md` §5.1,
    // §6.4, §6.5).
    let datagrams = handshake_datagrams();
    assert_eq!(datagrams.len(), 3);
    let kinds: Vec<(bool, bool, bool, u32)> = datagrams
        .iter()
        .map(|d| {
            assert_eq!(rules_fitting(d, &[]), (true, true));
            let p = decode_client_datagram(d, true).expect("decodes");
            let o = p.opt_headers;
            (
                o.session_auth.is_some(),
                o.connect_accept.is_some(),
                o.referral_auth.is_some(),
                p.header.seq_num,
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [
            (true, false, false, 0),
            (false, true, false, 0),
            (false, false, true, 0)
        ]
    );
}

#[test]
fn the_chat_challenge_is_an_unsupported_negotiation() {
    // `official-session.md` §6.3, §7.3 (OPEN-SESSION-1): the chat dictionary source is not
    // known, so the chat connection has no dictionaries and its challenge is refused with
    // the explicit unsupported result, logged as a chat handshake failure.
    let ((), logs) = logs_of(|| {
        let start = session_start(0, Some(CHAT_SERVER), Arc::default());
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
        d.attach_chat(
            net.chat_binder(d.first_local_ipv4()).expect("binder"),
            player,
            &name,
            1.2,
            &go,
        );
        d.step(2.3, true, &go);
        let chat = Server::new(0x0111, 0xBEEF_0003);
        world.deliver_chat(chat.challenge(0xBEEF_0003));
        d.step(2.4, true, &go);
        let _ = d.take_events();
        // No accept went to the chat server.
        let accepts = world
            .take_sent()
            .into_iter()
            .filter(|(_, to)| *to == CHAT_SERVER)
            .filter(|(d, _)| {
                decode_client_datagram(d, false)
                    .is_ok_and(|p| p.opt_headers.connect_accept.is_some())
            })
            .count();
        assert_eq!(accepts, 0);
    });
    let failed: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("official chat session: handshake failed"))
        .collect();
    assert!(
        failed.iter().any(|l| l.contains("UnsupportedCodec")),
        "{logs}"
    );
    assert_no_endpoint(&logs);
}

/// A `ConnectChallengeHeader` naming the dictionary whose on-disk UUID is `codec_uuid`.
fn challenge_with_codec(codec_uuid: [u8; 16]) -> Vec<u8> {
    challenge_datagram(0x0111, 0x0222, 0x3344, 1, 0xCAFE_1234, codec_uuid)
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
        "reason=unsupported dictionary",
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
fn a_null_or_unconverted_or_outgoing_challenge_uuid_is_unsupported() {
    // `official-session.md` §7.3: the challenge's bytes 28..43 are the incoming dictionary's
    // UUID converted field-wise; a null UUID, the raw on-disk bytes (unconverted) and the
    // outgoing dictionary's UUID are each the explicit unsupported result.
    let incoming = INCOMING_UUID;
    let outgoing = OUTGOING_UUID;
    for named in [[0u8; 16], guid_fields_swapped(incoming), outgoing] {
        let (world, mut d) = driver_at_login(Service::default(), 0);
        world.take_sent();
        world.deliver(challenge_with_codec(named), LOGIN_SERVER);
        d.step(0.0, true, &go);
        assert!(d.ended(), "{named:02X?}");
        let r = d.handshake_report(false, 0.0).expect("report");
        assert_eq!(r.link.rejected.codec, 1, "{named:02X?}");
        assert!(
            world.take_sent().iter().all(|(d, _)| {
                decode_client_datagram(d, false)
                    .is_ok_and(|p| p.opt_headers.connect_accept.is_none())
            }),
            "no accept for {named:02X?}"
        );
    }
    // The converted incoming UUID is accepted.
    let (world, mut d) = driver_at_login(Service::default(), 0);
    world.deliver(challenge_with_codec(incoming), LOGIN_SERVER);
    d.step(0.0, true, &go);
    assert!(!d.ended());
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
        format!("{:?}", SessionConnection::new(LOGIN_SERVER, 0.0)),
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
fn the_bind_case_is_the_plans_case() {
    use crate::session::BindPorts;
    let ip = Ipv4Addr::new(192, 0, 2, 1);
    assert_eq!(
        BindCase::of(&BindPorts::select(4000, true, Some(ip))),
        BindCase::ExplicitPort
    );
    assert_eq!(
        BindCase::of(&BindPorts::select(0, true, Some(ip))),
        BindCase::ComputedUnique
    );
    assert_eq!(
        BindCase::of(&BindPorts::select(0, false, Some(ip))),
        BindCase::Ephemeral
    );
    assert_eq!(
        BindCase::of(&BindPorts::select(0, true, None)),
        BindCase::Ephemeral
    );
    for case in [
        BindCase::ExplicitPort,
        BindCase::ComputedUnique,
        BindCase::Ephemeral,
    ] {
        assert!(case.label().starts_with("Case "));
    }
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
    use crate::session::{AUTH_TYPE_CHAT, AUTH_TYPE_PRIMARY};
    assert_eq!(AuthMode::of(AUTH_TYPE_PRIMARY, 0x04), AuthMode::Ticket);
    assert_eq!(
        AuthMode::of(AUTH_TYPE_PRIMARY, 0x06),
        AuthMode::TicketAndPassword
    );
    assert_eq!(AuthMode::of(AUTH_TYPE_CHAT, 0x04), AuthMode::ChatTicket);
    assert_eq!(AuthMode::of(AUTH_TYPE_PRIMARY, 0), AuthMode::AccountOnly);
    assert_eq!(AuthMode::of(AUTH_TYPE_CHAT, 0), AuthMode::AccountOnly);
    assert_eq!(AuthMode::of(AUTH_TYPE_PRIMARY, 0x02), AuthMode::Other);
    assert_eq!(AuthMode::of(AUTH_TYPE_CHAT, 0x06), AuthMode::Other);
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
    // The start line carries the `netVersion`, whose stamp contains the port's digits.
    let scrubbed = logs.replace(crate::session::NET_VERSION_STAMP, "<stamp>");
    assert!(!scrubbed.contains("8080"), "{logs}");
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

/// The lines of `logs` that contain `text`.
fn lines_with<'a>(logs: &'a str, text: &str) -> Vec<&'a str> {
    logs.lines().filter(|l| l.contains(text)).collect()
}

/// No address of the queue tests' status answers in `logs` (besides [`assert_no_endpoint`]).
fn assert_no_queue_address(logs: &str) {
    assert_no_endpoint(logs);
    // The start line carries the `netVersion`, whose stamp contains the port's digits.
    let logs = &logs.replace(crate::session::NET_VERSION_STAMP, "<stamp>");
    for needle in [
        "10.20.30.40",
        "192.168.7.8",
        "8080",
        "/queue",
        "/status",
        "/q",
    ] {
        assert!(!logs.contains(needle), "{needle} logged:\n{logs}");
    }
}

const READY: &str = "<Result><HResult>0x00000000</HResult><QueueNumber>0x00000010</QueueNumber>\
                     <NowServingNumber>0x00000010</NowServingNumber></Result>";

const QUEUE_HTTPS: &str = "https://gls.example.invalid/queue";

#[test]
fn the_queue_step_logs_counts_labels_and_polls_and_no_host() {
    // Two login servers and two queue URLs, both in a private range: the chosen one is the
    // first (`official-login.md` §3.3), a private-range one. The queue waits once, then
    // lets the user in on the second poll.
    let status = "<Status><loginservers>login.example.invalid:9001;login2.example.invalid:9001;\
                  </loginservers><queueurls>http://10.20.30.40:8080/q;http://192.168.7.8/q;\
                  </queueurls></Status>";
    let ((), logs) = logs_of(|| {
        let mut run = start(
            Service {
                alpha_queue: true,
                queue_waiting: true,
                answers: vec![(ALPHA_HTTPS.to_owned(), 200, status.to_owned())],
                ..Service::default()
            },
            true,
        );
        run.wait_event(is_world_list);
        run.choose(0);
        run.wait_event(|e| {
            matches!(
                e,
                Event::Account(AccountEvent::LoginQueueChanged {
                    position: Some(3),
                    ..
                })
            )
        });
        run.world
            .set_answers(vec![(QUEUE_HTTPS.to_owned(), 200, READY.to_owned())]);
        wait_binds(&mut run, 1);
        let _ = run.session.send(Instant::now(), Command::Disconnect);
    });
    let status_lines = lines_with(&logs, "official login: world status read");
    assert_eq!(status_lines.len(), 1, "{logs}");
    let line = status_lines.first().copied().unwrap_or_default();
    for field in [
        " INFO ",
        "login_servers=2",
        "queue_urls=2",
        "chosen_queue_url=\"private range (RFC 1918)\"",
    ] {
        assert!(line.contains(field), "{field} missing: {line}");
    }
    let queued = lines_with(&logs, "official login: queued");
    assert_eq!(queued.len(), 1, "{logs}");
    let line = queued.first().copied().unwrap_or_default();
    assert!(line.starts_with("DEBUG "), "{line}");
    assert!(line.contains("position=3"), "{line}");
    let step = lines_with(&logs, "official login: queue step (TakeANumber)");
    assert_eq!(step.len(), 1, "{logs}");
    let line = step.first().copied().unwrap_or_default();
    for field in [" INFO ", "outcome=\"ready\"", "polls=2"] {
        assert!(line.contains(field), "{field} missing: {line}");
    }
    assert_no_queue_address(&logs);
}

#[test]
fn a_world_without_a_queue_url_logs_the_queue_step_skipped() {
    let ((), logs) = logs_of(|| {
        let mut run = start(Service::default(), true);
        run.wait_event(is_world_list);
        run.choose(0);
        wait_binds(&mut run, 1);
        let _ = run.session.send(Instant::now(), Command::Disconnect);
    });
    let status_lines = lines_with(&logs, "official login: world status read");
    assert_eq!(status_lines.len(), 1, "{logs}");
    let line = status_lines.first().copied().unwrap_or_default();
    for field in [
        "login_servers=1",
        "queue_urls=0",
        "chosen_queue_url=\"none\"",
    ] {
        assert!(line.contains(field), "{field} missing: {line}");
    }
    let step = lines_with(&logs, "official login: queue step (TakeANumber)");
    assert_eq!(step.len(), 1, "{logs}");
    let line = step.first().copied().unwrap_or_default();
    for field in [" INFO ", "outcome=\"skipped\"", "reason=\"no queue URL\""] {
        assert!(line.contains(field), "{field} missing: {line}");
    }
    assert!(
        lines_with(&logs, "official login: queued").is_empty(),
        "{logs}"
    );
    assert_no_queue_address(&logs);
}

#[test]
fn a_failed_queue_step_logs_its_class_only() {
    // A public queue URL; the queue answers a negative `HResult` (§3.5: `F08`).
    let ((), logs) = logs_of(|| {
        let mut run = start(
            Service {
                alpha_queue: true,
                answers: vec![(
                    QUEUE_HTTPS.to_owned(),
                    200,
                    "<Result><HResult>0x80004005</HResult></Result>".to_owned(),
                )],
                ..Service::default()
            },
            true,
        );
        run.wait_event(is_world_list);
        run.choose(0);
        run.wait_event(|e| {
            matches!(
                e,
                Event::Account(AccountEvent::WorldRefused {
                    world: WorldId(0),
                    ..
                })
            )
        });
        assert_eq!(run.world.binds(), 0);
        let _ = run.session.send(Instant::now(), Command::Disconnect);
    });
    let status_lines = lines_with(&logs, "official login: world status read");
    let line = status_lines.first().copied().unwrap_or_default();
    for field in [
        "login_servers=1",
        "queue_urls=1",
        "chosen_queue_url=\"not a private range\"",
    ] {
        assert!(line.contains(field), "{field} missing: {line}");
    }
    let step = lines_with(&logs, "official login: queue step (TakeANumber)");
    assert_eq!(step.len(), 1, "{logs}");
    let line = step.first().copied().unwrap_or_default();
    for field in [" INFO ", "outcome=\"failed\"", "polls=1", "class=F08"] {
        assert!(line.contains(field), "{field} missing: {line}");
    }
    assert!(!logs.contains("0x80004005"), "{logs}");
    assert_no_queue_address(&logs);
}

/// EV-12: closing the window lets the session go at once. A wait at the exit sees the thread's end:
/// the session's Disconnect and its connection totals are out by then.
#[test]
fn an_exit_waits_for_the_session_it_just_let_go() {
    let ((), logs) = logs_of(|| {
        let mut run = start(Service::default(), true);
        run.wait_event(is_world_list);
        run.choose(0);
        let _ = run.wait_sent();
        let login = Server::new(0x0111, 0xBEEF_0001);
        login.connect(&run, 0xBEEF_0001, LOGIN_SERVER);
        let _ = run.wait_sent();
        let _ = run.session.send(Instant::now(), Command::Disconnect);
        drop(run);
        assert!(crate::connector::wait_for_ending_sessions(
            Duration::from_secs(10)
        ));
    });
    assert!(
        !lines_with(&logs, "official evidence: connection totals").is_empty(),
        "{logs}"
    );
    assert!(
        logs.contains("official session closed"),
        "the thread's own end: {logs}"
    );
}
