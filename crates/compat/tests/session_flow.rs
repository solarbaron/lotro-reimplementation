// SPDX-License-Identifier: GPL-3.0-or-later
//! The flow reports of `docs/specs/official-session.md` §5.5 under the closed P10-10L policy of
//! §9.3.12 on a virtual clock, with Vector 25: the first one on the first tick after State 5;
//! later ones only after newly accepted application-fragment or clock traffic, and every report
//! after the first, standalone or attached, at most once per [`FLOW_REPORT_ONLY_PERIOD_SEC`] since
//! the last report sent. Pure acknowledgements never make a report due, so a server that only
//! acknowledges gets exactly one report, and its bytes wait for the next eligible one. Every byte
//! here is synthetic; nothing leaves the process.

use std::collections::VecDeque;
use std::net::{Ipv4Addr, SocketAddrV4};

use tessera_compat::session::{
    AuthConnection, BlobFragment, CodecUuids, ConnectChallengeHeader, ConnectionState,
    DatagramHeader, FLAG_MASKED_CHECKSUM, FLOW_REPORT_ONLY_PERIOD_SEC, IsaacCipher,
    OptionalHeaders, SessionAuthHeader, SessionMultiplexer, WireCodecs, decode_datagram,
    encode_datagram, guid_fields_swapped, split_blob,
};

const SERVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(198, 51, 100, 70), 9000);
const ASSIGNED: u16 = 0x0111;
const SEED: u32 = 0xBEEF_0070;
const UUIDS: CodecUuids = CodecUuids {
    incoming: [0x21; 16],
    outgoing: [0x12; 16],
};
/// The synthetic round-trip time of the acknowledging server.
const RTT: f64 = 0.05;
/// Float noise allowance when comparing virtual times.
const EPS: f64 = 1e-9;

fn challenge() -> Vec<u8> {
    encode_datagram(
        DatagramHeader {
            rec_id: ASSIGNED,
            peer_word: 0x3344,
            ..DatagramHeader::default()
        },
        &OptionalHeaders {
            connect_challenge: Some(ConnectChallengeHeader {
                clock_candidate: 1.0,
                cookie: [1, 2, 3, 4, 5, 6, 7, 8],
                reserved_16: [0, 0],
                outgoing_rec_id: 0x0222,
                incoming_seed: SEED,
                outgoing_seed: 0xCAFE_1234,
                incoming_codec_guid: guid_fields_swapped(UUIDS.incoming),
            }),
            ..OptionalHeaders::default()
        },
        &[],
        None,
        None,
    )
    .expect("challenge")
}

/// The synthetic server's datagrams: sequenced ones masked as §7.2 has it, and the small
/// unsequenced acknowledgements.
struct Server {
    isaac: IsaacCipher,
    seq: u32,
}

impl Server {
    fn new() -> Self {
        Self {
            isaac: IsaacCipher::from_seed(SEED),
            seq: 1,
        }
    }

    /// The next sequenced packet, carrying a one-byte blob: eligible traffic (§9.3.12).
    fn packet(&mut self) -> Vec<u8> {
        let frags =
            split_blob(2, ((self.seq + 1) & 0xFFFF) as u16, 1, 0, None, 0x10, b"x").expect("split");
        self.sequenced(&OptionalHeaders::default(), &frags)
    }

    /// The next sequenced packet with `opt` and `frags`.
    fn sequenced(&mut self, opt: &OptionalHeaders, frags: &[BlobFragment]) -> Vec<u8> {
        self.seq += 1;
        let mask = self.isaac.next_mask();
        encode_datagram(
            DatagramHeader {
                rec_id: ASSIGNED,
                seq_num: self.seq,
                ..DatagramHeader::default()
            },
            opt,
            frags,
            Some(mask),
            None,
        )
        .expect("packet")
    }

