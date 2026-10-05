// SPDX-License-Identifier: GPL-3.0-or-later
//! The reliable, ordered, fragmenting endpoint on top of a [`Datagram`].
//!
//! See `docs/specs/transport-design.md` for the protocol description; the
//! short version:
//!
//! - Messages sent with [`ReliableEndpoint::send`] are split into segments of
//!   at most one datagram each. Every segment gets a per-direction sequence
//!   number and is retransmitted until acknowledged.
//! - The receiver delivers segments strictly in sequence order and
//!   reassembles messages from consecutive fragments, so at most one partial
//!   message exists at any time.
//! - Every packet carries a cumulative ack plus a 32-bit selective-ack
//!   bitmap. Retransmission timeouts follow the classic smoothed-RTT /
//!   RTT-variance estimator with Karn's rule and per-segment exponential
//!   backoff.
//! - [`ReliableEndpoint::send_unreliable`] sends a single, unfragmented,
//!   unordered datagram for data that is stale once superseded.
//!
//! Everything is driven by [`ReliableEndpoint::poll`], called once per frame
//! with the current time; nothing blocks, nothing spawns threads.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::datagram::Datagram;
use super::packet::{
    Ack, Body, HEADER_LEN, MAX_DATAGRAM_SIZE, MIN_DATAGRAM_SIZE, Packet, SEGMENT_HEADER_LEN,
    Segment, TRAILER_LEN, UNRELIABLE_HEADER_LEN,
};
use crate::NetError;

/// Upper bound on [`TransportConfig::send_window`] and
/// [`TransportConfig::recv_window`].
pub const MAX_WINDOW: u32 = 4096;
/// Bookkeeping overhead charged per queued segment against
/// [`TransportConfig::max_send_queue_bytes`] and
/// [`TransportConfig::max_reorder_bytes`], so empty segments are not free.
const SEGMENT_QUEUE_COST: usize = 32;
/// Bookkeeping overhead charged per inbox message against
/// [`TransportConfig::max_inbox_bytes`] (queue slot plus allocation), so a
/// flood of empty messages cannot grow the inbox without bound.
pub const INBOX_ENTRY_COST: usize = 64;
/// At most this many datagrams are read per [`ReliableEndpoint::poll`], so a
/// flood cannot starve the frame.
const MAX_RECV_PER_POLL: usize = 4096;
/// Maximum queued unreliable messages awaiting the next flush.
const MAX_UNRELIABLE_QUEUE: usize = 1024;
/// Close packets are sent this many times (best effort, never acked).
const CLOSE_REPEATS: usize = 3;

/// Tunables for a [`ReliableEndpoint`]. Both peers must agree on
/// `conn_id` and `initial_seq`; the other values are local.
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// Connection identifier; packets with any other id are ignored.
    pub conn_id: u32,
    /// First sequence number used in both directions.
    pub initial_seq: u32,
    /// Largest datagram to send; clamped to the datagram's own limit.
    pub max_datagram_size: usize,
    /// Largest reliable message accepted for sending or reassembly.
    pub max_message_size: usize,
    /// Largest fragment count accepted for sending or reassembly.
    pub max_fragments: u16,
    /// Maximum unacknowledged segments in flight.
    pub send_window: u32,
    /// Maximum distance ahead of the next expected segment that is buffered.
    pub recv_window: u32,
    /// Bound on queued outbound reliable data (payload plus a small
    /// per-segment charge).
    pub max_send_queue_bytes: usize,
    /// Bound on received-but-not-yet-[`recv`](ReliableEndpoint::recv)ed
    /// data, charging payload plus [`INBOX_ENTRY_COST`] per message. When
    /// reached, in-order delivery pauses and every outgoing packet carries
    /// the PAUSED flag, so the peer holds its retransmissions instead of
    /// counting the stall as loss. Nothing is lost however long the
    /// application stalls, as long as both sides keep polling. Unreliable
    /// messages that do not fit are dropped.
    pub max_inbox_bytes: usize,
    /// Bound on reliable segments buffered out of order while waiting for an
    /// earlier one (payload plus 32 bytes per segment). Segments that do not
    /// fit are dropped unacknowledged and retransmitted by the peer later;
    /// the next expected segment is always accepted, so progress is never
    /// blocked. `0` disables out-of-order buffering.
    pub max_reorder_bytes: usize,
    /// RTO before the first RTT sample.
    pub initial_rto: Duration,
    /// Lower RTO clamp.
    pub min_rto: Duration,
    /// Upper RTO clamp (also caps exponential backoff).
    pub max_rto: Duration,
    /// Retransmissions of a single segment, with no new acknowledgement from
    /// the peer in between, before giving up.
    pub max_retransmits: u32,
    /// Fail if nothing at all is received for this long.
    pub idle_timeout: Duration,
    /// Send an empty packet if nothing was sent for this long.
    pub keepalive_interval: Duration,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            conn_id: 0,
            initial_seq: 0,
            max_datagram_size: 1200,
            max_message_size: 256 * 1024,
            max_fragments: 1024,
            send_window: 256,
            recv_window: 256,
            max_send_queue_bytes: 8 * 1024 * 1024,
            max_inbox_bytes: 8 * 1024 * 1024,
            max_reorder_bytes: 1024 * 1024,
            initial_rto: Duration::from_millis(250),
            min_rto: Duration::from_millis(50),
            max_rto: Duration::from_secs(3),
            max_retransmits: 15,
            idle_timeout: Duration::from_secs(15),
            keepalive_interval: Duration::from_secs(1),
        }
    }
}

