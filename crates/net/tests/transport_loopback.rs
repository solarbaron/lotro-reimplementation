// SPDX-License-Identifier: GPL-3.0-or-later
//! Reliability, fragmentation and hostile-peer tests for the transport,
//! using the in-memory loopback with simulated time.

use std::time::{Duration, Instant};

use proptest::prelude::*;
use tessera_net::NetError;
use tessera_net::transport::endpoint::INBOX_ENTRY_COST;
use tessera_net::transport::packet::{Ack, Body, MAX_DATAGRAM_SIZE, Packet, Segment};
use tessera_net::transport::{
    Datagram, EndpointState, LoopbackDatagram, LossConfig, LossyDatagram, ReliableEndpoint,
    TransportConfig,
};

const MTU: usize = 1200;
const STEP: Duration = Duration::from_millis(5);

/// Deterministic message `i` of length `len`.
fn message(i: usize, len: usize) -> Vec<u8> {
    (0..len)
        .map(|j| (j.wrapping_mul(31) ^ i.wrapping_mul(131)) as u8)
        .collect()
}

/// Message lengths covering empty, tiny, exact fragment boundaries and
/// large multi-fragment messages.
fn lengths(frag: usize) -> Vec<usize> {
    let mut v = vec![
        0,
        1,
        frag - 1,
        frag,
        frag + 1,
        2 * frag,
        3 * frag + 7,
        40_000,
    ];
    let mut x = 0x1234_5678u32;
    for _ in 0..40 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        v.push((x % 30_000) as usize);
    }
    v
}

type Lossy = LossyDatagram<LoopbackDatagram>;

fn lossy_pair(
    loss: LossConfig,
    cfg: &TransportConfig,
    t0: Instant,
) -> (ReliableEndpoint<Lossy>, ReliableEndpoint<Lossy>) {
    let (a, b) = LoopbackDatagram::pair_with_capacity(MTU, 1 << 20);
    let a = LossyDatagram::new(a, loss);
    let b = LossyDatagram::new(
        b,
        LossConfig {
            seed: loss.seed ^ 0xDEAD_BEEF,
            ..loss
        },
    );
    (
        ReliableEndpoint::new(a, cfg.clone(), t0).unwrap(),
        ReliableEndpoint::new(b, cfg.clone(), t0).unwrap(),
    )
}

/// Sends `lengths` messages each way and runs until both sides received all
/// of them. Returns the simulated time taken.
fn exchange<D: Datagram>(
    a: &mut ReliableEndpoint<D>,
    b: &mut ReliableEndpoint<D>,
    t0: Instant,
    limit: Duration,
) -> Duration {
    let lens = lengths(a.fragment_payload_size());
    let a_msgs: Vec<_> = lens
        .iter()
        .enumerate()
        .map(|(i, &l)| message(i, l))
        .collect();
    let b_msgs: Vec<_> = lens
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &l)| message(i + 1000, l))
        .collect();
    for m in &a_msgs {
        a.send(m).unwrap();
    }
    for m in &b_msgs {
        b.send(m).unwrap();
    }

    let (mut got_a, mut got_b) = (Vec::new(), Vec::new());
    let mut now = t0;
    while got_a.len() < b_msgs.len() || got_b.len() < a_msgs.len() || !a.is_flushed() {
        assert!(now - t0 < limit, "did not converge within {limit:?}");
        now += STEP;
        a.poll(now).unwrap();
        b.poll(now).unwrap();
        while let Some(m) = a.recv() {
            assert!(m.reliable);
            got_a.push(m.payload);
        }
        while let Some(m) = b.recv() {
            assert!(m.reliable);
            got_b.push(m.payload);
        }
    }
    assert_eq!(got_b, a_msgs, "a->b stream corrupted or reordered");
    assert_eq!(got_a, b_msgs, "b->a stream corrupted or reordered");
    now - t0
}

#[test]
fn reliable_ordered_under_30_percent_loss_with_fragmentation() {
    let t0 = Instant::now();
    let cfg = TransportConfig {
        idle_timeout: Duration::from_secs(60),
        ..TransportConfig::default()
    };
    for seed in [1u64, 2, 3] {
        let (mut a, mut b) = lossy_pair(LossConfig::drop_only(0.3, seed), &cfg, t0);
        let took = exchange(&mut a, &mut b, t0, Duration::from_secs(600));
        assert!(a.datagram().dropped() > 0 && b.datagram().dropped() > 0);
        assert!(a.stats().retransmits > 0, "loss must force retransmits");
        assert_eq!(a.state(), EndpointState::Open);
        assert!(a.srtt().is_some());
        eprintln!(
            "seed {seed}: {took:?} simulated, a: {:?}, b dropped {}",
            a.stats(),
            b.datagram().dropped()
        );
    }
}