    /// An unsequenced CumulativeAck of the client's `seq` (§5.2).
    fn ack(&self, seq: u32) -> Vec<u8> {
        encode_datagram(
            DatagramHeader {
                rec_id: ASSIGNED,
                seq_num: self.seq,
                ..DatagramHeader::default()
            },
            &OptionalHeaders {
                cumulative_ack: Some(seq),
                ..OptionalHeaders::default()
            },
            &[],
            None,
            None,
        )
        .expect("ack")
    }
}

/// What the client sent, as the tests count it.
#[derive(Default)]
struct Sent {
    /// Send times of the FlowControl-only packets (no fragment, no other header).
    flow_only: Vec<f64>,
    /// Sequenced packets with a fragment that carried a report.
    attached: usize,
    /// Every reported byte count, FlowControl-only or attached.
    reported_bytes: u64,
    /// The sequence numbers of every sequenced packet, in order.
    sequenced: Vec<u32>,
    /// Send times of the FlowControl-only packets that went out with nothing received since
    /// the previous report (there must be none after the first).
    flow_only_with_nothing: usize,
}

/// A multiplexer whose one connection is in State 5, the first tick not yet run, with the
/// bytes it has received so far.
fn connected(server: &mut Server) -> (SessionMultiplexer, u16, u64) {
    connected_at(server, 0.1)
}

/// [`connected`] with the first server packet, and so State 5, at `t`.
fn connected_at(server: &mut Server, t: f64) -> (SessionMultiplexer, u16, u64) {
    let mut mux = SessionMultiplexer::new();
    let id = mux.create_connection(SERVER, 0.0).expect("id");
    let conn = mux.connection_mut(id).expect("conn");
    conn.set_codec_uuids(UUIDS);
    let auth = SessionAuthHeader::for_connection(
        AuthConnection::Primary,
        "netver:1234; didver:01020304-0506-0708-090A-0B0C0D0E0F10",
        "A",
        None,
        None,
        0,
    )
    .expect("auth");
    conn.start_login(auth, 0.0).expect("login");
    while conn.pop_outbound_datagram().is_some() {}
    let c = challenge();
    let id = mux
        .receive_from_peer(&c, SERVER, 0.0, WireCodecs::NONE)
        .expect("challenge")
        .0;
    let p = server.packet();
    mux.receive_from_peer(&p, SERVER, t, WireCodecs::NONE)
        .expect("the first server packet");
    let conn = mux.connection_mut(id).expect("conn");
    assert_eq!(conn.state(), ConnectionState::Connected);
    while conn.pop_outbound_datagram().is_some() {}
    (mux, id, (c.len() + p.len()) as u64)
}

/// Pops what `id` queued at `t` into `sent`; returns the sequence numbers of its sequenced
/// packets.
fn collect(mux: &mut SessionMultiplexer, id: u16, t: f64, sent: &mut Sent) -> Vec<u32> {
    let conn = mux.connection_mut(id).expect("conn");
    let mut seqs = Vec::new();
    while let Some(d) = conn.pop_outbound_datagram() {
        let p = decode_datagram(&d, false, None, false).expect("own datagram decodes");
        if p.header.flags & FLAG_MASKED_CHECKSUM == 0 {
            continue;
        }
        seqs.push(p.header.seq_num);
        sent.sequenced.push(p.header.seq_num);
        if let Some(flow) = p.opt_headers.flow_control {
            sent.reported_bytes += u64::from(flow.bytes_received_delta);
            if p.fragments.is_empty() {
                if flow.bytes_received_delta == 0 && !sent.flow_only.is_empty() {
                    sent.flow_only_with_nothing += 1;
                }
                sent.flow_only.push(t);
            } else {
                sent.attached += 1;
            }
        }
    }
    seqs
}

/// No two FlowControl-only packets after the first are closer than the report tick period.
fn assert_spaced(times: &[f64]) {
    for w in times.windows(2) {
        assert!(
            w[1] - w[0] >= FLOW_REPORT_ONLY_PERIOD_SEC - EPS,
            "FlowControl-only packets at {} and {}",
            w[0],
            w[1]
        );
    }
}