impl TransportConfig {
    fn validate(&self) -> Result<(), NetError> {
        let bad = NetError::InvalidConfig;
        if !(MIN_DATAGRAM_SIZE..=MAX_DATAGRAM_SIZE).contains(&self.max_datagram_size) {
            return Err(bad("max_datagram_size out of range"));
        }
        if self.max_message_size > 1 << 30 {
            return Err(bad("max_message_size above 1 GiB"));
        }
        if self.max_fragments == 0 {
            return Err(bad("max_fragments must be at least 1"));
        }
        if !(1..=MAX_WINDOW).contains(&self.send_window)
            || !(1..=MAX_WINDOW).contains(&self.recv_window)
        {
            return Err(bad("window out of range"));
        }
        if self.min_rto.is_zero()
            || self.min_rto > self.initial_rto
            || self.initial_rto > self.max_rto
            || self.max_rto > Duration::from_secs(3600)
        {
            return Err(bad("need 0 < min_rto <= initial_rto <= max_rto <= 1h"));
        }
        if self.max_send_queue_bytes == 0 || self.max_inbox_bytes == 0 {
            return Err(bad("queue and inbox budgets must be non-zero"));
        }
        if self.max_retransmits == 0 {
            return Err(bad("max_retransmits must be at least 1"));
        }
        if self.keepalive_interval.is_zero() || self.keepalive_interval >= self.idle_timeout {
            return Err(bad("need 0 < keepalive_interval < idle_timeout"));
        }
        Ok(())
    }
}

/// Lifecycle of a [`ReliableEndpoint`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointState {
    /// Usable.
    Open,
    /// Closed gracefully.
    Closed {
        /// Whether the peer initiated the close.
        by_peer: bool,
    },
    /// Failed; [`ReliableEndpoint::poll`] reports why.
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Failure {
    PeerUnresponsive,
    TimedOut,
    Violation(&'static str),
}

impl Failure {
    fn to_error(self) -> NetError {
        match self {
            Self::PeerUnresponsive => NetError::PeerUnresponsive,
            Self::TimedOut => NetError::TimedOut,
            Self::Violation(why) => NetError::ProtocolViolation(why),
        }
    }
}

/// A message handed to the application by [`ReliableEndpoint::recv`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Received {
    /// Message bytes.
    pub payload: Vec<u8>,
    /// `true` if sent with [`send`](ReliableEndpoint::send) (in order,
    /// exactly once); `false` for [`send_unreliable`](ReliableEndpoint::send_unreliable).
    pub reliable: bool,
}

/// Counters for diagnostics and tests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransportStats {
    /// Datagrams handed to the datagram layer.
    pub packets_sent: u64,
    /// Valid datagrams received for this connection.
    pub packets_received: u64,
    /// Bytes handed to the datagram layer.
    pub bytes_sent: u64,
    /// Bytes of valid datagrams received.
    pub bytes_received: u64,
    /// Segment retransmissions.
    pub retransmits: u64,
    /// Datagrams that failed to decode.
    pub malformed: u64,
    /// Valid datagrams with a different connection id.
    pub foreign: u64,
    /// Reliable segments received more than once.
    pub duplicates: u64,
    /// Reliable segments dropped for being beyond the receive window.
    pub out_of_window: u64,
    /// Acks that claimed unsent data (ignored).
    pub invalid_acks: u64,
    /// Reliable messages fully delivered to the inbox.
    pub messages_received: u64,
    /// Unreliable messages dropped because the inbox was full.
    pub unreliable_dropped: u64,
    /// Out-of-order reliable segments dropped (unacknowledged) because the
    /// reorder buffer was full.
    pub reorder_dropped: u64,
    /// Datagram-layer send/receive errors (non-fatal).
    pub io_errors: u64,
}