#[test]
fn survives_loss_duplication_and_reordering() {
    let t0 = Instant::now();
    let cfg = TransportConfig {
        idle_timeout: Duration::from_secs(60),
        ..TransportConfig::default()
    };
    let loss = LossConfig {
        drop: 0.3,
        duplicate: 0.2,
        reorder: 0.2,
        seed: 99,
    };
    let (mut a, mut b) = lossy_pair(loss, &cfg, t0);
    exchange(&mut a, &mut b, t0, Duration::from_secs(600));
    assert!(a.stats().duplicates + b.stats().duplicates > 0);
}

#[test]
fn small_windows_and_sequence_wraparound() {
    let t0 = Instant::now();
    let cfg = TransportConfig {
        initial_seq: u32::MAX - 20,
        send_window: 4,
        recv_window: 8,
        idle_timeout: Duration::from_secs(60),
        ..TransportConfig::default()
    };
    let (mut a, mut b) = lossy_pair(LossConfig::drop_only(0.3, 7), &cfg, t0);
    exchange(&mut a, &mut b, t0, Duration::from_secs(3600));
}

#[test]
fn unreliable_messages_are_delivered_unfragmented() {
    let t0 = Instant::now();
    let (a, b) = LoopbackDatagram::pair(MTU);
    let mut a = ReliableEndpoint::new(a, TransportConfig::default(), t0).unwrap();
    let mut b = ReliableEndpoint::new(b, TransportConfig::default(), t0).unwrap();
    let max = a.max_unreliable_size();
    assert!(matches!(
        a.send_unreliable(&vec![0; max + 1]),
        Err(NetError::MessageTooLarge { .. })
    ));
    a.send_unreliable(&vec![7; max]).unwrap();
    a.send(b"reliable").unwrap();
    a.poll(t0).unwrap();
    b.poll(t0).unwrap();
    let mut got = Vec::new();
    while let Some(m) = b.recv() {
        got.push((m.reliable, m.payload.len()));
    }
    got.sort_unstable();
    assert_eq!(got, vec![(false, max), (true, 8)]);
}

#[test]
fn inbox_backpressure_pauses_then_resumes() {
    let t0 = Instant::now();
    let cfg = TransportConfig {
        max_inbox_bytes: 4096,
        ..TransportConfig::default()
    };
    let (a, b) = LoopbackDatagram::pair(MTU);
    let mut a = ReliableEndpoint::new(a, cfg.clone(), t0).unwrap();
    let mut b = ReliableEndpoint::new(b, cfg, t0).unwrap();
    for i in 0..50 {
        a.send(&message(i, 1000)).unwrap();
    }
    let mut now = t0;
    // Receiver does not drain: delivery stalls, nothing is lost.
    for _ in 0..200 {
        now += STEP;
        a.poll(now).unwrap();
        b.poll(now).unwrap();
    }
    assert!(!a.is_flushed(), "sender must be held back");
    let mut got = Vec::new();
    while got.len() < 50 {
        now += STEP;
        a.poll(now).unwrap();
        b.poll(now).unwrap();
        while let Some(m) = b.recv() {
            got.push(m.payload);
        }
        assert!(now - t0 < Duration::from_secs(120));
    }
    assert_eq!(got, (0..50).map(|i| message(i, 1000)).collect::<Vec<_>>());
}

#[test]
fn send_limits() {
    let t0 = Instant::now();
    let cfg = TransportConfig {
        max_message_size: 10_000,
        max_send_queue_bytes: 25_000,
        ..TransportConfig::default()
    };
    let (a, _b) = LoopbackDatagram::pair(MTU);
    let mut a = ReliableEndpoint::new(a, cfg, t0).unwrap();
    assert!(matches!(
        a.send(&[0; 10_001]),
        Err(NetError::MessageTooLarge { len: 10_001, .. })
    ));
    a.send(&[0; 10_000]).unwrap();
    a.send(&[0; 10_000]).unwrap();
    assert!(matches!(a.send(&[0; 10_000]), Err(NetError::SendQueueFull)));
}