/// The first report still goes out on the first tick after State 5, at once, as sequence 2.
#[test]
fn the_first_report_comes_on_the_first_tick() {
    let mut server = Server::new();
    let (mut mux, id, received) = connected(&mut server);
    let due = mux.next_deadline(0.1).expect("a deadline");
    assert!(due <= 0.1, "due at once, not at {due}");
    mux.tick(0.1).expect("tick");
    let mut sent = Sent::default();
    collect(&mut mux, id, 0.1, &mut sent);
    assert_eq!(sent.flow_only, vec![0.1]);
    assert_eq!(sent.sequenced, vec![2]);
    assert_eq!(sent.reported_bytes, received, "challenge and first packet");
}

/// A server that acknowledges every sequenced packet at once, one round trip later, while a
/// reliable client blob is in flight: pure acknowledgements never make a report due (§9.3.12
/// P10-10J), so exactly one FlowControl-only packet goes out, the first, and the blob carries no
/// report. The client ticks every 5 ms and after every arrival, harder than the connector does.
/// The acknowledgements' bytes are held, and go out on the report that the next eligible datagram
/// makes due.
#[test]
fn an_acknowledging_server_starts_no_report_loop() {
    let mut server = Server::new();
    let (mut mux, id, mut received) = connected(&mut server);
    let mut sent = Sent::default();
    let mut arrivals: VecDeque<(f64, u32)> = VecDeque::new();
    let end = 0.1 + 5.0;
    let mut step = 0u32;
    let mut blob_sent = false;
    loop {
        let t = 0.1 + f64::from(step) * 0.005;
        if t > end + EPS {
            break;
        }
        step += 1;
        while arrivals.front().is_some_and(|&(at, _)| at <= t + EPS) {
            let Some((_, seq)) = arrivals.pop_front() else {
                break;
            };
            let ack = server.ack(seq);
            received += ack.len() as u64;
            mux.receive_from_peer(&ack, SERVER, t, WireCodecs::NONE)
                .expect("the ack");
            mux.tick(t).expect("tick after the arrival");
            for s in collect(&mut mux, id, t, &mut sent) {
                arrivals.push_back((t + RTT, s));
            }
        }
        mux.tick(t).expect("tick");
        if !blob_sent && t >= 0.3 - EPS {
            // A reliable blob of several fragments, once the first report's acknowledgement
            // is in.
            let conn = mux.connection_mut(id).expect("conn");
            conn.send_blob(2, 0, 0x6F, &[7; 3000], t, None)
                .expect("the blob");
            blob_sent = true;
        }
        for s in collect(&mut mux, id, t, &mut sent) {
            arrivals.push_back((t + RTT, s));
        }
    }
    assert_eq!(
        sent.flow_only,
        vec![0.1],
        "exactly one FlowControl-only packet, the first"
    );
    assert_eq!(sent.attached, 0, "acknowledgements make no report due");
    assert_eq!(sent.flow_only_with_nothing, 0);

    // The acknowledgements stop: still no report is due, however long the client waits.
    while let Some((at, seq)) = arrivals.pop_front() {
        let ack = server.ack(seq);
        received += ack.len() as u64;
        mux.receive_from_peer(&ack, SERVER, at, WireCodecs::NONE)
            .expect("the last acks");
    }
    let quiet = end + 3.0 * FLOW_REPORT_ONLY_PERIOD_SEC;
    mux.tick(quiet).expect("tick");
    collect(&mut mux, id, quiet, &mut sent);
    assert_eq!(sent.flow_only.len(), 1, "no report for held pure-Ack bytes");

    // An eligible datagram arrives: the next report carries every held byte with its own.
    let p = server.packet();
    received += p.len() as u64;
    let at = quiet + 0.1;
    mux.receive_from_peer(&p, SERVER, at, WireCodecs::NONE)
        .expect("a blob");
    let due = mux.next_deadline(at).expect("the report is a deadline");
    assert!(due <= at + EPS, "due at once, the cap long past: {due}");
    mux.tick(at).expect("tick");
    collect(&mut mux, id, at, &mut sent);
    assert_eq!(sent.flow_only.len(), 2);
    assert_eq!(sent.reported_bytes, received, "nothing received is lost");
}