#[derive(Debug)]
struct SentInfo {
    first: Instant,
    last: Instant,
    /// All retransmissions (Karn's rule).
    retransmits: u32,
    /// Outstanding while the peer reported PAUSED, so its acknowledgement
    /// may include an application stall and is no RTT sample.
    held: bool,
    /// Retransmissions with no acknowledgement progress since the previous
    /// send; this is what [`TransportConfig::max_retransmits`] bounds.
    strikes: u32,
    rto: Duration,
}

#[derive(Debug)]
struct OutSegment {
    seq: u32,
    msg_id: u32,
    frag_index: u16,
    frag_count: u16,
    payload: Vec<u8>,
    acked: bool,
    sent: Option<SentInfo>,
}

#[derive(Debug)]
struct InSegment {
    msg_id: u32,
    frag_index: u16,
    frag_count: u16,
    payload: Vec<u8>,
}

#[derive(Debug)]
struct Partial {
    msg_id: u32,
    frag_count: u16,
    next_index: u16,
    buf: Vec<u8>,
}

/// A reliable-datagram connection endpoint; see the module documentation.
#[derive(Debug)]
pub struct ReliableEndpoint<D> {
    datagram: D,
    config: TransportConfig,
    frag_payload: usize,
    unreliable_max: usize,

    // Send side.
    next_seq: u32,
    /// One past the highest sequence number ever transmitted.
    sent_end: u32,
    /// When an acknowledgement last covered a not-yet-acked segment.
    last_progress: Instant,
    /// The peer's most recent packet carried the PAUSED flag.
    peer_paused: bool,
    next_msg_id: u32,
    /// All not-yet-acknowledged segments, contiguous in sequence order.
    /// Sent segments always form a prefix.
    outq: VecDeque<OutSegment>,
    outq_cost: usize,
    unreliable_out: VecDeque<Vec<u8>>,

    // Receive side.
    recv_next: u32,
    /// Slot `i` holds segment `recv_next + i`, if received.
    recv_buf: VecDeque<Option<InSegment>>,
    /// Charged size of the segments in `recv_buf`.
    recv_buf_bytes: usize,
    ack_pending: bool,
    /// PAUSED state carried by the last packet we sent.
    advertised_paused: bool,
    partial: Option<Partial>,
    inbox: VecDeque<Received>,
    inbox_bytes: usize,

    // RTT estimation.
    srtt: Option<Duration>,
    rttvar: Duration,
    rto: Duration,

    last_recv: Instant,
    last_send: Instant,
    state: EndpointState,
    failure: Option<Failure>,
    stats: TransportStats,
    tx_buf: Vec<u8>,
    rx_buf: Vec<u8>,
}

impl<D: Datagram> ReliableEndpoint<D> {
    /// Creates an endpoint. `now` starts the idle and keepalive timers.
    ///
    /// # Errors
    ///
    /// [`NetError::InvalidConfig`] if `config` is out of range or the
    /// datagram layer's limit is below [`MIN_DATAGRAM_SIZE`].
    pub fn new(datagram: D, config: TransportConfig, now: Instant) -> Result<Self, NetError> {
        config.validate()?;
        let mtu = config.max_datagram_size.min(datagram.max_datagram_size());
        if mtu < MIN_DATAGRAM_SIZE {
            return Err(NetError::InvalidConfig(
                "datagram layer limit below minimum",
            ));
        }
        let frag_payload =
            (mtu - HEADER_LEN - SEGMENT_HEADER_LEN - TRAILER_LEN).min(usize::from(u16::MAX));
        let unreliable_max =
            (mtu - HEADER_LEN - UNRELIABLE_HEADER_LEN - TRAILER_LEN).min(usize::from(u16::MAX));
        let largest = config
            .max_message_size
            .min(usize::from(config.max_fragments) * frag_payload);
        if queue_cost(largest, frag_payload) > config.max_send_queue_bytes {
            return Err(NetError::InvalidConfig(
                "max_send_queue_bytes cannot hold the largest allowed message",
            ));
        }
        Ok(Self {
            datagram,
            frag_payload,
            unreliable_max,
            next_seq: config.initial_seq,
            sent_end: config.initial_seq,
            last_progress: now,
            peer_paused: false,
            next_msg_id: 0,
            outq: VecDeque::new(),
            outq_cost: 0,
            unreliable_out: VecDeque::new(),
            recv_next: config.initial_seq,
            recv_buf: VecDeque::new(),
            recv_buf_bytes: 0,
            ack_pending: false,
            advertised_paused: false,
            partial: None,
            inbox: VecDeque::new(),
            inbox_bytes: 0,
            srtt: None,
            rttvar: Duration::ZERO,
            rto: config.initial_rto,
            last_recv: now,
            last_send: now,
            state: EndpointState::Open,
            failure: None,
            stats: TransportStats::default(),
            tx_buf: Vec::with_capacity(mtu),
            rx_buf: vec![0; MAX_DATAGRAM_SIZE],
            config,
        })
    }