#[test]
fn rejects_invalid_config() {
    let t0 = Instant::now();
    let bad = [
        TransportConfig {
            max_datagram_size: 10,
            ..TransportConfig::default()
        },
        TransportConfig {
            send_window: 0,
            ..TransportConfig::default()
        },
        TransportConfig {
            min_rto: Duration::from_secs(1),
            initial_rto: Duration::from_millis(1),
            ..TransportConfig::default()
        },
        TransportConfig {
            keepalive_interval: Duration::from_secs(100),
            ..TransportConfig::default()
        },
    ];
    for cfg in bad {
        let (a, _b) = LoopbackDatagram::pair(MTU);
        assert!(matches!(
            ReliableEndpoint::new(a, cfg, t0),
            Err(NetError::InvalidConfig(_))
        ));
    }
}

#[test]
fn unresponsive_peer_fails_after_retransmit_budget() {
    let t0 = Instant::now();
    let cfg = TransportConfig {
        max_retransmits: 3,
        idle_timeout: Duration::from_secs(600),
        ..TransportConfig::default()
    };
    let (a, _silent) = LoopbackDatagram::pair(MTU);
    let mut a = ReliableEndpoint::new(a, cfg, t0).unwrap();
    a.send(b"anyone?").unwrap();
    let mut now = t0;
    let err = loop {
        now += STEP;
        if let Err(e) = a.poll(now) {
            break e;
        }
        assert!(now - t0 < Duration::from_secs(60));
    };
    assert!(matches!(err, NetError::PeerUnresponsive), "{err:?}");
    assert_eq!(a.stats().retransmits, 3);
    // Exponential backoff: 250 + 500 + 1000 + 2000 ms before giving up.
    assert!(now - t0 >= Duration::from_millis(3750));
    assert_eq!(a.state(), EndpointState::Failed);
    assert!(matches!(a.send(b"x"), Err(NetError::PeerUnresponsive)));
}

#[test]
fn idle_timeout() {
    let t0 = Instant::now();
    let (a, _silent) = LoopbackDatagram::pair(MTU);
    let mut a = ReliableEndpoint::new(a, TransportConfig::default(), t0).unwrap();
    a.poll(t0 + Duration::from_secs(14)).unwrap();
    assert!(matches!(
        a.poll(t0 + Duration::from_secs(15)),
        Err(NetError::TimedOut)
    ));
}

#[test]
fn keepalives_prevent_idle_timeout() {
    let t0 = Instant::now();
    let (a, b) = LoopbackDatagram::pair(MTU);
    let mut a = ReliableEndpoint::new(a, TransportConfig::default(), t0).unwrap();
    let mut b = ReliableEndpoint::new(b, TransportConfig::default(), t0).unwrap();
    let mut now = t0;
    for _ in 0..600 {
        now += Duration::from_millis(100);
        a.poll(now).unwrap();
        b.poll(now).unwrap();
    }
    assert!(a.stats().packets_sent > 0 && a.stats().packets_sent < 100);
}

#[test]
fn close_reaches_peer() {
    let t0 = Instant::now();
    let (a, b) = LoopbackDatagram::pair(MTU);
    let mut a = ReliableEndpoint::new(a, TransportConfig::default(), t0).unwrap();
    let mut b = ReliableEndpoint::new(b, TransportConfig::default(), t0).unwrap();
    a.send(b"last words").unwrap();
    a.poll(t0).unwrap();
    a.close(t0);
    assert_eq!(a.state(), EndpointState::Closed { by_peer: false });
    assert!(matches!(b.poll(t0), Err(NetError::Closed)));
    assert_eq!(b.state(), EndpointState::Closed { by_peer: true });
    assert_eq!(b.recv().unwrap().payload, b"last words");
}

// ---------------------------------------------------------------------------
// Hostile peer: hand-crafted, correctly checksummed packets.

struct Attacker {
    raw: LoopbackDatagram,
    victim: ReliableEndpoint<LoopbackDatagram>,
    t0: Instant,
}

impl Attacker {
    fn new(cfg: TransportConfig) -> Self {
        Self::with_link_mtu(cfg, MTU)
    }

    /// The link carries datagrams up to `mtu` bytes, whatever the victim's
    /// own `max_datagram_size` is.
    fn with_link_mtu(cfg: TransportConfig, mtu: usize) -> Self {
        let t0 = Instant::now();
        let (raw, v) = LoopbackDatagram::pair(mtu);
        Self {
            raw,
            victim: ReliableEndpoint::new(v, cfg, t0).unwrap(),
            t0,
        }
    }