/// Eligible traffic rides on the next outgoing sequenced packet once the one-second cap since the
/// last report is up; the bytes held from pure acknowledgements go with it (§9.3.12 "piggyback on
/// an ordinary outgoing application carrier only when eligible AND due").
#[test]
fn an_eligible_report_rides_on_outgoing_traffic() {
    let mut server = Server::new();
    let (mut mux, id, mut received) = connected(&mut server);
    let mut sent = Sent::default();
    mux.tick(0.1).expect("tick");
    collect(&mut mux, id, 0.1, &mut sent);
    let held = server.ack(2);
    received += held.len() as u64;
    mux.receive_from_peer(&held, SERVER, 0.2, WireCodecs::NONE)
        .expect("ack");
    // A pure acknowledgement: an outgoing blob carries no report.
    let conn = mux.connection_mut(id).expect("conn");
    conn.send_blob(2, 0, 0x6F, &[1], 0.25, None).expect("blob");
    collect(&mut mux, id, 0.25, &mut sent);
    assert_eq!(sent.attached, 0);
    // Eligible, but inside the cap: the next outgoing blob carries no report yet.
    let p = server.packet();
    received += p.len() as u64;
    mux.receive_from_peer(&p, SERVER, 0.3, WireCodecs::NONE)
        .expect("blob");
    let conn = mux.connection_mut(id).expect("conn");
    conn.send_blob(2, 0, 0x6F, &[2], 0.35, None).expect("blob");
    collect(&mut mux, id, 0.35, &mut sent);
    assert_eq!(sent.attached, 0, "the cap holds attached reports too");
    // The cap is up: the next carrier takes the report, and no standalone one follows.
    let conn = mux.connection_mut(id).expect("conn");
    conn.send_blob(2, 0, 0x6F, &[3], 1.1, None).expect("blob");
    collect(&mut mux, id, 1.1, &mut sent);
    assert_eq!(sent.attached, 1);
    assert_eq!(sent.flow_only.len(), 1, "only the first standalone report");
    assert_eq!(sent.reported_bytes, received);
    // Nothing more is due.
    mux.tick(5.0).expect("tick");
    collect(&mut mux, id, 5.0, &mut sent);
    assert_eq!(sent.flow_only.len(), 1);
}

/// One report as Vector 25 reads it: when, whether attached, and its two fields.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Report {
    at: f64,
    attached: bool,
    bytes: u32,
    interval_ms: u16,
}

/// Pops what `id` queued at `t`; returns its reports and the number of sequenced packets.
fn reports(mux: &mut SessionMultiplexer, id: u16, t: f64) -> (Vec<Report>, Vec<u32>) {
    let conn = mux.connection_mut(id).expect("conn");
    let mut out = Vec::new();
    let mut seqs = Vec::new();
    while let Some(d) = conn.pop_outbound_datagram() {
        let p = decode_datagram(&d, false, None, false).expect("own datagram decodes");
        if p.header.flags & FLAG_MASKED_CHECKSUM == 0 {
            continue;
        }
        seqs.push(p.header.seq_num);
        if let Some(flow) = p.opt_headers.flow_control {
            out.push(Report {
                at: t,
                attached: !p.fragments.is_empty(),
                bytes: flow.bytes_received_delta,
                interval_ms: flow.interval_ms,
            });
        }
    }
    (out, seqs)
}