    /// Current lifecycle state.
    #[must_use]
    pub fn state(&self) -> EndpointState {
        self.state
    }

    /// Diagnostic counters.
    #[must_use]
    pub fn stats(&self) -> &TransportStats {
        &self.stats
    }

    /// Current retransmission timeout for fresh segments.
    #[must_use]
    pub fn rto(&self) -> Duration {
        self.rto
    }

    /// Smoothed round-trip time, once measured.
    #[must_use]
    pub fn srtt(&self) -> Option<Duration> {
        self.srtt
    }

    /// Largest payload carried by one reliable fragment.
    #[must_use]
    pub fn fragment_payload_size(&self) -> usize {
        self.frag_payload
    }

    /// Largest message accepted by [`send_unreliable`](Self::send_unreliable).
    #[must_use]
    pub fn max_unreliable_size(&self) -> usize {
        self.unreliable_max
    }

    /// True when every reliable segment has been acknowledged and nothing
    /// unreliable is waiting. Wait for this before a graceful
    /// [`close`](Self::close).
    #[must_use]
    pub fn is_flushed(&self) -> bool {
        self.outq.is_empty() && self.unreliable_out.is_empty()
    }

    /// Charged size of the undelivered inbox (payload plus
    /// [`INBOX_ENTRY_COST`] per message); compare with
    /// [`TransportConfig::max_inbox_bytes`].
    #[must_use]
    pub fn inbox_bytes(&self) -> usize {
        self.inbox_bytes
    }

    /// Charged size of reliable segments buffered out of order; compare
    /// with [`TransportConfig::max_reorder_bytes`].
    #[must_use]
    pub fn reorder_bytes(&self) -> usize {
        self.recv_buf_bytes
    }

    /// The underlying datagram layer.
    pub fn datagram(&self) -> &D {
        &self.datagram
    }

    fn check_open(&self) -> Result<(), NetError> {
        match (self.state, self.failure) {
            (EndpointState::Open, _) => Ok(()),
            (_, Some(f)) => Err(f.to_error()),
            _ => Err(NetError::Closed),
        }
    }

    /// Queues a reliable, ordered message. It is transmitted by the next
    /// [`poll`](Self::poll).
    ///
    /// # Errors
    ///
    /// [`NetError::MessageTooLarge`], [`NetError::SendQueueFull`], or the
    /// reason the connection is no longer open.
    pub fn send(&mut self, message: &[u8]) -> Result<(), NetError> {
        self.check_open()?;
        let max_by_frags = usize::from(self.config.max_fragments) * self.frag_payload;
        let max = self.config.max_message_size.min(max_by_frags);
        if message.len() > max {
            return Err(NetError::MessageTooLarge {
                len: message.len(),
                max,
            });
        }
        let frag_count = message.len().div_ceil(self.frag_payload).max(1);
        let frag_count = u16::try_from(frag_count).map_err(|_| NetError::MessageTooLarge {
            len: message.len(),
            max,
        })?;
        let cost = queue_cost(message.len(), self.frag_payload);
        if self.outq_cost + cost > self.config.max_send_queue_bytes {
            return Err(NetError::SendQueueFull);
        }
        let msg_id = self.next_msg_id;
        self.next_msg_id = self.next_msg_id.wrapping_add(1);
        for frag_index in 0..frag_count {
            let start = usize::from(frag_index) * self.frag_payload;
            let end = (start + self.frag_payload).min(message.len());
            self.outq.push_back(OutSegment {
                seq: self.next_seq,
                msg_id,
                frag_index,
                frag_count,
                payload: message[start..end].to_vec(),
                acked: false,
                sent: None,
            });
            self.next_seq = self.next_seq.wrapping_add(1);
        }
        self.outq_cost += cost;
        Ok(())
    }