    fn packet(&mut self, ack: Ack, body: Body<'_>) {
        let mut buf = Vec::new();
        Packet {
            conn_id: 0,
            ack,
            close: false,
            body,
        }
        .encode(&mut buf)
        .unwrap();
        self.raw.send(&buf).unwrap();
    }

    fn ack(&mut self, next: u32, bits: u32) {
        self.packet(
            Ack {
                next,
                bits,
                paused: false,
            },
            Body::Empty,
        );
    }

    /// Reads everything the victim sent; returns the reliable seqs.
    fn sent_seqs(&mut self) -> Vec<u32> {
        let mut buf = vec![0u8; MAX_DATAGRAM_SIZE];
        let mut seqs = Vec::new();
        while let Some(n) = self.raw.recv(&mut buf).unwrap() {
            if let Body::Reliable(s) = Packet::decode(&buf[..n]).unwrap().body {
                seqs.push(s.seq);
            }
        }
        seqs
    }

    fn segment(&mut self, seq: u32, msg_id: u32, idx: u16, count: u16, payload: &[u8]) {
        let mut buf = Vec::new();
        Packet {
            conn_id: 0,
            ack: Ack::default(),
            close: false,
            body: Body::Reliable(Segment {
                seq,
                msg_id,
                frag_index: idx,
                frag_count: count,
                payload,
            }),
        }
        .encode(&mut buf)
        .unwrap();
        self.raw.send(&buf).unwrap();
    }

    fn poll(&mut self) -> Result<(), NetError> {
        self.victim.poll(self.t0)
    }
}

fn small_limits() -> TransportConfig {
    TransportConfig {
        max_message_size: 3000,
        max_fragments: 4,
        recv_window: 16,
        ..TransportConfig::default()
    }
}

#[test]
fn rejects_fragment_count_above_limit() {
    let mut x = Attacker::new(small_limits());
    x.segment(0, 0, 0, 5, b"a");
    assert!(matches!(x.poll(), Err(NetError::ProtocolViolation(_))));
    // The victim tells the peer it is going away.
    let mut buf = vec![0u8; MTU];
    let mut saw_close = false;
    while let Some(n) = x.raw.recv(&mut buf).unwrap() {
        saw_close |= Packet::decode(&buf[..n]).unwrap().close;
    }
    assert!(saw_close);
}

#[test]
fn rejects_reassembly_above_size_limit() {
    let mut x = Attacker::new(small_limits());
    let chunk = vec![0u8; 1000];
    for i in 0..4u16 {
        x.segment(u32::from(i), 0, i, 4, &chunk);
    }
    assert!(matches!(x.poll(), Err(NetError::ProtocolViolation(_))));
    assert!(x.victim.recv().is_none());
}

#[test]
fn rejects_inconsistent_fragments() {
    let mut x = Attacker::new(small_limits());
    x.segment(0, 0, 0, 2, b"a");
    x.segment(1, 9, 1, 2, b"b"); // different msg_id
    assert!(matches!(x.poll(), Err(NetError::ProtocolViolation(_))));

    let mut x = Attacker::new(small_limits());
    x.segment(0, 0, 1, 2, b"b"); // does not start at 0
    assert!(matches!(x.poll(), Err(NetError::ProtocolViolation(_))));

    let mut x = Attacker::new(small_limits());
    x.segment(0, 0, 0, 3, b"a");
    x.segment(1, 0, 2, 3, b"c"); // skips index 1
    assert!(matches!(x.poll(), Err(NetError::ProtocolViolation(_))));
}

#[test]
fn reassembles_out_of_order_fragments() {
    let mut x = Attacker::new(small_limits());
    x.segment(2, 0, 2, 3, b"c");
    x.segment(0, 0, 0, 3, b"a");
    x.segment(2, 0, 2, 3, b"c"); // duplicate
    x.poll().unwrap();
    assert!(x.victim.recv().is_none(), "incomplete message delivered");
    x.segment(1, 0, 1, 3, b"b");
    x.poll().unwrap();
    assert_eq!(x.victim.recv().unwrap().payload, b"abc");
    assert_eq!(x.victim.stats().duplicates, 1);
}