/// Vector 25 (`official-session.md` §10, flow rows), per connection, State 5 and the first report
/// at time 0, no send failures.
#[test]
fn vector_25_flow_schedule() {
    let mut server = Server::new();
    let (mut mux, id, first_bytes) = connected_at(&mut server, 0.0);

    // First report at 0: sent once, first-report exception to the cap.
    mux.tick(0.0).expect("tick");
    let (r, _) = reports(&mut mux, id, 0.0);
    assert_eq!(r.len(), 1);
    assert!(!r[0].attached);
    assert_eq!(u64::from(r[0].bytes), first_bytes);

    // Pure Ack at +0.2: counted, no eligibility, no report.
    let ack = server.ack(2);
    let mut held = ack.len() as u64;
    mux.receive_from_peer(&ack, SERVER, 0.2, WireCodecs::NONE)
        .expect("ack");
    mux.tick(0.2).expect("tick");
    assert!(reports(&mut mux, id, 0.2).0.is_empty());
    assert_eq!(mux.next_deadline(0.2).map(|d| d >= 1.0), Some(true));

    // New accepted ordinary fragment at +0.3: eligibility latches, no early report.
    let p = server.packet();
    held += p.len() as u64;
    mux.receive_from_peer(&p, SERVER, 0.3, WireCodecs::NONE)
        .expect("fragment");
    mux.tick(0.3).expect("tick");
    assert!(reports(&mut mux, id, 0.3).0.is_empty(), "no early report");
    let due = mux.next_deadline(0.3).expect("a deadline");
    assert!(due <= 1.0 + EPS, "the report is due at 1.0, not {due}");
    mux.tick(0.999).expect("tick");
    assert!(reports(&mut mux, id, 0.999).0.is_empty());

    // Maintenance at +1.0: the tick at the due instant sends the standalone report, which
    // carries the Ack and fragment bytes since 0; the next tick sends nothing more.
    mux.tick(1.0).expect("tick");
    let (r, _) = reports(&mut mux, id, 1.0);
    assert_eq!(r.len(), 1, "sends at +1.0");
    assert!(!r[0].attached, "standalone");
    assert_eq!(u64::from(r[0].bytes), held);
    assert_eq!(r[0].interval_ms, 1000);
    mux.tick(1.05).expect("tick");
    assert!(reports(&mut mux, id, 1.05).0.is_empty(), "not again");

    // Fresh fragment at +1.1, carrier at +1.5: the carrier goes without a report.
    let p = server.packet();
    let mut held = p.len() as u64;
    mux.receive_from_peer(&p, SERVER, 1.1, WireCodecs::NONE)
        .expect("fragment");
    let conn = mux.connection_mut(id).expect("conn");
    conn.send_blob(2, 0, 0x6F, &[1], 1.5, None)
        .expect("carrier");
    let (r, seqs) = reports(&mut mux, id, 1.5);
    assert!(r.is_empty(), "cap not due: {r:?}");
    assert_eq!(seqs.len(), 1, "the carrier itself");
    let mut client_seqs = seqs;

    // Carrier at +2.0: the report rides on it and restarts the cap like a standalone one.
    let conn = mux.connection_mut(id).expect("conn");
    conn.send_blob(2, 0, 0x6F, &[2], 2.0, None)
        .expect("carrier");
    let (r, seqs) = reports(&mut mux, id, 2.0);
    assert_eq!(r.len(), 1);
    assert!(r[0].attached);
    assert_eq!(u64::from(r[0].bytes), held);
    assert_eq!(r[0].interval_ms, 1000);
    client_seqs.extend(seqs);

    // Only pure Acks for 31 s after +2.0: no FlowControl-only send, and the due 30-second
    // ClockSync creates no carrier.
    held = 0;
    let mut t = 2.0;
    while t < 33.0 {
        t += 0.25;
        let ack = server.ack(client_seqs.last().copied().unwrap_or(2));
        held += ack.len() as u64;
        mux.receive_from_peer(&ack, SERVER, t, WireCodecs::NONE)
            .expect("ack");
        mux.tick(t).expect("tick");
        let (r, seqs) = reports(&mut mux, id, t);
        assert!(r.is_empty(), "no report at {t}: {r:?}");
        assert!(seqs.is_empty(), "no sequenced packet at {t}: {seqs:?}");
    }

    // Next eligible fragment at +33.1: the report is long due, so the tick at +33.1 sends it,
    // standalone, with intervalMs exactly 31100.
    let p = server.packet();
    held += p.len() as u64;
    mux.receive_from_peer(&p, SERVER, 33.1, WireCodecs::NONE)
        .expect("fragment");
    mux.tick(33.1).expect("tick");
    let (r, _) = reports(&mut mux, id, 33.1);
    assert_eq!(r.len(), 1, "sends at +33.1");
    assert!(!r[0].attached, "standalone");
    assert_eq!(r[0].interval_ms, 31_100, "no saturation");
    assert_eq!(u64::from(r[0].bytes), held);

    // The next real eligible report 65,537 ms after the prior one: intervalMs 1, not 65,535.
    // Pure Acks keep the connection inside its silence timeout meanwhile.
    let next = 33.1 + 65.537;
    let mut t = 33.1;
    while t + 10.0 < next {
        t += 10.0;
        let ack = server.ack(client_seqs.last().copied().unwrap_or(2));
        mux.receive_from_peer(&ack, SERVER, t, WireCodecs::NONE)
            .expect("ack");
        mux.tick(t).expect("tick");
        assert!(reports(&mut mux, id, t).0.is_empty());
    }
    let p = server.packet();
    mux.receive_from_peer(&p, SERVER, next, WireCodecs::NONE)
        .expect("fragment");
    mux.tick(next).expect("tick");
    let (r, _) = reports(&mut mux, id, next);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].interval_ms, 1, "modulo 65,536");
}