    /// Queues an unreliable message (no retransmission, no ordering, no
    /// fragmentation). Sent by the next [`poll`](Self::poll).
    ///
    /// # Errors
    ///
    /// [`NetError::MessageTooLarge`] above
    /// [`max_unreliable_size`](Self::max_unreliable_size),
    /// [`NetError::SendQueueFull`], or the reason the connection is closed.
    pub fn send_unreliable(&mut self, message: &[u8]) -> Result<(), NetError> {
        self.check_open()?;
        if message.len() > self.unreliable_max {
            return Err(NetError::MessageTooLarge {
                len: message.len(),
                max: self.unreliable_max,
            });
        }
        if self.unreliable_out.len() >= MAX_UNRELIABLE_QUEUE {
            return Err(NetError::SendQueueFull);
        }
        self.unreliable_out.push_back(message.to_vec());
        Ok(())
    }

    /// Takes the next delivered message, if any. Messages already delivered
    /// remain available after the connection closes or fails.
    pub fn recv(&mut self) -> Option<Received> {
        let msg = self.inbox.pop_front()?;
        self.inbox_bytes -= msg.payload.len() + INBOX_ENTRY_COST;
        Some(msg)
    }

    /// Receives pending datagrams, processes acks, delivers messages,
    /// checks timers, retransmits and sends queued data.
    ///
    /// # Errors
    ///
    /// Once the connection is closed or failed, every call returns why
    /// ([`NetError::Closed`], [`NetError::TimedOut`],
    /// [`NetError::PeerUnresponsive`], [`NetError::ProtocolViolation`]).
    pub fn poll(&mut self, now: Instant) -> Result<(), NetError> {
        self.check_open()?;

        let mut rx = std::mem::take(&mut self.rx_buf);
        for _ in 0..MAX_RECV_PER_POLL {
            match self.datagram.recv(&mut rx) {
                // `min` guards against a Datagram impl reporting too much.
                Ok(Some(n)) => self.on_datagram(&rx[..n.min(rx.len())], now),
                Ok(None) => break,
                Err(e) => {
                    self.stats.io_errors += 1;
                    tracing::debug!(error = %e, "datagram receive error");
                    break;
                }
            }
            if self.state != EndpointState::Open {
                break;
            }
        }
        self.rx_buf = rx;

        self.deliver_in_order(now);
        if self.failure.is_none()
            && self.state == EndpointState::Open
            && now.saturating_duration_since(self.last_recv) >= self.config.idle_timeout
        {
            self.fail(Failure::TimedOut, now);
        }
        if self.state == EndpointState::Open {
            self.flush(now);
        }
        self.check_open()
    }

    /// Closes the connection, telling the peer (best effort). Queued data
    /// that has not been acknowledged is discarded; wait for
    /// [`is_flushed`](Self::is_flushed) first for a graceful close.
    pub fn close(&mut self, now: Instant) {
        if self.state != EndpointState::Open {
            return;
        }
        self.send_close(now);
        self.state = EndpointState::Closed { by_peer: false };
        tracing::debug!(conn_id = self.config.conn_id, "transport closed locally");
    }

    fn send_close(&mut self, now: Instant) {
        let ack = self.current_ack();
        let packet = Packet {
            conn_id: self.config.conn_id,
            ack,
            close: true,
            body: Body::Empty,
        };
        for _ in 0..CLOSE_REPEATS {
            self.transmit(&packet, now);
        }
    }

    fn fail(&mut self, failure: Failure, now: Instant) {
        tracing::warn!(conn_id = self.config.conn_id, ?failure, "transport failed");
        self.send_close(now);
        self.state = EndpointState::Failed;
        self.failure = Some(failure);
    }