#[test]
fn ignores_garbage_foreign_and_out_of_window_packets() {
    let mut x = Attacker::new(small_limits());
    x.raw.send(b"definitely not a packet").unwrap();
    x.raw.send(&[]).unwrap();
    let mut foreign = Vec::new();
    Packet {
        conn_id: 77,
        ack: Ack::default(),
        close: false,
        body: Body::Unreliable(b"hi"),
    }
    .encode(&mut foreign)
    .unwrap();
    x.raw.send(&foreign).unwrap();
    x.segment(16, 0, 0, 1, b"too far ahead"); // window is 16
    x.segment(u32::MAX, 0, 0, 1, b"behind");
    x.poll().unwrap();
    let s = x.victim.stats();
    assert_eq!(s.malformed, 2);
    assert_eq!(s.foreign, 1);
    assert_eq!(s.out_of_window, 1);
    assert_eq!(s.duplicates, 1);
    assert!(x.victim.recv().is_none());
    assert_eq!(x.victim.state(), EndpointState::Open);
}

#[test]
fn ignores_acks_for_unsent_data() {
    let mut x = Attacker::new(TransportConfig::default());
    let mut buf = Vec::new();
    Packet {
        conn_id: 0,
        ack: Ack {
            next: 1000,
            bits: u32::MAX,
            paused: false,
        },
        close: false,
        body: Body::Empty,
    }
    .encode(&mut buf)
    .unwrap();
    x.victim.send(b"queued").unwrap();
    x.poll().unwrap(); // sends seq 0
    x.raw.send(&buf).unwrap();
    x.poll().unwrap();
    // Seq 0 was really sent, so the cumulative ack covers it; the claims on
    // 1..=1032 are bogus and counted.
    assert!(x.victim.is_flushed());
    assert_eq!(x.victim.stats().invalid_acks, 1);
    x.victim.send(b"more").unwrap(); // seq 1, not yet sent
    x.raw.send(&buf).unwrap();
    x.victim.poll(x.t0).unwrap();
    // The bogus ack arrives before seq 1 is first sent: it is ignored.
    assert_eq!(x.victim.stats().invalid_acks, 2);
    assert!(!x.victim.is_flushed());
}

#[test]
fn bogus_acks_are_counted_even_with_nothing_queued() {
    let mut x = Attacker::new(TransportConfig::default());
    x.ack(1000, u32::MAX);
    x.poll().unwrap();
    assert_eq!(
        x.victim.stats().invalid_acks,
        1,
        "cumulative ack beyond sent"
    );
    x.ack(0, 0b1);
    x.poll().unwrap();
    assert_eq!(
        x.victim.stats().invalid_acks,
        2,
        "selective ack of unsent seq"
    );

    x.victim.send(b"a").unwrap();
    x.victim.send(b"b").unwrap();
    x.poll().unwrap(); // sends seqs 0 and 1
    assert_eq!(x.sent_seqs(), [0, 1]);
    x.ack(0, 0b1); // acks seq 1 only: legitimate
    x.ack(2, 0); // acks both: legitimate
    x.poll().unwrap();
    assert_eq!(x.victim.stats().invalid_acks, 2);
    assert!(x.victim.is_flushed());
    x.ack(2, 0b1); // claims seq 3, never sent
    x.poll().unwrap();
    assert_eq!(x.victim.stats().invalid_acks, 3);
}

#[test]
fn send_window_is_a_sequence_span_from_oldest_unacked() {
    let mut x = Attacker::new(TransportConfig {
        send_window: 4,
        ..TransportConfig::default()
    });
    for i in 0..8u8 {
        x.victim.send(&[i]).unwrap();
    }
    x.poll().unwrap();
    assert_eq!(x.sent_seqs(), [0, 1, 2, 3]);
    // 1..=3 are selectively acked, 0 is not: the window still ends at 3,
    // so the sender stays inside a peer receive window of the same size.
    x.ack(0, 0b111);
    x.poll().unwrap();
    assert_eq!(x.sent_seqs(), Vec::<u32>::new());
    x.ack(4, 0);
    x.poll().unwrap();
    assert_eq!(x.sent_seqs(), [4, 5, 6, 7]);
}

