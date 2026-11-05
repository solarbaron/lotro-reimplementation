// SPDX-License-Identifier: GPL-3.0-or-later
//! The FlowControl-only packets of `docs/specs/official-session.md` §5.5 (P10-22-POLICY,
//! `OPEN-SESSION-13`) on a virtual clock: the first one on the first tick after State 5, later
//! ones at most once per report tick period ([`FLOW_REPORT_ONLY_PERIOD_SEC`], Tessera's value),
//! so a server that acknowledges every sequenced packet at once cannot start a loop at the
//! round-trip time. Every byte here is synthetic; nothing leaves the process.

use std::collections::VecDeque;
use std::net::{Ipv4Addr, SocketAddrV4};

use tessera_compat::session::{
    AuthConnection, CodecUuids, ConnectChallengeHeader, ConnectionState, DatagramHeader,
    FLAG_MASKED_CHECKSUM, FLOW_REPORT_ONLY_PERIOD_SEC, IsaacCipher, OptionalHeaders,
    SessionAuthHeader, SessionMultiplexer, WireCodecs, decode_datagram, encode_datagram,
    guid_fields_swapped, split_blob,
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

    /// The next sequenced packet, carrying a one-byte blob.
    fn packet(&mut self) -> Vec<u8> {
        let frags =
            split_blob(2, ((self.seq + 1) & 0xFFFF) as u16, 1, 0, None, 0x10, b"x").expect("split");
        self.seq += 1;
        let mask = self.isaac.next_mask();
        encode_datagram(
            DatagramHeader {
                rec_id: ASSIGNED,
                seq_num: self.seq,
                ..DatagramHeader::default()
            },
            &OptionalHeaders::default(),
            &frags,
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
/// reliable client blob is in flight: no loop at the round-trip time. The client ticks every
/// 5 ms and after every arrival, harder than the connector does.
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
    assert!(sent.attached >= 1, "the blob carried the pending report");
    assert!(
        sent.flow_only.len() <= 6,
        "{} FlowControl-only packets in 5 s: {:?}",
        sent.flow_only.len(),
        sent.flow_only
    );
    assert_spaced(&sent.flow_only);
    assert_eq!(sent.flow_only_with_nothing, 0);

    // The acknowledgements stop; the bytes still held go out on the next report, so nothing
    // received is lost to the period.
    let quiet = end + 2.0 * FLOW_REPORT_ONLY_PERIOD_SEC;
    while let Some((at, seq)) = arrivals.pop_front() {
        let ack = server.ack(seq);
        received += ack.len() as u64;
        mux.receive_from_peer(&ack, SERVER, at, WireCodecs::NONE)
            .expect("the last acks");
    }
    let due = mux
        .next_deadline(end)
        .expect("the held report is a deadline");
    assert!(due <= quiet, "{due}");
    mux.tick(quiet).expect("tick");
    collect(&mut mux, id, quiet, &mut sent);
    assert_eq!(sent.reported_bytes, received);
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