    fn on_datagram(&mut self, bytes: &[u8], now: Instant) {
        let packet = match Packet::decode(bytes) {
            Ok(p) => p,
            Err(e) => {
                self.stats.malformed += 1;
                tracing::trace!(error = %e, len = bytes.len(), "dropping malformed datagram");
                return;
            }
        };
        if packet.conn_id != self.config.conn_id {
            self.stats.foreign += 1;
            return;
        }
        self.stats.packets_received += 1;
        self.stats.bytes_received += bytes.len() as u64;
        self.last_recv = now;
        self.on_ack(packet.ack, now);
        match packet.body {
            Body::Empty => {}
            Body::Reliable(seg) => {
                self.on_segment(seg);
                // Advance through contiguous segments as they arrive, so a
                // segment that follows its predecessor within one poll is
                // the next expected one, not an out-of-order one charged to
                // the reorder budget. Back-pressure still applies (see
                // `deliver_in_order`). Spec: transport-design.md §6.4 steps
                // 1 and 3 (`next` is the lowest sequence number not yet
                // received) and the OPEN note in §10.
                self.deliver_in_order(now);
            }
            Body::Unreliable(payload) => {
                let cost = payload.len() + INBOX_ENTRY_COST;
                if self.inbox_bytes + cost <= self.config.max_inbox_bytes {
                    self.inbox_bytes += cost;
                    self.inbox.push_back(Received {
                        payload: payload.to_vec(),
                        reliable: false,
                    });
                } else {
                    self.stats.unreliable_dropped += 1;
                }
            }
        }
        if packet.close {
            tracing::debug!(conn_id = self.config.conn_id, "transport closed by peer");
            // Deliver whatever is already contiguous before going quiet.
            self.deliver_in_order(now);
            if self.state == EndpointState::Open {
                self.state = EndpointState::Closed { by_peer: true };
            }
        }
    }

    fn on_ack(&mut self, ack: Ack, now: Instant) {
        self.peer_paused = ack.paused;
        let (sample, newly_acked) = self.apply_ack(ack, now);
        if ack.paused {
            // Whatever is still unacknowledged may now be held back by the
            // peer's application, not the network; its eventual ack is no
            // RTT sample (transport-design.md §6.3, OPEN note in §10).
            for seg in &mut self.outq {
                let Some(sent) = &mut seg.sent else { break };
                if !seg.acked {
                    sent.held = true;
                }
            }
        }
        if newly_acked {
            self.last_progress = now;
            while self.outq.front().is_some_and(|s| s.acked) {
                if let Some(seg) = self.outq.pop_front() {
                    self.outq_cost -= seg.payload.len() + SEGMENT_QUEUE_COST;
                }
            }
        }
        if let Some(r) = sample {
            self.update_rtt(r);
        }
    }

    /// Marks the segments `ack` covers; reports the RTT sample and whether
    /// any segment was newly acknowledged.
    fn apply_ack(&mut self, ack: Ack, now: Instant) -> (Option<Duration>, bool) {
        if claims_unsent(ack, self.sent_end) {
            // Ignored per segment below; counted once per packet.
            self.stats.invalid_acks += 1;
        }
        let mut sample = None;
        let mut newly_acked = false;
        for seg in &mut self.outq {
            // Sent segments form a prefix; nothing past here can be acked.
            let Some(sent) = &seg.sent else { break };
            let d = seg.seq.wrapping_sub(ack.next).cast_signed();
            let acked = if d < 0 {
                true
            } else if (1..=32).contains(&d) {
                ack.bits & (1u32 << (d - 1)) != 0
            } else if d > 32 {
                break;
            } else {
                false
            };
            if acked && !seg.acked {
                seg.acked = true;
                newly_acked = true;
                // Karn's rule: only sample segments never retransmitted, and
                // never one the peer may have held back while PAUSED.
                if sent.retransmits == 0 && !sent.held {
                    sample = Some(now.saturating_duration_since(sent.first));
                }
            }
        }
        (sample, newly_acked)
    }

    fn update_rtt(&mut self, r: Duration) {
        match self.srtt {
            None => {
                self.srtt = Some(r);
                self.rttvar = r / 2;
            }
            Some(s) => {
                let diff = s.abs_diff(r);
                self.rttvar = (self.rttvar.saturating_mul(3).saturating_add(diff)) / 4;
                self.srtt = Some((s.saturating_mul(7).saturating_add(r)) / 8);
            }
        }
        let srtt = self.srtt.unwrap_or(r);
        let var = self.rttvar.saturating_mul(4).max(Duration::from_millis(1));
        self.rto = srtt
            .saturating_add(var)
            .clamp(self.config.min_rto, self.config.max_rto);
    }