/// Regression: a receiver whose application stops draining for longer than
/// the whole retransmission budget (~40 s at the defaults) used to make the
/// sender give up with `PeerUnresponsive`, losing every buffered message.
#[test]
fn backpressure_survives_stall_longer_than_retransmit_budget() {
    let t0 = Instant::now();
    let cfg = TransportConfig {
        max_inbox_bytes: 4096,
        ..TransportConfig::default()
    };
    let (a, b) = LoopbackDatagram::pair(MTU);
    let mut a = ReliableEndpoint::new(a, cfg.clone(), t0).unwrap();
    let mut b = ReliableEndpoint::new(b, cfg, t0).unwrap();
    for i in 0..50 {
        a.send(&message(i, 1000)).unwrap();
    }
    let frame = Duration::from_millis(16);
    let mut now = t0;
    let mut retransmits_at_60s = 0;
    while now - t0 < Duration::from_secs(120) {
        now += frame;
        a.poll(now).unwrap();
        b.poll(now).unwrap();
        if retransmits_at_60s == 0 && now - t0 >= Duration::from_secs(60) {
            retransmits_at_60s = a.stats().retransmits;
        }
    }
    assert!(!a.is_flushed());
    assert!(b.inbox_bytes() >= 4096);
    assert_eq!(
        a.stats().retransmits,
        retransmits_at_60s,
        "a paused peer must not be retransmitted to"
    );
    let mut got = Vec::new();
    while got.len() < 50 {
        now += frame;
        a.poll(now).unwrap();
        b.poll(now).unwrap();
        while let Some(m) = b.recv() {
            got.push(m.payload);
        }
        assert!(now - t0 < Duration::from_secs(240), "did not resume");
    }
    assert_eq!(got, (0..50).map(|i| message(i, 1000)).collect::<Vec<_>>());
}

/// A peer that goes silent while paused is still detected, by idle timeout.
#[test]
fn paused_peer_that_goes_silent_times_out() {
    let mut x = Attacker::new(TransportConfig::default());
    x.victim.send(b"held").unwrap();
    x.poll().unwrap();
    x.packet(
        Ack {
            next: 0,
            bits: 0,
            paused: true,
        },
        Body::Empty,
    );
    let mut now = x.t0;
    let err = loop {
        now += Duration::from_millis(50);
        if let Err(e) = x.victim.poll(now) {
            break e;
        }
        assert!(now - x.t0 < Duration::from_secs(60));
    };
    assert!(matches!(err, NetError::TimedOut), "{err:?}");
    assert_eq!(x.victim.stats().retransmits, 0);
}

#[test]
fn inbox_budget_bounds_floods_of_empty_messages() {
    let budget = 64 * 1024;
    let mut x = Attacker::new(TransportConfig {
        max_inbox_bytes: budget,
        ..TransportConfig::default()
    });
    for _ in 0..8 {
        for _ in 0..4000 {
            x.packet(Ack::default(), Body::Unreliable(&[]));
        }
        x.poll().unwrap();
    }
    assert!(x.victim.inbox_bytes() <= budget);
    assert!(x.victim.stats().unreliable_dropped > 0);
    let mut n = 0;
    while x.victim.recv().is_some() {
        n += 1;
    }
    assert!(n <= budget / INBOX_ENTRY_COST, "{n} empty messages queued");
    assert_eq!(x.victim.inbox_bytes(), 0);

    // Empty reliable messages: delivery pauses at the budget.
    let mut seq = 0u32;
    for _ in 0..40 {
        for _ in 0..256 {
            x.segment(seq, seq, 0, 1, &[]);
            seq += 1;
        }
        x.poll().unwrap();
    }
    assert!(x.victim.inbox_bytes() <= budget + INBOX_ENTRY_COST);
    let mut n = 0;
    while x.victim.recv().is_some() {
        n += 1;
    }
    assert!(
        n <= budget / INBOX_ENTRY_COST + 1,
        "{n} empty messages queued"
    );
}

#[test]
fn reorder_buffer_is_bounded_by_bytes_not_peer_mtu() {
    let budget = 256 * 1024;
    let mut x = Attacker::with_link_mtu(
        TransportConfig {
            max_reorder_bytes: budget,
            ..TransportConfig::default()
        },
        MAX_DATAGRAM_SIZE,
    );
    assert_eq!(x.victim.fragment_payload_size(), 1166);
    let big = |i: u32| vec![i as u8; 60_000];
    // Everything but seq 0: the hole is never filled.
    for seq in 1..64 {
        x.segment(seq, seq, 0, 1, &big(seq));
    }
    x.poll().unwrap();
    assert!(x.victim.reorder_bytes() <= budget);
    assert_eq!(x.victim.stats().reorder_dropped, 59);
    assert_eq!(x.victim.state(), EndpointState::Open);

    // Once the hole is filled, the peer's retransmissions make progress.
    let mut got = Vec::new();
    for _round in 0..100 {
        for seq in 0..64 {
            x.segment(seq, seq, 0, 1, &big(seq));
        }
        x.poll().unwrap();
        while let Some(m) = x.victim.recv() {
            got.push(m.payload);
        }
        assert!(x.victim.reorder_bytes() <= budget + 60_000 + 32);
        if got.len() == 64 {
            break;
        }
    }
    assert_eq!(got, (0..64).map(big).collect::<Vec<_>>());
}