/// The closed eligibility list of §9.3.12: a datagram whose only content is a ClockSyncHeader makes
/// a report due; one whose only fragment is an empty omitted-chunk control does not.
#[test]
fn clock_traffic_is_eligible_and_an_empty_control_is_not() {
    let mut server = Server::new();
    let (mut mux, id, _) = connected(&mut server);
    let mut sent = Sent::default();
    mux.tick(0.1).expect("tick");
    collect(&mut mux, id, 0.1, &mut sent);
    let (control, _) = BlobFragment::decode(&[0x01, 0xC0]).expect("an empty control");
    let d = server.sequenced(&OptionalHeaders::default(), &[control]);
    mux.receive_from_peer(&d, SERVER, 0.2, WireCodecs::NONE)
        .expect("empty control");
    mux.tick(2.0).expect("tick");
    collect(&mut mux, id, 2.0, &mut sent);
    assert_eq!(
        sent.flow_only.len(),
        1,
        "an empty control makes nothing due"
    );
    let clock = OptionalHeaders {
        clock_sync: Some(1000.0),
        ..OptionalHeaders::default()
    };
    let d = server.sequenced(&clock, &[]);
    mux.receive_from_peer(&d, SERVER, 2.1, WireCodecs::NONE)
        .expect("clock");
    mux.tick(2.1).expect("tick");
    collect(&mut mux, id, 2.1, &mut sent);
    assert_eq!(
        sent.flow_only,
        vec![0.1, 2.1],
        "clock traffic makes a report due"
    );
}