    fn on_segment(&mut self, seg: Segment<'_>) {
        self.ack_pending = true;
        let off = seg.seq.wrapping_sub(self.recv_next);
        if off.cast_signed() < 0 {
            self.stats.duplicates += 1;
            return;
        }
        if off >= self.config.recv_window {
            self.stats.out_of_window += 1;
            return;
        }
        let off = off as usize; // < MAX_WINDOW
        if self.recv_buf.get(off).is_some_and(Option::is_some) {
            self.stats.duplicates += 1;
            return;
        }
        let cost = seg.payload.len() + SEGMENT_QUEUE_COST;
        // The next expected segment is always taken, so a full reorder
        // buffer can delay but never block progress.
        if off > 0 && self.recv_buf_bytes + cost > self.config.max_reorder_bytes {
            self.stats.reorder_dropped += 1;
            return;
        }
        while self.recv_buf.len() <= off {
            self.recv_buf.push_back(None);
        }
        self.recv_buf_bytes += cost;
        self.recv_buf[off] = Some(InSegment {
            msg_id: seg.msg_id,
            frag_index: seg.frag_index,
            frag_count: seg.frag_count,
            payload: seg.payload.to_vec(),
        });
    }

    /// Moves contiguous segments from the receive buffer through reassembly.
    fn deliver_in_order(&mut self, now: Instant) {
        while self.failure.is_none() && !self.inbox_full() {
            if !matches!(self.recv_buf.front(), Some(Some(_))) {
                break;
            }
            let Some(Some(seg)) = self.recv_buf.pop_front() else {
                break;
            };
            self.recv_buf_bytes -= seg.payload.len() + SEGMENT_QUEUE_COST;
            self.recv_next = self.recv_next.wrapping_add(1);
            if let Err(why) = self.reassemble(seg) {
                self.fail(Failure::Violation(why), now);
            }
        }
    }

    /// Back-pressure: in-order delivery is paused while this holds.
    fn inbox_full(&self) -> bool {
        self.inbox_bytes >= self.config.max_inbox_bytes
    }

    fn reassemble(&mut self, seg: InSegment) -> Result<(), &'static str> {
        if seg.frag_count > self.config.max_fragments {
            return Err("fragment count exceeds limit");
        }
        let max = self.config.max_message_size;
        let done = match self.partial.take() {
            None => {
                if seg.frag_index != 0 {
                    return Err("message does not start at fragment 0");
                }
                if seg.payload.len() > max {
                    return Err("message exceeds size limit");
                }
                if seg.frag_count == 1 {
                    Some(seg.payload)
                } else {
                    self.partial = Some(Partial {
                        msg_id: seg.msg_id,
                        frag_count: seg.frag_count,
                        next_index: 1,
                        buf: seg.payload,
                    });
                    None
                }
            }
            Some(mut p) => {
                if seg.msg_id != p.msg_id
                    || seg.frag_count != p.frag_count
                    || seg.frag_index != p.next_index
                {
                    return Err("inconsistent fragment sequence");
                }
                if p.buf.len() + seg.payload.len() > max {
                    return Err("reassembled message exceeds size limit");
                }
                p.buf.extend_from_slice(&seg.payload);
                p.next_index += 1;
                if p.next_index == p.frag_count {
                    Some(p.buf)
                } else {
                    self.partial = Some(p);
                    None
                }
            }
        };
        if let Some(payload) = done {
            self.inbox_bytes += payload.len() + INBOX_ENTRY_COST;
            self.stats.messages_received += 1;
            self.inbox.push_back(Received {
                payload,
                reliable: true,
            });
        }
        Ok(())
    }

    fn current_ack(&self) -> Ack {
        let mut bits = 0u32;
        for (i, slot) in self.recv_buf.iter().enumerate().skip(1).take(32) {
            if slot.is_some() {
                bits |= 1 << (i - 1);
            }
        }
        Ack {
            next: self.recv_next,
            bits,
            paused: self.inbox_full(),
        }
    }

    /// Encodes and sends one packet; send failures count as packet loss.
    fn transmit(&mut self, packet: &Packet<'_>, now: Instant) {
        send_packet(
            &mut self.datagram,
            &mut self.tx_buf,
            &mut self.stats,
            packet,
        );
        self.last_send = now;
        self.ack_pending = false;
    }

    fn flush(&mut self, now: Instant) {
        let ack = self.current_ack();
        if ack.paused != self.advertised_paused {
            // Tell the peer promptly when back-pressure starts or ends.
            self.ack_pending = true;
        }
        let conn_id = self.config.conn_id;
        let window = self.config.send_window as usize;
        let peer_paused = self.peer_paused;
        let last_progress = self.last_progress;
        let mut give_up = false;
        let mut sent_any = false;

        // Retransmit due segments and send new ones within the window. The
        // window is a span of sequence numbers starting at the oldest
        // unacknowledged segment (selectively acked segments inside it still
        // occupy their place), which keeps us inside the peer's receive
        // window.
        {
            let Self {
                outq,
                datagram,
                tx_buf,
                stats,
                config,
                rto,
                sent_end,
                ..
            } = self;
            for seg in outq.iter_mut().take(window) {
                if seg.acked {
                    continue;
                }
                match &mut seg.sent {
                    None => {
                        seg.sent = Some(SentInfo {
                            first: now,
                            last: now,
                            retransmits: 0,
                            // Sent into a paused peer: may be held back.
                            held: peer_paused,
                            strikes: 0,
                            rto: *rto,
                        });
                        *sent_end = seg.seq.wrapping_add(1);
                    }
                    Some(s) => {
                        let deadline = s.last.checked_add(s.rto).unwrap_or(s.last);
                        if now < deadline {
                            continue;
                        }
                        if peer_paused {
                            // The peer is alive but holding data back; the
                            // missing ack is not loss. Wait for it to resume.
                            continue;
                        }
                        // Only a retransmission with no acknowledgement
                        // progress since the last send counts toward the
                        // budget, so a live peer that is slowly draining a
                        // full reorder buffer is not mistaken for a dead one.
                        if last_progress <= s.last {
                            if s.strikes >= config.max_retransmits {
                                give_up = true;
                                break;
                            }
                            s.strikes += 1;
                        }
                        s.retransmits += 1;
                        s.last = now;
                        s.rto = s.rto.saturating_mul(2).min(config.max_rto);
                        stats.retransmits += 1;
                    }
                }
                let packet = Packet {
                    conn_id,
                    ack,
                    close: false,
                    body: Body::Reliable(Segment {
                        seq: seg.seq,
                        msg_id: seg.msg_id,
                        frag_index: seg.frag_index,
                        frag_count: seg.frag_count,
                        payload: &seg.payload,
                    }),
                };
                send_packet(datagram, tx_buf, stats, &packet);
                sent_any = true;
            }
        }
        if sent_any {
            self.last_send = now;
            self.ack_pending = false;
        }
        if give_up {
            self.fail(Failure::PeerUnresponsive, now);
            return;
        }

        while let Some(msg) = self.unreliable_out.pop_front() {
            let packet = Packet {
                conn_id,
                ack,
                close: false,
                body: Body::Unreliable(&msg),
            };
            self.transmit(&packet, now);
        }

        let keepalive_due =
            now.saturating_duration_since(self.last_send) >= self.config.keepalive_interval;
        if self.ack_pending || keepalive_due {
            let packet = Packet {
                conn_id,
                ack,
                close: false,
                body: Body::Empty,
            };
            self.transmit(&packet, now);
        }
        // A change in `ack.paused` forced a packet above, so the peer has
        // been told (best effort; keepalives repeat it).
        self.advertised_paused = ack.paused;
    }
}