#[test]
fn rejects_unusable_budgets() {
    let t0 = Instant::now();
    for cfg in [
        TransportConfig {
            max_inbox_bytes: 0,
            ..TransportConfig::default()
        },
        TransportConfig {
            max_send_queue_bytes: 0,
            ..TransportConfig::default()
        },
        // The default 256 KiB message could never be queued.
        TransportConfig {
            max_send_queue_bytes: 1000,
            ..TransportConfig::default()
        },
    ] {
        let (a, _b) = LoopbackDatagram::pair(MTU);
        assert!(matches!(
            ReliableEndpoint::new(a, cfg, t0),
            Err(NetError::InvalidConfig(_))
        ));
    }
}

/// One step of a hostile peer: raw bytes or a well-formed packet with
/// arbitrary field values.
#[derive(Debug, Clone)]
enum HostileOp {
    Garbage(Vec<u8>),
    Packet {
        ack: (u32, u32, bool),
        close: bool,
        body: u8,
        seq: u32,
        msg_id: u32,
        frag: (u16, u16),
        len: usize,
    },
    Poll(u16),
}

fn hostile_op() -> impl Strategy<Value = HostileOp> {
    prop_oneof![
        proptest::collection::vec(any::<u8>(), 0..64).prop_map(HostileOp::Garbage),
        (
            (0u32..80, any::<u32>(), any::<bool>()),
            proptest::bool::weighted(0.02),
            0u8..3,
            0u32..80,
            0u32..4,
            (0u16..6, 1u16..6),
            0usize..3000,
        )
            .prop_map(|(ack, close, body, seq, msg_id, (idx, count), len)| {
                HostileOp::Packet {
                    ack,
                    close,
                    body,
                    seq,
                    msg_id,
                    frag: (idx.min(count - 1), count),
                    len,
                }
            }),
        (0u16..2000).prop_map(HostileOp::Poll),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// In-crate stand-in for an endpoint fuzz target: arbitrary datagram
    /// sequences never panic the endpoint and never push its buffers past
    /// their budgets.
    #[test]
    fn hostile_datagram_sequences_stay_bounded(
        ops in proptest::collection::vec(hostile_op(), 0..80)
    ) {
        let cfg = TransportConfig {
            max_message_size: 4000,
            max_fragments: 4,
            recv_window: 64,
            max_inbox_bytes: 8 * 1024,
            max_reorder_bytes: 8 * 1024,
            max_retransmits: 3,
            idle_timeout: Duration::from_secs(5),
            ..TransportConfig::default()
        };
        let mut x = Attacker::with_link_mtu(cfg.clone(), 4096);
        for i in 0..6 {
            x.victim.send(&message(i, 700 * i)).unwrap();
        }
        let payload = vec![0xA5u8; 3000];
        let mut now = x.t0;
        for op in ops {
            match op {
                HostileOp::Garbage(bytes) => x.raw.send(&bytes).unwrap(),
                HostileOp::Packet { ack, close, body, seq, msg_id, frag, len } => {
                    let body = match body {
                        0 => Body::Empty,
                        1 => Body::Unreliable(&payload[..len]),
                        _ => Body::Reliable(Segment {
                            seq,
                            msg_id,
                            frag_index: frag.0,
                            frag_count: frag.1,
                            payload: &payload[..len],
                        }),
                    };
                    let mut buf = Vec::new();
                    Packet {
                        conn_id: 0,
                        ack: Ack { next: ack.0, bits: ack.1, paused: ack.2 },
                        close,
                        body,
                    }
                    .encode(&mut buf)
                    .unwrap();
                    x.raw.send(&buf).unwrap();
                }
                HostileOp::Poll(ms) => {
                    now += Duration::from_millis(u64::from(ms));
                    let _ = x.victim.poll(now);
                    let _ = x.victim.recv();
                }
            }
            prop_assert!(
                x.victim.inbox_bytes()
                    <= cfg.max_inbox_bytes + cfg.max_message_size + INBOX_ENTRY_COST
            );
            prop_assert!(x.victim.reorder_bytes() <= cfg.max_reorder_bytes + 3000 + 32);
        }
        let _ = x.victim.poll(now + Duration::from_secs(1));
        prop_assert!(x.victim.reorder_bytes() <= cfg.max_reorder_bytes + 3000 + 32);
    }
}

/// Runs `a` and `b` in 5 ms steps until `b` has received `count` messages,
/// returning them and the simulated time taken.
fn drain_until(
    a: &mut ReliableEndpoint<LoopbackDatagram>,
    b: &mut ReliableEndpoint<LoopbackDatagram>,
    t0: Instant,
    count: usize,
    limit: Duration,
) -> (Vec<Vec<u8>>, Duration) {
    let mut got = Vec::new();
    let mut now = t0;
    while got.len() < count {
        assert!(now - t0 < limit, "only {} of {count} arrived", got.len());
        now += STEP;
        a.poll(now).unwrap();
        b.poll(now).unwrap();
        while let Some(m) = b.recv() {
            got.push(m.payload);
        }
    }
    (got, now - t0)
}

/// Regression: segments arriving in sequence within one poll were charged to
/// the reorder budget (only the first of a burst counted as "next"), so a
/// lossless link with `max_reorder_bytes: 0` dropped every fragment but the
/// first and recovered one RTO at a time (18.8 s for 10 fragments).
#[test]
fn in_order_burst_is_not_charged_to_reorder_budget() {
    let t0 = Instant::now();
    let cfg = TransportConfig {
        max_reorder_bytes: 0,
        ..TransportConfig::default()
    };
    let (a, b) = LoopbackDatagram::pair(MTU);
    let mut a = ReliableEndpoint::new(a, cfg.clone(), t0).unwrap();
    let mut b = ReliableEndpoint::new(b, cfg, t0).unwrap();
    let msg = message(7, 10 * a.fragment_payload_size());
    a.send(&msg).unwrap();
    let (got, took) = drain_until(&mut a, &mut b, t0, 1, Duration::from_secs(60));
    assert_eq!(got, vec![msg]);
    assert_eq!(b.stats().reorder_dropped, 0);
    assert_eq!(a.stats().retransmits, 0);
    assert!(took <= 2 * STEP, "took {took:?}");
}

/// Regression for the same bug with the default reorder budget and a large
/// (legal) datagram size: bursts of 64 KiB segments overflowed the 1 MiB
/// budget although nothing was out of order.
#[test]
fn large_datagrams_on_lossless_link_never_retransmit() {
    let t0 = Instant::now();
    let mtu = MAX_DATAGRAM_SIZE;
    let cfg = TransportConfig {
        max_datagram_size: mtu,
        ..TransportConfig::default()
    };
    let (a, b) = LoopbackDatagram::pair(mtu);
    let mut a = ReliableEndpoint::new(a, cfg.clone(), t0).unwrap();
    let mut b = ReliableEndpoint::new(b, cfg.clone(), t0).unwrap();
    let msgs: Vec<_> = (0..16).map(|i| message(i, cfg.max_message_size)).collect();
    for m in &msgs {
        a.send(m).unwrap();
    }
    let (got, _) = drain_until(&mut a, &mut b, t0, msgs.len(), Duration::from_secs(60));
    assert_eq!(got, msgs);
    assert_eq!(b.stats().reorder_dropped, 0);
    assert_eq!(a.stats().retransmits, 0);
}

/// Regression: segments the peer held back while PAUSED were later taken as
/// RTT samples covering the whole stall, pinning the RTO at `max_rto`.
#[test]
fn paused_hold_is_not_an_rtt_sample() {
    let t0 = Instant::now();
    let cfg = TransportConfig {
        max_inbox_bytes: 4096,
        ..TransportConfig::default()
    };
    let (a, b) = LoopbackDatagram::pair(MTU);
    let mut a = ReliableEndpoint::new(a, cfg.clone(), t0).unwrap();
    let mut b = ReliableEndpoint::new(b, cfg, t0).unwrap();
    for i in 0..50 {
        a.send(&message(i, 1000)).unwrap();
    }
    let frame = Duration::from_millis(16);
    let mut now = t0;
    while now - t0 < Duration::from_secs(120) {
        now += frame;
        a.poll(now).unwrap();
        b.poll(now).unwrap();
    }
    let before = a.srtt().expect("sampled before the stall");
    let mut got = 0;
    while got < 50 || !a.is_flushed() {
        now += frame;
        a.poll(now).unwrap();
        b.poll(now).unwrap();
        while b.recv().is_some() {
            got += 1;
        }
        assert!(now - t0 < Duration::from_secs(240), "did not resume");
    }
    let after = a.srtt().unwrap();
    assert!(
        after <= before.max(Duration::from_millis(100)),
        "srtt {before:?} -> {after:?}"
    );
    assert!(a.rto() < Duration::from_millis(500), "rto {:?}", a.rto());
}
