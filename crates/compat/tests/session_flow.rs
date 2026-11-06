// SPDX-License-Identifier: GPL-3.0-or-later
//! The FlowControl-only packets of `docs/specs/official-session.md` §5.5 under the P10-10J schedule
//! of §9.3.12 (`OPEN-SESSION-13`) on a virtual clock: the first one on the first tick after State 5;
//! later ones only after newly accepted application-fragment or clock traffic, and at most once per
//! [`FLOW_REPORT_ONLY_PERIOD_SEC`] (Tessera's cap). Pure acknowledgements never make a report due,
//! so a server that only acknowledges gets exactly one report, and its bytes wait for the next
//! eligible one. Every byte here is synthetic; nothing leaves the process.

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
    mux.receive_from_peer(&p, SERVER, 0.1, WireCodecs::NONE)
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

/// Eligible traffic rides on the next outgoing sequenced packet when one goes out first; the bytes
/// held from pure acknowledgements go with it (§9.3.12 "piggyback on real outgoing application
/// traffic when due").
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
    // Eligible: the next outgoing blob carries the report, at once, inside the standalone cap.
    let p = server.packet();
    received += p.len() as u64;
    mux.receive_from_peer(&p, SERVER, 0.3, WireCodecs::NONE)
        .expect("blob");
    let conn = mux.connection_mut(id).expect("conn");
    conn.send_blob(2, 0, 0x6F, &[2], 0.35, None).expect("blob");
    collect(&mut mux, id, 0.35, &mut sent);
    assert_eq!(sent.attached, 1);
    assert_eq!(sent.flow_only.len(), 1, "only the first standalone report");
    assert_eq!(sent.reported_bytes, received);
    // Nothing more is due.
    mux.tick(5.0).expect("tick");
    collect(&mut mux, id, 5.0, &mut sent);
    assert_eq!(sent.flow_only.len(), 1);
}

/// Labelled eligibility (`OPEN-SESSION-13`): a datagram whose only content is a ClockSyncHeader makes
/// a report due; one whose only fragment is an empty control does not.
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