/// A long quiet stretch with periodic server datagrams: at most one FlowControl-only packet a
/// second, whatever the datagrams' period, and every datagram is reported in the end. The
/// client sleeps until its next deadline or the next datagram, as the connector does.
#[test]
fn periodic_server_datagrams_get_at_most_one_report_a_second() {
    for period in [2.0, 0.3, 0.05] {
        let mut server = Server::new();
        let (mut mux, id, mut received) = connected(&mut server);
        let mut sent = Sent::default();
        let start = 0.1;
        let end = start + 40.0;
        let mut next_datagram = start + period;
        let mut t = start;
        let mut wakes = 0u32;
        while t <= end {
            wakes += 1;
            assert!(wakes < 100_000, "the client spins at {t}");
            if next_datagram <= t + EPS {
                let p = server.packet();
                received += p.len() as u64;
                mux.receive_from_peer(&p, SERVER, t, WireCodecs::NONE)
                    .expect("a server datagram");
                next_datagram += period;
            }
            mux.tick(t).expect("tick");
            collect(&mut mux, id, t, &mut sent);
            let deadline = mux.next_deadline(t).unwrap_or(f64::INFINITY);
            // The connector waits at least a little; 1 ms here.
            t = deadline.min(next_datagram).max(t + 1e-3);
        }
        let seconds = end - start;
        assert!(
            sent.flow_only.len() as f64 <= seconds + 1.0,
            "period {period}: {} FlowControl-only packets in {seconds} s",
            sent.flow_only.len()
        );
        assert_spaced(&sent.flow_only);
        assert_eq!(sent.flow_only_with_nothing, 0, "period {period}");
        // Reports keep coming: roughly one per datagram when they are slower than the
        // period, about one per period otherwise.
        let expected = (seconds / period.max(FLOW_REPORT_ONLY_PERIOD_SEC)).floor() as usize;
        assert!(
            sent.flow_only.len() + 1 >= expected,
            "period {period}: only {} reports, {expected} expected",
            sent.flow_only.len()
        );
        // What is still held goes out a period later.
        let last = t + FLOW_REPORT_ONLY_PERIOD_SEC;
        mux.tick(last).expect("tick");
        collect(&mut mux, id, last, &mut sent);
        assert_eq!(sent.reported_bytes, received, "period {period}");
    }
}

/// §9.3.12 (P10-10M) and Vector 25 ("Carrier coincides with due send"): the standalone report is
/// scheduled AT its due instant (the connection's next deadline wakes the driver then), a later
/// tick still sends it once, and a carrier and a tick at the same instant carry one report, on the
/// carrier (the driver runs a pass's carriers before its tick).
#[test]
fn a_due_report_goes_at_the_first_tick_or_on_a_carrier_at_that_instant() {
    // The due instant 1.0 is the next deadline; a tick that comes late, at 1.37, sends once.
    let mut server = Server::new();
    let (mut mux, id, _) = connected_at(&mut server, 0.0);
    mux.tick(0.0).expect("tick");
    let _ = reports(&mut mux, id, 0.0);
    let p = server.packet();
    mux.receive_from_peer(&p, SERVER, 0.3, WireCodecs::NONE)
        .expect("fragment");
    assert_eq!(
        mux.connection(id).and_then(|c| c.next_deadline()),
        Some(1.0),
        "scheduled at the due instant"
    );
    mux.tick(0.9).expect("tick");
    assert!(
        reports(&mut mux, id, 0.9).0.is_empty(),
        "not before it is due"
    );
    mux.tick(1.37).expect("tick");
    let (r, _) = reports(&mut mux, id, 1.37);
    assert_eq!(r.len(), 1);
    assert!(!r[0].attached);
    assert_eq!(r[0].interval_ms, 1370);
    mux.tick(1.5).expect("tick");
    assert!(reports(&mut mux, id, 1.5).0.is_empty());

    // A carrier and a tick both at the due instant 2.37: one report, on the carrier.
    let p = server.packet();
    mux.receive_from_peer(&p, SERVER, 1.6, WireCodecs::NONE)
        .expect("fragment");
    let conn = mux.connection_mut(id).expect("conn");
    conn.send_blob(2, 0, 0x6F, &[1], 2.37, None)
        .expect("carrier");
    mux.tick(2.37).expect("tick");
    let (r, seqs) = reports(&mut mux, id, 2.37);
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].attached, "on the carrier");
    assert_eq!(r[0].interval_ms, 1000);
    assert_eq!(seqs.len(), 1, "no standalone packet beside the carrier");
}