/// Encodes `packet` into `buf` and hands it to `datagram`. Send failures are
/// counted and otherwise treated as packet loss.
fn send_packet<D: Datagram>(
    datagram: &mut D,
    buf: &mut Vec<u8>,
    stats: &mut TransportStats,
    packet: &Packet<'_>,
) {
    buf.clear();
    if let Err(e) = packet.encode(buf) {
        // Payload sizes are bounded by construction; this is a logic error.
        tracing::error!(error = %e, "failed to encode packet");
        return;
    }
    match datagram.send(buf) {
        Ok(()) => {
            stats.packets_sent += 1;
            stats.bytes_sent += buf.len() as u64;
        }
        Err(e) => {
            stats.io_errors += 1;
            tracing::debug!(error = %e, "datagram send error");
        }
    }
}

/// Charge against [`TransportConfig::max_send_queue_bytes`] for a message of
/// `len` bytes split into fragments of `frag_payload` bytes.
fn queue_cost(len: usize, frag_payload: usize) -> usize {
    len + len.div_ceil(frag_payload).max(1) * SEGMENT_QUEUE_COST
}

/// Whether `ack` claims any sequence number at or after `sent_end` (one past
/// the highest ever transmitted), cumulatively or selectively.
fn claims_unsent(ack: Ack, sent_end: u32) -> bool {
    let span = sent_end.wrapping_sub(ack.next).cast_signed();
    if span < 0 {
        return true;
    }
    // Bits 0..span-1 name `ack.next + 1 ..= sent_end - 1`, all transmitted.
    let valid_bits = span.cast_unsigned().saturating_sub(1);
    valid_bits < 32 && ack.bits >> valid_bits != 0
}
