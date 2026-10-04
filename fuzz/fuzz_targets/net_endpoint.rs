// SPDX-License-Identifier: GPL-3.0-or-later
//! Drives a `ReliableEndpoint` with an arbitrary sequence of raw and
//! well-formed (correctly checksummed) hostile datagrams and clock steps.
//! Exercises ack processing, the reorder buffer, reassembly and
//! back-pressure. Must never panic, and the inbox and reorder buffer must
//! stay within their budgets.

#![no_main]

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use libfuzzer_sys::arbitrary::{self, Arbitrary};
use libfuzzer_sys::fuzz_target;
use tessera_net::transport::endpoint::INBOX_ENTRY_COST;
use tessera_net::transport::packet::{Ack, Body, Packet, Segment};
use tessera_net::transport::{Datagram, LoopbackDatagram, ReliableEndpoint, TransportConfig};

const LINK_MTU: usize = 4096;
const MAX_OPS: usize = 512;

#[derive(Debug, Arbitrary)]
enum Op {
    Raw(Vec<u8>),
    Packet {
        ack_next: u16,
        ack_bits: u32,
        paused: bool,
        close: bool,
        body: u8,
        seq: u16,
        msg_id: u8,
        frag_index: u8,
        frag_count: u8,
        len: u16,
    },
    Poll {
        advance_ms: u16,
        drain: u8,
    },
    Send {
        len: u16,
    },
}

fuzz_target!(|ops: Vec<Op>| {
    static T0: OnceLock<Instant> = OnceLock::new();
    let t0 = *T0.get_or_init(Instant::now);
    let cfg = TransportConfig {
        max_message_size: 4000,
        max_fragments: 4,
        recv_window: 64,
        send_window: 16,
        max_inbox_bytes: 8 * 1024,
        max_reorder_bytes: 8 * 1024,
        max_send_queue_bytes: 64 * 1024,
        max_retransmits: 3,
        ..TransportConfig::default()
    };
    let (mut raw, link) = LoopbackDatagram::pair(LINK_MTU);
    let mut ep = ReliableEndpoint::new(link, cfg.clone(), t0).expect("valid config");
    let payload = vec![0x5Au8; LINK_MTU];
    let mut now = t0;
    let mut sink = vec![0u8; LINK_MTU];

    for op in ops.into_iter().take(MAX_OPS) {
        match op {
            Op::Raw(bytes) => {
                let _ = raw.send(&bytes);
            }
            Op::Packet {
                ack_next,
                ack_bits,
                paused,
                close,
                body,
                seq,
                msg_id,
                frag_index,
                frag_count,
                len,
            } => {
                let len = usize::from(len) % 4000;
                let frag_count = u16::from(frag_count.max(1));
                let body = match body % 3 {
                    0 => Body::Empty,
                    1 => Body::Unreliable(&payload[..len]),
                    _ => Body::Reliable(Segment {
                        seq: u32::from(seq),
                        msg_id: u32::from(msg_id),
                        frag_index: u16::from(frag_index) % frag_count,
                        frag_count,
                        payload: &payload[..len],
                    }),
                };
                let mut buf = Vec::new();
                Packet {
                    conn_id: 0,
                    ack: Ack {
                        next: u32::from(ack_next),
                        bits: ack_bits,
                        paused,
                    },
                    close,
                    body,
                }
                .encode(&mut buf)
                .expect("payload fits");
                let _ = raw.send(&buf);
            }
            Op::Poll { advance_ms, drain } => {
                now += Duration::from_millis(u64::from(advance_ms));
                let _ = ep.poll(now);
                for _ in 0..drain {
                    if ep.recv().is_none() {
                        break;
                    }
                }
                // Discard what the endpoint sent so the link never fills.
                while let Ok(Some(_)) = raw.recv(&mut sink) {}
            }
            Op::Send { len } => {
                let _ = ep.send(&payload[..usize::from(len) % 4000]);
            }
        }
        assert!(ep.inbox_bytes() <= cfg.max_inbox_bytes + cfg.max_message_size + INBOX_ENTRY_COST);
        assert!(ep.reorder_bytes() <= cfg.max_reorder_bytes + LINK_MTU + 32);
    }
});
